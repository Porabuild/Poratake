//! Area selection overlay — a fullscreen transparent window per the ported
//! `area-overlay` flow: dimmed screen, drag to draw the selection, Esc to
//! cancel, release to confirm and capture.

use gpui::{
    div, prelude::*, px, AnyWindowHandle, App, Bounds, Context, DisplayId, Global, KeyBinding,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Render, Styled,
    Window, WindowOptions,
};
use herogpui::actions;
use herogpui::gpui;
#[cfg(not(test))]
use poratake_daemon_common::contract::RecordingOverlayShowWindowRequest;
use poratake_daemon_common::geometry::{CaptureRect, DisplayOrigin};

use crate::capture::selection;
use crate::capture::CaptureService;

actions!(overlay, [Cancel]);

pub fn init_bindings(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Cancel, Some("AreaOverlay"))]);
}

/// Which pointer gesture is in flight, mirroring the `Interaction` union in
/// `use-area-selection.ts`.
#[derive(Clone, Copy, Debug)]
enum Interaction {
    Creating { start: selection::Point },
    Moving { offset: selection::Point },
    Resizing { handle: selection::Handle },
}

/// A confirmed selection in global *physical* pixels.
pub type ScreenRect = CaptureRect;

#[derive(Clone, Copy)]
pub struct OverlayLaunch {
    pub focus: bool,
    pub deferred_show: bool,
    pub generation: u64,
}

#[derive(Default)]
struct OverlaySession {
    handles: Vec<TrackedOverlay>,
    #[cfg(target_os = "macos")]
    scroll_selectors: std::collections::HashMap<u64, Vec<TrackedOverlay>>,
    capture_hide: Option<std::sync::Arc<crate::capture::desktop_icons::CaptureHide>>,
    revision: u64,
    pending_freeze: Option<u64>,
    timer_cancel: Option<smol::channel::Sender<bool>>,
    #[cfg(target_os = "macos")]
    timer_release_owner: Option<u64>,
    #[cfg(any(windows, target_os = "macos"))]
    pooled: Vec<PooledOverlay>,
    outlined_window_id: Option<i64>,
    #[cfg(target_os = "macos")]
    native_recording_overlay_requested: bool,
    #[cfg(target_os = "macos")]
    pending_recording_preparation: Option<smol::channel::Receiver<()>>,
}

#[derive(Clone)]
struct TrackedOverlay {
    handle: AnyWindowHandle,
    #[cfg(any(windows, target_os = "macos"))]
    display_id: Option<DisplayId>,
    generation: u64,
    #[cfg(any(windows, target_os = "macos"))]
    focus: bool,
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PooledSurfaceState {
    Clean,
    Dirty,
    AwaitingFirstPaint,
    Live,
}

#[cfg(any(windows, target_os = "macos"))]
struct PooledOverlay {
    handle: gpui::WindowHandle<AreaOverlay>,
    display_id: DisplayId,
}

impl Global for OverlaySession {}

fn session(cx: &mut App) -> &mut OverlaySession {
    cx.default_global::<OverlaySession>()
}

pub(crate) fn revision(cx: &mut App) -> u64 {
    session(cx).revision
}

pub(crate) fn set_pending_freeze(generation: u64, cx: &mut App) {
    if generation == 0 {
        return;
    }
    let pending = session(cx).pending_freeze;
    session(cx).pending_freeze =
        Some(pending.map_or(generation, |pending| pending.max(generation)));
}

pub(crate) fn finish_pending_freeze_release(generation: u64, released: bool, cx: &mut App) {
    if generation != 0 && released && session(cx).pending_freeze == Some(generation) {
        session(cx).pending_freeze = None;
        crate::system::memory::schedule_idle_reclaim(cx);
    }
}

pub(crate) fn retry_pending_freeze_release(generation: u64, cx: &mut App) {
    if generation == 0 || session(cx).pending_freeze != Some(generation) {
        return;
    }
    let service = crate::state::state(cx);
    cx.spawn(async move |cx| {
        let released = cx
            .background_executor()
            .spawn(async move { service.release_screen(generation) })
            .await;
        cx.update(|cx| {
            if session(cx).pending_freeze != Some(generation) {
                return;
            }
            if released {
                finish_pending_freeze_release(generation, true, cx);
                return;
            }
            crate::windows::toast::Toast::show(
                cx,
                "Screen still frozen",
                "The frozen screen could not be released. The next capture will retry before opening.",
            );
        });
    })
    .detach();
}

pub(crate) fn clear_pending_freeze(generation: u64, owner: u64, cx: &mut App) {
    if session(cx).revision == owner && session(cx).pending_freeze == Some(generation) {
        session(cx).pending_freeze = None;
        crate::system::memory::schedule_idle_reclaim(cx);
    }
}

pub(crate) fn watch_timer(
    revision: u64,
    cancel: smol::channel::Sender<bool>,
    cx: &mut App,
) -> bool {
    if session(cx).revision != revision {
        return false;
    }
    session(cx).timer_cancel = Some(cancel);
    true
}

pub(crate) fn close_timer(revision: u64, cx: &mut App) -> bool {
    if session(cx).revision != revision {
        return false;
    }
    session(cx).timer_cancel = None;
    close_all(cx);
    true
}

pub(crate) fn set_capture_hide(
    hide: Option<std::sync::Arc<crate::capture::desktop_icons::CaptureHide>>,
    cx: &mut App,
) {
    session(cx).capture_hide = hide;
}

fn take_capture_hide(
    cx: &mut App,
) -> Option<std::sync::Arc<crate::capture::desktop_icons::CaptureHide>> {
    session(cx).capture_hide.take()
}

pub(crate) fn capture_hide(
    cx: &mut App,
) -> Option<std::sync::Weak<crate::capture::desktop_icons::CaptureHide>> {
    session(cx)
        .capture_hide
        .as_ref()
        .map(std::sync::Arc::downgrade)
}

pub(crate) fn release_capture_hide(
    expected: Option<std::sync::Weak<crate::capture::desktop_icons::CaptureHide>>,
    cx: &mut App,
) {
    let Some(expected) = expected else { return };
    if session(cx)
        .capture_hide
        .as_ref()
        .is_some_and(|current| expected.ptr_eq(&std::sync::Arc::downgrade(current)))
    {
        let hide = session(cx).capture_hide.take();
        crate::capture::desktop_icons::release_for_capture(hide, cx);
    }
}

pub(crate) fn is_active(cx: &App) -> bool {
    cx.try_global::<OverlaySession>()
        .is_some_and(|session| !session.handles.is_empty())
}

#[cfg(any(all(test, windows), feature = "parity"))]
pub(crate) fn parity_is_active(cx: &mut App) -> bool {
    is_active(cx)
}

#[cfg(any(all(test, windows), feature = "parity"))]
pub(crate) fn parity_active_overlay(cx: &mut App) -> Option<AnyWindowHandle> {
    session(cx).handles.first().map(|overlay| overlay.handle)
}

#[cfg(all(target_os = "macos", feature = "parity", not(test)))]
pub(crate) fn parity_has_pending_freeze(cx: &mut App) -> bool {
    session(cx).pending_freeze.is_some()
}

#[cfg(feature = "parity")]
pub(crate) fn parity_selection(cx: &mut App) -> Option<selection::Rect> {
    let handle = parity_active_overlay(cx)?.downcast::<AreaOverlay>()?;
    handle.update(cx, |overlay, _, _| overlay.rect).ok()?
}

#[cfg(all(windows, feature = "parity"))]
pub(crate) fn parity_active_overlays(cx: &mut App) -> Vec<AnyWindowHandle> {
    session(cx)
        .handles
        .iter()
        .map(|overlay| overlay.handle)
        .collect()
}

#[cfg(any(all(test, windows), feature = "parity"))]
pub(crate) fn active_intents_for_test(cx: &mut App) -> Vec<crate::capture::intent::CaptureIntent> {
    session(cx)
        .handles
        .clone()
        .into_iter()
        .filter_map(|tracked| {
            tracked
                .handle
                .downcast::<AreaOverlay>()?
                .update(cx, |overlay, _, _| overlay.intent)
                .ok()
        })
        .collect()
}

pub fn close_all(cx: &mut App) {
    if let Some(generation) = remove_all(cx, true) {
        if generation != 0 {
            set_pending_freeze(generation, cx);
        }
        release_frozen_screen(generation, cx);
    }
}

pub(crate) fn frozen_selection_generation(cx: &mut App) -> u64 {
    session(cx)
        .handles
        .iter()
        .map(|tracked| tracked.generation)
        .max()
        .unwrap_or(0)
}

pub(crate) fn mark_frozen_selection_released(generation: u64, cx: &mut App) {
    for tracked in &mut session(cx).handles {
        if tracked.generation <= generation {
            tracked.generation = 0;
        }
    }
    finish_pending_freeze_release(generation, true, cx);
}

pub fn release_frozen_screen_in_place(cx: &mut App) -> bool {
    let generation = frozen_selection_generation(cx);
    if !crate::state::state(cx).release_screen(generation) {
        return false;
    }
    mark_frozen_selection_released(generation, cx);
    true
}

#[cfg(not(target_os = "macos"))]
pub fn begin_recording_handoff(rect: ScreenRect, cx: &mut App) {
    if session(cx).outlined_window_id.is_some() {
        return;
    }
    let handles = session(cx).handles.clone();
    for tracked in handles {
        let Some(handle) = tracked.handle.downcast::<AreaOverlay>() else {
            continue;
        };
        let _ = handle.update(cx, |overlay, window, cx| {
            if overlay.intent != crate::capture::intent::CaptureIntent::Recording {
                return;
            }
            overlay.rect = local_recording_rect(rect, overlay.display_bounds, overlay.scale);
            overlay.handed_off = true;
            overlay.interaction = None;
            overlay.pointer = None;
            set_overlay_hole(window, overlay.rect);
            cx.notify();
        });
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn prepare_recording_overlay(cx: &mut App) -> Option<smol::channel::Sender<()>> {
    let session = session(cx);
    if session.outlined_window_id.is_some() {
        return None;
    }
    session.native_recording_overlay_requested = true;
    Some(begin_recording_preparation(cx))
}

#[cfg(target_os = "macos")]
pub(crate) fn begin_recording_preparation(cx: &mut App) -> smol::channel::Sender<()> {
    let (finished, pending) = smol::channel::bounded(1);
    session(cx).pending_recording_preparation = Some(pending);
    finished
}

#[cfg(target_os = "macos")]
pub(crate) fn pending_recording_preparation(cx: &App) -> Option<smol::channel::Receiver<()>> {
    cx.try_global::<OverlaySession>()?
        .pending_recording_preparation
        .clone()
}

#[cfg(target_os = "macos")]
pub(crate) fn finish_recording_preparation(cx: &mut App) {
    session(cx).pending_recording_preparation = None;
}

#[cfg(target_os = "macos")]
pub(crate) fn recording_overlay_needs_hide(cx: &mut App) -> bool {
    session(cx).native_recording_overlay_requested
}

pub(crate) fn clear_recording_handoff(cx: &mut App) {
    session(cx).outlined_window_id = None;
    close_all(cx);
}

#[cfg(target_os = "macos")]
pub(crate) fn mark_recording_overlay_hidden(cx: &mut App) {
    session(cx).native_recording_overlay_requested = false;
}

pub fn end_recording_handoff(cx: &mut App) {
    #[cfg(all(not(target_os = "macos"), not(test)))]
    {
        let daemon = crate::state::state(cx).daemon;
        let _ = daemon.recording_overlay().hide();
    }
    clear_recording_handoff(cx);
    #[cfg(target_os = "macos")]
    crate::intents::hide_recording_outline(None, cx);
}

#[cfg(target_os = "macos")]
pub fn rollback_recording_handoff(cx: &mut App) {
    if session(cx).outlined_window_id.is_none() {
        end_recording_handoff(cx);
    }
}

pub(crate) fn show_window_recording_outline(window_id: i64, cx: &mut App) -> bool {
    if session(cx).outlined_window_id == Some(window_id) {
        return true;
    }

    #[cfg(target_os = "macos")]
    {
        if pending_recording_preparation(cx).is_some()
            || crate::intents::recording_cleanup_is_pending(cx)
        {
            return false;
        }
        session(cx).native_recording_overlay_requested = true;
    }
    #[cfg(test)]
    {
        session(cx).outlined_window_id = Some(window_id);
        true
    }
    #[cfg(not(test))]
    {
        let service = crate::state::state(cx);
        let appearance = service.config.get().appearance;
        let preset = crate::theme::presets::get_theme_preset(&appearance.theme);
        let color = match crate::theme::vars::active_mode(cx) {
            crate::theme::presets::ThemeMode::Light => preset.light.accent,
            _ => preset.dark.accent,
        };
        let request = RecordingOverlayShowWindowRequest {
            window_id,
            color: Some(color.to_string()),
            below_window_id: recording_control_window_id(cx),
        };
        #[cfg(target_os = "macos")]
        {
            let owner = revision(cx);
            let finished = begin_recording_preparation(cx);
            cx.spawn(async move |cx| {
                let visible = smol::unblock(move || service.daemon.recording_overlay().show_window(request))
                    .await
                    .unwrap_or(false);
                cx.update(|cx| {
                    finish_recording_preparation(cx);
                    if revision(cx) != owner {
                        crate::intents::hide_recording_outline(None, cx);
                    } else if visible {
                        session(cx).outlined_window_id = Some(window_id);
                    } else {
                        eprintln!("[recording-overlay] failed to outline window {window_id}");
                    }
                    crate::windows::recording_control::RecordingControl::refresh_after_outline_cleanup(cx);
                });
                drop(finished);
            })
            .detach();
            true
        }
        #[cfg(not(target_os = "macos"))]
        {
            let visible = service
                .daemon
                .recording_overlay()
                .show_window(request)
                .unwrap_or(false);
            if visible {
                session(cx).outlined_window_id = Some(window_id);
            }
            visible
        }
    }
}

#[cfg(all(windows, not(test)))]
fn recording_control_window_id(cx: &mut App) -> Option<i64> {
    for window_handle in cx.windows() {
        let Some(handle) =
            window_handle.downcast::<crate::windows::recording_control::RecordingControl>()
        else {
            continue;
        };
        let window_id = handle
            .update(cx, |_, window, _| {
                crate::windows::window_hwnd(window).map(|hwnd| hwnd.0 as isize as i64)
            })
            .ok()
            .flatten();
        if window_id.is_some() {
            return window_id;
        }
    }
    None
}

#[cfg(all(not(windows), not(test)))]
fn recording_control_window_id(_cx: &mut App) -> Option<i64> {
    None
}

pub fn replace_all(cx: &mut App) -> Option<u64> {
    remove_all(cx, false)
}

fn remove_all(cx: &mut App, deferred: bool) -> Option<u64> {
    let (handles, generation) = take_all(cx);
    if deferred {
        App::defer(cx, move |cx| dispose_overlays(handles, cx));
    } else {
        dispose_overlays(handles, cx);
    }
    generation
}

fn take_all(cx: &mut App) -> (Vec<TrackedOverlay>, Option<u64>) {
    session(cx).revision = session(cx).revision.wrapping_add(1);
    #[cfg(target_os = "macos")]
    {
        session(cx).timer_release_owner = None;
    }
    if let Some(cancel) = session(cx).timer_cancel.take() {
        let _ = cancel.try_send(false);
    }
    let handles = std::mem::take(&mut session(cx).handles);
    let hide = take_capture_hide(cx);
    crate::capture::desktop_icons::release_for_capture(hide, cx);
    let pending = session(cx).pending_freeze;
    let generation = handles
        .iter()
        .map(|tracked| tracked.generation)
        .chain(pending)
        .max();
    if let Some(generation) = generation {
        set_pending_freeze(generation, cx);
    }
    (handles, generation)
}

fn dispose_overlays(handles: Vec<TrackedOverlay>, cx: &mut App) {
    #[cfg(any(windows, target_os = "macos"))]
    let mut parked = false;
    for tracked in handles {
        #[cfg(windows)]
        if let (Some(display_id), Some(handle)) =
            (tracked.display_id, tracked.handle.downcast::<AreaOverlay>())
        {
            if !session(cx)
                .pooled
                .iter()
                .any(|entry| entry.display_id == display_id)
                && handle
                    .update(cx, |overlay, window, cx| {
                        let needs_clear = overlay.surface_state != PooledSurfaceState::Clean;
                        overlay.reset_for_park(window, cx);
                        cx.notify();
                        if !needs_clear {
                            park_overlay(window);
                            return;
                        }
                        overlay.surface_state = PooledSurfaceState::Dirty;
                        window.refresh();
                        if cloak_for_park(window) {
                            schedule_park_hide(
                                cx.entity().downgrade(),
                                overlay.park_generation,
                                window,
                            );
                        } else {
                            park_overlay(window);
                        }
                    })
                    .is_ok()
            {
                session(cx)
                    .pooled
                    .push(PooledOverlay { handle, display_id });
                parked = true;
                continue;
            }
        }
        #[cfg(target_os = "macos")]
        if let (Some(display_id), Some(handle)) =
            (tracked.display_id, tracked.handle.downcast::<AreaOverlay>())
        {
            if !session(cx)
                .pooled
                .iter()
                .any(|entry| entry.display_id == display_id)
                && handle
                    .update(cx, |overlay, window, cx| {
                        let origin = overlay.display_bounds.origin;
                        overlay.reset_for_park(window, cx);
                        cx.notify();
                        park_macos_overlay(window, origin, cx);
                    })
                    .is_ok()
            {
                session(cx)
                    .pooled
                    .push(PooledOverlay { handle, display_id });
                parked = true;
                continue;
            }
        }
        let _ = tracked.handle.update(cx, |_, window, _| {
            #[cfg(target_os = "macos")]
            crate::system::window_visibility::hide(window);
            window.remove_window();
        });
    }
    #[cfg(any(windows, target_os = "macos"))]
    if parked {
        crate::system::memory::schedule_idle_reclaim(cx);
    }
}

pub(crate) fn has_active_or_pending(cx: &App) -> bool {
    cx.try_global::<OverlaySession>()
        .is_some_and(|session| !session.handles.is_empty() || session.pending_freeze.is_some())
}

pub(crate) fn pooled_count(cx: &App) -> usize {
    #[cfg(any(windows, target_os = "macos"))]
    {
        let windows = cx.windows();
        cx.try_global::<OverlaySession>().map_or(0, |session| {
            session
                .pooled
                .iter()
                .filter(|entry| windows.contains(&entry.handle.into()))
                .count()
        })
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = cx;
        0
    }
}

#[cfg_attr(
    not(any(all(target_os = "macos", not(test)), all(windows, test))),
    allow(dead_code)
)]
pub(crate) fn evict_pooled(cx: &mut App) {
    #[cfg(any(windows, target_os = "macos"))]
    {
        if cx.try_global::<OverlaySession>().is_none() {
            return;
        }
        let pooled = std::mem::take(&mut cx.global_mut::<OverlaySession>().pooled);
        for entry in pooled {
            let _ = entry
                .handle
                .update(cx, |_, window, _| window.remove_window());
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = cx;
    }
}

#[cfg(any(windows, target_os = "macos"))]
pub fn raise_all(generation: u64, cx: &mut App) {
    let handles = session(cx).handles.clone();
    for tracked in handles {
        if tracked.generation != generation {
            continue;
        }
        #[cfg(windows)]
        {
            if let Some(handle) = tracked.handle.downcast::<AreaOverlay>() {
                let _ = handle.update(cx, |overlay, window, cx| {
                    overlay.raise_for_session(window, tracked.focus, cx);
                });
                continue;
            }
            let _ = tracked.handle.update(cx, |_, window, _| {
                set_overlay_topmost(window);
                if tracked.focus {
                    window.activate_window();
                }
            });
        }
        #[cfg(target_os = "macos")]
        {
            if let Some(handle) = tracked.handle.downcast::<AreaOverlay>() {
                let _ = handle.update(cx, |overlay, window, cx| {
                    if overlay.parked {
                        return;
                    }
                    crate::system::window_visibility::show_overlay(window, tracked.focus);
                    let position = window.mouse_position();
                    overlay.pointer_inside =
                        Bounds::new(gpui::point(px(0.), px(0.)), window.viewport_size())
                            .contains(&position);
                    if overlay.pointer_inside {
                        overlay.pointer = Some(position);
                        overlay.hovered_window = overlay.window_at(position);
                    }
                    if tracked.focus {
                        window.activate_window();
                    }
                    cx.notify();
                });
                continue;
            }
            let _ = tracked.handle.update(cx, |_, window, _| {
                crate::system::window_visibility::show_overlay(window, tracked.focus);
                if tracked.focus {
                    window.activate_window();
                }
            });
        }
    }
}

/// Drops the daemon's frozen snapshot. Cheap and idempotent when the screen was
/// never frozen, which is why the callers do not track whether it was.
pub(crate) fn release_frozen_screen(generation: u64, cx: &mut App) {
    if generation == 0 {
        return;
    }
    let service = crate::state::state(cx);
    cx.spawn(async move |cx| {
        let released = cx
            .background_executor()
            .spawn(async move { service.release_screen(generation) })
            .await;
        cx.update(|cx| finish_pending_freeze_release(generation, released, cx));
    })
    .detach();
}

pub(crate) fn dismiss(window: &mut Window, cx: &mut App) {
    if let Some(generation) = conceal_for_capture(window, cx) {
        if generation != 0 {
            set_pending_freeze(generation, cx);
        }
        release_frozen_screen(generation, cx);
    }
}

fn conceal_for_capture(window: &mut Window, cx: &mut App) -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        crate::system::window_visibility::hide(window);
        let current = window.window_handle();
        let others: Vec<_> = session(cx)
            .handles
            .iter()
            .map(|tracked| tracked.handle)
            .filter(|handle| *handle != current)
            .collect();
        for handle in others {
            let _ = handle.update(cx, |_, window, _| {
                crate::system::window_visibility::hide(window);
            });
        }
    }
    let generation = remove_all(cx, true);
    if !cfg!(any(windows, target_os = "macos")) || generation.is_none() {
        window.remove_window();
    }
    generation
}

#[cfg(target_os = "macos")]
fn conceal_for_scroll(window: &mut Window, owner: u64, cx: &mut App) -> Option<u64> {
    let (mut handles, generation) = take_all(cx);
    let current = window.window_handle();
    if !handles.iter().any(|tracked| tracked.handle == current) {
        handles.push(TrackedOverlay {
            handle: current,
            display_id: window.display(cx).map(|display| display.id()),
            generation: 0,
            focus: false,
        });
    }
    crate::system::window_visibility::hide(window);
    for tracked in &handles {
        if tracked.handle == current {
            continue;
        }
        let _ = tracked.handle.update(cx, |_, window, _| {
            crate::system::window_visibility::hide(window);
        });
    }
    session(cx).scroll_selectors.insert(owner, handles);
    generation
}

pub(crate) fn release_scroll_selectors(owner: Option<u64>, cx: &mut App) {
    #[cfg(target_os = "macos")]
    if let Some(handles) = owner.and_then(|owner| session(cx).scroll_selectors.remove(&owner)) {
        for tracked in handles {
            let _ = tracked.handle.update(cx, |_, window, _| {
                crate::system::window_visibility::hide(window);
                window.remove_window();
            });
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (owner, cx);
}

#[cfg(all(target_os = "macos", feature = "parity"))]
pub(crate) fn retained_scroll_selector_count(cx: &mut App) -> usize {
    session(cx).scroll_selectors.values().map(Vec::len).sum()
}

fn conceal_for_intent(
    window: &mut Window,
    intent: crate::capture::intent::CaptureIntent,
    owner: Option<u64>,
    cx: &mut App,
) -> Option<u64> {
    #[cfg(target_os = "macos")]
    if intent == crate::capture::intent::CaptureIntent::ScrollCapture {
        return conceal_for_scroll(window, owner.expect("scroll selector owner"), cx);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (intent, owner);
    conceal_for_capture(window, cx)
}

fn selection_owner(intent: crate::capture::intent::CaptureIntent, cx: &mut App) -> Option<u64> {
    (intent == crate::capture::intent::CaptureIntent::Timer
        || (cfg!(target_os = "macos")
            && intent == crate::capture::intent::CaptureIntent::ScrollCapture))
        .then(|| revision(cx))
}

#[cfg(all(target_os = "macos", feature = "parity", not(test)))]
pub(crate) fn native_scroll_selector_probe(cx: &mut App) {
    use cocoa::base::{id, BOOL, YES};
    use cocoa::foundation::NSPoint;
    use gpui::AppContext;
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    struct Probe;
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    let display_id = cx.primary_display().expect("primary display").id();
    let open = |cx: &mut App| {
        let bounds = Bounds::new(
            gpui::point(px(100.), px(100.)),
            gpui::size(px(32.), px(32.)),
        );
        let options = crate::windows::popup_window_options(
            bounds,
            crate::windows::PopupWindowConfig {
                show: false,
                focus: false,
                ..Default::default()
            },
        );
        let handle = cx.open_window(options, |_, cx| cx.new(|_| Probe)).unwrap();
        track_overlay(handle.into(), display_id, 0, false, cx);
        handle
    };
    let first = open(cx);
    let owner = revision(cx);
    let first_id = first
        .update(cx, |_, window, _| {
            crate::windows::macos_window_id(window).unwrap()
        })
        .unwrap();
    first
        .update(cx, |_, window, cx| {
            let RawWindowHandle::AppKit(handle) =
                HasWindowHandle::window_handle(window).unwrap().as_raw()
            else {
                panic!("expected AppKit handle");
            };
            let view = handle.ns_view.as_ptr() as id;
            let native: id = unsafe { msg_send![view, window] };
            unsafe {
                let _: () = msg_send![native, setFrameOrigin: NSPoint::new(-50000., -50000.)];
                let _: () = msg_send![native, orderFrontRegardless];
            }
            let visible_before: BOOL = unsafe { msg_send![native, isVisible] };
            let key: BOOL = unsafe { msg_send![native, isKeyWindow] };
            assert_eq!(visible_before, YES);
            assert_ne!(key, YES);
            conceal_for_scroll(window, owner, cx);
            assert_eq!(crate::windows::macos_window_id(window), Some(first_id));
            let visible: BOOL = unsafe { msg_send![native, isVisible] };
            assert_ne!(visible, YES);
        })
        .unwrap();
    crate::windows::scroll_capture::native_window_probe(cx, Some(first_id));
    assert_eq!(
        first
            .update(cx, |_, window, _| crate::windows::macos_window_id(window))
            .unwrap(),
        Some(first_id)
    );
    let second = open(cx);
    let second_id = second
        .update(cx, |_, window, _| {
            crate::windows::macos_window_id(window).unwrap()
        })
        .unwrap();
    assert_ne!(first_id, second_id);
    release_scroll_selectors(Some(owner), cx);
    assert!(!session(cx).scroll_selectors.contains_key(&owner));
    assert_eq!(
        second
            .update(cx, |_, window, _| crate::windows::macos_window_id(window))
            .unwrap(),
        Some(second_id)
    );
    remove_all(cx, false);
    println!("scroll selector lease: off-screen ordered window hidden without changing ID; releasing lease leaves newer selector intact");
}

#[cfg(feature = "parity")]
pub(crate) const PARITY_OVERLAY_WINDOW_IDS: [i64; 2] = [7101, 7102];

#[cfg(all(target_os = "macos", not(test)))]
fn capture_overlay_window_ids(window: &Window, cx: &mut App) -> Option<Vec<i64>> {
    use gpui::AppContext;

    #[cfg(feature = "parity")]
    if std::env::var_os(crate::ui::parity::daemon_fixture::PROCESS_ENV).is_some() {
        return Some(PARITY_OVERLAY_WINDOW_IDS.to_vec());
    }
    let current = window.window_handle();
    let mut ids = vec![crate::windows::macos_window_id(window)?];
    let handles: Vec<_> = session(cx)
        .handles
        .iter()
        .map(|tracked| tracked.handle)
        .collect();
    for handle in handles {
        if handle == current {
            continue;
        }
        let id = cx
            .update_window(handle, |_, window, _| {
                crate::windows::macos_window_id(window)
            })
            .ok()
            .flatten()?;
        ids.push(id);
    }
    Some(ids)
}

#[cfg(any(not(target_os = "macos"), test))]
fn capture_overlay_window_ids(_window: &Window, _cx: &mut App) -> Option<Vec<i64>> {
    Some(Vec::new())
}

#[cfg(any(not(target_os = "macos"), test))]
fn local_recording_rect(
    rect: ScreenRect,
    display_bounds: Bounds<Pixels>,
    scale: f32,
) -> Option<selection::Rect> {
    let scale = capture_coordinate_scale(scale);
    let display_x = f32::from(display_bounds.left()) * scale;
    let display_y = f32::from(display_bounds.top()) * scale;
    let display_width = f32::from(display_bounds.size.width) * scale;
    let display_height = f32::from(display_bounds.size.height) * scale;
    let center_x = rect.x as f32 + rect.width as f32 / 2.0;
    let center_y = rect.y as f32 + rect.height as f32 / 2.0;
    if center_x < display_x
        || center_x >= display_x + display_width
        || center_y < display_y
        || center_y >= display_y + display_height
    {
        return None;
    }
    Some(selection::Rect {
        x: (rect.x as f32 - display_x) / scale,
        y: (rect.y as f32 - display_y) / scale,
        width: rect.width as f32 / scale,
        height: rect.height as f32 / scale,
    })
}

#[cfg(not(target_os = "macos"))]
fn set_overlay_hole(window: &Window, hole: Option<selection::Rect>) {
    #[cfg(windows)]
    {
        use windows::Win32::Graphics::Gdi::{
            CombineRgn, CreateRectRgn, DeleteObject, SetWindowRgn, RGN_DIFF,
        };
        use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

        let Some(hwnd) = crate::windows::window_hwnd(window) else {
            return;
        };
        let mut client = windows::Win32::Foundation::RECT::default();
        if unsafe { GetClientRect(hwnd, &mut client) }.is_err() {
            return;
        }
        let region = unsafe { CreateRectRgn(0, 0, client.right, client.bottom) };
        if region.is_invalid() {
            return;
        }
        let Some(hole) = hole else {
            if unsafe { SetWindowRgn(hwnd, Some(region), true) } == 0 {
                unsafe {
                    let _ = DeleteObject(region.into());
                }
            }
            return;
        };
        let scale = window.scale_factor();
        let expand = scale.ceil() as i32;
        let left = ((hole.x * scale).round() as i32 - expand).max(0);
        let top = ((hole.y * scale).round() as i32 - expand).max(0);
        let right = (((hole.x + hole.width) * scale).round() as i32 + expand).min(client.right);
        let bottom = (((hole.y + hole.height) * scale).round() as i32 + expand).min(client.bottom);
        let hole_region = unsafe { CreateRectRgn(left, top, right, bottom) };
        if hole_region.is_invalid() {
            unsafe {
                let _ = DeleteObject(region.into());
            }
            return;
        }
        unsafe {
            let _ = CombineRgn(Some(region), Some(region), Some(hole_region), RGN_DIFF);
            let _ = DeleteObject(hole_region.into());
            if SetWindowRgn(hwnd, Some(region), true) == 0 {
                let _ = DeleteObject(region.into());
            }
        }
    }
    #[cfg(not(windows))]
    let _ = (window, hole);
}

pub fn display_scale_factor(display: &dyn gpui::PlatformDisplay, cx: &mut App) -> f32 {
    for handle in cx.windows() {
        let scale = handle
            .update(cx, |_, window, cx| {
                window
                    .display(cx)
                    .filter(|current| current.id() == display.id())
                    .map(|_| window.scale_factor())
            })
            .ok()
            .flatten();
        if let Some(scale) = scale {
            return scale;
        }
    }
    crate::system::work_area::capture_scale_factor(display, cx)
}

pub fn physical_rect(bounds: Bounds<Pixels>, scale: f32) -> ScreenRect {
    let scale = capture_coordinate_scale(scale);
    ScreenRect {
        x: (f32::from(bounds.origin.x) * scale).round() as i32,
        y: (f32::from(bounds.origin.y) * scale).round() as i32,
        width: (f32::from(bounds.size.width) * scale).round() as i32,
        height: (f32::from(bounds.size.height) * scale).round() as i32,
    }
}

fn physical_selection_rect(bounds: Bounds<Pixels>, scale: f32) -> ScreenRect {
    let mut rect = physical_rect(bounds, scale);
    let scale = capture_coordinate_scale(scale);
    let right = (f32::from(bounds.right()) * scale).round() as i32;
    let bottom = (f32::from(bounds.bottom()) * scale).round() as i32;
    rect.width = right.saturating_sub(rect.x);
    rect.height = bottom.saturating_sub(rect.y);
    rect
}

pub fn display_capture(
    bounds: Bounds<Pixels>,
    scale: f32,
    display_id: Option<u32>,
) -> crate::capture::DisplayCapture {
    let rect = physical_rect(bounds, scale);
    crate::capture::DisplayCapture::new(
        rect,
        f64::from(scale),
        DisplayOrigin {
            x: rect.x,
            y: rect.y,
        },
        display_id,
    )
}

pub(crate) fn capture_coordinate_scale(scale: f32) -> f32 {
    if cfg!(target_os = "macos") {
        1.0
    } else {
        scale.max(0.01)
    }
}

fn overlay_options(
    display_bounds: Bounds<Pixels>,
    display_id: DisplayId,
    focus: bool,
    show: bool,
) -> WindowOptions {
    #[cfg(target_os = "macos")]
    let window_bounds = Bounds {
        origin: gpui::point(px(0.0), px(0.0)),
        size: display_bounds.size,
    };
    #[cfg(not(target_os = "macos"))]
    let window_bounds = display_bounds;
    #[allow(unused_mut)]
    let mut options = crate::windows::popup_window_options(
        window_bounds,
        crate::windows::PopupWindowConfig {
            focus: focus && show,
            show: !cfg!(windows) && show,
            display_id: Some(display_id),
            ..Default::default()
        },
    );
    #[cfg(target_os = "linux")]
    if crate::system::linux_session::current()
        == crate::system::linux_session::LinuxSession::Wayland
    {
        options.window_bounds = Some(gpui::WindowBounds::Maximized(window_bounds));
    }
    options
}

#[cfg(windows)]
fn show_noactivate_topmost(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SW_SHOWNOACTIVATE,
    };

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

#[cfg(all(windows, not(test)))]
fn set_overlay_cloaked(window: &Window, cloaked: bool) -> bool {
    let Some(hwnd) = crate::windows::window_hwnd(window) else {
        return false;
    };
    crate::system::window_composition::set_cloaked(hwnd, cloaked)
}

#[cfg(all(windows, test))]
fn set_overlay_cloaked(_window: &Window, _cloaked: bool) -> bool {
    true
}

#[cfg(all(windows, not(test)))]
fn cloak_for_park(window: &Window) -> bool {
    use windows::Win32::Graphics::Dwm::DwmFlush;
    use windows::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject, SetWindowRgn};
    use windows::Win32::UI::WindowsAndMessaging::{
        IsWindowVisible, SetWindowPos, HWND_BOTTOM, HWND_NOTOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE,
    };

    let Some(hwnd) = crate::windows::window_hwnd(window) else {
        return false;
    };
    if !unsafe { IsWindowVisible(hwnd).as_bool() } {
        return false;
    }
    if !crate::system::window_composition::set_cloaked(hwnd, true) {
        return false;
    }
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_NOTOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_BOTTOM),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let tiny = CreateRectRgn(0, 0, 1, 1);
        if !tiny.is_invalid() && SetWindowRgn(hwnd, Some(tiny), false) == 0 {
            let _ = DeleteObject(tiny.into());
        }
        if windows_build() < 19041 {
            let _ = DwmFlush();
        }
    }
    true
}

#[cfg(all(windows, test))]
fn cloak_for_park(_window: &Window) -> bool {
    true
}

#[cfg(windows)]
fn after_next_present(window: &Window, callback: impl FnOnce(&mut Window, &mut App) + 'static) {
    window.on_next_frame(move |window, _| {
        window.on_next_frame(callback);
    });
}

#[cfg(windows)]
fn schedule_park_hide(
    view: gpui::WeakEntity<AreaOverlay>,
    park_generation: u64,
    window: &mut Window,
) {
    after_next_present(window, move |window, cx| {
        let Some(view) = view.upgrade() else {
            return;
        };
        view.update(cx, |overlay, _| {
            if !overlay.parked || overlay.park_generation != park_generation {
                return;
            }
            overlay.surface_state = PooledSurfaceState::Clean;
            park_overlay(window);
        });
    });
}

#[cfg(all(windows, not(test)))]
fn park_overlay(window: &Window) {
    use windows::Win32::Graphics::Gdi::SetWindowRgn;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOREDRAW, SWP_NOZORDER, SW_HIDE,
    };

    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = SetWindowRgn(hwnd, None, false);
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                1,
                1,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW,
            );
        }
        crate::system::window_composition::set_cloaked(hwnd, false);
    }
}

#[cfg(all(windows, test))]
fn park_overlay(_window: &Window) {}

#[cfg(all(windows, not(test)))]
fn hide_overlay_without_shrinking(window: &Window) {
    use windows::Win32::Graphics::Gdi::SetWindowRgn;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};

    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = SetWindowRgn(hwnd, None, false);
        }
        crate::system::window_composition::set_cloaked(hwnd, false);
    }
}

#[cfg(all(windows, test))]
fn hide_overlay_without_shrinking(_window: &Window) {}

#[cfg(target_os = "macos")]
fn park_macos_overlay(window: &mut Window, origin: Point<Pixels>, cx: &mut App) {
    crate::system::window_visibility::hide(window);
    #[cfg(not(test))]
    {
        let parked_bounds = Bounds::new(origin, gpui::size(px(1.0), px(1.0)));
        if crate::windows::set_macos_window_bounds_with_display(window, parked_bounds, false) {
            window.bounds_changed(cx);
        }
    }
    #[cfg(test)]
    let _ = (origin, cx);
}

fn set_overlay_topmost(window: &Window) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
        };

        if let Some(hwnd) = crate::windows::window_hwnd(window) {
            unsafe {
                let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
            }
            show_noactivate_topmost(hwnd);
        }
    }
    #[cfg(not(windows))]
    let _ = window;
}

#[cfg(windows)]
pub(crate) fn windows_build() -> u32 {
    use std::sync::OnceLock;

    use windows::Wdk::System::SystemServices::RtlGetVersion;
    use windows::Win32::System::SystemInformation::OSVERSIONINFOW;

    static BUILD: OnceLock<u32> = OnceLock::new();
    *BUILD.get_or_init(|| {
        let mut version = OSVERSIONINFOW {
            dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        if unsafe { RtlGetVersion(&mut version) }.0 < 0 {
            return 0;
        }
        version.dwBuildNumber
    })
}

fn track_overlay(
    handle: AnyWindowHandle,
    display_id: DisplayId,
    generation: u64,
    focus: bool,
    cx: &mut App,
) {
    #[cfg(not(any(windows, target_os = "macos")))]
    let _ = (display_id, focus);
    session(cx).handles.push(TrackedOverlay {
        handle,
        #[cfg(any(windows, target_os = "macos"))]
        display_id: Some(display_id),
        generation,
        #[cfg(any(windows, target_os = "macos"))]
        focus,
    });
}

#[cfg(any(windows, target_os = "macos"))]
fn take_pooled_overlay(
    display_id: DisplayId,
    cx: &mut App,
) -> Option<gpui::WindowHandle<AreaOverlay>> {
    let pooled = &mut session(cx).pooled;
    let index = pooled
        .iter()
        .position(|overlay| overlay.display_id == display_id)?;
    Some(pooled.swap_remove(index).handle)
}

fn prepare_overlay_window(window: &mut Window, display_bounds: Bounds<Pixels>) {
    window.set_window_title("Poratake capture overlay");
    crate::windows::remove_popup_frame(window);
    #[cfg(all(target_os = "macos", not(test)))]
    crate::system::window_visibility::disable_macos_transitions(window);
    #[cfg(all(windows, not(test)))]
    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        crate::system::window_composition::configure_transparent_surface(hwnd);
        crate::system::window_composition::apply_window_bounds(
            hwnd,
            display_bounds,
            window.scale_factor(),
        );
    }
    #[cfg(any(not(windows), test))]
    let _ = (window, display_bounds);
}

#[cfg(target_os = "linux")]
fn align_fallback_overlay_with_work_area(
    handle: gpui::WindowHandle<AreaOverlay>,
    display_id: DisplayId,
    cx: &mut App,
) {
    let Some(display) = cx.find_display(display_id) else {
        return;
    };
    let work = crate::system::work_area::work_area(display.as_ref());
    let _ = handle.update(cx, |overlay, _, cx| {
        overlay.display_bounds = work;
        cx.notify();
    });
}

fn reveal_overlay_window(window: &Window, launch: OverlayLaunch, active: bool) -> bool {
    if !active || launch.deferred_show {
        return false;
    }
    set_overlay_topmost(window);
    true
}

fn open_overlay_window(
    display_id: DisplayId,
    display_bounds: Bounds<Pixels>,
    launch: OverlayLaunch,
    build: impl FnOnce(f32, gpui::FocusHandle) -> AreaOverlay,
    after_new: impl FnOnce(&mut AreaOverlay, &mut Context<AreaOverlay>),
    active: bool,
    error: &'static str,
    cx: &mut App,
) -> Option<gpui::WindowHandle<AreaOverlay>> {
    #[cfg(any(windows, target_os = "macos"))]
    if active {
        if let Some(handle) = take_pooled_overlay(display_id, cx) {
            let updated = handle.update(cx, |overlay, window, cx| {
                #[cfg(windows)]
                {
                    if launch.deferred_show {
                        hide_overlay_without_shrinking(window);
                    }
                    prepare_overlay_window(window, display_bounds);
                    if !launch.deferred_show {
                        set_overlay_hole(window, None);
                    }
                }
                #[cfg(target_os = "macos")]
                {
                    prepare_overlay_window(window, display_bounds);
                    #[cfg(not(test))]
                    if crate::windows::set_macos_window_bounds(window, display_bounds) {
                        window.bounds_changed(cx);
                    }
                }
                let scale = window.scale_factor();
                let focus_handle = overlay.focus_handle.clone();
                let window_list_generation = overlay.window_list_generation.wrapping_add(1);
                let color_frame_generation = overlay.color_frame_generation.wrapping_add(1);
                let park_generation = overlay.park_generation.wrapping_add(1);
                #[cfg(windows)]
                let surface_state = overlay.surface_state;
                let mut replacement = build(scale, focus_handle);
                replacement.display_id = Some(u64::from(display_id) as u32);
                replacement.window_list_generation = window_list_generation;
                replacement.color_frame_generation = color_frame_generation;
                replacement.park_generation = park_generation;
                #[cfg(windows)]
                {
                    replacement.surface_state = surface_state;
                }
                *overlay = replacement;
                #[cfg(all(target_os = "macos", not(test)))]
                {
                    let position = window.mouse_position();
                    overlay.pointer_inside =
                        Bounds::new(gpui::point(px(0.), px(0.)), window.viewport_size())
                            .contains(&position);
                    if overlay.pointer_inside {
                        overlay.pointer = Some(position);
                        overlay.hovered_window = overlay.window_at(position);
                    }
                }
                after_new(overlay, cx);
                cx.notify();
                window.refresh();
                #[cfg(windows)]
                overlay.reveal_for_session(window, launch, cx);
                #[cfg(target_os = "macos")]
                if !launch.deferred_show {
                    crate::system::window_visibility::show_overlay(window, launch.focus);
                    if launch.focus {
                        window.activate_window();
                    }
                }
                window.focus(&overlay.focus_handle, cx);
            });
            if let Err(failure) = updated {
                eprintln!("[overlay] failed to reuse {error}: {failure}");
                return None;
            }
            track_overlay(
                handle.into(),
                display_id,
                launch.generation,
                launch.focus,
                cx,
            );
            return Some(handle);
        }
    }

    let initial_bounds = if active {
        display_bounds
    } else {
        Bounds::new(display_bounds.origin, gpui::size(px(1.0), px(1.0)))
    };
    let show_immediately = active && !launch.deferred_show;
    let make_options =
        || overlay_options(initial_bounds, display_id, launch.focus, show_immediately);
    let build_overlay = |window: &mut Window, cx: &mut App| {
        prepare_overlay_window(window, initial_bounds);
        let scale = window.scale_factor();
        let focus_handle = cx.focus_handle();
        let view = cx.new(|_| {
            let mut view = build(scale, focus_handle);
            view.display_id = Some(u64::from(display_id) as u32);
            #[cfg(any(windows, target_os = "macos"))]
            {
                view.parked = !active;
            }
            view
        });
        view.update(cx, |overlay, cx| {
            cx.observe_window_activation(window, |overlay, window, _| {
                let active = window.is_window_active();
                if !active {
                    overlay.menu_activation_key = None;
                    overlay.menu_return_focus = false;
                }
                #[cfg(all(windows, not(test)))]
                if active && !overlay.parked {
                    prepare_overlay_window(window, overlay.display_bounds);
                }
            })
            .detach();
            after_new(overlay, cx);
        });
        let shown = reveal_overlay_window(window, launch, active);
        #[cfg(windows)]
        if shown {
            view.update(cx, |overlay, _| {
                overlay.surface_state = PooledSurfaceState::Live;
            });
        }
        if launch.focus && shown {
            window.activate_window();
        }
        let focus = view.read(cx).focus_handle.clone();
        window.focus(&focus, cx);
        view
    };
    #[cfg(target_os = "linux")]
    let opened = crate::windows::open_wayland_layer_or_fallback(
        cx,
        gpui::layer_shell::LayerShellOptions {
            namespace: "poratake-overlay".to_string(),
            layer: gpui::layer_shell::Layer::Overlay,
            anchor: gpui::layer_shell::Anchor::TOP
                | gpui::layer_shell::Anchor::BOTTOM
                | gpui::layer_shell::Anchor::LEFT
                | gpui::layer_shell::Anchor::RIGHT,
            exclusive_zone: Some(px(-1.0)),
            keyboard_interactivity: if launch.focus {
                gpui::layer_shell::KeyboardInteractivity::Exclusive
            } else {
                gpui::layer_shell::KeyboardInteractivity::OnDemand
            },
            ..Default::default()
        },
        make_options,
        build_overlay,
    );
    #[cfg(not(target_os = "linux"))]
    let opened = cx
        .open_window(make_options(), build_overlay)
        .map(|handle| (handle, false));
    match opened {
        Ok((handle, layer_shell)) => {
            #[cfg(target_os = "linux")]
            if !layer_shell
                && crate::system::linux_session::current()
                    == crate::system::linux_session::LinuxSession::Wayland
            {
                align_fallback_overlay_with_work_area(handle, display_id, cx);
            }
            #[cfg(not(target_os = "linux"))]
            let _ = layer_shell;
            #[cfg(all(target_os = "macos", not(test)))]
            let positioned = handle
                .update(cx, |overlay, window, cx| {
                    crate::windows::remove_popup_frame(window);
                    if crate::windows::set_macos_window_bounds(window, initial_bounds) {
                        window.bounds_changed(cx);
                        if active {
                            let position = window.mouse_position();
                            overlay.pointer_inside =
                                Bounds::new(gpui::point(px(0.), px(0.)), window.viewport_size())
                                    .contains(&position);
                            if overlay.pointer_inside {
                                overlay.pointer = Some(position);
                                overlay.hovered_window = overlay.window_at(position);
                            }
                            cx.notify();
                        }
                        return true;
                    }
                    #[cfg(feature = "parity")]
                    if std::env::var_os(crate::ui::parity::daemon_fixture::PROCESS_ENV).is_some() {
                        let scale = window.scale_factor();
                        window.resize(initial_bounds.size);
                        window.bounds_changed(cx);
                        window.set_scale_factor(scale);
                        return true;
                    }
                    false
                })
                .unwrap_or(false);
            #[cfg(all(target_os = "macos", not(test)))]
            if !positioned {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
                eprintln!("[overlay] failed to position {error}");
                return None;
            }
            if active {
                track_overlay(
                    handle.into(),
                    display_id,
                    launch.generation,
                    launch.focus,
                    cx,
                );
            }
            Some(handle)
        }
        Err(failure) => {
            eprintln!("[overlay] failed to open {error}: {failure}");
            None
        }
    }
}

#[cfg(any(windows, target_os = "macos"))]
pub fn prewarm(cx: &mut App) {
    if has_active_or_pending(cx) {
        return;
    }
    let Some(service) = crate::state::try_state(cx) else {
        return;
    };
    for display in cx.displays() {
        let display_id = display.id();
        if session(cx)
            .pooled
            .iter()
            .any(|entry| entry.display_id == display_id)
        {
            continue;
        }
        let display_bounds = crate::system::work_area::display_bounds(display.as_ref());
        let Some(handle) = open_overlay_window(
            display_id,
            display_bounds,
            OverlayLaunch {
                focus: false,
                deferred_show: true,
                generation: 0,
            },
            {
                let service = service.clone();
                move |scale, focus_handle| {
                    AreaOverlay::with_focus(
                        display_bounds,
                        scale,
                        service,
                        crate::capture::intent::CaptureIntent::Screenshot,
                        focus_handle,
                    )
                }
            },
            |_, _| {},
            false,
            "the prewarmed area overlay",
            cx,
        ) else {
            continue;
        };
        session(cx)
            .pooled
            .push(PooledOverlay { handle, display_id });
    }
}

fn begin_recording(
    cx: &mut App,
    target: crate::video::recorder::RecordingTarget,
    rect: ScreenRect,
    display_id: Option<u32>,
    window_id: Option<i64>,
    target_name: Option<String>,
    preference_scope: crate::windows::recording_control::PreferenceScope,
) {
    crate::windows::recording_control::RecordingControl::open(
        cx,
        target,
        rect,
        display_id,
        window_id,
        target_name,
        preference_scope,
    );
}

fn begin_recording_after_icon_restore(
    cx: &mut Context<AreaOverlay>,
    all_in_one: bool,
    target: crate::video::recorder::RecordingTarget,
    rect: ScreenRect,
    display_id: Option<u32>,
    window_id: Option<i64>,
    target_name: Option<String>,
    preference_scope: crate::windows::recording_control::PreferenceScope,
    after_open: impl FnOnce(&mut App) + 'static,
) {
    if !all_in_one {
        begin_recording(
            cx,
            target,
            rect,
            display_id,
            window_id,
            target_name,
            preference_scope,
        );
        after_open(cx);
        return;
    }
    let owner = revision(cx);
    let selected_overlay = cx.entity().downgrade();
    let restore_area = matches!(&target, crate::video::recorder::RecordingTarget::Area);
    App::defer(cx, move |cx| {
        if revision(cx) != owner || crate::capture::desktop_icons::is_shutting_down() {
            return;
        }
        for tracked in session(cx).handles.clone() {
            let Some(handle) = tracked.handle.downcast::<AreaOverlay>() else {
                continue;
            };
            let _ = handle.update(cx, |overlay, _, cx| {
                if overlay.intent != crate::capture::intent::CaptureIntent::Recording
                    || overlay.all_in_one.is_none()
                {
                    return;
                }
                overlay.all_in_one = None;
                overlay.handed_off = true;
                overlay.interaction = None;
                overlay.pointer = None;
                cx.notify();
            });
        }
        let Some(hide) = take_capture_hide(cx) else {
            begin_recording(
                cx,
                target,
                rect,
                display_id,
                window_id,
                target_name,
                preference_scope,
            );
            after_open(cx);
            if restore_area {
                let _ = selected_overlay.update(cx, |overlay, cx| {
                    overlay.handed_off = false;
                    cx.notify();
                });
            }
            return;
        };
        let transition = crate::capture::desktop_icons::track_transition();
        cx.spawn(async move |cx| {
            let worker_transition = transition.clone();
            let restored = smol::unblock(move || {
                let restored = crate::capture::desktop_icons::restore_after_capture_hide(hide);
                drop(worker_transition);
                restored
            })
            .await;
            cx.update(|cx| {
                if revision(cx) != owner || crate::capture::desktop_icons::is_shutting_down() {
                    return;
                }
                if !restored {
                    close_all(cx);
                    crate::windows::toast::Toast::show(
                        cx,
                        "Recording failed",
                        "Could not restore desktop icons",
                    );
                    return;
                }
                begin_recording(
                    cx,
                    target,
                    rect,
                    display_id,
                    window_id,
                    target_name,
                    preference_scope,
                );
                after_open(cx);
                if restore_area {
                    let _ = selected_overlay.update(cx, |overlay, cx| {
                        overlay.handed_off = false;
                        cx.notify();
                    });
                }
            });
            drop(transition);
        })
        .detach();
    });
}

pub struct AreaOverlay {
    pub focus_handle: gpui::FocusHandle,
    display_id: Option<u32>,
    display_bounds: Bounds<Pixels>,
    scale: f32,
    intent: crate::capture::intent::CaptureIntent,
    /// Non-empty in window-pick mode: hovering highlights a window and a
    /// click captures it, mirroring the Electron overlay's pick targets.
    windows: Vec<crate::capture::windows_list::WindowListItem>,
    picking_windows: bool,
    window_list_generation: u64,
    hovered_window: Option<usize>,
    /// Last pointer position in client coordinates, for `CrosshairGuides`.
    pointer: Option<Point<Pixels>>,
    #[cfg(target_os = "macos")]
    pointer_inside: bool,
    /// The live selection, which stays editable after it is drawn — the
    /// renderer keeps a box the user can move, resize and re-capture.
    rect: Option<selection::Rect>,
    interaction: Option<Interaction>,
    cursor: gpui::CursorStyle,
    screen_picker: bool,
    recording_preference_scope: crate::windows::recording_control::PreferenceScope,
    /// Set in all-in-one mode: the toolbar's current mode and target.
    all_in_one: Option<crate::capture::all_in_one::Choices>,
    all_in_one_targets: [crate::capture::all_in_one::Target; 2],
    picking_color: bool,
    color_frame: Option<std::sync::Arc<image::RgbaImage>>,
    color_frame_generation: u64,
    handed_off: bool,
    menu: crate::ui::menu::MenuHandle,
    menu_trigger_focus: Option<gpui::FocusHandle>,
    menu_activation_key: Option<&'static str>,
    menu_return_focus: bool,
    #[cfg(any(windows, target_os = "macos"))]
    parked: bool,
    #[cfg(any(windows, target_os = "macos"))]
    park_generation: u64,
    #[cfg(windows)]
    surface_state: PooledSurfaceState,
    service: CaptureService,
}

impl AreaOverlay {
    pub fn with_focus(
        display_bounds: Bounds<Pixels>,
        scale: f32,
        service: CaptureService,
        intent: crate::capture::intent::CaptureIntent,
        focus_handle: gpui::FocusHandle,
    ) -> Self {
        Self {
            focus_handle,
            display_id: None,
            display_bounds,
            scale,
            pointer: None,
            #[cfg(target_os = "macos")]
            pointer_inside: false,
            rect: None,
            interaction: None,
            cursor: gpui::CursorStyle::Crosshair,
            intent,
            windows: Vec::new(),
            picking_windows: false,
            window_list_generation: 0,
            hovered_window: None,
            screen_picker: false,
            recording_preference_scope: crate::windows::recording_control::PreferenceScope::Global,
            all_in_one: None,
            all_in_one_targets: [crate::capture::all_in_one::Target::Area; 2],
            picking_color: false,
            color_frame: None,
            color_frame_generation: 0,
            handed_off: false,
            menu: crate::ui::menu::MenuHandle::new(),
            menu_trigger_focus: None,
            menu_activation_key: None,
            menu_return_focus: false,
            #[cfg(any(windows, target_os = "macos"))]
            parked: false,
            #[cfg(any(windows, target_os = "macos"))]
            park_generation: 0,
            #[cfg(windows)]
            surface_state: PooledSurfaceState::Clean,
            service,
        }
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn reset_for_park(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.window_list_generation = self.window_list_generation.wrapping_add(1);
        self.picking_windows = false;
        self.windows.clear();
        self.hovered_window = None;
        self.color_frame_generation = self.color_frame_generation.wrapping_add(1);
        self.picking_color = false;
        self.color_frame = None;
        self.screen_picker = false;
        self.all_in_one = None;
        self.menu.close(window, cx);
        self.menu_trigger_focus = None;
        self.menu_activation_key = None;
        self.menu_return_focus = false;
        self.rect = None;
        self.handed_off = false;
        self.interaction = None;
        self.pointer = None;
        #[cfg(target_os = "macos")]
        {
            self.pointer_inside = false;
        }
        self.cursor = gpui::CursorStyle::Crosshair;
        self.parked = true;
        self.park_generation = self.park_generation.wrapping_add(1);
    }

    #[cfg(windows)]
    fn reveal_for_session(
        &mut self,
        window: &mut Window,
        launch: OverlayLaunch,
        cx: &mut Context<Self>,
    ) {
        if launch.deferred_show {
            return;
        }
        let clean = self.surface_state == PooledSurfaceState::Clean;
        let cloaked = !clean && set_overlay_cloaked(window, true);
        if !cloaked {
            set_overlay_cloaked(window, false);
        }
        let shown = reveal_overlay_window(window, launch, true);
        if !shown {
            self.surface_state = if clean {
                PooledSurfaceState::Clean
            } else {
                PooledSurfaceState::Dirty
            };
            return;
        }
        if clean || !cloaked {
            self.surface_state = PooledSurfaceState::Live;
            if launch.focus {
                window.activate_window();
            }
            return;
        }
        self.surface_state = PooledSurfaceState::AwaitingFirstPaint;
        self.schedule_uncloak_after_paint(window, launch.focus, cx);
    }

    #[cfg(windows)]
    fn raise_for_session(&mut self, window: &mut Window, focus: bool, cx: &mut Context<Self>) {
        if self.parked {
            return;
        }
        match self.surface_state {
            PooledSurfaceState::Clean => {
                set_overlay_cloaked(window, false);
                set_overlay_topmost(window);
                self.surface_state = PooledSurfaceState::Live;
                if focus {
                    window.activate_window();
                }
            }
            PooledSurfaceState::Dirty => {
                let cloaked = set_overlay_cloaked(window, true);
                set_overlay_topmost(window);
                if cloaked {
                    self.surface_state = PooledSurfaceState::AwaitingFirstPaint;
                    window.refresh();
                    self.schedule_uncloak_after_paint(window, focus, cx);
                } else {
                    self.surface_state = PooledSurfaceState::Live;
                    if focus {
                        window.activate_window();
                    }
                }
            }
            PooledSurfaceState::AwaitingFirstPaint => {
                set_overlay_topmost(window);
            }
            PooledSurfaceState::Live => {
                set_overlay_cloaked(window, false);
                set_overlay_topmost(window);
                if focus {
                    window.activate_window();
                }
            }
        }
    }

    #[cfg(windows)]
    fn schedule_uncloak_after_paint(
        &mut self,
        window: &mut Window,
        focus: bool,
        cx: &mut Context<Self>,
    ) {
        let park_generation = self.park_generation;
        let view = cx.entity().downgrade();
        after_next_present(window, move |window, cx| {
            let Some(view) = view.upgrade() else {
                return;
            };
            view.update(cx, |overlay, _| {
                if overlay.parked
                    || overlay.park_generation != park_generation
                    || overlay.surface_state != PooledSurfaceState::AwaitingFirstPaint
                {
                    return;
                }
                overlay.surface_state = PooledSurfaceState::Live;
                set_overlay_cloaked(window, false);
                if focus {
                    window.activate_window();
                }
            });
        });
    }

    pub fn with_windows(
        mut self,
        windows: Vec<crate::capture::windows_list::WindowListItem>,
    ) -> Self {
        self.windows = windows;
        self.picking_windows = true;
        self
    }

    pub(crate) fn apply_window_list(
        &mut self,
        request_generation: u64,
        windows: Vec<crate::capture::windows_list::WindowListItem>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.window_list_generation != request_generation
            || !self.picking_windows
            || self.all_in_one.is_some()
        {
            return false;
        }
        self.windows = windows;
        self.hovered_window = None;
        #[cfg(target_os = "macos")]
        if self.pointer_inside {
            self.hovered_window = self.pointer.and_then(|pointer| self.window_at(pointer));
        }
        cx.notify();
        true
    }

    pub(crate) fn window_list_generation(&self) -> u64 {
        self.window_list_generation
    }

    #[cfg(any(not(test), windows))]
    fn accepts_color_frame(&self, generation: u64) -> bool {
        self.picking_color && self.color_frame_generation == generation
    }

    #[cfg(feature = "parity")]
    pub(crate) fn parity_picking_color(&self) -> bool {
        self.picking_color
    }

    #[cfg(feature = "parity")]
    pub(crate) fn parity_all_in_one_state(
        &self,
        cx: &App,
    ) -> (Option<crate::capture::all_in_one::Choices>, bool) {
        (self.all_in_one, self.menu.is_open(cx))
    }

    pub fn with_all_in_one(mut self, choices: crate::capture::all_in_one::Choices) -> Self {
        use crate::capture::all_in_one::{Mode, Target};

        self.recording_preference_scope =
            crate::windows::recording_control::PreferenceScope::AllInOne;
        let config = self.service.config.get();
        if config.all_in_one.remember_choices {
            self.all_in_one_targets = [
                Target::parse(&config.all_in_one.last_targets.screenshot),
                Target::parse(&config.all_in_one.last_targets.record),
            ];
        }
        match choices.mode {
            Mode::Screenshot => self.all_in_one_targets[0] = choices.target,
            Mode::Record => self.all_in_one_targets[1] = choices.target,
            Mode::Ocr => {}
        }
        self.intent = match choices.mode {
            Mode::Screenshot => crate::capture::intent::CaptureIntent::Screenshot,
            Mode::Record => crate::capture::intent::CaptureIntent::Recording,
            Mode::Ocr => crate::capture::intent::CaptureIntent::Ocr,
        };
        self.all_in_one = Some(choices);
        self
    }

    pub fn with_screen_picker(mut self) -> Self {
        self.screen_picker = true;
        self.cursor = gpui::CursorStyle::default();
        self
    }

    #[cfg(all(feature = "parity", target_os = "macos", not(test)))]
    pub(crate) fn parity_pointer_inside(&self) -> bool {
        self.pointer_inside
    }

    fn is_picking_windows(&self) -> bool {
        self.picking_windows
    }

    fn is_picking_screen(&self) -> bool {
        self.screen_picker
            || self
                .all_in_one
                .is_some_and(|choices| choices.target == crate::capture::all_in_one::Target::Screen)
    }

    fn screen_picker_dimmed(&self, window_hovered: bool) -> Option<bool> {
        self.is_picking_screen().then_some(!window_hovered)
    }

    /// The hovered window's frame, converted from physical screen pixels into
    /// this overlay's logical client coordinates.
    fn hovered_frame(&self) -> Option<(Point<Pixels>, gpui::Size<Pixels>)> {
        let window = self.windows.get(self.hovered_window?)?;
        let scale = capture_coordinate_scale(self.scale);
        let left = px(window.bounds.x as f32 / scale) - self.display_bounds.left();
        let top = px(window.bounds.y as f32 / scale) - self.display_bounds.top();
        Some((
            point(left, top),
            size(
                px(window.bounds.width as f32 / scale),
                px(window.bounds.height as f32 / scale),
            ),
        ))
    }

    fn window_at(&self, position: Point<Pixels>) -> Option<usize> {
        let scale = capture_coordinate_scale(self.scale);
        let x = f32::from(self.display_bounds.left() + position.x) * scale;
        let y = f32::from(self.display_bounds.top() + position.y) * scale;
        crate::capture::windows_list::hit_test(&self.windows, x as f64, y as f64).and_then(
            |picked| {
                self.windows
                    .iter()
                    .position(|window| window.window_id == picked.window_id)
            },
        )
    }

    fn confirm_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.hovered_window else {
            return;
        };
        let Some(picked) = self.windows.get(index).cloned() else {
            return;
        };
        if self.intent == crate::capture::intent::CaptureIntent::Recording {
            #[cfg(target_os = "macos")]
            {
                let frozen = frozen_selection_generation(cx);
                if frozen != 0 {
                    self.release_before_window_recording(frozen, picked, window, cx);
                    return;
                }
            }
            #[cfg(not(target_os = "macos"))]
            if !release_frozen_screen_in_place(cx) {
                crate::windows::toast::Toast::show(
                    cx,
                    "Recording failed",
                    "Could not release the frozen screen",
                );
                return;
            }
            self.handoff_window_recording(picked, window, cx);
            return;
        }
        let mut capture = display_capture(self.display_bounds, self.scale, self.display_id);
        capture.rect = CaptureRect {
            x: picked.bounds.x as i32,
            y: picked.bounds.y as i32,
            width: picked.bounds.width as i32,
            height: picked.bounds.height as i32,
        };
        let reservation = self
            .service
            .config
            .get()
            .screenshot
            .freeze_screen
            .then(|| self.service.reserve_cached_capture());
        let hide = take_capture_hide(cx);
        let release_after = conceal_for_capture(window, cx);
        let coordinator = crate::state::coordinator(cx);
        coordinator.update(cx, |coordinator, cx| {
            coordinator.capture_window(
                picked.window_id,
                capture,
                reservation,
                release_after,
                hide,
                cx,
            );
        });
    }

    #[cfg(target_os = "macos")]
    fn release_before_window_recording(
        &self,
        frozen: u64,
        picked: crate::capture::windows_list::WindowListItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = revision(cx);
        let list_generation = self.window_list_generation;
        let handle = window
            .window_handle()
            .downcast::<Self>()
            .expect("area overlay");
        let finished = begin_recording_preparation(cx);
        let service = self.service.clone();
        cx.spawn(async move |_, cx| {
            #[cfg(not(test))]
            let released = smol::unblock(move || service.release_screen(frozen)).await;
            #[cfg(test)]
            let released = cx
                .background_executor()
                .spawn(async move { service.release_screen(frozen) })
                .await;
            cx.update(|cx| {
                finish_recording_preparation(cx);
                if revision(cx) != owner {
                    crate::windows::recording_control::RecordingControl::refresh_after_outline_cleanup(cx);
                    return;
                }
                if released {
                    mark_frozen_selection_released(frozen, cx);
                }
                let _ = handle.update(cx, |overlay, window, cx| {
                    if overlay.window_list_generation != list_generation
                        || overlay.intent != crate::capture::intent::CaptureIntent::Recording
                        || !overlay.is_picking_windows()
                    {
                        return;
                    }
                    if released {
                        overlay.handoff_window_recording(picked, window, cx);
                    } else {
                        crate::windows::toast::Toast::show(
                            cx,
                            "Recording failed",
                            "Could not release the frozen screen",
                        );
                    }
                });
            });
            drop(finished);
        })
        .detach();
    }

    fn handoff_window_recording(
        &mut self,
        picked: crate::capture::windows_list::WindowListItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let all_in_one = self.all_in_one.is_some();
        let rect = ScreenRect {
            x: picked.bounds.x as i32,
            y: picked.bounds.y as i32,
            width: picked.bounds.width as i32,
            height: picked.bounds.height as i32,
        };
        let label = picked.label();
        let window_id = picked.window_id;
        self.all_in_one = None;
        self.picking_windows = false;
        self.hovered_window = None;
        self.interaction = None;
        self.pointer = None;
        self.handed_off = true;
        conceal_recording_selector(window);
        sync_window_recording_handoff(cx);
        begin_recording_after_icon_restore(
            cx,
            all_in_one,
            crate::video::recorder::RecordingTarget::Window,
            rect,
            self.display_id,
            Some(window_id),
            Some(label),
            self.recording_preference_scope,
            move |cx| {
                for window_handle in cx.windows() {
                    let Some(handle) = window_handle
                        .downcast::<crate::windows::recording_control::RecordingControl>(
                    ) else {
                        continue;
                    };
                    let _ = handle.update(cx, |_, window, _| window.activate_window());
                    break;
                }
                if !show_window_recording_outline(window_id, cx) {
                    eprintln!("[recording-overlay] failed to outline window {window_id}");
                }
            },
        );
        cx.notify();
    }

    /// Opens the overlay covering one display.
    pub fn open(
        service: CaptureService,
        display_id: DisplayId,
        display_bounds: Bounds<Pixels>,
        intent: crate::capture::intent::CaptureIntent,
        launch: OverlayLaunch,
        cx: &mut App,
    ) -> Option<gpui::WindowHandle<AreaOverlay>> {
        open_overlay_window(
            display_id,
            display_bounds,
            launch,
            |scale, focus_handle| {
                AreaOverlay::with_focus(display_bounds, scale, service, intent, focus_handle)
            },
            |_, _| {},
            true,
            "the area overlay",
            cx,
        )
    }

    /// All-in-one variant of `open`: the overlay carries the toolbar and
    /// routes the confirmed selection through the picked mode.
    pub fn open_all_in_one(
        service: CaptureService,
        display_id: DisplayId,
        display_bounds: Bounds<Pixels>,
        choices: crate::capture::all_in_one::Choices,
        launch: OverlayLaunch,
        cx: &mut App,
    ) -> Option<gpui::WindowHandle<AreaOverlay>> {
        let prefetch_window_list = launch.focus;
        open_overlay_window(
            display_id,
            display_bounds,
            launch,
            |scale, focus_handle| {
                AreaOverlay::with_focus(
                    display_bounds,
                    scale,
                    service,
                    crate::capture::intent::CaptureIntent::Screenshot,
                    focus_handle,
                )
                .with_all_in_one(choices)
            },
            move |this, cx| {
                this.apply_all_in_one_target(cx);
                if prefetch_window_list
                    && !this.picking_windows
                    && this.all_in_one.is_some_and(|choices| {
                        choices.mode != crate::capture::all_in_one::Mode::Ocr
                    })
                {
                    this.request_window_list(cx);
                }
            },
            true,
            "the all-in-one overlay",
            cx,
        )
    }

    pub fn open_screen_picker(
        service: CaptureService,
        display_id: DisplayId,
        display_bounds: Bounds<Pixels>,
        intent: crate::capture::intent::CaptureIntent,
        launch: OverlayLaunch,
        cx: &mut App,
    ) -> Option<gpui::WindowHandle<AreaOverlay>> {
        open_overlay_window(
            display_id,
            display_bounds,
            launch,
            |scale, focus_handle| {
                AreaOverlay::with_focus(display_bounds, scale, service, intent, focus_handle)
                    .with_screen_picker()
            },
            |_, _| {},
            true,
            "the display picker",
            cx,
        )
    }

    pub fn set_all_in_one_mode(
        &mut self,
        mode: crate::capture::all_in_one::Mode,
        cx: &mut Context<Self>,
    ) {
        use crate::capture::all_in_one::Mode as AioMode;
        self.stop_color_picker(cx);
        let Some(choices) = &mut self.all_in_one else {
            return;
        };
        choices.mode = mode;
        choices.target = match mode {
            AioMode::Screenshot => self.all_in_one_targets[0],
            AioMode::Record => self.all_in_one_targets[1],
            AioMode::Ocr => crate::capture::all_in_one::Target::Area,
        };
        let choices = *choices;
        self.intent = capture_intent(mode);
        crate::capture::all_in_one::remember(&self.service.config, choices);
        self.apply_all_in_one_target(cx);
        sync_all_in_one(choices, self.all_in_one_targets, cx);
        cx.notify();
    }

    pub fn close_all_in_one_menu(&self, window: &mut Window, cx: &App) {
        self.menu.close(window, cx);
    }

    pub fn set_all_in_one_menu_focus(&mut self, focus: Option<gpui::FocusHandle>) {
        self.menu_trigger_focus = focus;
    }

    pub fn finish_all_in_one_target_selection(&mut self, window: &mut Window, cx: &mut App) {
        if self.menu_activation_key.is_some() {
            self.menu_return_focus = true;
            window.focus(&self.focus_handle, cx);
            return;
        }
        self.focus_all_in_one_menu_trigger(window, cx);
    }

    fn focus_all_in_one_menu_trigger(&self, window: &mut Window, cx: &mut App) {
        let handle = self
            .menu_trigger_focus
            .clone()
            .unwrap_or_else(|| self.focus_handle.clone());
        window.defer(cx, move |window, cx| {
            window.focus(&handle, cx);
        });
    }

    pub fn set_all_in_one_target(
        &mut self,
        target: crate::capture::all_in_one::Target,
        cx: &mut Context<Self>,
    ) {
        self.stop_color_picker(cx);
        let Some(choices) = &mut self.all_in_one else {
            return;
        };
        if choices.mode == crate::capture::all_in_one::Mode::Ocr {
            return;
        }
        choices.target = target;
        match choices.mode {
            crate::capture::all_in_one::Mode::Screenshot => self.all_in_one_targets[0] = target,
            crate::capture::all_in_one::Mode::Record => self.all_in_one_targets[1] = target,
            crate::capture::all_in_one::Mode::Ocr => {}
        }
        let choices = *choices;
        crate::capture::all_in_one::remember(&self.service.config, choices);
        self.apply_all_in_one_target(cx);
        sync_all_in_one(choices, self.all_in_one_targets, cx);
        cx.notify();
    }

    fn apply_all_in_one_target(&mut self, cx: &mut Context<Self>) {
        use crate::capture::all_in_one::Target;

        let Some(choices) = self.all_in_one else {
            return;
        };
        self.rect = None;
        self.interaction = None;
        self.cursor = if choices.target == Target::Screen {
            gpui::CursorStyle::default()
        } else {
            gpui::CursorStyle::Crosshair
        };
        self.hovered_window = None;
        self.picking_windows = choices.target == Target::Window;
        if self.picking_windows {
            self.request_window_list(cx);
        }
        cx.notify();
    }

    fn request_window_list(&mut self, cx: &mut Context<Self>) {
        self.window_list_generation = self.window_list_generation.wrapping_add(1);
        let request_generation = self.window_list_generation;
        let daemon = self.service.daemon.clone();
        cx.spawn(async move |entity, cx| {
            let windows = cx
                .background_executor()
                .spawn(async move { crate::capture::windows_list::list(&daemon) })
                .await;
            let _ = entity.update(cx, |this, cx| {
                if !accepts_window_list(
                    this.all_in_one,
                    this.window_list_generation,
                    request_generation,
                ) {
                    return;
                }
                this.windows = windows;
                cx.notify();
            });
        })
        .detach();
    }

    pub fn start_color_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu.close(window, cx);
        window.focus(&self.focus_handle, cx);
        let Some(excluded_window_ids) = capture_overlay_window_ids(window, cx) else {
            crate::windows::toast::Toast::show(
                cx,
                "Pick failed",
                "Could not exclude the capture selector",
            );
            return;
        };
        self.activate_color_picker(excluded_window_ids.clone(), cx);
        sync_color_picker(true, excluded_window_ids, cx);
    }

    fn stop_color_picker(&mut self, cx: &mut Context<Self>) {
        if !self.picking_color {
            return;
        }
        self.picking_color = false;
        self.color_frame = None;
        sync_color_picker(false, Vec::new(), cx);
        cx.notify();
    }

    fn activate_color_picker(&mut self, excluded_window_ids: Vec<i64>, cx: &mut Context<Self>) {
        self.picking_color = true;
        self.color_frame = None;
        self.color_frame_generation = self.color_frame_generation.wrapping_add(1);
        self.cursor = gpui::CursorStyle::Crosshair;
        #[cfg(not(test))]
        {
            let generation = self.color_frame_generation;
            let service = self.service.clone();
            let freeze = service.config.get().screenshot.freeze_screen;
            let reservation = freeze.then(|| service.reserve_cached_capture());
            let scale = self.scale;
            let capture = display_capture(self.display_bounds, scale, self.display_id);
            let rect = capture.rect;
            cx.spawn(async move |entity, cx| {
                let frame = cx
                    .background_executor()
                    .spawn(async move {
                        let path = std::env::temp_dir().join(format!(
                            "poratake-color-frame-{}-{}-{}-{}.png",
                            std::process::id(),
                            rect.x,
                            rect.y,
                            generation
                        ));
                        let frame = service
                            .capture_area_cached(
                                capture,
                                &path,
                                reservation,
                                None,
                                excluded_window_ids,
                            )
                            .ok()
                            .and_then(|_| crate::render::images::open(&path).ok())
                            .map(|image| std::sync::Arc::new(image.into_rgba8()));
                        let _ = std::fs::remove_file(&path);
                        frame
                    })
                    .await;
                let _ = entity.update(cx, |this, cx| {
                    if !this.accepts_color_frame(generation) {
                        return;
                    }
                    let Some(frame) = frame else {
                        this.stop_color_picker(cx);
                        crate::windows::toast::Toast::show(
                            cx,
                            "Pick failed",
                            "Could not read the display pixels",
                        );
                        return;
                    };
                    this.color_frame = Some(frame);
                    cx.notify();
                });
            })
            .detach();
        }
        #[cfg(test)]
        let _ = excluded_window_ids;
        cx.notify();
    }

    fn pick_color(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        let Some(frame) = &self.color_frame else {
            return false;
        };
        let (x, y) = crate::capture::color_picker::frame_point(frame, position, self.viewport());
        let pixel = frame.get_pixel(x, y).0;
        let hex = format!("#{:02x}{:02x}{:02x}", pixel[0], pixel[1], pixel[2]);
        crate::system::clipboard::ClipboardService::write_text(cx, hex.clone());
        crate::windows::toast::Toast::show_transient(
            cx,
            "Color copied",
            format!("{} copied to the clipboard", hex.to_uppercase()),
        );
        true
    }

    /// Window-pick variant of `open`.
    pub fn open_with_windows(
        service: CaptureService,
        display_id: DisplayId,
        display_bounds: Bounds<Pixels>,
        intent: crate::capture::intent::CaptureIntent,
        windows: Vec<crate::capture::windows_list::WindowListItem>,
        launch: OverlayLaunch,
        cx: &mut App,
    ) -> Option<gpui::WindowHandle<AreaOverlay>> {
        open_overlay_window(
            display_id,
            display_bounds,
            launch,
            |scale, focus_handle| {
                AreaOverlay::with_focus(display_bounds, scale, service, intent, focus_handle)
                    .with_windows(windows)
            },
            |_, _| {},
            true,
            "the window picker",
            cx,
        )
    }

    fn selection(&self) -> Option<(Point<Pixels>, gpui::Size<Pixels>)> {
        let rect = self.rect?;
        Some((
            point(px(rect.x), px(rect.y)),
            size(px(rect.width), px(rect.height)),
        ))
    }

    /// The display in the units `selection` works in.
    fn viewport(&self) -> selection::Size {
        selection::Size {
            width: f32::from(self.display_bounds.size.width),
            height: f32::from(self.display_bounds.size.height),
        }
    }

    fn to_selection_point(&self, position: Point<Pixels>) -> selection::Point {
        let viewport = self.viewport();
        let edge = 1.0 / self.scale.max(0.01);
        selection::Point {
            x: if f32::from(position.x) >= viewport.width - edge {
                viewport.width
            } else {
                f32::from(position.x)
            },
            y: if f32::from(position.y) >= viewport.height - edge {
                viewport.height
            } else {
                f32::from(position.y)
            },
        }
    }

    /// `dragTo` in `use-area-selection.ts`.
    fn drag_to(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(interaction) = self.interaction else {
            return;
        };
        let bounds = self.viewport();
        let point = selection::clamp_point(self.to_selection_point(position), bounds);
        // The renderer constrains the box when a caller sets an aspect-ratio
        // preset over IPC (`setAreaSelectorAspectRatio`). No GPUI flow supplies
        // one yet, so the geometry runs unconstrained; `selection::resize_rect`
        // takes the ratio the day one does.
        let ratio: Option<f32> = None;

        let next = match interaction {
            Interaction::Creating { start } => {
                let created = selection::normalize_rect(start, point);
                match ratio {
                    Some(ratio) => selection::fit_rect(
                        selection::adjust_rect_to_ratio(created, ratio, None),
                        bounds,
                    ),
                    None => created,
                }
            }
            Interaction::Moving { offset } => {
                let Some(current) = self.rect else { return };
                selection::move_rect(current, point, offset, bounds)
            }
            Interaction::Resizing { handle } => {
                let Some(current) = self.rect else { return };
                selection::fit_rect(
                    selection::resize_rect(current, point, handle, ratio),
                    bounds,
                )
            }
        };

        self.rect = Some(next);
        self.cursor = selection::cursor_for(self.rect, point);
        cx.notify();
    }

    #[cfg(target_os = "macos")]
    fn release_before_timer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        confirm: fn(&mut Self, &mut Window, &mut Context<Self>),
    ) -> bool {
        use crate::capture::intent::CaptureIntent;

        if self.intent != CaptureIntent::Timer {
            return false;
        }
        let owner = revision(cx);
        if session(cx).timer_release_owner == Some(owner) {
            return true;
        }
        let frozen = frozen_selection_generation(cx);
        if frozen == 0 {
            return false;
        }

        session(cx).timer_release_owner = Some(owner);
        let service = self.service.clone();
        let handle = window
            .window_handle()
            .downcast::<Self>()
            .expect("area overlay");
        cx.spawn(async move |_, cx| {
            #[cfg(not(test))]
            let released = smol::unblock(move || service.release_screen(frozen)).await;
            #[cfg(test)]
            let released = cx
                .background_executor()
                .spawn(async move { service.release_screen(frozen) })
                .await;
            cx.update(|cx| {
                if revision(cx) != owner || session(cx).timer_release_owner != Some(owner) {
                    return;
                }
                session(cx).timer_release_owner = None;
                if released {
                    mark_frozen_selection_released(frozen, cx);
                }
                let _ = handle.update(cx, |overlay, window, cx| {
                    if overlay.intent != CaptureIntent::Timer {
                        return;
                    }
                    if released {
                        confirm(overlay, window, cx);
                    } else {
                        crate::windows::toast::Toast::show(
                            cx,
                            "Timer Capture Failed",
                            "Could not release the frozen screen",
                        );
                        dismiss(window, cx);
                    }
                });
            });
        })
        .detach();
        true
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((origin, size)) = self.selection() else {
            return;
        };
        if size.width < px(4.0) || size.height < px(4.0) {
            return;
        }
        #[cfg(target_os = "macos")]
        if self.release_before_timer(window, cx, Self::confirm) {
            return;
        }

        let coordinate_scale = capture_coordinate_scale(self.scale);
        let rect = physical_selection_rect(
            Bounds {
                origin: self.display_bounds.origin + origin,
                size,
            },
            self.scale,
        );
        let ScreenRect {
            x,
            y,
            width,
            height,
        } = rect;

        let intent = self.intent;
        let scale = self.scale;
        if intent == crate::capture::intent::CaptureIntent::Recording {
            let all_in_one = self.all_in_one.is_some();
            if all_in_one {
                self.all_in_one = None;
                self.handed_off = true;
            }
            begin_recording_after_icon_restore(
                cx,
                all_in_one,
                crate::video::recorder::RecordingTarget::Area,
                ScreenRect {
                    x,
                    y,
                    width,
                    height,
                },
                self.display_id,
                None,
                None,
                self.recording_preference_scope,
                |_| {},
            );
            cx.notify();
            return;
        }

        let hide = take_capture_hide(cx);

        let Some(excluded_window_ids) = capture_overlay_window_ids(window, cx) else {
            dismiss(window, cx);
            crate::capture::desktop_icons::release_for_capture(hide, cx);
            crate::windows::toast::Toast::show(
                cx,
                "Capture Failed",
                "Could not exclude the capture selector",
            );
            return;
        };

        let owner = selection_owner(intent, cx);
        let coordinator = crate::state::coordinator(cx);
        let display_origin = (
            (f32::from(self.display_bounds.left()) * coordinate_scale).round() as i32,
            (f32::from(self.display_bounds.top()) * coordinate_scale).round() as i32,
        );
        let display_id = self.display_id;
        let retained = retains_overlay_after_selection(intent);
        let reservation = (!retained && self.service.config.get().screenshot.freeze_screen)
            .then(|| self.service.reserve_cached_capture());
        let release_after = if !retained {
            conceal_for_intent(window, intent, owner, cx)
        } else if !hand_off_for_timer(cx) {
            crate::windows::toast::Toast::show(
                cx,
                "Timer Capture Failed",
                "Could not release the frozen screen",
            );
            dismiss(window, cx);
            crate::capture::desktop_icons::release_for_capture(hide, cx);
            return;
        } else {
            None
        };
        cx.defer(move |cx| {
            coordinator.update(cx, |coordinator, cx| {
                coordinator.capture_area_reserved(
                    crate::capture::DisplayCapture::new(
                        ScreenRect {
                            x,
                            y,
                            width,
                            height,
                        },
                        f64::from(scale),
                        DisplayOrigin {
                            x: display_origin.0,
                            y: display_origin.1,
                        },
                        display_id,
                    ),
                    intent,
                    excluded_window_ids,
                    reservation,
                    release_after,
                    hide,
                    owner,
                    cx,
                );
            });
        });
    }
}

/// The hint the Electron overlay shows while nothing is selected yet.
fn prompt(text: &'static str, toolbar: bool) -> gpui::AnyElement {
    use crate::ui::chrome;
    let top = chrome::overlay_prompt_top(toolbar);
    div()
        .absolute()
        .top(px(top))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(
            div()
                .rounded_full()
                .bg(crate::ui::colors::black(0.7))
                .px(px(chrome::OVERLAY_PROMPT_PX))
                .py(px(chrome::OVERLAY_PROMPT_PY))
                .text_size(px(chrome::OVERLAY_PROMPT_SIZE))
                .text_color(crate::ui::colors::white(1.0))
                .shadow_lg()
                .child(text),
        )
        .into_any_element()
}

fn point(x: Pixels, y: Pixels) -> Point<Pixels> {
    gpui::point(x, y)
}

/// `CrosshairGuides`: two 1px `bg-primary/70` rules through the pointer, shown
/// while nothing is selected and no drag is in progress.
fn crosshair_guides(pointer: Point<Pixels>, accent: gpui::Hsla) -> gpui::AnyElement {
    let rule = accent.opacity(0.7);
    div()
        .absolute()
        .inset_0()
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(pointer.x)
                .w(px(1.0))
                .bg(rule),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(pointer.y)
                .h(px(1.0))
                .bg(rule),
        )
        .into_any_element()
}

/// `SelectionScrim` with a rect: four dim bars around the hole, leaving the
/// selected (or hovered) region undimmed.
fn scrim_around(
    origin: Point<Pixels>,
    hole: gpui::Size<Pixels>,
    window: &Window,
    dim: gpui::Hsla,
) -> Vec<gpui::AnyElement> {
    let left = window.pixel_snap(origin.x);
    let top = window.pixel_snap(origin.y);
    let right = window.pixel_snap(origin.x + hole.width);
    let bottom = window.pixel_snap(origin.y + hole.height);
    vec![
        div()
            .absolute()
            .left_0()
            .right_0()
            .top_0()
            .h(top.max(px(0.0)))
            .bg(dim)
            .into_any_element(),
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(bottom)
            .bottom_0()
            .bg(dim)
            .into_any_element(),
        div()
            .absolute()
            .left_0()
            .top(top)
            .w(left.max(px(0.0)))
            .h((bottom - top).max(px(0.0)))
            .bg(dim)
            .into_any_element(),
        div()
            .absolute()
            .left(right)
            .right_0()
            .top(top)
            .h((bottom - top).max(px(0.0)))
            .bg(dim)
            .into_any_element(),
    ]
}

fn size(width: Pixels, height: Pixels) -> gpui::Size<Pixels> {
    gpui::size(width, height)
}

fn accepts_window_list(
    all_in_one: Option<crate::capture::all_in_one::Choices>,
    current_generation: u64,
    request_generation: u64,
) -> bool {
    current_generation == request_generation
        && all_in_one.is_some_and(|choices| choices.mode != crate::capture::all_in_one::Mode::Ocr)
}

fn capture_intent(mode: crate::capture::all_in_one::Mode) -> crate::capture::intent::CaptureIntent {
    match mode {
        crate::capture::all_in_one::Mode::Ocr => crate::capture::intent::CaptureIntent::Ocr,
        crate::capture::all_in_one::Mode::Record => {
            crate::capture::intent::CaptureIntent::Recording
        }
        crate::capture::all_in_one::Mode::Screenshot => {
            crate::capture::intent::CaptureIntent::Screenshot
        }
    }
}

fn hand_off_for_timer(cx: &mut Context<AreaOverlay>) -> bool {
    if !release_frozen_screen_in_place(cx) {
        return false;
    }
    for_each_area_overlay(cx, |overlay, cx| {
        if overlay.intent != crate::capture::intent::CaptureIntent::Timer {
            return;
        }
        overlay.all_in_one = None;
        overlay.interaction = None;
        overlay.pointer = None;
        overlay.handed_off = true;
        cx.notify();
    });
    true
}

fn retains_overlay_after_selection(intent: crate::capture::intent::CaptureIntent) -> bool {
    matches!(
        intent,
        crate::capture::intent::CaptureIntent::Recording
            | crate::capture::intent::CaptureIntent::Timer
    )
}

fn for_each_area_overlay(
    cx: &mut Context<AreaOverlay>,
    apply: impl Fn(&mut AreaOverlay, &mut Context<AreaOverlay>) + 'static,
) {
    let handles = session(cx).handles.clone();
    App::defer(cx, move |cx| {
        for tracked in handles {
            let Some(handle) = tracked.handle.downcast::<AreaOverlay>() else {
                continue;
            };
            let _ = handle.update(cx, |overlay, _window, cx| apply(overlay, cx));
        }
    });
}

fn sync_all_in_one(
    choices: crate::capture::all_in_one::Choices,
    targets: [crate::capture::all_in_one::Target; 2],
    cx: &mut Context<AreaOverlay>,
) {
    for_each_area_overlay(cx, move |overlay, cx| {
        if overlay.all_in_one.is_none()
            || (overlay.all_in_one == Some(choices) && overlay.all_in_one_targets == targets)
        {
            return;
        }
        overlay.all_in_one = Some(choices);
        overlay.all_in_one_targets = targets;
        overlay.intent = capture_intent(choices.mode);
        overlay.apply_all_in_one_target(cx);
        cx.notify();
    });
}

fn sync_color_picker(active: bool, excluded_window_ids: Vec<i64>, cx: &mut Context<AreaOverlay>) {
    for_each_area_overlay(cx, move |overlay, cx| {
        if overlay.all_in_one.is_none() || overlay.picking_color == active {
            return;
        }
        if active {
            overlay.activate_color_picker(excluded_window_ids.clone(), cx);
            return;
        }
        overlay.picking_color = false;
        overlay.color_frame = None;
        cx.notify();
    });
}

fn sync_window_recording_handoff(cx: &mut Context<AreaOverlay>) {
    let handles = session(cx).handles.clone();
    App::defer(cx, move |cx| {
        for tracked in handles {
            let Some(handle) = tracked.handle.downcast::<AreaOverlay>() else {
                continue;
            };
            let _ = handle.update(cx, |overlay, window, cx| {
                if overlay.intent != crate::capture::intent::CaptureIntent::Recording {
                    return;
                }
                overlay.all_in_one = None;
                overlay.picking_windows = false;
                overlay.hovered_window = None;
                overlay.interaction = None;
                overlay.pointer = None;
                overlay.handed_off = true;
                conceal_recording_selector(window);
                cx.notify();
            });
        }
        for window_handle in cx.windows() {
            let Some(handle) =
                window_handle.downcast::<crate::windows::recording_control::RecordingControl>()
            else {
                continue;
            };
            let _ = handle.update(cx, |_, window, _| window.activate_window());
            break;
        }
    });
}

fn conceal_screen_recording_pick(window: &mut Window, cx: &mut Context<AreaOverlay>) {
    conceal_recording_selector(window);
    let handles = session(cx).handles.clone();
    App::defer(cx, move |cx| {
        for tracked in handles {
            let Some(handle) = tracked.handle.downcast::<AreaOverlay>() else {
                continue;
            };
            let _ = handle.update(cx, |overlay, window, cx| {
                if overlay.intent != crate::capture::intent::CaptureIntent::Recording
                    || !overlay.screen_picker
                {
                    return;
                }
                overlay.screen_picker = false;
                overlay.interaction = None;
                overlay.pointer = None;
                conceal_recording_selector(window);
                cx.notify();
            });
        }
    });
}

fn conceal_recording_selector(window: &mut Window) {
    #[cfg(windows)]
    park_overlay(window);
    window.remove_window();
}

impl Render for AreaOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(any(windows, target_os = "macos"))]
        if self.parked {
            return div().size_full().into_any_element();
        }
        #[cfg(windows)]
        if self.surface_state == PooledSurfaceState::Clean {
            self.surface_state = PooledSurfaceState::Live;
        }
        #[cfg(target_os = "macos")]
        let window_hovered = self.pointer_inside;
        #[cfg(not(target_os = "macos"))]
        let window_hovered = window.is_window_hovered();
        if window.focused(cx).is_none() {
            window.focus(&self.focus_handle, cx);
        }
        let content = self.surface(window_hovered, window, cx);
        crate::ui::window_root::focus_root(content, window, cx)
            .key_context("AreaOverlay")
            .on_action(cx.listener(Self::on_cancel))
            .into_any_element()
    }
}

impl AreaOverlay {
    fn on_cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if self.handed_off {
            return;
        }
        if self.menu.is_open(cx) {
            self.menu.close(window, cx);
            self.focus_all_in_one_menu_trigger(window, cx);
            cx.stop_propagation();
            return;
        }
        if self.picking_color {
            self.stop_color_picker(cx);
            return;
        }
        if self.intent == crate::capture::intent::CaptureIntent::Recording {
            crate::windows::recording_control::RecordingControl::cancel_pre_recording(cx);
        }
        if self.intent == crate::capture::intent::CaptureIntent::EditorAttach {
            let coordinator = crate::state::coordinator(cx);
            coordinator.update(cx, |coordinator, cx| {
                coordinator.cancel_editor_attach(cx);
            });
        }
        dismiss(window, cx);
    }

    pub(crate) fn surface(
        &mut self,
        window_hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = crate::theme::vars::active_theme(cx);
        // The overlay's frame, handles and crosshairs are `border-primary` /
        // `bg-primary` / `bg-primary/70`, and `--primary` is the operating
        // system's accent rather than the theme's -- see `theme::vars`.
        let accent = theme.primary;

        let selection = self.selection();
        let dim = crate::ui::colors::black(crate::ui::chrome::OVERLAY_DIM);

        let root = crate::ui::font::root()
            .id("area-overlay")
            .size_full()
            .key_context("AreaOverlay")
            .track_focus(&self.focus_handle);

        if self.handed_off {
            let viewport = window.viewport_size();
            return match selection {
                Some((origin, bounds)) => {
                    let left = (origin.x - px(1.0)).max(px(0.0));
                    let top = (origin.y - px(1.0)).max(px(0.0));
                    let right = (origin.x + bounds.width + px(1.0)).min(viewport.width);
                    let bottom = (origin.y + bounds.height + px(1.0)).min(viewport.height);
                    root.children(scrim_around(
                        point(left, top),
                        size(right - left, bottom - top),
                        window,
                        dim,
                    ))
                }
                None => root.child(div().absolute().inset_0().bg(dim)),
            };
        }

        let root = root
            .cursor(self.cursor)
            .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                if this.menu.is_open(cx) {
                    let menu = this.menu.clone();
                    let focus = this.focus_handle.clone();
                    let previous = window.focused(cx);
                    window.defer(cx, move |window, cx| {
                        if !menu.is_open(cx) && window.focused(cx) == previous {
                            window.focus(&focus, cx);
                        }
                    });
                    return;
                }
                this.menu_activation_key = None;
                this.menu_return_focus = false;
            }))
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    if this.menu.is_open(cx) {
                        this.menu.close(window, cx);
                        this.focus_all_in_one_menu_trigger(window, cx);
                        cx.stop_propagation();
                        return;
                    }
                    this.on_cancel(&Cancel, window, cx);
                    cx.stop_propagation();
                    return;
                }
                if this.menu.is_open(cx) && !event.keystroke.modifiers.modified() {
                    this.menu_activation_key = match event.keystroke.key.as_str() {
                        "enter" => Some("enter"),
                        "space" => Some("space"),
                        _ => None,
                    };
                    this.menu_return_focus = false;
                } else if !event.is_held {
                    this.menu_activation_key = None;
                    this.menu_return_focus = false;
                }
            }))
            .on_key_up(cx.listener(|this, event: &gpui::KeyUpEvent, window, cx| {
                if this.menu_activation_key != Some(event.keystroke.key.as_str()) {
                    return;
                }
                this.menu_activation_key = None;
                let restore = std::mem::take(&mut this.menu_return_focus);
                if restore
                    && this.focus_handle.is_focused(window)
                    && this.all_in_one.is_some_and(|choices| {
                        choices.mode != crate::capture::all_in_one::Mode::Ocr
                    })
                {
                    this.focus_all_in_one_menu_trigger(window, cx);
                }
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
            .on_mouse_move(cx.listener(Self::on_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up));

        #[cfg(target_os = "macos")]
        let root = root.on_mouse_exit(cx.listener(|this, _, _, cx| {
            if this.pointer_inside {
                this.pointer_inside = false;
                cx.notify();
            }
        }));

        let toolbar = self.all_in_one.map(|choices| {
            crate::capture::all_in_one_toolbar::render(choices, &self.menu, &theme, cx)
        });

        if self.picking_color {
            return root.children(self.pointer.and_then(|pointer| {
                self.color_frame.as_ref().map(|frame| {
                    crate::capture::color_picker::render(frame, pointer, window, &theme)
                })
            }));
        }

        if self.is_picking_windows() {
            // `SelectionScrim` takes the hovered target as its rect while
            // picking, so the candidate window shows through undimmed. The DOM
            // overlay draws neither a frame nor a name label in this mode.
            let mut picking = root;
            let hovered_frame = if window_hovered {
                self.hovered_frame()
            } else {
                None
            };
            picking = match hovered_frame {
                Some((origin, frame)) => picking.children(scrim_around(origin, frame, window, dim)),
                None => picking.child(
                    div()
                        .debug_selector(|| "window-picker-dim".to_string())
                        .absolute()
                        .inset_0()
                        .bg(dim),
                ),
            };
            if let Some(toolbar) = toolbar {
                picking = picking.child(toolbar);
            }
            // While picking a window the reference shows the pick-targets
            // prompt, not the drag one.
            return picking.child(prompt(
                crate::capture::intent::WINDOW_PICK_PROMPT,
                self.all_in_one.is_some(),
            ));
        }

        if let Some(dimmed) = self.screen_picker_dimmed(window_hovered) {
            let mut picking = if dimmed {
                root.child(
                    div()
                        .debug_selector(|| "display-picker-dim".to_string())
                        .absolute()
                        .inset_0()
                        .bg(dim),
                )
            } else {
                root
            };
            if let Some(toolbar) = toolbar {
                picking = picking.child(toolbar);
            }
            return picking.child(prompt(
                crate::capture::intent::DISPLAY_PICK_PROMPT,
                self.all_in_one.is_some(),
            ));
        }

        let mut element = match selection {
            None => root
                .child(div().absolute().inset_0().bg(dim))
                .children(
                    self.pointer
                        .map(|pointer| crosshair_guides(pointer, accent)),
                )
                .children(
                    self.intent
                        .shows_prompt()
                        .then(|| prompt(self.intent.prompt(), self.all_in_one.is_some())),
                ),
            Some((origin, bounds_size)) => {
                let left_w = window.pixel_snap(origin.x);
                let top_h = window.pixel_snap(origin.y);
                let selection_width = window.pixel_snap(bounds_size.width);
                let selection_height = window.pixel_snap(bounds_size.height);
                let outline_width = window.pixel_snap(px(1.0));
                let right_x = left_w + selection_width;
                let bottom_y = top_h + selection_height;

                root.child(div().absolute().left_0().top_0().w(left_w).h_full().bg(dim))
                    .child(
                        div()
                            .absolute()
                            .left(right_x)
                            .right_0()
                            .top_0()
                            .h_full()
                            .bg(dim),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(left_w)
                            .top_0()
                            .w(selection_width)
                            .h(top_h)
                            .bg(dim),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(left_w)
                            .top(bottom_y)
                            .bottom_0()
                            .w(selection_width)
                            .bg(dim),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(left_w - outline_width)
                            .top(top_h - outline_width)
                            .w(selection_width + outline_width * 2.0)
                            .h(selection_height + outline_width * 2.0)
                            .border_1()
                            .border_color(crate::ui::colors::black(0.35)),
                    )
                    .child({
                        let mut frame = div()
                            .absolute()
                            .left(origin.x)
                            .top(origin.y)
                            .w(bounds_size.width)
                            .h(bounds_size.height)
                            .border_1()
                            .border_color(accent);
                        for (handle_x, handle_y, handle_w, handle_h) in
                            crate::ui::chrome::overlay_handle_rects(
                                f32::from(bounds_size.width),
                                f32::from(bounds_size.height),
                            )
                        {
                            frame = frame.child(
                                div()
                                    .absolute()
                                    .left(px(handle_x))
                                    .top(px(handle_y))
                                    .w(px(handle_w))
                                    .h(px(handle_h))
                                    .bg(accent)
                                    // `ring-1 ring-black/20` on every bar.
                                    .shadow(vec![gpui::BoxShadow {
                                        color: crate::ui::colors::black(0.2),
                                        offset: point(px(0.0), px(0.0)),
                                        blur_radius: px(0.0),
                                        spread_radius: px(1.0),
                                        inset: false,
                                    }]),
                            );
                        }
                        frame
                    })
                    .child({
                        let viewport_h = f32::from(window.viewport_size().height);
                        let top = f32::from(origin.y);
                        let height = f32::from(bounds_size.height);
                        let above = height < crate::ui::chrome::OVERLAY_LABEL_CLEARANCE
                            && top >= crate::ui::chrome::OVERLAY_LABEL_CLEARANCE
                            && !crate::ui::chrome::overlay_label_below(top, height, viewport_h);
                        let (anchor, label_y) = if above {
                            (gpui::Anchor::BottomCenter, top - 8.)
                        } else {
                            (
                                gpui::Anchor::TopCenter,
                                crate::ui::chrome::overlay_label_top(top, height, viewport_h),
                            )
                        };
                        gpui::anchored()
                            .anchor(anchor)
                            .position(point(origin.x + bounds_size.width / 2., px(label_y)))
                            .snap_to_window_with_margin(px(1.))
                            .child(
                                div()
                                    .debug_selector(|| "overlay-size-label".to_string())
                                    .rounded(px(crate::ui::chrome::OVERLAY_LABEL_RADIUS))
                                    .bg(crate::ui::colors::black(0.75))
                                    .px(px(crate::ui::chrome::OVERLAY_LABEL_PX))
                                    .py(px(crate::ui::chrome::OVERLAY_LABEL_PY))
                                    .text_size(px(crate::ui::chrome::OVERLAY_LABEL_SIZE))
                                    .text_color(crate::ui::colors::white(1.0))
                                    .font_family(crate::ui::colors::MONO_FONT)
                                    .child(format!(
                                        "{} × {}",
                                        f32::from(bounds_size.width).round() as i32,
                                        f32::from(bounds_size.height).round() as i32
                                    )),
                            )
                    })
            }
        };

        if let Some(toolbar) = toolbar {
            element = element.child(toolbar);
        }

        element
    }
}

impl AreaOverlay {
    /// `startDrag`: a press on a handle resizes, inside the box moves, and
    /// anywhere else starts a new box.
    fn on_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(target_os = "macos")]
        if pending_recording_preparation(cx).is_some() || session(cx).timer_release_owner.is_some()
        {
            return;
        }
        if self.handed_off {
            return;
        }
        if self.picking_color {
            if self.pick_color(event.position, cx) {
                dismiss(window, cx);
            }
            return;
        }
        if self.menu.is_present(cx) {
            return;
        }
        if self.is_picking_windows() {
            self.hovered_window = self.window_at(event.position);
            cx.notify();
            self.confirm_window(window, cx);
            return;
        }
        if self.is_picking_screen() {
            self.confirm_screen(window, cx);
            return;
        }

        let bounds = self.viewport();
        let point = selection::clamp_point(self.to_selection_point(event.position), bounds);

        match selection::gesture_at(self.rect, point) {
            selection::Gesture::Resize { handle } => {
                self.interaction = Some(Interaction::Resizing { handle });
                self.cursor = handle.cursor();
            }
            selection::Gesture::Move { offset } => {
                self.interaction = Some(Interaction::Moving { offset });
                self.cursor = gpui::CursorStyle::ClosedHand;
            }
            selection::Gesture::Create => {
                self.interaction = Some(Interaction::Creating { start: point });
                self.rect = None;
                self.cursor = gpui::CursorStyle::Crosshair;
            }
        }
        cx.notify();
    }

    fn confirm_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(target_os = "macos")]
        if self.release_before_timer(window, cx, Self::confirm_screen) {
            return;
        }
        let scale = self.scale;
        let capture = display_capture(self.display_bounds, scale, self.display_id);
        let rect = capture.rect;
        let intent = self.intent;
        if intent == crate::capture::intent::CaptureIntent::Recording {
            let all_in_one = self.all_in_one.is_some();
            if all_in_one {
                self.handed_off = true;
            }
            if self.screen_picker {
                conceal_screen_recording_pick(window, cx);
            } else {
                self.rect = Some(selection::Rect {
                    x: 0.0,
                    y: 0.0,
                    width: self.viewport().width,
                    height: self.viewport().height,
                });
            }
            begin_recording_after_icon_restore(
                cx,
                all_in_one,
                crate::video::recorder::RecordingTarget::Screen,
                rect,
                self.display_id,
                None,
                None,
                self.recording_preference_scope,
                |_| {},
            );
            cx.notify();
            return;
        }
        let hide = take_capture_hide(cx);
        let Some(excluded_window_ids) = capture_overlay_window_ids(window, cx) else {
            dismiss(window, cx);
            crate::capture::desktop_icons::release_for_capture(hide, cx);
            crate::windows::toast::Toast::show(
                cx,
                "Capture Failed",
                "Could not exclude the capture selector",
            );
            return;
        };
        let owner = selection_owner(intent, cx);
        let coordinator = crate::state::coordinator(cx);
        let retained = retains_overlay_after_selection(intent);
        let reservation = (!retained && self.service.config.get().screenshot.freeze_screen)
            .then(|| self.service.reserve_cached_capture());
        let release_after = if !retained {
            conceal_for_intent(window, intent, owner, cx)
        } else if !hand_off_for_timer(cx) {
            crate::windows::toast::Toast::show(
                cx,
                "Timer Capture Failed",
                "Could not release the frozen screen",
            );
            dismiss(window, cx);
            crate::capture::desktop_icons::release_for_capture(hide, cx);
            return;
        } else {
            None
        };
        cx.defer(move |cx| {
            coordinator.update(cx, |coordinator, cx| {
                coordinator.capture_area_reserved(
                    capture,
                    intent,
                    excluded_window_ids,
                    reservation,
                    release_after,
                    hide,
                    owner,
                    cx,
                );
            });
        });
    }

    fn on_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(target_os = "macos")]
        if !self.pointer_inside {
            self.pointer_inside = true;
            cx.notify();
        }
        if self.pointer != Some(event.position) {
            self.pointer = Some(event.position);
            if !self.is_picking_windows() && !self.is_picking_screen() && self.selection().is_none()
            {
                cx.notify();
            }
        }
        if self.picking_color {
            if self.cursor != gpui::CursorStyle::Crosshair {
                self.cursor = gpui::CursorStyle::Crosshair;
                cx.notify();
            }
            return;
        }
        if self.is_picking_screen() {
            return;
        }
        if self.is_picking_windows() {
            let hovered = self.window_at(event.position);
            if hovered != self.hovered_window {
                self.hovered_window = hovered;
                cx.notify();
            }
            return;
        }
        if self.interaction.is_some() {
            self.drag_to(event.position, cx);
            return;
        }

        // With no gesture in flight the cursor still tracks the box, so the
        // move and resize affordances are discoverable.
        let bounds = self.viewport();
        let point = selection::clamp_point(self.to_selection_point(event.position), bounds);
        let cursor = selection::cursor_for(self.rect, point);
        if cursor != self.cursor {
            self.cursor = cursor;
            cx.notify();
        }
    }

    /// `endDrag`: a freshly drawn box is committed (or discarded when it is too
    /// small); an adjusted one stays put.
    fn on_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.handed_off {
            return;
        }
        if self.is_picking_windows() {
            return;
        }
        self.drag_to(event.position, cx);
        let Some(interaction) = self.interaction.take() else {
            return;
        };

        let bounds = self.viewport();
        let point = selection::clamp_point(self.to_selection_point(event.position), bounds);
        self.cursor = selection::cursor_for(self.rect, point);

        if !matches!(interaction, Interaction::Creating { .. }) {
            if self.intent == crate::capture::intent::CaptureIntent::Recording {
                self.confirm(window, cx);
                return;
            }
            cx.notify();
            return;
        }

        match self.rect {
            Some(rect) if selection::is_usable_selection(rect) => self.confirm(window, cx),
            _ => {
                self.rect = None;
                cx.notify();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gpui::{px, size, TestAppContext};
    use herogpui::gpui;
    use poratake_daemon_common::contract::RecordingOverlayShowWindowRequest;

    use crate::config::store::ConfigStore;
    use crate::ui::chrome;

    #[test]
    fn overlay_dim_label_and_handles_match_electron() {
        assert_eq!(chrome::OVERLAY_DIM, 0.5);
        assert_eq!(chrome::OVERLAY_FRAME_BORDER, 1.0);
        assert!(chrome::overlay_label_below(20.0, 80.0, 200.0));
        assert!(!chrome::overlay_label_below(150.0, 40.0, 200.0));
        assert_eq!(chrome::overlay_label_top(20.0, 80.0, 200.0), 108.0);
        assert_eq!(chrome::overlay_label_top(150.0, 40.0, 200.0), 154.0);
        let handles = chrome::overlay_handle_rects(100.0, 80.0);
        assert_eq!(handles.len(), 12);
        assert_eq!(handles[0], (0.0, 0.0, 20.0, 4.0));
        assert_eq!(handles[1], (0.0, 0.0, 4.0, 20.0));
        assert_eq!(handles[2], (80.0, 0.0, 20.0, 4.0));
        assert_eq!(handles[8], (40.0, 0.0, 20.0, 4.0));
    }

    #[test]
    fn recording_and_the_timer_countdown_keep_the_selection_overlay() {
        use crate::capture::intent::CaptureIntent;

        assert!(super::retains_overlay_after_selection(
            CaptureIntent::Recording
        ));
        assert!(super::retains_overlay_after_selection(CaptureIntent::Timer));
        assert!(!super::retains_overlay_after_selection(
            CaptureIntent::Screenshot
        ));
        assert!(!super::retains_overlay_after_selection(CaptureIntent::Ocr));
    }

    #[test]
    fn window_outline_uses_the_electron_daemon_contract() {
        let request = RecordingOverlayShowWindowRequest {
            window_id: 42,
            color: Some("#8892ef".to_string()),
            below_window_id: Some(84),
        };
        assert_eq!(
            serde_json::to_value(request).expect("serialize request"),
            serde_json::json!({
                "windowId": 42,
                "color": "#8892ef",
                "belowWindowId": 84,
            })
        );
    }

    #[test]
    fn capture_rect_uses_platform_coordinate_units() {
        let bounds = gpui::Bounds {
            origin: gpui::point(px(100.0), px(50.0)),
            size: size(px(800.0), px(600.0)),
        };
        let rect = super::physical_rect(bounds, 1.5);
        let scale = super::capture_coordinate_scale(1.5);
        assert_eq!(rect.x, (100.0 * scale) as i32);
        assert_eq!(rect.y, (50.0 * scale) as i32);
        assert_eq!(rect.width, (800.0 * scale) as i32);
        assert_eq!(rect.height, (600.0 * scale) as i32);
    }

    #[test]
    fn fractional_selection_stays_inside_display_edges() {
        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.5), px(0.5)),
            size: size(px(1727.5), px(1116.5)),
        };
        let rect = super::physical_selection_rect(bounds, 1.0);
        assert_eq!(rect.x, 1);
        assert_eq!(rect.y, 1);
        assert_eq!(rect.x + rect.width, 1728);
        assert_eq!(rect.y + rect.height, 1117);
        let layout = super::physical_rect(bounds, 1.0);
        assert_eq!(layout.width, 1728);
        assert_eq!(layout.height, 1117);

        let negative = gpui::Bounds {
            origin: gpui::point(px(-1599.5), px(-899.5)),
            size: size(px(1599.5), px(899.5)),
        };
        let rect = super::physical_selection_rect(negative, 1.0);
        assert_eq!(rect.x, -1600);
        assert_eq!(rect.y, -900);
        assert_eq!(rect.x + rect.width, 0);
        assert_eq!(rect.y + rect.height, 0);
    }

    #[test]
    fn display_capture_keeps_target_display_geometry_scale_and_id_together() {
        let bounds = gpui::Bounds {
            origin: gpui::point(px(-1600.0), px(120.0)),
            size: size(px(1000.0), px(800.0)),
        };
        let capture = super::display_capture(bounds, 1.5, Some(73));
        let coordinate_scale = super::capture_coordinate_scale(1.5);

        assert_eq!(capture.rect.x, (-1600.0 * coordinate_scale) as i32);
        assert_eq!(capture.rect.y, (120.0 * coordinate_scale) as i32);
        assert_eq!(capture.rect.width, (1000.0 * coordinate_scale) as i32);
        assert_eq!(capture.rect.height, (800.0 * coordinate_scale) as i32);
        assert_eq!(capture.scale_factor, 1.5);
        assert_eq!(capture.display_origin().x, capture.rect.x);
        assert_eq!(capture.display_origin().y, capture.rect.y);
        assert_eq!(capture.display_id, Some(73));
    }

    #[herogpui::test]
    fn opened_overlay_keeps_the_selected_display_id(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let (display_id, display_bounds) = cx.read(|cx| {
            let display = cx.primary_display().expect("primary display");
            (display.id(), display.bounds())
        });
        let opened = cx.update(|cx| {
            super::AreaOverlay::open(
                crate::state::state(cx),
                display_id,
                display_bounds,
                crate::capture::intent::CaptureIntent::Screenshot,
                super::OverlayLaunch {
                    focus: false,
                    deferred_show: false,
                    generation: 0,
                },
                cx,
            )
            .expect("open overlay")
        });

        assert_eq!(
            opened
                .update(cx, |overlay, _, _| overlay.display_id)
                .expect("read display id"),
            Some(u64::from(display_id) as u32)
        );
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn scroll_selector_leases_are_independent_and_leave_new_selections_open(
        cx: &mut TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let (display_id, bounds) = cx.read(|cx| {
            let display = cx.primary_display().unwrap();
            (display.id(), display.bounds())
        });
        let open = |cx: &mut gpui::App| {
            super::AreaOverlay::open(
                crate::state::state(cx),
                display_id,
                bounds,
                crate::capture::intent::CaptureIntent::ScrollCapture,
                super::OverlayLaunch {
                    focus: false,
                    deferred_show: false,
                    generation: 0,
                },
                cx,
            )
            .unwrap()
        };
        let first = cx.update(open);
        let first_owner = cx.update(super::revision);
        first
            .update(cx, |_, window, cx| {
                super::conceal_for_scroll(window, first_owner, cx)
            })
            .unwrap();
        let second = cx.update(open);
        let second_owner = cx.update(super::revision);
        second
            .update(cx, |_, window, cx| {
                super::conceal_for_scroll(window, second_owner, cx)
            })
            .unwrap();
        let active = cx.update(open);
        assert_ne!(first_owner, second_owner);
        assert!(first.update(cx, |_, _, _| ()).is_ok());
        assert!(second.update(cx, |_, _, _| ()).is_ok());
        cx.update(|cx| super::release_scroll_selectors(Some(first_owner), cx));
        assert!(first.update(cx, |_, _, _| ()).is_err());
        assert!(second.update(cx, |_, _, _| ()).is_ok());
        assert!(active.update(cx, |_, _, _| ()).is_ok());
        cx.update(|cx| super::release_scroll_selectors(Some(second_owner), cx));
        assert!(second.update(cx, |_, _, _| ()).is_err());
        assert!(active.update(cx, |_, _, _| ()).is_ok());
        cx.update(super::replace_all);
    }

    #[herogpui::test]
    fn deactivation_clears_a_menu_key_whose_release_was_lost(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let (display_id, bounds) = cx.read(|cx| {
            let display = cx.primary_display().expect("primary display");
            (display.id(), display.bounds())
        });
        let first = cx.update(|cx| {
            super::AreaOverlay::open(
                crate::state::state(cx),
                display_id,
                bounds,
                crate::capture::intent::CaptureIntent::Screenshot,
                super::OverlayLaunch {
                    focus: false,
                    deferred_show: false,
                    generation: 0,
                },
                cx,
            )
            .expect("open overlay")
        });
        let service = cx.read(crate::state::state);
        let second = cx.add_window(|_, cx| {
            super::AreaOverlay::with_focus(
                bounds,
                1.0,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                cx.focus_handle(),
            )
        });
        first
            .update(cx, |_, window, _| window.activate_window())
            .expect("activate overlay");
        cx.run_until_parked();
        first
            .update(cx, |overlay, window, _| {
                assert!(window.is_window_active());
                overlay.menu_activation_key = Some("enter");
                overlay.menu_return_focus = true;
            })
            .expect("hold menu key");
        second
            .update(cx, |_, window, _| window.activate_window())
            .expect("activate another window");
        cx.run_until_parked();
        assert_eq!(
            first
                .update(cx, |overlay, _, _| (
                    overlay.menu_activation_key,
                    overlay.menu_return_focus
                ))
                .expect("read menu key"),
            (None, false)
        );
    }

    #[herogpui::test]
    fn screen_picker_keeps_the_recording_intent(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let (display_id, display_bounds) = cx.read(|cx| {
            let display = cx.primary_display().expect("primary display");
            (display.id(), display.bounds())
        });
        let opened = cx.update(|cx| {
            super::AreaOverlay::open_screen_picker(
                crate::state::state(cx),
                display_id,
                display_bounds,
                crate::capture::intent::CaptureIntent::Recording,
                super::OverlayLaunch {
                    focus: false,
                    deferred_show: false,
                    generation: 0,
                },
                cx,
            )
            .expect("open picker")
        });

        assert_eq!(
            opened
                .update(cx, |overlay, _, _| (
                    overlay.intent,
                    overlay.is_picking_screen()
                ))
                .expect("read intent"),
            (crate::capture::intent::CaptureIntent::Recording, true)
        );
    }

    #[test]
    fn recording_rect_maps_only_to_its_display() {
        let display = gpui::Bounds {
            origin: gpui::point(px(100.0), px(50.0)),
            size: size(px(800.0), px(600.0)),
        };
        let scale = super::capture_coordinate_scale(1.5);
        assert_eq!(
            super::local_recording_rect(
                super::ScreenRect {
                    x: (200.0 * scale) as i32,
                    y: (100.0 * scale) as i32,
                    width: (400.0 * scale) as i32,
                    height: (300.0 * scale) as i32,
                },
                display,
                1.5,
            ),
            Some(crate::capture::selection::Rect {
                x: 100.0,
                y: 50.0,
                width: 400.0,
                height: 300.0,
            })
        );
        assert_eq!(
            super::local_recording_rect(super::physical_rect(display, 1.5), display, 1.5,),
            Some(crate::capture::selection::Rect {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            })
        );
        assert_eq!(
            super::local_recording_rect(
                super::ScreenRect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                },
                display,
                1.5,
            ),
            None
        );
    }

    #[herogpui::test]
    fn resizing_reaches_the_outer_screen_edge(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let overlay = cx.add_window(|_, cx| {
            let focus = cx.focus_handle();
            super::AreaOverlay::with_focus(
                bounds,
                1.25,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                focus,
            )
        });

        assert_eq!(
            overlay
                .update(cx, |overlay, _window, cx| {
                    overlay.rect = Some(crate::capture::selection::Rect {
                        x: 100.0,
                        y: 100.0,
                        width: 200.0,
                        height: 100.0,
                    });
                    overlay.interaction = Some(super::Interaction::Resizing {
                        handle: crate::capture::selection::Handle::BottomRight,
                    });
                    overlay.drag_to(gpui::point(px(799.2), px(599.2)), cx);
                    overlay.rect
                })
                .expect("resize selection"),
            Some(crate::capture::selection::Rect {
                x: 100.0,
                y: 100.0,
                width: 700.0,
                height: 500.0,
            })
        );
    }

    #[herogpui::test]
    fn selection_size_label_stays_inside_display_edges(cx: &mut TestAppContext) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let (overlay, visual) = herogpui::test::open_window(cx, |_, cx| {
            super::AreaOverlay::with_focus(
                gpui::Bounds::new(gpui::point(px(0.), px(0.)), size(px(400.), px(300.))),
                1.,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                cx.focus_handle(),
            )
        });
        for scale in [1., 1.25, 1.5, 1.75, 2.] {
            visual.simulate_scale_factor_change(scale);
            visual.simulate_resize(size(px(400.), px(300.)));
            for (width, height) in [(12., 12.), (20., 20.), (123., 40.)] {
                for y in [0., 100., 300. - height] {
                    let mut label_width = None;
                    for x in [0., (400. - width) / 2., 400. - width] {
                        visual.update(|_, cx| {
                            overlay.update(cx, |overlay, cx| {
                                overlay.rect = Some(super::selection::Rect {
                                    x,
                                    y,
                                    width,
                                    height,
                                });
                                cx.notify();
                            });
                        });
                        let label = visual.expect("overlay-size-label");
                        assert!(
                            label.left() >= px(0.)
                                && label.right() <= px(400.)
                                && label.top() >= px(0.)
                                && label.bottom() <= px(300.),
                            "scale {scale}, selection ({x}, {y}, {width}, {height}): label {label:?}"
                        );
                        if y == 300. - height && height < 36. {
                            assert!(
                                label.bottom() <= px(y - 7.),
                                "scale {scale}, selection ({x}, {y}, {width}, {height}): label overlaps selection {label:?}"
                            );
                        }
                        if let Some(previous) = label_width {
                            assert_eq!(label.size.width, previous);
                        }
                        label_width = Some(label.size.width);
                        if x == (400. - width) / 2. {
                            assert!((f32::from(label.center().x) - 200.).abs() <= 1.);
                        }
                        visual.update(|window, _| {
                            assert_eq!(window.scale_factor(), scale);
                            let quads: Vec<_> = window
                                .painted_quads()
                                .into_iter()
                                .filter(|quad| {
                                    [
                                        (quad.bounds.left().0 / scale, f32::from(label.left())),
                                        (quad.bounds.top().0 / scale, f32::from(label.top())),
                                        (
                                            quad.bounds.size.width.0 / scale,
                                            f32::from(label.size.width),
                                        ),
                                        (
                                            quad.bounds.size.height.0 / scale,
                                            f32::from(label.size.height),
                                        ),
                                    ]
                                    .iter()
                                    .all(
                                        |(actual, expected)| {
                                            (actual - expected).abs() <= 1. / scale + 0.001
                                        },
                                    )
                                })
                                .collect();
                            assert_eq!(quads.len(), 1, "label {label:?}: {quads:?}");
                            let quad = quads[0];
                            let bounds = quad.bounds;
                            let mask = quad.content_mask.bounds;
                            assert!(
                                mask.left().0 <= bounds.left().0
                                    && mask.right().0 >= bounds.right().0
                                    && mask.top().0 <= bounds.top().0
                                    && mask.bottom().0 >= bounds.bottom().0,
                                "label {bounds:?}, mask {mask:?}"
                            );
                        });
                    }
                }
            }
        }
        visual.update(|_, cx| {
            overlay.update(cx, |overlay, cx| {
                overlay.rect = Some(super::selection::Rect {
                    x: 0.,
                    y: 0.,
                    width: 20.,
                    height: 20.,
                });
                cx.notify();
            });
        });
        let label = visual.expect("overlay-size-label");
        visual.simulate_mouse_down(
            label.center(),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        assert!(matches!(
            overlay.read_with(visual, |overlay, _| overlay.interaction),
            Some(super::Interaction::Creating { .. })
        ));
        visual.update(|_, cx| {
            overlay.update(cx, |overlay, cx| {
                overlay.rect = Some(super::selection::Rect {
                    x: 100.,
                    y: 100.,
                    width: 123.,
                    height: 40.,
                });
                overlay.interaction = None;
                cx.notify();
            });
        });
        visual.simulate_mouse_down(
            gpui::point(px(224.), px(120.)),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        assert!(matches!(
            overlay.read_with(visual, |overlay, _| overlay.interaction),
            Some(super::Interaction::Resizing {
                handle: super::selection::Handle::Right,
            })
        ));
    }

    #[herogpui::test]
    fn recording_handoff_scrim_covers_the_display_without_seams(cx: &mut TestAppContext) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let (overlay, visual) = herogpui::test::open_window(cx, |_, cx| {
            super::AreaOverlay::with_focus(
                gpui::Bounds::new(gpui::point(px(0.), px(0.)), size(px(400.), px(300.))),
                1.,
                service,
                crate::capture::intent::CaptureIntent::Recording,
                cx.focus_handle(),
            )
        });
        for scale in [1., 1.25, 1.5, 1.75, 2.] {
            visual.simulate_scale_factor_change(scale);
            visual.simulate_resize(size(px(400.), px(300.)));
            let viewport_width = (400. * scale) as u32;
            let viewport_height = (300. * scale) as u32;
            for (x, y, width, height) in [
                (53, 79, 173, 151),
                (0, 0, 119, 91),
                (viewport_width - 121, viewport_height - 101, 121, 101),
            ] {
                let rect = super::selection::Rect {
                    x: x as f32 / scale,
                    y: y as f32 / scale,
                    width: width as f32 / scale,
                    height: height as f32 / scale,
                };
                visual.update(|_, cx| {
                    overlay.update(cx, |overlay, cx| {
                        overlay.scale = scale;
                        overlay.rect = Some(rect);
                        overlay.handed_off = true;
                        cx.notify();
                    });
                });
                visual.settle();
                visual.update(|window, _| {
                    assert_eq!(window.scale_factor(), scale);
                    let edges = [
                        (rect.x - 1.).max(0.),
                        (rect.y - 1.).max(0.),
                        (rect.x + rect.width + 1.).min(400.),
                        (rect.y + rect.height + 1.).min(300.),
                    ]
                    .map(|value| (f32::from(window.pixel_snap(px(value))) * scale).round() as u32);
                    let quads = window.painted_quads();
                    for y in 0..viewport_height {
                        for x in 0..viewport_width {
                            let point_x = x as f32 + 0.5;
                            let point_y = y as f32 + 0.5;
                            let count = quads
                                .iter()
                                .filter(|quad| {
                                    let bounds = quad.bounds;
                                    let mask = quad.content_mask.bounds;
                                    [bounds, mask].iter().all(|bounds| {
                                        point_x >= bounds.left().0
                                            && point_x < bounds.right().0
                                            && point_y >= bounds.top().0
                                            && point_y < bounds.bottom().0
                                    })
                                })
                                .count();
                            let inside = x >= edges[0]
                                && y >= edges[1]
                                && x < edges[2]
                                && y < edges[3];
                            assert_eq!(
                                count,
                                usize::from(!inside),
                                "scale {scale}, selection {rect:?}, pixel ({x}, {y}), edges {edges:?}, quads {quads:?}"
                            );
                        }
                    }
                });
            }
        }
    }

    #[herogpui::test]
    fn deferred_show_keeps_overlay_hidden_until_freeze_completes(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let (_, visual) = herogpui::test::open_window(cx, |_, cx| {
            super::AreaOverlay::with_focus(
                gpui::Bounds::new(gpui::point(px(0.), px(0.)), size(px(400.), px(300.))),
                1.,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                cx.focus_handle(),
            )
        });
        visual.update(|window, _| {
            assert!(!super::reveal_overlay_window(
                window,
                super::OverlayLaunch {
                    focus: true,
                    deferred_show: true,
                    generation: 1,
                },
                true,
            ));
            assert!(!super::reveal_overlay_window(
                window,
                super::OverlayLaunch {
                    focus: true,
                    deferred_show: false,
                    generation: 1,
                },
                false,
            ));
            assert!(super::reveal_overlay_window(
                window,
                super::OverlayLaunch {
                    focus: true,
                    deferred_show: false,
                    generation: 1,
                },
                true,
            ));
        });
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn prewarmed_overlay_window_is_reused(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));

        let display_id = cx.read(|cx| cx.primary_display().expect("primary display").id());
        let display_bounds = cx.read(|cx| cx.primary_display().expect("primary display").bounds());
        cx.update(super::prewarm);
        let window_count = cx.read(|cx| cx.windows().len());

        cx.update(|cx| {
            let service = crate::state::state(cx);
            let opened = super::AreaOverlay::open(
                service,
                display_id,
                display_bounds,
                crate::capture::intent::CaptureIntent::Screenshot,
                super::OverlayLaunch {
                    focus: false,
                    deferred_show: true,
                    generation: 0,
                },
                cx,
            )
            .expect("open pooled overlay");
            assert_eq!(
                opened
                    .update(cx, |overlay, _, _| (
                        overlay.window_list_generation,
                        overlay.color_frame_generation,
                    ))
                    .expect("read pooled generations"),
                (1, 1)
            );
            assert_eq!(cx.windows().len(), window_count);
            assert!(super::session(cx)
                .pooled
                .iter()
                .all(|overlay| overlay.display_id != display_id));
            assert_eq!(super::session(cx).handles.len(), 1);
            let (window_list_generation, color_frame_generation) = opened
                .update(cx, |overlay, _, _| {
                    overlay.picking_windows = true;
                    overlay.picking_color = true;
                    overlay.rect = Some(crate::capture::selection::Rect {
                        x: 100.,
                        y: 100.,
                        width: 200.,
                        height: 100.,
                    });
                    overlay.interaction = Some(super::Interaction::Creating {
                        start: crate::capture::selection::Point { x: 100., y: 100. },
                    });
                    (
                        overlay.window_list_generation,
                        overlay.color_frame_generation,
                    )
                })
                .expect("prepare pending work");
            assert_eq!(super::replace_all(cx), Some(0));
            opened
                .update(cx, |overlay, _, _| {
                    assert!(overlay.rect.is_none(), "parked selections must be cleared");
                    assert!(
                        overlay.interaction.is_none(),
                        "parked gestures must be cleared"
                    );
                })
                .unwrap();
            assert!(super::session(cx)
                .pooled
                .iter()
                .any(|overlay| overlay.display_id == display_id));
            assert!(!opened
                .update(cx, |overlay, _, cx| {
                    overlay.apply_window_list(window_list_generation, Vec::new(), cx)
                })
                .expect("reject stale window list"));
            assert!(!opened
                .update(cx, |overlay, _, _| {
                    overlay.accepts_color_frame(color_frame_generation)
                })
                .expect("reject stale color frame"));
            let reopened = super::AreaOverlay::open(
                crate::state::state(cx),
                display_id,
                display_bounds,
                crate::capture::intent::CaptureIntent::Screenshot,
                super::OverlayLaunch {
                    focus: false,
                    deferred_show: true,
                    generation: 0,
                },
                cx,
            )
            .expect("reopen pooled overlay");
            assert_eq!(
                reopened
                    .update(cx, |overlay, _, _| (
                        overlay.window_list_generation,
                        overlay.color_frame_generation,
                    ))
                    .expect("read reopened generations"),
                (3, 3)
            );
            assert_eq!(cx.windows().len(), window_count);
            super::close_all(cx);
        });
        cx.run_until_parked();

        cx.update(|cx| {
            assert!(super::session(cx)
                .pooled
                .iter()
                .any(|overlay| overlay.display_id == display_id));
        });
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn parked_overlay_clears_surface_on_finish_before_reopen(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));

        let display_id = cx.read(|cx| cx.primary_display().expect("primary display").id());
        let display_bounds = cx.read(|cx| cx.primary_display().expect("primary display").bounds());
        cx.update(super::prewarm);

        let opened = cx.update(|cx| {
            let service = crate::state::state(cx);
            let opened = super::AreaOverlay::open_all_in_one(
                service,
                display_id,
                display_bounds,
                crate::capture::all_in_one::Choices::default(),
                super::OverlayLaunch {
                    focus: false,
                    deferred_show: true,
                    generation: 0,
                },
                cx,
            )
            .expect("open pooled overlay");
            let any_window: gpui::AnyWindowHandle = opened.into();
            opened
                .update(cx, |overlay, _, _| {
                    overlay.rect = Some(crate::capture::selection::Rect {
                        x: 40.,
                        y: 40.,
                        width: 160.,
                        height: 90.,
                    });
                })
                .unwrap();
            any_window
                .update(cx, |_, window, cx| {
                    window.draw(cx).clear(cx);
                    assert!(
                        !window.painted_quads().is_empty(),
                        "active selection should paint quads"
                    );
                })
                .unwrap();
            opened
                .update(cx, |overlay, _, _| {
                    assert_eq!(overlay.surface_state, super::PooledSurfaceState::Live);
                    overlay.screen_picker = true;
                    overlay.hovered_window = Some(0);
                })
                .unwrap();
            super::close_all(cx);
            opened
        });
        cx.run_until_parked();

        let any_window: gpui::AnyWindowHandle = opened.into();

        cx.update(|cx| {
            opened
                .update(cx, |overlay, _, _| {
                    assert!(overlay.parked);
                    assert!(overlay.rect.is_none());
                    assert!(overlay.all_in_one.is_none());
                    assert!(!overlay.screen_picker);
                    assert!(overlay.windows.is_empty());
                    assert!(overlay.hovered_window.is_none());
                    assert_eq!(overlay.surface_state, super::PooledSurfaceState::Dirty);
                })
                .unwrap();

            any_window
                .update(cx, |_, window, cx| {
                    window.simulate_next_frame(cx);
                    window.draw(cx).clear(cx);
                    assert!(
                        window.painted_quads().is_empty(),
                        "parked finish frame must clear rendered selection"
                    );
                })
                .unwrap();
            opened
                .update(cx, |overlay, _, _| {
                    assert_eq!(overlay.surface_state, super::PooledSurfaceState::Dirty);
                })
                .unwrap();

            any_window
                .update(cx, |_, window, cx| {
                    window.simulate_next_frame(cx);
                })
                .unwrap();
            opened
                .update(cx, |overlay, _, _| {
                    assert_eq!(overlay.surface_state, super::PooledSurfaceState::Clean);
                })
                .unwrap();

            let reopened = super::AreaOverlay::open(
                crate::state::state(cx),
                display_id,
                display_bounds,
                crate::capture::intent::CaptureIntent::Screenshot,
                super::OverlayLaunch {
                    focus: false,
                    deferred_show: true,
                    generation: 0,
                },
                cx,
            )
            .expect("reopen cleaned overlay");
            assert_eq!(reopened, opened);
            reopened
                .update(cx, |overlay, _, _| {
                    assert!(!overlay.parked);
                    assert!(overlay.rect.is_none());
                    assert_eq!(overlay.surface_state, super::PooledSurfaceState::Clean);
                })
                .unwrap();
            super::raise_all(0, cx);
            reopened
                .update(cx, |overlay, _, _| {
                    assert_eq!(overlay.surface_state, super::PooledSurfaceState::Live);
                })
                .unwrap();
            super::close_all(cx);
        });
        cx.run_until_parked();
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn rapid_reopen_before_park_paint_gates_reveal_until_first_frame(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));

        let display_id = cx.read(|cx| cx.primary_display().expect("primary display").id());
        let display_bounds = cx.read(|cx| cx.primary_display().expect("primary display").bounds());
        cx.update(super::prewarm);

        cx.update(|cx| {
            let opened = super::AreaOverlay::open(
                crate::state::state(cx),
                display_id,
                display_bounds,
                crate::capture::intent::CaptureIntent::Screenshot,
                super::OverlayLaunch {
                    focus: false,
                    deferred_show: true,
                    generation: 0,
                },
                cx,
            )
            .expect("open pooled overlay");
            let any_window: gpui::AnyWindowHandle = opened.into();
            opened
                .update(cx, |overlay, _, _| {
                    overlay.rect = Some(crate::capture::selection::Rect {
                        x: 20.,
                        y: 20.,
                        width: 120.,
                        height: 80.,
                    });
                })
                .unwrap();
            any_window
                .update(cx, |_, window, cx| {
                    window.draw(cx).clear(cx);
                })
                .unwrap();

            super::replace_all(cx);

            let reopened = super::AreaOverlay::open(
                crate::state::state(cx),
                display_id,
                display_bounds,
                crate::capture::intent::CaptureIntent::Screenshot,
                super::OverlayLaunch {
                    focus: false,
                    deferred_show: true,
                    generation: 0,
                },
                cx,
            )
            .expect("reopen before park frame");
            assert_eq!(reopened, opened);
            reopened
                .update(cx, |overlay, _, _| {
                    assert!(!overlay.parked);
                    assert!(overlay.rect.is_none());
                    assert_eq!(overlay.surface_state, super::PooledSurfaceState::Dirty);
                })
                .unwrap();
            super::raise_all(0, cx);
            reopened
                .update(cx, |overlay, _, _| {
                    assert_eq!(
                        overlay.surface_state,
                        super::PooledSurfaceState::AwaitingFirstPaint
                    );
                })
                .unwrap();

            any_window
                .update(cx, |_, window, cx| {
                    window.simulate_next_frame(cx);
                    window.draw(cx).clear(cx);
                })
                .unwrap();
            reopened
                .update(cx, |overlay, _, _| {
                    assert!(!overlay.parked);
                    assert_eq!(
                        overlay.surface_state,
                        super::PooledSurfaceState::AwaitingFirstPaint
                    );
                })
                .unwrap();

            any_window
                .update(cx, |_, window, cx| {
                    window.simulate_next_frame(cx);
                })
                .unwrap();
            reopened
                .update(cx, |overlay, _, _| {
                    assert!(!overlay.parked);
                    assert_eq!(overlay.surface_state, super::PooledSurfaceState::Live);
                })
                .unwrap();
            super::close_all(cx);
        });
        cx.run_until_parked();
    }

    #[herogpui::test]
    fn pending_window_list_does_not_start_an_area_selection(cx: &mut TestAppContext) {
        use gpui::{point, Modifiers, MouseButton};
        use herogpui::gpui;

        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let (overlay, cx) = cx.add_window_view(|window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            super::AreaOverlay::with_focus(
                bounds,
                1.0,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                focus,
            )
            .with_windows(Vec::new())
        });
        cx.refresh().expect("schedule a redraw");
        cx.run_until_parked();

        cx.simulate_mouse_down(
            point(px(100.0), px(100.0)),
            MouseButton::Left,
            Modifiers::none(),
        );

        assert!(overlay.read_with(cx, |overlay, _| {
            overlay.is_picking_windows() && overlay.interaction.is_none() && overlay.rect.is_none()
        }));
    }

    #[test]
    fn window_lists_follow_the_overlay_generation_and_skip_ocr() {
        use crate::capture::all_in_one::{Choices, Mode, Target};

        assert!(super::accepts_window_list(
            Some(Choices {
                mode: Mode::Screenshot,
                target: Target::Window,
            }),
            2,
            2,
        ));
        assert!(!super::accepts_window_list(
            Some(Choices {
                mode: Mode::Screenshot,
                target: Target::Window,
            }),
            3,
            2,
        ));
        assert!(!super::accepts_window_list(
            Some(Choices {
                mode: Mode::Ocr,
                target: Target::Area,
            }),
            2,
            2,
        ));
        assert!(super::accepts_window_list(
            Some(Choices {
                mode: Mode::Record,
                target: Target::Screen,
            }),
            2,
            2,
        ));
    }

    #[herogpui::test]
    fn switching_to_the_window_target_reuses_a_prefetched_list(cx: &mut TestAppContext) {
        use crate::capture::all_in_one::{Choices, Mode, Target};
        use crate::capture::windows_list::{Rect, WindowListItem};

        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let (overlay, cx) = cx.add_window_view(|window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            let mut overlay = super::AreaOverlay::with_focus(
                bounds,
                1.0,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                focus,
            )
            .with_all_in_one(Choices {
                mode: Mode::Screenshot,
                target: Target::Area,
            });
            overlay.windows = vec![WindowListItem {
                window_id: 7,
                title: "Docs".into(),
                owner_name: "Browser".into(),
                owner_pid: 1,
                bounds: Rect {
                    x: 10.0,
                    y: 10.0,
                    width: 300.0,
                    height: 200.0,
                },
            }];
            overlay
        });
        cx.refresh().expect("schedule a redraw");
        cx.run_until_parked();
        let generation = overlay.read_with(cx, |overlay, _| overlay.window_list_generation());
        overlay.update(cx, |overlay, cx| {
            overlay.set_all_in_one_target(Target::Window, cx);
        });
        overlay.read_with(cx, |overlay, _| {
            assert!(overlay.is_picking_windows());
            assert!(overlay.window_list_generation() > generation);
            assert_eq!(overlay.windows.len(), 1);
            assert_eq!(overlay.windows[0].window_id, 7);
        });
    }

    #[herogpui::test]
    fn escape_closes_the_focused_overlay(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            super::init_bindings(cx);
        });
        let service = cx.read(crate::state::state);
        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let window = cx.add_window(|window, cx| {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            super::AreaOverlay::with_focus(
                bounds,
                1.0,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                focus_handle,
            )
        });
        cx.refresh().expect("schedule a redraw");
        cx.run_until_parked();

        cx.simulate_keystrokes(window.into(), "escape");

        assert!(window.update(cx, |_, _, _| ()).is_err());
    }

    #[herogpui::test]
    fn all_in_one_area_capture_closes_the_overlay(cx: &mut TestAppContext) {
        use gpui::{point, Modifiers, MouseButton};
        use herogpui::gpui;

        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        config.update(|settings| settings.screenshot.hide_desktop_icons = false);
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let (_overlay, cx) = cx.add_window_view(|window, cx| {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            super::AreaOverlay::with_focus(
                bounds,
                1.0,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                focus_handle,
            )
            .with_all_in_one(crate::capture::all_in_one::Choices::default())
        });
        cx.refresh().expect("schedule a redraw");
        cx.run_until_parked();

        cx.simulate_mouse_down(
            point(px(100.0), px(100.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_move(
            point(px(300.0), px(250.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_up(
            point(px(300.0), px(250.0)),
            MouseButton::Left,
            Modifiers::none(),
        );

        assert!(cx.read(|cx| {
            cx.windows()
                .into_iter()
                .all(|window| window.downcast::<super::AreaOverlay>().is_none())
        }));
    }

    #[herogpui::test]
    fn all_in_one_toolbar_click_does_not_start_a_selection(cx: &mut TestAppContext) {
        use gpui::{point, Modifiers, MouseButton};

        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let bounds = gpui::Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let (overlay, cx) = cx.add_window_view(|window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            super::AreaOverlay::with_focus(
                bounds,
                1.0,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                focus,
            )
            .with_all_in_one(crate::capture::all_in_one::Choices::default())
        });
        cx.simulate_resize(bounds.size);
        cx.refresh().expect("schedule a redraw");
        cx.run_until_parked();

        cx.simulate_mouse_down(
            point(px(400.0), px(chrome::overlay_toolbar_top() + 22.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        assert!(overlay.read_with(cx, |overlay, _| overlay.interaction.is_none()));
    }

    #[herogpui::test]
    fn video_area_selection_opens_the_recording_control(cx: &mut TestAppContext) {
        use gpui::{point, Modifiers, MouseButton};
        use herogpui::gpui;

        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let (overlay, cx) = cx.add_window_view(|window, cx| {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            super::AreaOverlay::with_focus(
                bounds,
                1.0,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                focus_handle,
            )
            .with_all_in_one(crate::capture::all_in_one::Choices::default())
        });
        overlay.update(cx, |overlay, cx| {
            overlay.set_all_in_one_mode(crate::capture::all_in_one::Mode::Record, cx)
        });
        cx.refresh().expect("schedule a redraw");
        cx.run_until_parked();

        cx.simulate_mouse_down(
            point(px(100.0), px(100.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_move(
            point(px(300.0), px(250.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_up(
            point(px(300.0), px(250.0)),
            MouseButton::Left,
            Modifiers::none(),
        );

        assert!(cx.read(|cx| cx.windows().into_iter().any(|window| window
            .downcast::<crate::windows::recording_control::RecordingControl>()
            .is_some())));
    }

    #[herogpui::test]
    fn video_window_selection_releases_the_freeze_and_removes_the_selector(
        cx: &mut TestAppContext,
    ) {
        use crate::capture::all_in_one::{Choices, Mode, Target};
        use crate::capture::windows_list::{Rect, WindowListItem};

        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let generation = service.begin_freeze();
        service
            .finish_freeze(generation, Ok(()))
            .expect("finish freeze");
        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let overlay = cx.add_window(|window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            super::AreaOverlay::with_focus(
                bounds,
                1.0,
                service,
                crate::capture::intent::CaptureIntent::Recording,
                focus,
            )
            .with_all_in_one(Choices {
                mode: Mode::Record,
                target: Target::Window,
            })
            .with_windows(vec![WindowListItem {
                window_id: 42,
                title: "Video".into(),
                owner_name: "Player".into(),
                owner_pid: 7,
                bounds: Rect {
                    x: 100.0,
                    y: 100.0,
                    width: 640.0,
                    height: 360.0,
                },
            }])
        });
        cx.update(|cx| {
            let display_id = cx.primary_display().expect("primary display").id();
            super::track_overlay(overlay.into(), display_id, generation, true, cx);
        });

        overlay
            .update(cx, |overlay, window, cx| {
                overlay.on_move(
                    &gpui::MouseMoveEvent {
                        position: gpui::point(px(200.0), px(200.0)),
                        ..Default::default()
                    },
                    window,
                    cx,
                );
                overlay.on_down(
                    &gpui::MouseDownEvent {
                        position: gpui::point(px(780.0), px(580.0)),
                        ..Default::default()
                    },
                    window,
                    cx,
                );
            })
            .expect("click outside the hovered window");
        cx.run_until_parked();
        assert!(overlay.update(cx, |_, _, _| ()).is_ok());
        overlay
            .update(cx, |overlay, window, cx| {
                overlay.on_down(
                    &gpui::MouseDownEvent {
                        position: gpui::point(px(300.0), px(300.0)),
                        ..Default::default()
                    },
                    window,
                    cx,
                );
            })
            .expect("confirm from the click without another move");
        cx.run_until_parked();

        assert!(overlay.update(cx, |_, _, _| ()).is_err());
        cx.update(|cx| {
            assert_eq!(super::session(cx).handles.len(), 1);
            assert_eq!(super::session(cx).handles[0].generation, 0);
            assert_eq!(super::session(cx).outlined_window_id, Some(42));
            assert!(cx.windows().into_iter().any(|window| window
                .downcast::<crate::windows::recording_control::RecordingControl>()
                .is_some()));
            #[cfg(target_os = "macos")]
            {
                assert!(super::session(cx).native_recording_overlay_requested);
                assert!(
                    crate::windows::recording_control::RecordingControl::cancel_pre_recording(cx)
                );
                assert_eq!(super::session(cx).outlined_window_id, None);
            }
        });
        #[cfg(target_os = "macos")]
        {
            cx.run_until_parked();
            cx.update(|cx| {
                assert!(!super::session(cx).native_recording_overlay_requested);
            });
        }
    }

    #[herogpui::test]
    fn all_in_one_mode_and_intent_stay_in_sync(cx: &mut TestAppContext) {
        use crate::capture::all_in_one::{Choices, Mode, Target};
        use crate::capture::intent::CaptureIntent;

        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let (overlay, cx) = cx.add_window_view(|window, cx| {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            super::AreaOverlay::with_focus(
                bounds,
                1.0,
                service,
                CaptureIntent::Screenshot,
                focus_handle,
            )
            .with_all_in_one(Choices {
                mode: Mode::Record,
                target: Target::Screen,
            })
        });
        overlay.update(cx, |overlay, cx| overlay.apply_all_in_one_target(cx));

        assert_eq!(
            overlay.read_with(cx, |overlay, _| overlay.intent),
            CaptureIntent::Recording
        );
        assert!(overlay.read_with(cx, |overlay, _| overlay.is_picking_screen()));
        assert_eq!(
            overlay.read_with(cx, |overlay, _| overlay.cursor),
            gpui::CursorStyle::default()
        );
        assert_eq!(
            overlay.read_with(cx, |overlay, _| (
                overlay.screen_picker_dimmed(false),
                overlay.screen_picker_dimmed(true),
            )),
            (Some(true), Some(false))
        );
        overlay.update(cx, |overlay, cx| overlay.set_all_in_one_mode(Mode::Ocr, cx));
        assert_eq!(
            overlay.read_with(cx, |overlay, _| overlay.intent),
            CaptureIntent::Ocr
        );
        assert_eq!(
            overlay.read_with(cx, |overlay, _| overlay.all_in_one.expect("choices").target),
            Target::Area
        );
        overlay.update(cx, |overlay, cx| {
            overlay.set_all_in_one_mode(Mode::Record, cx)
        });
        assert_eq!(
            overlay.read_with(cx, |overlay, _| overlay.intent),
            CaptureIntent::Recording
        );
        assert_eq!(
            overlay.read_with(cx, |overlay, _| overlay.all_in_one.expect("choices").target),
            Target::Screen
        );
    }

    #[herogpui::test]
    fn dedicated_display_picker_reuses_the_screen_target_state(cx: &mut TestAppContext) {
        use crate::capture::intent::CaptureIntent;

        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let (overlay, cx) = cx.add_window_view(|window, cx| {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle, cx);
            super::AreaOverlay::with_focus(
                bounds,
                1.0,
                service,
                CaptureIntent::Screenshot,
                focus_handle,
            )
            .with_screen_picker()
        });

        assert!(overlay.read_with(cx, |overlay, _| overlay.is_picking_screen()));
        assert_eq!(
            overlay.read_with(cx, |overlay, _| overlay.screen_picker_dimmed(false)),
            Some(true)
        );
        assert_eq!(overlay.read_with(cx, |overlay, _| overlay.all_in_one), None);
        assert_eq!(
            overlay.read_with(cx, |overlay, _| overlay.cursor),
            gpui::CursorStyle::default()
        );
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn unfocused_display_picker_highlights_pointer_entry_and_exit(cx: &mut TestAppContext) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        cx.update(|cx| {
            herogpui::init(cx);
            crate::state::set_test_state(cx, config);
        });
        let service = cx.read(crate::state::state);
        let (_, visual) = herogpui::test::open_window(cx, move |_, cx| {
            super::AreaOverlay::with_focus(
                gpui::Bounds::new(gpui::point(px(0.), px(0.)), size(px(800.), px(600.))),
                1.,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                cx.focus_handle(),
            )
            .with_screen_picker()
        });
        visual.simulate_resize(size(px(800.), px(600.)));
        visual.settle();
        assert!(!visual.update(|window, _| window.is_window_hovered()));
        assert!(visual.debug_bounds("display-picker-dim").is_some());

        for inside in [true, false, true] {
            if inside {
                visual.simulate_mouse_move(
                    gpui::point(px(100.), px(100.)),
                    None,
                    gpui::Modifiers::none(),
                );
            } else {
                visual.simulate_event(gpui::MouseExitEvent {
                    position: gpui::point(px(801.), px(100.)),
                    pressed_button: None,
                    modifiers: gpui::Modifiers::none(),
                });
            }
            visual.settle();
            assert_eq!(visual.debug_bounds("display-picker-dim").is_none(), inside);
        }
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn unfocused_window_picker_highlights_target_after_list_arrives(cx: &mut TestAppContext) {
        use crate::capture::windows_list::{Rect, WindowListItem};
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        cx.update(|cx| {
            herogpui::init(cx);
            crate::state::set_test_state(cx, config);
        });
        let service = cx.read(crate::state::state);
        let (overlay, visual) = herogpui::test::open_window(cx, move |_, cx| {
            super::AreaOverlay::with_focus(
                gpui::Bounds::new(gpui::point(px(0.), px(0.)), size(px(800.), px(600.))),
                1.,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                cx.focus_handle(),
            )
            .with_windows(Vec::new())
        });
        visual.simulate_resize(size(px(800.), px(600.)));
        visual.settle();
        visual.simulate_mouse_move(
            gpui::point(px(100.), px(100.)),
            None,
            gpui::Modifiers::none(),
        );
        visual.settle();
        assert!(visual.debug_bounds("window-picker-dim").is_some());
        visual.update(|_, cx| {
            overlay.update(cx, |overlay, cx| {
                assert!(overlay.apply_window_list(
                    overlay.window_list_generation(),
                    vec![WindowListItem {
                        window_id: 42,
                        title: "Target".into(),
                        owner_name: "App".into(),
                        owner_pid: 1,
                        bounds: Rect {
                            x: 50.,
                            y: 50.,
                            width: 200.,
                            height: 200.,
                        },
                    }],
                    cx,
                ));
            });
        });
        visual.settle();
        assert!(visual.debug_bounds("window-picker-dim").is_none());
        visual.simulate_event(gpui::MouseExitEvent {
            position: gpui::point(px(801.), px(100.)),
            pressed_button: None,
            modifiers: gpui::Modifiers::none(),
        });
        visual.settle();
        assert!(visual.debug_bounds("window-picker-dim").is_some());
        visual.simulate_mouse_move(
            gpui::point(px(100.), px(100.)),
            None,
            gpui::Modifiers::none(),
        );
        visual.settle();
        assert!(visual.debug_bounds("window-picker-dim").is_none());
    }

    #[herogpui::test]
    fn all_in_one_mode_syncs_across_overlay_windows(cx: &mut TestAppContext) {
        use crate::capture::all_in_one::{Choices, Mode};
        use crate::capture::intent::CaptureIntent;

        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let first = cx.add_window(|window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            super::AreaOverlay::with_focus(
                bounds,
                1.0,
                service.clone(),
                CaptureIntent::Screenshot,
                focus,
            )
            .with_all_in_one(Choices::default())
        });
        let second = cx.add_window(|window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            super::AreaOverlay::with_focus(bounds, 1.0, service, CaptureIntent::Screenshot, focus)
                .with_all_in_one(Choices::default())
        });
        cx.update(|cx| {
            let display_id = cx.primary_display().expect("primary display").id();
            super::track_overlay(first.into(), display_id, 1, true, cx);
            super::track_overlay(second.into(), display_id, 1, false, cx);
        });

        first
            .update(cx, |overlay, _window, cx| {
                overlay.set_all_in_one_mode(Mode::Record, cx);
            })
            .expect("update first overlay");
        cx.run_until_parked();

        let (choices, intent) = second
            .update(cx, |overlay, _window, _cx| {
                (overlay.all_in_one.expect("choices"), overlay.intent)
            })
            .expect("read second overlay");
        assert_eq!(choices.mode, Mode::Record);
        assert_eq!(intent, CaptureIntent::Recording);
    }

    #[herogpui::test]
    fn color_picker_is_exclusive_and_escape_leaves_the_overlay_open(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            super::init_bindings(cx);
        });
        let service = cx.read(crate::state::state);
        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let first = cx.add_window(|window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            super::AreaOverlay::with_focus(
                bounds,
                1.0,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                focus,
            )
            .with_all_in_one(crate::capture::all_in_one::Choices::default())
        });
        cx.refresh().expect("schedule the initial redraw");
        cx.run_until_parked();
        first
            .update(cx, |overlay, window, cx| {
                let toolbar_focus = cx.focus_handle();
                window.focus(&toolbar_focus, cx);
                overlay.start_color_picker(window, cx);
                assert!(overlay.focus_handle.is_focused(window));
            })
            .expect("start color picker");

        assert!(first
            .update(cx, |overlay, _window, _cx| {
                overlay.picking_color && overlay.all_in_one.is_some()
            })
            .expect("read picker state"));
        first
            .update(cx, |overlay, _window, cx| {
                overlay.set_all_in_one_target(crate::capture::all_in_one::Target::Screen, cx);
                assert!(!overlay.picking_color);
            })
            .expect("select another target");
        first
            .update(cx, |overlay, window, cx| {
                overlay.start_color_picker(window, cx);
                overlay.set_all_in_one_mode(crate::capture::all_in_one::Mode::Record, cx);
                assert!(!overlay.picking_color);
                overlay.start_color_picker(window, cx);
            })
            .expect("select another mode");
        cx.simulate_keystrokes(first.into(), "escape");
        cx.run_until_parked();

        assert!(first
            .update(cx, |overlay, _window, _cx| !overlay.picking_color)
            .expect("overlay stays open after leaving pick mode"));
        cx.simulate_keystrokes(first.into(), "escape");
        cx.run_until_parked();

        assert!(first.update(cx, |_, _, _| ()).is_err());
    }

    #[herogpui::test]
    fn color_picker_card_click_copies_the_display_pixel(cx: &mut TestAppContext) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let (overlay, visual) = herogpui::test::open_window(cx, |_, cx| {
            super::AreaOverlay::with_focus(
                gpui::Bounds::new(gpui::point(px(0.), px(0.)), size(px(400.), px(300.))),
                1.,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                cx.focus_handle(),
            )
        });
        visual.simulate_resize(size(px(400.), px(300.)));
        visual.update(|_, cx| {
            overlay.update(cx, |overlay, cx| {
                overlay.picking_color = true;
                overlay.pointer = Some(gpui::point(px(100.), px(100.)));
                overlay.color_frame =
                    Some(Arc::new(image::RgbaImage::from_fn(800, 600, |x, y| {
                        image::Rgba(match (x >= 400, y >= 300) {
                            (false, false) => [240, 80, 64, 255],
                            (true, false) => [64, 112, 192, 255],
                            (false, true) => [48, 160, 104, 255],
                            (true, true) => [184, 96, 192, 255],
                        })
                    })));
                cx.notify();
            });
        });
        let card = visual.expect("color-picker-card");
        assert!(card.center().x > px(200.) && card.center().y > px(150.));
        visual.simulate_mouse_down(
            card.center(),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        assert_eq!(
            cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
            Some("#b860c0".to_string())
        );
    }

    #[herogpui::test]
    fn selection_uses_the_release_position_when_moving_and_resizing(cx: &mut TestAppContext) {
        use gpui::{point, Modifiers, MouseButton};

        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let (overlay, cx) = cx.add_window_view(|_, cx| {
            super::AreaOverlay::with_focus(
                gpui::Bounds::new(point(px(-800.), px(0.)), size(px(800.), px(600.))),
                1.5,
                service,
                crate::capture::intent::CaptureIntent::Screenshot,
                cx.focus_handle(),
            )
        });
        for (start, moved, released, expected) in [
            (
                (150., 150.),
                (160., 160.),
                (175., 180.),
                (125., 130., 200., 150.),
            ),
            (
                (300., 250.),
                (310., 260.),
                (330., 280.),
                (100., 100., 230., 180.),
            ),
        ] {
            overlay.update(cx, |overlay, cx| {
                overlay.rect = Some(super::selection::Rect {
                    x: 100.,
                    y: 100.,
                    width: 200.,
                    height: 150.,
                });
                cx.notify();
            });
            cx.refresh().unwrap();
            cx.run_until_parked();
            cx.simulate_mouse_down(
                point(px(start.0), px(start.1)),
                MouseButton::Left,
                Modifiers::none(),
            );
            cx.simulate_mouse_move(
                point(px(moved.0), px(moved.1)),
                MouseButton::Left,
                Modifiers::none(),
            );
            cx.simulate_mouse_up(
                point(px(released.0), px(released.1)),
                MouseButton::Left,
                Modifiers::none(),
            );
            let rect = overlay.read_with(cx, |overlay, _| overlay.rect.unwrap());
            assert_eq!((rect.x, rect.y, rect.width, rect.height), expected);
        }
    }
}
