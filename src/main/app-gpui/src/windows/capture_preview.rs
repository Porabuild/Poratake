//! Capture preview — port of `renderer/windows/capture-preview-window.tsx`
//! and `main/capture/capture-preview/index.ts`. A 200×140 thumbnail docks in
//! the configured corner, stacks up to four deep, and reveals hover chrome.

#[cfg(any(target_os = "macos", all(feature = "parity", any(windows, test))))]
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(any(windows, test))]
use gpui::Entity;
use gpui::{
    div, img, prelude::*, px, size, AnyWindowHandle, App, Bounds, Context, MouseButton, Pixels,
    Render, Size, Styled, Window,
};
use herogpui::components::TooltipPlacement;
use herogpui::gpui;

use crate::theme::bridge::{MENU_PANEL_PADDING, MENU_ROW_HEIGHT};
use crate::theme::vars::active_theme;
use crate::ui::chrome::{
    self, PreviewCorner, PREVIEW_CONTROL, PREVIEW_CONTROL_INSET, PREVIEW_HEIGHT,
    PREVIEW_HOVER_SCALE, PREVIEW_MAX_STACK, PREVIEW_RADIUS, PREVIEW_SHADOW_PADDING,
    PREVIEW_STACK_GAP, PREVIEW_WIDTH,
};
use crate::ui::icon::icon_element;
use crate::ui::preview;
use crate::ui::primitives::{
    OVERLAY_ENTER_MS as PREVIEW_ENTER_MS, OVERLAY_ENTER_SLIDE as PREVIEW_ENTER_OFFSET,
    OVERLAY_EXIT_MS as PREVIEW_EXIT_MS,
};
use crate::windows::registry::{self, WindowKind};

static NEXT_PREVIEW_ID: AtomicU64 = AtomicU64::new(1);
const UPLOAD_DONE_DISPLAY_MS: u64 = 800;
const PREVIEW_MOVE_MS: u64 = 120;
/// Video previews autoplay the recording muted and looped, like the Electron
/// `<video>` tag. 8fps keeps the thumbnail alive without decoding full rate.
const VIDEO_PLAYBACK_TICK: Duration = Duration::from_millis(125);

#[derive(Clone)]
struct PreviewThumbnailDecodeSlots(Arc<smol::lock::Semaphore>);

impl gpui::Global for PreviewThumbnailDecodeSlots {}

fn preview_thumbnail_decode_slots(cx: &mut App) -> Arc<smol::lock::Semaphore> {
    if let Some(slots) = cx.try_global::<PreviewThumbnailDecodeSlots>() {
        return slots.0.clone();
    }
    let slots = Arc::new(smol::lock::Semaphore::new(1));
    cx.set_global(PreviewThumbnailDecodeSlots(slots.clone()));
    slots
}

#[cfg(all(feature = "parity", any(windows, test)))]
thread_local! {
    static SIMULATE_HEADLESS_PREVIEW_REVEAL: Cell<bool> = const { Cell::new(false) };
}

#[cfg(all(feature = "parity", any(windows, test)))]
pub(crate) struct HeadlessPreviewRevealGuard(bool);

#[cfg(all(feature = "parity", any(windows, test)))]
impl HeadlessPreviewRevealGuard {
    pub(crate) fn enter() -> Self {
        Self(SIMULATE_HEADLESS_PREVIEW_REVEAL.with(|active| active.replace(true)))
    }
}

#[cfg(all(feature = "parity", any(windows, test)))]
impl Drop for HeadlessPreviewRevealGuard {
    fn drop(&mut self) {
        SIMULATE_HEADLESS_PREVIEW_REVEAL.with(|active| active.set(self.0));
    }
}

#[cfg(all(feature = "parity", any(windows, test)))]
fn simulate_headless_preview_reveal() -> bool {
    SIMULATE_HEADLESS_PREVIEW_REVEAL.with(Cell::get)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreviewKind {
    Screenshot,
    Video,
}

/// The `capture-preview:start-drag` payload: dragging a screenshot preview out
/// of the window hands the file to the OS, like Electron's `startDrag`.
/// Videos are excluded on both shells.
struct PreviewDrag {
    path: PathBuf,
    image: Option<Arc<gpui::RenderImage>>,
}

struct PreviewDragGhost {
    image: Option<Arc<gpui::RenderImage>>,
}

impl Render for PreviewDragGhost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut ghost = div()
            .w(px(PREVIEW_WIDTH))
            .h(px(PREVIEW_HEIGHT))
            .overflow_hidden()
            .rounded(px(PREVIEW_RADIUS))
            .opacity(0.9);
        if let Some(image) = self.image.clone() {
            ghost = ghost.child(
                img(image)
                    .size_full()
                    .object_fit(gpui::ObjectFit::Cover)
                    .when(chrome::is_macos(), |el| el.rounded(px(PREVIEW_RADIUS))),
            );
        }
        ghost
    }
}

struct VideoPlayback {
    decoder: Arc<crate::video::decoder::VideoDecoder>,
    duration: f64,
    started: Instant,
    in_flight: bool,
    #[cfg(target_os = "macos")]
    frames_released: bool,
    #[cfg(target_os = "macos")]
    viewport_clipped: Cell<bool>,
    #[cfg(target_os = "macos")]
    visibility_changed: smol::channel::Sender<()>,
}

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
    kind: PreviewKind,
    path: PathBuf,
    image: Option<Arc<gpui::RenderImage>>,
    thumbnail_loading: bool,
    thumbnail_cancel: Option<smol::channel::Sender<()>>,
    thumbnail_generation: u64,
    #[cfg(all(test, feature = "parity"))]
    thumbnail_ready: Option<smol::channel::Receiver<()>>,
    video: Option<VideoPlayback>,
    export_cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
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
    deleting: bool,
    copying: bool,
    finalizing: bool,
    completed: Option<Instant>,
    export_progress: Option<Arc<AtomicU32>>,
    dismiss_token: Arc<AtomicU64>,
    #[cfg(target_os = "macos")]
    dismiss_viewport_clipped: Cell<bool>,
    entered_at: Instant,
    layout_y: Option<f32>,
    layout_animation: Option<LayoutAnimation>,
    dismiss_animation: Option<DismissAnimation>,
}

impl CapturePreview {
    fn is_busy(&self) -> bool {
        self.busy || self.deleting || self.copying
    }
}

fn cancel_thumbnail_load(preview: &mut CapturePreview) {
    if let Some(cancel) = preview.thumbnail_cancel.take() {
        let _ = cancel.try_send(());
    }
    preview.thumbnail_generation = preview.thumbnail_generation.wrapping_add(1);
    preview.thumbnail_loading = false;
}

fn release_preview_images(
    preview: &mut CapturePreview,
    cx: &mut App,
    mut window: Option<&mut Window>,
) {
    for image in [preview.image.take(), preview.blurred.take()]
        .into_iter()
        .flatten()
    {
        cx.drop_image(image, window.as_deref_mut());
    }
}

fn thumbnail_preview_matches(
    preview: &CapturePreview,
    id: u64,
    path: &Path,
    generation: u64,
) -> bool {
    preview.id == id
        && preview.kind == PreviewKind::Screenshot
        && preview.path == path
        && preview.thumbnail_loading
        && preview.thumbnail_generation == generation
        && preview.dismiss_animation.is_none()
}

fn thumbnail_preview_mut<'a>(
    previews: &'a mut [CapturePreview],
    id: u64,
    path: &Path,
    generation: u64,
) -> Option<&'a mut CapturePreview> {
    previews
        .iter_mut()
        .find(|preview| thumbnail_preview_matches(preview, id, path, generation))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn restart_screenshot_thumbnail_loads(
    previews: &mut [CapturePreview],
) -> Vec<(
    u64,
    PathBuf,
    u64,
    smol::channel::Receiver<()>,
    Option<smol::channel::Sender<()>>,
)> {
    let mut pending = Vec::new();
    for preview in previews {
        cancel_thumbnail_load(preview);
        if preview.kind != PreviewKind::Screenshot
            || preview.image.is_some()
            || preview.dismiss_animation.is_some()
        {
            continue;
        }
        let (cancel, canceled) = smol::channel::bounded(1);
        preview.thumbnail_cancel = Some(cancel);
        preview.thumbnail_loading = true;
        preview.thumbnail_generation = preview.thumbnail_generation.wrapping_add(1);
        #[cfg(all(test, feature = "parity"))]
        let (ready_sender, ready_receiver) = smol::channel::bounded(1);
        #[cfg(not(all(test, feature = "parity")))]
        let ready_sender = None;
        #[cfg(all(test, feature = "parity"))]
        {
            preview.thumbnail_ready = Some(ready_receiver);
        }
        #[cfg(all(test, feature = "parity"))]
        let ready_sender = Some(ready_sender);
        pending.push((
            preview.id,
            preview.path.clone(),
            preview.thumbnail_generation,
            canceled,
            ready_sender,
        ));
    }
    pending
}

pub struct CapturePreviewWindow {
    previews: Vec<CapturePreview>,
    revealed: bool,
    layout_generation: u64,
    #[cfg(target_os = "macos")]
    pointer_inside: bool,
    #[cfg(target_os = "macos")]
    keyboard_controls: bool,
    #[cfg(target_os = "macos")]
    pending_keyboard_tab: Option<bool>,
    #[cfg(target_os = "macos")]
    viewport_scroll: Option<(PreviewCorner, gpui::ScrollHandle)>,
    display_menu: crate::ui::menu::MenuHandle,
    tooltip_presenter: Arc<crate::ui::tooltip::NativeTooltipPresenter>,
    _display_observer: Option<gpui::Subscription>,
    #[cfg(target_os = "macos")]
    _visibility_observer: Option<gpui::Subscription>,
}

impl CapturePreviewWindow {
    #[cfg(all(feature = "parity", target_os = "macos", not(test)))]
    pub(crate) fn parity_viewport_scroll_offset(&self) -> Option<gpui::Point<Pixels>> {
        self.viewport_scroll
            .as_ref()
            .map(|(_, scroll)| scroll.offset())
    }

    #[cfg(all(feature = "parity", target_os = "macos", not(test)))]
    pub(crate) fn parity_hovered(&self) -> bool {
        self.previews.iter().any(|preview| preview.hovered)
    }

    #[cfg(all(feature = "parity", target_os = "macos", not(test)))]
    pub(crate) fn parity_pointer_inside(&self) -> bool {
        self.pointer_inside
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub(crate) fn act_on_latest_for_parity(
        handle: gpui::WindowHandle<Self>,
        copy: bool,
        cx: &mut App,
    ) -> bool {
        handle
            .update(cx, |view, window, cx| {
                let Some(preview) = view.previews.last() else {
                    return false;
                };
                let id = preview.id;
                let entity = cx.entity().downgrade();
                window.defer(cx, move |window, cx| {
                    if copy {
                        copy_screenshot(&entity, id, false, window, cx);
                    } else {
                        delete_preview(&entity, id, window, cx);
                    }
                });
                true
            })
            .unwrap_or(false)
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub(crate) fn action_pending_for_parity(&self, copy: bool) -> bool {
        self.previews.last().is_some_and(|preview| {
            if copy {
                preview.copying
            } else {
                preview.deleting
            }
        })
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub(crate) fn finalizing_for_parity(&self) -> bool {
        self.previews.iter().any(|preview| preview.finalizing)
    }

    #[cfg(feature = "parity")]
    pub(crate) fn thumbnails_ready_for_parity(&self) -> bool {
        self.previews.iter().all(|preview| {
            preview.kind != PreviewKind::Screenshot
                || (!preview.thumbnail_loading
                    && preview.image.is_some()
                    && preview.blurred.is_some())
        })
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub(crate) fn video_thumbnail_status_for_parity(&self) -> Option<(bool, bool, bool)> {
        self.previews
            .iter()
            .find(|preview| preview.kind == PreviewKind::Video)
            .map(|preview| {
                (
                    preview.hovered,
                    preview.image.is_some(),
                    preview.blurred.is_some(),
                )
            })
    }

    fn active_preview_count(&self) -> usize {
        self.previews
            .iter()
            .filter(|preview| preview.dismiss_animation.is_none())
            .count()
    }

    fn window_preview_count(&self) -> usize {
        if cfg!(target_os = "macos") {
            self.previews.len().min(PREVIEW_MAX_STACK)
        } else {
            self.active_preview_count()
        }
    }

    pub fn open(cx: &mut App, path: PathBuf) {
        Self::open_screenshot(cx, path, None);
    }

    pub(crate) fn open_screenshot(
        cx: &mut App,
        path: PathBuf,
        finalization: Option<smol::channel::Receiver<()>>,
    ) {
        let id = NEXT_PREVIEW_ID.fetch_add(1, Ordering::Relaxed);
        let dismiss_token = Arc::new(AtomicU64::new(0));
        let (thumbnail_cancel, thumbnail_canceled) = smol::channel::bounded(1);
        #[cfg(all(test, feature = "parity"))]
        let (thumbnail_ready_sender, thumbnail_ready_receiver) = smol::channel::bounded(1);
        #[cfg(not(all(test, feature = "parity")))]
        let thumbnail_ready_sender = None;
        let entered_at = cx.background_executor().now();
        Self::push_preview(
            cx,
            id,
            dismiss_token.clone(),
            CapturePreview {
                id,
                kind: PreviewKind::Screenshot,
                image: None,
                thumbnail_loading: true,
                thumbnail_cancel: Some(thumbnail_cancel),
                thumbnail_generation: 1,
                #[cfg(all(test, feature = "parity"))]
                thumbnail_ready: Some(thumbnail_ready_receiver),
                blurred: None,
                path: path.clone(),
                video: None,
                export_cancel: None,
                hover_progress: 0.0,
                last_frame: None,
                hovered: false,
                busy: false,
                deleting: false,
                copying: false,
                finalizing: finalization.is_some(),
                completed: None,
                export_progress: None,
                dismiss_token,
                #[cfg(target_os = "macos")]
                dismiss_viewport_clipped: Cell::new(false),
                entered_at,
                layout_y: None,
                layout_animation: None,
                dismiss_animation: None,
            },
        );
        Self::load_screenshot_thumbnails(
            id,
            path,
            1,
            thumbnail_canceled,
            #[cfg(all(test, feature = "parity"))]
            Some(thumbnail_ready_sender),
            #[cfg(not(all(test, feature = "parity")))]
            thumbnail_ready_sender,
            cx,
        );
        Self::watch_finalization(id, finalization, cx);
    }

    fn watch_finalization(
        id: u64,
        finalization: Option<smol::channel::Receiver<()>>,
        cx: &mut App,
    ) {
        let Some(finalization) = finalization else {
            return;
        };
        let Some(handle) = registry::handle(WindowKind::CapturePreview, cx)
            .and_then(|handle| handle.downcast::<CapturePreviewWindow>())
        else {
            return;
        };
        cx.spawn(async move |cx| {
            let _ = finalization.recv().await;
            cx.update(|cx| {
                let _ = handle.update(cx, |view, _, cx| {
                    if let Some(preview) = view.previews.iter_mut().find(|preview| preview.id == id)
                    {
                        preview.finalizing = false;
                        cx.notify();
                    }
                });
            });
        })
        .detach();
    }

    /// Recording counterpart of `open`: the stack shows the project's video
    /// with play/export actions (`recording-actions.ts` shows a `'video'`
    /// preview when `recording.showPreview` is on).
    pub(crate) fn open_video(
        cx: &mut App,
        project: PathBuf,
        finalization: Option<smol::channel::Receiver<()>>,
    ) {
        let id = NEXT_PREVIEW_ID.fetch_add(1, Ordering::Relaxed);
        let dismiss_token = Arc::new(AtomicU64::new(0));
        let entered_at = cx.background_executor().now();
        Self::push_preview(
            cx,
            id,
            dismiss_token.clone(),
            CapturePreview {
                id,
                kind: PreviewKind::Video,
                image: None,
                thumbnail_loading: true,
                thumbnail_cancel: None,
                thumbnail_generation: 0,
                #[cfg(all(test, feature = "parity"))]
                thumbnail_ready: None,
                blurred: None,
                path: project,
                video: None,
                export_cancel: None,
                hover_progress: 0.0,
                last_frame: None,
                hovered: false,
                busy: false,
                deleting: false,
                copying: false,
                finalizing: finalization.is_some(),
                completed: None,
                export_progress: None,
                dismiss_token,
                #[cfg(target_os = "macos")]
                dismiss_viewport_clipped: Cell::new(false),
                entered_at,
                layout_y: None,
                layout_animation: None,
                dismiss_animation: None,
            },
        );
        pump_video_preview(cx, id);
        Self::watch_finalization(id, finalization, cx);
    }

    fn load_screenshot_thumbnails(
        id: u64,
        path: PathBuf,
        generation: u64,
        thumbnail_canceled: smol::channel::Receiver<()>,
        thumbnail_ready: Option<smol::channel::Sender<()>>,
        cx: &mut App,
    ) {
        let Some(handle) = registry::handle(WindowKind::CapturePreview, cx)
            .and_then(|handle| handle.downcast::<CapturePreviewWindow>())
        else {
            return;
        };
        let active = handle
            .update(cx, |view, _, _| {
                view.previews
                    .iter()
                    .any(|preview| thumbnail_preview_matches(preview, id, &path, generation))
            })
            .unwrap_or(false);
        if !active {
            return;
        }
        let decode_slots = preview_thumbnail_decode_slots(cx);
        cx.spawn(async move |cx| {
            let permit = smol::future::race(async { Some(decode_slots.acquire().await) }, async {
                let _ = thumbnail_canceled.recv().await;
                None
            })
            .await;
            let Some(_permit) = permit else { return };
            let active = cx.update(|cx| {
                registry::handle(WindowKind::CapturePreview, cx)
                    .and_then(|handle| handle.downcast::<CapturePreviewWindow>())
                    .is_some_and(|handle| {
                        handle
                            .update(cx, |view, _, _| {
                                view.previews.iter().any(|preview| {
                                    thumbnail_preview_matches(preview, id, &path, generation)
                                })
                            })
                            .unwrap_or(false)
                    })
            });
            if !active {
                return;
            }
            let decode_path = path.clone();
            let (image, blurred) = cx
                .background_executor()
                .spawn(async move { load_preview_thumbnails(&decode_path) })
                .await;
            #[cfg(all(test, feature = "parity"))]
            let mut thumbnail_ready = thumbnail_ready;
            #[cfg(not(all(test, feature = "parity")))]
            let _ = thumbnail_ready;
            cx.update(|cx| {
                let Some(handle) = registry::handle(WindowKind::CapturePreview, cx)
                    .and_then(|handle| handle.downcast::<CapturePreviewWindow>())
                else {
                    return;
                };
                let timer = handle
                    .update(cx, |view, _, cx| {
                        let revealed = view.revealed;
                        let preview =
                            thumbnail_preview_mut(&mut view.previews, id, &path, generation)?;
                        preview.image = image;
                        preview.blurred = blurred;
                        preview.thumbnail_loading = false;
                        preview.thumbnail_cancel.take();
                        #[cfg(all(test, feature = "parity"))]
                        if let Some(ready) = thumbnail_ready.take() {
                            let _ = ready.try_send(());
                        }
                        let timer = revealed.then(|| {
                            preview.dismiss_token.fetch_add(1, Ordering::Relaxed);
                            (preview.id, preview.dismiss_token.clone())
                        });
                        cx.notify();
                        timer
                    })
                    .ok()
                    .flatten();
                if let Some((id, dismiss_token)) = timer {
                    schedule_auto_dismiss(handle.into(), id, cx, dismiss_token);
                }
            });
        })
        .detach();
    }

    fn push_preview(cx: &mut App, id: u64, dismiss_token: Arc<AtomicU64>, preview: CapturePreview) {
        let mut preview = Some(preview);

        let existing = registry::handle(WindowKind::CapturePreview, cx);
        if let Some(handle) = existing {
            let updated = handle
                .downcast::<CapturePreviewWindow>()
                .and_then(|handle| {
                    handle
                        .update(cx, |view, window, cx| {
                            let Some(preview) = preview.take() else {
                                return false;
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
                            configure_stack_window(window, cx, view.window_preview_count());
                            #[cfg(windows)]
                            if !view.revealed {
                                schedule_preview_reveal(window, cx.entity());
                            }
                            cx.notify();
                            view.revealed
                                && view
                                    .previews
                                    .iter()
                                    .find(|preview| preview.id == id)
                                    .is_some_and(|preview| !preview.thumbnail_loading)
                        })
                        .ok()
                });
            if let Some(revealed) = updated {
                if revealed {
                    schedule_auto_dismiss(handle, id, cx, dismiss_token);
                }
                return;
            }
            registry::forget(WindowKind::CapturePreview, cx);
        }

        let Some(preview) = preview else {
            return;
        };
        Self::open_stack_window(cx, vec![preview]);
    }

    pub fn reposition(cx: &mut App) {
        let Some(stack) = registry::handle(WindowKind::CapturePreview, cx) else {
            return;
        };
        let Some(handle) = stack.downcast::<CapturePreviewWindow>() else {
            registry::forget(WindowKind::CapturePreview, cx);
            return;
        };
        #[cfg(any(windows, target_os = "macos"))]
        {
            let moved = handle
                .update(cx, |view, window, cx| {
                    #[cfg(target_os = "macos")]
                    {
                        view.display_menu.close(window, cx);
                        view.pointer_inside = false;
                        view.clear_native_tooltip(window);
                    }
                    configure_stack_window(window, cx, view.window_preview_count());
                    cx.notify();
                })
                .is_ok();
            if !moved {
                registry::forget(WindowKind::CapturePreview, cx);
            }
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let previews =
                handle.update(cx, |view, _window, _cx| std::mem::take(&mut view.previews));
            let Ok(previews) = previews else {
                registry::forget(WindowKind::CapturePreview, cx);
                return;
            };
            registry::forget(WindowKind::CapturePreview, cx);
            let _ = handle.update(cx, |_, window, _| window.remove_window());
            if !previews.is_empty() {
                let mut previews = previews;
                for preview in previews.iter_mut() {
                    if preview.kind == PreviewKind::Video {
                        preview.video = None;
                    }
                }
                let thumbnail_loads = restart_screenshot_thumbnail_loads(&mut previews);
                let ids: Vec<u64> = previews
                    .iter()
                    .filter(|preview| preview.kind == PreviewKind::Video)
                    .map(|preview| preview.id)
                    .collect();
                Self::open_stack_window(cx, previews);
                for (id, path, generation, canceled, ready_sender) in thumbnail_loads {
                    Self::load_screenshot_thumbnails(
                        id,
                        path,
                        generation,
                        canceled,
                        ready_sender,
                        cx,
                    );
                }
                for id in ids {
                    pump_video_preview(cx, id);
                }
            }
        }
    }

    #[cfg(windows)]
    pub(crate) fn prewarm(cx: &mut App) {
        if registry::handle(WindowKind::CapturePreview, cx).is_some() {
            return;
        }
        Self::open_stack_window(cx, Vec::new());
    }

    #[cfg(not(windows))]
    pub(crate) fn prewarm(_cx: &mut App) {}

    #[cfg(windows)]
    pub(crate) fn discard_idle(cx: &mut App) {
        let Some(handle) = registry::handle(WindowKind::CapturePreview, cx) else {
            return;
        };
        let Some(handle) = handle.downcast::<CapturePreviewWindow>() else {
            registry::forget(WindowKind::CapturePreview, cx);
            return;
        };
        let idle = handle
            .update(cx, |view, _, _| !view.revealed && view.previews.is_empty())
            .unwrap_or(true);
        if idle {
            registry::forget(WindowKind::CapturePreview, cx);
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }

    #[cfg(not(windows))]
    pub(crate) fn discard_idle(_cx: &mut App) {}

    fn open_stack_window(cx: &mut App, previews: Vec<CapturePreview>) {
        let announce = !previews.is_empty();
        let (bounds, display_id) = preview_stack_placement(cx, previews.len());
        let window_bounds = display_id
            .and_then(|id| cx.find_display(id))
            .map(|display| crate::system::work_area::local_window_bounds(bounds, display.as_ref()))
            .unwrap_or(bounds);
        let failure_body = match previews.as_slice() {
            [preview] => match preview.kind {
                PreviewKind::Screenshot => {
                    "The screenshot was saved, but its preview could not be opened."
                }
                PreviewKind::Video => {
                    "The recording was saved, but its preview could not be opened."
                }
            },
            _ => "The saved previews could not be opened.",
        };
        #[cfg(target_os = "linux")]
        let layer_anchor =
            match PreviewCorner::parse(&crate::state::state(cx).config.get().preview.corner) {
                PreviewCorner::TopLeft => {
                    gpui::layer_shell::Anchor::TOP | gpui::layer_shell::Anchor::LEFT
                }
                PreviewCorner::TopRight => {
                    gpui::layer_shell::Anchor::TOP | gpui::layer_shell::Anchor::RIGHT
                }
                PreviewCorner::BottomLeft => {
                    gpui::layer_shell::Anchor::BOTTOM | gpui::layer_shell::Anchor::LEFT
                }
                PreviewCorner::BottomRight => {
                    gpui::layer_shell::Anchor::BOTTOM | gpui::layer_shell::Anchor::RIGHT
                }
            };
        let make_options = || {
            super::popup_window_options(
                window_bounds,
                super::PopupWindowConfig {
                    focus: false,
                    show: !cfg!(windows),
                    movable: true,
                    display_id,
                    ..Default::default()
                },
            )
        };
        let build_preview = |window: &mut Window, cx: &mut App| {
            window.set_window_title("Poratake capture preview");
            super::remove_popup_frame(window);
            #[cfg(all(target_os = "macos", not(test)))]
            super::pin_macos_window_contents(window, preview_bottom_aligned(cx));
            #[cfg(windows)]
            if !cfg!(test) {
                crate::system::window_surface::prepare_transparent_surface(window);
                crate::system::window_surface::stage(window, bounds, None, false);
                sync_preview_viewport(window, bounds);
            }
            let handle = window.window_handle();
            let display_observer = cx.observe_displays(move |_, cx| {
                let Some(preview) = handle.downcast::<CapturePreviewWindow>() else {
                    return;
                };
                if preview
                    .update(cx, |view, window, cx| {
                        view.display_menu.close(window, cx);
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
                cx.defer(move |cx| {
                    if registry::handle(WindowKind::CapturePreview, cx) == Some(handle) {
                        CapturePreviewWindow::reposition(cx);
                    }
                });
            });
            let view = cx.new(|_| Self {
                previews,
                revealed: !cfg!(windows) || cfg!(test),
                layout_generation: 0,
                #[cfg(target_os = "macos")]
                pointer_inside: false,
                #[cfg(target_os = "macos")]
                keyboard_controls: false,
                #[cfg(target_os = "macos")]
                pending_keyboard_tab: None,
                #[cfg(target_os = "macos")]
                viewport_scroll: None,
                display_menu: crate::ui::menu::MenuHandle::new(),
                tooltip_presenter: crate::ui::tooltip::NativeTooltipPresenter::new(
                    "capture-preview",
                ),
                _display_observer: Some(display_observer),
                #[cfg(target_os = "macos")]
                _visibility_observer: None,
            });
            #[cfg(target_os = "macos")]
            view.update(cx, |view, cx| {
                view._visibility_observer = Some(cx.observe_window_visibility(
                    window,
                    |view, visibility, window, cx| {
                        let visible = visibility.is_visible();
                        for preview in &view.previews {
                            if let Some(video) = &preview.video {
                                let _ = video.visibility_changed.try_send(());
                            }
                            if !visible {
                                preview.dismiss_token.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        if visible {
                            let handle = window.window_handle();
                            cx.defer(move |cx| {
                                if registry::handle(WindowKind::CapturePreview, cx) == Some(handle)
                                {
                                    schedule_revealed_previews(handle, cx);
                                }
                            });
                        } else {
                            view.pointer_inside = false;
                            view.clear_native_tooltip(window);
                            cx.notify();
                        }
                    },
                ));
            });
            #[cfg(windows)]
            if announce {
                schedule_preview_reveal(window, view.clone());
            }
            view
        };
        #[cfg(target_os = "linux")]
        let opened = super::open_wayland_layer_or_fallback(
            cx,
            gpui::layer_shell::LayerShellOptions {
                namespace: "poratake-preview".to_string(),
                layer: gpui::layer_shell::Layer::Overlay,
                anchor: layer_anchor,
                margin: Some((
                    px(chrome::PREVIEW_MARGIN - chrome::PREVIEW_SHADOW_PADDING),
                    px(chrome::PREVIEW_MARGIN - chrome::PREVIEW_SHADOW_PADDING),
                    px(chrome::PREVIEW_MARGIN - chrome::PREVIEW_SHADOW_PADDING),
                    px(chrome::PREVIEW_MARGIN - chrome::PREVIEW_SHADOW_PADDING),
                )),
                keyboard_interactivity: gpui::layer_shell::KeyboardInteractivity::None,
                ..Default::default()
            },
            make_options,
            build_preview,
        );
        #[cfg(not(target_os = "linux"))]
        let opened = cx
            .open_window(make_options(), build_preview)
            .map(|handle| (handle, false));
        if let Ok((handle, _)) = opened {
            let handle: AnyWindowHandle = handle.into();
            registry::remember(WindowKind::CapturePreview, handle, cx);
            if announce {
                schedule_revealed_previews(handle, cx);
            }
        } else if announce {
            crate::windows::toast::Toast::show(cx, "Preview unavailable", failure_body);
        }
    }
}

#[cfg(windows)]
fn sync_preview_viewport(window: &mut Window, bounds: Bounds<gpui::Pixels>) {
    window.resize(bounds.size);
}

#[cfg(any(windows, test))]
fn preview_viewport_ready(
    viewport: gpui::Size<gpui::Pixels>,
    bounds: Bounds<gpui::Pixels>,
) -> bool {
    viewport == bounds.size
}

#[cfg(windows)]
fn schedule_preview_reveal(window: &mut Window, view: Entity<CapturePreviewWindow>) {
    if cfg!(test) {
        return;
    }
    let handle = window.window_handle();
    window.on_next_frame(move |window, cx| {
        reveal_preview_when_ready(window, handle, view, false, cx);
    });
}

#[cfg(windows)]
fn reveal_preview_when_ready(
    window: &mut Window,
    handle: AnyWindowHandle,
    view: Entity<CapturePreviewWindow>,
    configured: bool,
    cx: &mut App,
) {
    let (bounds, display_id) = preview_stack_placement(cx, PREVIEW_MAX_STACK);
    let native_window = crate::windows::window_hwnd(window);
    #[cfg(feature = "parity")]
    let simulate_headless = native_window.is_none() && simulate_headless_preview_reveal();
    #[cfg(not(feature = "parity"))]
    let simulate_headless = false;
    if native_window.is_none() && !simulate_headless {
        fail_preview_reveal(window, view, cx);
        return;
    }
    let display = display_id.and_then(|id| cx.find_display(id));
    let scale = display
        .as_ref()
        .map(|display| crate::system::work_area::capture_scale_factor(display.as_ref(), cx))
        .unwrap_or_else(|| window.scale_factor());
    if let Some(hwnd) = native_window {
        crate::system::window_composition::apply_window_bounds(hwnd, bounds, scale);
    }
    if !preview_viewport_ready(window.viewport_size(), bounds) {
        sync_preview_viewport(window, bounds);
        window.on_next_frame(move |window, cx| {
            reveal_preview_when_ready(window, handle, view, false, cx)
        });
        return;
    }
    let count = view.update(cx, |view, _| view.active_preview_count());
    if count == 0 {
        fail_preview_reveal(window, view, cx);
        return;
    }
    configure_preview_stack_window(window, count, preview_bottom_aligned(cx));
    if !configured {
        window.on_next_frame(move |window, cx| {
            reveal_preview_when_ready(window, handle, view, true, cx)
        });
        return;
    }
    if let Some(hwnd) = native_window {
        crate::system::window_composition::reveal_window(hwnd, false);
        if !unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() } {
            fail_preview_reveal(window, view, cx);
            return;
        }
    }
    reveal_preview_stack_view(handle, view, cx);
}

#[cfg(windows)]
fn fail_preview_reveal(window: &mut Window, view: Entity<CapturePreviewWindow>, cx: &mut App) {
    let failure_body = view.update(cx, |view, _| {
        for preview in &mut view.previews {
            cancel_thumbnail_load(preview);
        }
        preview_open_failure_body(&view.previews)
    });
    registry::forget(WindowKind::CapturePreview, cx);
    window.remove_window();
    crate::windows::toast::Toast::show(cx, "Preview unavailable", failure_body);
}

#[cfg(any(windows, test))]
fn reveal_preview_stack_view(
    handle: AnyWindowHandle,
    view: Entity<CapturePreviewWindow>,
    cx: &mut App,
) {
    let timers = view.update(cx, |view, _| {
        if view.revealed {
            return None;
        }
        view.revealed = true;
        Some(
            view.previews
                .iter()
                .filter(|preview| !preview.thumbnail_loading)
                .map(|preview| (preview.id, preview.dismiss_token.clone()))
                .collect::<Vec<_>>(),
        )
    });
    let Some(timers) = timers else { return };
    for (id, dismiss_token) in timers {
        schedule_auto_dismiss(handle, id, cx, dismiss_token);
    }
}

#[cfg(windows)]
fn preview_open_failure_body(previews: &[CapturePreview]) -> &'static str {
    match previews {
        [preview] => match preview.kind {
            PreviewKind::Screenshot => {
                "The screenshot was saved, but its preview could not be opened."
            }
            PreviewKind::Video => "The recording was saved, but its preview could not be opened.",
        },
        _ => "The saved previews could not be opened.",
    }
}

fn schedule_revealed_previews(handle: AnyWindowHandle, cx: &mut App) {
    let timers = handle
        .downcast::<CapturePreviewWindow>()
        .and_then(|handle| {
            handle
                .update(cx, |view, _, _| {
                    view.revealed.then(|| {
                        view.previews
                            .iter()
                            .filter(|preview| {
                                !preview.thumbnail_loading && preview.dismiss_animation.is_none()
                            })
                            .map(|preview| (preview.id, preview.dismiss_token.clone()))
                            .collect::<Vec<_>>()
                    })
                })
                .ok()
        })
        .flatten();
    let Some(timers) = timers else { return };
    for (id, dismiss_token) in timers {
        schedule_auto_dismiss(handle, id, cx, dismiss_token);
    }
}

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
    let start = cx.background_executor().now();
    cx.spawn(async move |cx| {
        let mut elapsed = false;
        loop {
            let verdict = cx.update(|cx| dismiss_verdict(handle, id, generation, cx));
            match verdict {
                DismissVerdict::Gone => return,
                _ if !elapsed => {
                    let executor = cx.background_executor();
                    let remaining = timeout.saturating_sub(executor.now() - start);
                    if !remaining.is_zero() {
                        executor.timer(remaining).await;
                    }
                    elapsed = true;
                }
                DismissVerdict::Ready => {
                    let removed = cx.update(|cx| {
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
                    });
                    if removed {
                        return;
                    }
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
        .update(cx, |view, window, cx| {
            #[cfg(target_os = "macos")]
            if !window.is_visible() {
                return DismissVerdict::Gone;
            }
            let Some(preview) = view.previews.iter().find(|preview| preview.id == id) else {
                return DismissVerdict::Gone;
            };
            let verdict = dismiss_state(
                preview.dismiss_token.load(Ordering::Relaxed),
                generation,
                preview.hovered,
                view.preview_window_hovered(window),
                view.keyboard_controls_visible(window)
                    || preview.thumbnail_loading
                    || preview.finalizing
                    || preview.is_busy()
                    || preview.completed.is_some()
                    || view
                        .display_menu
                        .is_open_for(&format!("preview-display-menu-{id}"), cx),
            );
            #[cfg(target_os = "macos")]
            if verdict != DismissVerdict::Gone && view.preview_is_clipped(preview, window) {
                preview.dismiss_viewport_clipped.set(true);
                return DismissVerdict::Gone;
            }
            verdict
        })
        .unwrap_or(DismissVerdict::Gone)
}

fn dismiss_state(
    current_generation: u64,
    generation: u64,
    hovered: bool,
    window_hovered: bool,
    blocked: bool,
) -> DismissVerdict {
    if current_generation != generation {
        return DismissVerdict::Gone;
    }
    if hovered && window_hovered || blocked {
        return DismissVerdict::Blocked;
    }
    DismissVerdict::Ready
}

/// Starts the autoplay loop for a video preview: the decoder opens on a
/// background thread, then a tick decodes the looped playhead time into the
/// thumbnail until the preview is removed.
fn pump_video_preview(cx: &mut App, id: u64) {
    let Some(stack) = registry::handle(WindowKind::CapturePreview, cx) else {
        return;
    };
    let Some(handle) = stack.downcast::<CapturePreviewWindow>() else {
        return;
    };
    let started = handle
        .update(cx, |view, _, cx| {
            let preview = view.previews.iter().find(|preview| preview.id == id)?;
            if preview.kind != PreviewKind::Video
                || preview.video.is_some()
                || preview.dismiss_animation.is_some()
            {
                return None;
            }
            let video_path = crate::video::project::recording_video_path(&preview.path);
            cx.spawn(async move |entity, cx| {
                let decoder = cx
                    .background_executor()
                    .spawn(async move { crate::video::decoder::VideoDecoder::open(&video_path) })
                    .await;
                let Some(decoder) = decoder else {
                    cx.update(|cx| apply_video_preview_frame(handle.into(), &entity, id, None, cx));
                    return;
                };
                let duration = decoder.info().duration;
                let decoder = Arc::new(decoder);
                #[cfg(target_os = "macos")]
                let (visibility_changed, visibility_changes) = smol::channel::bounded(1);
                let attached = cx.update(|cx| {
                    entity
                        .update(cx, |view, cx| {
                            let preview =
                                view.previews.iter_mut().find(|preview| preview.id == id)?;
                            if preview.dismiss_animation.is_some() {
                                return None;
                            }
                            preview.video = Some(VideoPlayback {
                                decoder,
                                duration,
                                started: cx.background_executor().now(),
                                in_flight: false,
                                #[cfg(target_os = "macos")]
                                frames_released: false,
                                #[cfg(target_os = "macos")]
                                viewport_clipped: Cell::new(false),
                                #[cfg(target_os = "macos")]
                                visibility_changed,
                            });
                            cx.notify();
                            Some(())
                        })
                        .ok()
                        .flatten()
                });
                if attached.is_none() {
                    return;
                };
                let mut first_frame = true;
                #[cfg(target_os = "macos")]
                let mut wait_for_visibility = false;
                loop {
                    if !first_frame {
                        #[cfg(not(target_os = "macos"))]
                        cx.background_executor().timer(VIDEO_PLAYBACK_TICK).await;
                        #[cfg(target_os = "macos")]
                        {
                            let awake = if wait_for_visibility {
                                visibility_changes.recv().await.is_ok()
                            } else {
                                smol::future::or(
                                    async { visibility_changes.recv().await.is_ok() },
                                    async {
                                        cx.background_executor().timer(VIDEO_PLAYBACK_TICK).await;
                                        true
                                    },
                                )
                                .await
                            };
                            if !awake {
                                return;
                            }
                            wait_for_visibility = false;
                        }
                    }
                    first_frame = false;
                    let job = cx.update(|cx| {
                        handle
                            .update(cx, |view, _window, cx| {
                                let index =
                                    view.previews.iter().position(|preview| preview.id == id)?;
                                #[cfg(target_os = "macos")]
                                let clipped =
                                    view.preview_is_clipped(&view.previews[index], _window);
                                let now = cx.background_executor().now();
                                let menu_open = view
                                    .display_menu
                                    .is_open_for(&format!("preview-display-menu-{id}"), cx);
                                let preview = &mut view.previews[index];
                                let blur = preview.hovered
                                    || preview.is_busy()
                                    || menu_open
                                    || preview.dismiss_animation.is_some_and(|animation| {
                                        animation.controls_progress.is_some()
                                    });
                                let playback = preview.video.as_mut()?;
                                if playback.in_flight {
                                    return None;
                                }
                                #[cfg(target_os = "macos")]
                                {
                                    playback.viewport_clipped.set(clipped);
                                    if clipped
                                        || (!_window.is_visible() && !preview.thumbnail_loading)
                                    {
                                        wait_for_visibility = true;
                                        if !playback.frames_released {
                                            playback.decoder.release_frames();
                                            playback.frames_released = true;
                                        }
                                        return None;
                                    }
                                    playback.frames_released = false;
                                }
                                playback.in_flight = true;
                                let time = (!preview.thumbnail_loading).then(|| {
                                    looped_playhead(
                                        now.saturating_duration_since(playback.started)
                                            .as_secs_f64(),
                                        playback.duration,
                                    )
                                });
                                Some((playback.decoder.clone(), time, blur))
                            })
                            .ok()
                            .flatten()
                    });
                    let Some((decoder, time, blur)) = job else {
                        let alive = entity
                            .update(cx, |view, _| {
                                view.previews
                                    .iter()
                                    .any(|preview| preview.id == id && preview.video.is_some())
                            })
                            .unwrap_or(false);
                        if !alive {
                            return;
                        }
                        continue;
                    };
                    let frame = cx
                        .background_executor()
                        .spawn({
                            async move {
                                let frame = match time {
                                    Some(time) => decoder.frame_at(time),
                                    None => decoder.first_frame(),
                                };
                                frame.and_then(|frame| video_thumbnails(&frame, blur))
                            }
                        })
                        .await;
                    cx.update(|cx| {
                        apply_video_preview_frame(handle.into(), &entity, id, frame, cx)
                    });
                }
            })
            .detach();
            Some(())
        })
        .ok()
        .flatten();
    let _ = started;
}

type VideoThumbnails = (Arc<gpui::RenderImage>, Option<Arc<gpui::RenderImage>>);

fn apply_video_preview_frame(
    handle: AnyWindowHandle,
    entity: &gpui::WeakEntity<CapturePreviewWindow>,
    id: u64,
    frame: Option<VideoThumbnails>,
    cx: &mut App,
) {
    let dismiss_token = entity
        .update(cx, |view, cx| {
            let preview = view.previews.iter_mut().find(|preview| preview.id == id)?;
            if preview.dismiss_animation.is_some() {
                return None;
            }
            if let Some(playback) = preview.video.as_mut() {
                playback.in_flight = false;
            }
            if let Some((image, blurred)) = frame {
                release_preview_images(preview, cx, None);
                preview.image = Some(image);
                preview.blurred = blurred;
            }
            let finished_loading = std::mem::take(&mut preview.thumbnail_loading);
            cx.notify();
            (finished_loading && view.revealed).then(|| preview.dismiss_token.clone())
        })
        .ok()
        .flatten();
    if let Some(dismiss_token) = dismiss_token {
        schedule_auto_dismiss(handle, id, cx, dismiss_token);
    }
}

fn looped_playhead(elapsed: f64, duration: f64) -> f64 {
    if duration.is_finite() && duration > 0.0 {
        elapsed % duration
    } else {
        0.0
    }
}

/// A decoded BGRA frame as a cover-fit thumbnail, following `load_thumbnail`.
fn video_thumbnails(
    frame: &crate::video::decoder::DecodedFrame,
    blur: bool,
) -> Option<VideoThumbnails> {
    if frame.width == 0 || frame.height == 0 {
        return None;
    }
    let decoded = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra.to_vec())?;
    let mut buffer = image::DynamicImage::ImageRgba8(decoded)
        .resize_to_fill(
            PREVIEW_WIDTH as u32,
            PREVIEW_HEIGHT as u32,
            image::imageops::FilterType::Triangle,
        )
        .into_rgba8();
    if !blur {
        return Some((render_bgra_thumbnail(buffer), None));
    }
    for pixel in buffer.as_mut().as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let blurred = blur_thumbnail(&buffer);
    Some((render_thumbnail(buffer), blurred))
}

fn start_video_export(
    id: u64,
    project: PathBuf,
    entity: gpui::WeakEntity<CapturePreviewWindow>,
    cx: &mut App,
) {
    let prepared = registry::handle(WindowKind::CapturePreview, cx)
        .and_then(|stack| stack.downcast::<CapturePreviewWindow>())
        .and_then(|handle| {
            handle
                .update(cx, |view, _, cx| {
                    let preview = view.previews.iter_mut().find(|preview| preview.id == id)?;
                    if preview.finalizing || preview.is_busy() || preview.completed.is_some() {
                        return None;
                    }
                    preview.busy = true;
                    preview.dismiss_token.fetch_add(1, Ordering::Relaxed);
                    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
                    preview.export_cancel = Some(cancel.clone());
                    let progress = Arc::new(AtomicU32::new(0));
                    preview.export_progress = Some(progress.clone());
                    cx.notify();
                    Some((cancel, progress, preview.dismiss_token.clone()))
                })
                .ok()
                .flatten()
        });
    let Some((cancel, progress, dismiss_token)) = prepared else {
        return;
    };
    cx.spawn(async move |cx| {
        let output = cx
            .background_executor()
            .spawn({
                let project = project.clone();
                async move {
                    let (state, camera_segments_were_saved) = crate::windows::video_editor::model::
                        load_state_with_camera_segments_presence(&project);
                    let output = crate::video::export::default_output_path(&project, &state);
                    let result = crate::video::export::run_with_camera_segments_presence(
                        crate::video::export::Request {
                            project,
                            output,
                            state,
                        },
                        camera_segments_were_saved,
                        &mut |value| {
                            progress
                                .store((value.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed);
                        },
                        &|| !cancel.load(Ordering::Relaxed),
                    );
                    (result, cancel.load(Ordering::Relaxed))
                }
            })
            .await;
        cx.update(|cx| {
            let handle = registry::handle(WindowKind::CapturePreview, cx);
            let _ = entity.update(cx, |view, cx| {
                if let Some(preview) = view.previews.iter_mut().find(|preview| preview.id == id) {
                    preview.busy = false;
                    preview.export_cancel = None;
                    preview.export_progress = None;
                }
                cx.notify();
            });
            let (result, cancelled) = output;
            let copied = match result {
                Ok(path) if !cancelled => {
                    let copied = crate::system::clipboard::ClipboardService::write_file(&path);
                    if !copied {
                        report_copy_failure(cx, "could not put the recording on the clipboard");
                    }
                    copied
                }
                Ok(_) => false,
                Err(error) => {
                    if !cancelled {
                        crate::windows::toast::Toast::show(cx, "Export failed", error);
                    }
                    false
                }
            };
            let Some(handle) = handle else {
                return;
            };
            if !copied {
                schedule_auto_dismiss(handle, id, cx, dismiss_token);
                return;
            }
            mark_completed(handle, id, cx);
            hold_then_dismiss(handle, id, cx);
        });
    })
    .detach();
}

fn cancel_video_export(id: u64, cx: &mut App) {
    if let Some(stack) = registry::handle(WindowKind::CapturePreview, cx) {
        let _ = stack.downcast::<CapturePreviewWindow>().map(|handle| {
            handle.update(cx, |view, _, _| {
                if let Some(preview) = view.previews.iter().find(|preview| preview.id == id) {
                    if let Some(cancel) = preview.export_cancel.as_ref() {
                        cancel.store(true, Ordering::Relaxed);
                    }
                }
            })
        });
    }
}

fn selected_display(cx: &mut App) -> Option<(Bounds<gpui::Pixels>, gpui::DisplayId)> {
    let displays = cx.displays();
    let config = crate::state::state(cx).config.get();
    config
        .preview
        .display_id
        .and_then(|id| {
            displays
                .iter()
                .find(|display| {
                    crate::system::work_area::display_preference_id(display.id()) == Some(id)
                })
                .cloned()
        })
        .or_else(|| cx.primary_display())
        .or_else(|| displays.first().cloned())
        .map(|display| {
            (
                crate::system::work_area::work_area(display.as_ref()),
                display.id(),
            )
        })
}

fn preview_stack_height(count: usize) -> f32 {
    count as f32 * PREVIEW_HEIGHT
        + count.saturating_sub(1) as f32 * PREVIEW_STACK_GAP
        + PREVIEW_SHADOW_PADDING * 2.0
}

fn preview_stack_size(count: usize) -> Size<Pixels> {
    size(
        px(PREVIEW_WIDTH + PREVIEW_SHADOW_PADDING * 2.0),
        px(preview_stack_height(count)),
    )
}

#[cfg(target_os = "macos")]
fn preview_stack_top_inset(height: Pixels, bottom_aligned: bool) -> Pixels {
    if bottom_aligned {
        (px(preview_stack_height(PREVIEW_MAX_STACK)) - height).max(px(0.))
    } else {
        px(0.)
    }
}

fn preview_bottom_aligned(cx: &mut App) -> bool {
    matches!(
        PreviewCorner::parse(&crate::state::state(cx).config.get().preview.corner),
        PreviewCorner::BottomLeft | PreviewCorner::BottomRight
    )
}

fn preview_stack_placement(
    cx: &mut App,
    count: usize,
) -> (Bounds<gpui::Pixels>, Option<gpui::DisplayId>) {
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
    let count = if cfg!(target_os = "macos") {
        count.clamp(1, PREVIEW_MAX_STACK)
    } else {
        PREVIEW_MAX_STACK
    };
    (
        preview_fitted_stack_bounds(display, corner, count),
        display_id,
    )
}

fn preview_fitted_stack_bounds(
    display: Bounds<gpui::Pixels>,
    corner: PreviewCorner,
    count: usize,
) -> Bounds<gpui::Pixels> {
    let count = count.clamp(1, PREVIEW_MAX_STACK);
    let index = if matches!(
        corner,
        PreviewCorner::BottomLeft | PreviewCorner::BottomRight
    ) {
        count - 1
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
    let bounds = Bounds::new(
        gpui::point(
            px(x - PREVIEW_SHADOW_PADDING),
            px(y - PREVIEW_SHADOW_PADDING),
        ),
        preview_stack_size(count),
    );
    #[cfg(target_os = "macos")]
    if display.size.width > px(0.) && display.size.height > px(0.) {
        let viewport = bounds.size.min(&display.size);
        let origin = bounds.origin.clamp(
            &display.origin,
            &gpui::point(
                display.right() - viewport.width,
                display.bottom() - viewport.height,
            ),
        );
        return Bounds::new(origin, viewport);
    }
    bounds
}

fn configure_stack_window(window: &mut Window, cx: &mut App, count: usize) {
    let (bounds, display_id) = preview_stack_placement(cx, count);
    #[cfg(all(target_os = "macos", not(test)))]
    {
        super::pin_macos_window_contents(window, preview_bottom_aligned(cx));
        if super::set_macos_window_bounds(window, bounds) {
            window.bounds_changed(cx);
        } else {
            #[cfg(feature = "parity")]
            {
                let scale = window.scale_factor();
                window.resize(bounds.size);
                window.bounds_changed(cx);
                window.set_scale_factor(scale);
            }
        }
    }
    #[cfg(windows)]
    if !cfg!(test) {
        if let Some(hwnd) = crate::windows::window_hwnd(window) {
            let scale = display_id
                .and_then(|id| cx.find_display(id))
                .map(|display| crate::system::work_area::capture_scale_factor(display.as_ref(), cx))
                .unwrap_or(window.scale_factor());
            crate::system::window_composition::apply_window_bounds(hwnd, bounds, scale);
            let bottom_aligned = preview_bottom_aligned(cx);
            window.on_next_frame(move |window, _| {
                configure_preview_stack_window(window, count, bottom_aligned)
            });
        }
    }
    #[cfg(test)]
    window.resize(bounds.size);
    #[cfg(all(test, target_os = "macos"))]
    window.bounds_changed(cx);
    #[cfg(not(windows))]
    let _ = (window, count, bounds, display_id);
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
fn decode_preview_fallback(path: &Path) -> Option<image::RgbaImage> {
    let target_w = PREVIEW_WIDTH as u32;
    let target_h = PREVIEW_HEIGHT as u32;
    let decoded = crate::render::images::open(path).ok()?;
    let scaled = decoded
        .resize_to_fill(target_w, target_h, image::imageops::FilterType::Triangle)
        .into_rgba8();
    drop(decoded);
    crate::system::memory::compact_heap();
    Some(scaled)
}

fn build_preview_thumbnails(
    scaled: image::RgbaImage,
) -> (
    Option<Arc<gpui::RenderImage>>,
    Option<Arc<gpui::RenderImage>>,
) {
    let blurred = blur_thumbnail(&scaled);
    (Some(render_thumbnail(scaled)), blurred)
}

fn load_preview_thumbnails(
    path: &Path,
) -> (
    Option<Arc<gpui::RenderImage>>,
    Option<Arc<gpui::RenderImage>>,
) {
    let target_w = PREVIEW_WIDTH as u32;
    let target_h = PREVIEW_HEIGHT as u32;
    let Some(scaled) = crate::thumbnails::decode_cover_rgba(path, target_w, target_h)
        .or_else(|| decode_preview_fallback(path))
    else {
        return (None, None);
    };
    build_preview_thumbnails(scaled)
}

fn blur_thumbnail(scaled: &image::RgbaImage) -> Option<Arc<gpui::RenderImage>> {
    crate::editor::export::from_rgba(scaled).map(|mut pixmap| {
        crate::render::blur::blur(&mut pixmap, BLUR_SIGMA);
        render_thumbnail(crate::editor::export::to_rgba(&pixmap))
    })
}

fn render_thumbnail(mut image: image::RgbaImage) -> Arc<gpui::RenderImage> {
    for pixel in image.as_mut().as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    render_bgra_thumbnail(image)
}

fn render_bgra_thumbnail(image: image::RgbaImage) -> Arc<gpui::RenderImage> {
    let frame = image::Frame::new(image);
    Arc::new(gpui::RenderImage::new(smallvec::smallvec![frame]))
}

/// The preset the user starred, which is what `usePolishCopy` looks up via
/// `wallpaper.defaultPresetId`. `None` -- the default -- means Electron shows no
/// Polish button either, so neither does this.
pub(crate) fn starred_wallpaper_preset(
    wallpaper: &crate::config::schema::WallpaperConfig,
) -> Option<&crate::config::schema::WallpaperPreset> {
    let id = wallpaper.default_preset_id.as_deref()?;
    wallpaper.presets.iter().find(|preset| preset.id == id)
}

fn starred_preset(cx: &mut App) -> Option<crate::config::schema::WallpaperPreset> {
    crate::state::state(cx)
        .config
        .read(|config| starred_wallpaper_preset(&config.wallpaper).cloned())
}

pub(crate) fn prepare_screenshot_copy(
    path: &Path,
    polish: Option<&crate::config::schema::WallpaperPreset>,
    scale_factor: f32,
) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|_| format!("could not read {}", path.display()))?;
    let decoded =
        crate::render::images::decode(&bytes).map_err(|_| "could not decode the capture")?;
    let image = match polish {
        None => decoded.into_rgba8(),
        Some(preset) => {
            let mut settings = crate::editor::wallpaper::WallpaperSettings::default();
            crate::editor::wallpaper::apply_preset(&mut settings, preset);
            let native_scale = crate::editor::wallpaper::native_scale(
                f64::from(decoded.width()),
                f64::from(decoded.height()),
                f64::from(scale_factor),
            );
            crate::editor::export::compose(
                Some(&decoded),
                decoded.width(),
                decoded.height(),
                &[],
                &settings.for_scale(native_scale),
                native_scale,
            )
        }
    };
    crate::render::images::encode_srgb_png(&image)
        .map_err(|error| format!("could not prepare the clipboard image: {error}"))
}

fn copy_screenshot(
    preview_entity: &gpui::WeakEntity<CapturePreviewWindow>,
    id: u64,
    polish: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let preset = if polish {
        let Some(preset) = starred_preset(cx) else {
            return;
        };
        Some(preset)
    } else {
        None
    };
    let Ok(Some(path)) = preview_entity.update(cx, |view, cx| {
        if view.previews.iter().any(|preview| preview.copying) {
            return None;
        }
        let preview = view.previews.iter_mut().find(|preview| preview.id == id)?;
        if preview.kind != PreviewKind::Screenshot
            || preview.is_busy()
            || preview.finalizing
            || preview.completed.is_some()
            || preview.dismiss_animation.is_some()
        {
            return None;
        }
        preview.copying = true;
        cx.notify();
        Some(preview.path.clone())
    }) else {
        return;
    };
    let scale = window.scale_factor();
    let handle = window.window_handle();
    let preview_entity = preview_entity.clone();
    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move { prepare_screenshot_copy(&path, preset.as_ref(), scale) })
            .await;
        cx.update(|cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = preview_entity.update(cx, |view, cx| {
                    let Some(preview) = view.previews.iter_mut().find(|preview| preview.id == id)
                    else {
                        return;
                    };
                    preview.copying = false;
                    if preview.dismiss_animation.is_some() {
                        return;
                    }
                    match result {
                        Ok(bytes) => {
                            crate::system::clipboard::ClipboardService::write_png(cx, bytes);
                            crate::system::memory::compact_heap();
                            begin_remove_preview(
                                view,
                                id,
                                DismissBehavior::PreserveControls,
                                window,
                                cx,
                            );
                        }
                        Err(error) => {
                            report_copy_failure(cx, &error);
                            cx.notify();
                        }
                    }
                });
            });
        });
    })
    .detach();
}

fn report_copy_failure(cx: &mut App, message: &str) {
    crate::windows::toast::Toast::show(cx, "Copy failed", message.to_string());
}

fn delete_preview(
    preview_entity: &gpui::WeakEntity<CapturePreviewWindow>,
    id: u64,
    window: &mut Window,
    cx: &mut App,
) {
    let Ok(Some((path, kind))) = preview_entity.update(cx, |view, cx| {
        let preview = view.previews.iter_mut().find(|preview| preview.id == id)?;
        if preview.finalizing
            || preview.is_busy()
            || preview.completed.is_some()
            || preview.dismiss_animation.is_some()
        {
            return None;
        }
        preview.deleting = true;
        cx.notify();
        let kind = match preview.kind {
            PreviewKind::Video => crate::history_store::HistoryItemType::Video,
            PreviewKind::Screenshot => crate::history_store::HistoryItemType::Screenshot,
        };
        Some((preview.path.clone(), kind))
    }) else {
        return;
    };
    let handle = window.window_handle();
    let preview_entity = preview_entity.clone();
    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move { crate::history_store::delete_path(&path, kind) })
            .await;
        cx.update(|cx| {
            let deleted = report_delete_result(
                result,
                kind == crate::history_store::HistoryItemType::Video,
                cx,
            );
            let _ = handle.update(cx, |_, window, cx| {
                let _ = preview_entity.update(cx, |view, cx| {
                    let Some(preview) = view.previews.iter_mut().find(|preview| preview.id == id)
                    else {
                        return;
                    };
                    preview.deleting = false;
                    if deleted {
                        begin_remove_preview(
                            view,
                            id,
                            DismissBehavior::PreserveControls,
                            window,
                            cx,
                        );
                    } else {
                        cx.notify();
                    }
                });
            });
        });
    })
    .detach();
}

fn report_delete_result(
    result: crate::history_store::DeleteOutcome,
    is_video: bool,
    cx: &mut App,
) -> bool {
    use crate::history_store::DeleteOutcome;

    match result {
        DeleteOutcome::Deleted => {}
        DeleteOutcome::DeletedWithBackupWarning(path) => {
            crate::windows::toast::Toast::show(
                cx,
                "History recovery copy remains",
                format!("Could not remove the saved recovery copy at {path}"),
            );
        }
        DeleteOutcome::HistoryPersistenceFailed(error) => {
            crate::windows::toast::Toast::show(
                cx,
                "History update failed",
                format!("The capture was deleted, but its history entry may return after restart: {error}"),
            );
        }
        DeleteOutcome::SourceDeletionFailed
        | DeleteOutcome::HistoryUnavailable
        | DeleteOutcome::NotFound => {
            let message = if is_video {
                "Could not delete the recording."
            } else {
                "Could not delete the screenshot."
            };
            crate::windows::toast::Toast::show(cx, "Delete failed", message);
            return false;
        }
    }
    true
}

fn edit_preview(
    preview_entity: &gpui::WeakEntity<CapturePreviewWindow>,
    id: u64,
    window: &mut Window,
    cx: &mut App,
) {
    let edited = preview_entity.update(cx, |view, cx| {
        let preview = view.previews.iter().find(|preview| preview.id == id)?;
        if preview.dismiss_animation.is_some()
            || preview.finalizing
            || preview.is_busy()
            || preview.completed.is_some()
        {
            return None;
        }
        let source = (preview.kind, preview.path.to_string_lossy().into_owned());
        begin_remove_preview(view, id, DismissBehavior::PreserveControls, window, cx);
        Some(source)
    });
    let Ok(Some((kind, path))) = edited else {
        return;
    };
    match kind {
        PreviewKind::Screenshot => crate::open_editor_async(cx, path, false),
        PreviewKind::Video => crate::windows::video_editor::VideoEditorWindow::open(cx, Some(path)),
    }
}

fn begin_remove_preview(
    view: &mut CapturePreviewWindow,
    id: u64,
    behavior: DismissBehavior,
    window: &mut Window,
    cx: &mut Context<CapturePreviewWindow>,
) {
    let now = cx.background_executor().now();
    let Some(index) = view.previews.iter().position(|preview| preview.id == id) else {
        return;
    };
    if view.previews[index].dismiss_animation.is_some() {
        return;
    }
    cancel_thumbnail_load(&mut view.previews[index]);
    if view
        .display_menu
        .is_open_for(&format!("preview-display-menu-{id}"), cx)
    {
        view.display_menu.close(window, cx);
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
    preview.video = None;
    let bottom_aligned = preview_bottom_aligned(cx);
    let layout_generation = start_layout_transition(view, bottom_aligned, now);
    cx.notify();
    let removal_handle = window.window_handle();
    cx.spawn(async move |_, cx| {
        cx.background_executor()
            .timer(Duration::from_millis(PREVIEW_EXIT_MS))
            .await;
        cx.update(|cx| {
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
        cx.update(|cx| {
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
    if let Some(preview) = view.previews.iter_mut().find(|preview| preview.id == id) {
        cancel_thumbnail_load(preview);
        release_preview_images(preview, cx, Some(window));
    }
    view.previews.retain(|preview| preview.id != id);
    if view.previews.is_empty() {
        registry::forget(WindowKind::CapturePreview, cx);
        window.remove_window();
        return;
    }
    #[cfg(target_os = "macos")]
    configure_stack_window(window, cx, view.window_preview_count());
    cx.notify();
}

fn finished_progress(started: Instant, now: Instant) -> f32 {
    let elapsed = now.saturating_duration_since(started).as_secs_f32() * 1000.0;
    (elapsed / crate::ui::preview::FINISHED_ENTER_MS as f32).clamp(0.0, 1.0)
}

fn export_fraction(progress: &Arc<AtomicU32>) -> f32 {
    (progress.load(Ordering::Relaxed) as f32 / 1000.0).clamp(0.0, 1.0)
}

fn mark_completed(handle: AnyWindowHandle, id: u64, cx: &mut App) {
    let Some(handle) = handle.downcast::<CapturePreviewWindow>() else {
        return;
    };
    let _ = handle.update(cx, |view, _window, cx| {
        let Some(preview) = view.previews.iter_mut().find(|preview| preview.id == id) else {
            return;
        };
        preview.busy = false;
        preview.export_progress = None;
        preview.completed = Some(cx.background_executor().now());
        cx.notify();
    });
}

fn hold_then_dismiss(handle: AnyWindowHandle, id: u64, cx: &mut App) {
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(UPLOAD_DONE_DISPLAY_MS))
            .await;
        cx.update(|cx| {
            let Some(handle) = handle.downcast::<CapturePreviewWindow>() else {
                return;
            };
            let _ = handle.update(cx, |view, window, cx| {
                begin_remove_preview(view, id, DismissBehavior::Automatic, window, cx)
            });
        });
    })
    .detach();
}

impl CapturePreviewWindow {
    fn show_controls(hovered: bool, busy: bool, completed: bool) -> bool {
        !completed && (hovered || busy)
    }

    fn advance_hover(preview: &mut CapturePreview, hovered: bool, now: Instant) -> (f32, bool) {
        let target = if Self::show_controls(hovered, preview.is_busy(), preview.completed.is_some())
        {
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
        menu_open: bool,
        keyboard_controls: bool,
        now: Instant,
    ) -> (bool, f32, bool) {
        if let Some(progress) = preview
            .dismiss_animation
            .and_then(|animation| animation.controls_progress)
        {
            preview.last_frame = None;
            return (true, progress, false);
        }
        let controls_active = preview.hovered && window_hovered || menu_open || keyboard_controls;
        let (progress, animating) = Self::advance_hover(preview, controls_active, now);
        (
            Self::show_controls(
                controls_active,
                preview.is_busy(),
                preview.completed.is_some(),
            ),
            progress,
            animating,
        )
    }

    fn render_preview(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
        theme: &crate::theme::vars::ThemeVars,
        polish: Option<&crate::config::schema::WallpaperPreset>,
        displays: &[(gpui::DisplayId, bool)],
        window_hovered: bool,
        target_y: f32,
        enter_offset: f32,
        now: Instant,
        needs_frame: &mut bool,
    ) -> gpui::AnyElement {
        let preview_entity = cx.entity().downgrade();
        let id = self.previews[index].id;
        let menu_open = self.display_menu.is_open(cx)
            && self
                .display_menu
                .is_open_for(&format!("preview-display-menu-{id}"), cx);
        if menu_open {
            self.clear_native_tooltip(window);
        }
        let keyboard_controls = self.keyboard_controls_visible(window);
        self.tooltip_presenter
            .set_hover_suppressed(keyboard_controls);
        let stack_copying = self.previews.iter().any(|preview| preview.copying);
        #[cfg(target_os = "macos")]
        let wrap_reveal = |chip_id: gpui::ElementId, element: gpui::AnyElement| {
            preview::focus_reveal(chip_id, preview_entity.clone(), element)
        };
        #[cfg(not(target_os = "macos"))]
        let wrap_reveal = |_chip_id: gpui::ElementId, element: gpui::AnyElement| element;
        let preview = &mut self.previews[index];
        let is_video = preview.kind == PreviewKind::Video;
        let polish = if is_video { None } else { polish };
        let path = preview.path.clone();
        let dismiss_token = preview.dismiss_token.clone();
        let busy = preview.busy;
        let finished = preview.completed.map(|at| finished_progress(at, now));
        let export_progress = preview.export_progress.clone();
        let deleting = preview.deleting;
        let copying = preview.copying;
        let finalizing = preview.finalizing;
        let gated = finalizing || preview.is_busy() || finished.is_some();
        let (show_controls, progress, animating) =
            Self::control_frame(preview, window_hovered, menu_open, keyboard_controls, now);
        let (layout_y, layout_animating) = advance_layout(preview, target_y, enter_offset, now);
        let (opacity, opacity_animating) = preview_opacity(preview, now);
        *needs_frame |= animating || layout_animating || opacity_animating;
        *needs_frame |= finished.is_some_and(|value| value < 1.0) || export_progress.is_some();
        // `scale-105` over `duration-200` rather than an instant jump.
        let scale = 1.0 + (PREVIEW_HOVER_SCALE - 1.0) * progress;
        let image_w = PREVIEW_WIDTH * scale;
        let image_h = PREVIEW_HEIGHT * scale;
        let has_multiple_displays = displays.len() > 1;

        let mut root = div()
            .id(("capture-preview", id))
            .relative()
            .size_full()
            .overflow_hidden()
            .rounded(px(PREVIEW_RADIUS))
            .bg(theme.muted_background)
            .border_1()
            .border_color(theme.border)
            .on_mouse_down(MouseButton::Left, {
                let preview_entity = preview_entity.clone();
                move |event: &gpui::MouseDownEvent, window, cx| {
                    if event.click_count >= 2 {
                        edit_preview(&preview_entity, id, window, cx);
                    }
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
        let placeholder = if preview.thumbnail_loading {
            crate::ui::icon::spinner_element(
                gpui::ElementId::Name(format!("capture-preview-thumbnail-{id}").into()),
                px(20.0),
            )
        } else {
            icon_element(if is_video { "film" } else { "image" }, px(48.0))
        };
        if !is_video {
            let drag = PreviewDrag {
                path: path.clone(),
                image: preview.image.clone(),
            };
            root = root
                .on_drag(drag, |drag, _, _, cx| {
                    cx.new(|_| PreviewDragGhost {
                        image: drag.image.clone(),
                    })
                })
                .external_drag_payload(|drag: &PreviewDrag, _, _| {
                    Some(gpui::ExternalDragPayload::Files(gpui::FileDragPaths::new(
                        [(drag.path.clone(), false)],
                    )))
                });
        }
        root = root.child(match thumbnail {
            Some(render_image) => {
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                {
                    let render_image = render_image.clone();
                    gpui::canvas(
                        |_, _, _| {},
                        move |bounds, _: (), window, _| {
                            let natural = render_image.size(0);
                            let nat_w = natural.width.0 as f32;
                            let nat_h = natural.height.0 as f32;
                            let (fitted_w, fitted_h) = if nat_w > 0.0 && nat_h > 0.0 {
                                let cover = (image_w / nat_w).max(image_h / nat_h);
                                (nat_w * cover, nat_h * cover)
                            } else {
                                (image_w, image_h)
                            };
                            let image_bounds = Bounds::new(
                                gpui::point(
                                    bounds.origin.x
                                        + px((f32::from(bounds.size.width) - fitted_w) * 0.5),
                                    bounds.origin.y
                                        + px((f32::from(bounds.size.height) - fitted_h) * 0.5),
                                ),
                                size(px(fitted_w), px(fitted_h)),
                            );
                            let _ = window.paint_image(
                                bounds,
                                image_bounds,
                                gpui::Corners::all(px(PREVIEW_RADIUS - 1.0)),
                                render_image,
                                0,
                                false,
                            );
                        },
                    )
                    .absolute()
                    .inset_0()
                    .into_any_element()
                }
                #[cfg(not(any(target_os = "macos", target_os = "linux")))]
                div()
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
                    .into_any_element()
            }
            None => div()
                .absolute()
                .inset_0()
                .when(cfg!(any(target_os = "macos", target_os = "linux")), |el| {
                    el.rounded(px(PREVIEW_RADIUS - 1.0))
                })
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.muted_background)
                .text_color(theme.muted_foreground)
                .child(
                    div()
                        .when(is_video && show_controls, |el| el.opacity(1.0 - progress))
                        .child(placeholder),
                )
                .into_any_element(),
        });

        if show_controls {
            root = root
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .when(cfg!(any(target_os = "macos", target_os = "linux")), |el| {
                            el.rounded(px(PREVIEW_RADIUS - 1.0))
                        })
                        // `bg-black/25` behind `animate-in fade-in duration-200`.
                        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.25 * progress)),
                )
                .child(
                    div()
                        .absolute()
                        .opacity(progress)
                        .top(px(PREVIEW_CONTROL_INSET))
                        .left(px(PREVIEW_CONTROL_INSET))
                        .when(menu_open, |el| el.hidden())
                        .child(wrap_reveal(
                            ("preview-close", id).into(),
                            preview::circle(
                                &self.tooltip_presenter,
                                ("preview-close", id),
                                "x",
                                false,
                                "Close preview",
                                TooltipPlacement::BottomStart,
                                theme,
                                theme.destructive,
                                {
                                    let preview_entity = preview_entity.clone();
                                    move |window, cx| {
                                        if gated && !finalizing {
                                            return;
                                        }
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
                            ),
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .opacity(progress)
                        .top(px(PREVIEW_CONTROL_INSET))
                        .right(px(PREVIEW_CONTROL_INSET))
                        .when(menu_open, |el| el.hidden())
                        .child(wrap_reveal(
                            ("preview-delete", id).into(),
                            preview::circle(
                                &self.tooltip_presenter,
                                ("preview-delete", id),
                                "trash-2",
                                deleting,
                                if deleting {
                                    "Deleting…"
                                } else if is_video {
                                    "Delete recording"
                                } else {
                                    "Delete screenshot"
                                },
                                TooltipPlacement::BottomEnd,
                                theme,
                                theme.destructive,
                                {
                                    let preview_entity = preview_entity.clone();
                                    move |window, cx| {
                                        if gated {
                                            return;
                                        }
                                        delete_preview(&preview_entity, id, window, cx);
                                    }
                                },
                            ),
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
                        .when(menu_open, |el| el.hidden())
                        .when(is_video && !gated, |el| {
                            el.child(preview::pill(
                                &self.tooltip_presenter,
                                ("preview-copy-video", id),
                                "Copy",
                                Some("Copy".into()),
                                TooltipPlacement::Top,
                                theme,
                                {
                                    let path = path.clone();
                                    let preview_entity = preview_entity.clone();
                                    move |_window, cx| {
                                        start_video_export(id, path.clone(), preview_entity.clone(), cx);
                                        cx.stop_propagation();
                                    }
                                },
                            ))
                        })
                        .when(is_video && busy, |el| {
                            el.child(preview::pill(
                                &self.tooltip_presenter,
                                ("preview-exporting", id),
                                "Exporting...",
                                Some("Exporting...".into()),
                                TooltipPlacement::Top,
                                theme,
                                |_, cx| {
                                    cx.stop_propagation();
                                },
                            ))
                        })
                        .when(finalizing, |el| {
                            el.child(
                                div()
                                    .text_sm()
                                    .px(px(10.))
                                    .py(px(4.))
                                    .rounded_full()
                                    .bg(theme.popover)
                                    .text_color(theme.popover_foreground)
                                    .child(if is_video { "Saving recording…" } else { "Saving capture…" }),
                            )
                        })
                        .when_some(polish.filter(|_| !gated && !stack_copying), |el, preset| {
                            let tooltip = format!("Copy with \"{}\"", preset.name);
                            el.child(preview::pill(
                                &self.tooltip_presenter,
                                ("preview-polish", id),
                                "Polish",
                                Some(tooltip.into()),
                                TooltipPlacement::Bottom,
                                theme,
                                {
                                    let preview_entity = preview_entity.clone();
                                    move |window, cx| copy_screenshot(&preview_entity, id, true, window, cx)
                                },
                            ))
                        })
                        .when(!gated, |el| {
                            el.child(wrap_reveal(
                                ("preview-edit", id).into(),
                                preview::pill(
                                    &self.tooltip_presenter,
                                    ("preview-edit", id),
                                    "Edit",
                                    None,
                                    TooltipPlacement::Top,
                                    theme,
                                    {
                                        let preview_entity = preview_entity.clone();
                                        move |window, cx| {
                                            edit_preview(&preview_entity, id, window, cx);
                                        }
                                    },
                                ),
                            ))
                        })
                        .when(is_video && busy, |el| {
                            el.child(preview::pill(
                                &self.tooltip_presenter,
                                ("preview-cancel-export", id),
                                "Cancel",
                                Some("Cancel".into()),
                                TooltipPlacement::Top,
                                theme,
                                move |_, cx| {
                                    cancel_video_export(id, cx);
                                    cx.stop_propagation();
                                },
                            ))
                        }),
                )
                .when(!is_video, |el| {
                    el.child(
                        div()
                            .absolute()
                            .opacity(progress)
                            .bottom(px(PREVIEW_CONTROL_INSET))
                            .left(px(PREVIEW_CONTROL_INSET))
                            .when(menu_open, |el| el.hidden())
                            .child(preview::circle(
                                &self.tooltip_presenter,
                                ("preview-copy", id),
                                "copy",
                                copying,
                                if copying { "Copying…" } else { "Copy" },
                                TooltipPlacement::TopStart,
                                theme,
                                theme.primary,
                            {
                                let preview_entity = preview_entity.clone();
                                move |window, cx| {
                                    if !gated && !stack_copying {
                                        copy_screenshot(&preview_entity, id, false, window, cx);
                                    }
                                    cx.stop_propagation();
                                }
                            },
                        )),
                    )
                })
                .when(is_video, |el| {
                    el.child(
                        div()
                            .absolute()
                            .opacity(progress)
                            .bottom(px(PREVIEW_CONTROL_INSET))
                            .left(px(PREVIEW_CONTROL_INSET))
                            .when(menu_open, |el| el.hidden())
                            .child(preview::circle(
                                &self.tooltip_presenter,
                                ("preview-show-in-folder", id),
                                "folder-open",
                                false,
                                "Show in Folder",
                                TooltipPlacement::TopStart,
                                theme,
                                theme.primary,
                                {
                                    let path = path.clone();
                                    move |_, cx| {
                                        if !deleting {
                                            cx.reveal_path(&path);
                                        }
                                        cx.stop_propagation();
                                    }
                                },
                            )),
                    )
                })
                .when(!is_video, |el| {
                    el.child(
                        div()
                            .absolute()
                            .opacity(progress)
                            .bottom(px(PREVIEW_CONTROL_INSET))
                            .right(px(PREVIEW_CONTROL_INSET))
                            .when(menu_open, |el| el.hidden())
                            .child(preview::circle(
                                &self.tooltip_presenter,
                                ("preview-upload", id),
                                "cloud-upload",
                                busy,
                                "Upload to Cloud",
                                TooltipPlacement::TopEnd,
                                theme,
                                theme.primary,
                            {
                                let path = path.clone();
                                let preview_entity = preview_entity.clone();
                                move |window, cx| {
                                    if gated {
                                        cx.stop_propagation();
                                        return;
                                    }
                                    let config = crate::state::state(cx).config.get().cloud;
                                    let path = path.clone();
                                    let handle = window.window_handle();
                                    let dismiss_token = preview_entity
                                        .update(cx, |view, cx| {
                                            let preview = view
                                                .previews
                                                .iter_mut()
                                                .find(|preview| preview.id == id)?;
                                            if preview.is_busy() || preview.completed.is_some() {
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
                                        cx.update(|cx| {
                                            let _ = task_entity.update(cx, |view, cx| {
                                                if let Some(preview) = view
                                                    .previews
                                                    .iter_mut()
                                                    .find(|preview| preview.id == id)
                                                {
                                                    preview.busy = false;
                                                    if succeeded {
                                                        preview.completed =
                                                            Some(cx.background_executor().now());
                                                    }
                                                }
                                                cx.notify();
                                            });
                                            match result {
                                                Ok(url) => {
                                                    crate::system::clipboard::ClipboardService::write_text(
                                                        cx, url,
                                                    );
                                                    crate::windows::toast::Toast::show_transient(
                                                        cx,
                                                        "Image Uploaded",
                                                        "Link copied to clipboard",
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
                                            cx.update(|cx| {
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
                                            cx.update(|cx| {
                                                schedule_auto_dismiss(handle, id, cx, dismiss_token)
                                            });
                                        }
                                    })
                                    .detach();
                                    cx.stop_propagation();
                                }
                            },
                        )),
                    )
                });

            if has_multiple_displays {
                let owner = format!("preview-display-menu-{id}");
                let displays = displays.to_vec();
                let handle = self.display_menu.clone();
                let placement = owner.clone();
                root = root.child(
                    div()
                        .absolute()
                        .opacity(progress)
                        .bottom(px(PREVIEW_CONTROL_INSET + PREVIEW_CONTROL + 4.0))
                        .right(px(PREVIEW_CONTROL_INSET))
                        .child(preview::circle(
                            &self.tooltip_presenter,
                            owner.clone(),
                            "monitor",
                            false,
                            "Move previews to another display",
                            TooltipPlacement::TopEnd,
                            theme,
                            theme.primary,
                            move |window, cx| {
                                let current =
                                    crate::state::state(cx).config.get().preview.display_id;
                                let ids = displays
                                    .iter()
                                    .map(|&(id, primary)| {
                                        (
                                            id,
                                            primary,
                                            crate::system::work_area::display_preference_id(id),
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                let selected = ids
                                    .iter()
                                    .find(|(_, _, id)| id.is_some() && *id == current)
                                    .or_else(|| ids.iter().find(|(_, primary, _)| *primary))
                                    .or_else(|| ids.first())
                                    .map(|(id, _, _)| *id);
                                let mut builder = crate::ui::menu::MenuBuilder::new();
                                for (index, &(display_id, primary, _)) in ids.iter().enumerate() {
                                    let label = format!(
                                        "Display {}{}",
                                        index + 1,
                                        if primary { " (Primary)" } else { "" }
                                    );
                                    builder = builder.item(
                                        crate::ui::menu::MenuItem::new(label)
                                            .trailing_check(selected == Some(display_id))
                                            .on_select(move |_window, app| {
                                                if app.find_display(display_id).is_none() {
                                                    return;
                                                }
                                                let Some(id) =
                                                    crate::system::work_area::display_preference_id(
                                                        display_id,
                                                    )
                                                else {
                                                    return;
                                                };
                                                let config =
                                                    crate::state::state(app).config.clone();
                                                config.update(|settings| {
                                                    settings.preview.display_id = Some(id);
                                                });
                                                app.defer(CapturePreviewWindow::reposition);
                                            }),
                                    );
                                }
                                handle.toggle(
                                    crate::ui::menu::MenuPlacement::below(placement.clone())
                                        .aligned_right()
                                        .offset(gpui::point(
                                            px(PREVIEW_CONTROL),
                                            px(PREVIEW_CONTROL_INSET * 2.0 + PREVIEW_CONTROL + 4.0
                                                - PREVIEW_HEIGHT),
                                        ))
                                        .min_width(px(PREVIEW_WIDTH - PREVIEW_CONTROL_INSET * 2.0))
                                        .max_width(px(PREVIEW_WIDTH - PREVIEW_CONTROL_INSET * 2.0))
                                        .max_height(px(
                                            MENU_ROW_HEIGHT * 3.0 + MENU_PANEL_PADDING * 2.0
                                        )),
                                    builder.build(),
                                    window,
                                    cx,
                                );
                                cx.stop_propagation();
                            },
                        ))
                        .child(self.display_menu.render_dropdown(&owner, cx)),
                );
            }
        }

        if let Some(fraction) = export_progress.as_ref().map(export_fraction) {
            root = root.child(preview::progress_bar(fraction, theme));
        }
        if let Some(value) = finished {
            root = root.child(preview::finished_badge(value, theme));
        }

        div()
            .id(("capture-preview-hover", id))
            .absolute()
            .top(px(layout_y))
            .left(px(PREVIEW_SHADOW_PADDING))
            .w(px(PREVIEW_WIDTH))
            .h(px(PREVIEW_HEIGHT))
            .opacity(opacity)
            .rounded(px(PREVIEW_RADIUS))
            .shadow_sm()
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
                        #[cfg(target_os = "macos")]
                        if !*hovered
                            && !Bounds::new(gpui::point(px(0.), px(0.)), window.viewport_size())
                                .contains(&window.mouse_position())
                        {
                            view.pointer_inside = false;
                            view.clear_native_tooltip(window);
                        }
                        cx.notify();
                    });
                    schedule_auto_dismiss(handle, id, cx, dismiss_token.clone());
                }
            })
            .child(root)
            .into_any_element()
    }
}

impl CapturePreviewWindow {
    #[cfg(target_os = "macos")]
    fn preview_is_clipped(&self, preview: &CapturePreview, window: &Window) -> bool {
        if self.viewport_scroll.is_none() {
            return false;
        }
        let Some(bounds) = self.preview_scroll_bounds(preview) else {
            return true;
        };
        let viewport = window.viewport_size();
        bounds.top() >= viewport.height
            || bounds.bottom() <= px(0.)
            || bounds.left() >= viewport.width
            || bounds.right() <= px(0.)
    }

    #[cfg(target_os = "macos")]
    fn update_viewport_visibility(&self, window: &Window) -> Vec<(u64, Arc<AtomicU64>)> {
        let visible = self.revealed && window.is_visible();
        self.previews
            .iter()
            .filter_map(|preview| {
                let clipped = self.preview_is_clipped(preview, window);
                if let Some(video) = &preview.video {
                    if video.viewport_clipped.replace(clipped) != clipped {
                        let _ = video.visibility_changed.try_send(());
                    }
                }
                if preview.dismiss_viewport_clipped.replace(clipped) == clipped {
                    return None;
                }
                if clipped {
                    preview.dismiss_token.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
                (visible && !preview.thumbnail_loading && preview.dismiss_animation.is_none())
                    .then(|| (preview.id, preview.dismiss_token.clone()))
            })
            .collect()
    }

    #[cfg(target_os = "macos")]
    fn preview_scroll_bounds(&self, preview: &CapturePreview) -> Option<Bounds<Pixels>> {
        let (corner, scroll) = self.viewport_scroll.as_ref()?;
        let y = preview.layout_y?;
        let bottom_aligned = matches!(
            corner,
            PreviewCorner::BottomLeft | PreviewCorner::BottomRight
        );
        let height = px(preview_stack_height(self.window_preview_count()));
        let offset = scroll.offset();
        let top = px(y) - preview_stack_top_inset(height, bottom_aligned) + offset.y;
        let left = px(PREVIEW_SHADOW_PADDING) + offset.x;
        Some(Bounds::new(
            gpui::point(left, top),
            size(px(PREVIEW_WIDTH), px(PREVIEW_HEIGHT)),
        ))
    }

    fn keyboard_controls_visible(&self, _window: &Window) -> bool {
        #[cfg(target_os = "macos")]
        return self.keyboard_controls && _window.is_window_active();
        #[cfg(not(target_os = "macos"))]
        false
    }

    fn preview_window_hovered(&self, window: &Window) -> bool {
        #[cfg(target_os = "macos")]
        {
            let _ = window;
            self.pointer_inside
        }
        #[cfg(not(target_os = "macos"))]
        {
            window.is_window_hovered()
        }
    }

    pub(crate) fn surface(
        &mut self,
        window_hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let displays = cx
            .displays()
            .iter()
            .map(|display| {
                let bounds = crate::system::work_area::display_bounds(display.as_ref());
                let primary =
                    f32::from(bounds.origin.x) == 0.0 && f32::from(bounds.origin.y) == 0.0;
                (display.id(), primary)
            })
            .collect::<Vec<_>>();
        self.surface_with_displays(window_hovered, &displays, window, cx)
    }

    pub(crate) fn surface_with_displays(
        &mut self,
        window_hovered: bool,
        displays: &[(gpui::DisplayId, bool)],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let corner = PreviewCorner::parse(&crate::state::state(cx).config.get().preview.corner);
        let theme = active_theme(cx);
        let polish = starred_preset(cx);
        let bottom_aligned = matches!(
            corner,
            PreviewCorner::BottomLeft | PreviewCorner::BottomRight
        );
        let enter_offset = if bottom_aligned {
            -PREVIEW_ENTER_OFFSET
        } else {
            PREVIEW_ENTER_OFFSET
        };
        let now = cx.background_executor().now();
        let mut needs_frame = false;
        let mut root = crate::ui::font::root().relative().size_full();
        #[cfg(target_os = "macos")]
        let content = {
            let content = preview_stack_size(self.window_preview_count());
            let viewport = window.viewport_size();
            let overflow_x = (content.width - viewport.width).max(px(0.));
            let overflow_y = (content.height - viewport.height).max(px(0.));
            if overflow_x == px(0.) && overflow_y == px(0.) {
                self.viewport_scroll = None;
            } else {
                let (previous_corner, scroll) = self
                    .viewport_scroll
                    .get_or_insert_with(|| (corner, gpui::ScrollHandle::new()));
                let corner_changed = std::mem::replace(previous_corner, corner) != corner;
                let right_aligned =
                    matches!(corner, PreviewCorner::TopRight | PreviewCorner::BottomRight);
                let current = scroll.offset();
                let limit = scroll.max_offset();
                let anchored_offset = |offset, limit: Pixels, overflow: Pixels, end_aligned| {
                    if end_aligned && (corner_changed || offset == -limit) {
                        -overflow
                    } else if corner_changed {
                        px(0.)
                    } else {
                        offset
                    }
                };
                scroll.set_offset(gpui::point(
                    anchored_offset(current.x, limit.x, overflow_x, right_aligned),
                    anchored_offset(current.y, limit.y, overflow_y, bottom_aligned),
                ));
            }
            let full_height = preview_stack_height(PREVIEW_MAX_STACK);
            let content_height = if self.viewport_scroll.is_some() {
                content.height
            } else {
                viewport.height
            };
            let offset = preview_stack_top_inset(content_height, bottom_aligned);
            root = root.h(px(full_height)).top(-offset);
            content
        };
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
                window,
                cx,
                &theme,
                polish.as_ref(),
                displays,
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
        #[cfg(feature = "parity")]
        let root = root.on_children_prepainted(|bounds, _, _| {
            crate::ui::parity::record("capture-previews", bounds);
        });
        #[cfg(target_os = "macos")]
        let root = root
            .on_mouse_move(cx.listener(|this, _, _, cx| {
                if !this.pointer_inside {
                    this.pointer_inside = true;
                    cx.notify();
                }
            }))
            .on_mouse_exit(cx.listener(|this, _, window, cx| {
                this.pointer_inside = false;
                this.clear_native_tooltip(window);
                cx.notify();
            }));
        #[cfg(target_os = "macos")]
        let root = if let Some((_, scroll)) = &self.viewport_scroll {
            div()
                .id("capture-preview-scroll")
                .size_full()
                .overflow_scroll()
                .track_scroll(scroll)
                .child(
                    div()
                        .relative()
                        .w(content.width)
                        .h(content.height)
                        .child(root),
                )
                .into_any_element()
        } else {
            root.into_any_element()
        };
        #[cfg(target_os = "macos")]
        if let Some(backward) = self.pending_keyboard_tab.take() {
            let owner = cx.entity().downgrade();
            window.defer(cx, move |window, cx| {
                if !owner
                    .upgrade()
                    .is_some_and(|view| view.read(cx).keyboard_controls_visible(window))
                {
                    return;
                }
                if backward {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
            });
        }
        let root = crate::ui::window_root::focus_root(root, window, cx).capture_key_down(
            cx.listener(|this, _event: &gpui::KeyDownEvent, window, _cx| {
                this.clear_native_tooltip(window);
                #[cfg(target_os = "macos")]
                if _event.keystroke.key == "tab"
                    && !this.keyboard_controls
                    && window.is_window_active()
                    && !this.display_menu.is_open(_cx)
                {
                    this.keyboard_controls = true;
                    this.pending_keyboard_tab = Some(_event.keystroke.modifiers.shift);
                    herogpui::extend::set_focus_visible(true, _cx);
                    _cx.stop_propagation();
                    _cx.notify();
                }
            }),
        );
        #[cfg(target_os = "macos")]
        let root = root.capture_any_mouse_down(cx.listener(|this, _, _, cx| {
            if std::mem::take(&mut this.keyboard_controls) {
                this.pending_keyboard_tab = None;
                cx.notify();
            }
        }));
        #[cfg(target_os = "macos")]
        let root = if self.viewport_scroll.is_some()
            || self.previews.iter().any(|preview| {
                preview.dismiss_viewport_clipped.get()
                    || preview
                        .video
                        .as_ref()
                        .is_some_and(|video| video.viewport_clipped.get())
            }) {
            let owner = cx.entity().downgrade();
            root.on_children_prepainted(move |_, window, cx| {
                if let Some(owner) = owner.upgrade() {
                    let timers = owner.read(cx).update_viewport_visibility(window);
                    if !timers.is_empty() {
                        let handle = window.window_handle();
                        cx.defer(move |cx| {
                            for (id, token) in timers {
                                schedule_auto_dismiss(handle, id, cx, token);
                            }
                        });
                    }
                }
            })
        } else {
            root
        };
        root
    }
}

impl Render for CapturePreviewWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.surface(self.preview_window_hovered(window), window, cx)
    }
}

#[cfg(target_os = "macos")]
impl preview::PreviewFocusRevealHost for CapturePreviewWindow {
    fn keyboard_reveal_target(
        &self,
        window: &Window,
        button: Bounds<Pixels>,
    ) -> Option<(gpui::ScrollHandle, Bounds<Pixels>)> {
        if !self.keyboard_controls_visible(window) {
            return None;
        }
        let (_, scroll) = self.viewport_scroll.as_ref()?;
        let viewport = scroll.bounds();
        let card = self
            .previews
            .iter()
            .filter_map(|preview| self.preview_scroll_bounds(preview))
            .map(|card| Bounds::new(card.origin + viewport.origin, card.size))
            .find(|card| {
                card.size.width <= viewport.size.width
                    && card.size.height <= viewport.size.height
                    && card.contains(&button.center())
            });
        Some((scroll.clone(), card.unwrap_or(button)))
    }
}

impl CapturePreviewWindow {
    fn clear_native_tooltip(&self, window: &mut Window) {
        self.tooltip_presenter.dismiss_active(Some(window));
    }

    #[cfg(feature = "parity")]
    pub(crate) fn native_tooltip_active(&self) -> bool {
        self.tooltip_presenter.active_content().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[herogpui::test]
    fn failed_preview_deletion_reports_error_and_keeps_the_preview(cx: &mut gpui::TestAppContext) {
        use herogpui::test::TestWindowExt;

        const CHILD: &str = "PORATAKE_TEST_CAPTURE_PREVIEW_DELETE_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let directory = tempfile::tempdir().unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "windows::capture_preview::tests::failed_preview_deletion_reports_error_and_keeps_the_preview",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .env("RUST_BACKTRACE", "1")
                .env(crate::config::store::CONFIG_DIR_ENV, directory.path())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let config_path = crate::config::store::config_dir().join("config.json");
        let config = Arc::new(crate::config::store::ConfigStore::load_at(config_path).unwrap());
        config.update(|settings| {
            settings.preview.auto_dismiss = false;
            settings.preview.corner = "bottom-right".into();
        });
        cx.update(|cx| {
            herogpui::init(cx);
            crate::state::set_test_state(cx, config);
        });
        let now = Instant::now();
        let preview = |id, kind, path| CapturePreview {
            id,
            kind,
            path,
            image: None,
            thumbnail_loading: false,
            thumbnail_cancel: None,
            thumbnail_generation: 0,
            #[cfg(all(test, feature = "parity"))]
            thumbnail_ready: None,
            blurred: None,
            video: None,
            export_cancel: None,
            hover_progress: 0.0,
            last_frame: None,
            hovered: false,
            busy: false,
            deleting: false,
            copying: false,
            finalizing: false,
            completed: None,
            export_progress: None,
            dismiss_token: Arc::new(AtomicU64::new(0)),
            #[cfg(target_os = "macos")]
            dismiss_viewport_clipped: Cell::new(false),
            entered_at: now,
            layout_y: None,
            layout_animation: None,
            dismiss_animation: None,
        };
        let notifications = crate::system::notification::capture_for_test();
        let screenshot_path = crate::config::store::config_dir().join("capture.png");
        std::fs::create_dir(&screenshot_path).unwrap();
        let recording_path = crate::config::store::config_dir().join("Recording.poratake");
        std::fs::write(&recording_path, b"not a project directory").unwrap();
        let preview = cx.update(|cx| {
            cx.new(|_| CapturePreviewWindow {
                previews: vec![preview(1, PreviewKind::Screenshot, screenshot_path.clone())],
                revealed: true,
                layout_generation: 0,
                #[cfg(target_os = "macos")]
                pointer_inside: false,
                #[cfg(target_os = "macos")]
                keyboard_controls: false,
                #[cfg(target_os = "macos")]
                pending_keyboard_tab: None,
                #[cfg(target_os = "macos")]
                viewport_scroll: None,
                display_menu: crate::ui::menu::MenuHandle::new(),
                tooltip_presenter: crate::ui::tooltip::NativeTooltipPresenter::new(
                    "capture-preview",
                ),
                _display_observer: None,
                #[cfg(target_os = "macos")]
                _visibility_observer: None,
            })
        });
        let (_wrapper, visual) = herogpui::test::open_window(cx, |_, _| HoveredPreview {
            preview: preview.clone(),
        });
        visual.simulate_resize(size(px(208.), px(604.)));
        visual.settle();

        for (kind, file_path) in [
            (PreviewKind::Screenshot, screenshot_path.clone()),
            (PreviewKind::Video, recording_path.clone()),
        ] {
            preview.update(visual, |view, _| {
                view.previews[0].kind = kind;
                view.previews[0].path = file_path.clone();
                view.previews[0].video = None;
                view.previews[0].image = None;
            });
            visual.settle();
            visual.simulate_mouse_move(
                gpui::point(px(100.), px(500.)),
                None,
                gpui::Modifiers::none(),
            );
            assert!(preview.update(visual, |view, _| view.previews[0].hovered));
            visual.executor().advance_clock(Duration::from_millis(250));
            visual.settle();
            let delete_bounds = visual
                .debug_bounds("preview-delete-1-tooltip-trigger")
                .expect("Delete control must render while the preview is hovered");
            assert!(
                delete_bounds.origin.x >= px(160.) && delete_bounds.origin.y >= px(450.),
                "Delete control unexpectedly laid out at {:?}",
                delete_bounds
            );
            visual.simulate_click(delete_bounds.center(), gpui::Modifiers::none());
            visual.run_until_parked();
            visual.settle();

            assert!(
                file_path.exists(),
                "failed deletion must keep the source path"
            );
            assert_eq!(
                preview.update(visual, |view, _| {
                    (
                        view.previews.len(),
                        view.previews[0].dismiss_animation.is_none(),
                    )
                }),
                (1, true),
                "failed deletion must keep its preview mounted and actionable"
            );
            assert_eq!(
                *notifications.borrow(),
                ["Delete failed"],
                "failed deletion must show user-visible feedback"
            );
            notifications.borrow_mut().clear();
        }

        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::FILE_SHARE_READ;

        let source_directory = tempfile::tempdir().unwrap();
        let persisted_capture = source_directory.path().join("persisted-capture.png");
        image::RgbaImage::new(32, 24)
            .save(&persisted_capture)
            .unwrap();
        let history_path = crate::history_store::history_file_path();
        let history_items = [crate::history_store::HistoryItem {
            id: "persist-failure-preview".into(),
            timestamp: 1,
            original_path: persisted_capture.to_string_lossy().into_owned(),
            r#type: crate::history_store::HistoryItemType::Screenshot,
            editor_state: None,
            duration: None,
        }];
        let original_history = serde_json::to_vec_pretty(&history_items).unwrap();
        std::fs::write(&history_path, &original_history).unwrap();
        let _history_lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0)
            .open(&history_path)
            .unwrap();
        preview.update(visual, |view, _| {
            let preview = &mut view.previews[0];
            preview.kind = PreviewKind::Screenshot;
            preview.path = persisted_capture.clone();
            preview.image = None;
        });
        visual.settle();
        visual.simulate_mouse_move(
            gpui::point(px(100.), px(500.)),
            None,
            gpui::Modifiers::none(),
        );
        visual.executor().advance_clock(Duration::from_millis(250));
        visual.settle();
        let delete_bounds = visual
            .debug_bounds("preview-delete-1-tooltip-trigger")
            .expect("Delete control must render while the preview is hovered");
        visual.simulate_click(delete_bounds.center(), gpui::Modifiers::none());
        visual.run_until_parked();
        visual.settle();
        assert!(!persisted_capture.exists());
        assert!(preview.update(visual, |view, _| {
            view.previews[0].dismiss_animation.is_some()
        }));
        assert_eq!(*notifications.borrow(), ["History update failed"]);
        assert_eq!(std::fs::read(&history_path).unwrap(), original_history);
        notifications.borrow_mut().clear();

        let successful_capture = crate::config::store::config_dir().join("successful.png");
        image::RgbaImage::new(32, 24)
            .save(&successful_capture)
            .unwrap();
        preview.update(visual, |view, _| {
            let preview = &mut view.previews[0];
            preview.kind = PreviewKind::Screenshot;
            preview.path = successful_capture.clone();
            preview.image = None;
            preview.dismiss_animation = None;
            preview.deleting = false;
        });
        visual.settle();
        let delete_point = gpui::point(px(100.), px(500.));
        visual.simulate_mouse_move(delete_point, None, gpui::Modifiers::none());
        visual.executor().advance_clock(Duration::from_millis(250));
        visual.settle();
        let delete_bounds = visual
            .debug_bounds("preview-delete-1-tooltip-trigger")
            .expect("Delete control must render while the preview is hovered");
        visual.simulate_click(delete_bounds.center(), gpui::Modifiers::none());
        visual.run_until_parked();
        visual.settle();
        assert!(!successful_capture.exists());
        assert!(preview.update(visual, |view, _| {
            view.previews[0].dismiss_animation.is_some()
        }));
        assert!(notifications.borrow().is_empty());
        visual.update(|window, _| window.remove_window());
    }

    #[cfg(windows)]
    fn preview_open_failure_context(
    ) -> (gpui::HeadlessAppContext, Arc<std::sync::atomic::AtomicU32>) {
        let attempts = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let factory_attempts = attempts.clone();
        let cx = gpui::HeadlessAppContext::with_platform(
            Arc::new(gpui::NoopTextSystem),
            Arc::new(()),
            move || {
                if factory_attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    anyhow::bail!("injected preview window creation failure")
                }
                Ok(None)
            },
        );
        (cx, attempts)
    }

    #[cfg(windows)]
    #[test]
    fn screenshot_preview_window_failure_notifies_without_opening_an_editor() {
        let directory = tempfile::tempdir().unwrap();
        let capture_path = directory.path().join("capture.png");
        image::RgbaImage::new(32, 24).save(&capture_path).unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        let notifications = crate::system::notification::capture_for_test();
        let (mut cx, attempts) = preview_open_failure_context();
        cx.update(|cx| crate::state::set_test_state(cx, config));

        cx.update(|cx| CapturePreviewWindow::open(cx, capture_path.clone()));

        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(&*notifications.borrow(), &["Preview unavailable"]);
        assert!(cx
            .update(|cx| registry::handle(WindowKind::CapturePreview, cx))
            .is_none());
        assert_eq!(cx.update(|cx| cx.windows().len()), 0);
        assert!(capture_path.exists());
    }

    #[cfg(windows)]
    #[test]
    fn a_prewarmed_preview_window_is_discarded_until_a_preview_arrives() {
        let directory = tempfile::tempdir().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        let mut cx = gpui::HeadlessAppContext::with_platform(
            Arc::new(gpui::NoopTextSystem),
            Arc::new(()),
            || Ok(None),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));

        cx.update(CapturePreviewWindow::prewarm);
        assert_eq!(cx.update(|cx| cx.windows().len()), 1);

        let handle = cx
            .update(|cx| registry::handle(WindowKind::CapturePreview, cx))
            .expect("prewarmed window");
        cx.update(|cx| {
            handle
                .downcast::<CapturePreviewWindow>()
                .expect("preview window")
                .update(cx, |view, _, _| view.revealed = true)
                .unwrap();
        });
        cx.update(CapturePreviewWindow::discard_idle);
        assert!(cx
            .update(|cx| registry::handle(WindowKind::CapturePreview, cx))
            .is_some());

        let handle = cx
            .update(|cx| registry::handle(WindowKind::CapturePreview, cx))
            .unwrap();
        cx.update(|cx| {
            handle
                .downcast::<CapturePreviewWindow>()
                .unwrap()
                .update(cx, |view, _, _| view.revealed = false)
                .unwrap();
        });
        cx.update(CapturePreviewWindow::discard_idle);
        assert!(cx
            .update(|cx| registry::handle(WindowKind::CapturePreview, cx))
            .is_none());
        assert_eq!(cx.update(|cx| cx.windows().len()), 0);
    }

    #[cfg(windows)]
    #[test]
    fn video_preview_window_failure_notifies_without_opening_an_editor() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Recording.poratake");
        std::fs::create_dir(&project).unwrap();
        let recording_path = crate::video::project::recording_video_path(&project);
        std::fs::write(&recording_path, b"test recording").unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        let notifications = crate::system::notification::capture_for_test();
        let (mut cx, attempts) = preview_open_failure_context();
        cx.update(|cx| crate::state::set_test_state(cx, config));

        cx.update(|cx| CapturePreviewWindow::open_video(cx, project.clone(), None));

        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(&*notifications.borrow(), &["Preview unavailable"]);
        assert!(cx
            .update(|cx| registry::handle(WindowKind::CapturePreview, cx))
            .is_none());
        assert_eq!(cx.update(|cx| cx.windows().len()), 0);
        assert!(recording_path.exists());
    }

    #[cfg(feature = "parity")]
    #[test]
    fn parity_headless_reveal_retains_preview_for_its_full_dismiss_delay() {
        let directory = tempfile::tempdir().unwrap();
        let capture_path = directory.path().join("capture.png");
        image::RgbaImage::new(32, 24).save(&capture_path).unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        config.update(|settings| {
            settings.preview.auto_dismiss = true;
            settings.preview.auto_dismiss_seconds = 1.0;
        });
        let mut cx = gpui::HeadlessAppContext::with_platform(
            Arc::new(gpui::NoopTextSystem),
            Arc::new(()),
            || Ok(None),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));

        let _reveal_guard = HeadlessPreviewRevealGuard::enter();
        cx.update(|cx| CapturePreviewWindow::open(cx, capture_path.clone()));
        cx.run_until_parked();
        let handle = cx
            .update(|cx| registry::handle(WindowKind::CapturePreview, cx))
            .expect("headless preview window stays registered");
        let view = handle
            .downcast::<CapturePreviewWindow>()
            .expect("capture preview window")
            .root(&mut cx)
            .expect("capture preview root");
        let thumbnail_ready = handle
            .downcast::<CapturePreviewWindow>()
            .expect("capture preview window")
            .update(&mut cx, |view, _, _| {
                let preview = &mut view.previews[0];
                (
                    !preview.thumbnail_loading && preview.image.is_some(),
                    preview.thumbnail_ready.take(),
                )
            })
            .unwrap();
        assert!(thumbnail_ready.0);
        assert!(thumbnail_ready.1.unwrap().try_recv().is_ok());
        let state = view.update(&mut cx, |view, _| {
            view.revealed = false;
            for preview in &view.previews {
                preview.dismiss_token.fetch_add(1, Ordering::Relaxed);
            }
            (
                view.active_preview_count(),
                view.previews[0].layout_animation.is_some(),
            )
        });
        assert_eq!(state.0, 1);
        assert!(state.1);
        assert!(simulate_headless_preview_reveal());

        cx.update(|cx| reveal_preview_stack_view(handle, view, cx));

        assert_eq!(
            cx.update(|cx| registry::handle(WindowKind::CapturePreview, cx)),
            Some(handle)
        );
        cx.run_until_parked();
        let state = handle
            .downcast::<CapturePreviewWindow>()
            .expect("capture preview window")
            .update(&mut cx, |view, _, _| {
                (
                    view.revealed,
                    view.active_preview_count(),
                    view.previews[0].layout_animation.is_some(),
                )
            })
            .unwrap();
        assert_eq!(state, (true, 1, true));
        cx.advance_clock(Duration::from_millis(999));
        cx.run_until_parked();
        assert!(cx.update(|cx| registry::is_open(WindowKind::CapturePreview, cx)));
        cx.advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        cx.advance_clock(Duration::from_millis(PREVIEW_EXIT_MS));
        cx.run_until_parked();
        assert!(!cx.update(|cx| registry::is_open(WindowKind::CapturePreview, cx)));
        assert!(capture_path.exists());
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn preview_tooltip_timer_requests_show_only_while_hovered(cx: &mut gpui::TestAppContext) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(20, 20, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|settings| {
            settings.preview.corner = "bottom-right".into();
            settings.preview.auto_dismiss = false;
        });
        cx.update(|cx| {
            herogpui::init(cx);
            crate::state::set_test_state(cx, config);
        });
        cx.simulate_display_added(gpui::DisplayId::new(10), None);
        cx.simulate_display_added(gpui::DisplayId::new(11), None);
        cx.update(|cx| CapturePreviewWindow::open(cx, path));
        let stack = stack_handle(cx)
            .unwrap()
            .downcast::<CapturePreviewWindow>()
            .unwrap();
        let preview = stack.update(cx, |_, _, cx| cx.entity()).unwrap();
        let preview_id = preview.update(cx, |view, _| view.previews[0].id);
        let (_wrapper, visual) = cx.add_window_view(|_, _| HoveredPreview { preview });
        visual.simulate_resize(size(px(208.), px(604.)));
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        visual.simulate_mouse_move(
            gpui::point(px(100.), px(500.)),
            None,
            gpui::Modifiers::none(),
        );
        visual.settle();
        visual.executor().advance_clock(Duration::from_millis(250));
        visual.settle();
        let trigger = visual
            .expect(&format!(
                "preview-display-menu-{preview_id}-tooltip-trigger"
            ))
            .center();

        crate::system::native_tooltip::take_test_requests();
        visual.simulate_mouse_move(trigger, None, gpui::Modifiers::none());
        visual.settle();
        visual.executor().advance_clock(Duration::from_millis(149));
        visual.settle();
        assert!(crate::system::native_tooltip::take_test_requests().is_empty());

        visual.executor().advance_clock(Duration::from_millis(1));
        visual.settle();
        assert_eq!(
            crate::system::native_tooltip::take_test_requests(),
            [crate::system::native_tooltip::TooltipRequest::Show(
                "capture-preview",
                "Move previews to another display".into(),
            )]
        );

        visual.press("escape");
        assert_eq!(
            crate::system::native_tooltip::take_test_requests(),
            [crate::system::native_tooltip::TooltipRequest::Hide(
                "capture-preview"
            )]
        );

        visual.simulate_mouse_move(gpui::point(px(0.), px(0.)), None, gpui::Modifiers::none());
        visual.settle();
        visual.simulate_mouse_move(trigger, None, gpui::Modifiers::none());
        visual.settle();
        visual.executor().advance_clock(Duration::from_millis(150));
        visual.settle();
        crate::system::native_tooltip::take_test_requests();
        visual.simulate_mouse_move(gpui::point(px(0.), px(0.)), None, gpui::Modifiers::none());
        visual.settle();
        assert_eq!(
            crate::system::native_tooltip::take_test_requests(),
            [crate::system::native_tooltip::TooltipRequest::Hide(
                "capture-preview"
            )]
        );

        visual.simulate_mouse_move(trigger, None, gpui::Modifiers::none());
        visual.settle();
        crate::system::native_tooltip::take_test_requests();
        visual.simulate_mouse_move(gpui::point(px(0.), px(0.)), None, gpui::Modifiers::none());
        visual.settle();
        crate::system::native_tooltip::take_test_requests();
        visual.executor().advance_clock(Duration::from_millis(150));
        visual.settle();
        assert!(crate::system::native_tooltip::take_test_requests().is_empty());
    }

    struct HoveredPreview {
        preview: gpui::Entity<CapturePreviewWindow>,
    }

    impl Render for HoveredPreview {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.preview
                .update(cx, |preview, cx| preview.surface(true, window, cx))
        }
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn keyboard_reveals_preview_controls_without_pointer_hover(cx: &mut gpui::TestAppContext) {
        use herogpui::test::TestWindowExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(20, 20, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        config.update(|settings| settings.preview.auto_dismiss = false);
        cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            CapturePreviewWindow::open(cx, path);
        });
        let stack = stack_handle(cx).unwrap();
        let control = stack.downcast::<CapturePreviewWindow>().unwrap();
        let mut visual = gpui::VisualTestContext::from_window(stack, cx);
        visual.update(|window, _| window.activate_window());
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        let (id, generation) = control
            .update(&mut visual, |view, _, _| {
                let preview = &view.previews[0];
                (preview.id, preview.dismiss_token.load(Ordering::Relaxed))
            })
            .unwrap();
        let selector: &'static str =
            Box::leak(format!("preview-delete-{id}-tooltip-trigger").into_boxed_str());
        let initial_focus = visual.update(|window, cx| {
            assert!(window.is_window_active());
            window.focused(cx)
        });
        assert!(visual.debug_bounds(selector).is_none());

        visual.press("tab");
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        assert!(visual.debug_bounds(selector).is_some());
        let first_focus = visual.update(|window, cx| window.focused(cx));
        assert!(first_focus.is_some() && first_focus != initial_focus);
        visual.press("tab");
        assert_ne!(visual.update(|window, cx| window.focused(cx)), first_focus);
        visual.cx.update(|cx| {
            assert_eq!(
                dismiss_verdict(stack, id, generation, cx),
                DismissVerdict::Blocked
            );
        });

        let button = visual.debug_bounds(selector).unwrap().center();
        visual.simulate_mouse_move(button, None, gpui::Modifiers::none());
        visual.executor().advance_clock(Duration::from_millis(200));
        visual.settle();
        assert_eq!(crate::windows::scroll_capture::test_native_tooltip(), None);
        visual.click_at(gpui::point(px(100.), px(15.)));
        visual.settle();
        assert!(!control
            .update(&mut visual, |view, _, _| view.keyboard_controls)
            .unwrap());
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn unfocused_preview_hover_and_exit_control_visibility(cx: &mut gpui::TestAppContext) {
        use herogpui::test::TestWindowExt;

        struct NativeHoverPreview {
            preview: gpui::Entity<CapturePreviewWindow>,
        }

        impl Render for NativeHoverPreview {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.preview.update(cx, |preview, cx| {
                    preview.surface(preview.preview_window_hovered(window), window, cx)
                })
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(20, 20, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|settings| {
            settings.preview.corner = "bottom-right".into();
            settings.preview.auto_dismiss = false;
        });
        cx.update(|cx| {
            herogpui::init(cx);
            crate::theme::vars::init_theme(
                cx,
                crate::theme::presets::ThemeMode::Light,
                crate::theme::presets::DEFAULT_THEME_ID,
            );
            crate::state::set_test_state(cx, config);
            CapturePreviewWindow::open(cx, path);
        });
        let stack = stack_handle(cx)
            .unwrap()
            .downcast::<CapturePreviewWindow>()
            .unwrap();
        let preview = stack.update(cx, |_, _, cx| cx.entity()).unwrap();
        let check = preview.clone();
        let (_, visual) = herogpui::test::open_window(cx, |_, _| NativeHoverPreview { preview });
        visual.update(|_, cx| {
            crate::theme::vars::init_theme(
                cx,
                crate::theme::presets::ThemeMode::Light,
                crate::theme::presets::DEFAULT_THEME_ID,
            );
        });
        visual.simulate_resize(size(px(208.), px(148.)));
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        let id = visual.update(|_, cx| check.read(cx).previews[0].id);
        let delete_selector: &'static str =
            Box::leak(format!("preview-delete-{id}-tooltip-trigger").into_boxed_str());

        for inside in [true, false, true] {
            if inside {
                visual.simulate_mouse_move(
                    gpui::point(px(100.), px(60.)),
                    None,
                    gpui::Modifiers::none(),
                );
            } else {
                visual.simulate_event(gpui::MouseExitEvent {
                    position: gpui::point(px(210.), px(60.)),
                    pressed_button: None,
                    modifiers: gpui::Modifiers::none(),
                });
            }
            visual.executor().advance_clock(Duration::from_millis(500));
            visual.settle();
            assert_eq!(visual.update(|_, cx| check.read(cx).pointer_inside), inside);
            assert_eq!(
                visual.debug_bounds(delete_selector).is_some(),
                inside,
                "preview controls must follow window entry and exit"
            );
            assert_eq!(
                dismiss_state(1, 1, true, inside, false),
                if inside {
                    DismissVerdict::Blocked
                } else {
                    DismissVerdict::Ready
                }
            );
        }
        visual.simulate_mouse_move(
            gpui::point(px(210.), px(60.)),
            None,
            gpui::Modifiers::none(),
        );
        visual.settle();
        assert!(!visual.update(|_, cx| check.read(cx).pointer_inside));
        visual.simulate_mouse_move(gpui::point(px(25.), px(25.)), None, gpui::Modifiers::none());
        visual.settle();
        visual.executor().advance_clock(Duration::from_millis(200));
        visual.settle();
        assert_eq!(
            crate::windows::scroll_capture::test_native_tooltip().as_deref(),
            Some("Close preview")
        );
        visual.simulate_mouse_move(
            gpui::point(px(183.), px(25.)),
            None,
            gpui::Modifiers::none(),
        );
        visual.settle();
        visual.executor().advance_clock(Duration::from_millis(200));
        visual.settle();
        assert_eq!(
            crate::windows::scroll_capture::test_native_tooltip().as_deref(),
            Some("Delete screenshot")
        );
        visual.simulate_event(gpui::MouseExitEvent {
            position: gpui::point(px(210.), px(25.)),
            pressed_button: None,
            modifiers: gpui::Modifiers::none(),
        });
        visual.executor().advance_clock(Duration::from_millis(200));
        visual.settle();
        assert_eq!(crate::windows::scroll_capture::test_native_tooltip(), None);
    }

    fn bind_stack(
        visual: &mut gpui::VisualTestContext,
        wrapper: &gpui::Entity<HoveredPreview>,
    ) -> (
        gpui::WindowHandle<CapturePreviewWindow>,
        gpui::Entity<CapturePreviewWindow>,
    ) {
        use herogpui::test::TestWindowExt;

        let stack = stack_handle(visual)
            .unwrap()
            .downcast::<CapturePreviewWindow>()
            .unwrap();
        let check = stack.update(visual, |_, _, cx| cx.entity()).unwrap();
        visual.update(|_, cx| {
            wrapper.update(cx, |view, cx| {
                view.preview = check.clone();
                cx.notify();
            });
        });
        visual.settle();
        (stack, check)
    }

    #[test]
    fn preview_stack_fits_scaled_displays_on_either_side() {
        for (x, width, height, scale, expected) in [
            (1280.0, 1280.0, 680.0, 1.5, (3498, 84, 312, 906)),
            (-1536.0, 1536.0, 832.0, 1.25, (-285, 260, 260, 755)),
        ] {
            let display = Bounds {
                origin: gpui::point(px(x), px(0.0)),
                size: size(px(width), px(height)),
            };
            let bounds =
                preview_fitted_stack_bounds(display, PreviewCorner::BottomRight, PREVIEW_MAX_STACK);
            let physical = (
                (f32::from(bounds.origin.x) * scale).round() as i32,
                (f32::from(bounds.origin.y) * scale).round() as i32,
                (f32::from(bounds.size.width) * scale).round() as i32,
                (f32::from(bounds.size.height) * scale).round() as i32,
            );
            assert_eq!(physical, expected);
        }
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn display_menu_fits_a_single_fitted_preview(cx: &mut gpui::TestAppContext) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(20, 20, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|settings| {
            settings.preview.corner = "bottom-right".into();
            settings.preview.auto_dismiss = false;
        });
        cx.update(|cx| {
            herogpui::init(cx);
            crate::state::set_test_state(cx, config);
        });
        for id in 10..13 {
            cx.simulate_display_added(gpui::DisplayId::new(id), None);
        }
        cx.update(|cx| CapturePreviewWindow::open(cx, path));
        let stack = stack_handle(cx)
            .unwrap()
            .downcast::<CapturePreviewWindow>()
            .unwrap();
        let preview = stack.update(cx, |_, _, cx| cx.entity()).unwrap();
        let check = preview.clone();
        let (_, visual) = herogpui::test::open_window(cx, |_, _| HoveredPreview { preview });
        visual.simulate_resize(size(px(208.), px(148.)));
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        visual.simulate_mouse_move(
            gpui::point(px(184.), px(96.)),
            None,
            gpui::Modifiers::none(),
        );
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        visual.click_at(gpui::point(px(184.), px(96.)));
        assert!(visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        visual.update(|window, _| {
            let scale = window.scale_factor();
            let panel = window
                .painted_quads()
                .into_iter()
                .find(|quad| {
                    (quad.bounds.size.width.0 / scale - 184.).abs() < 0.5
                        && (quad.bounds.size.height.0 / scale - 120.).abs() < 0.5
                })
                .expect("the display menu must paint within the fitted preview");
            assert!(panel.bounds.top().0 >= 0.);
            assert!(panel.bounds.bottom().0 <= 148. * scale);
        });
    }

    #[herogpui::test]
    fn multi_display_menu_fits_and_selects_the_last_display(cx: &mut gpui::TestAppContext) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(20, 20, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|settings| {
            settings.preview.corner = "bottom-right".into();
            settings.preview.auto_dismiss = false;
        });
        cx.update(|cx| {
            herogpui::init(cx);
            crate::state::set_test_state(cx, config.clone());
        });
        for id in 10..13 {
            cx.simulate_display_added(gpui::DisplayId::new(id), None);
        }
        cx.update(|cx| CapturePreviewWindow::open(cx, path.clone()));
        let mut stack = stack_handle(cx)
            .unwrap()
            .downcast::<CapturePreviewWindow>()
            .unwrap();
        let preview = stack.update(cx, |_, _, cx| cx.entity()).unwrap();
        let mut check = preview.clone();
        let (wrapper, visual) = herogpui::test::open_window(cx, |_, _| HoveredPreview { preview });
        visual.simulate_resize(size(px(208.), px(604.)));
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        visual.simulate_mouse_move(
            gpui::point(px(184.), px(552.)),
            None,
            gpui::Modifiers::none(),
        );
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        visual.simulate_click(gpui::point(px(184.), px(552.)), gpui::Modifiers::none());
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        visual.update(|window, _| {
            let scale = window.scale_factor();
            let panel = window
                .painted_quads()
                .into_iter()
                .find(|quad| {
                    (quad.bounds.size.width.0 / scale - 184.).abs() < 0.5
                        && (quad.bounds.size.height.0 / scale - 120.).abs() < 0.5
                })
                .expect("four-display menu must paint a bounded, scrollable panel");
            let bounds = panel.bounds;
            assert!(bounds.left().0 >= 4. * scale);
            assert!(bounds.right().0 <= 204. * scale);
            assert!(bounds.top().0 >= 460. * scale);
            assert!(bounds.bottom().0 <= 600. * scale);
        });
        visual.update(|_, cx| {
            crate::system::clipboard::ClipboardService::write_text(cx, "unchanged".into());
        });
        visual.click_at(gpui::point(px(24.), px(590.)));
        assert!(!visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        assert!(visual.update(|_, cx| check.read(cx).previews[0].dismiss_animation.is_none()));
        assert_eq!(
            visual.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text())),
            Some("unchanged".to_string())
        );
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.click_at(gpui::point(px(184.), px(552.)));
        assert!(visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        visual.simulate_mouse_move(gpui::point(px(0.), px(0.)), None, gpui::Modifiers::none());
        visual.settle();
        let (id, generation, open, hovered) = visual.update(|_, cx| {
            let view = check.read(cx);
            let preview = &view.previews[0];
            (
                preview.id,
                preview.dismiss_token.load(Ordering::Relaxed),
                view.display_menu.is_open(cx),
                preview.hovered,
            )
        });
        assert!(open && !hovered, "the menu stays open after pointer leave");
        assert_eq!(
            visual.update(|_, cx| dismiss_verdict(stack.into(), id, generation, cx)),
            DismissVerdict::Blocked
        );
        visual.press("home");
        visual.press("end");
        visual.press("enter");
        assert_eq!(config.get().preview.display_id, Some(12));
        assert!(!visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        (stack, check) = bind_stack(visual, &wrapper);
        assert_eq!(
            visual.update(|_, cx| dismiss_verdict(stack.into(), id, generation, cx)),
            DismissVerdict::Ready
        );
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.click_at(gpui::point(px(184.), px(552.)));
        assert!(visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        #[cfg(windows)]
        stack
            .update(visual, |_, window, _| {
                window.resize(size(px(32.), px(32.)));
            })
            .unwrap();
        let previous_stack: AnyWindowHandle = stack.into();
        visual.simulate_display_removed(gpui::DisplayId::new(10));
        visual.settle();
        assert!(!visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        #[cfg(any(windows, target_os = "macos"))]
        assert_eq!(stack_handle(visual), Some(previous_stack));
        #[cfg(windows)]
        assert_eq!(
            stack
                .update(visual, |_, window, _| window
                    .window_bounds()
                    .get_bounds()
                    .size)
                .unwrap(),
            size(px(208.), px(604.))
        );
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            (stack, check) = bind_stack(visual, &wrapper);
            assert_ne!(AnyWindowHandle::from(stack), previous_stack);
        }
        assert_eq!(
            visual.update(|_, cx| selected_display(cx).unwrap().1),
            gpui::DisplayId::new(12)
        );
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.click_at(gpui::point(px(184.), px(552.)));
        assert!(visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        visual.press("end");
        let position = visual.update(|window, _| {
            let scale = window.scale_factor();
            let row = window
                .painted_quads()
                .into_iter()
                .find(|quad| {
                    (quad.bounds.size.width.0 / scale - 180.).abs() < 0.5
                        && (quad.bounds.size.height.0 / scale - 40.).abs() < 0.5
                })
                .expect("the focused last display row must paint its gap");
            gpui::point(
                px(row.bounds.center().x.0 / scale),
                px(row.bounds.center().y.0 / scale),
            )
        });
        visual.click_at(position);
        assert_eq!(config.get().preview.display_id, Some(12));
        check = bind_stack(visual, &wrapper).1;
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.click_at(gpui::point(px(184.), px(552.)));
        assert!(visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        visual.press("escape");
        assert!(!visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.click_at(gpui::point(px(184.), px(552.)));
        assert!(visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        visual.simulate_display_removed(gpui::DisplayId::new(11));
        visual.settle();
        assert!(!visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            check = bind_stack(visual, &wrapper).1;
        }
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.click_at(gpui::point(px(184.), px(552.)));
        visual.press("home");
        let row_positions = |visual: &mut gpui::VisualTestContext| {
            visual.update(|window, _| {
                let scale = window.scale_factor();
                let mut rows: Vec<_> = window
                    .painted_quads()
                    .into_iter()
                    .filter(|quad| {
                        (quad.bounds.size.width.0 / scale - 180.).abs() < 0.5
                            && (quad.bounds.size.height.0 / scale - 40.).abs() < 0.5
                    })
                    .map(|quad| quad.bounds.top().0 / scale)
                    .collect();
                rows.sort_by(f32::total_cmp);
                rows
            })
        };
        let before = row_positions(visual);
        assert_eq!(before.len(), 4);
        let pointer = gpui::point(px(100.), px(before[0] + 20.));
        visual.simulate_mouse_move(pointer, None, gpui::Modifiers::none());
        visual.simulate_event(gpui::ScrollEvent {
            position: pointer,
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-100.))),
            modifiers: gpui::Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        visual.settle();
        assert_eq!(
            row_positions(visual),
            before,
            "a short menu must not scroll"
        );
        visual.simulate_display_removed(gpui::DisplayId::new(12));
        visual.settle();
        assert!(!visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        visual.update(|_, cx| {
            assert_eq!(
                selected_display(cx).unwrap().1,
                cx.primary_display().unwrap().id()
            );
        });
        assert_eq!(config.get().preview.display_id, Some(12));

        for id in 11..13 {
            visual.simulate_display_added(gpui::DisplayId::new(id), None);
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            (stack, check) = bind_stack(visual, &wrapper);
        }
        for _ in 1..PREVIEW_MAX_STACK {
            visual.update(|_, cx| CapturePreviewWindow::open(cx, path.clone()));
        }
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        visual.click_at(gpui::point(px(184.), px(552.)));
        assert!(visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        let (other_id, other_generation) = visual.update(|_, cx| {
            let other = &check.read(cx).previews[1];
            (other.id, other.dismiss_token.load(Ordering::Relaxed))
        });
        assert_eq!(
            visual.update(|_, cx| dismiss_verdict(stack.into(), other_id, other_generation, cx)),
            DismissVerdict::Ready
        );
        visual.update(|_, cx| CapturePreviewWindow::open(cx, path));
        assert!(!visual.update(|_, cx| check.read(cx).display_menu.is_open(cx)));
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.simulate_mouse_move(
            gpui::point(px(24.), px(480.)),
            None,
            gpui::Modifiers::none(),
        );
        visual.click_at(gpui::point(px(24.), px(480.)));
        assert!(visual.update(|_, cx| check.read(cx).previews[0].dismiss_animation.is_some()));
    }

    fn polish_and_copy(path: &Path, scale_factor: f32, cx: &mut App) -> bool {
        let Some(preset) = starred_preset(cx) else {
            return false;
        };
        let Ok(bytes) = prepare_screenshot_copy(path, Some(&preset), scale_factor) else {
            return false;
        };
        crate::system::clipboard::ClipboardService::write_png(cx, bytes);
        true
    }

    #[herogpui::test]
    fn screenshot_copy_publishes_pixels_before_dismissing(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.png");
        let source = image::RgbaImage::from_fn(320, 240, |x, y| {
            image::Rgba([(x % 256) as u8, y as u8, 64, 255])
        });
        source.save(&path).unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            CapturePreviewWindow::open(cx, path);
        });
        cx.run_until_parked();
        let handle = stack_handle(cx)
            .and_then(|handle| handle.downcast::<CapturePreviewWindow>())
            .unwrap();
        handle
            .update(cx, |view, window, cx| {
                let id = view.previews[0].id;
                let entity = cx.weak_entity();
                window.defer(cx, move |window, cx| {
                    copy_screenshot(&entity, id, false, window, cx);
                });
            })
            .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let clipboard = crate::system::clipboard::ClipboardService::read_image(cx).unwrap();
            assert_eq!(
                image::load_from_memory(&clipboard.bytes)
                    .unwrap()
                    .into_rgba8(),
                source
            );
            handle
                .update(cx, |view, _, _| {
                    assert!(view.previews[0].dismiss_animation.is_some());
                    assert!(!view.previews[0].is_busy());
                })
                .unwrap();
        });
    }

    #[herogpui::test]
    fn polish_copy_matches_electron_at_multiple_scales(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(1600, 1000, image::Rgba([240, 80, 64, 255]))
            .save(&path)
            .unwrap();
        let odd_path = dir.path().join("odd-capture.png");
        image::RgbaImage::from_pixel(641, 361, image::Rgba([240, 80, 64, 255]))
            .save(&odd_path)
            .unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|settings| {
            settings.wallpaper.presets = vec![serde_json::from_value(serde_json::json!({
                "id": "scaled", "name": "Scaled", "padding": 50,
                "corners": 12, "shadow": 0,
            }))
            .unwrap()];
            settings.wallpaper.default_preset_id = Some("scaled".into());
        });
        cx.update(|cx| crate::state::set_test_state(cx, config.clone()));
        cx.update(|cx| CapturePreviewWindow::open(cx, path.clone()));
        let preview = stack_handle(cx)
            .unwrap()
            .downcast::<CapturePreviewWindow>()
            .unwrap();
        for (path, scale, dimensions, padding) in [
            (&path, 1., (1700, 1100), 50),
            (&path, 1.25, (1726, 1126), 63),
            (&odd_path, 1.25, (767, 487), 63),
            (&path, 2., (1800, 1200), 100),
        ] {
            let copied = preview
                .update(cx, |_, window, cx| {
                    window.set_scale_factor(scale);
                    assert_eq!(window.scale_factor(), scale);
                    assert!(polish_and_copy(path, window.scale_factor(), cx));
                    let copied =
                        crate::system::clipboard::ClipboardService::read_image(cx).unwrap();
                    image::load_from_memory(&copied.bytes).unwrap().to_rgba8()
                })
                .unwrap();
            assert_eq!(copied.dimensions(), dimensions);
            assert_eq!(copied.get_pixel(padding, padding).0[3], 0);
            assert_eq!(
                copied.get_pixel(padding + 30, padding + 30).0,
                [240, 80, 64, 255]
            );
        }
        config.update(|settings| {
            settings.wallpaper.presets[0].window_frame =
                Some(crate::config::schema::WindowFrameSettings {
                    style: "macos-light".into(),
                });
        });
        let framed = preview
            .update(cx, |_, window, cx| {
                window.set_scale_factor(2.);
                assert!(polish_and_copy(&path, window.scale_factor(), cx));
                let copied = crate::system::clipboard::ClipboardService::read_image(cx).unwrap();
                image::load_from_memory(&copied.bytes).unwrap().to_rgba8()
            })
            .unwrap();
        assert_eq!(framed.dimensions(), (1800, 1256));
        assert_eq!(framed.get_pixel(900, 600).0, [240, 80, 64, 255]);
        config.update(|settings| {
            settings.wallpaper.presets[0].padding = 0.;
            settings.wallpaper.presets[0].corners = 0.;
        });
        let framed_without_padding = preview
            .update(cx, |_, window, cx| {
                assert!(polish_and_copy(&path, window.scale_factor(), cx));
                let copied = crate::system::clipboard::ClipboardService::read_image(cx).unwrap();
                image::load_from_memory(&copied.bytes).unwrap().to_rgba8()
            })
            .unwrap();
        assert_eq!(framed_without_padding.dimensions(), (1600, 1056));
        assert_eq!(
            framed_without_padding.get_pixel(800, 600).0,
            [240, 80, 64, 255]
        );
    }

    #[herogpui::test]
    fn polish_copy_matches_editor_export_at_two_x(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(640, 360, image::Rgba([240, 80, 64, 255]))
            .save(&path)
            .unwrap();
        let preset: crate::config::schema::WallpaperPreset =
            serde_json::from_value(serde_json::json!({
                "id": "scaled", "name": "Scaled", "padding": 20,
                "corners": 12, "shadow": 0,
                "windowFrame": { "style": "windows-dark" },
            }))
            .unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|settings| {
            settings.wallpaper.presets = vec![preset.clone()];
            settings.wallpaper.default_preset_id = Some("scaled".into());
        });
        cx.update(|cx| crate::state::set_test_state(cx, config.clone()));
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            window.set_scale_factor(2.);
            let mut editor =
                crate::editor::window::EditorWindow::from_file(path.to_str().unwrap(), window, cx);
            editor.file_path.clear();
            crate::editor::wallpaper::apply_preset(&mut editor.wallpaper, &preset);
            editor
        });
        let (saved, copied) = cx.update(|window, cx| {
            let saved = editor.read(cx).export_png().unwrap();
            assert!(polish_and_copy(&path, window.scale_factor(), cx));
            let copied = crate::system::clipboard::ClipboardService::read_image(cx).unwrap();
            (saved, copied.bytes)
        });
        let saved = image::load_from_memory(&saved).unwrap().to_rgba8();
        let copied = image::load_from_memory(&copied).unwrap().to_rgba8();
        assert_eq!(saved.dimensions(), (720, 496));
        assert_eq!(copied, saved);

        let mut zero_padding = preset.clone();
        zero_padding.padding = 0.;
        zero_padding.corners = 0.;
        config.update(|settings| settings.wallpaper.presets[0] = zero_padding.clone());
        let (saved, copied) = cx.update(|window, cx| {
            editor.update(cx, |editor, _| {
                crate::editor::wallpaper::apply_preset(&mut editor.wallpaper, &zero_padding);
            });
            let saved = editor.read(cx).export_png().unwrap();
            assert!(polish_and_copy(&path, window.scale_factor(), cx));
            let copied = crate::system::clipboard::ClipboardService::read_image(cx).unwrap();
            (saved, copied.bytes)
        });
        let saved = image::load_from_memory(&saved).unwrap().to_rgba8();
        let copied = image::load_from_memory(&copied).unwrap().to_rgba8();
        assert_eq!(saved.dimensions(), (640, 416));
        assert_eq!(saved.get_pixel(320, 20).0, [32, 32, 32, 255]);
        assert_eq!(saved.get_pixel(320, 200).0, [240, 80, 64, 255]);
        assert_eq!(copied, saved);

        zero_padding.window_frame = Some(crate::config::schema::WindowFrameSettings {
            style: "none".into(),
        });
        zero_padding.corners = 12.;
        config.update(|settings| settings.wallpaper.presets[0] = zero_padding.clone());
        let (saved, copied) = cx.update(|window, cx| {
            editor.update(cx, |editor, _| {
                crate::editor::wallpaper::apply_preset(&mut editor.wallpaper, &zero_padding);
            });
            let saved = editor.read(cx).export_png().unwrap();
            assert!(polish_and_copy(&path, window.scale_factor(), cx));
            let copied = crate::system::clipboard::ClipboardService::read_image(cx).unwrap();
            (saved, copied.bytes)
        });
        let saved = image::load_from_memory(&saved).unwrap().to_rgba8();
        let copied = image::load_from_memory(&copied).unwrap().to_rgba8();
        assert_eq!(saved.dimensions(), (640, 360));
        assert_eq!(saved.get_pixel(0, 0).0, [0, 0, 0, 0]);
        assert_eq!(saved.get_pixel(320, 200).0, [240, 80, 64, 255]);
        assert_eq!(copied, saved);
    }

    #[herogpui::test]
    fn polish_copy_scales_blur_and_framed_shadow(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let background = dir.path().join("background.png");
        image::RgbaImage::from_fn(80, 80, |x, _| {
            if x < 40 {
                image::Rgba([0, 0, 0, 255])
            } else {
                image::Rgba([255, 255, 255, 255])
            }
        })
        .save(&background)
        .unwrap();
        let one = dir.path().join("one.png");
        let two = dir.path().join("two.png");
        for (path, width, height) in [(&one, 160, 100), (&two, 320, 200)] {
            image::RgbaImage::from_pixel(width, height, image::Rgba([240, 80, 64, 255]))
                .save(path)
                .unwrap();
        }
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|settings| {
            settings.wallpaper.presets = vec![serde_json::from_value(serde_json::json!({
                "id": "effects", "name": "Effects", "padding": 80,
                "backgroundImage": background.to_string_lossy(),
                "windowFrame": { "style": "windows-dark" },
            }))
            .unwrap()];
            settings.wallpaper.default_preset_id = Some("effects".into());
        });
        cx.update(|cx| crate::state::set_test_state(cx, config.clone()));
        let mut images = Vec::new();
        for (path, scale) in [(&one, 1.), (&two, 2.)] {
            for (blur, shadow) in [(0., 0.), (30., 0.), (30., 70.)] {
                config.update(|settings| {
                    settings.wallpaper.presets[0].background_blur = Some(blur);
                    settings.wallpaper.presets[0].shadow = shadow;
                });
                images.push(cx.update(|cx| {
                    assert!(polish_and_copy(path, scale, cx));
                    let copied =
                        crate::system::clipboard::ClipboardService::read_image(cx).unwrap();
                    image::load_from_memory(&copied.bytes).unwrap().to_rgba8()
                }));
            }
        }
        let blur_delta: u64 = (140..180)
            .flat_map(|x| (20..60).map(move |y| (x, y)))
            .map(|(x, y)| {
                u64::from(images[0].get_pixel(x, y)[0].abs_diff(images[1].get_pixel(x, y)[0]))
            })
            .sum();
        let shadow_delta: u64 = (180..220)
            .flat_map(|x| (209..235).map(move |y| (x, y)))
            .map(|(x, y)| {
                u64::from(images[1].get_pixel(x, y)[0].abs_diff(images[2].get_pixel(x, y)[0]))
            })
            .sum();
        assert!(blur_delta > 10_000, "background blur had no visible effect");
        assert!(shadow_delta > 5_000, "frame shadow had no visible effect");
        let half_blur =
            image::imageops::resize(&images[4], 320, 288, image::imageops::FilterType::Triangle);
        let half_shadow =
            image::imageops::resize(&images[5], 320, 288, image::imageops::FilterType::Triangle);
        let blur_error: u64 = (140..180)
            .flat_map(|x| (20..60).map(move |y| (x, y)))
            .map(|(x, y)| {
                u64::from(images[1].get_pixel(x, y)[0].abs_diff(half_blur.get_pixel(x, y)[0]))
            })
            .sum();
        let shadow_error: u64 = (180..220)
            .flat_map(|x| (209..235).map(move |y| (x, y)))
            .map(|(x, y)| {
                u64::from(images[2].get_pixel(x, y)[0].abs_diff(half_shadow.get_pixel(x, y)[0]))
            })
            .sum();
        assert!(blur_error < 4_800, "scaled blur differs: {blur_error}");
        assert!(
            shadow_error < 3_120,
            "scaled shadow differs: {shadow_error}"
        );
    }

    fn stack_handle(cx: &mut gpui::TestAppContext) -> Option<AnyWindowHandle> {
        cx.update(|cx| registry::handle(WindowKind::CapturePreview, cx))
    }

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

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_capture_preview_viewport_fits_small_work_areas() {
        for (width, height) in [(800., 610.), (800., 600.), (640., 480.), (180., 120.)] {
            let display = Bounds::new(
                gpui::point(px(-800.), px(-300.)),
                size(px(width), px(height)),
            );
            for corner in [
                PreviewCorner::TopLeft,
                PreviewCorner::TopRight,
                PreviewCorner::BottomLeft,
                PreviewCorner::BottomRight,
            ] {
                let viewport = preview_fitted_stack_bounds(display, corner, PREVIEW_MAX_STACK);
                assert!(
                    viewport.left() >= display.left()
                        && viewport.right() <= display.right()
                        && viewport.top() >= display.top()
                        && viewport.bottom() <= display.bottom(),
                    "{corner:?} preview {viewport:?} must fit {display:?}"
                );
                assert_eq!(
                    viewport.size,
                    size(
                        px(width.min(PREVIEW_WIDTH + PREVIEW_SHADOW_PADDING * 2.)),
                        px(height.min(preview_stack_height(PREVIEW_MAX_STACK))),
                    )
                );
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn a_short_preview_viewport_scrolls_to_the_clipped_card_controls(
        cx: &mut gpui::TestAppContext,
    ) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        store.update(|config| {
            config.preview.corner = "bottom-right".into();
            config.preview.auto_dismiss = false;
        });
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(20, 20, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        cx.update(|cx| {
            crate::state::set_test_state(cx, store);
            for _ in 0..PREVIEW_MAX_STACK {
                CapturePreviewWindow::open(cx, path.clone());
            }
        });
        cx.run_until_parked();
        let stack = stack_handle(cx).unwrap();
        let handle = stack.downcast::<CapturePreviewWindow>().unwrap();
        let newest = handle
            .update(cx, |view, _, _| view.previews.last().unwrap().id)
            .unwrap();
        let mut visual = gpui::VisualTestContext::from_window(stack, cx);
        visual.simulate_resize(size(px(208.), px(480.)));
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        let scroll_offset = |visual: &mut gpui::VisualTestContext| {
            handle
                .update(visual, |view, _, _| {
                    view.viewport_scroll.as_ref().unwrap().1.offset().y
                })
                .unwrap()
        };
        assert_eq!(scroll_offset(&mut visual), px(-124.));
        visual.simulate_resize(size(px(208.), px(360.)));
        visual.settle();
        assert_eq!(scroll_offset(&mut visual), px(-244.));

        visual.simulate_event(gpui::ScrollEvent {
            position: gpui::point(px(100.), px(240.)),
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(320.))),
            modifiers: gpui::Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        visual.settle();
        assert_eq!(scroll_offset(&mut visual), px(0.));
        visual.simulate_resize(size(px(208.), px(480.)));
        visual.settle();
        assert_eq!(scroll_offset(&mut visual), px(0.));

        let oldest = handle
            .update(&mut visual, |view, _, _| view.previews[0].id)
            .unwrap();
        let oldest_close: &'static str =
            Box::leak(format!("preview-close-{oldest}-tooltip-trigger").into_boxed_str());
        visual.simulate_resize(size(px(208.), px(360.)));
        visual.update(|window, _| window.activate_window());
        visual.settle();
        assert_eq!(scroll_offset(&mut visual), px(0.));
        visual.press("tab");
        visual.executor().advance_clock(Duration::from_millis(500));
        let bounds = visual.expect(oldest_close);
        assert!(
            bounds.top() >= px(0.) && bounds.bottom() < px(360.),
            "{bounds:?}"
        );
        assert!(scroll_offset(&mut visual) < px(0.));
        assert!(
            visual.update(|window, cx| {
                let overlay = gpui::Background::from(crate::theme::vars::active_theme(cx).overlay);
                let scale = window.scale_factor();
                window.painted_quads().into_iter().any(|quad| {
                    quad.background == overlay
                        && (31. ..=35.).contains(&(quad.bounds.size.height.0 / scale))
                        && quad.bounds.top().0 >= 0.
                        && quad.bounds.bottom().0
                            <= f32::from(window.viewport_size().height) * scale
                })
            }),
            "the focused action's keyboard label must also be visible"
        );
        let close_focus = visual.update(|window, cx| window.focused(cx)).unwrap();
        visual.simulate_resize(size(px(128.), px(100.)));
        let bounds = visual.expect(oldest_close);
        assert!(
            bounds.top() >= px(0.) && bounds.bottom() <= px(100.),
            "{bounds:?}"
        );
        assert_eq!(
            visual.update(|window, cx| window.focused(cx)),
            Some(close_focus.clone())
        );
        let oldest_delete: &'static str =
            Box::leak(format!("preview-delete-{oldest}-tooltip-trigger").into_boxed_str());
        visual.press("tab");
        let bounds = visual.expect(oldest_delete);
        assert!(
            bounds.left() >= px(0.) && bounds.right() <= px(128.),
            "{bounds:?}"
        );
        assert!(
            bounds.top() >= px(0.) && bounds.bottom() <= px(100.),
            "{bounds:?}"
        );
        assert_ne!(
            visual.update(|window, cx| window.focused(cx)),
            Some(close_focus)
        );
        visual.simulate_resize(size(px(208.), px(480.)));
        visual.settle();
        visual.click_at(gpui::point(px(100.), px(12.)));
        visual.simulate_event(gpui::ScrollEvent {
            position: gpui::point(px(100.), px(240.)),
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(320.))),
            modifiers: gpui::Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        visual.simulate_resize(size(px(208.), px(480.)));
        visual.settle();

        let close = gpui::point(px(25.), px(25.));
        visual.simulate_mouse_move(close, None, gpui::Modifiers::none());
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        visual.click_at(close);
        visual
            .executor()
            .advance_clock(Duration::from_millis(PREVIEW_EXIT_MS + 1));
        visual.settle();
        handle
            .update(&mut visual, |view, _, _| {
                assert_eq!(view.previews.len(), PREVIEW_MAX_STACK - 1);
                assert!(view.previews.iter().all(|preview| preview.id != newest));
            })
            .unwrap();
        assert!(path.is_file());
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn clipped_capture_preview_gets_its_full_dismiss_delay_after_scrolling_into_view(
        cx: &mut gpui::TestAppContext,
    ) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        store.update(|config| {
            config.preview.corner = "bottom-right".into();
            config.preview.auto_dismiss = false;
            config.preview.auto_dismiss_seconds = 2.;
        });
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(20, 20, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        let project = dir.path().join("Take.poratake");
        std::fs::create_dir(&project).unwrap();
        std::fs::write(
            crate::video::project::recording_video_path(&project),
            include_bytes!("../ui/parity/fixtures/capture.mov"),
        )
        .unwrap();
        cx.update(|cx| {
            crate::state::set_test_state(cx, store.clone());
            for _ in 1..PREVIEW_MAX_STACK {
                CapturePreviewWindow::open(cx, path.clone());
            }
            CapturePreviewWindow::open_video(cx, project, None);
        });
        cx.run_until_parked();
        let stack = stack_handle(cx).unwrap();
        let handle = stack.downcast::<CapturePreviewWindow>().unwrap();
        let mut visual = gpui::VisualTestContext::from_window(stack, cx);
        visual.simulate_resize(size(px(208.), px(300.)));
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        let (id, token, hidden_frame) = handle
            .update(&mut visual, |view, window, _| {
                let newest = view.previews.last().unwrap();
                assert!(view.preview_is_clipped(newest, window));
                assert!(newest.video.as_ref().unwrap().frames_released);
                (
                    newest.id,
                    newest.dismiss_token.clone(),
                    newest.image.clone().expect("initial video frame"),
                )
            })
            .unwrap();
        store.update(|config| config.preview.auto_dismiss = true);
        visual.update(|_, cx| schedule_auto_dismiss(stack, id, cx, token));
        visual.settle();
        let current_frame = |visual: &mut gpui::VisualTestContext| {
            handle
                .update(visual, |view, _, _| {
                    view.previews
                        .iter()
                        .find(|preview| preview.id == id)
                        .unwrap()
                        .image
                        .clone()
                        .unwrap()
                })
                .unwrap()
        };
        let pending = |visual: &mut gpui::VisualTestContext| {
            handle
                .update(visual, |view, _, _| {
                    view.previews
                        .iter()
                        .any(|preview| preview.id == id && preview.dismiss_animation.is_none())
                })
                .unwrap()
        };
        visual.executor().advance_clock(Duration::from_secs(3));
        visual.settle();
        assert!(
            pending(&mut visual),
            "a completely clipped card must stay available"
        );
        assert!(
            Arc::ptr_eq(&hidden_frame, &current_frame(&mut visual)),
            "a completely clipped card must stop decoding frames"
        );

        visual.simulate_event(gpui::ScrollEvent {
            position: gpui::point(px(100.), px(240.)),
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(400.))),
            modifiers: gpui::Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            ..Default::default()
        });
        visual.settle();
        assert!(
            !Arc::ptr_eq(&hidden_frame, &current_frame(&mut visual)),
            "a revealed card must decode frames again without a playback tick"
        );
        visual
            .executor()
            .advance_clock(Duration::from_millis(1_999));
        visual.settle();
        assert!(pending(&mut visual), "the revealed card gets a full delay");
        handle
            .update(&mut visual, |view, _, _| {
                let preview = view
                    .previews
                    .iter()
                    .find(|preview| preview.id == id)
                    .unwrap();
                assert!(!preview.video.as_ref().unwrap().frames_released);
            })
            .unwrap();
        visual.executor().advance_clock(Duration::from_millis(1));
        visual.settle();
        assert!(!pending(&mut visual));
        assert!(path.is_file());
    }

    #[test]
    fn fitted_stack_keeps_each_card_at_the_same_screen_position() {
        let display = Bounds {
            origin: gpui::point(px(-1920.), px(100.)),
            size: size(px(1920.), px(1080.)),
        };
        for corner in [
            PreviewCorner::TopLeft,
            PreviewCorner::TopRight,
            PreviewCorner::BottomLeft,
            PreviewCorner::BottomRight,
        ] {
            let bottom = matches!(
                corner,
                PreviewCorner::BottomLeft | PreviewCorner::BottomRight
            );
            let full = preview_fitted_stack_bounds(display, corner, PREVIEW_MAX_STACK);
            for count in 1..=PREVIEW_MAX_STACK {
                let fitted = preview_fitted_stack_bounds(display, corner, count);
                let offset = if bottom {
                    preview_stack_height(PREVIEW_MAX_STACK) - preview_stack_height(count)
                } else {
                    0.0
                };
                assert_eq!(fitted.size.height, px(preview_stack_height(count)));
                assert_eq!(fitted.left(), full.left());
                for index in 0..count {
                    let full_y = full.top() + px(preview_layout_y(index, bottom));
                    let fitted_y = fitted.top() + px(preview_layout_y(index, bottom) - offset);
                    assert_eq!(fitted_y, full_y);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn moving_the_preview_to_another_display_keeps_pending_work(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        let display = gpui::DisplayId::new(42);
        cx.simulate_display_added(display, None);
        store.update(|config| {
            config.preview.auto_dismiss = false;
            config.preview.display_id = Some(42);
        });
        cx.update(|cx| crate::state::set_test_state(cx, store.clone()));
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(20, 20, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();

        cx.update(|cx| {
            assert_eq!(selected_display(cx).unwrap().1, display);
            CapturePreviewWindow::open(cx, path.clone());
            CapturePreviewWindow::open(cx, path.clone());
        });
        cx.run_until_parked();
        let original = stack_handle(cx).unwrap();
        let (survivor, pending_owner) = original
            .downcast::<CapturePreviewWindow>()
            .unwrap()
            .update(cx, |view, window, cx| {
                let dismissed = view.previews[0].id;
                begin_remove_preview(view, dismissed, DismissBehavior::Automatic, window, cx);
                view.previews[1].busy = true;
                (view.previews[1].id, cx.entity().downgrade())
            })
            .unwrap();
        cx.run_until_parked();
        cx.simulate_display_removed(display);
        cx.run_until_parked();
        cx.update(|cx| {
            let _ = pending_owner.update(cx, |view, _| {
                if let Some(preview) = view
                    .previews
                    .iter_mut()
                    .find(|preview| preview.id == survivor)
                {
                    preview.busy = false;
                }
            });
        });
        cx.executor()
            .advance_clock(Duration::from_millis(PREVIEW_EXIT_MS + 1));
        cx.run_until_parked();

        let current = stack_handle(cx).unwrap();
        current
            .downcast::<CapturePreviewWindow>()
            .unwrap()
            .update(cx, |view, _, cx| {
                assert_eq!(view.previews.len(), 1);
                assert_eq!(view.previews[0].id, survivor);
                assert!(
                    !view.previews[0].busy,
                    "the pending preview action must finish"
                );
                assert_eq!(
                    selected_display(cx).unwrap().1,
                    cx.primary_display().unwrap().id()
                );
            })
            .unwrap();
        assert_eq!(current, original);
        assert_eq!(store.get().preview.display_id, Some(42));
        assert!(path.is_file());
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn preview_window_grows_and_shrinks_with_its_stack(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        store.update(|config| config.preview.auto_dismiss = false);
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(20, 20, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();

        for count in 1..=3 {
            cx.update(|cx| CapturePreviewWindow::open(cx, path.clone()));
            cx.run_until_parked();
            let handle = stack_handle(cx).unwrap();
            handle
                .update(cx, |_, window, cx| {
                    window.bounds_changed(cx);
                    assert_eq!(
                        window.viewport_size().height,
                        px(preview_stack_height(count))
                    );
                })
                .unwrap();
        }
        let handle = stack_handle(cx)
            .unwrap()
            .downcast::<CapturePreviewWindow>()
            .unwrap();
        handle
            .update(cx, |view, window, cx| {
                let id = view.previews[0].id;
                remove_preview_now(view, id, window, cx);
            })
            .unwrap();
        cx.run_until_parked();
        handle
            .update(cx, |_, window, cx| {
                window.bounds_changed(cx);
                assert_eq!(window.viewport_size().height, px(preview_stack_height(2)));
            })
            .unwrap();
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
            revealed: false,
            layout_generation: 0,
            #[cfg(target_os = "macos")]
            pointer_inside: false,
            #[cfg(target_os = "macos")]
            keyboard_controls: false,
            #[cfg(target_os = "macos")]
            pending_keyboard_tab: None,
            #[cfg(target_os = "macos")]
            viewport_scroll: None,
            display_menu: crate::ui::menu::MenuHandle::new(),
            tooltip_presenter: crate::ui::tooltip::NativeTooltipPresenter::new("capture-preview"),
            _display_observer: None,
            #[cfg(target_os = "macos")]
            _visibility_observer: None,
        };
        let first = start_layout_transition(&mut view, true, Instant::now());
        let second = start_layout_transition(&mut view, true, Instant::now());
        assert_ne!(first, second);
        assert_eq!(view.layout_generation, second);
    }

    #[test]
    fn controls_hide_once_an_operation_finishes() {
        assert!(CapturePreviewWindow::show_controls(true, false, false));
        assert!(CapturePreviewWindow::show_controls(false, true, false));
        assert!(!CapturePreviewWindow::show_controls(false, false, false));
        assert!(!CapturePreviewWindow::show_controls(true, true, true));
    }

    #[test]
    fn a_finished_preview_blocks_the_auto_dismiss_timer() {
        assert_eq!(
            dismiss_state(4, 4, false, false, true),
            DismissVerdict::Blocked
        );
        assert_eq!(
            dismiss_state(4, 4, false, false, false),
            DismissVerdict::Ready
        );
    }

    #[test]
    fn the_finished_badge_zooms_in_from_half_size_over_its_enter_duration() {
        let started = Instant::now();
        assert_eq!(
            crate::ui::preview::finished_zoom(finished_progress(started, started)),
            crate::ui::preview::FINISHED_ZOOM_FROM
        );
        let half = started + Duration::from_millis(crate::ui::preview::FINISHED_ENTER_MS / 2);
        assert!((finished_progress(started, half) - 0.5).abs() < 0.01);
        let done = started + Duration::from_millis(crate::ui::preview::FINISHED_ENTER_MS * 2);
        assert_eq!(finished_progress(started, done), 1.0);
        assert_eq!(crate::ui::preview::finished_zoom(1.0), 1.0);
    }

    #[test]
    fn export_progress_reads_back_as_a_clamped_fraction() {
        let progress = Arc::new(AtomicU32::new(0));
        assert_eq!(export_fraction(&progress), 0.0);
        progress.store(250, Ordering::Relaxed);
        assert_eq!(export_fraction(&progress), 0.25);
        progress.store(4000, Ordering::Relaxed);
        assert_eq!(export_fraction(&progress), 1.0);
    }

    #[test]
    fn the_video_playhead_loops_over_the_duration() {
        assert_eq!(super::looped_playhead(1.0, 10.0), 1.0);
        assert_eq!(super::looped_playhead(12.5, 10.0), 2.5);
        assert_eq!(super::looped_playhead(5.0, 0.0), 0.0);
        assert_eq!(super::looped_playhead(5.0, f64::NAN), 0.0);
    }

    #[test]
    fn a_decoded_frame_cover_fits_the_thumbnail() {
        let frame = crate::video::decoder::DecodedFrame {
            width: 4,
            height: 2,
            bgra: vec![10u8; 4 * 4 * 2].into(),
        };
        assert!(super::video_thumbnails(&frame, false).is_some());
        let empty = crate::video::decoder::DecodedFrame {
            width: 0,
            height: 0,
            bgra: Vec::new().into(),
        };
        assert!(super::video_thumbnails(&empty, true).is_none());
    }

    #[test]
    fn video_thumbnails_preserve_the_cover_crop_and_alpha() {
        for (width, height) in [
            (3840, 2160),
            (2160, 3840),
            (1543, 663),
            (663, 1543),
            (200, 140),
            (7, 5),
            (3, 7),
        ] {
            let pixels = image::RgbaImage::from_fn(width, height, |x, y| {
                image::Rgba([
                    (x.wrapping_mul(7) % 256) as u8,
                    (y.wrapping_mul(11) % 256) as u8,
                    ((x + y).wrapping_mul(3) % 256) as u8,
                    ((x + y) % 256) as u8,
                ])
            });
            let expected = image::DynamicImage::ImageRgba8(pixels.clone())
                .resize_to_fill(
                    PREVIEW_WIDTH as u32,
                    PREVIEW_HEIGHT as u32,
                    image::imageops::FilterType::Triangle,
                )
                .into_rgba8();
            let mut rgba = expected.clone();
            for pixel in rgba.as_mut().as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            let blurred = blur_thumbnail(&rgba).unwrap();
            let source = crate::video::decoder::DecodedFrame {
                width,
                height,
                bgra: pixels.into_raw().into(),
            };
            let (thumbnail, no_blur) = video_thumbnails(&source, false).unwrap();
            assert_eq!(thumbnail.as_bytes(0), Some(expected.as_raw().as_slice()));
            assert!(no_blur.is_none());
            let (thumbnail, actual_blur) = video_thumbnails(&source, true).unwrap();
            assert_eq!(thumbnail.as_bytes(0), Some(expected.as_raw().as_slice()));
            assert_eq!(actual_blur.unwrap().as_bytes(0), blurred.as_bytes(0));
        }
    }

    #[test]
    fn video_hover_blur_matches_the_screenshot_thumbnail() {
        let directory = tempfile::tempdir().unwrap();
        for (width, height) in [(320, 180), (180, 320)] {
            let path = directory.path().join("frame.png");
            let pixels = image::RgbaImage::from_fn(width, height, |x, y| {
                let value = if (x / 12 + y / 12) % 2 == 0 { 0 } else { 255 };
                image::Rgba([value, 80, 255 - value, 255])
            });
            pixels.save(&path).unwrap();
            let mut bgra = pixels.into_raw();
            for pixel in bgra.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            let frame = crate::video::decoder::DecodedFrame {
                width,
                height,
                bgra: bgra.into(),
            };
            let (thumbnail, blurred) = super::video_thumbnails(&frame, true).unwrap();
            let (screenshot, screenshot_blurred) =
                super::build_preview_thumbnails(super::decode_preview_fallback(&path).unwrap());
            let blurred = blurred.unwrap();
            assert_eq!(thumbnail.as_bytes(0), screenshot.unwrap().as_bytes(0));
            assert_eq!(blurred.as_bytes(0), screenshot_blurred.unwrap().as_bytes(0));
            assert_ne!(thumbnail.as_bytes(0), blurred.as_bytes(0));
            let (native_thumb, native_blur) = load_preview_thumbnails(&path);
            let native_thumb = native_thumb.unwrap();
            let native_blur = native_blur.unwrap();
            assert_eq!(native_thumb.size(0), thumbnail.size(0));
            assert_eq!(native_blur.size(0), blurred.size(0));
            assert_ne!(native_thumb.as_bytes(0), native_blur.as_bytes(0));
            assert!(super::video_thumbnails(&frame, false).unwrap().1.is_none());
        }
    }

    #[test]
    fn single_decode_thumbnails_match_the_previous_two_decode_output() {
        let directory = tempfile::tempdir().unwrap();
        for (index, (width, height, translucent)) in
            [(640, 360, false), (90, 240, true), (180, 180, true)]
                .into_iter()
                .enumerate()
        {
            let path = directory.path().join(format!("fixture-{index}.png"));
            let fixture = image::RgbaImage::from_fn(width, height, |x, y| {
                let alpha = if translucent {
                    ((x + y) % 5 * 63) as u8
                } else {
                    255
                };
                image::Rgba([
                    (x.wrapping_mul(7) % 256) as u8,
                    (y.wrapping_mul(11) % 256) as u8,
                    ((x + y).wrapping_mul(3) % 256) as u8,
                    alpha,
                ])
            });
            fixture.save(&path).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            let expected_thumbnail = render_thumbnail(
                image::load_from_memory(&bytes)
                    .unwrap()
                    .resize_to_fill(
                        PREVIEW_WIDTH as u32,
                        PREVIEW_HEIGHT as u32,
                        image::imageops::FilterType::Triangle,
                    )
                    .to_rgba8(),
            );
            let mut expected_blur = image::load_from_memory(&bytes)
                .unwrap()
                .resize_to_fill(
                    PREVIEW_WIDTH as u32,
                    PREVIEW_HEIGHT as u32,
                    image::imageops::FilterType::Triangle,
                )
                .to_rgba8();
            let mut pixmap = crate::editor::export::from_rgba(&expected_blur).unwrap();
            crate::render::blur::blur(&mut pixmap, BLUR_SIGMA);
            expected_blur = crate::editor::export::to_rgba(&pixmap);
            let expected_blur = render_thumbnail(expected_blur);

            let (fallback_thumb, fallback_blur) =
                super::build_preview_thumbnails(super::decode_preview_fallback(&path).unwrap());
            assert_eq!(
                fallback_thumb.unwrap().as_bytes(0),
                expected_thumbnail.as_bytes(0),
                "thumbnail pixels differ for {width}x{height}, translucent={translucent}"
            );
            assert_eq!(
                fallback_blur.unwrap().as_bytes(0),
                expected_blur.as_bytes(0),
                "blurred pixels differ for {width}x{height}, translucent={translucent}"
            );

            let (thumbnail, blurred) = load_preview_thumbnails(&path);
            let thumbnail = thumbnail.unwrap();
            let blurred = blurred.unwrap();
            assert_eq!(thumbnail.size(0), expected_thumbnail.size(0));
            assert_eq!(blurred.size(0), expected_blur.size(0));
            let mut rgba = thumbnail.as_bytes(0).unwrap().to_vec();
            for pixel in rgba.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            let reblurred = super::blur_thumbnail(
                &image::RgbaImage::from_raw(PREVIEW_WIDTH as u32, PREVIEW_HEIGHT as u32, rgba)
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(
                blurred.as_bytes(0),
                reblurred.as_bytes(0),
                "single-decode blur differs for {width}x{height}, translucent={translucent}"
            );
        }
    }

    #[herogpui::test]
    fn open_video_stacks_a_recording_preview(cx: &mut gpui::TestAppContext) {
        use crate::config::store::ConfigStore;

        let dir = tempfile::tempdir().expect("temp dir");
        let project = dir.path().join("Take 1.poratake");
        std::fs::create_dir(&project).expect("project dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        cx.update(|cx| CapturePreviewWindow::open_video(cx, project.clone(), None));
        cx.run_until_parked();

        let kinds = stack_handle(cx)
            .and_then(|stack| stack.downcast::<CapturePreviewWindow>())
            .and_then(|handle| {
                handle
                    .update(cx, |view, _, _| {
                        view.previews
                            .iter()
                            .map(|preview| {
                                assert!(!preview.thumbnail_loading);
                                assert!(preview.dismiss_token.load(Ordering::Relaxed) > 0);
                                (preview.kind, preview.path.clone())
                            })
                            .collect::<Vec<_>>()
                    })
                    .ok()
            })
            .expect("stack window");
        assert_eq!(kinds, vec![(super::PreviewKind::Video, project)]);
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn video_preview_decodes_its_first_frame_before_the_playback_tick(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("Take.poratake");
        std::fs::create_dir(&project).unwrap();
        std::fs::write(
            crate::video::project::recording_video_path(&project),
            include_bytes!("../ui/parity/fixtures/capture.mov"),
        )
        .unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            CapturePreviewWindow::open_video(cx, project, None);
            let handle = registry::handle(WindowKind::CapturePreview, cx)
                .and_then(|handle| handle.downcast::<CapturePreviewWindow>())
                .unwrap();
            handle
                .update(cx, |view, _, _| {
                    let preview = &view.previews[0];
                    assert!(preview.thumbnail_loading);
                    assert_eq!(preview.dismiss_token.load(Ordering::Relaxed), 0);
                })
                .unwrap();
        });
        cx.run_until_parked();

        let stack = stack_handle(cx)
            .and_then(|stack| stack.downcast::<CapturePreviewWindow>())
            .unwrap();
        let redraw = |cx: &mut gpui::TestAppContext,
                      current: &gpui::RenderImage,
                      retired: Option<&gpui::RenderImage>| {
            cx.update(|cx| {
                cx.update_window(stack.into(), |_, window, cx| {
                    window.draw(cx).clear(cx);
                    assert!(window.has_image_atlas_entry(current));
                    if let Some(retired) = retired {
                        assert!(!window.has_image_atlas_entry(retired));
                    }
                })
                .unwrap();
            });
        };
        let first = stack
            .update(cx, |view, _, _| {
                let preview = &view.previews[0];
                assert!(!preview.thumbnail_loading);
                assert!(preview.dismiss_token.load(Ordering::Relaxed) > 0);
                preview.image.clone()
            })
            .unwrap()
            .expect("the video thumbnail should be ready");
        redraw(cx, &first, None);

        cx.simulate_window_visibility_change(stack.into(), gpui::WindowVisibility::Hidden);
        cx.run_until_parked();
        stack
            .update(cx, |view, _, _| {
                let preview = &view.previews[0];
                assert!(preview.video.as_ref().unwrap().frames_released);
                assert!(Arc::ptr_eq(preview.image.as_ref().unwrap(), &first));
            })
            .unwrap();
        cx.simulate_window_visibility_change(stack.into(), gpui::WindowVisibility::Visible);
        cx.run_until_parked();
        let resumed = stack
            .update(cx, |view, _, _| {
                let preview = &view.previews[0];
                assert!(!preview.video.as_ref().unwrap().frames_released);
                preview.image.clone().unwrap()
            })
            .unwrap();
        assert!(!Arc::ptr_eq(&resumed, &first));
        redraw(cx, &resumed, Some(&first));

        stack
            .update(cx, |view, _, _| view.previews[0].busy = true)
            .unwrap();

        cx.executor().advance_clock(super::VIDEO_PLAYBACK_TICK);
        cx.run_until_parked();
        let (id, blurred) = stack
            .update(cx, |view, _, _| {
                let preview = &view.previews[0];
                (
                    preview.id,
                    preview.blurred.clone().expect("blurred playback frame"),
                )
            })
            .unwrap();
        redraw(cx, &blurred, Some(&resumed));

        let decoder = stack
            .update(cx, |view, window, cx| {
                let preview = &view.previews[0];
                let decoder = Arc::downgrade(&preview.video.as_ref().unwrap().decoder);
                let displayed = preview.image.clone().unwrap();
                begin_remove_preview(view, id, DismissBehavior::PreserveControls, window, cx);
                let dismissing = &view.previews[0];
                assert!(dismissing.dismiss_animation.is_some());
                assert!(dismissing.video.is_none());
                assert!(Arc::ptr_eq(dismissing.image.as_ref().unwrap(), &displayed));
                assert!(Arc::ptr_eq(dismissing.blurred.as_ref().unwrap(), &blurred));
                decoder
            })
            .unwrap();
        assert!(
            decoder.upgrade().is_none(),
            "dismissal retained the decoder"
        );

        cx.update(|cx| {
            CapturePreviewWindow::open_video(cx, dir.path().join("Other.poratake"), None)
        });
        stack
            .update(cx, |view, window, cx| {
                super::remove_preview_now(view, id, window, cx);
                assert_eq!(view.previews.len(), 1);
                assert!(!window.has_image_atlas_entry(&blurred));
            })
            .unwrap();
    }

    #[test]
    fn interactive_dismissal_preserves_controls_after_window_hover_ends() {
        let now = Instant::now();
        let mut preview = CapturePreview {
            id: 1,
            kind: super::PreviewKind::Screenshot,
            path: PathBuf::new(),
            image: None,
            thumbnail_loading: false,
            thumbnail_cancel: None,
            thumbnail_generation: 0,
            #[cfg(all(test, feature = "parity"))]
            thumbnail_ready: None,
            blurred: None,
            video: None,
            export_cancel: None,
            hover_progress: 0.75,
            last_frame: Some(now),
            hovered: true,
            busy: false,
            deleting: false,
            copying: false,
            finalizing: false,
            completed: None,
            export_progress: None,
            dismiss_token: Arc::new(AtomicU64::new(0)),
            #[cfg(target_os = "macos")]
            dismiss_viewport_clipped: Cell::new(false),
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
            CapturePreviewWindow::control_frame(&mut preview, false, false, false, now),
            (true, 0.75, false)
        );
    }

    #[test]
    fn canceled_or_removed_thumbnail_generation_cannot_be_applied() {
        let now = Instant::now();
        let path = PathBuf::from("capture.png");
        let (cancel, canceled) = smol::channel::bounded(1);
        let mut preview = CapturePreview {
            id: 71,
            kind: PreviewKind::Screenshot,
            path: path.clone(),
            image: None,
            thumbnail_loading: true,
            thumbnail_cancel: Some(cancel),
            thumbnail_generation: 9,
            #[cfg(all(test, feature = "parity"))]
            thumbnail_ready: None,
            blurred: None,
            video: None,
            export_cancel: None,
            hover_progress: 0.0,
            last_frame: None,
            hovered: false,
            busy: false,
            deleting: false,
            copying: false,
            finalizing: false,
            completed: None,
            export_progress: None,
            dismiss_token: Arc::new(AtomicU64::new(0)),
            #[cfg(target_os = "macos")]
            dismiss_viewport_clipped: Cell::new(false),
            entered_at: now,
            layout_y: None,
            layout_animation: None,
            dismiss_animation: None,
        };
        assert!(thumbnail_preview_matches(&preview, 71, &path, 9));

        cancel_thumbnail_load(&mut preview);

        assert!(canceled.try_recv().is_ok());
        assert!(!thumbnail_preview_matches(&preview, 71, &path, 9));
        assert!(thumbnail_preview_mut(&mut [], 71, &path, 9).is_none());
    }

    #[test]
    fn thumbnail_decode_gate_is_one_slot_per_app() {
        let mut first_app = gpui::HeadlessAppContext::with_platform(
            Arc::new(gpui::NoopTextSystem),
            Arc::new(()),
            || Ok(None),
        );
        let slots = first_app.update(preview_thumbnail_decode_slots);
        let same_app_slots = first_app.update(preview_thumbnail_decode_slots);
        assert!(Arc::ptr_eq(&slots, &same_app_slots));

        let mut second_app = gpui::HeadlessAppContext::with_platform(
            Arc::new(gpui::NoopTextSystem),
            Arc::new(()),
            || Ok(None),
        );
        let other_app_slots = second_app.update(preview_thumbnail_decode_slots);
        assert!(!Arc::ptr_eq(&slots, &other_app_slots));

        smol::block_on(async {
            let first = slots.acquire().await;
            let mut second = Box::pin(slots.acquire());
            assert!(smol::future::poll_once(second.as_mut()).await.is_none());
            drop(first);
            assert!(smol::future::poll_once(second.as_mut()).await.is_some());
        });
    }

    #[cfg(feature = "parity")]
    #[test]
    fn queued_thumbnail_cancel_and_reopen_reuses_the_app_decode_gate() {
        let directory = tempfile::tempdir().unwrap();
        let first_path = directory.path().join("first.png");
        let second_path = directory.path().join("second.png");
        image::RgbaImage::new(32, 24).save(&first_path).unwrap();
        image::RgbaImage::new(24, 32).save(&second_path).unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        let mut cx = gpui::HeadlessAppContext::with_platform(
            Arc::new(gpui::NoopTextSystem),
            Arc::new(()),
            || Ok(None),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let _reveal_guard = HeadlessPreviewRevealGuard::enter();
        let decode_slots = cx.update(preview_thumbnail_decode_slots);
        let held_slot = smol::block_on(decode_slots.acquire());

        cx.update(|cx| CapturePreviewWindow::open(cx, first_path));
        cx.run_until_parked();
        let first_handle = cx
            .update(|cx| registry::handle(WindowKind::CapturePreview, cx))
            .expect("first preview stack");
        assert!(first_handle
            .downcast::<CapturePreviewWindow>()
            .unwrap()
            .update(&mut cx, |view, _, _| view.previews[0].thumbnail_loading)
            .unwrap());
        first_handle
            .downcast::<CapturePreviewWindow>()
            .unwrap()
            .update(&mut cx, |view, window, cx| {
                for preview in &mut view.previews {
                    cancel_thumbnail_load(preview);
                }
                view.previews.clear();
                registry::forget(WindowKind::CapturePreview, cx);
                window.remove_window();
            })
            .unwrap();
        cx.run_until_parked();

        cx.update(|cx| CapturePreviewWindow::open(cx, second_path));
        cx.run_until_parked();
        let second_handle = cx
            .update(|cx| registry::handle(WindowKind::CapturePreview, cx))
            .expect("reopened preview stack");
        let ready = second_handle
            .downcast::<CapturePreviewWindow>()
            .unwrap()
            .update(&mut cx, |view, _, _| {
                assert!(view.previews[0].thumbnail_loading);
                view.previews[0].thumbnail_ready.take().unwrap()
            })
            .unwrap();
        drop(held_slot);
        cx.run_until_parked();

        assert!(ready.try_recv().is_ok());
        assert!(second_handle
            .downcast::<CapturePreviewWindow>()
            .unwrap()
            .update(&mut cx, |view, _, _| {
                !view.previews[0].thumbnail_loading && view.previews[0].image.is_some()
            })
            .unwrap());
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

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn covering_a_preview_cancels_pending_hover_tooltips(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(20, 20, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|config| config.preview.auto_dismiss = false);
        cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            CapturePreviewWindow::open(cx, path);
        });
        cx.run_until_parked();
        let window = stack_handle(cx).unwrap();
        let stack = window.downcast::<CapturePreviewWindow>().unwrap();
        stack
            .update(cx, |view, window, _| {
                view.pointer_inside = true;
                view.previews[0].hovered = true;
                view.tooltip_presenter.show_for_test(
                    "close".into(),
                    "Close preview".into(),
                    window,
                );
                assert!(view.tooltip_presenter.active_content().is_some());
            })
            .unwrap();
        cx.simulate_window_visibility_change(window, gpui::WindowVisibility::Hidden);
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        stack
            .update(cx, |view, window, _| {
                assert!(!view.preview_window_hovered(window));
                assert!(view.tooltip_presenter.active_content().is_none());
            })
            .unwrap();
        assert_eq!(crate::windows::scroll_capture::test_native_tooltip(), None);

        cx.simulate_window_visibility_change(window, gpui::WindowVisibility::Visible);
        cx.run_until_parked();
        assert_eq!(crate::windows::scroll_capture::test_native_tooltip(), None);
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn covered_preview_gets_its_dismiss_delay_after_becoming_visible(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(20, 20, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        store.update(|config| {
            config.preview.auto_dismiss = true;
            config.preview.auto_dismiss_seconds = 2.0;
        });
        cx.update(|cx| {
            crate::state::set_test_state(cx, store);
            CapturePreviewWindow::open(cx, path.clone());
        });
        cx.run_until_parked();
        let window = stack_handle(cx).unwrap();
        let stack = window.downcast::<CapturePreviewWindow>().unwrap();
        let active = |cx: &mut gpui::TestAppContext| {
            stack
                .update(cx, |view, _, _| view.active_preview_count())
                .unwrap_or(0)
        };

        cx.executor().advance_clock(Duration::from_secs(1));
        cx.simulate_window_visibility_change(window, gpui::WindowVisibility::Hidden);
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(3));
        cx.run_until_parked();
        assert_eq!(active(cx), 1, "the covered card must stay available");

        cx.simulate_window_visibility_change(window, gpui::WindowVisibility::Visible);
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(1_900));
        cx.run_until_parked();
        assert_eq!(active(cx), 1, "the revealed card gets a full delay");
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        assert_eq!(active(cx), 0);
        assert!(path.is_file());
    }

    #[herogpui::test]
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
        let handle = stack_handle(cx).expect("the preview opened the shared stack window");
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

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn auto_dismiss_waits_for_unfocused_pointer_exit(cx: &mut gpui::TestAppContext) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(60, 40, image::Rgba([10, 20, 30, 255]))
            .save(&path)
            .unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|settings| {
            settings.preview.auto_dismiss = true;
            settings.preview.auto_dismiss_seconds = 1.0;
        });
        cx.update(|cx| {
            herogpui::init(cx);
            crate::state::set_test_state(cx, config);
            CapturePreviewWindow::open(cx, path.clone());
        });
        let handle = stack_handle(cx)
            .unwrap()
            .downcast::<CapturePreviewWindow>()
            .unwrap();
        let mut visual = gpui::VisualTestContext::from_window(handle.into(), cx);
        visual.settle();
        visual.simulate_mouse_move(
            gpui::point(px(100.), px(60.)),
            None,
            gpui::Modifiers::none(),
        );
        visual.settle();
        assert!(handle
            .update(&mut visual, |view, _, _| view.pointer_inside
                && view.previews[0].hovered)
            .unwrap());
        cx.executor().advance_clock(Duration::from_secs(3));
        cx.run_until_parked();
        assert!(
            handle.update(cx, |_, _, _| ()).is_ok(),
            "hover must hold the preview open"
        );
        visual.simulate_event(gpui::MouseExitEvent {
            position: gpui::point(px(210.), px(60.)),
            pressed_button: None,
            modifiers: gpui::Modifiers::none(),
        });
        visual.settle();
        cx.executor().advance_clock(Duration::from_secs(2));
        cx.run_until_parked();
        cx.executor()
            .advance_clock(Duration::from_millis(PREVIEW_EXIT_MS));
        cx.run_until_parked();
        assert!(
            handle.update(cx, |_, _, _| ()).is_err(),
            "preview must close after exit"
        );
        assert!(
            path.is_file(),
            "automatic dismissal must preserve the source image"
        );
    }

    #[herogpui::test]
    fn hidden_preview_starts_its_dismiss_delay_after_reveal(cx: &mut gpui::TestAppContext) {
        use crate::config::store::ConfigStore;

        let dir = tempfile::tempdir().expect("temp dir");
        let store = std::sync::Arc::new(
            ConfigStore::load_at(dir.path().join("config.json")).expect("load config"),
        );
        store.update(|config| {
            config.preview.auto_dismiss = true;
            config.preview.auto_dismiss_seconds = 3.0;
        });
        cx.update(|cx| crate::state::set_test_state(cx, store));

        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(60, 40, image::Rgba([10, 20, 30, 255]))
            .save(&path)
            .expect("write capture");
        cx.update(|cx| CapturePreviewWindow::open(cx, path.clone()));
        cx.run_until_parked();
        let handle = stack_handle(cx)
            .expect("the preview opened the shared stack window")
            .downcast::<CapturePreviewWindow>()
            .expect("preview stack");
        handle
            .update(cx, |view, _, _| {
                view.revealed = false;
                for preview in &view.previews {
                    preview.dismiss_token.fetch_add(1, Ordering::Relaxed);
                }
            })
            .expect("delay preview reveal");

        cx.update(|cx| CapturePreviewWindow::open(cx, path));
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(4));
        cx.run_until_parked();
        assert_eq!(
            handle
                .update(cx, |view, _, _| view.active_preview_count())
                .unwrap(),
            2,
            "hidden previews must not consume their dismiss delay"
        );

        let view = handle.root(cx).expect("preview stack root");
        let any_handle: AnyWindowHandle = handle.into();
        cx.update(|cx| reveal_preview_stack_view(any_handle, view, cx));
        cx.executor().advance_clock(Duration::from_secs(3));
        cx.run_until_parked();
        cx.executor()
            .advance_clock(Duration::from_millis(PREVIEW_EXIT_MS));
        cx.run_until_parked();
        assert!(
            handle.update(cx, |_, _, _| ()).is_err(),
            "both hidden previews must start their full dismiss delay after reveal"
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
        assert_eq!(
            dismiss_state(4, 4, false, false, false),
            DismissVerdict::Ready
        );
    }

    #[test]
    fn window_exit_and_reentry_gate_the_stored_hover() {
        let stored_hover = true;
        let visible = [false, true].map(|window_hovered| {
            CapturePreviewWindow::show_controls(stored_hover && window_hovered, false, false)
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
        let control_call = |id: &str| {
            let marker = format!("(\"{id}\", id),");
            source
                .split_once(&marker)
                .unwrap_or_else(|| panic!("missing {id}"))
                .1
                .split_once("cx.listener")
                .unwrap_or_else(|| panic!("missing {id} listener"))
                .0
        };

        for id in ["preview-close", "preview-delete"] {
            assert!(control_call(id).contains("theme.destructive,"), "{id}");
        }
        for id in ["preview-copy", "preview-upload"] {
            assert!(control_call(id).contains("theme.primary,"), "{id}");
        }
        let display = source
            .split_once("let owner = format!(\"preview-display-menu-")
            .unwrap_or_else(|| panic!("missing preview-display-menu"))
            .1
            .split_once("cx.stop_propagation")
            .unwrap_or_else(|| panic!("missing display menu listener"))
            .0;
        assert!(
            display.contains("theme.primary,"),
            "display menu hover is primary"
        );
        assert!(
            control_call("preview-show-in-folder").contains("theme.primary,"),
            "preview-show-in-folder"
        );

        let pill = include_str!("../ui/preview.rs")
            .split_once("pub fn pill")
            .expect("pill helper")
            .1
            .split_once("fn chip")
            .expect("pill helper body")
            .0;
        assert!(pill.contains("theme.primary,"));
    }

    fn open_hovered_screenshot_preview<'a>(
        cx: &'a mut gpui::TestAppContext,
        dir: &tempfile::TempDir,
    ) -> (
        gpui::Entity<CapturePreviewWindow>,
        &'a mut gpui::VisualTestContext,
        &'static str,
        gpui::Point<Pixels>,
    ) {
        use herogpui::test::TestWindowExt;

        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(60, 40, image::Rgba([10, 20, 30, 255]))
            .save(&path)
            .unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|settings| {
            settings.preview.corner = "top-right".into();
            settings.preview.auto_dismiss = false;
        });
        cx.update(|cx| {
            herogpui::init(cx);
            crate::state::set_test_state(cx, config);
            CapturePreviewWindow::open(cx, path);
        });
        let stack = stack_handle(cx)
            .unwrap()
            .downcast::<CapturePreviewWindow>()
            .unwrap();
        let preview = stack.update(cx, |_, _, cx| cx.entity()).unwrap();
        let check = preview.clone();
        let (_, visual) = herogpui::test::open_window(cx, |_, _| HoveredPreview { preview });
        visual.simulate_resize(preview_stack_size(PREVIEW_MAX_STACK));
        visual.executor().advance_clock(Duration::from_millis(500));
        visual.settle();
        visual.simulate_mouse_move(
            gpui::point(px(100.), px(70.)),
            None,
            gpui::Modifiers::none(),
        );
        visual.executor().advance_clock(Duration::from_millis(250));
        visual.settle();
        let id = visual.update(|_, cx| check.read(cx).previews[0].id);
        let close_selector: &'static str =
            Box::leak(format!("preview-close-{id}-tooltip-trigger").into_boxed_str());
        let close_point = visual
            .debug_bounds(close_selector)
            .expect("Close control must render while hovered")
            .center();
        (check, visual, close_selector, close_point)
    }

    #[herogpui::test]
    fn mouse_down_on_preview_icon_keeps_controls_until_drag_starts(cx: &mut gpui::TestAppContext) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let (preview, visual, close_selector, close_point) =
            open_hovered_screenshot_preview(cx, &dir);

        visual.simulate_mouse_move(close_point, None, gpui::Modifiers::none());
        visual.settle();
        visual.simulate_mouse_down(close_point, MouseButton::Left, gpui::Modifiers::none());
        visual.settle();
        visual.simulate_mouse_move(
            close_point,
            Some(MouseButton::Left),
            gpui::Modifiers::none(),
        );
        visual.settle();
        visual.simulate_mouse_move(
            gpui::point(close_point.x + px(1.), close_point.y),
            Some(MouseButton::Left),
            gpui::Modifiers::none(),
        );
        visual.settle();

        assert!(!visual.update(|_, cx| cx.has_active_drag()));
        assert!(visual.update(|_, cx| preview.read(cx).previews[0].hovered));
        assert!(visual.debug_bounds(close_selector).is_some());

        visual.simulate_mouse_move(
            gpui::point(close_point.x + px(12.), close_point.y),
            Some(MouseButton::Left),
            gpui::Modifiers::none(),
        );
        visual.settle();

        assert!(visual.update(|_, cx| cx.has_active_drag()));
        assert!(!visual.update(|_, cx| preview.read(cx).previews[0].hovered));
        assert!(visual.debug_bounds(close_selector).is_none());

        visual.simulate_mouse_up(
            gpui::point(close_point.x + px(12.), close_point.y),
            MouseButton::Left,
            gpui::Modifiers::none(),
        );
        visual.settle();
        visual.executor().advance_clock(Duration::from_millis(250));
        visual.settle();

        assert!(!visual.update(|_, cx| cx.has_active_drag()));
        assert!(visual.update(|_, cx| preview.read(cx).previews[0].dismiss_animation.is_none()));
        assert!(visual.update(|_, cx| preview.read(cx).previews[0].hovered));
        assert!(visual.debug_bounds(close_selector).is_some());
    }

    #[herogpui::test]
    fn mouse_down_with_stationary_move_still_clicks_preview_icon(cx: &mut gpui::TestAppContext) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let (preview, visual, _, close_point) = open_hovered_screenshot_preview(cx, &dir);

        visual.simulate_mouse_move(close_point, None, gpui::Modifiers::none());
        visual.settle();
        visual.simulate_mouse_down(close_point, MouseButton::Left, gpui::Modifiers::none());
        visual.settle();
        visual.simulate_mouse_move(
            close_point,
            Some(MouseButton::Left),
            gpui::Modifiers::none(),
        );
        visual.settle();
        visual.simulate_mouse_up(close_point, MouseButton::Left, gpui::Modifiers::none());
        visual.settle();

        assert!(visual.update(|_, cx| preview.read(cx).previews[0].dismiss_animation.is_some()));
    }
}
