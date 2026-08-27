//! Capture preview — port of `renderer/windows/capture-preview-window.tsx`
//! and `main/capture/capture-preview/index.ts`. A 200×140 thumbnail docks in
//! the configured corner, stacks up to four deep, and reveals hover chrome.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    div, img, prelude::*, px, size, AnyWindowHandle, App, Bounds, Context, MouseButton, Render,
    Styled, Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};
use parking_lot::Mutex;

use crate::theme::vars::active_theme;
use crate::ui::chrome::{
    self, PreviewCorner, PREVIEW_CONTROL, PREVIEW_CONTROL_INSET, PREVIEW_HEIGHT,
    PREVIEW_HOVER_SCALE, PREVIEW_MAX_STACK, PREVIEW_RADIUS, PREVIEW_WIDTH, WINDOW_MOVE_STEPS,
};
use crate::ui::icon::icon_element;

static STACK: Mutex<Vec<AnyWindowHandle>> = Mutex::new(Vec::new());
static RESTACK_GENERATION: AtomicU64 = AtomicU64::new(0);
const UPLOAD_DONE_DISPLAY_MS: u64 = 800;

pub struct CapturePreviewWindow {
    path: PathBuf,
    image: Option<Arc<gpui::RenderImage>>,
    /// A pre-blurred copy of the thumbnail, swapped in while the controls show.
    /// Electron's scrim is `backdrop-blur-md` over the thumbnail; gpui cannot
    /// blur a region, so the blur is baked once here instead of per frame.
    blurred: Option<Arc<gpui::RenderImage>>,
    hovered: bool,
    /// How far through the 200ms hover transition we are, 0.0 to 1.0.
    /// `transition-transform duration-200` animates in *and* out, so this
    /// decays after the pointer leaves rather than snapping back.
    hover_progress: f32,
    /// When the last frame was drawn, so the step is time-based rather than
    /// frame-rate dependent.
    last_frame: Option<std::time::Instant>,
    display_menu_open: bool,
    busy: bool,
    dismiss_token: Arc<AtomicU64>,
}

impl CapturePreviewWindow {
    pub fn open(cx: &mut App, path: PathBuf) {
        prune_stack(cx);
        let mut stack = STACK.lock();
        while stack.len() >= PREVIEW_MAX_STACK {
            let Some(oldest) = stack.first().copied() else {
                break;
            };
            drop_handle(oldest, cx);
            stack.remove(0);
        }
        let index = stack.len();
        drop(stack);

        let image = load_thumbnail(&path);
        let blurred = load_blurred_thumbnail(&path);
        let dismiss_token = Arc::new(AtomicU64::new(0));
        let bounds = preview_bounds(cx, index);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                focus: false,
                show: true,
                kind: WindowKind::PopUp,
                is_movable: true,
                is_resizable: false,
                is_minimizable: false,
                window_background: WindowBackgroundAppearance::Opaque,
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| Self {
                    path: path.clone(),
                    image,
                    blurred,
                    hovered: false,
                    hover_progress: 0.0,
                    last_frame: None,
                    display_menu_open: false,
                    busy: false,
                    dismiss_token: dismiss_token.clone(),
                })
            },
        );
        if let Ok(handle) = opened {
            let handle: AnyWindowHandle = handle.into();
            STACK.lock().push(handle);
            schedule_auto_dismiss(handle, cx, dismiss_token);
        }
        restack(cx);
    }
}

fn schedule_auto_dismiss(handle: AnyWindowHandle, cx: &mut App, dismiss_token: Arc<AtomicU64>) {
    let preview = crate::state::state(cx).config.get().preview;
    if !preview.auto_dismiss || preview.auto_dismiss_seconds <= 0.0 {
        return;
    }
    let timeout = Duration::from_secs_f64(preview.auto_dismiss_seconds);
    let generation = dismiss_token.fetch_add(1, Ordering::Relaxed) + 1;
    cx.spawn(async move |cx| {
        cx.background_executor().timer(timeout).await;
        let should_close = cx
            .update(|cx| {
                let Some(handle) = handle.downcast::<CapturePreviewWindow>() else {
                    return false;
                };
                handle
                    .update(cx, |view, _, _| {
                        auto_dismiss_ready(
                            view.dismiss_token.load(Ordering::Relaxed),
                            generation,
                            view.hovered,
                            view.display_menu_open,
                            view.busy,
                        )
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        if should_close {
            let closed = cx
                .update(|cx| {
                    handle
                        .downcast::<CapturePreviewWindow>()
                        .is_some_and(|handle| {
                            handle
                                .update(cx, |_, window, _| window.remove_window())
                                .is_ok()
                        })
                })
                .unwrap_or(false);
            if closed {
                let _ = cx.update(restack);
            }
        }
    })
    .detach();
}

fn auto_dismiss_ready(
    current_generation: u64,
    generation: u64,
    hovered: bool,
    menu: bool,
    busy: bool,
) -> bool {
    current_generation == generation && !hovered && !menu && !busy
}

fn prune_stack(cx: &mut App) {
    STACK
        .lock()
        .retain(|handle| handle.update(cx, |_, _, _| ()).is_ok());
}

fn drop_handle(handle: AnyWindowHandle, cx: &mut App) {
    let _ = handle.update(cx, |_, window, _| window.remove_window());
}

fn selected_display_bounds(cx: &mut App) -> Bounds<gpui::Pixels> {
    let displays = cx.displays();
    let config = crate::state::state(cx).config.get();
    let chosen = config
        .preview
        .display_id
        .and_then(|id| displays.get(id.max(0) as usize).cloned())
        .or_else(|| displays.first().cloned());
    match chosen {
        // Electron anchors previews to `display.workArea`, so the taskbar has to
        // come off here -- gpui's `bounds()` is the whole monitor.
        Some(display) => crate::system::work_area::work_area(display.bounds()),
        None => Bounds::from_corners(
            gpui::point(px(0.0), px(0.0)),
            gpui::point(px(1920.0), px(1080.0)),
        ),
    }
}

fn preview_bounds(cx: &mut App, index: usize) -> Bounds<gpui::Pixels> {
    let display = selected_display_bounds(cx);
    let config = crate::state::state(cx).config.get();
    let corner = PreviewCorner::parse(&config.preview.corner);
    let (x, y) = chrome::preview_origin(
        f32::from(display.origin.x),
        f32::from(display.origin.y),
        f32::from(display.size.width),
        f32::from(display.size.height),
        index,
        corner,
    );
    Bounds {
        origin: gpui::point(px(x), px(y)),
        size: size(px(PREVIEW_WIDTH), px(PREVIEW_HEIGHT)),
    }
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

fn close_self(window: &mut Window, cx: &mut App) {
    let handle = window.window_handle();
    STACK.lock().retain(|item| *item != handle);
    window.remove_window();
    restack(cx);
}

fn restack(cx: &mut App) {
    let generation = RESTACK_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    prune_stack(cx);
    let handles: Vec<_> = STACK.lock().clone();
    for (index, handle) in handles.into_iter().enumerate() {
        let target = preview_bounds(cx, index);
        let Some(current) = handle.update(cx, |_, window, _| window.bounds()).ok() else {
            continue;
        };
        let from_x = f32::from(current.origin.x);
        let from_y = f32::from(current.origin.y);
        let to_x = f32::from(target.origin.x);
        let to_y = f32::from(target.origin.y);
        if (from_x - to_x).abs() < f32::EPSILON && (from_y - to_y).abs() < f32::EPSILON {
            continue;
        }
        let step_ms = chrome::window_move_step_ms();
        cx.spawn(async move |cx| {
            for step in 1..=WINDOW_MOVE_STEPS {
                if RESTACK_GENERATION.load(Ordering::Relaxed) != generation {
                    break;
                }
                let (x, y) = chrome::window_move_position(
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    step,
                    WINDOW_MOVE_STEPS,
                );
                let applied = cx.update(|cx| {
                    handle.update(cx, |_, window, _| {
                        apply_preview_frame(window, x, y);
                    })
                });
                if applied.is_err() {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(step_ms))
                    .await;
            }
        })
        .detach();
    }
}

fn preview_frame(x: f32, y: f32) -> (f32, f32, f32, f32) {
    (x, y, PREVIEW_WIDTH, PREVIEW_HEIGHT)
}

fn window_origin_device_pixels(x: f32, y: f32, scale: f32) -> (i32, i32) {
    ((x * scale).round() as i32, (y * scale).round() as i32)
}

fn apply_preview_frame(window: &mut Window, x: f32, y: f32) {
    let (origin_x, origin_y, _, _) = preview_frame(x, y);
    set_native_window_origin(window, origin_x, origin_y);
}

fn set_native_window_origin(window: &Window, x: f32, y: f32) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
        };

        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return;
        };
        let hwnd = HWND(win32.hwnd.get() as *mut core::ffi::c_void);
        let (device_x, device_y) = window_origin_device_pixels(x, y, window.scale_factor());
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                None,
                device_x,
                device_y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (window, x, y);
    }
}

impl CapturePreviewWindow {
    fn show_controls(&self) -> bool {
        self.hovered || self.display_menu_open || self.busy
    }

    /// Moves `hover_progress` towards its target and asks for another frame
    /// while it is still moving.
    ///
    /// gpui has no per-property transitions, so `transition-transform
    /// duration-200` has to be driven by hand.
    fn advance_hover(&mut self, window: &mut Window) -> f32 {
        let target = if self.show_controls() { 1.0 } else { 0.0 };
        if (self.hover_progress - target).abs() < f32::EPSILON {
            self.last_frame = None;
            return self.hover_progress;
        }

        let now = std::time::Instant::now();
        // The first frame of a transition has no previous frame to measure
        // against; stepping by 0 there just defers the motion by one frame.
        let delta = self
            .last_frame
            .map(|previous| now.saturating_duration_since(previous).as_secs_f32())
            .unwrap_or(0.0);
        self.last_frame = Some(now);

        self.hover_progress = step_hover(self.hover_progress, target, delta);

        window.request_animation_frame();
        self.hover_progress
    }

    /// `hover_bg` is passed in rather than fixed because Electron does not use
    /// one colour for these: close and delete are `hover:bg-destructive` while
    /// copy, upload and the display picker are `hover:bg-primary`. Styling a
    /// delete the same as a copy loses the only warning the control gives.
    fn circle_button(
        id: &'static str,
        icon: &'static str,
        busy: bool,
        tooltip: impl Into<gpui::SharedString>,
        theme: &crate::theme::vars::ThemeVars,
        hover_bg: gpui::Hsla,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> gpui::AnyElement {
        let tooltip = tooltip.into();
        div()
            .id(id)
            .size(px(PREVIEW_CONTROL))
            .rounded_full()
            .bg(theme.background.opacity(0.8))
            .flex()
            .items_center()
            .justify_center()
            .tooltip(move |_window, cx| {
                cx.new(|_| crate::ui::tooltip::Tooltip::new(tooltip.clone()))
                    .into()
            })
            .hover(move |style| style.bg(hover_bg))
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                on_click(window, cx);
                cx.stop_propagation();
            })
            .child(if busy {
                crate::ui::icon::spinner_element(
                    gpui::ElementId::Name(format!("{id}-spinner").into()),
                    px(14.0),
                )
            } else {
                icon_element(icon, px(14.0))
            })
            .into_any_element()
    }
}

impl Render for CapturePreviewWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        let preview_entity = cx.entity().downgrade();
        let show_controls = self.show_controls();
        let progress = self.advance_hover(window);
        let polish = starred_preset(cx);
        // `scale-105` over `duration-200` rather than an instant jump.
        let scale = 1.0 + (PREVIEW_HOVER_SCALE - 1.0) * progress;
        let image_w = PREVIEW_WIDTH * scale;
        let image_h = PREVIEW_HEIGHT * scale;
        let displays = cx.displays();
        let has_multiple_displays = displays.len() > 1;

        let mut root = div()
            .id("capture-preview")
            .relative()
            .size_full()
            .overflow_hidden()
            .rounded(px(PREVIEW_RADIUS))
            .bg(theme.muted_background)
            .on_hover({
                let view = cx.entity().downgrade();
                move |hovered: &bool, window, cx| {
                    let handle = window.window_handle();
                    let dismiss_token = view
                        .update(cx, |this, cx| {
                            this.hovered = *hovered;
                            cx.notify();
                            this.dismiss_token.clone()
                        })
                        .ok();
                    if let Some(dismiss_token) = dismiss_token {
                        schedule_auto_dismiss(handle, cx, dismiss_token);
                    }
                }
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _window, cx| {
                    if event.click_count >= 2 {
                        let path = this.path.to_string_lossy().into_owned();
                        close_self(_window, cx);
                        crate::open_editor_for(cx, &path);
                    }
                }),
            );

        // `capture-preview-window.tsx` lays a `bg-black/25 backdrop-blur-md`
        // scrim over the thumbnail while the controls show. The tint is drawn
        // below; this is the blur half of it.
        let thumbnail = if show_controls {
            self.blurred.as_ref().or(self.image.as_ref())
        } else {
            self.image.as_ref()
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
                .child(icon_element("image", px(48.0)))
                .into_any_element(),
        });

        if show_controls {
            let path = self.path.clone();
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
                        .top(px(PREVIEW_CONTROL_INSET))
                        .left(px(PREVIEW_CONTROL_INSET))
                        .child(Self::circle_button(
                            "preview-close",
                            "x",
                            false,
                            "Close preview",
                            &theme,
                            theme.destructive,
                            close_self,
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(PREVIEW_CONTROL_INSET))
                        .right(px(PREVIEW_CONTROL_INSET))
                        .child(Self::circle_button(
                            "preview-delete",
                            "trash-2",
                            false,
                            "Delete screenshot",
                            &theme,
                            theme.destructive,
                            {
                                let path = path.clone();
                                move |window, cx| {
                                    delete_capture(&path);
                                    close_self(window, cx);
                                }
                            },
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        // `flex-col gap-1`: Polish stacks above Edit when a
                        // preset is starred, and is absent otherwise -- which is
                        // also what Electron renders with no starred preset.
                        .flex_col()
                        .gap(px(4.0))
                        .when_some(polish.clone(), |el, preset| {
                            let tooltip = format!("Copy with \"{}\"", preset.name);
                            el.child(
                                div()
                                    .id("preview-polish")
                                    .rounded_full()
                                    .bg(theme.background.opacity(0.8))
                                    .px(px(12.0))
                                    .py(px(4.0))
                                    .text_size(px(12.0))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .hover(|style| style.bg(theme.primary))
                                    .tooltip(move |_window, cx| {
                                        cx.new(|_| {
                                            crate::ui::tooltip::Tooltip::new(tooltip.clone())
                                        })
                                        .into()
                                    })
                                    .on_mouse_down(MouseButton::Left, {
                                        let path = path.clone();
                                        move |_, window, cx| {
                                            if polish_and_copy(&path, cx) {
                                                close_self(window, cx);
                                            }
                                            cx.stop_propagation();
                                        }
                                    })
                                    .child("Polish"),
                            )
                        })
                        .child(
                            div()
                                .id("preview-edit")
                                .rounded_full()
                                .bg(theme.background.opacity(0.8))
                                .px(px(12.0))
                                .py(px(4.0))
                                .text_size(px(12.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .hover(|style| style.bg(theme.primary))
                                .tooltip(|_window, cx| {
                                    cx.new(|_| crate::ui::tooltip::Tooltip::new("Edit")).into()
                                })
                                .on_mouse_down(MouseButton::Left, {
                                    let path = path.clone();
                                    move |_, window, cx| {
                                        let path = path.to_string_lossy().into_owned();
                                        close_self(window, cx);
                                        crate::open_editor_for(cx, &path);
                                        cx.stop_propagation();
                                    }
                                })
                                .child("Edit"),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .bottom(px(PREVIEW_CONTROL_INSET))
                        .left(px(PREVIEW_CONTROL_INSET))
                        .child(Self::circle_button(
                            "preview-copy",
                            "copy",
                            false,
                            "Copy",
                            &theme,
                            theme.primary,
                            {
                                let path = path.clone();
                                move |window, cx| {
                                    if copy_image(&path, cx) {
                                        close_self(window, cx);
                                    }
                                    cx.stop_propagation();
                                }
                            },
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .bottom(px(PREVIEW_CONTROL_INSET))
                        .right(px(PREVIEW_CONTROL_INSET))
                        .child(Self::circle_button(
                            "preview-upload",
                            "cloud-upload",
                            self.busy,
                            "Upload to Cloud",
                            &theme,
                            theme.primary,
                            {
                                let path = path.clone();
                                let preview_entity = preview_entity.clone();
                                move |window, cx| {
                                    let config = crate::state::state(cx).config.get().cloud;
                                    let path = path.clone();
                                    let handle = window.window_handle();
                                    let dismiss_token = preview_entity
                                        .update(cx, |preview, cx| {
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
                                            let _ = task_entity.update(cx, |preview, cx| {
                                                if !succeeded {
                                                    preview.busy = false;
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
                                                let _ = preview.update(cx, |_, window, cx| {
                                                    close_self(window, cx)
                                                });
                                            });
                                        } else {
                                            let _ = cx.update(|cx| {
                                                schedule_auto_dismiss(handle, cx, dismiss_token)
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
                        .bottom(px(PREVIEW_CONTROL_INSET + PREVIEW_CONTROL + 4.0))
                        .right(px(PREVIEW_CONTROL_INSET))
                        .child(Self::circle_button(
                            "preview-pin-display",
                            "monitor",
                            false,
                            "Move previews to another display",
                            &theme,
                            theme.primary,
                            {
                                let count = displays.len();
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

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restack_drives_electron_window_move() {
        assert_eq!(WINDOW_MOVE_STEPS, 8);
        assert_eq!(chrome::WINDOW_MOVE_DURATION_MS, 120);
        assert_eq!(chrome::window_move_step_ms(), 15);
        assert_eq!(
            chrome::window_move_position(10.0, 20.0, 10.0, 20.0, 8, WINDOW_MOVE_STEPS),
            (10.0, 20.0)
        );
        assert_eq!(
            chrome::window_move_position(0.0, 0.0, 80.0, 40.0, 8, WINDOW_MOVE_STEPS),
            (80.0, 40.0)
        );
        let (x, y) = chrome::window_move_position(0.0, 0.0, 80.0, 40.0, 4, WINDOW_MOVE_STEPS);
        assert_eq!(preview_frame(x, y), (x, y, PREVIEW_WIDTH, PREVIEW_HEIGHT));
        assert_eq!((x, y), (60.0, 30.0));
        assert_eq!(window_origin_device_pixels(x, y, 1.0), (60, 30));
        assert_eq!(window_origin_device_pixels(80.0, 40.0, 1.5), (120, 60));
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

    #[test]
    fn auto_dismiss_is_cancelled_by_interaction() {
        assert!(auto_dismiss_ready(4, 4, false, false, false));
        assert!(!auto_dismiss_ready(5, 4, false, false, false));
        assert!(!auto_dismiss_ready(4, 4, true, false, false));
        assert!(!auto_dismiss_ready(4, 4, false, true, false));
        assert!(!auto_dismiss_ready(4, 4, false, false, true));
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
