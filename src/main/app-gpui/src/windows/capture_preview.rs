//! Capture preview — port of `renderer/windows/capture-preview-window.tsx`
//! and `main/capture/capture-preview/index.ts`. A 200×140 thumbnail docks in
//! the configured corner, stacks up to four deep, and reveals hover chrome.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    div, img, prelude::*, px, size, AnyWindowHandle, App, Bounds, Context, MouseButton, Render,
    Styled, Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};
use parking_lot::Mutex;

use crate::theme::vars::active_theme;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome::{
    self, PreviewCorner, PREVIEW_CONTROL, PREVIEW_CONTROL_INSET, PREVIEW_HEIGHT,
    PREVIEW_HOVER_SCALE, PREVIEW_MAX_STACK, PREVIEW_PILL_HEIGHT, PREVIEW_RADIUS,
    PREVIEW_SHADOW_PADDING, PREVIEW_STACK_GAP, PREVIEW_WIDTH,
};
use crate::ui::icon::icon_element;
use crate::ui::primitives::{
    OVERLAY_ENTER_MS as PREVIEW_ENTER_MS, OVERLAY_ENTER_SLIDE as PREVIEW_ENTER_OFFSET,
    OVERLAY_EXIT_MS as PREVIEW_EXIT_MS,
};

static STACK: Mutex<Option<AnyWindowHandle>> = Mutex::new(None);
static NEXT_PREVIEW_ID: AtomicU64 = AtomicU64::new(1);
const UPLOAD_DONE_DISPLAY_MS: u64 = 800;
const PREVIEW_MOVE_MS: u64 = 120;

#[derive(Clone, Copy)]
struct LayoutAnimation {
    from: f32,
    to: f32,
    started: Instant,
    duration_ms: u64,
}

#[derive(Clone, Copy)]
struct DismissAnimation {
    started: Instant,
    from_opacity: f32,
    controls_progress: Option<f32>,
}

#[derive(Clone, Copy)]
enum DismissBehavior {
    Automatic,
    PreserveControls,
}

struct CapturePreview {
    id: u64,
    path: PathBuf,
    image: Option<Arc<gpui::RenderImage>>,
    /// Electron's scrim is `backdrop-blur-md` over the thumbnail; gpui cannot
    /// blur a region, so the blur is baked once here instead of per frame.
    blurred: Option<Arc<gpui::RenderImage>>,
    /// How far through the 200ms hover transition we are, 0.0 to 1.0.
    /// `transition-transform duration-200` animates in *and* out, so this
    /// decays after the pointer leaves rather than snapping back.
    hover_progress: f32,
    /// When the last frame was drawn, so the step is time-based rather than
    /// frame-rate dependent.
    last_frame: Option<std::time::Instant>,
    hovered: bool,
    busy: bool,
    dismiss_token: Arc<AtomicU64>,
    entered_at: Instant,
    layout_y: Option<f32>,
    layout_animation: Option<LayoutAnimation>,
    dismiss_animation: Option<DismissAnimation>,
}

pub struct CapturePreviewWindow {
    previews: Vec<CapturePreview>,
    layout_generation: u64,
}

impl CapturePreviewWindow {
    fn active_preview_count(&self) -> usize {
        self.previews
            .iter()
            .filter(|preview| preview.dismiss_animation.is_none())
            .count()
    }

    pub fn open(cx: &mut App, path: PathBuf) {
        let id = NEXT_PREVIEW_ID.fetch_add(1, Ordering::Relaxed);
        let dismiss_token = Arc::new(AtomicU64::new(0));
        let mut preview = Some(CapturePreview {
            id,
            image: load_thumbnail(&path),
            blurred: load_blurred_thumbnail(&path),
            path,
            hover_progress: 0.0,
            last_frame: None,
            hovered: false,
            busy: false,
            dismiss_token: dismiss_token.clone(),
            entered_at: Instant::now(),
            layout_y: None,
            layout_animation: None,
            dismiss_animation: None,
        });

        let existing = *STACK.lock();
        if let Some(handle) = existing {
            let updated = handle
                .downcast::<CapturePreviewWindow>()
                .is_some_and(|handle| {
                    handle
                        .update(cx, |view, window, cx| {
                            let Some(preview) = preview.take() else {
                                return;
                            };
                            if view.active_preview_count() >= PREVIEW_MAX_STACK {
                                if let Some(oldest) = view
                                    .previews
                                    .iter()
                                    .find(|preview| preview.dismiss_animation.is_none())
                                    .map(|preview| preview.id)
                                {
                                    begin_remove_preview(
                                        view,
                                        oldest,
                                        DismissBehavior::Automatic,
                                        window,
                                        cx,
                                    );
                                }
                            }
                            view.previews.push(preview);
                            configure_stack_window(window, cx, view.active_preview_count());
                            cx.notify();
                        })
                        .is_ok()
                });
            if updated {
                schedule_auto_dismiss(handle, id, cx, dismiss_token);
                return;
            }
            *STACK.lock() = None;
        }

        let Some(preview) = preview else {
            return;
        };
        let (bounds, display_id) = preview_stack_placement(cx);
        let bottom_aligned = preview_bottom_aligned(cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                focus: false,
                show: !cfg!(windows),
                kind: WindowKind::PopUp,
                is_movable: true,
                is_resizable: false,
                is_minimizable: false,
                display_id,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            |window, cx| {
                #[cfg(windows)]
                if !cfg!(test) {
                    if let Some(hwnd) = crate::windows::window_hwnd(window) {
                        crate::system::window_composition::configure_transparent_surface(hwnd);
                        crate::system::window_composition::stage_window(
                            hwnd,
                            bounds,
                            window.scale_factor(),
                            false,
                        );
                        sync_preview_viewport(window, bounds);
                    }
                }
                let view = cx.new(|_| Self {
                    previews: vec![preview],
                    layout_generation: 0,
                });
                #[cfg(windows)]
                if !cfg!(test) {
                    window.on_next_frame(move |window, _cx| {
                        reveal_preview_when_ready(window, bounds, 1, bottom_aligned);
                    });
                }
                view
            },
        );
        if let Ok(handle) = opened {
            let handle: AnyWindowHandle = handle.into();
            *STACK.lock() = Some(handle);
            schedule_auto_dismiss(handle, id, cx, dismiss_token);
        }
    }
}

fn sync_preview_viewport(window: &mut Window, bounds: Bounds<gpui::Pixels>) {
    window.resize(bounds.size);
}

fn preview_viewport_ready(
    viewport: gpui::Size<gpui::Pixels>,
    bounds: Bounds<gpui::Pixels>,
) -> bool {
    viewport == bounds.size
}

#[cfg(windows)]
fn reveal_preview_when_ready(
    window: &mut Window,
    bounds: Bounds<gpui::Pixels>,
    count: usize,
    bottom_aligned: bool,
) {
    if !preview_viewport_ready(window.viewport_size(), bounds) {
        sync_preview_viewport(window, bounds);
        window.on_next_frame(move |window, _cx| {
            reveal_preview_when_ready(window, bounds, count, bottom_aligned)
        });
        return;
    }
    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        crate::system::window_composition::apply_window_bounds(hwnd, bounds, window.scale_factor());
    }
    configure_preview_stack_window(window, count, bottom_aligned);
    window.on_next_frame(move |window, _cx| {
        if let Some(hwnd) = crate::windows::window_hwnd(window) {
            crate::system::window_composition::reveal_window(hwnd, false, 0);
        }
    });
}

/// How long a blocked dismissal waits before re-checking. The div-level
/// `on_hover` only re-evaluates when a mouse move reaches this window, so a
/// pointer that simply leaves never reports hover end; the retry is what
/// notices the cursor is gone and closes the preview.
const DISMISS_RETRY: Duration = Duration::from_millis(250);

#[derive(Debug, PartialEq, Eq)]
enum DismissVerdict {
    Ready,
    Blocked,
    Gone,
}

fn schedule_auto_dismiss(
    handle: AnyWindowHandle,
    id: u64,
    cx: &mut App,
    dismiss_token: Arc<AtomicU64>,
) {
    let preview = crate::state::state(cx).config.get().preview;
    if !preview.auto_dismiss || preview.auto_dismiss_seconds <= 0.0 {
        return;
    }
    let timeout = Duration::from_secs_f64(preview.auto_dismiss_seconds);
    let generation = dismiss_token.fetch_add(1, Ordering::Relaxed) + 1;
    cx.spawn(async move |cx| {
        cx.background_executor().timer(timeout).await;
        loop {
            let verdict = cx
                .update(|cx| dismiss_verdict(handle, id, generation, cx))
                .unwrap_or(DismissVerdict::Gone);
            match verdict {
                DismissVerdict::Gone => return,
                DismissVerdict::Ready => {
                    let _ = cx
                        .update(|cx| {
                            handle
                                .downcast::<CapturePreviewWindow>()
                                .is_some_and(|handle| {
                                    handle
                                        .update(cx, |view, window, cx| {
                                            begin_remove_preview(
                                                view,
                                                id,
                                                DismissBehavior::Automatic,
                                                window,
                                                cx,
                                            )
                                        })
                                        .is_ok()
                                })
                        })
                        .unwrap_or(false);
                    return;
                }
                DismissVerdict::Blocked => {
                    cx.background_executor().timer(DISMISS_RETRY).await;
                }
            }
        }
    })
    .detach();
}

fn dismiss_verdict(
    handle: AnyWindowHandle,
    id: u64,
    generation: u64,
    cx: &mut App,
) -> DismissVerdict {
    let Some(handle) = handle.downcast::<CapturePreviewWindow>() else {
        return DismissVerdict::Gone;
    };
    handle
        .update(cx, |view, window, _| {
            let Some(preview) = view.previews.iter().find(|preview| preview.id == id) else {
                return DismissVerdict::Gone;
            };
            dismiss_state(
                preview.dismiss_token.load(Ordering::Relaxed),
                generation,
                preview.hovered,
                window.is_window_hovered(),
                preview.busy,
            )
        })
        .unwrap_or(DismissVerdict::Gone)
}

fn dismiss_state(
    current_generation: u64,
    generation: u64,
    hovered: bool,
    window_hovered: bool,
    busy: bool,
) -> DismissVerdict {
    if current_generation != generation {
        return DismissVerdict::Gone;
    }
    if hovered && window_hovered || busy {
        return DismissVerdict::Blocked;
    }
    DismissVerdict::Ready
}

fn selected_display(cx: &mut App) -> Option<(Bounds<gpui::Pixels>, gpui::DisplayId)> {
    let displays = cx.displays();
    let config = crate::state::state(cx).config.get();
    config
        .preview
        .display_id
        .and_then(|id| displays.get(id.max(0) as usize).cloned())
        .or_else(|| displays.first().cloned())
        .map(|display| {
            (
                crate::system::work_area::work_area(display.bounds()),
                display.id(),
            )
        })
}

fn preview_stack_height(count: usize) -> f32 {
    count as f32 * PREVIEW_HEIGHT
        + count.saturating_sub(1) as f32 * PREVIEW_STACK_GAP
        + PREVIEW_SHADOW_PADDING * 2.0
}

fn preview_bottom_aligned(cx: &mut App) -> bool {
    matches!(
        PreviewCorner::parse(&crate::state::state(cx).config.get().preview.corner),
        PreviewCorner::BottomLeft | PreviewCorner::BottomRight
    )
}

fn preview_stack_placement(cx: &mut App) -> (Bounds<gpui::Pixels>, Option<gpui::DisplayId>) {
    let (display, display_id) = selected_display(cx)
        .map(|(bounds, id)| (bounds, Some(id)))
        .unwrap_or_else(|| {
            (
                Bounds::from_corners(
                    gpui::point(px(0.0), px(0.0)),
                    gpui::point(px(1920.0), px(1080.0)),
                ),
                None,
            )
        });
    let config = crate::state::state(cx).config.get();
    let corner = PreviewCorner::parse(&config.preview.corner);
    let index = if matches!(
        corner,
        PreviewCorner::BottomLeft | PreviewCorner::BottomRight
    ) {
        PREVIEW_MAX_STACK.saturating_sub(1)
    } else {
        0
    };
    let (x, y) = chrome::preview_origin(
        f32::from(display.origin.x),
        f32::from(display.origin.y),
        f32::from(display.size.width),
        f32::from(display.size.height),
        index,
        corner,
    );
    (
        Bounds {
            origin: gpui::point(
                px(x - PREVIEW_SHADOW_PADDING),
                px(y - PREVIEW_SHADOW_PADDING),
            ),
            size: size(
                px(PREVIEW_WIDTH + PREVIEW_SHADOW_PADDING * 2.0),
                px(preview_stack_height(PREVIEW_MAX_STACK)),
            ),
        },
        display_id,
    )
}

fn configure_stack_window(window: &mut Window, cx: &mut App, count: usize) {
    let bounds = preview_stack_placement(cx).0;
    #[cfg(windows)]
    if !cfg!(test) {
        if let Some(hwnd) = crate::windows::window_hwnd(window) {
            crate::system::window_composition::apply_window_bounds(
                hwnd,
                bounds,
                window.scale_factor(),
            );
            let bottom_aligned = preview_bottom_aligned(cx);
            window.on_next_frame(move |window, _| {
                configure_preview_stack_window(window, count, bottom_aligned)
            });
        }
    }
    #[cfg(not(windows))]
    let _ = (window, count, bounds);
}

fn configure_preview_stack_window(window: &Window, count: usize, bottom_aligned: bool) {
    let offset = if bottom_aligned {
        PREVIEW_MAX_STACK.saturating_sub(count) as f32 * (PREVIEW_HEIGHT + PREVIEW_STACK_GAP)
    } else {
        0.0
    };
    #[cfg(windows)]
    if !cfg!(test) {
        crate::system::window_composition::set_stacked_rounded_client_region(
            window,
            offset,
            PREVIEW_HEIGHT + PREVIEW_SHADOW_PADDING * 2.0,
            PREVIEW_STACK_GAP - PREVIEW_SHADOW_PADDING * 2.0,
            count,
            PREVIEW_RADIUS + PREVIEW_SHADOW_PADDING,
        );
    }
    #[cfg(not(windows))]
    let _ = (window, count, bottom_aligned, offset);
}

/// One frame of the hover transition: `current` moved towards `target` by
/// however much of `PREVIEW_HOVER_MS` `delta` seconds represents, clamped.
///
/// Split out from `advance_hover` because the stepping is the part worth
/// testing and the rest needs a live `Window`.
fn step_hover(current: f32, target: f32, delta: f32) -> f32 {
    let step = delta / (chrome::PREVIEW_HOVER_MS as f32 / 1000.0);
    if target > current {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}

fn animation_progress(started: Instant, now: Instant, duration_ms: u64) -> f32 {
    now.saturating_duration_since(started).as_secs_f32() / (duration_ms as f32 / 1000.0)
}

fn ease_out(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    1.0 - (1.0 - progress) * (1.0 - progress)
}

fn preview_layout_y(index: usize, bottom_aligned: bool) -> f32 {
    let slot = if bottom_aligned {
        PREVIEW_MAX_STACK - 1 - index
    } else {
        index
    };
    PREVIEW_SHADOW_PADDING + slot as f32 * (PREVIEW_HEIGHT + PREVIEW_STACK_GAP)
}

fn layout_position(animation: LayoutAnimation, now: Instant) -> (f32, bool) {
    let progress = animation_progress(animation.started, now, animation.duration_ms);
    let position = animation.from + (animation.to - animation.from) * ease_out(progress);
    (position, progress < 1.0)
}

fn advance_layout(
    preview: &mut CapturePreview,
    target: f32,
    enter_offset: f32,
    now: Instant,
) -> (f32, bool) {
    if preview.layout_y.is_none() {
        let from = target + enter_offset;
        preview.layout_y = Some(from);
        preview.layout_animation = Some(LayoutAnimation {
            from,
            to: target,
            started: preview.entered_at,
            duration_ms: PREVIEW_ENTER_MS,
        });
    }

    let target_changed = preview
        .layout_animation
        .map(|animation| (animation.to - target).abs() >= f32::EPSILON)
        .unwrap_or_else(|| {
            preview
                .layout_y
                .is_some_and(|position| (position - target).abs() >= f32::EPSILON)
        });
    if target_changed {
        let current = preview
            .layout_animation
            .map(|animation| layout_position(animation, now).0)
            .or(preview.layout_y)
            .unwrap_or(target);
        preview.layout_y = Some(current);
        preview.layout_animation = Some(LayoutAnimation {
            from: current,
            to: target,
            started: now,
            duration_ms: PREVIEW_MOVE_MS,
        });
    }

    let Some(animation) = preview.layout_animation else {
        return (preview.layout_y.unwrap_or(target), false);
    };
    let (position, animating) = layout_position(animation, now);
    preview.layout_y = Some(position);
    if !animating {
        preview.layout_y = Some(animation.to);
        preview.layout_animation = None;
    }
    (preview.layout_y.unwrap_or(target), animating)
}

fn preview_opacity(preview: &CapturePreview, now: Instant) -> (f32, bool) {
    if let Some(animation) = preview.dismiss_animation {
        let progress = animation_progress(animation.started, now, PREVIEW_EXIT_MS);
        return (
            animation.from_opacity * (1.0 - ease_out(progress)),
            progress < 1.0,
        );
    }
    let progress = animation_progress(preview.entered_at, now, PREVIEW_ENTER_MS);
    (ease_out(progress), progress < 1.0)
}

fn start_dismiss_animation(
    preview: &CapturePreview,
    behavior: DismissBehavior,
    now: Instant,
) -> DismissAnimation {
    DismissAnimation {
        started: now,
        from_opacity: preview_opacity(preview, now).0,
        controls_progress: match behavior {
            DismissBehavior::Automatic => None,
            DismissBehavior::PreserveControls => Some(preview.hover_progress),
        },
    }
}

/// Tailwind's `backdrop-blur-md` is `blur(12px)`, and a CSS filter blur takes a
/// standard deviation, so the sigma is the radius.
const BLUR_SIGMA: f32 = 12.0;

/// The thumbnail, scaled to the size it is drawn at and then blurred.
///
/// Scaling first matters: `backdrop-blur-md` blurs the *rendered* pixels, so
/// blurring a 3440x1440 capture and then shrinking it to 200x140 would come out
/// almost sharp.
fn load_blurred_thumbnail(path: &PathBuf) -> Option<Arc<gpui::RenderImage>> {
    let bytes = std::fs::read(path).ok()?;
    let decoded = image::load_from_memory(&bytes).ok()?;
    // `object_fit: Cover` crops rather than squashes, so match it here.
    let scaled = decoded
        .resize_to_fill(
            PREVIEW_WIDTH as u32,
            PREVIEW_HEIGHT as u32,
            image::imageops::FilterType::Triangle,
        )
        .to_rgba8();

    let mut pixmap = crate::editor::export::from_rgba(&scaled)?;
    crate::render::blur::blur(&mut pixmap, BLUR_SIGMA);
    let mut buffer = crate::editor::export::to_rgba(&pixmap);
    // `RenderImage` wants BGRA, the same swap `load_thumbnail` makes.
    for pixel in buffer.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let frame = image::Frame::new(buffer);
    Some(Arc::new(gpui::RenderImage::new(smallvec::smallvec![frame])))
}

fn load_thumbnail(path: &PathBuf) -> Option<Arc<gpui::RenderImage>> {
    let bytes = std::fs::read(path).ok()?;
    let decoded = image::load_from_memory(&bytes).ok()?;
    let mut buffer = decoded
        .resize_to_fill(
            PREVIEW_WIDTH as u32,
            PREVIEW_HEIGHT as u32,
            image::imageops::FilterType::Triangle,
        )
        .to_rgba8();
    for pixel in buffer.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let frame = image::Frame::new(buffer);
    Some(Arc::new(gpui::RenderImage::new(smallvec::smallvec![frame])))
}

/// The preset the user starred, which is what `usePolishCopy` looks up via
/// `wallpaper.defaultPresetId`. `None` -- the default -- means Electron shows no
/// Polish button either, so neither does this.
fn starred_preset(cx: &mut App) -> Option<crate::config::schema::WallpaperPreset> {
    let wallpaper = crate::state::state(cx).config.get().wallpaper;
    let id = wallpaper.default_preset_id.as_deref()?;
    wallpaper
        .presets
        .iter()
        .find(|preset| preset.id == id)
        .cloned()
}

/// Composes the capture with the starred preset and puts the result on the
/// clipboard -- `polish` in `use-polish-copy.ts`, which renders the capture over
/// the preset's wallpaper and copies it rather than saving it.
fn polish_and_copy(path: &Path, cx: &mut App) -> bool {
    let Some(preset) = starred_preset(cx) else {
        return false;
    };
    let Ok(bytes) = std::fs::read(path) else {
        report_copy_failure(cx, &format!("could not read {}", path.display()));
        return false;
    };
    let Ok(decoded) = image::load_from_memory(&bytes) else {
        report_copy_failure(cx, "could not decode the capture");
        return false;
    };

    let mut settings = crate::editor::wallpaper::WallpaperSettings::default();
    crate::editor::wallpaper::apply_preset(&mut settings, &preset);
    let ((canvas_width, canvas_height), _) = crate::editor::wallpaper::layout(
        &settings,
        decoded.width() as f64,
        decoded.height() as f64,
    );
    let canvas = crate::editor::export::compose(
        Some(&decoded),
        canvas_width.max(1.0) as u32,
        canvas_height.max(1.0) as u32,
        &[],
        &settings,
    );

    let (width, height) = (canvas.width() as usize, canvas.height() as usize);
    let copied = arboard::Clipboard::new()
        .and_then(|mut clipboard| {
            clipboard.set_image(arboard::ImageData {
                width,
                height,
                bytes: std::borrow::Cow::Owned(canvas.into_raw()),
            })
        })
        .is_ok();
    if !copied {
        report_copy_failure(cx, "could not reach the clipboard");
    }
    copied
}

fn copy_image(path: &Path, cx: &mut App) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        report_copy_failure(cx, &format!("could not read {}", path.display()));
        return false;
    };
    let Ok(decoded) = image::load_from_memory(&bytes) else {
        report_copy_failure(cx, "could not decode the capture");
        return false;
    };
    let rgba = decoded.to_rgba8();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    let copied = arboard::Clipboard::new()
        .and_then(|mut clipboard| {
            clipboard.set_image(arboard::ImageData {
                width,
                height,
                bytes: std::borrow::Cow::Owned(rgba.into_raw()),
            })
        })
        .is_ok();
    if !copied {
        report_copy_failure(cx, "could not reach the clipboard");
    }
    copied
}

fn report_copy_failure(cx: &mut App, message: &str) {
    crate::windows::toast::Toast::show(cx, "Copy failed", message.to_string());
}

fn delete_capture(path: &Path) {
    let path_str = path.to_string_lossy().to_string();
    let mut items = crate::history_store::load_history();
    items.retain(|item| item.original_path != path_str);
    crate::history_store::save_history(&items);
    let _ = std::fs::remove_file(path);
}

fn begin_remove_preview(
    view: &mut CapturePreviewWindow,
    id: u64,
    behavior: DismissBehavior,
    window: &mut Window,
    cx: &mut Context<CapturePreviewWindow>,
) {
    let now = Instant::now();
    let Some(index) = view.previews.iter().position(|preview| preview.id == id) else {
        return;
    };
    if view.previews[index].dismiss_animation.is_some() {
        return;
    }
    let active_index = view.previews[..index]
        .iter()
        .filter(|preview| preview.dismiss_animation.is_none())
        .count();
    let initial_y = preview_layout_y(active_index, preview_bottom_aligned(cx));
    let preview = &mut view.previews[index];
    if preview.layout_y.is_none() {
        preview.layout_y = Some(initial_y);
    }
    preview.dismiss_token.fetch_add(1, Ordering::Relaxed);
    preview.dismiss_animation = Some(start_dismiss_animation(preview, behavior, now));
    let bottom_aligned = preview_bottom_aligned(cx);
    let layout_generation = start_layout_transition(view, bottom_aligned, now);
    cx.notify();
    let removal_handle = window.window_handle();
    cx.spawn(async move |_, cx| {
        cx.background_executor()
            .timer(Duration::from_millis(PREVIEW_EXIT_MS))
            .await;
        let _ = cx.update(|cx| {
            let Some(handle) = removal_handle.downcast::<CapturePreviewWindow>() else {
                return;
            };
            let _ = handle.update(cx, |view, window, cx| {
                remove_preview_now(view, id, window, cx)
            });
        });
    })
    .detach();
    let region_handle = window.window_handle();
    cx.spawn(async move |_, cx| {
        cx.background_executor()
            .timer(Duration::from_millis(PREVIEW_MOVE_MS))
            .await;
        let _ = cx.update(|cx| {
            let Some(handle) = region_handle.downcast::<CapturePreviewWindow>() else {
                return;
            };
            let _ = handle.update(cx, |view, window, cx| {
                if view.layout_generation != layout_generation {
                    return;
                }
                configure_preview_stack_window(
                    window,
                    view.active_preview_count(),
                    preview_bottom_aligned(cx),
                );
            });
        });
    })
    .detach();
}

fn start_layout_transition(
    view: &mut CapturePreviewWindow,
    bottom_aligned: bool,
    now: Instant,
) -> u64 {
    let mut active_index = 0;
    for preview in &mut view.previews {
        if preview.dismiss_animation.is_some() {
            continue;
        }
        let target = preview_layout_y(active_index, bottom_aligned);
        active_index += 1;
        let current = preview.layout_y.unwrap_or(target);
        if (current - target).abs() < f32::EPSILON {
            continue;
        }
        preview.layout_animation = Some(LayoutAnimation {
            from: current,
            to: target,
            started: now,
            duration_ms: PREVIEW_MOVE_MS,
        });
    }
    view.layout_generation = view.layout_generation.wrapping_add(1);
    view.layout_generation
}

fn remove_preview_now(
    view: &mut CapturePreviewWindow,
    id: u64,
    window: &mut Window,
    cx: &mut Context<CapturePreviewWindow>,
) {
    view.previews.retain(|preview| preview.id != id);
    if view.previews.is_empty() {
        *STACK.lock() = None;
        window.remove_window();
        return;
    }
    cx.notify();
}

impl CapturePreviewWindow {
    fn show_controls(hovered: bool, busy: bool) -> bool {
        hovered || busy
    }

    fn advance_hover(preview: &mut CapturePreview, hovered: bool, now: Instant) -> (f32, bool) {
        let target = if Self::show_controls(hovered, preview.busy) {
            1.0
        } else {
            0.0
        };
        if (preview.hover_progress - target).abs() < f32::EPSILON {
            preview.last_frame = None;
            return (preview.hover_progress, false);
        }

        let delta = preview
            .last_frame
            .map(|previous| now.saturating_duration_since(previous).as_secs_f32())
            .unwrap_or(0.0);
        preview.last_frame = Some(now);

        preview.hover_progress = step_hover(preview.hover_progress, target, delta);
        let animating = (preview.hover_progress - target).abs() >= f32::EPSILON;
        if !animating {
            preview.last_frame = None;
        }
        (preview.hover_progress, animating)
    }

    fn control_frame(
        preview: &mut CapturePreview,
        window_hovered: bool,
        now: Instant,
    ) -> (bool, f32, bool) {
        if let Some(progress) = preview
            .dismiss_animation
            .and_then(|animation| animation.controls_progress)
        {
            preview.last_frame = None;
            return (true, progress, false);
        }
        let hovered = preview.hovered && window_hovered;
        let (progress, animating) = Self::advance_hover(preview, hovered, now);
        (
            Self::show_controls(hovered, preview.busy),
            progress,
            animating,
        )
    }

    /// The corner controls: Electron's `size-6 rounded-full bg-background/80`
    /// with a 14px glyph.
    fn circle_button(
        id: impl Into<gpui::ElementId>,
        icon: &'static str,
        busy: bool,
        tooltip: impl Into<gpui::SharedString>,
        theme: &crate::theme::vars::ThemeVars,
        hover_bg: gpui::Hsla,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> gpui::AnyElement {
        Self::chip(
            id,
            ButtonSize::IconXs,
            px(PREVIEW_CONTROL),
            theme,
            hover_bg,
            busy,
            tooltip,
            on_click,
        )
        .icon(icon)
        .icon_spinning(busy)
        .into_any_element()
    }

    /// The two centre actions: `rounded-full bg-background/80 px-3 py-1 text-xs
    /// font-medium hover:bg-primary`.
    fn pill_button(
        id: impl Into<gpui::ElementId>,
        label: &'static str,
        tooltip: impl Into<gpui::SharedString>,
        theme: &crate::theme::vars::ThemeVars,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> gpui::AnyElement {
        Self::chip(
            id,
            ButtonSize::Xs,
            px(PREVIEW_PILL_HEIGHT),
            theme,
            theme.primary,
            false,
            tooltip,
            on_click,
        )
        .label(label)
        .padding_x(px(chrome::BUTTON_SM_PAD_X))
        .into_any_element()
    }

    fn chip(
        id: impl Into<gpui::ElementId>,
        size: ButtonSize,
        height: gpui::Pixels,
        theme: &crate::theme::vars::ThemeVars,
        hover_bg: gpui::Hsla,
        disabled: bool,
        tooltip: impl Into<gpui::SharedString>,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Button {
        Button::new(id)
            .variant(ButtonVariant::Ghost)
            .size(size)
            .height(height)
            .radius(px(f32::from(height) / 2.0))
            .surface(theme.background.opacity(0.8))
            .surface_hover(hover_bg)
            .foreground(theme.foreground)
            .disabled(disabled)
            .tooltip(tooltip)
            .on_press(move |_event, window, cx| {
                on_click(window, cx);
                cx.stop_propagation();
            })
    }
}

impl CapturePreviewWindow {
    fn render_preview(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
        theme: &crate::theme::vars::ThemeVars,
        polish: Option<&crate::config::schema::WallpaperPreset>,
        display_count: usize,
        window_hovered: bool,
        target_y: f32,
        enter_offset: f32,
        now: Instant,
        needs_frame: &mut bool,
    ) -> gpui::AnyElement {
        let preview_entity = cx.entity().downgrade();
        let preview = &mut self.previews[index];
        let id = preview.id;
        let path = preview.path.clone();
        let dismiss_token = preview.dismiss_token.clone();
        let busy = preview.busy;
        let (show_controls, progress, animating) =
            Self::control_frame(preview, window_hovered, now);
        let (layout_y, layout_animating) = advance_layout(preview, target_y, enter_offset, now);
        let (opacity, opacity_animating) = preview_opacity(preview, now);
        *needs_frame |= animating || layout_animating || opacity_animating;
        // `scale-105` over `duration-200` rather than an instant jump.
        let scale = 1.0 + (PREVIEW_HOVER_SCALE - 1.0) * progress;
        let image_w = PREVIEW_WIDTH * scale;
        let image_h = PREVIEW_HEIGHT * scale;
        let has_multiple_displays = display_count > 1;

        let mut root = div()
            .id(("capture-preview", id))
            .relative()
            .size_full()
            .overflow_hidden()
            .rounded(px(PREVIEW_RADIUS))
            .bg(theme.muted_background)
            .border_1()
            .border_color(theme.border)
            .on_hover({
                let preview_entity = preview_entity.clone();
                move |hovered: &bool, window, cx| {
                    let handle = window.window_handle();
                    let _ = preview_entity.update(cx, |view, cx| {
                        let Some(preview) =
                            view.previews.iter_mut().find(|preview| preview.id == id)
                        else {
                            return;
                        };
                        preview.hovered = *hovered;
                        cx.notify();
                    });
                    schedule_auto_dismiss(handle, id, cx, dismiss_token.clone());
                }
            })
            .on_mouse_down(MouseButton::Left, {
                let preview_entity = preview_entity.clone();
                let path = path.clone();
                move |event: &gpui::MouseDownEvent, window, cx| {
                    if event.click_count < 2 {
                        return;
                    }
                    let path = path.to_string_lossy().into_owned();
                    let _ = preview_entity.update(cx, |view, cx| {
                        begin_remove_preview(
                            view,
                            id,
                            DismissBehavior::PreserveControls,
                            window,
                            cx,
                        )
                    });
                    crate::open_editor_for(cx, &path);
                }
            });

        // `capture-preview-window.tsx` lays a `bg-black/25 backdrop-blur-md`
        // scrim over the thumbnail while the controls show. The tint is drawn
        // below; this is the blur half of it.
        let thumbnail = if show_controls {
            preview.blurred.as_ref().or(preview.image.as_ref())
        } else {
            preview.image.as_ref()
        };
        root = root.child(match thumbnail {
            Some(render_image) => div()
                .absolute()
                .inset_0()
                .overflow_hidden()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    img(render_image.clone())
                        .w(px(image_w))
                        .h(px(image_h))
                        .object_fit(gpui::ObjectFit::Cover),
                )
                .into_any_element(),
            None => div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.muted_background)
                .text_color(theme.muted_foreground)
                .child(icon_element("image", px(48.0)))
                .into_any_element(),
        });

        if show_controls {
            root = root
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        // `bg-black/25` behind `animate-in fade-in duration-200`.
                        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.25 * progress)),
                )
                .child(
                    div()
                        .absolute()
                        .opacity(progress)
                        .top(px(PREVIEW_CONTROL_INSET))
                        .left(px(PREVIEW_CONTROL_INSET))
                        .child(Self::circle_button(
                            ("preview-close", id),
                            "x",
                            false,
                            "Close preview",
                            theme,
                            theme.destructive,
                            {
                                let preview_entity = preview_entity.clone();
                                move |window, cx| {
                                    let _ = preview_entity.update(cx, |view, cx| {
                                        begin_remove_preview(
                                            view,
                                            id,
                                            DismissBehavior::PreserveControls,
                                            window,
                                            cx,
                                        )
                                    });
                                }
                            },
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .opacity(progress)
                        .top(px(PREVIEW_CONTROL_INSET))
                        .right(px(PREVIEW_CONTROL_INSET))
                        .child(Self::circle_button(
                            ("preview-delete", id),
                            "trash-2",
                            false,
                            "Delete screenshot",
                            theme,
                            theme.destructive,
                            {
                                let path = path.clone();
                                let preview_entity = preview_entity.clone();
                                move |window, cx| {
                                    delete_capture(&path);
                                    let _ = preview_entity.update(cx, |view, cx| {
                                        begin_remove_preview(
                                            view,
                                            id,
                                            DismissBehavior::PreserveControls,
                                            window,
                                            cx,
                                        )
                                    });
                                }
                            },
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .opacity(progress)
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        // `flex-col gap-1`: Polish stacks above Edit when a
                        // preset is starred, and is absent otherwise -- which is
                        // also what Electron renders with no starred preset.
                        .flex_col()
                        .gap(px(4.0))
                        .when_some(polish, |el, preset| {
                            let tooltip = format!("Copy with \"{}\"", preset.name);
                            el.child(Self::pill_button(
                                ("preview-polish", id),
                                "Polish",
                                tooltip,
                                theme,
                                {
                                    let path = path.clone();
                                    let preview_entity = preview_entity.clone();
                                    move |window, cx| {
                                        if polish_and_copy(&path, cx) {
                                            let _ = preview_entity.update(cx, |view, cx| {
                                                begin_remove_preview(
                                                    view,
                                                    id,
                                                    DismissBehavior::PreserveControls,
                                                    window,
                                                    cx,
                                                )
                                            });
                                        }
                                    }
                                },
                            ))
                        })
                        .child(Self::pill_button(
                            ("preview-edit", id),
                            "Edit",
                            "Edit",
                            theme,
                            {
                                let path = path.clone();
                                let preview_entity = preview_entity.clone();
                                move |window, cx| {
                                    let path = path.to_string_lossy().into_owned();
                                    let _ = preview_entity.update(cx, |view, cx| {
                                        begin_remove_preview(
                                            view,
                                            id,
                                            DismissBehavior::PreserveControls,
                                            window,
                                            cx,
                                        )
                                    });
                                    crate::open_editor_for(cx, &path);
                                }
                            },
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .opacity(progress)
                        .bottom(px(PREVIEW_CONTROL_INSET))
                        .left(px(PREVIEW_CONTROL_INSET))
                        .child(Self::circle_button(
                            ("preview-copy", id),
                            "copy",
                            false,
                            "Copy",
                            theme,
                            theme.primary,
                            {
                                let path = path.clone();
                                let preview_entity = preview_entity.clone();
                                move |window, cx| {
                                    if copy_image(&path, cx) {
                                        let _ = preview_entity.update(cx, |view, cx| {
                                            begin_remove_preview(
                                                view,
                                                id,
                                                DismissBehavior::PreserveControls,
                                                window,
                                                cx,
                                            )
                                        });
                                    }
                                    cx.stop_propagation();
                                }
                            },
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .opacity(progress)
                        .bottom(px(PREVIEW_CONTROL_INSET))
                        .right(px(PREVIEW_CONTROL_INSET))
                        .child(Self::circle_button(
                            ("preview-upload", id),
                            "cloud-upload",
                            busy,
                            "Upload to Cloud",
                            theme,
                            theme.primary,
                            {
                                let path = path.clone();
                                let preview_entity = preview_entity.clone();
                                move |window, cx| {
                                    let config = crate::state::state(cx).config.get().cloud;
                                    let path = path.clone();
                                    let handle = window.window_handle();
                                    let dismiss_token = preview_entity
                                        .update(cx, |view, cx| {
                                            let preview = view
                                                .previews
                                                .iter_mut()
                                                .find(|preview| preview.id == id)?;
                                            if preview.busy {
                                                return None;
                                            }
                                            preview.busy = true;
                                            preview.dismiss_token.fetch_add(1, Ordering::Relaxed);
                                            cx.notify();
                                            Some(preview.dismiss_token.clone())
                                        })
                                        .ok()
                                        .flatten();
                                    let Some(dismiss_token) = dismiss_token else {
                                        return;
                                    };
                                    let task_entity = preview_entity.clone();
                                    cx.spawn(async move |cx| {
                                        let result = cx
                                            .background_executor()
                                            .spawn(
                                                async move { crate::cloud::upload(&config, &path) },
                                            )
                                            .await;
                                        let succeeded = result.is_ok();
                                        let _ = cx.update(|cx| {
                                            let _ = task_entity.update(cx, |view, cx| {
                                                if !succeeded {
                                                    if let Some(preview) = view
                                                        .previews
                                                        .iter_mut()
                                                        .find(|preview| preview.id == id)
                                                    {
                                                        preview.busy = false;
                                                    }
                                                }
                                                cx.notify();
                                            });
                                            match result {
                                                Ok(url) => {
                                                    let _ = arboard::Clipboard::new().and_then(
                                                        |mut clipboard| {
                                                            clipboard.set_text(url.clone())
                                                        },
                                                    );
                                                    crate::windows::toast::Toast::show(
                                                        cx,
                                                        "Link copied",
                                                        url,
                                                    );
                                                }
                                                Err(error) => crate::windows::toast::Toast::show(
                                                    cx,
                                                    "Upload failed",
                                                    error.to_string(),
                                                ),
                                            }
                                        });
                                        if succeeded {
                                            cx.background_executor()
                                                .timer(Duration::from_millis(
                                                    UPLOAD_DONE_DISPLAY_MS,
                                                ))
                                                .await;
                                            let _ = cx.update(|cx| {
                                                let Some(preview) =
                                                    handle.downcast::<CapturePreviewWindow>()
                                                else {
                                                    return;
                                                };
                                                let _ = preview.update(cx, |view, window, cx| {
                                                    begin_remove_preview(
                                                        view,
                                                        id,
                                                        DismissBehavior::PreserveControls,
                                                        window,
                                                        cx,
                                                    )
                                                });
                                            });
                                        } else {
                                            let _ = cx.update(|cx| {
                                                schedule_auto_dismiss(handle, id, cx, dismiss_token)
                                            });
                                        }
                                    })
                                    .detach();
                                    cx.stop_propagation();
                                }
                            },
                        )),
                );

            if has_multiple_displays {
                root = root.child(
                    div()
                        .absolute()
                        .opacity(progress)
                        .bottom(px(PREVIEW_CONTROL_INSET + PREVIEW_CONTROL + 4.0))
                        .right(px(PREVIEW_CONTROL_INSET))
                        .child(Self::circle_button(
                            ("preview-pin-display", id),
                            "monitor",
                            false,
                            "Move previews to another display",
                            theme,
                            theme.primary,
                            {
                                let count = display_count;
                                move |_, cx| {
                                    let config = crate::state::state(cx).config.clone();
                                    config.update(|settings| {
                                        let current = settings.preview.display_id.unwrap_or(0);
                                        settings.preview.display_id =
                                            Some((current + 1).rem_euclid(count as i64));
                                    });
                                    cx.stop_propagation();
                                }
                            },
                        )),
                );
            }
        }

        div()
            .absolute()
            .top(px(layout_y))
            .left(px(PREVIEW_SHADOW_PADDING))
            .w(px(PREVIEW_WIDTH))
            .h(px(PREVIEW_HEIGHT))
            .opacity(opacity)
            .rounded(px(PREVIEW_RADIUS))
            .shadow_sm()
            .child(root)
            .into_any_element()
    }
}

impl Render for CapturePreviewWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let corner = PreviewCorner::parse(&crate::state::state(cx).config.get().preview.corner);
        let theme = active_theme(cx);
        let polish = starred_preset(cx);
        let display_count = cx.displays().len();
        let window_hovered = window.is_window_hovered();
        let bottom_aligned = matches!(
            corner,
            PreviewCorner::BottomLeft | PreviewCorner::BottomRight
        );
        let enter_offset = if bottom_aligned {
            -PREVIEW_ENTER_OFFSET
        } else {
            PREVIEW_ENTER_OFFSET
        };
        let now = Instant::now();
        let mut needs_frame = false;
        let mut root = div().relative().size_full();
        let mut active_index = 0;
        for index in 0..self.previews.len() {
            let target_y = if self.previews[index].dismiss_animation.is_some() {
                self.previews[index]
                    .layout_y
                    .unwrap_or_else(|| preview_layout_y(active_index, bottom_aligned))
            } else {
                let target = preview_layout_y(active_index, bottom_aligned);
                active_index += 1;
                target
            };
            root = root.child(self.render_preview(
                index,
                cx,
                &theme,
                polish.as_ref(),
                display_count,
                window_hovered,
                target_y,
                enter_offset,
                now,
                &mut needs_frame,
            ));
        }
        if needs_frame {
            crate::ui::primitives::request_animation_frame(window);
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_height_preserves_card_size_and_shadow_gutters() {
        assert_eq!(
            preview_stack_height(1),
            PREVIEW_HEIGHT + PREVIEW_SHADOW_PADDING * 2.0
        );
        assert_eq!(
            preview_stack_height(2),
            PREVIEW_HEIGHT * 2.0 + PREVIEW_STACK_GAP + PREVIEW_SHADOW_PADDING * 2.0
        );
        assert_eq!(
            preview_stack_height(3),
            PREVIEW_HEIGHT * 3.0 + PREVIEW_STACK_GAP * 2.0 + PREVIEW_SHADOW_PADDING * 2.0
        );
    }

    #[test]
    fn fixed_stack_slots_anchor_to_the_selected_edge() {
        let step = PREVIEW_HEIGHT + PREVIEW_STACK_GAP;
        assert_eq!(preview_layout_y(0, false), PREVIEW_SHADOW_PADDING);
        assert_eq!(
            preview_layout_y(2, false),
            PREVIEW_SHADOW_PADDING + step * 2.0
        );
        assert_eq!(
            preview_layout_y(0, true),
            PREVIEW_SHADOW_PADDING + step * 3.0
        );
        assert_eq!(preview_layout_y(2, true), PREVIEW_SHADOW_PADDING + step);
    }

    #[test]
    fn layout_motion_is_time_based() {
        let now = Instant::now();
        let animation = LayoutAnimation {
            from: 0.0,
            to: 100.0,
            started: now - Duration::from_millis(PREVIEW_MOVE_MS / 2),
            duration_ms: PREVIEW_MOVE_MS,
        };
        let (position, animating) = layout_position(animation, now);
        assert!((position - 75.0).abs() < 0.01);
        assert!(animating);
    }

    #[test]
    fn newer_layout_invalidates_an_older_region_completion() {
        let mut view = CapturePreviewWindow {
            previews: Vec::new(),
            layout_generation: 0,
        };
        let first = start_layout_transition(&mut view, true, Instant::now());
        let second = start_layout_transition(&mut view, true, Instant::now());
        assert_ne!(first, second);
        assert_eq!(view.layout_generation, second);
    }

    #[test]
    fn interactive_dismissal_preserves_controls_after_window_hover_ends() {
        let now = Instant::now();
        let mut preview = CapturePreview {
            id: 1,
            path: PathBuf::new(),
            image: None,
            blurred: None,
            hover_progress: 0.75,
            last_frame: Some(now),
            hovered: true,
            busy: false,
            dismiss_token: Arc::new(AtomicU64::new(0)),
            entered_at: now,
            layout_y: None,
            layout_animation: None,
            dismiss_animation: None,
        };
        preview.dismiss_animation = Some(start_dismiss_animation(
            &preview,
            DismissBehavior::PreserveControls,
            now,
        ));

        assert_eq!(
            CapturePreviewWindow::control_frame(&mut preview, false, now),
            (true, 0.75, false)
        );
    }

    /// The whole point of the transition is that it takes 200ms, so a full
    /// second of frames must not overshoot and a single frame must not jump.
    #[test]
    fn the_hover_transition_takes_two_hundred_milliseconds() {
        // 200ms in one go is exactly the whole way.
        assert_eq!(step_hover(0.0, 1.0, 0.200), 1.0);
        // Half the duration is half the distance.
        assert_eq!(step_hover(0.0, 1.0, 0.100), 0.5);
        // A 60fps frame moves 16.6ms worth, not the whole distance.
        let frame = step_hover(0.0, 1.0, 1.0 / 60.0);
        assert!(frame > 0.08 && frame < 0.09, "one frame moved {frame}");
    }

    /// `transition-transform` animates out as well as in, and neither
    /// direction may overshoot its target.
    #[test]
    fn the_hover_transition_reverses_and_never_overshoots() {
        assert_eq!(step_hover(1.0, 0.0, 0.200), 0.0, "eases back out");
        assert_eq!(step_hover(0.5, 0.0, 10.0), 0.0, "a long frame clamps at 0");
        assert_eq!(step_hover(0.5, 1.0, 10.0), 1.0, "and clamps at 1");
        assert_eq!(step_hover(0.3, 0.3, 0.016), 0.3, "already there, no drift");
    }

    #[gpui::test]
    fn the_preview_closes_itself_when_the_dismiss_timer_elapses(cx: &mut gpui::TestAppContext) {
        use crate::config::store::ConfigStore;

        let dir = tempfile::tempdir().expect("temp dir");
        let store = std::sync::Arc::new(
            ConfigStore::load_at(dir.path().join("config.json")).expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));

        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(60, 40, image::Rgba([10, 20, 30, 255]))
            .save(&path)
            .expect("write capture");

        for _ in 0..PREVIEW_MAX_STACK {
            cx.update(|cx| CapturePreviewWindow::open(cx, path.clone()));
            cx.run_until_parked();
        }
        let handle = STACK
            .lock()
            .expect("the preview opened the shared stack window");
        handle
            .downcast::<CapturePreviewWindow>()
            .expect("preview stack")
            .update(cx, |view, _, _| {
                view.previews.first_mut().expect("oldest preview").hovered = true;
            })
            .expect("hover oldest preview");
        cx.update(|cx| CapturePreviewWindow::open(cx, path.clone()));
        cx.run_until_parked();
        let (
            total,
            active,
            oldest_id,
            oldest_hovered,
            survivor_id,
            survivor_started,
            layout_generation,
        ) = handle
            .downcast::<CapturePreviewWindow>()
            .expect("preview stack")
            .update(cx, |view, _, _| {
                let oldest = view.previews.first().expect("oldest preview");
                let survivor = view
                    .previews
                    .iter()
                    .find(|preview| {
                        preview.dismiss_animation.is_none() && preview.layout_animation.is_some()
                    })
                    .expect("moving survivor");
                (
                    view.previews.len(),
                    view.active_preview_count(),
                    oldest.id,
                    oldest.hovered,
                    survivor.id,
                    survivor.layout_animation.expect("layout animation").started,
                    view.layout_generation,
                )
            })
            .expect("read preview stack");
        assert_eq!(total, PREVIEW_MAX_STACK + 1);
        assert_eq!(active, PREVIEW_MAX_STACK);
        assert!(oldest_hovered);
        let (started_after_removal, generation_after_removal) = handle
            .downcast::<CapturePreviewWindow>()
            .expect("preview stack")
            .update(cx, |view, window, cx| {
                remove_preview_now(view, oldest_id, window, cx);
                let survivor = view
                    .previews
                    .iter()
                    .find(|preview| preview.id == survivor_id)
                    .expect("survivor remains");
                (
                    survivor.layout_animation.expect("layout animation").started,
                    view.layout_generation,
                )
            })
            .expect("remove oldest preview");
        assert_eq!(started_after_removal, survivor_started);
        assert_eq!(generation_after_removal, layout_generation);

        cx.executor().advance_clock(Duration::from_secs(11));
        cx.run_until_parked();
        cx.executor()
            .advance_clock(Duration::from_millis(PREVIEW_EXIT_MS));
        cx.run_until_parked();
        assert!(
            handle.update(cx, |_, _, _| ()).is_err(),
            "the preview must dismiss after the configured delay"
        );
    }

    #[test]
    fn auto_dismiss_is_blocked_by_interaction() {
        assert_eq!(
            [
                dismiss_state(4, 4, true, true, false),
                dismiss_state(4, 4, false, false, true),
            ],
            [DismissVerdict::Blocked, DismissVerdict::Blocked]
        );
    }

    #[test]
    fn stale_auto_dismiss_generation_is_gone() {
        assert_eq!(
            dismiss_state(5, 4, false, false, false),
            DismissVerdict::Gone
        );
    }

    #[test]
    fn leaving_the_stack_unblocks_auto_dismiss() {
        assert_eq!(
            dismiss_state(4, 4, true, false, false),
            DismissVerdict::Ready
        );
    }

    #[test]
    fn window_exit_and_reentry_gate_the_stored_hover() {
        let stored_hover = true;
        let visible = [false, true].map(|window_hovered| {
            CapturePreviewWindow::show_controls(stored_hover && window_hovered, false)
        });
        assert_eq!(visible, [false, true]);
    }

    #[test]
    fn stale_preview_viewport_cannot_reveal() {
        let bounds = Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(
                px(PREVIEW_WIDTH + PREVIEW_SHADOW_PADDING * 2.0),
                px(preview_stack_height(PREVIEW_MAX_STACK)),
            ),
        };
        assert!(!preview_viewport_ready(size(px(400.0), px(300.0)), bounds));
        assert!(preview_viewport_ready(bounds.size, bounds));
    }

    /// `capture-preview-window.tsx` styles the two destructive controls with
    /// `hover:bg-destructive` and everything else with `hover:bg-primary`. A
    /// single shared hover colour reads as a bug only when you hover a delete
    /// button and it lights up the same as copy, which no test would notice --
    /// hence checking the source.
    #[test]
    fn destructive_controls_hover_destructive_and_the_rest_hover_primary() {
        let source = include_str!("capture_preview.rs");
        // `theme.accent` is the theme's own accent; `theme.primary` is the OS
        // accent, which is what Electron's `--primary` resolves to.
        for line in source.lines() {
            let trimmed = line.trim();
            assert!(
                !trimmed.starts_with(".hover(|style| style.bg(theme.accent))"),
                "a preview control still hovers to the theme accent instead of                  the OS accent: {trimmed}"
            );
        }
        // Counted as whole lines: the call sites each sit on their own line, so
        // this cannot match the string literal in this very test.
        let destructive = source
            .lines()
            .filter(|line| line.trim() == "theme.destructive,")
            .count();
        assert_eq!(
            destructive, 2,
            "close and delete are the two destructive controls"
        );
    }
}
