//! Area selection overlay — a fullscreen transparent window per the ported
//! `area-overlay` flow: dimmed screen, drag to draw the selection, Esc to
//! cancel, release to confirm and capture.

use gpui::{
    actions, div, prelude::*, px, AnyWindowHandle, App, Bounds, Context, DisplayId, Global,
    KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Render,
    Styled, Window, WindowBackgroundAppearance, WindowKind, WindowOptions,
};
#[cfg(not(test))]
use poratake_daemon_common::contract::RecordingOverlayShowWindowRequest;
use poratake_daemon_common::geometry::{CaptureRect, DisplayOrigin};

use crate::capture::selection;
use crate::capture::CaptureService;
use crate::theme::color::Srgba;

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
    #[cfg(windows)]
    pooled: Vec<PooledOverlay>,
    outlined_window_id: Option<i64>,
}

#[derive(Clone)]
struct TrackedOverlay {
    handle: AnyWindowHandle,
    #[cfg(windows)]
    display_id: DisplayId,
    generation: u64,
    #[cfg(windows)]
    focus: bool,
}

#[cfg(windows)]
struct PooledOverlay {
    handle: gpui::WindowHandle<AreaOverlay>,
    display_id: DisplayId,
}

impl Global for OverlaySession {}

fn session(cx: &mut App) -> &mut OverlaySession {
    cx.default_global::<OverlaySession>()
}

pub fn close_all(cx: &mut App) {
    if let Some(generation) = remove_all(cx, true) {
        release_frozen_screen(generation, cx);
    }
}

pub fn release_frozen_for_recording(cx: &mut App) -> bool {
    let generation = session(cx)
        .handles
        .iter()
        .map(|tracked| tracked.generation)
        .max()
        .unwrap_or(0);
    if !crate::state::state(cx).release_screen(generation) {
        return false;
    }
    for tracked in &mut session(cx).handles {
        tracked.generation = 0;
    }
    true
}

pub fn begin_recording_handoff(rect: ScreenRect, cx: &mut App) -> bool {
    if session(cx).outlined_window_id.is_some() {
        return true;
    }

    #[cfg(target_os = "macos")]
    {
        let daemon = crate::state::state(cx).daemon;
        if daemon.recording_overlay().show(rect).ok() != Some(true) {
            return false;
        }
        close_all(cx);
        return true;
    }

    #[cfg(not(target_os = "macos"))]
    {
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
        true
    }
}

pub fn end_recording_handoff(cx: &mut App) {
    session(cx).outlined_window_id = None;
    #[cfg(not(test))]
    {
        let daemon = crate::state::state(cx).daemon;
        let _ = daemon.recording_overlay().hide();
    }
    close_all(cx);
}

pub(crate) fn show_window_recording_outline(window_id: i64, cx: &mut App) -> bool {
    if session(cx).outlined_window_id == Some(window_id) {
        return true;
    }

    #[cfg(test)]
    let visible = true;
    #[cfg(not(test))]
    let visible = {
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
        service
            .daemon
            .recording_overlay()
            .show_window(request)
            .unwrap_or(false)
    };

    if visible {
        session(cx).outlined_window_id = Some(window_id);
    }
    visible
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
    let handles = std::mem::take(&mut session(cx).handles);
    let generation = handles.iter().map(|tracked| tracked.generation).max();
    if deferred {
        App::defer(cx, move |cx| dispose_overlays(handles, cx));
    } else {
        dispose_overlays(handles, cx);
    }
    generation
}

fn dispose_overlays(handles: Vec<TrackedOverlay>, cx: &mut App) {
    for tracked in handles {
        #[cfg(windows)]
        if let Some(handle) = tracked.handle.downcast::<AreaOverlay>() {
            if handle
                .update(cx, |overlay, window, _| {
                    overlay.window_list_generation = overlay.window_list_generation.wrapping_add(1);
                    overlay.picking_windows = false;
                    overlay.color_frame_generation = overlay.color_frame_generation.wrapping_add(1);
                    overlay.picking_color = false;
                    overlay.color_frame = None;
                    park_overlay(window);
                })
                .is_ok()
            {
                session(cx).pooled.push(PooledOverlay {
                    handle,
                    display_id: tracked.display_id,
                });
                continue;
            }
        }
        let _ = tracked
            .handle
            .update(cx, |_, window, _| window.remove_window());
    }
}

#[cfg(windows)]
pub fn raise_all(generation: u64, cx: &mut App) {
    let handles = session(cx).handles.clone();
    for tracked in handles {
        if tracked.generation != generation {
            continue;
        }
        let _ = tracked.handle.update(cx, |_, window, _| {
            set_overlay_topmost(window);
            if tracked.focus {
                window.activate_window();
            }
        });
    }
}

/// Drops the daemon's frozen snapshot. Cheap and idempotent when the screen was
/// never frozen, which is why the callers do not track whether it was.
fn release_frozen_screen(generation: u64, cx: &mut App) {
    let service = crate::state::state(cx);
    cx.background_executor()
        .spawn(async move { service.release_screen(generation) })
        .detach();
}

pub(crate) fn dismiss(window: &mut Window, cx: &mut App) {
    let tracked = !session(cx).handles.is_empty();
    close_all(cx);
    if !cfg!(windows) || !tracked {
        window.remove_window();
    }
}

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
) -> WindowOptions {
    #[cfg(target_os = "macos")]
    let window_bounds = Bounds {
        origin: gpui::point(px(0.0), px(0.0)),
        size: display_bounds.size,
    };
    #[cfg(not(target_os = "macos"))]
    let window_bounds = display_bounds;
    WindowOptions {
        window_bounds: Some(gpui::WindowBounds::Windowed(window_bounds)),
        titlebar: None,
        focus,
        show: !cfg!(windows),
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(display_id),
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    }
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
fn park_overlay(window: &Window) {
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};

    set_overlay_hole(window, None);
    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}

#[cfg(all(windows, test))]
fn park_overlay(_window: &Window) {}

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

#[cfg(all(windows, not(test)))]
fn windows_build() -> u32 {
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

fn show_overlay_before_freeze(window: &Window) -> bool {
    #[cfg(windows)]
    {
        #[cfg(test)]
        {
            let _ = window;
            true
        }
        #[cfg(not(test))]
        {
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
            };

            let build = windows_build();
            if !can_show_before_freeze(build, true) {
                return false;
            }
            let Some(hwnd) = crate::windows::window_hwnd(window) else {
                return false;
            };
            let excluded =
                unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) }.is_ok();
            if !can_show_before_freeze(build, excluded) {
                return false;
            }
            show_noactivate_topmost(hwnd);
            true
        }
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        true
    }
}

#[cfg(any(windows, test))]
fn can_show_before_freeze(windows_build: u32, capture_excluded: bool) -> bool {
    windows_build >= 19041 && capture_excluded
}

fn track_overlay(
    handle: AnyWindowHandle,
    display_id: DisplayId,
    generation: u64,
    focus: bool,
    cx: &mut App,
) {
    #[cfg(not(windows))]
    let _ = (display_id, focus);
    session(cx).handles.push(TrackedOverlay {
        handle,
        #[cfg(windows)]
        display_id,
        generation,
        #[cfg(windows)]
        focus,
    });
}

#[cfg(windows)]
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

fn prepare_overlay_window(window: &Window, display_bounds: Bounds<Pixels>) {
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

fn reveal_overlay_window(window: &Window, launch: OverlayLaunch, active: bool) -> bool {
    if !active {
        return false;
    }
    if launch.deferred_show {
        return show_overlay_before_freeze(window);
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
    #[cfg(windows)]
    if active {
        if let Some(handle) = take_pooled_overlay(display_id, cx) {
            let updated = handle.update(cx, |overlay, window, cx| {
                prepare_overlay_window(window, display_bounds);
                let scale = window.scale_factor();
                let focus_handle = cx.focus_handle();
                let window_list_generation = overlay.window_list_generation.wrapping_add(1);
                let color_frame_generation = overlay.color_frame_generation.wrapping_add(1);
                let mut replacement = build(scale, focus_handle);
                replacement.display_id = Some(u32::from(display_id));
                replacement.window_list_generation = window_list_generation;
                replacement.color_frame_generation = color_frame_generation;
                *overlay = replacement;
                after_new(overlay, cx);
                cx.notify();
                let shown = reveal_overlay_window(window, launch, active);
                if launch.focus && shown {
                    window.activate_window();
                }
                window.focus(&overlay.focus_handle);
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

    let opened = cx.open_window(
        overlay_options(display_bounds, display_id, launch.focus),
        |window, cx| {
            prepare_overlay_window(window, display_bounds);
            let scale = window.scale_factor();
            let focus_handle = cx.focus_handle();
            let view = cx.new(|_| {
                let mut view = build(scale, focus_handle);
                view.display_id = Some(u32::from(display_id));
                view
            });
            view.update(cx, after_new);
            let shown = reveal_overlay_window(window, launch, active);
            if launch.focus && shown {
                window.activate_window();
            }
            window.focus(&view.read(cx).focus_handle);
            view
        },
    );
    match opened {
        Ok(handle) => {
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

#[cfg(windows)]
pub fn prewarm(cx: &mut App) {
    let service = crate::state::state(cx);
    for display in cx.displays() {
        let display_id = display.id();
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
) {
    crate::windows::recording_control::RecordingControl::open(
        cx,
        target,
        rect,
        display_id,
        window_id,
        target_name,
    );
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
    /// The live selection, which stays editable after it is drawn — the
    /// renderer keeps a box the user can move, resize and re-capture.
    rect: Option<selection::Rect>,
    interaction: Option<Interaction>,
    cursor: gpui::CursorStyle,
    screen_picker: bool,
    /// Set in all-in-one mode: the toolbar's current mode and target.
    all_in_one: Option<crate::capture::all_in_one::Choices>,
    all_in_one_targets: [crate::capture::all_in_one::Target; 2],
    picking_color: bool,
    color_frame: Option<std::sync::Arc<image::RgbaImage>>,
    color_frame_generation: u64,
    handed_off: bool,
    menu: crate::ui::menu::MenuHandle,
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
            rect: None,
            interaction: None,
            cursor: gpui::CursorStyle::Crosshair,
            intent,
            windows: Vec::new(),
            picking_windows: false,
            window_list_generation: 0,
            hovered_window: None,
            screen_picker: false,
            all_in_one: None,
            all_in_one_targets: [crate::capture::all_in_one::Target::Area; 2],
            picking_color: false,
            color_frame: None,
            color_frame_generation: 0,
            handed_off: false,
            menu: crate::ui::menu::MenuHandle::new(),
            service,
        }
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

    pub fn with_all_in_one(mut self, choices: crate::capture::all_in_one::Choices) -> Self {
        use crate::capture::all_in_one::{Mode, Target};

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

    fn confirm_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.hovered_window else {
            return;
        };
        let Some(picked) = self.windows.get(index).cloned() else {
            return;
        };
        if self.intent == crate::capture::intent::CaptureIntent::Recording {
            let rect = ScreenRect {
                x: picked.bounds.x as i32,
                y: picked.bounds.y as i32,
                width: picked.bounds.width as i32,
                height: picked.bounds.height as i32,
            };
            let label = picked.label();
            let window_id = picked.window_id;
            if !release_frozen_for_recording(cx) {
                crate::windows::toast::Toast::show(
                    cx,
                    "Recording failed",
                    "Could not release the frozen screen",
                );
                return;
            }
            begin_recording(
                cx,
                crate::video::recorder::RecordingTarget::Window,
                rect,
                self.display_id,
                Some(window_id),
                Some(label),
            );
            if !show_window_recording_outline(window_id, cx) {
                eprintln!("[recording-overlay] failed to outline window {window_id}");
            }
            self.all_in_one = None;
            self.picking_windows = false;
            self.hovered_window = None;
            self.interaction = None;
            self.pointer = None;
            self.handed_off = true;
            conceal_recording_selector(window);
            sync_window_recording_handoff(cx);
            cx.notify();
            return;
        }
        let coordinator = crate::state::coordinator(cx);
        coordinator.update(cx, |coordinator, cx| {
            coordinator.capture_window(picked.window_id, cx);
        });
        dismiss(window, cx);
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
            |this, cx| this.apply_all_in_one_target(cx),
            true,
            "the all-in-one overlay",
            cx,
        )
    }

    pub fn open_screen_picker(
        service: CaptureService,
        display_id: DisplayId,
        display_bounds: Bounds<Pixels>,
        launch: OverlayLaunch,
        cx: &mut App,
    ) -> Option<gpui::WindowHandle<AreaOverlay>> {
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

    pub fn close_all_in_one_menu(&self, window: &mut Window) {
        self.menu.close(window);
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
        self.window_list_generation = self.window_list_generation.wrapping_add(1);
        self.picking_windows = choices.target == Target::Window;
        if choices.target == Target::Window {
            self.windows.clear();
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
        } else {
            self.windows = Vec::new();
        }
        cx.notify();
    }

    pub fn start_color_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu.close(window);
        window.focus(&self.focus_handle);
        self.activate_color_picker(cx);
        sync_color_picker(true, cx);
    }

    fn stop_color_picker(&mut self, cx: &mut Context<Self>) {
        if !self.picking_color {
            return;
        }
        self.picking_color = false;
        self.color_frame = None;
        sync_color_picker(false, cx);
        cx.notify();
    }

    fn activate_color_picker(&mut self, cx: &mut Context<Self>) {
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
                        if let Some(reservation) = reservation.as_ref() {
                            reservation.wait_for_freeze();
                        }
                        let path = std::env::temp_dir().join(format!(
                            "poratake-color-frame-{}-{}-{}-{}.png",
                            std::process::id(),
                            rect.x,
                            rect.y,
                            generation
                        ));
                        let frame = service
                            .capture_area_to_file_with_options(capture, &path, freeze)
                            .ok()
                            .and_then(|_| image::open(&path).ok())
                            .map(|image| std::sync::Arc::new(image.to_rgba8()));
                        let _ = std::fs::remove_file(&path);
                        frame
                    })
                    .await;
                let _ = entity.update(cx, |this, cx| {
                    if !this.accepts_color_frame(generation) {
                        return;
                    }
                    let Some(frame) = frame else {
                        this.picking_color = false;
                        sync_color_picker(false, cx);
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
        crate::windows::toast::Toast::show(cx, "Color copied", hex);
        true
    }

    /// Window-pick variant of `open`.
    pub fn open_with_windows(
        service: CaptureService,
        display_id: DisplayId,
        display_bounds: Bounds<Pixels>,
        intent: crate::capture::intent::CaptureIntent,
        windows: Vec<crate::capture::windows_list::WindowListItem>,
        focus: bool,
        cx: &mut App,
    ) -> Option<gpui::WindowHandle<AreaOverlay>> {
        open_overlay_window(
            display_id,
            display_bounds,
            OverlayLaunch {
                focus,
                deferred_show: false,
                generation: 0,
            },
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

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((origin, size)) = self.selection() else {
            return;
        };
        if size.width < px(4.0) || size.height < px(4.0) {
            return;
        }

        let coordinate_scale = capture_coordinate_scale(self.scale);
        let x =
            (f32::from(self.display_bounds.left() + origin.x) * coordinate_scale).round() as i32;
        let y = (f32::from(self.display_bounds.top() + origin.y) * coordinate_scale).round() as i32;
        let width = (f32::from(size.width) * coordinate_scale).round() as i32;
        let height = (f32::from(size.height) * coordinate_scale).round() as i32;

        let intent = self.intent;
        let scale = self.scale;
        if retains_overlay_after_selection(intent) {
            self.all_in_one = None;
            sync_recording_handoff(cx);
            begin_recording(
                cx,
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
            );
            cx.notify();
            return;
        }

        let coordinator = crate::state::coordinator(cx);
        let display_origin = (
            (f32::from(self.display_bounds.left()) * coordinate_scale).round() as i32,
            (f32::from(self.display_bounds.top()) * coordinate_scale).round() as i32,
        );
        let display_id = self.display_id;
        let reservation = self
            .service
            .config
            .get()
            .screenshot
            .freeze_screen
            .then(|| self.service.reserve_cached_capture());
        dismiss(window, cx);
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
                    reservation,
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
                .bg(Srgba::parse("#000000").to_hsla().opacity(0.7))
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
    viewport: gpui::Size<Pixels>,
    dim: gpui::Hsla,
) -> Vec<gpui::AnyElement> {
    let right = origin.x + hole.width;
    let bottom = origin.y + hole.height;
    vec![
        div()
            .absolute()
            .left_0()
            .right_0()
            .top_0()
            .h(origin.y.max(px(0.0)))
            .bg(dim)
            .into_any_element(),
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(bottom)
            .h((viewport.height - bottom).max(px(0.0)))
            .bg(dim)
            .into_any_element(),
        div()
            .absolute()
            .left_0()
            .top(origin.y)
            .w(origin.x.max(px(0.0)))
            .h(hole.height)
            .bg(dim)
            .into_any_element(),
        div()
            .absolute()
            .left(right)
            .top(origin.y)
            .w((viewport.width - right).max(px(0.0)))
            .h(hole.height)
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
        && all_in_one.is_some_and(|choices| {
            choices.mode != crate::capture::all_in_one::Mode::Ocr
                && choices.target == crate::capture::all_in_one::Target::Window
        })
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

fn retains_overlay_after_selection(intent: crate::capture::intent::CaptureIntent) -> bool {
    intent == crate::capture::intent::CaptureIntent::Recording
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

fn sync_color_picker(active: bool, cx: &mut Context<AreaOverlay>) {
    for_each_area_overlay(cx, move |overlay, cx| {
        if overlay.all_in_one.is_none() || overlay.picking_color == active {
            return;
        }
        if active {
            overlay.activate_color_picker(cx);
            return;
        }
        overlay.picking_color = false;
        overlay.color_frame = None;
        cx.notify();
    });
}

fn sync_recording_handoff(cx: &mut Context<AreaOverlay>) {
    for_each_area_overlay(cx, move |overlay, cx| {
        if overlay.intent != crate::capture::intent::CaptureIntent::Recording
            || overlay.all_in_one.is_none()
        {
            return;
        }
        overlay.all_in_one = None;
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

fn conceal_recording_selector(window: &mut Window) {
    #[cfg(windows)]
    park_overlay(window);
    window.remove_window();
}

impl Render for AreaOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = crate::theme::vars::active_theme(cx);
        // The overlay's frame, handles and crosshairs are `border-primary` /
        // `bg-primary` / `bg-primary/70`, and `--primary` is the operating
        // system's accent rather than the theme's -- see `theme::vars`.
        let accent = theme.primary;

        let selection = self.selection();
        let dim = Srgba::parse("#000000")
            .to_hsla()
            .opacity(crate::ui::chrome::OVERLAY_DIM);

        let root = div()
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
                        viewport,
                        dim,
                    ))
                }
                None => root.child(div().absolute().inset_0().bg(dim)),
            };
        }

        let root = root
            .cursor(self.cursor)
            .on_action(cx.listener(|this, _: &Cancel, window, cx| {
                if this.intent == crate::capture::intent::CaptureIntent::Recording {
                    crate::windows::recording_control::RecordingControl::cancel_pre_recording(cx);
                }
                dismiss(window, cx);
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
            .on_mouse_move(cx.listener(Self::on_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up));

        let toolbar = self.all_in_one.map(|choices| {
            crate::capture::all_in_one_toolbar::render(
                choices,
                self.picking_color,
                &self.menu,
                &theme,
                window,
                cx,
            )
        });

        if self.picking_color {
            let mut picking = root.children(self.pointer.and_then(|pointer| {
                self.color_frame.as_ref().map(|frame| {
                    crate::capture::color_picker::render(
                        frame,
                        pointer,
                        window.viewport_size(),
                        &theme,
                    )
                })
            }));
            if let Some(toolbar) = toolbar {
                picking = picking.child(toolbar);
            }
            return picking;
        }

        if self.is_picking_windows() {
            // `SelectionScrim` takes the hovered target as its rect while
            // picking, so the candidate window shows through undimmed. The DOM
            // overlay draws neither a frame nor a name label in this mode.
            let viewport = window.viewport_size();
            let mut picking = root.cursor_pointer();
            let hovered_frame = if window.is_window_hovered() {
                self.hovered_frame()
            } else {
                None
            };
            picking = match hovered_frame {
                Some((origin, frame)) => {
                    picking.children(scrim_around(origin, frame, viewport, dim))
                }
                None => picking.child(div().absolute().inset_0().bg(dim)),
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

        if let Some(dimmed) = self.screen_picker_dimmed(window.is_window_hovered()) {
            let mut picking = if dimmed {
                root.child(div().absolute().inset_0().bg(dim))
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
                .child(prompt(self.intent.prompt(), self.all_in_one.is_some())),
            Some((origin, bounds_size)) => {
                let left_w = origin.x;
                let right_x = origin.x + bounds_size.width;
                let right_w = px(f32::from(window.viewport_size().width) - f32::from(right_x));
                let bottom_y = origin.y + bounds_size.height;
                let bottom_h = px(f32::from(window.viewport_size().height) - f32::from(bottom_y));

                root.child(div().absolute().left_0().top_0().w(left_w).h_full().bg(dim))
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .top_0()
                            .w(right_w.max(px(0.0)))
                            .h_full()
                            .bg(dim),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(origin.x)
                            .top_0()
                            .w(bounds_size.width)
                            .h(origin.y)
                            .bg(dim),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(origin.x)
                            .top(bottom_y)
                            .w(bounds_size.width)
                            .h(bottom_h.max(px(0.0)))
                            .bg(dim),
                    )
                    .child({
                        let mut frame = div()
                            .absolute()
                            .left(origin.x)
                            .top(origin.y)
                            .w(bounds_size.width)
                            .h(bounds_size.height)
                            .border_1()
                            .border_color(accent)
                            // `shadow-[0_0_0_1px_rgba(0,0,0,0.35)]` keeps the
                            // accent frame legible over light content.
                            .shadow(vec![gpui::BoxShadow {
                                color: crate::ui::colors::black(0.35),
                                offset: point(px(0.0), px(0.0)),
                                blur_radius: px(0.0),
                                spread_radius: px(1.0),
                            }]);
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
                                    }]),
                            );
                        }
                        frame
                    })
                    .child({
                        let viewport_h = f32::from(window.viewport_size().height);
                        let top = f32::from(origin.y);
                        let height = f32::from(bounds_size.height);
                        let label_y = crate::ui::chrome::overlay_label_top(top, height, viewport_h);
                        div()
                            .absolute()
                            .left(origin.x)
                            .w(bounds_size.width)
                            .top(px(label_y))
                            .flex()
                            .justify_center()
                            .child(
                                div()
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
        if self.picking_color {
            if self.pick_color(event.position, cx) {
                dismiss(window, cx);
            }
            return;
        }
        if self.is_picking_windows() {
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
        let scale = self.scale;
        let capture = display_capture(self.display_bounds, scale, self.display_id);
        let rect = capture.rect;
        let intent = self.intent;
        if retains_overlay_after_selection(intent) {
            self.rect = Some(selection::Rect {
                x: 0.0,
                y: 0.0,
                width: self.viewport().width,
                height: self.viewport().height,
            });
            self.all_in_one = None;
            sync_recording_handoff(cx);
            begin_recording(
                cx,
                crate::video::recorder::RecordingTarget::Screen,
                rect,
                self.display_id,
                None,
                None,
            );
            cx.notify();
            return;
        }
        let coordinator = crate::state::coordinator(cx);
        let reservation = self
            .service
            .config
            .get()
            .screenshot
            .freeze_screen
            .then(|| self.service.reserve_cached_capture());
        dismiss(window, cx);
        cx.defer(move |cx| {
            coordinator.update(cx, |coordinator, cx| {
                coordinator.capture_area_reserved(capture, intent, reservation, cx);
            });
        });
    }

    fn on_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
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
            let scale = capture_coordinate_scale(self.scale);
            let x = f32::from(self.display_bounds.left() + event.position.x) * scale;
            let y = f32::from(self.display_bounds.top() + event.position.y) * scale;
            let hovered = crate::capture::windows_list::hit_test(&self.windows, x as f64, y as f64)
                .and_then(|picked| {
                    self.windows
                        .iter()
                        .position(|window| window.window_id == picked.window_id)
                });
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
        if self.is_picking_windows() {
            self.confirm_window(window, cx);
            return;
        }
        let Some(interaction) = self.interaction.take() else {
            return;
        };
        self.drag_to(event.position, cx);

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
    fn only_recording_keeps_the_selection_overlay() {
        use crate::capture::intent::CaptureIntent;

        assert!(super::retains_overlay_after_selection(
            CaptureIntent::Recording
        ));
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

    #[gpui::test]
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
            Some(u32::from(display_id))
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

    #[gpui::test]
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

    #[test]
    fn early_show_requires_capture_exclusion_support() {
        assert!(!super::can_show_before_freeze(19040, true));
        assert!(!super::can_show_before_freeze(19041, false));
        assert!(super::can_show_before_freeze(19041, true));
    }

    #[cfg(windows)]
    #[gpui::test]
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
                    (
                        overlay.window_list_generation,
                        overlay.color_frame_generation,
                    )
                })
                .expect("prepare pending work");
            assert_eq!(super::replace_all(cx), Some(0));
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

    #[gpui::test]
    fn pending_window_list_does_not_start_an_area_selection(cx: &mut TestAppContext) {
        use gpui::{point, Modifiers, MouseButton};

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
            window.focus(&focus);
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
    fn stale_window_lists_are_rejected_after_leaving_window_mode() {
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
        assert!(!super::accepts_window_list(
            Some(Choices {
                mode: Mode::Record,
                target: Target::Screen,
            }),
            2,
            2,
        ));
    }

    #[gpui::test]
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
            window.focus(&focus_handle);
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

    #[gpui::test]
    fn all_in_one_area_capture_closes_the_overlay(cx: &mut TestAppContext) {
        use gpui::{point, Modifiers, MouseButton};

        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let service = cx.read(crate::state::state);
        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let (_overlay, cx) = cx.add_window_view(|window, cx| {
            let focus_handle = cx.focus_handle();
            window.focus(&focus_handle);
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

    #[gpui::test]
    fn video_area_selection_opens_the_recording_control(cx: &mut TestAppContext) {
        use gpui::{point, Modifiers, MouseButton};

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
            window.focus(&focus_handle);
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

    #[gpui::test]
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
            window.focus(&focus);
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
                overlay.hovered_window = Some(0);
                overlay.confirm_window(window, cx);
            })
            .expect("confirm window");
        cx.run_until_parked();

        assert!(overlay.update(cx, |_, _, _| ()).is_err());
        cx.update(|cx| {
            assert_eq!(super::session(cx).handles.len(), 1);
            assert_eq!(super::session(cx).handles[0].generation, 0);
            assert_eq!(super::session(cx).outlined_window_id, Some(42));
            assert!(cx.windows().into_iter().any(|window| window
                .downcast::<crate::windows::recording_control::RecordingControl>()
                .is_some()));
        });
    }

    #[gpui::test]
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
            window.focus(&focus_handle);
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

    #[gpui::test]
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
            window.focus(&focus_handle);
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

    #[gpui::test]
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
            window.focus(&focus);
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
            window.focus(&focus);
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

    #[gpui::test]
    fn color_picker_is_exclusive_and_escape_closes_the_overlay(cx: &mut TestAppContext) {
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
            window.focus(&focus);
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
                window.focus(&toolbar_focus);
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

        assert!(first.update(cx, |_, _, _| ()).is_err());
    }

    #[test]
    fn selection_reserves_frozen_pixels_before_dismissal() {
        let source = include_str!("overlay.rs");
        for (start, end) in [
            ("fn confirm(", "fn prompt("),
            ("fn confirm_screen(", "fn on_move("),
        ] {
            let start = source.find(start).expect("selection handler");
            let source = &source[start..];
            let body = &source[..source.find(end).expect("next selection handler")];
            let reserved = body
                .find("reserve_cached_capture()")
                .expect("cached reservation");
            let dismissed = body.find("dismiss(window, cx)").expect("dismissal");
            let captured = body
                .find("capture_area_reserved(")
                .expect("reserved capture");

            assert!(reserved < dismissed);
            assert!(dismissed < captured);
        }
    }
}
