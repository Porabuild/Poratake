//! The recording control bar — port of
//! `capture/video/recording-control-window.tsx`. One frameless always-on-top
//! surface anchored at the top centre of the recorded display, in both the
//! pre-recording and recording modes.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    div, point, prelude::*, px, size, AnyElement, App, Bounds, Context, FocusHandle, Pixels,
    Render, SharedString, Styled, Subscription, WeakEntity, Window,
};
use herogpui::gpui;
use poratake_daemon_common::contract::{
    CameraPreviewRequest, CameraPreviewUpdateRequest, RECORDING_TARGET_CLOSED,
    SCREEN_RECORDER_ERROR_EVENT,
};

use crate::capture::overlay::ScreenRect;
use crate::theme::vars::active_theme;
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::menu::{MenuBuilder, MenuEntry, MenuHandle, MenuItem, MenuPlacement};
use crate::ui::toolbar;
use crate::video::recorder::{self, RecordingConfig, RecordingTarget};
use crate::windows::registry::{self, WindowKind as RegistryKind};

const DEVICE_MENU_WINDOW_HEIGHT: f32 = 300.0;
const DEVICE_DROPDOWN_WIDTH: f32 = 256.0;
const DEVICE_DROPDOWN_MIN_WIDTH: f32 = 224.0;
const DEVICE_DROPDOWN_HEIGHT: f32 = 224.0;
const COUNTDOWN_WINDOW_HEIGHT: f32 = 148.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    PreRecording,
    Recording,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PreferenceScope {
    Global,
    AllInOne,
}

#[derive(Clone, Copy)]
enum RecordingPreference {
    SystemAudio(bool),
    Microphone(bool),
    Camera(bool),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeviceMenuKind {
    Media(crate::system::devices::DeviceKind),
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Ios,
}

fn can_cancel_with_escape(mode: Mode) -> bool {
    mode == Mode::PreRecording
}

pub(crate) fn set_pre_recording_escape(enabled: bool, cx: &App) {
    let Some(native) = crate::state::try_native(cx) else {
        return;
    };
    native.send(crate::system::native::NativeCommand::SetPreRecordingEscape(
        enabled,
    ));
}

pub struct RecordingControl {
    mode: Mode,
    preference_scope: PreferenceScope,
    #[allow(dead_code)]
    target: RecordingTarget,
    target_name: Option<SharedString>,
    rect: ScreenRect,
    display_id: Option<u32>,
    window_id: Option<i64>,
    project: Option<PathBuf>,
    system_audio: bool,
    #[cfg(target_os = "macos")]
    system_audio_pending: bool,
    #[cfg(target_os = "macos")]
    microphone_pending: bool,
    #[cfg(target_os = "macos")]
    camera_pending: bool,
    #[cfg(target_os = "macos")]
    pause_pending: bool,
    #[cfg(target_os = "macos")]
    camera_protection_pending: bool,
    microphone: bool,
    camera: bool,
    camera_locked: bool,
    camera_preview_visible: bool,
    camera_preview_position: Arc<parking_lot::Mutex<Option<(i32, i32)>>>,
    camera_preview_subscription: Option<crate::daemon::EventSubscription>,
    selected_mic_id: Option<String>,
    selected_camera_id: Option<String>,
    selected_ios_id: Option<String>,
    selected_ios_name: Option<String>,
    ios_devices: Vec<crate::system::devices::MediaDevice>,
    devices: crate::system::devices::MediaDeviceLists,
    device_request: u64,
    pending_device_menu: Option<(DeviceMenuKind, SharedString)>,
    menu: MenuHandle,
    bounds_subscription: Option<Subscription>,
    #[cfg(target_os = "macos")]
    activation_subscription: Option<Subscription>,
    started_at: Option<Instant>,
    paused_at: Option<Instant>,
    paused_total: Duration,
    elapsed: u64,
    countdown_generation: u64,
    countdown_active: bool,
    countdown_remaining: Option<u32>,
    measured_width: Option<f32>,
    #[cfg(all(target_os = "macos", not(test)))]
    pending_native_bounds: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    focus_handle: FocusHandle,
    recorder_error_subscription: Option<crate::daemon::EventSubscription>,
    recorder_terminal_error: Option<Arc<parking_lot::Mutex<Option<String>>>>,
    #[cfg(target_os = "macos")]
    stop_previous_state: Option<recorder::RecorderState>,
    #[cfg(target_os = "macos")]
    stop_target_closed: bool,
    #[cfg(target_os = "macos")]
    start_target_closed: bool,
    #[cfg(target_os = "macos")]
    start_stop_requested: Option<bool>,
    recording_error_generation: u64,
    #[cfg(target_os = "macos")]
    pointer_inside: bool,
    tooltip_presenter: Arc<crate::ui::tooltip::NativeTooltipPresenter>,
}

impl RecordingControl {
    #[cfg(feature = "parity")]
    pub(crate) fn parity_rect(&self) -> ScreenRect {
        self.rect
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub(crate) fn parity_system_audio(&self) -> bool {
        self.system_audio
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub(crate) fn parity_microphone(&self) -> bool {
        self.microphone
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub(crate) fn parity_microphone_selected(&self, device: &str) -> bool {
        self.microphone && self.selected_mic_id.as_deref() == Some(device)
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub(crate) fn parity_camera_status(&self) -> (bool, bool) {
        (self.camera, self.camera_pending)
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub(crate) fn parity_camera_position(&self) -> Option<(i32, i32)> {
        *self.camera_preview_position.lock()
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub(crate) fn parity_pause_pending(&self) -> bool {
        self.pause_pending
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub(crate) fn parity_protection_pending(&self) -> bool {
        self.camera_protection_pending
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub(crate) fn parity_bar_width(&self) -> Option<f32> {
        self.measured_width
    }

    #[cfg(all(feature = "parity", target_os = "macos", not(test)))]
    pub(crate) fn parity_menu_open(&self, cx: &App) -> bool {
        self.menu.is_present(cx)
    }

    #[cfg(all(feature = "parity", target_os = "macos", not(test)))]
    pub(crate) fn parity_countdown_remaining(&self) -> Option<u32> {
        self.countdown_remaining
    }

    #[cfg(all(feature = "parity", any(windows, target_os = "macos")))]
    pub(crate) fn parity_hovered_tooltip(&self) -> Option<SharedString> {
        self.tooltip_presenter.active_content()
    }

    pub(crate) fn open(
        cx: &mut App,
        target: RecordingTarget,
        rect: ScreenRect,
        display_id: Option<u32>,
        window_id: Option<i64>,
        target_name: Option<String>,
        preference_scope: PreferenceScope,
    ) {
        if recorder::is_recording() {
            return;
        }
        if Self::update_pre_recording(
            cx,
            target,
            rect,
            display_id,
            window_id,
            target_name.as_deref(),
            preference_scope,
        ) {
            set_pre_recording_escape(true, cx);
            return;
        }
        let config = crate::state::state(cx).config.get();
        let toggles = match preference_scope {
            PreferenceScope::Global => crate::config::schema::AllInOneRecordingChoices {
                system_audio: config.recording.system_audio,
                mic_enabled: config.recording.mic_enabled,
                camera_enabled: config.recording.camera.enabled,
            },
            PreferenceScope::AllInOne if config.all_in_one.remember_choices => {
                config.all_in_one.recording
            }
            PreferenceScope::AllInOne => Default::default(),
        };
        let config = config.recording;
        let width = recording_control_width(false, target_name.is_some());
        let bounds = bar_bounds(cx, rect, width, false, false, false);
        let display = display_for_rect(cx, rect);
        let platform_display_id = display.as_ref().map(|display| display.id());
        let window_bounds = display
            .as_ref()
            .map(|display| crate::system::work_area::local_window_bounds(bounds, display.as_ref()))
            .unwrap_or(bounds);

        registry::open_or_activate(RegistryKind::RecordingControl, cx, move |cx| {
            cx.open_window(
                super::popup_window_options(
                    window_bounds,
                    super::PopupWindowConfig {
                        display_id: platform_display_id,
                        ..Default::default()
                    },
                ),
                |window, cx| {
                    window.set_window_title("Poratake recording controls");
                    configure_toolbar_window(window);
                    let view = cx.new(|cx| Self {
                        mode: Mode::PreRecording,
                        preference_scope,
                        target,
                        target_name: target_name.map(SharedString::from),
                        rect,
                        display_id,
                        window_id,
                        project: None,
                        system_audio: toggles.system_audio,
                        #[cfg(target_os = "macos")]
                        system_audio_pending: false,
                        #[cfg(target_os = "macos")]
                        microphone_pending: false,
                        #[cfg(target_os = "macos")]
                        camera_pending: false,
                        #[cfg(target_os = "macos")]
                        pause_pending: false,
                        #[cfg(target_os = "macos")]
                        camera_protection_pending: false,
                        microphone: toggles.mic_enabled,
                        camera: toggles.camera_enabled,
                        camera_locked: false,
                        camera_preview_visible: false,
                        camera_preview_position: Arc::default(),
                        camera_preview_subscription: None,
                        selected_mic_id: config.selected_mic_id.clone(),
                        selected_camera_id: config.camera.selected_device_id.clone(),
                        selected_ios_id: config.ios_device.as_ref().map(|device| device.id.clone()),
                        selected_ios_name: config
                            .ios_device
                            .as_ref()
                            .and_then(|device| device.name.clone()),
                        ios_devices: Vec::new(),
                        devices: crate::system::devices::MediaDeviceLists::default(),
                        device_request: 0,
                        pending_device_menu: None,
                        menu: MenuHandle::new(),
                        bounds_subscription: None,
                        #[cfg(target_os = "macos")]
                        activation_subscription: None,
                        started_at: None,
                        paused_at: None,
                        paused_total: Duration::ZERO,
                        elapsed: 0,
                        measured_width: None,
                        #[cfg(all(target_os = "macos", not(test)))]
                        pending_native_bounds: Default::default(),
                        countdown_generation: 0,
                        countdown_active: false,
                        countdown_remaining: None,
                        focus_handle: cx.focus_handle(),
                        recorder_error_subscription: None,
                        recorder_terminal_error: None,
                        #[cfg(target_os = "macos")]
                        stop_previous_state: None,
                        #[cfg(target_os = "macos")]
                        stop_target_closed: false,
                        #[cfg(target_os = "macos")]
                        start_target_closed: false,
                        #[cfg(target_os = "macos")]
                        start_stop_requested: None,
                        recording_error_generation: 0,
                        #[cfg(target_os = "macos")]
                        pointer_inside: false,
                        tooltip_presenter: crate::ui::tooltip::NativeTooltipPresenter::new(
                            "recording-control",
                        ),
                    });
                    view.update(cx, |this, cx| {
                        this.bounds_subscription =
                            Some(cx.observe_window_bounds(window, |this, window, cx| {
                                this.open_pending_device_menu(window, cx);
                            }));
                        #[cfg(target_os = "macos")]
                        {
                            this.activation_subscription =
                                Some(cx.observe_window_activation(window, |_, _, cx| cx.notify()));
                        }
                        #[cfg(target_os = "macos")]
                        if this.camera && !this.request_pre_recording_camera(None, cx) {
                            this.camera = false;
                            this.persist_pre_recording_preference(
                                RecordingPreference::Camera(false),
                                cx,
                            );
                        }
                        #[cfg(not(target_os = "macos"))]
                        if this.camera {
                            if let Err(error) = this.show_camera_preview(cx) {
                                this.camera = false;
                                this.persist_pre_recording_preference(
                                    RecordingPreference::Camera(false),
                                    cx,
                                );
                                crate::windows::toast::Toast::show(
                                    cx,
                                    "Camera preview failed",
                                    error.to_string(),
                                );
                            }
                        }
                    });
                    let focus = view.read(cx).focus_handle.clone();
                    window.focus(&focus, cx);
                    view
                },
            )
            .ok()
            .map(Into::into)
        });
        if registry::is_open(RegistryKind::RecordingControl, cx) {
            set_pre_recording_escape(true, cx);
        }
    }

    pub fn cancel_pre_recording(cx: &mut App) -> bool {
        if !Self::dismiss_pre_recording(false, cx) {
            return false;
        }
        Self::finish_closed_handoff(cx);
        true
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn watch_window_target_closure(
        daemon: &crate::daemon::DaemonHandle,
        cx: &mut App,
    ) -> crate::daemon::EventSubscription {
        use poratake_daemon_common::contract::RECORDING_OVERLAY_TARGET_CLOSED_EVENT;

        let (sender, receiver) = smol::channel::unbounded();
        let subscription = daemon.subscribe(Arc::new(move |event, data| {
            if event != RECORDING_OVERLAY_TARGET_CLOSED_EVENT {
                return;
            }
            if let Some(window_id) = data.get("windowId").and_then(serde_json::Value::as_i64) {
                let _ = sender.try_send(window_id);
            }
        }));
        cx.spawn(async move |cx| {
            while let Ok(window_id) = receiver.recv().await {
                cx.update(|cx| Self::window_target_closed(window_id, cx));
            }
        })
        .detach();
        subscription
    }

    #[cfg(target_os = "macos")]
    fn window_target_closed(window_id: i64, cx: &mut App) -> bool {
        let Some(toolbar) = registry::handle(RegistryKind::RecordingControl, cx)
            .and_then(|handle| handle.downcast::<Self>())
        else {
            return false;
        };
        toolbar
            .update(cx, |toolbar, window, cx| {
                if toolbar.mode != Mode::PreRecording
                    || toolbar.target != RecordingTarget::Window
                    || toolbar.window_id != Some(window_id)
                {
                    return false;
                }
                toolbar.cancel(window, cx);
                crate::windows::toast::Toast::show(
                    cx,
                    "Window closed",
                    "The selected window closed before recording started.",
                );
                crate::intents::refresh_shell(cx);
                true
            })
            .unwrap_or(false)
    }

    fn finish_closed_handoff(cx: &mut App) {
        #[cfg(target_os = "macos")]
        {
            crate::capture::overlay::clear_recording_handoff(cx);
            if !crate::intents::hide_recording_outline(None, cx) {
                set_pre_recording_escape(false, cx);
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            set_pre_recording_escape(false, cx);
            crate::capture::overlay::end_recording_handoff(cx);
        }
    }

    pub(crate) fn dismiss_pre_recording(switching_capture: bool, cx: &mut App) -> bool {
        for window_handle in cx.windows() {
            let Some(handle) = window_handle.downcast::<Self>() else {
                continue;
            };
            let cancelled = handle
                .update(cx, |this, window, cx| {
                    if this.dismiss_device_menu(window, cx) && !switching_capture {
                        return false;
                    }
                    if !can_cancel_with_escape(this.mode) {
                        return false;
                    }
                    this.close_pre_recording_toolbar(window, cx);
                    true
                })
                .unwrap_or(false);
            if cancelled {
                registry::forget(RegistryKind::RecordingControl, cx);
                return true;
            }
        }
        false
    }

    fn dismiss_device_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.pending_device_menu.take().is_some() {
            self.sync_window_bounds(window, cx, false);
            cx.notify();
            return true;
        }
        if !self.menu.is_present(cx) {
            return false;
        }
        self.menu.close(window, cx);
        self.sync_window_bounds(window, cx, false);
        true
    }

    fn tooltip(
        id: &'static str,
        tooltip: impl Into<SharedString>,
        child: impl IntoElement,
        max_body_width: Option<f32>,
        presenter: &Arc<crate::ui::tooltip::NativeTooltipPresenter>,
    ) -> AnyElement {
        let tooltip = tooltip.into();
        let component = match max_body_width {
            Some(width) => {
                let body = tooltip.clone();
                herogpui::components::Tooltip::new(tooltip.clone()).body(move |_, _| {
                    div()
                        .max_w(px(width))
                        .line_clamp(2)
                        .text_ellipsis()
                        .child(body.clone())
                        .into_any_element()
                })
            }
            None => herogpui::components::Tooltip::new(tooltip.clone()),
        }
        .id((gpui::ElementId::from("recording-focus-tooltip"), id));
        #[cfg(windows)]
        let placement = herogpui::components::TooltipPlacement::Bottom;
        #[cfg(not(windows))]
        let placement = match id {
            "recording-target-tip"
            | "recording-start"
            | "recording-pause"
            | "recording-stop"
            | "camera" => herogpui::components::TooltipPlacement::Right,
            _ => herogpui::components::TooltipPlacement::Left,
        };
        presenter.wrap(
            id.into(),
            tooltip,
            component.placement(placement),
            div()
                .id((gpui::ElementId::from("recording-native-tooltip"), id))
                .debug_selector(move || format!("{id}-tooltip-trigger"))
                .flex()
                .flex_shrink_0()
                .child(child),
        )
    }

    fn update_pre_recording(
        cx: &mut App,
        target: RecordingTarget,
        rect: ScreenRect,
        display_id: Option<u32>,
        window_id: Option<i64>,
        target_name: Option<&str>,
        preference_scope: PreferenceScope,
    ) -> bool {
        #[cfg(not(any(windows, target_os = "macos")))]
        let bounds = bar_bounds(
            cx,
            rect,
            recording_control_width(false, target_name.is_some()),
            false,
            false,
            false,
        );
        let target_name = target_name.map(str::to_owned);
        for window_handle in cx.windows() {
            let Some(handle) = window_handle.downcast::<Self>() else {
                continue;
            };
            let updated = handle
                .update(cx, |this, window, cx| {
                    if this.mode != Mode::PreRecording {
                        return false;
                    }
                    if this.preference_scope != preference_scope {
                        this.hide_camera_preview(cx);
                        #[cfg(target_os = "macos")]
                        crate::intents::forget_recording_camera_hide(cx);
                        this.close_pre_recording_toolbar(window, cx);
                        return false;
                    }
                    #[cfg(not(any(windows, target_os = "macos")))]
                    if super::desktop_window_bounds(window).origin != bounds.origin {
                        close_toolbar(window);
                        return false;
                    }
                    let moved = this.rect != rect || this.display_id != display_id;
                    #[cfg(target_os = "macos")]
                    {
                        if this.target_name.is_some() != target_name.is_some() {
                            this.measured_width = None;
                        }
                        if moved || this.target != target || this.window_id != window_id {
                            this.cancel_camera_protection(cx);
                            this.pending_device_menu = None;
                            this.menu.close(window, cx);
                            this.device_request = this.device_request.wrapping_add(1);
                            this.tooltip_presenter.dismiss_active(Some(window));
                            this.pointer_inside = false;
                            this.countdown_generation = this.countdown_generation.wrapping_add(1);
                            this.countdown_active = false;
                            this.countdown_remaining = None;
                        }
                    }
                    this.target = target;
                    this.rect = rect;
                    this.display_id = display_id;
                    this.window_id = window_id;
                    this.target_name = target_name.clone().map(SharedString::from);
                    if moved || !cfg!(target_os = "macos") {
                        this.reposition_camera_preview(cx);
                    }
                    let device_menu_open = cfg!(target_os = "macos")
                        && (this.pending_device_menu.is_some() || this.menu.is_open(cx));
                    this.sync_window_bounds(window, cx, device_menu_open);
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if updated {
                return true;
            }
            registry::forget(RegistryKind::RecordingControl, cx);
            return false;
        }
        false
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn refresh_after_outline_cleanup(cx: &mut App) -> bool {
        let Some(handle) = registry::handle(RegistryKind::RecordingControl, cx)
            .and_then(|handle| handle.downcast::<Self>())
        else {
            return false;
        };
        handle
            .update(cx, |this, _, cx| {
                if this.mode != Mode::PreRecording {
                    return false;
                }
                cx.notify();
                true
            })
            .unwrap_or(false)
    }

    pub(crate) fn start_enabled(&self, cx: &App) -> bool {
        #[cfg(target_os = "macos")]
        if self.camera_pending
            || self.camera_protection_pending
            || crate::intents::recording_cleanup_is_pending(cx)
            || crate::capture::overlay::pending_recording_preparation(cx).is_some()
        {
            return false;
        }
        let _ = cx;
        self.mode == Mode::PreRecording && !self.countdown_active
    }

    pub(crate) fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.start_enabled(cx) {
            return;
        }
        self.device_request = self.device_request.wrapping_add(1);
        let delay = crate::state::state(cx)
            .config
            .get()
            .recording
            .start_delay
            .round()
            .clamp(0.0, 10.0) as u32;
        if delay == 0 {
            self.begin_recording(window, cx);
            return;
        }
        self.countdown_generation += 1;
        let generation = self.countdown_generation;
        let deadline = cx.background_executor().now() + Duration::from_secs(u64::from(delay));
        self.countdown_active = true;
        self.countdown_remaining = Some(delay);
        window.focus(&self.focus_handle, cx);
        self.sync_window_bounds(window, cx, false);
        cx.notify();
        cx.spawn_in(window, async move |entity, cx| loop {
            let Ok(Some(remaining)) = entity.update_in(cx, |this, window, cx| {
                if !this.countdown_active || this.countdown_generation != generation {
                    return None;
                }
                let remaining = deadline.saturating_duration_since(cx.background_executor().now());
                if remaining.is_zero() {
                    this.countdown_active = false;
                    this.countdown_remaining = None;
                    this.begin_recording(window, cx);
                    return None;
                }
                let seconds = remaining.as_secs_f64().ceil() as u32;
                if this.countdown_remaining != Some(seconds) {
                    this.countdown_remaining = Some(seconds);
                    cx.notify();
                }
                Some(remaining)
            }) else {
                break;
            };
            let fraction = remaining.subsec_nanos();
            let until_tick = if fraction == 0 {
                Duration::from_secs(1)
            } else {
                Duration::from_nanos(u64::from(fraction))
            };
            cx.background_executor().timer(until_tick).await;
        })
        .detach();
    }

    fn start_failed(
        &mut self,
        project: &std::path::Path,
        error: anyhow::Error,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        eprintln!("[recorder] {error}");
        self.recording_error_generation = self.recording_error_generation.wrapping_add(1);
        self.recorder_error_subscription = None;
        self.recorder_terminal_error = None;
        #[cfg(target_os = "macos")]
        {
            self.start_target_closed = false;
            self.start_stop_requested = None;
        }
        let removed = remove_empty_failed_start_project(project, &error);
        let message = if removed {
            error.to_string()
        } else {
            format!(
                "{error} Partial recording files may be available at {}.",
                project.display()
            )
        };
        self.project = None;
        self.mode = Mode::PreRecording;
        self.started_at = None;
        self.paused_at = None;
        self.paused_total = Duration::ZERO;
        #[cfg(target_os = "macos")]
        {
            if self.target == RecordingTarget::Window
                && error
                    .downcast_ref::<crate::daemon::DaemonRpcError>()
                    .is_some_and(|error| error.code == RECORDING_TARGET_CLOSED)
            {
                self.cancel(window, cx);
                crate::windows::toast::Toast::show(cx, "Window closed", message);
                crate::intents::refresh_shell(cx);
                return;
            }
            self.rollback_start_handoff(cx);
        }
        set_pre_recording_escape(true, cx);
        self.sync_window_bounds(
            window,
            cx,
            self.pending_device_menu.is_some() || self.menu.is_present(cx),
        );
        crate::windows::toast::Toast::show(cx, "Recording failed", message);
        crate::intents::refresh_shell(cx);
        cx.notify();
    }

    fn begin_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(target_os = "macos")]
        {
            let frozen = crate::capture::overlay::frozen_selection_generation(cx);
            if frozen != 0 {
                self.release_before_recording(frozen, window, cx);
                return;
            }
        }
        #[cfg(not(target_os = "macos"))]
        if !crate::capture::overlay::release_frozen_screen_in_place(cx) {
            crate::windows::toast::Toast::show(
                cx,
                "Recording failed",
                "Could not release the frozen screen",
            );
            return;
        }
        self.prepare_recording(window, cx);
    }

    #[cfg(target_os = "macos")]
    fn release_before_recording(&self, frozen: u64, window: &mut Window, cx: &mut Context<Self>) {
        use crate::capture::overlay;

        let owner = overlay::revision(cx);
        let generation = self.countdown_generation;
        let control = window
            .window_handle()
            .downcast::<Self>()
            .expect("recording control");
        let finished = overlay::begin_recording_preparation(cx);
        let service = crate::state::state(cx);
        cx.notify();
        cx.spawn(async move |_, cx| {
            let released = smol::unblock(move || service.release_screen(frozen)).await;
            cx.update(|cx| {
                overlay::finish_recording_preparation(cx);
                let owns_selection = overlay::revision(cx) == owner;
                if released && owns_selection {
                    overlay::mark_frozen_selection_released(frozen, cx);
                }
                let current = control.update(cx, |this, window, cx| {
                    cx.notify();
                    if !owns_selection
                        || this.mode != Mode::PreRecording
                        || this.countdown_generation != generation
                    {
                        return;
                    }
                    if released {
                        this.prepare_recording(window, cx);
                    } else {
                        crate::windows::toast::Toast::show(
                            cx,
                            "Recording failed",
                            "Could not release the frozen screen",
                        );
                    }
                });
                if current.is_err() {
                    Self::refresh_after_outline_cleanup(cx);
                }
            });
            drop(finished);
        })
        .detach();
    }

    fn prepare_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ios_recording = self.selected_ios_id.is_some();
        #[cfg(all(target_os = "macos", not(test)))]
        let native_window_id = super::macos_window_id(window);
        #[cfg(all(target_os = "macos", not(test), feature = "parity"))]
        let native_window_id =
            if std::env::var_os(crate::ui::parity::daemon_fixture::PROCESS_ENV).is_some() {
                Some(PARITY_RECORDING_CONTROL_WINDOW_ID)
            } else {
                native_window_id
            };
        #[cfg(all(target_os = "macos", not(test)))]
        let excluded_window_ids = match recording_control_exclusion_ids(
            native_window_id,
            self.window_id.is_some() || ios_recording,
        ) {
            Ok(window_ids) => window_ids,
            Err(message) => {
                crate::windows::toast::Toast::show(cx, "Recording failed", message);
                return;
            }
        };
        #[cfg(any(not(target_os = "macos"), test))]
        let excluded_window_ids = Vec::new();
        if ios_recording {
            crate::capture::overlay::close_all(cx);
        }
        #[cfg(target_os = "macos")]
        if !ios_recording {
            if let Some(finished) = crate::capture::overlay::prepare_recording_overlay(cx) {
                let rect = self.rect;
                let generation = self.countdown_generation;
                let control = window
                    .window_handle()
                    .downcast::<Self>()
                    .expect("recording control");
                let daemon = crate::state::state(cx).daemon;
                cx.notify();
                cx.spawn(async move |_, cx| {
                    let shown = smol::unblock(move || daemon.recording_overlay().show(rect))
                        .await
                        .ok()
                        == Some(true);
                    cx.update(|cx| {
                        crate::capture::overlay::finish_recording_preparation(cx);
                        let continue_start = control
                            .update(cx, |this, window, cx| {
                                if this.mode != Mode::PreRecording
                                    || this.countdown_generation != generation
                                {
                                    return false;
                                }
                                cx.notify();
                                if shown {
                                    crate::capture::overlay::close_all(cx);
                                    this.request_recording_start(excluded_window_ids, window, cx);
                                } else {
                                    crate::intents::hide_recording_outline(None, cx);
                                    set_pre_recording_escape(true, cx);
                                    crate::windows::toast::Toast::show(
                                        cx,
                                        "Recording failed",
                                        "Could not show the recording overlay",
                                    );
                                }
                                true
                            })
                            .unwrap_or(false);
                        if !continue_start {
                            crate::intents::hide_recording_outline(None, cx);
                        }
                    });
                    drop(finished);
                })
                .detach();
                return;
            }
        }
        self.request_recording_start(excluded_window_ids, window, cx);
    }

    fn request_recording_start(
        &mut self,
        excluded_window_ids: Vec<i64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(not(target_os = "macos"))]
        match recorder::create_project(&crate::state::state(cx).config) {
            Ok(project) => self.start_after_handoff(project, excluded_window_ids, window, cx),
            Err(error) => self.project_creation_failed(error, cx),
        }

        #[cfg(target_os = "macos")]
        {
            let service = crate::state::state(cx);
            let wait = match service
                .daemon
                .camera_preview()
                .begin_content_protection(true)
            {
                Ok(wait) => wait,
                Err(error) => {
                    self.camera_protection_failed(error, cx);
                    return;
                }
            };
            self.camera_protection_pending = true;
            let generation = self.countdown_generation;
            cx.spawn_in(window, async move |entity, cx| {
                let protection = smol::unblock(wait).await;
                let current = entity
                    .update_in(cx, |this, _, _| {
                        this.camera_protection_pending && this.countdown_generation == generation
                    })
                    .unwrap_or(false);
                if !current {
                    return;
                }
                let mut prepared = Some(match protection {
                    Ok(_) => {
                        Ok(smol::unblock(move || recorder::create_project(&service.config)).await)
                    }
                    Err(error) => Err(error),
                });
                let _ = entity.update_in(cx, |this, window, cx| {
                    if !this.camera_protection_pending || this.countdown_generation != generation {
                        return;
                    }
                    this.camera_protection_pending = false;
                    match prepared.take().expect("recording preparation result") {
                        Ok(Ok(project)) => {
                            this.start_after_handoff(project, excluded_window_ids, window, cx);
                        }
                        Ok(Err(error)) => this.project_creation_failed(error, cx),
                        Err(error) => this.camera_protection_failed(error, cx),
                    }
                    cx.notify();
                });
                if let Some(Ok(Ok(project))) = prepared {
                    smol::unblock(move || {
                        let _ = std::fs::remove_dir(project);
                    })
                    .await;
                }
            })
            .detach();
            cx.notify();
        }
    }

    #[cfg(target_os = "macos")]
    fn camera_protection_failed(&self, error: anyhow::Error, cx: &mut Context<Self>) {
        recorder::release_camera_protection(&crate::state::state(cx).daemon);
        self.rollback_start_handoff(cx);
        set_pre_recording_escape(true, cx);
        crate::windows::toast::Toast::show(
            cx,
            "Recording failed",
            format!("Camera preview protection failed: {error}"),
        );
    }

    #[cfg(target_os = "macos")]
    fn rollback_start_handoff(&self, cx: &mut App) {
        if self.selected_ios_id.is_some() {
            crate::capture::overlay::end_recording_handoff(cx);
        } else {
            crate::capture::overlay::rollback_recording_handoff(cx);
        }
    }

    #[cfg(target_os = "macos")]
    fn cancel_camera_protection(&mut self, cx: &mut App) {
        if std::mem::take(&mut self.camera_protection_pending) {
            recorder::release_camera_protection(&crate::state::state(cx).daemon);
        }
    }

    fn project_creation_failed(&self, error: std::io::Error, cx: &mut Context<Self>) {
        eprintln!("[recorder] {error}");
        #[cfg(target_os = "macos")]
        {
            recorder::release_camera_protection(&crate::state::state(cx).daemon);
            crate::capture::overlay::rollback_recording_handoff(cx);
        }
        set_pre_recording_escape(true, cx);
        crate::windows::toast::Toast::show(cx, "Recording failed", error.to_string());
    }

    fn start_after_handoff(
        &mut self,
        project: std::path::PathBuf,
        excluded_window_ids: Vec<i64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let service = crate::state::state(cx);
        let recording = service.config.get().recording;
        let config = RecordingConfig {
            x: self.rect.x,
            y: self.rect.y,
            width: self.rect.width,
            height: self.rect.height,
            display_id: self.display_id,
            window_id: self.window_id,
            excluded_window_ids,
            include_audio: self.system_audio,
            mic_enabled: self.microphone,
            mic_device_id: recording.selected_mic_id.clone(),
            camera_enabled: self.camera,
            camera_device_id: recording.camera.selected_device_id.clone(),
            ios_device_id: self.selected_ios_id.clone(),
            ios_device_name: self.selected_ios_name.clone(),
            keyboard_enabled: true,
            frame_rate: recording.frame_rate,
            output_path: project.clone(),
        };

        self.recording_error_generation = self.recording_error_generation.wrapping_add(1);
        let error_generation = self.recording_error_generation;
        self.watch_recorder_errors(window, cx, error_generation);
        let start = match recorder::start(&service.daemon, &config) {
            Ok(start) => start,
            Err(error) => {
                self.start_failed(&project, error, window, cx);
                return;
            }
        };
        let started = matches!(&start, crate::daemon::ScreenRecordingStart::Started);
        if started && self.camera && !self.camera_preview_visible {
            if let Err(error) = self.show_camera_preview(cx) {
                eprintln!("[recorder] camera-preview show failed: {error}");
            }
        }
        if self.target == RecordingTarget::Window {
            if let Some(window_id) = self.window_id {
                if !crate::capture::overlay::show_window_recording_outline(window_id, cx) {
                    eprintln!("[recording-overlay] failed to outline window {window_id}");
                }
            }
        }
        set_pre_recording_escape(false, cx);
        #[cfg(not(target_os = "macos"))]
        crate::capture::overlay::begin_recording_handoff(self.rect, cx);

        self.project = Some(project);
        self.mode = Mode::Recording;
        self.camera_locked = self.camera;
        self.started_at = started.then(|| cx.background_executor().now());
        self.paused_at = None;
        self.paused_total = Duration::ZERO;
        self.elapsed = 0;
        self.sync_window_bounds(
            window,
            cx,
            self.pending_device_menu.is_some() || self.menu.is_present(cx),
        );
        crate::intents::refresh_shell(cx);
        if started {
            self.tick(cx);
        }
        #[cfg(target_os = "macos")]
        if let crate::daemon::ScreenRecordingStart::Pending(response) = start {
            let (sender, receiver) = smol::channel::bounded(1);
            std::thread::spawn(move || {
                let result = match response.recv() {
                    Ok(Ok(_)) => Ok(()),
                    Ok(Err(error)) => Err(error),
                    Err(_) => Err(crate::daemon::DaemonRpcError {
                        code: "DAEMON_UNAVAILABLE".into(),
                        message: "The recorder disconnected while starting".into(),
                    }),
                };
                let _ = sender.try_send(result);
            });
            cx.spawn_in(window, async move |entity, cx| {
                let Ok(result) = receiver.recv().await else {
                    return;
                };
                let _ = entity.update_in(cx, |this, window, cx| {
                    if this.recording_error_generation != error_generation
                        || this.mode != Mode::Recording
                    {
                        return;
                    }
                    match result {
                        Ok(()) => {
                            let requested_stop = this.start_stop_requested.take();
                            if this.stop_previous_state.is_none() {
                                let terminal_error = this
                                    .recorder_terminal_error
                                    .as_ref()
                                    .and_then(|pending| pending.lock().take());
                                if let Some(error) = terminal_error {
                                    this.fail_recording(error, window, cx);
                                    return;
                                }
                            }
                            if this.stop_previous_state == Some(recorder::RecorderState::Starting) {
                                this.stop_previous_state = Some(recorder::RecorderState::Recording);
                            } else {
                                recorder::confirm_started();
                            }
                            this.started_at = Some(cx.background_executor().now());
                            if this.camera
                                && !this.camera_preview_visible
                                && this.stop_previous_state.is_none()
                            {
                                if let Err(error) = this.show_camera_preview(cx) {
                                    eprintln!("[recorder] camera-preview show failed: {error}");
                                }
                            }
                            this.tick(cx);
                            cx.notify();
                            if std::mem::take(&mut this.start_target_closed) {
                                if recorder::state() == recorder::RecorderState::Stopping {
                                    this.stop_target_closed = true;
                                } else {
                                    this.finish(requested_stop.unwrap_or(false), window, cx);
                                }
                            }
                        }
                        Err(error) => {
                            if error.code != "DAEMON_UNAVAILABLE" {
                                recorder::release_camera_protection(
                                    &crate::state::state(cx).daemon,
                                );
                            }
                            if error.code != "DAEMON_UNAVAILABLE"
                                && this.stop_previous_state.is_none()
                                && this.start_stop_requested.is_none()
                            {
                                if let Some(project) = this.project.take() {
                                    recorder::force_idle();
                                    this.start_failed(
                                        &project,
                                        anyhow::Error::new(error),
                                        window,
                                        cx,
                                    );
                                    return;
                                }
                            }
                            this.fail_recording(error.to_string(), window, cx);
                        }
                    }
                });
            })
            .detach();
        }
        #[cfg(not(target_os = "macos"))]
        let _ = start;
        cx.notify();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |entity, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let running = entity.update(cx, |this, cx| {
                if this.started_at.is_none() {
                    return false;
                }
                if recorder::state() == recorder::RecorderState::Recording {
                    this.elapsed = this.recording_duration(cx).as_secs();
                    cx.notify();
                }
                true
            });
            if !matches!(running, Ok(true)) {
                break;
            }
        })
        .detach();
    }

    fn persist_pre_recording_preference(&self, preference: RecordingPreference, cx: &App) {
        if self.mode != Mode::PreRecording {
            return;
        }
        let config = crate::state::state(cx).config;
        if self.preference_scope == PreferenceScope::AllInOne
            && !config.get().all_in_one.remember_choices
        {
            return;
        }
        config.update(|config| match (self.preference_scope, preference) {
            (PreferenceScope::Global, RecordingPreference::SystemAudio(enabled)) => {
                config.recording.system_audio = enabled;
            }
            (PreferenceScope::Global, RecordingPreference::Microphone(enabled)) => {
                config.recording.mic_enabled = enabled;
            }
            (PreferenceScope::Global, RecordingPreference::Camera(enabled)) => {
                config.recording.camera.enabled = enabled;
            }
            (PreferenceScope::AllInOne, RecordingPreference::SystemAudio(enabled)) => {
                config.all_in_one.recording.system_audio = enabled;
            }
            (PreferenceScope::AllInOne, RecordingPreference::Microphone(enabled)) => {
                config.all_in_one.recording.mic_enabled = enabled;
            }
            (PreferenceScope::AllInOne, RecordingPreference::Camera(enabled)) => {
                config.all_in_one.recording.camera_enabled = enabled;
            }
        });
    }

    /// AGENTS documents that the microphone, system-sound and camera toggles
    /// stay live while a recording runs, so they are applied through the
    /// recorder's setters instead of being frozen at start.
    pub(crate) fn set_system_audio(&mut self, enabled: bool, cx: &mut Context<Self>) -> bool {
        #[cfg(target_os = "macos")]
        if self.camera_protection_pending {
            return false;
        }
        if self.mode == Mode::Recording {
            #[cfg(target_os = "macos")]
            return self.request_system_audio(enabled, cx);

            #[cfg(not(target_os = "macos"))]
            let result = recorder::set_system_audio(&crate::state::state(cx).daemon, enabled);
            #[cfg(not(target_os = "macos"))]
            if let Err(error) = result {
                report_toggle_failure(cx, &error);
                return false;
            }
        }
        self.system_audio = enabled;
        self.persist_pre_recording_preference(RecordingPreference::SystemAudio(enabled), cx);
        cx.notify();
        true
    }

    #[cfg(target_os = "macos")]
    fn request_system_audio(&mut self, enabled: bool, cx: &mut Context<Self>) -> bool {
        if self.system_audio_pending {
            return false;
        }
        let daemon = crate::state::state(cx).daemon;
        let wait = match daemon.screen_recorder().begin_system_audio(enabled) {
            Ok(wait) => wait,
            Err(error) => {
                report_toggle_failure(cx, &error);
                return false;
            }
        };
        self.system_audio_pending = true;
        self.await_recording_setting(
            wait,
            move |this, accepted, _| {
                this.system_audio_pending = false;
                if accepted {
                    this.system_audio = enabled;
                }
            },
            cx,
        );
        true
    }

    #[cfg(target_os = "macos")]
    fn await_recording_setting(
        &mut self,
        wait: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
        complete: impl FnOnce(&mut Self, bool, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let generation = self.recording_error_generation;
        let mode = self.mode;
        cx.spawn(async move |entity, cx| {
            let result = smol::unblock(wait).await;
            let _ = entity.update(cx, |this, cx| {
                if this.recording_error_generation != generation || this.mode != mode {
                    return;
                }
                complete(this, result.is_ok(), cx);
                if let Err(error) = result {
                    report_toggle_failure(cx, &error);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn set_microphone(
        &mut self,
        enabled: bool,
        device_id: Option<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        #[cfg(target_os = "macos")]
        if self.microphone_pending || self.camera_protection_pending {
            return false;
        }
        if enabled
            && !crate::system::permissions::ensure_access(
                crate::system::permissions::Device::Microphone,
            )
        {
            return false;
        }
        if self.mode == Mode::Recording {
            let service = crate::state::state(cx);
            #[cfg(target_os = "macos")]
            {
                let request = poratake_daemon_common::contract::ScreenRecorderMicrophoneRequest {
                    enabled,
                    device_id: device_id.clone(),
                    device_name: None,
                };
                let wait = match service.daemon.screen_recorder().begin_microphone(request) {
                    Ok(wait) => wait,
                    Err(error) => {
                        report_toggle_failure(cx, &error);
                        return false;
                    }
                };
                self.microphone_pending = true;
                self.await_recording_setting(
                    wait,
                    move |this, accepted, _| {
                        this.microphone_pending = false;
                        if accepted {
                            this.microphone = enabled;
                            this.selected_mic_id = device_id;
                        }
                    },
                    cx,
                );
                return true;
            }
            #[cfg(not(target_os = "macos"))]
            if let Err(error) =
                recorder::set_microphone(&service.daemon, enabled, device_id.as_deref())
            {
                report_toggle_failure(cx, &error);
                return false;
            }
        }
        self.microphone = enabled;
        self.selected_mic_id = device_id;
        self.persist_pre_recording_preference(RecordingPreference::Microphone(enabled), cx);
        cx.notify();
        true
    }

    pub(crate) fn set_camera(&mut self, enabled: bool, cx: &mut Context<Self>) -> bool {
        #[cfg(target_os = "macos")]
        if self.camera_pending || self.camera_protection_pending {
            return false;
        }
        if enabled
            && !crate::system::permissions::ensure_access(
                crate::system::permissions::Device::Camera,
            )
        {
            return false;
        }
        #[cfg(target_os = "macos")]
        {
            if self.mode == Mode::Recording {
                return self.request_recording_camera(enabled, cx);
            }
            if enabled {
                return self.request_pre_recording_camera(None, cx);
            }
            if self.camera_preview_visible {
                self.request_pre_recording_camera_off(cx);
                return true;
            }
        }
        #[cfg(not(target_os = "macos"))]
        if enabled {
            if let Err(error) = self.show_camera_preview(cx) {
                report_toggle_failure(cx, &error);
                return false;
            }
        }
        #[cfg(not(target_os = "macos"))]
        if self.mode == Mode::Recording {
            let result = recorder::set_camera(&crate::state::state(cx).daemon, enabled);
            if let Err(error) = result {
                if enabled {
                    self.hide_camera_preview(cx);
                }
                report_toggle_failure(cx, &error);
                return false;
            }
        }
        if !enabled {
            self.hide_camera_preview(cx);
        }
        self.camera = enabled;
        self.persist_pre_recording_preference(RecordingPreference::Camera(enabled), cx);
        cx.notify();
        true
    }

    #[cfg(target_os = "macos")]
    fn request_pre_recording_camera_off(&mut self, cx: &mut Context<Self>) {
        let response = crate::intents::begin_recording_camera_hide(cx);
        self.camera_pending = true;
        let generation = self.recording_error_generation;
        cx.spawn(async move |entity, cx| {
            let Ok(result) = response.recv().await else {
                return;
            };
            let _ = entity.update(cx, |this, cx| {
                if this.recording_error_generation != generation || this.mode != Mode::PreRecording
                {
                    return;
                }
                crate::intents::forget_recording_camera_hide(cx);
                if let Err(error) = result {
                    report_toggle_failure(cx, &anyhow::anyhow!(error));
                } else {
                    this.detach_camera_preview();
                    this.camera = false;
                    this.persist_pre_recording_preference(RecordingPreference::Camera(false), cx);
                }
                this.camera_pending = false;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    #[cfg(target_os = "macos")]
    fn request_pre_recording_camera(
        &mut self,
        selection: Option<(Option<String>, Option<String>)>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.camera_pending || self.camera_protection_pending {
            return false;
        }
        let had_preview = self.camera_preview_visible;
        let rect = self.rect;
        let mut request = self.prepare_camera_preview(cx);
        if let Some((id, name)) = &selection {
            request.device_id = id.clone();
            request.device_name = name.clone();
        }
        self.request_camera_preview(
            request,
            move |this, shown, cx| {
                this.camera_pending = false;
                if !shown {
                    if !had_preview {
                        this.hide_camera_preview(cx);
                        this.camera = false;
                        this.persist_pre_recording_preference(
                            RecordingPreference::Camera(false),
                            cx,
                        );
                    }
                    return;
                }
                this.camera = true;
                this.persist_pre_recording_preference(RecordingPreference::Camera(true), cx);
                if let Some((id, name)) = selection {
                    this.selected_camera_id = id.clone();
                    crate::state::state(cx).config.update(move |config| {
                        config.recording.camera.selected_device_id = id;
                        config.recording.camera.selected_device_name = name;
                    });
                }
                if this.rect != rect {
                    this.reposition_camera_preview(cx);
                }
            },
            cx,
        )
    }

    #[cfg(target_os = "macos")]
    fn request_recording_camera(&mut self, enabled: bool, cx: &mut Context<Self>) -> bool {
        if enabled && self.camera && self.camera_preview_visible {
            let request = self.prepare_camera_preview(cx);
            return self.request_camera_preview(
                request,
                |this, _, _| this.camera_pending = false,
                cx,
            );
        }
        if !enabled || self.camera_preview_visible {
            return self.send_recording_camera(enabled, cx);
        }

        let request = self.prepare_camera_preview(cx);
        self.request_camera_preview(
            request,
            |this, shown, cx| {
                if !shown
                    || !matches!(
                        recorder::state(),
                        recorder::RecorderState::Recording | recorder::RecorderState::Paused
                    )
                {
                    this.hide_camera_preview(cx);
                    return;
                }
                if !this.send_recording_camera(true, cx) {
                    this.hide_camera_preview(cx);
                }
            },
            cx,
        )
    }

    #[cfg(target_os = "macos")]
    fn request_camera_preview(
        &mut self,
        request: CameraPreviewRequest,
        complete: impl FnOnce(&mut Self, bool, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> bool {
        let daemon = crate::state::state(cx).daemon;
        let had_preview = self.camera_preview_visible;
        let wait = match daemon.camera_preview().begin_show(&request) {
            Ok(wait) => wait,
            Err(error) => {
                if !had_preview {
                    self.camera_preview_subscription = None;
                }
                report_toggle_failure(cx, &error);
                return false;
            }
        };
        self.camera_pending = true;
        self.camera_preview_visible = true;
        self.await_recording_setting(wait, complete, cx);
        true
    }

    #[cfg(target_os = "macos")]
    fn send_recording_camera(&mut self, enabled: bool, cx: &mut Context<Self>) -> bool {
        let daemon = crate::state::state(cx).daemon;
        let wait = match daemon.screen_recorder().begin_camera(enabled) {
            Ok(wait) => wait,
            Err(error) => {
                report_toggle_failure(cx, &error);
                return false;
            }
        };
        self.camera_pending = true;
        self.await_recording_setting(
            wait,
            move |this, accepted, cx| {
                if accepted {
                    this.camera = enabled;
                }
                if this.camera {
                    this.camera_pending = false;
                } else {
                    this.hide_camera_preview(cx);
                }
            },
            cx,
        );
        true
    }

    fn show_camera_preview(&mut self, cx: &mut App) -> anyhow::Result<()> {
        let daemon = crate::state::state(cx).daemon;
        let request = self.prepare_camera_preview(cx);
        if let Err(error) = daemon.camera_preview().show(&request) {
            self.camera_preview_subscription = None;
            return Err(error);
        }
        self.camera_preview_visible = true;
        Ok(())
    }

    fn prepare_camera_preview(&mut self, cx: &App) -> CameraPreviewRequest {
        let service = crate::state::state(cx);
        let camera = service.config.get().recording.camera;
        let saved_position = *self.camera_preview_position.lock();
        let (x, y) = saved_position
            .or_else(|| {
                camera
                    .position
                    .map(|position| (position.x as i32, position.y as i32))
            })
            .unwrap_or_else(|| camera_preview_position(self.rect));
        let device_name = self
            .selected_camera_id
            .as_ref()
            .and_then(|id| {
                self.devices
                    .cameras
                    .iter()
                    .find(|device| &device.id == id)
                    .map(|device| device.label.clone())
            })
            .or(camera.selected_device_name.clone());
        let request = CameraPreviewRequest {
            device_id: self
                .selected_camera_id
                .clone()
                .or(camera.selected_device_id),
            device_name,
            resolution: Some(camera.resolution),
            flipped: Some(camera.flipped),
            x: Some(x),
            y: Some(y),
        };
        let position = self.camera_preview_position.clone();
        self.camera_preview_subscription =
            Some(service.daemon.subscribe(Arc::new(move |event, data| {
                if event != "camera-preview:position-changed" {
                    return;
                }
                let Some((x, y)) = data
                    .get("x")
                    .and_then(serde_json::Value::as_i64)
                    .and_then(|x| i32::try_from(x).ok())
                    .zip(
                        data.get("y")
                            .and_then(serde_json::Value::as_i64)
                            .and_then(|y| i32::try_from(y).ok()),
                    )
                else {
                    return;
                };
                *position.lock() = Some((x, y));
            })));
        request
    }

    fn detach_camera_preview(&mut self) -> bool {
        if !self.camera_preview_visible {
            #[cfg(target_os = "macos")]
            {
                self.camera_pending = false;
            }
            return false;
        }
        self.camera_preview_visible = false;
        self.camera_preview_subscription = None;
        let position = if self.mode == Mode::Recording {
            *self.camera_preview_position.lock()
        } else {
            None
        };
        self.camera_preview_position = Arc::new(parking_lot::Mutex::new(position));
        true
    }

    fn hide_camera_preview(&mut self, cx: &mut Context<Self>) {
        if !self.detach_camera_preview() {
            return;
        }
        #[cfg(target_os = "macos")]
        if self.mode == Mode::Recording {
            let daemon = crate::state::state(cx).daemon;
            match daemon.camera_preview().begin_hide() {
                Ok(wait) => {
                    self.await_recording_setting(wait, |this, _, _| this.camera_pending = false, cx)
                }
                Err(error) => {
                    self.camera_pending = false;
                    eprintln!("[recorder] camera-preview hide failed: {error}");
                }
            }
            return;
        }
        if let Err(error) = crate::state::state(cx).daemon.camera_preview().hide() {
            eprintln!("[recorder] camera-preview hide failed: {error}");
        }
    }

    fn reposition_camera_preview(&self, cx: &mut App) {
        #[cfg(target_os = "macos")]
        if self.camera_pending {
            return;
        }
        if !self.camera_preview_visible {
            return;
        }
        let (x, y) = camera_preview_position(self.rect);
        let request = CameraPreviewUpdateRequest {
            x: Some(x),
            y: Some(y),
            ..Default::default()
        };
        let daemon = crate::state::state(cx).daemon;
        #[cfg(target_os = "macos")]
        match daemon.camera_preview().begin_update(&request) {
            Ok(wait) => smol::spawn(async move {
                if let Err(error) = smol::unblock(wait).await {
                    eprintln!("[recorder] camera-preview update failed: {error}");
                }
            })
            .detach(),
            Err(error) => eprintln!("[recorder] camera-preview update failed: {error}"),
        }
        #[cfg(not(target_os = "macos"))]
        match daemon.camera_preview().update(&request) {
            Ok(()) => *self.camera_preview_position.lock() = Some((x, y)),
            Err(error) => eprintln!("[recorder] camera-preview update failed: {error}"),
        }
    }

    fn input_toggles(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut toggles = Vec::new();
        let starting = matches!(
            recorder::state(),
            recorder::RecorderState::Starting | recorder::RecorderState::Stopping
        );
        #[cfg(target_os = "macos")]
        let starting = starting || self.camera_protection_pending;
        if self.mode == Mode::PreRecording || self.camera_locked {
            toggles.push(self.device_dropdown(
                "camera",
                if self.camera { "video" } else { "video-off" },
                "Select camera",
                DeviceMenuKind::Media(crate::system::devices::DeviceKind::Camera),
                false,
                cx,
            ));
        }
        toggles.extend([
            self.device_dropdown(
                "microphone",
                if self.microphone { "mic" } else { "mic-off" },
                "Select microphone",
                DeviceMenuKind::Media(crate::system::devices::DeviceKind::Microphone),
                false,
                cx,
            ),
            overlay_icon(
                "recording-system-audio",
                if self.system_audio {
                    "volume-2"
                } else {
                    "volume-x"
                },
                if self.system_audio {
                    "Turn system sounds off"
                } else {
                    "Turn system sounds on"
                },
                {
                    let disabled = self.countdown_active || starting;
                    #[cfg(target_os = "macos")]
                    let disabled = disabled || self.system_audio_pending;
                    disabled
                },
                |this, _window, cx| {
                    if this.countdown_active
                        || matches!(
                            recorder::state(),
                            recorder::RecorderState::Starting | recorder::RecorderState::Stopping
                        )
                    {
                        return;
                    }
                    let next = !this.system_audio;
                    this.set_system_audio(next, cx);
                },
                cx,
                &self.tooltip_presenter,
            ),
        ]);
        #[cfg(target_os = "macos")]
        if self.mode == Mode::PreRecording {
            toggles.push(self.device_dropdown(
                "ios-device",
                "smartphone",
                "Select iPhone or iPad",
                DeviceMenuKind::Ios,
                self.selected_ios_id.is_some(),
                cx,
            ));
        }
        toggles
    }

    fn device_dropdown(
        &self,
        id: &'static str,
        icon: &'static str,
        tooltip: &'static str,
        kind: DeviceMenuKind,
        tinted: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = SharedString::from(format!("recording-{id}"));
        let entity = cx.entity().downgrade();
        let menu_id = owner.clone();
        let disabled = self.countdown_active
            || matches!(
                recorder::state(),
                recorder::RecorderState::Starting | recorder::RecorderState::Stopping
            );
        #[cfg(target_os = "macos")]
        let disabled = disabled
            || self.camera_protection_pending
            || (self.microphone_pending
                && kind == DeviceMenuKind::Media(crate::system::devices::DeviceKind::Microphone));
        #[cfg(target_os = "macos")]
        let disabled = disabled
            || (self.camera_pending
                && kind == DeviceMenuKind::Media(crate::system::devices::DeviceKind::Camera));
        let trigger = herogpui::components::Button::new(owner)
            .aria_label(tooltip)
            .variant(herogpui::Variant::Ghost)
            .is_disabled(disabled)
            .recipe("overlay-trigger")
            .when(disabled, |button| button.recipe("overlay-disabled"))
            .when(tinted, |button| button.recipe("overlay-trigger-tinted"))
            .child(icon_element(icon, px(chrome::TOOL_BUTTON_ICON)))
            .child(icon_element("chevron-down", px(12.0)))
            .on_press(move |_, window, cx| {
                if let Some(entity) = entity.upgrade() {
                    entity.update(cx, |this, cx| {
                        this.toggle_device_menu(kind, menu_id.clone(), window, cx)
                    });
                }
                cx.stop_propagation();
            });
        div()
            .relative()
            .flex()
            .child(Self::tooltip(
                id,
                tooltip,
                trigger,
                None,
                &self.tooltip_presenter,
            ))
            .child(self.menu.render_dropdown(&format!("recording-{id}"), cx))
            .into_any_element()
    }

    fn toggle_device_menu(
        &mut self,
        kind: DeviceMenuKind,
        owner: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pending_for_owner = self
            .pending_device_menu
            .as_ref()
            .is_some_and(|(_, pending_owner)| pending_owner == &owner);
        if pending_for_owner || self.menu.is_open_for(owner.as_ref(), cx) {
            self.pending_device_menu = None;
            self.menu.close(window, cx);
            self.sync_window_bounds(window, cx, false);
            cx.notify();
            return;
        }

        let expanded = window.bounds().size.height >= px(DEVICE_MENU_WINDOW_HEIGHT);
        self.pending_device_menu = Some((kind, owner));
        self.sync_window_bounds(window, cx, true);
        if expanded {
            self.open_pending_device_menu(window, cx);
        }
        cx.notify();
    }

    fn open_pending_device_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((kind, owner)) =
            take_ready_device_menu(&mut self.pending_device_menu, window.bounds().size.height)
        else {
            return;
        };
        let entries = match kind {
            DeviceMenuKind::Media(kind) => self.device_menu_entries(kind, cx.entity().downgrade()),
            DeviceMenuKind::Ios => self.ios_menu_entries(cx.entity().downgrade()),
        };
        self.menu
            .toggle(device_menu_placement(owner.clone()), entries, window, cx);
        if self.menu.is_open_for(owner.as_ref(), cx) {
            match kind {
                DeviceMenuKind::Media(kind) => self.refresh_devices(kind, owner, window, cx),
                DeviceMenuKind::Ios => self.refresh_ios_devices(owner, window, cx),
            }
        }
    }

    fn device_menu_entries(
        &self,
        kind: crate::system::devices::DeviceKind,
        entity: WeakEntity<Self>,
    ) -> Vec<MenuEntry> {
        use crate::system::devices::DeviceKind;

        let (label, enabled, selected, devices, default_id) = match kind {
            DeviceKind::Microphone => (
                "Microphone",
                self.microphone,
                self.selected_mic_id.as_deref(),
                &self.devices.microphones,
                self.devices.default_microphone_id.as_deref(),
            ),
            DeviceKind::Camera => (
                "Camera",
                self.camera,
                self.selected_camera_id.as_deref(),
                &self.devices.cameras,
                self.devices.default_camera_id.as_deref(),
            ),
        };
        let options = crate::system::devices::options(devices);
        let default_label = default_id
            .and_then(|id| {
                options
                    .iter()
                    .find(|(device_id, _)| device_id == id)
                    .map(|(_, label)| format!("System Default ({label})"))
            })
            .unwrap_or_else(|| "System Default".to_string());
        let device_locked = kind == DeviceKind::Camera && self.mode == Mode::Recording;
        let locked_device_id = selected.or(default_id);
        let default_locked = device_locked && selected.is_some() && selected != default_id;
        let toggle_entity = entity.clone();
        let default_entity = entity.clone();
        let mut builder = MenuBuilder::new()
            .item(
                MenuItem::new(label)
                    .trailing_check(enabled)
                    .on_select(move |_window, cx| {
                        if let Some(entity) = toggle_entity.upgrade() {
                            entity.update(cx, |this, cx| match kind {
                                DeviceKind::Microphone => {
                                    this.set_microphone(
                                        !this.microphone,
                                        this.selected_mic_id.clone(),
                                        cx,
                                    );
                                }
                                DeviceKind::Camera => {
                                    this.set_camera(!this.camera, cx);
                                }
                            });
                        }
                    }),
            )
            .separator()
            .item(
                MenuItem::new(default_label)
                    .trailing_check(selected.is_none())
                    .disabled(default_locked)
                    .on_select(move |_window, cx| {
                        if let Some(entity) = default_entity.upgrade() {
                            entity.update(cx, |this, cx| this.select_device(kind, None, cx));
                        }
                    }),
            );

        for (device_id, label) in options {
            let disabled = device_locked && locked_device_id != Some(device_id.as_str());
            let selected = selected == Some(device_id.as_str());
            let device_entity = entity.clone();
            builder = builder.item(
                MenuItem::new(label.clone())
                    .trailing_check(selected)
                    .disabled(disabled)
                    .on_select(move |_window, cx| {
                        if let Some(entity) = device_entity.upgrade() {
                            entity.update(cx, |this, cx| {
                                this.select_device(
                                    kind,
                                    Some((device_id.clone(), label.clone())),
                                    cx,
                                );
                            });
                        }
                    }),
            );
        }
        builder.build()
    }

    fn ios_menu_entries(&self, entity: WeakEntity<Self>) -> Vec<MenuEntry> {
        let none_entity = entity.clone();
        let mut builder = MenuBuilder::new()
            .item(
                MenuItem::new("None")
                    .trailing_check(self.selected_ios_id.is_none())
                    .on_select(move |_window, cx| {
                        if let Some(entity) = none_entity.upgrade() {
                            entity.update(cx, |this, cx| this.select_ios_device(None, cx));
                        }
                    }),
            )
            .separator();
        for device in &self.ios_devices {
            let device = device.clone();
            let selected = self.selected_ios_id.as_deref() == Some(device.id.as_str());
            let device_entity = entity.clone();
            builder = builder.item(
                MenuItem::new(device.label.clone())
                    .trailing_check(selected)
                    .on_select(move |_window, cx| {
                        if let Some(entity) = device_entity.upgrade() {
                            let device = device.clone();
                            entity.update(cx, |this, cx| {
                                this.select_ios_device(Some((device.id, device.label)), cx);
                            });
                        }
                    }),
            );
        }
        builder.build()
    }

    fn refresh_devices(
        &mut self,
        kind: crate::system::devices::DeviceKind,
        owner: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.device_request = self.device_request.wrapping_add(1);
        let request = self.device_request;
        let daemon = crate::state::state(cx).daemon;
        cx.spawn_in(window, async move |entity, cx| {
            let listed = cx
                .background_executor()
                .spawn(async move { daemon.media_devices().list(&[kind]) })
                .await;
            let _ = entity.update_in(cx, |this, window, cx| {
                if !merge_device_refresh(
                    &mut this.devices,
                    this.device_request,
                    request,
                    kind,
                    listed,
                ) {
                    return;
                }
                if this.menu.is_open_for(owner.as_ref(), cx) {
                    let entries = this.device_menu_entries(kind, cx.entity().downgrade());
                    this.menu
                        .open(device_menu_placement(owner.clone()), entries, window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn refresh_ios_devices(
        &mut self,
        owner: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.device_request = self.device_request.wrapping_add(1);
        let request = self.device_request;
        let daemon = crate::state::state(cx).daemon;
        cx.spawn_in(window, async move |entity, cx| {
            let reply = cx
                .background_executor()
                .spawn(async move { daemon.recording_control().list_ios_devices() })
                .await;
            let listed = match reply {
                Ok(listed) => listed,
                Err(error) => {
                    eprintln!("[devices] iOS device list failed: {error}");
                    return;
                }
            };
            let _ = entity.update_in(cx, |this, window, cx| {
                if this.device_request != request {
                    return;
                }
                let disconnected = this
                    .selected_ios_id
                    .as_deref()
                    .is_some_and(|id| !listed.iter().any(|device| device.id == id));
                if this.ios_devices == listed && !disconnected {
                    return;
                }
                this.ios_devices = listed;
                if disconnected {
                    this.select_ios_device(None, cx);
                }
                if this.menu.is_open_for(owner.as_ref(), cx) {
                    let entries = this.ios_menu_entries(cx.entity().downgrade());
                    this.menu
                        .open(device_menu_placement(owner.clone()), entries, window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn select_ios_device(&mut self, selection: Option<(String, String)>, cx: &mut Context<Self>) {
        self.selected_ios_id = selection.as_ref().map(|(id, _)| id.clone());
        self.selected_ios_name = selection.as_ref().map(|(_, name)| name.clone());
        crate::state::state(cx).config.update(move |config| {
            config.recording.ios_device =
                selection.map(|(id, name)| crate::config::schema::IosDeviceSettings {
                    id,
                    name: Some(name),
                });
        });
        cx.notify();
    }

    pub(crate) fn select_device(
        &mut self,
        kind: crate::system::devices::DeviceKind,
        selection: Option<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        let id = selection.as_ref().map(|(id, _)| id.clone());
        let name = selection.as_ref().map(|(_, name)| name.clone());
        match kind {
            crate::system::devices::DeviceKind::Microphone => {
                if !self.set_microphone(true, id.clone(), cx) {
                    return;
                }
                if self.mode == Mode::PreRecording {
                    crate::state::state(cx).config.update(move |config| {
                        config.recording.selected_mic_id = id;
                        config.recording.selected_mic_name = name;
                    });
                }
            }
            crate::system::devices::DeviceKind::Camera => {
                if self.mode == Mode::Recording {
                    // The camera device is locked to the one the recording
                    // started with, so a mid-recording pick only re-enables
                    // it instead of switching devices.
                    self.set_camera(true, cx);
                    cx.notify();
                    return;
                }
                #[cfg(target_os = "macos")]
                if crate::system::permissions::ensure_access(
                    crate::system::permissions::Device::Camera,
                ) {
                    self.request_pre_recording_camera(Some((id, name)), cx);
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let previous = self.selected_camera_id.clone();
                    self.selected_camera_id = id.clone();
                    if !self.set_camera(true, cx) {
                        self.selected_camera_id = previous;
                        cx.notify();
                        return;
                    }
                    crate::state::state(cx).config.update(move |config| {
                        config.recording.camera.selected_device_id = id;
                        config.recording.camera.selected_device_name = name;
                    });
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn request_pause_change(&mut self, cx: &mut Context<Self>) -> bool {
        use recorder::RecorderState::{Paused, Recording, Stopping};

        if self.pause_pending {
            return false;
        }
        let paused = match recorder::state() {
            Recording => true,
            Paused => false,
            _ => return false,
        };
        let daemon = crate::state::state(cx).daemon;
        let wait = match daemon.screen_recorder().begin_pause_change(paused) {
            Ok(wait) => wait,
            Err(error) => {
                report_toggle_failure(cx, &error);
                return false;
            }
        };
        self.pause_pending = true;
        self.await_recording_setting(
            wait,
            move |this, accepted, cx| {
                this.pause_pending = false;
                if !accepted {
                    return;
                }
                let confirmed = if paused { Paused } else { Recording };
                if recorder::state() == Stopping {
                    this.stop_previous_state = Some(confirmed);
                } else {
                    recorder::confirm_pause_change(confirmed);
                }
                let now = cx.background_executor().now();
                if paused {
                    this.paused_at = Some(now);
                } else if let Some(paused_at) = this.paused_at.take() {
                    this.paused_total += now.saturating_duration_since(paused_at);
                }
            },
            cx,
        );
        true
    }

    fn toggle_pause(&mut self, cx: &mut Context<Self>) {
        #[cfg(target_os = "macos")]
        self.request_pause_change(cx);
        #[cfg(not(target_os = "macos"))]
        self.toggle_pause_sync(cx);
    }

    #[cfg(not(target_os = "macos"))]
    fn toggle_pause_sync(&mut self, cx: &mut Context<Self>) {
        let daemon = crate::state::state(cx).daemon;
        match recorder::state() {
            recorder::RecorderState::Recording => {
                recorder::pause(&daemon);
                if recorder::state() == recorder::RecorderState::Paused {
                    self.paused_at = Some(cx.background_executor().now());
                }
            }
            recorder::RecorderState::Paused => {
                recorder::resume(&daemon);
                if recorder::state() == recorder::RecorderState::Recording {
                    if let Some(paused_at) = self.paused_at.take() {
                        self.paused_total += cx
                            .background_executor()
                            .now()
                            .saturating_duration_since(paused_at);
                    }
                }
            }
            recorder::RecorderState::Idle
            | recorder::RecorderState::Starting
            | recorder::RecorderState::Stopping => {}
        }
        cx.notify();
    }

    fn recording_duration(&self, cx: &App) -> Duration {
        let Some(started_at) = self.started_at else {
            return Duration::from_secs(self.elapsed);
        };
        let now = cx.background_executor().now();
        let current_pause = self
            .paused_at
            .map(|paused_at| now.saturating_duration_since(paused_at))
            .unwrap_or_default();
        now.saturating_duration_since(started_at)
            .saturating_sub(self.paused_total + current_pause)
    }

    /// Port of the `screen-recorder:error` listener in `recorder.ts`: a closed
    /// target ends the take normally, any other daemon failure tears it down.
    fn watch_recorder_errors(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        generation: u64,
    ) {
        let service = crate::state::state(cx);
        let (error_tx, error_rx) = smol::channel::unbounded::<(String, String)>();
        let terminal_error = Arc::new(parking_lot::Mutex::new(None));
        self.recorder_terminal_error = Some(terminal_error.clone());
        self.recorder_error_subscription =
            Some(service.daemon.subscribe(Arc::new(move |event, data| {
                if event != SCREEN_RECORDER_ERROR_EVENT {
                    return;
                }
                let code = data
                    .get("code")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let message = data
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Recording failed")
                    .to_string();
                if code != RECORDING_TARGET_CLOSED {
                    let mut pending = terminal_error.lock();
                    if pending.is_none() {
                        *pending = Some(message.clone());
                    }
                }
                let _ = error_tx.try_send((code, message));
            })));

        let handle = window.window_handle();
        cx.spawn(async move |_, cx| loop {
            let Ok((mut code, mut message)) = error_rx.recv().await else {
                break;
            };
            while let Ok((next_code, next_message)) = error_rx.try_recv() {
                if code == RECORDING_TARGET_CLOSED && next_code != RECORDING_TARGET_CLOSED {
                    code = next_code;
                    message = next_message;
                }
            }
            let continue_listening = cx.update(|cx| {
                let Some(handle) = handle.downcast::<Self>() else {
                    return false;
                };
                handle
                    .update(cx, |this, window, cx| {
                        if this.recording_error_generation != generation
                            || this.mode != Mode::Recording
                        {
                            return false;
                        }
                        #[cfg(target_os = "macos")]
                        if recorder::state() == recorder::RecorderState::Starting {
                            if code == RECORDING_TARGET_CLOSED {
                                this.start_target_closed = true;
                            }
                            return true;
                        }
                        if code == RECORDING_TARGET_CLOSED {
                            #[cfg(target_os = "macos")]
                            if recorder::state() == recorder::RecorderState::Stopping {
                                this.stop_target_closed = true;
                                return true;
                            }
                            !this.finish(false, window, cx)
                        } else if recorder::state() == recorder::RecorderState::Stopping {
                            true
                        } else {
                            this.fail_recording(message, window, cx);
                            false
                        }
                    })
                    .unwrap_or(false)
            });
            if !continue_listening {
                break;
            }
        })
        .detach();
    }

    fn fail_recording(&mut self, message: String, window: &mut Window, cx: &mut Context<Self>) {
        self.recording_error_generation = self.recording_error_generation.wrapping_add(1);
        self.recorder_error_subscription = None;
        self.recorder_terminal_error = None;
        #[cfg(target_os = "macos")]
        {
            self.stop_previous_state = None;
            self.stop_target_closed = false;
            self.start_target_closed = false;
            self.start_stop_requested = None;
        }
        self.hide_camera_preview(cx);
        recorder::force_idle();
        let message = match self.project.take() {
            Some(project) => format!(
                "{message} Partial recording files may be available at {}.",
                project.display()
            ),
            None => message,
        };
        self.started_at = None;
        self.paused_at = None;
        self.paused_total = Duration::ZERO;

        close_toolbar(window);
        registry::close(RegistryKind::RecordingControl, cx);
        Self::finish_closed_handoff(cx);
        crate::intents::refresh_shell(cx);
        crate::windows::toast::Toast::show(cx, "Recording failed", message);
    }

    pub(crate) fn finish(
        &mut self,
        discard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if recorder::state() == recorder::RecorderState::Stopping {
            return false;
        }
        #[cfg(target_os = "macos")]
        if recorder::state() == recorder::RecorderState::Starting {
            self.start_stop_requested = Some(discard);
        }
        let fallback_duration = self.recording_duration(cx).as_secs_f64();
        let service = crate::state::state(cx);
        let result = match recorder::stop(&service.daemon) {
            Ok(recorder::RecordingStop::Stopped(result)) => result,
            #[cfg(target_os = "macos")]
            Ok(recorder::RecordingStop::Pending(response, previous)) => {
                self.stop_previous_state = Some(previous);
                self.stop_target_closed = false;
                let generation = self.recording_error_generation;
                let (sender, receiver) = smol::channel::bounded(1);
                std::thread::spawn(move || {
                    let result = match response.recv() {
                        Ok(result) => result,
                        Err(_) => Err(crate::daemon::DaemonRpcError {
                            code: "DAEMON_UNAVAILABLE".into(),
                            message: "The recorder disconnected while stopping".into(),
                        }),
                    };
                    let _ = sender.try_send(result);
                });
                cx.spawn_in(window, async move |entity, cx| {
                    let Ok(result) = receiver.recv().await else {
                        return;
                    };
                    let _ = entity.update_in(cx, |this, window, cx| {
                        if this.recording_error_generation != generation
                            || this.mode != Mode::Recording
                        {
                            return;
                        }
                        let previous = this.stop_previous_state.take().unwrap_or(previous);
                        match result {
                            Ok(value) => {
                                recorder::complete_stop(&crate::state::state(cx).daemon);
                                let result = crate::daemon::recording_stop_info(&value);
                                this.finish_stopped(discard, result, fallback_duration, window, cx);
                            }
                            Err(error) => {
                                recorder::restore_after_failed_stop(previous);
                                let target_closed = std::mem::take(&mut this.stop_target_closed);
                                if !this.stop_failed(
                                    anyhow::Error::new(error),
                                    target_closed,
                                    window,
                                    cx,
                                ) && target_closed
                                {
                                    this.finish(discard, window, cx);
                                }
                            }
                        }
                    });
                })
                .detach();
                cx.notify();
                return false;
            }
            Err(error) => return self.stop_failed(error, false, window, cx),
        };
        self.finish_stopped(discard, result, fallback_duration, window, cx)
    }

    fn stop_failed(
        &mut self,
        error: anyhow::Error,
        retry_queued: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        eprintln!("[recorder] stop failed: {error}");
        let terminal_error = self
            .recorder_terminal_error
            .as_ref()
            .and_then(|pending| pending.lock().take());
        if let Some(message) = terminal_error {
            self.fail_recording(message, window, cx);
            return true;
        }
        #[cfg(target_os = "macos")]
        if error
            .downcast_ref::<crate::daemon::DaemonRpcError>()
            .is_some_and(|error| {
                matches!(error.code.as_str(), "CAPTURE_ERROR" | "DAEMON_UNAVAILABLE")
            })
        {
            self.fail_recording(error.to_string(), window, cx);
            return true;
        }
        if retry_queued {
            return false;
        }
        crate::windows::toast::Toast::show(
            cx,
            "Recording not finalized",
            "Stop failed. The recording controls remain available so you can retry.",
        );
        cx.notify();
        false
    }

    fn finish_stopped(
        &mut self,
        discard: bool,
        result: crate::daemon::ScreenRecordingStopInfo,
        fallback_duration: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let duration = result.duration.unwrap_or(fallback_duration);
        let service = crate::state::state(cx);
        let terminal_error = self
            .recorder_terminal_error
            .as_ref()
            .and_then(|pending| pending.lock().take());
        if let Some(error) = terminal_error {
            self.fail_recording(error, window, cx);
            return true;
        }
        self.hide_camera_preview(cx);
        self.recording_error_generation = self.recording_error_generation.wrapping_add(1);
        self.recorder_error_subscription = None;
        self.recorder_terminal_error = None;
        #[cfg(target_os = "macos")]
        {
            self.stop_previous_state = None;
            self.stop_target_closed = false;
            self.start_target_closed = false;
            self.start_stop_requested = None;
        }

        let project = self.project.take();
        self.started_at = None;
        self.paused_at = None;
        self.paused_total = Duration::ZERO;

        close_toolbar(window);
        registry::close(RegistryKind::RecordingControl, cx);
        Self::finish_closed_handoff(cx);
        crate::intents::refresh_shell(cx);

        let Some(project) = project else {
            return true;
        };
        if discard {
            cx.spawn(async move |_, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        std::fs::remove_dir_all(&project).map_err(|error| (project, error))
                    })
                    .await;
                let Err((project, error)) = result else {
                    return;
                };
                if error.kind() != std::io::ErrorKind::NotFound {
                    cx.update(|cx| {
                        crate::windows::toast::Toast::show(
                            cx,
                            "Recording could not be discarded",
                            format!(
                                "Check the recording folder at {}. {error}",
                                project.display()
                            ),
                        );
                    });
                }
            })
            .detach();
            return true;
        }

        if result.camera_track_missing {
            cx.defer(|_| {
                rfd::MessageDialog::new()
                    .set_level(rfd::MessageLevel::Warning)
                    .set_title("Camera recording unavailable")
                    .set_description(
                        "The recording was saved, but the camera video could not be saved.",
                    )
                    .show();
            });
        }

        let (history_enabled, max_items, show_preview, recording) = {
            let config = service.config.get();
            (
                config.history.enabled,
                config.history.max_items as usize,
                config.recording.show_preview,
                config.recording.clone(),
            )
        };
        let mut state = crate::windows::video_editor::model::VideoEditorState {
            saved_at: chrono::Utc::now().to_rfc3339(),
            recording_type: self
                .selected_ios_id
                .as_ref()
                .map(|_| "ios-device".to_string()),
            source_duration: (duration > 0.0).then_some(duration),
            ..Default::default()
        };
        state.camera_style.mirrored = recording.camera.flipped;
        if duration > 0.0 {
            state.segments = vec![crate::windows::video_editor::model::Segment::spanning(
                duration,
            )];
        }
        let history = history_enabled.then(|| {
            let write = crate::capture::coordinator::CaptureHistoryWrite::reserve(max_items, cx);
            let item = crate::history_store::HistoryItem {
                id: format!(
                    "{}",
                    chrono::Local::now().timestamp_nanos_opt().unwrap_or(0)
                ),
                timestamp: chrono::Local::now().timestamp_millis(),
                original_path: project.to_string_lossy().to_string(),
                r#type: crate::history_store::HistoryItemType::Video,
                editor_state: None,
                duration: Some(duration),
            };
            (write, item)
        });
        let finalization = show_preview.then(|| {
            let (sender, receiver) = smol::channel::bounded(1);
            crate::windows::capture_preview::CapturePreviewWindow::open_video(
                cx,
                project.clone(),
                Some(receiver),
            );
            sender
        });
        cx.spawn(async move |_, cx| {
            let state_project = project.clone();
            cx.background_executor()
                .spawn(async move {
                    if recording.auto_zoom {
                        if let Some(cursor_data) =
                            crate::video::sidecars::load_cursor(&state_project)
                        {
                            state.zoom_segments = crate::video::auto_zoom::generate(
                                &cursor_data,
                                chrono::Utc::now().timestamp_millis(),
                            );
                            if !state.zoom_segments.is_empty() {
                                state.ui.sidebar_open = true;
                                state.ui.sidebar_tab = "zoom".to_string();
                            }
                        }
                    }
                    save_initial_recording_state(&state_project, &state);
                })
                .await;

            if let Some((write, item)) = history {
                if let Some((title, message)) = write.persist(item, cx).await.warning("recording") {
                    cx.update(|cx| crate::windows::toast::Toast::show(cx, title, message));
                }
            }
            drop(finalization);
            if !show_preview {
                cx.update(|cx| {
                    let path = project.to_string_lossy().into_owned();
                    crate::windows::video_editor::VideoEditorWindow::open(cx, Some(path));
                });
            }
        })
        .detach();
        true
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == Mode::Recording {
            self.finish(true, window, cx);
            return;
        }
        self.close_pre_recording_toolbar(window, cx);
        registry::forget(RegistryKind::RecordingControl, cx);
        Self::finish_closed_handoff(cx);
    }

    fn close_pre_recording_toolbar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.recording_error_generation = self.recording_error_generation.wrapping_add(1);
        self.countdown_generation += 1;
        self.countdown_active = false;
        self.countdown_remaining = None;
        #[cfg(target_os = "macos")]
        self.cancel_camera_protection(cx);
        #[cfg(target_os = "macos")]
        if self.detach_camera_preview() && !crate::intents::recording_camera_hide_queued(cx) {
            let _ = crate::intents::begin_recording_camera_hide(cx);
        }
        #[cfg(not(target_os = "macos"))]
        self.hide_camera_preview(cx);
        close_toolbar(window);
    }

    fn sync_window_bounds(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        device_menu_open: bool,
    ) {
        let width = self.measured_width.unwrap_or_else(|| {
            let mut estimate =
                recording_control_width(self.mode == Mode::Recording, self.target_name.is_some());
            if self.mode == Mode::Recording && !self.camera_locked {
                estimate -= 48.0;
            }
            estimate
        });
        let keyboard_tooltip = self.keyboard_tooltip_visible(window, cx);
        let bounds = bar_bounds(
            cx,
            self.rect,
            width,
            device_menu_open,
            self.countdown_active,
            keyboard_tooltip,
        );
        if super::desktop_window_bounds(window) == bounds {
            #[cfg(all(target_os = "macos", not(test)))]
            self.pending_native_bounds.set(None);
            return;
        }
        #[cfg(all(target_os = "macos", not(test)))]
        {
            if self.pending_native_bounds.replace(Some(bounds)).is_some() {
                return;
            }
            let pending = self.pending_native_bounds.clone();
            let handle = window.window_handle();
            cx.spawn(async move |_, cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    let Some(bounds) = pending.take() else {
                        return;
                    };
                    if super::set_macos_window_bounds(window, bounds) {
                        window.bounds_changed(cx);
                    } else {
                        window.resize(bounds.size);
                    }
                });
            })
            .detach();
        }
        #[cfg(any(not(target_os = "macos"), test))]
        {
            #[cfg(all(windows, not(test)))]
            let reposition = window.bounds().origin != bounds.origin;
            window.resize(bounds.size);
            #[cfg(all(windows, not(test)))]
            if reposition {
                if let Some(hwnd) = crate::windows::window_hwnd(window) {
                    let scale = window.scale_factor();
                    cx.spawn(async move |_, _| {
                        crate::system::window_composition::apply_window_origin(
                            hwnd,
                            bounds.origin,
                            scale,
                        );
                    })
                    .detach();
                }
            }
        }
    }

    fn keyboard_tooltip_visible(&self, window: &Window, cx: &App) -> bool {
        let active = window.is_window_active();
        #[cfg(feature = "parity")]
        let active =
            active || std::env::var_os(crate::ui::parity::daemon_fixture::PROCESS_ENV).is_some();
        cfg!(target_os = "macos")
            && active
            && herogpui::extend::focus_visible(cx)
            && self.focus_handle.contains_focused(window, cx)
    }
}

#[cfg(all(target_os = "macos", feature = "parity"))]
pub(crate) const PARITY_RECORDING_CONTROL_WINDOW_ID: i64 = 7103;

fn remove_empty_failed_start_project(project: &std::path::Path, error: &anyhow::Error) -> bool {
    let definite_failure = error
        .downcast_ref::<crate::daemon::DaemonRpcError>()
        .is_some_and(|error| error.code != "DAEMON_UNAVAILABLE");
    definite_failure && std::fs::remove_dir(project).is_ok()
}

fn device_menu_placement(owner: impl Into<SharedString>) -> MenuPlacement {
    MenuPlacement::below(owner)
        .min_width(px(DEVICE_DROPDOWN_MIN_WIDTH))
        .max_width(px(DEVICE_DROPDOWN_WIDTH))
        .max_height(px(DEVICE_DROPDOWN_HEIGHT))
        .offset(point(
            px((chrome::OVERLAY_TARGET_TRIGGER_WIDTH - DEVICE_DROPDOWN_WIDTH) / 2.0),
            px(8.0),
        ))
}

fn merge_device_refresh(
    devices: &mut crate::system::devices::MediaDeviceLists,
    current_request: u64,
    request: u64,
    kind: crate::system::devices::DeviceKind,
    listed: anyhow::Result<crate::system::devices::MediaDeviceLists>,
) -> bool {
    if current_request != request {
        return false;
    }
    let listed = match listed {
        Ok(listed) => listed,
        Err(error) => {
            eprintln!("[devices] list failed: {error}");
            return false;
        }
    };
    let (current_devices, current_default, reported_devices, reported_default) = match kind {
        crate::system::devices::DeviceKind::Microphone => (
            &mut devices.microphones,
            &mut devices.default_microphone_id,
            listed.microphones,
            listed.default_microphone_id,
        ),
        crate::system::devices::DeviceKind::Camera => (
            &mut devices.cameras,
            &mut devices.default_camera_id,
            listed.cameras,
            listed.default_camera_id,
        ),
    };
    if *current_devices == reported_devices && *current_default == reported_default {
        return false;
    }
    *current_devices = reported_devices;
    *current_default = reported_default;
    true
}

fn take_ready_device_menu(
    pending: &mut Option<(DeviceMenuKind, SharedString)>,
    window_height: gpui::Pixels,
) -> Option<(DeviceMenuKind, SharedString)> {
    if window_height < px(DEVICE_MENU_WINDOW_HEIGHT) {
        return None;
    }
    pending.take()
}

fn save_initial_recording_state(
    project: &std::path::Path,
    state: &crate::windows::video_editor::model::VideoEditorState,
) {
    crate::windows::video_editor::model::save_state_with_camera_segments_presence(
        project, state, false,
    );
}

fn recording_control_width(recording: bool, has_target_name: bool) -> f32 {
    let width = chrome::recording_control_width(recording, has_target_name);
    if cfg!(target_os = "macos") && !recording {
        width + 48.0
    } else {
        width
    }
}

fn report_toggle_failure(cx: &mut Context<RecordingControl>, error: &anyhow::Error) {
    crate::windows::toast::Toast::show(cx, "Recording control failed", error.to_string());
}

const CAMERA_PREVIEW_SIZE: i32 = 270;
const CAMERA_PREVIEW_MARGIN: i32 = 32;

fn camera_preview_position(rect: ScreenRect) -> (i32, i32) {
    (
        rect.x + (rect.width - CAMERA_PREVIEW_SIZE - CAMERA_PREVIEW_MARGIN).max(0),
        rect.y + (rect.height - CAMERA_PREVIEW_SIZE - CAMERA_PREVIEW_MARGIN).max(0),
    )
}

#[cfg(windows)]
fn exclude_toolbar_from_capture(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
    };

    unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE).is_ok() }
}

#[cfg(any(target_os = "macos", test))]
fn recording_control_exclusion_ids(
    window_id: Option<i64>,
    is_window_recording: bool,
) -> Result<Vec<i64>, &'static str> {
    if is_window_recording {
        return Ok(Vec::new());
    }
    window_id
        .map(|window_id| vec![window_id])
        .ok_or("Could not protect the recording controls from capture")
}

fn configure_toolbar_window(window: &Window) {
    super::remove_popup_frame(window);
    #[cfg(all(target_os = "macos", not(test)))]
    {
        crate::system::window_visibility::disable_macos_transitions(window);
        super::pin_macos_window_contents(window, false);
    }
    #[cfg(all(windows, not(test)))]
    {
        let Some(hwnd) = crate::windows::window_hwnd(window) else {
            return;
        };
        crate::system::window_composition::configure_transparent_surface(hwnd);
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            };
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            );
        }
        if !exclude_toolbar_from_capture(hwnd) {
            eprintln!("[recording-control] failed to exclude toolbar from capture");
        }
    }
}

fn close_toolbar(window: &mut Window) {
    #[cfg(target_os = "macos")]
    crate::system::window_visibility::hide(window);
    window.remove_window();
}

fn bar_bounds(
    cx: &mut App,
    rect: ScreenRect,
    width: f32,
    device_menu_open: bool,
    countdown_active: bool,
    keyboard_tooltip: bool,
) -> Bounds<gpui::Pixels> {
    let (work_x, work_y, work_width) = display_for_rect(cx, rect)
        .map(|display| {
            let bounds = crate::system::work_area::work_area(display.as_ref());
            (
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(bounds.size.width),
            )
        })
        .unwrap_or((0.0, 0.0, 1920.0));
    let (bar_x, mut y) = chrome::recording_bar_origin(work_x, work_y, work_width, width);
    if cfg!(target_os = "macos") && !device_menu_open && !countdown_active && !keyboard_tooltip {
        y += chrome::RECORDING_BAR_PAD_TOP;
    }
    let (window_width, bar_offset, height) =
        control_window_metrics(width, device_menu_open, countdown_active, keyboard_tooltip);
    let x = bar_x - bar_offset;
    Bounds {
        origin: gpui::point(px(x), px(y)),
        size: size(px(window_width), px(height)),
    }
}

pub(crate) fn display_for_rect(
    cx: &mut App,
    rect: ScreenRect,
) -> Option<std::rc::Rc<dyn gpui::PlatformDisplay>> {
    let displays = cx.displays();
    let geometries: Vec<_> = displays
        .iter()
        .map(|display| {
            let bounds = crate::system::work_area::display_bounds(display.as_ref());
            let scale = crate::capture::overlay::display_scale_factor(display.as_ref(), cx);
            capture_display_geometry(bounds, scale)
        })
        .collect();
    #[cfg(target_os = "macos")]
    let selected = *geometries.get(display_matching_rect(&geometries, rect)?)?;
    #[cfg(not(target_os = "macos"))]
    let selected = chrome::display_containing(
        &geometries,
        rect.x as f32 + rect.width as f32 / 2.0,
        rect.y as f32 + rect.height as f32 / 2.0,
    )?;
    displays
        .into_iter()
        .zip(geometries)
        .find_map(|(display, geometry)| (geometry == selected).then_some(display))
}

#[cfg(any(target_os = "macos", test))]
fn display_matching_rect(displays: &[(f32, f32, f32, f32)], rect: ScreenRect) -> Option<usize> {
    let (left, top) = (rect.x as f32, rect.y as f32);
    let (right, bottom) = (left + rect.width as f32, top + rect.height as f32);
    let mut best = None;
    let mut best_area = 0.0;
    for (index, &(x, y, width, height)) in displays.iter().enumerate() {
        let overlap_width = (right.min(x + width) - left.max(x)).max(0.0);
        let overlap_height = (bottom.min(y + height) - top.max(y)).max(0.0);
        let area = overlap_width * overlap_height;
        if area > best_area {
            best = Some(index);
            best_area = area;
        }
    }
    best.or_else(|| (!displays.is_empty()).then_some(0))
}

fn capture_display_geometry(bounds: Bounds<Pixels>, scale: f32) -> (f32, f32, f32, f32) {
    let bounds = crate::capture::overlay::physical_rect(bounds, scale);
    (
        bounds.x as f32,
        bounds.y as f32,
        bounds.width as f32,
        bounds.height as f32,
    )
}

fn control_window_metrics(
    width: f32,
    device_menu_open: bool,
    countdown_active: bool,
    keyboard_tooltip: bool,
) -> (f32, f32, f32) {
    let compact =
        cfg!(target_os = "macos") && !device_menu_open && !countdown_active && !keyboard_tooltip;
    let window_width = if compact {
        width
    } else {
        chrome::recording_window_width(width)
    };
    let bar_offset = ((window_width - width) / 2.0).round();
    let height = if device_menu_open {
        DEVICE_MENU_WINDOW_HEIGHT
    } else if countdown_active {
        COUNTDOWN_WINDOW_HEIGHT
    } else if compact {
        chrome::recording_inner_bar_height()
    } else {
        chrome::RECORDING_WINDOW_HEIGHT
    };
    (window_width, bar_offset, height)
}

fn overlay_icon(
    id: &'static str,
    icon: &'static str,
    tooltip: &'static str,
    disabled: bool,
    on_click: impl Fn(&mut RecordingControl, &mut Window, &mut Context<RecordingControl>) + 'static,
    cx: &mut Context<RecordingControl>,
    presenter: &Arc<crate::ui::tooltip::NativeTooltipPresenter>,
) -> AnyElement {
    RecordingControl::tooltip(
        id,
        tooltip,
        toolbar::icon(id, icon, tooltip)
            .is_disabled(disabled)
            .on_press(cx.listener(move |this, _, window, cx| {
                on_click(this, window, cx);
            })),
        None,
        presenter,
    )
}

fn width_probe(cx: &mut Context<RecordingControl>) -> AnyElement {
    let owner = cx.entity().downgrade();
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _window: &mut Window, app: &mut App| {
            let width = f32::from(bounds.size.width);
            if width <= 0.0 {
                return;
            }
            let _ = owner.update(app, |this, cx| {
                if this.measured_width == Some(width) {
                    return;
                }
                this.measured_width = Some(width);
                cx.notify();
            });
        },
        |_bounds, _state: (), _window, _app| {},
    )
    .absolute()
    .size_full()
    .into_any_element()
}

impl Render for RecordingControl {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.pending_device_menu.is_none() && !self.menu.is_present(cx) {
            self.sync_window_bounds(window, cx, false);
        }
        let theme = active_theme(cx);
        let paused = recorder::state() == recorder::RecorderState::Paused;
        let starting = recorder::state() == recorder::RecorderState::Starting;
        let stopping = recorder::state() == recorder::RecorderState::Stopping;
        let toggles = self.input_toggles(cx);
        let countdown = self.countdown_remaining;
        let compact = cfg!(target_os = "macos")
            && !self.countdown_active
            && self.pending_device_menu.is_none()
            && !self.menu.is_present(cx)
            && !self.keyboard_tooltip_visible(window, cx);
        let top_padding = if compact {
            0.0
        } else {
            chrome::RECORDING_BAR_PAD_TOP
        };

        let mut bar = toolbar::surface(&theme).h(px(chrome::recording_inner_bar_height()));

        if let Some(name) = &self.target_name {
            use herogpui::a11y::{A11y as _, Name, Role};

            let bar_width = self
                .measured_width
                .unwrap_or_else(|| recording_control_width(self.mode == Mode::Recording, true));
            let window_width = f32::from(window.bounds().size.width);
            let bar_offset = ((window_width - bar_width) / 2.0).max(0.0);
            let tooltip_body_width = (window_width
                - bar_offset
                - chrome::OVERLAY_BORDER_WIDTH
                - chrome::OVERLAY_SURFACE_PADDING
                - chrome::RECORDING_TARGET_CHIP_MAX_W
                - 8.0
                - 3.0
                - 22.0)
                .max(1.0);
            let target_label = div()
                .id("recording-target-label")
                .tab_index(0)
                .a11y_named(Role::Label, &Name::labelled(name.clone()))
                .debug_selector(|| "recording-target-label".into())
                .max_w(px(chrome::RECORDING_TARGET_CHIP_MAX_W))
                .truncate()
                .px(px(4.0))
                .text_size(px(12.0))
                .child(name.clone())
                .map(|label| crate::ui::primitives::keyboard_focus_ring(label, &theme, cx));
            bar = bar
                .child(Self::tooltip(
                    "recording-target-tip",
                    name.clone(),
                    target_label,
                    Some(tooltip_body_width),
                    &self.tooltip_presenter,
                ))
                .child(toolbar::hairline(&theme));
        }

        #[cfg(feature = "parity")]
        let bar = bar.on_children_prepainted(|bounds, _, _| {
            crate::ui::parity::record("recording-controls", bounds);
        });
        let bar = bar.id("recording-control-bar");

        if self.mode == Mode::PreRecording {
            let start_name = "Start recording";
            return recording_shell(
                self,
                bar.child(Self::tooltip(
                    "recording-start",
                    start_name,
                    toolbar::button("recording-start", start_name)
                        .is_disabled(!self.start_enabled(cx))
                        .child(toolbar::filled_glyph(theme.accent, true))
                        .on_press(cx.listener(|this, _, window, cx| this.start(window, cx))),
                    None,
                    &self.tooltip_presenter,
                ))
                .child(toolbar::hairline(&theme))
                .children(toggles)
                .child(toolbar::hairline(&theme))
                .child(overlay_icon(
                    "recording-cancel",
                    "x",
                    "Close",
                    self.countdown_active,
                    |this, window, cx| this.cancel(window, cx),
                    cx,
                    &self.tooltip_presenter,
                )),
                &self.menu,
                &self.tooltip_presenter,
                countdown,
                top_padding,
                window,
                cx,
            );
        }

        let pause_disabled = starting || stopping;
        #[cfg(target_os = "macos")]
        let pause_disabled = pause_disabled || self.pause_pending;
        let stop_name = "Stop recording";
        recording_shell(
            self,
            bar.child(overlay_icon(
                "recording-pause",
                if paused { "play" } else { "pause" },
                if paused {
                    "Resume recording"
                } else {
                    "Pause recording"
                },
                pause_disabled,
                |this, _window, cx| this.toggle_pause(cx),
                cx,
                &self.tooltip_presenter,
            ))
            .child(Self::tooltip(
                "recording-stop",
                stop_name,
                toolbar::button("recording-stop", stop_name)
                    .is_disabled(stopping)
                    .child(toolbar::filled_glyph(theme.destructive, false))
                    .on_press(cx.listener(|this, _, window, cx| {
                        this.finish(false, window, cx);
                    })),
                None,
                &self.tooltip_presenter,
            ))
            .child(
                div()
                    .min_w(px(64.0))
                    .font_family(crate::ui::colors::MONO_FONT)
                    .text_size(px(12.0))
                    .text_center()
                    .child(if stopping {
                        "Stopping".to_string()
                    } else if starting {
                        "Starting".to_string()
                    } else {
                        crate::util::format::format_elapsed(self.elapsed)
                    }),
            )
            .child(toolbar::hairline(&theme))
            .children(toggles)
            .child(toolbar::hairline(&theme))
            .child(overlay_icon(
                "recording-discard",
                "trash-2",
                "Discard recording",
                stopping,
                |this, window, cx| {
                    this.finish(true, window, cx);
                },
                cx,
                &self.tooltip_presenter,
            )),
            &self.menu,
            &self.tooltip_presenter,
            None,
            top_padding,
            window,
            cx,
        )
    }
}

fn recording_shell(
    control: &RecordingControl,
    bar: impl IntoElement,
    menu: &MenuHandle,
    presenter: &Arc<crate::ui::tooltip::NativeTooltipPresenter>,
    countdown: Option<u32>,
    top_padding: f32,
    window: &mut Window,
    cx: &mut Context<RecordingControl>,
) -> impl IntoElement {
    let content = crate::ui::font::root()
        .id("recording-control")
        .track_focus(&control.focus_handle)
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .on_key_down(
            cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                this.tooltip_presenter.dismiss_active(Some(window));
                if event.keystroke.key != "escape" {
                    return;
                }
                if this.dismiss_device_menu(window, cx) {
                    return;
                }
                if can_cancel_with_escape(this.mode) {
                    this.cancel(window, cx);
                }
            }),
        )
        .when(cfg!(target_os = "macos"), |content| {
            content.capture_any_mouse_down(cx.listener(|_, _, _, cx| cx.notify()))
        })
        .pt(px(top_padding))
        .child(div().relative().child(bar).child(width_probe(cx)))
        .when_some(countdown, |el, seconds| {
            el.child(
                toolbar::surface(&active_theme(cx))
                    .mt(px(8.0))
                    .gap(px(12.0))
                    .px(px(16.0))
                    .py(px(10.0))
                    .child(
                        div()
                            .text_size(px(30.0))
                            .line_height(px(30.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(seconds.to_string()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child("Recording starts soon"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(active_theme(cx).muted_foreground)
                                    .child("Press Escape to cancel"),
                            ),
                    )
                    .child(overlay_icon(
                        "recording-countdown-cancel",
                        "x",
                        "Cancel countdown",
                        false,
                        |this, window, cx| this.cancel(window, cx),
                        cx,
                        presenter,
                    )),
            )
        })
        .children(menu.render(cx));
    #[cfg(target_os = "macos")]
    let content = {
        let control = cx.entity().downgrade();
        let pointer_listener = gpui::canvas(
            |_, _, _| (),
            move |_, (), window, _| {
                let entering = control.clone();
                window.on_mouse_event(move |_: &gpui::MouseMoveEvent, phase, _, cx| {
                    if phase == gpui::DispatchPhase::Capture {
                        let _ = entering.update(cx, |this, _| {
                            this.pointer_inside = true;
                            this.tooltip_presenter.set_hover_suppressed(false);
                        });
                    }
                });
                let exiting = control.clone();
                window.on_mouse_event(move |_: &gpui::MouseExitEvent, phase, window, cx| {
                    if phase == gpui::DispatchPhase::Capture {
                        let _ = exiting.update(cx, |this, _| {
                            this.pointer_inside = false;
                            this.tooltip_presenter.set_hover_suppressed(true);
                            this.tooltip_presenter.dismiss_active(Some(window));
                        });
                    }
                });
            },
        )
        .absolute()
        .inset_0();
        content.relative().child(pointer_listener)
    };
    crate::ui::window_root::focus_root(content, window, cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_start_removes_only_empty_projects_after_definite_rejection() {
        let root = tempfile::tempdir().unwrap();
        let rejection = anyhow::Error::new(crate::daemon::DaemonRpcError {
            code: "START_FAILED".into(),
            message: "rejected".into(),
        })
        .context("start failed");
        let empty = root.path().join("empty.poratake");
        std::fs::create_dir(&empty).unwrap();
        assert!(remove_empty_failed_start_project(&empty, &rejection));
        assert!(!empty.exists());

        let partial = root.path().join("partial.poratake");
        std::fs::create_dir(&partial).unwrap();
        std::fs::write(partial.join("recording.mov"), b"partial").unwrap();
        assert!(!remove_empty_failed_start_project(&partial, &rejection));
        assert_eq!(
            std::fs::read(partial.join("recording.mov")).unwrap(),
            b"partial"
        );

        let uncertain = root.path().join("uncertain.poratake");
        std::fs::create_dir(&uncertain).unwrap();
        let timeout = anyhow::anyhow!("Request timeout: screen-recorder.start");
        assert!(!remove_empty_failed_start_project(&uncertain, &timeout));
        let disconnected = anyhow::Error::new(crate::daemon::DaemonRpcError {
            code: "DAEMON_UNAVAILABLE".into(),
            message: "Daemon exited".into(),
        });
        assert!(!remove_empty_failed_start_project(
            &uncertain,
            &disconnected
        ));
        assert!(uncertain.is_dir());
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn only_a_destroyed_window_closes_the_failed_start_toolbar(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
        for (code, stays_open) in [
            ("CONFIGURATION_ERROR", true),
            (RECORDING_TARGET_CLOSED, false),
        ] {
            let project = directory.path().join(format!("{code}.poratake"));
            std::fs::create_dir(&project).unwrap();
            cx.update(|cx| {
                RecordingControl::open(
                    cx,
                    RecordingTarget::Window,
                    ScreenRect {
                        x: 100,
                        y: 100,
                        width: 400,
                        height: 250,
                    },
                    None,
                    Some(7),
                    Some("Generated window".into()),
                    PreferenceScope::Global,
                );
                registry::handle(RegistryKind::RecordingControl, cx)
                    .unwrap()
                    .downcast::<RecordingControl>()
                    .unwrap()
                    .update(cx, |toolbar, window, cx| {
                        let error = crate::daemon::DaemonRpcError {
                            code: code.into(),
                            message: "Target unavailable".into(),
                        };
                        toolbar.start_failed(&project, anyhow::Error::new(error), window, cx);
                    })
                    .unwrap();
                assert_eq!(
                    registry::handle(RegistryKind::RecordingControl, cx).is_some(),
                    stays_open
                );
                assert_eq!(RecordingControl::cancel_pre_recording(cx), stays_open);
            });
            assert!(!project.exists());
        }
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn target_closure_cancels_only_the_matching_pre_recording_countdown(
        cx: &mut gpui::TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        config.update(|config| config.recording.start_delay = 5.);
        cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            RecordingControl::open(
                cx,
                RecordingTarget::Window,
                ScreenRect {
                    x: 100,
                    y: 100,
                    width: 400,
                    height: 250,
                },
                None,
                Some(7),
                Some("Generated window".into()),
                PreferenceScope::Global,
            );
            let toolbar = registry::handle(RegistryKind::RecordingControl, cx)
                .unwrap()
                .downcast::<RecordingControl>()
                .unwrap();
            toolbar
                .update(cx, |toolbar, window, cx| {
                    toolbar.start(window, cx);
                    assert!(toolbar.countdown_active);
                    toolbar.mode = Mode::Recording;
                })
                .unwrap();
            assert!(!RecordingControl::window_target_closed(7, cx));
            toolbar
                .update(cx, |toolbar, _, _| toolbar.mode = Mode::PreRecording)
                .unwrap();
            assert!(!RecordingControl::window_target_closed(8, cx));
            assert!(RecordingControl::window_target_closed(7, cx));
            assert!(registry::handle(RegistryKind::RecordingControl, cx).is_none());
        });
        cx.executor().advance_clock(Duration::from_secs(6));
        cx.run_until_parked();
        assert_eq!(recorder::state(), recorder::RecorderState::Idle);
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn unfocused_recording_tooltip_clears_on_exit_and_returns_on_reentry(
        cx: &mut gpui::TestAppContext,
    ) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|settings| {
            settings.recording.system_audio = false;
            settings.recording.mic_enabled = false;
            settings.recording.camera.enabled = false;
        });
        cx.update(|cx| {
            herogpui::init(cx);
            crate::state::set_test_state(cx, config);
            RecordingControl::open(
                cx,
                RecordingTarget::Area,
                ScreenRect {
                    x: 0,
                    y: 0,
                    width: 640,
                    height: 360,
                },
                None,
                None,
                None,
                PreferenceScope::Global,
            );
        });
        let control = cx
            .update(|cx| registry::handle(RegistryKind::RecordingControl, cx))
            .unwrap()
            .downcast::<RecordingControl>()
            .unwrap();
        let mut visual = gpui::VisualTestContext::from_window(control.into(), cx);
        visual.settle();
        let width = visual.update(|window, _| window.bounds().size.width.as_f32());
        let mut start = None;
        let mut close = None;
        let mut system_sound = None;
        for x in (0..width.ceil() as usize).step_by(2) {
            let point = gpui::point(px(x as f32), px(26.));
            visual.simulate_mouse_move(point, None, gpui::Modifiers::none());
            visual.settle();
            match crate::windows::scroll_capture::test_native_tooltip().as_deref() {
                Some("Start recording") => start = Some(point),
                Some("Close") => close = Some(point),
                Some("Turn system sounds on") => system_sound = Some(point),
                _ => {}
            }
        }
        let start = start.expect("start control must be hit-testable");
        let close = close.expect("close control must be hit-testable");
        for (point, expected) in [(start, "Start recording"), (close, "Close")] {
            visual.simulate_mouse_move(point, None, gpui::Modifiers::none());
            visual.settle();
            assert_eq!(
                crate::windows::scroll_capture::test_native_tooltip().as_deref(),
                Some(expected)
            );
            visual.simulate_event(gpui::MouseExitEvent {
                position: gpui::point(px(width + 1.), px(26.)),
                pressed_button: None,
                modifiers: gpui::Modifiers::none(),
            });
            visual.settle();
            assert_eq!(crate::windows::scroll_capture::test_native_tooltip(), None);
            visual.simulate_mouse_move(point, None, gpui::Modifiers::none());
            visual.settle();
            assert_eq!(
                crate::windows::scroll_capture::test_native_tooltip().as_deref(),
                Some(expected)
            );
        }
        let system_sound = system_sound.expect("system sound control must be hit-testable");
        for enabled in [true, false] {
            visual.simulate_click(system_sound, gpui::Modifiers::none());
            visual.settle();
            assert_eq!(
                control
                    .update(&mut visual, |view, _, _| view.system_audio)
                    .unwrap(),
                enabled
            );
        }
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn recording_tooltip_timer_requests_show_and_dismisses_on_leave_or_key(
        cx: &mut gpui::TestAppContext,
    ) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| {
            herogpui::init(cx);
            crate::state::set_test_state(cx, config);
            RecordingControl::open(
                cx,
                RecordingTarget::Area,
                ScreenRect {
                    x: 0,
                    y: 0,
                    width: 640,
                    height: 360,
                },
                None,
                None,
                None,
                PreferenceScope::Global,
            );
        });
        let control = cx
            .update(|cx| registry::handle(RegistryKind::RecordingControl, cx))
            .unwrap()
            .downcast::<RecordingControl>()
            .unwrap();
        let mut visual = gpui::VisualTestContext::from_window(control.into(), cx);
        visual.settle();
        let microphone_point = visual.expect("microphone-tooltip-trigger").center();

        crate::system::native_tooltip::take_test_requests();
        visual.simulate_mouse_move(microphone_point, None, gpui::Modifiers::none());
        cx.executor().advance_clock(Duration::from_millis(149));
        cx.run_until_parked();
        assert!(crate::system::native_tooltip::take_test_requests().is_empty());

        cx.executor().advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        assert_eq!(
            crate::system::native_tooltip::take_test_requests(),
            [crate::system::native_tooltip::TooltipRequest::Show(
                "recording-control",
                "Select microphone".into(),
            )]
        );
        cx.simulate_keystrokes(control.into(), "escape");
        cx.run_until_parked();
        assert_eq!(
            crate::system::native_tooltip::take_test_requests(),
            [crate::system::native_tooltip::TooltipRequest::Hide(
                "recording-control"
            )]
        );

        visual.simulate_mouse_move(gpui::point(px(0.0), px(0.0)), None, gpui::Modifiers::none());
        visual.settle();
        visual.simulate_mouse_move(microphone_point, None, gpui::Modifiers::none());
        visual.settle();
        visual.simulate_mouse_move(gpui::point(px(0.0), px(0.0)), None, gpui::Modifiers::none());
        visual.settle();
        crate::system::native_tooltip::take_test_requests();
        cx.executor().advance_clock(Duration::from_millis(150));
        cx.run_until_parked();
        assert!(crate::system::native_tooltip::take_test_requests().is_empty());

        for label in [
            "Start recording",
            "Select camera",
            "Select microphone",
            "Turn system sounds off",
            "Close",
            "Start recording",
        ] {
            visual.press("tab");
            for _ in 0..3 {
                visual.settle();
            }
            let tooltip = control
                .update(&mut visual, |control, _, _| {
                    control.tooltip_presenter.active_content()
                })
                .unwrap();
            assert_eq!(tooltip.as_deref(), Some(label));
        }
    }

    #[cfg(any(target_os = "linux", windows))]
    #[herogpui::test]
    fn recording_target_title_tooltip_uses_measured_chip_bounds(cx: &mut gpui::TestAppContext) {
        use herogpui::test::TestWindowExt;

        let target_name = "Browser — an exceptionally long recording target name that must remain accessible when its visible label is truncated";
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| {
            herogpui::init(cx);
            crate::state::set_test_state(cx, config);
            RecordingControl::open(
                cx,
                RecordingTarget::Window,
                ScreenRect {
                    x: 0,
                    y: 0,
                    width: 640,
                    height: 360,
                },
                None,
                Some(1),
                Some(target_name.into()),
                PreferenceScope::Global,
            );
        });
        let control = cx
            .update(|cx| registry::handle(RegistryKind::RecordingControl, cx))
            .unwrap()
            .downcast::<RecordingControl>()
            .unwrap();
        let mut visual = gpui::VisualTestContext::from_window(control.into(), cx);
        visual.settle();
        let window_size = visual.update(|window, _| window.bounds().size);
        let target_chip = visual.expect("recording-target-label");
        let target_center = target_chip.center();
        #[cfg(target_os = "linux")]
        let tooltip_geometry = |visual: &mut gpui::VisualTestContext| {
            visual.update(|window, cx| {
                let scale = window.scale_factor();
                let background =
                    gpui::Background::from(crate::theme::vars::active_theme(cx).overlay);
                let tooltips = window
                    .painted_quads()
                    .into_iter()
                    .filter(|quad| {
                        quad.background == background
                            && (31.0..=50.0)
                                .contains(&(quad.bounds.size.height.as_f32() / scale))
                            && quad.bounds.size.width.as_f32() / scale > 35.0
                    })
                    .collect::<Vec<_>>();
                let tooltip = tooltips.first().unwrap_or_else(|| {
                    panic!(
                        "target-title tooltip must be painted after hovering {target_chip:?} at {target_center:?}"
                    )
                });
                (
                    tooltip.bounds,
                    tooltip.content_mask.bounds,
                    window.viewport_size(),
                    scale,
                    tooltips.len(),
                )
            })
        };
        visual.simulate_mouse_move(target_center, None, gpui::Modifiers::none());
        #[cfg(target_os = "linux")]
        visual.executor().advance_clock(Duration::from_millis(300));
        #[cfg(windows)]
        {
            cx.executor().advance_clock(Duration::from_millis(150));
            cx.run_until_parked();
            let tooltip = control
                .update(cx, |this, _, _| {
                    this.tooltip_presenter
                        .active_content()
                        .map(|text| text.to_string())
                })
                .unwrap();
            assert_eq!(
                tooltip.as_deref(),
                Some(target_name),
                "pointer at {target_center:?} missed target chip {target_chip:?}"
            );
        }
        visual.settle();
        #[cfg(target_os = "linux")]
        {
            let (tooltip, mask, viewport, scale, tooltip_count) = tooltip_geometry(&mut visual);
            assert_eq!(tooltip_count, 1, "only the target-title tooltip is open");
            assert_eq!(tooltip.size.height.as_f32() / scale, 48.0);
            assert!(
                tooltip.left().as_f32() >= f32::from(target_chip.right()) * scale
                    && tooltip.top().as_f32() >= 0.0
                    && tooltip.right().as_f32() <= f32::from(viewport.width) * scale
                    && tooltip.bottom().as_f32() <= f32::from(viewport.height) * scale
                    && mask.left().as_f32() <= tooltip.left().as_f32()
                    && mask.top().as_f32() <= tooltip.top().as_f32()
                    && mask.right().as_f32() >= tooltip.right().as_f32()
                    && mask.bottom().as_f32() >= tooltip.bottom().as_f32(),
                "long target-title tooltip is clipped: {tooltip:?}, mask {mask:?}"
            );
        }
        visual.simulate_mouse_move(
            gpui::point(px(-100.0), px(-100.0)),
            None,
            gpui::Modifiers::none(),
        );
        #[cfg(target_os = "linux")]
        {
            use herogpui::theme::ActiveTheme as _;

            let close_delay = visual.update(|_, cx| cx.layout().tooltip_close_delay_ms);
            visual
                .executor()
                .advance_clock(Duration::from_millis(close_delay + 1));
        }
        visual.settle();
        visual.press(if cfg!(windows) { "tab" } else { "tab tab" });
        assert!(visual.update(|window, cx| {
            window.focused(cx).is_some() && herogpui::extend::focus_visible(cx)
        }));
        #[cfg(windows)]
        assert_eq!(
            control
                .update(cx, |this, _, _| this.tooltip_presenter.active_content())
                .unwrap()
                .as_deref(),
            Some(target_name),
        );
        #[cfg(target_os = "linux")]
        {
            let (tooltip, mask, viewport, scale, tooltip_count) = tooltip_geometry(&mut visual);
            assert_eq!(
                tooltip_count, 1,
                "keyboard focus opens the target-title tooltip"
            );
            assert_eq!(tooltip.size.height.as_f32() / scale, 48.0);
            assert!(tooltip.size.width.as_f32() / scale > f32::from(target_chip.size.width));
            assert!(
                tooltip.left().as_f32() >= f32::from(target_chip.right()) * scale
                    && tooltip.top().as_f32() >= 0.0
                    && tooltip.right().as_f32() <= f32::from(viewport.width) * scale
                    && tooltip.bottom().as_f32() <= f32::from(viewport.height) * scale
                    && mask.left().as_f32() <= tooltip.left().as_f32()
                    && mask.top().as_f32() <= tooltip.top().as_f32()
                    && mask.right().as_f32() >= tooltip.right().as_f32()
                    && mask.bottom().as_f32() >= tooltip.bottom().as_f32(),
                "keyboard target-title tooltip is clipped: {tooltip:?}, mask {mask:?}"
            );
        }
        assert_eq!(visual.update(|window, _| window.bounds().size), window_size);
    }

    #[cfg(any(target_os = "linux", windows))]
    #[herogpui::test]
    fn recording_target_tooltip_preserves_unbroken_and_unicode_titles_at_each_scale(
        cx: &mut gpui::TestAppContext,
    ) {
        use herogpui::test::TestWindowExt;

        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| {
            herogpui::init(cx);
            crate::state::set_test_state(cx, config);
            RecordingControl::open(
                cx,
                RecordingTarget::Window,
                ScreenRect {
                    x: 0,
                    y: 0,
                    width: 640,
                    height: 360,
                },
                None,
                Some(1),
                Some("Initial title".into()),
                PreferenceScope::Global,
            );
        });
        let window_handle = cx
            .update(|cx| registry::handle(RegistryKind::RecordingControl, cx))
            .unwrap();
        let control = window_handle.downcast::<RecordingControl>().unwrap();
        let mut visual = gpui::VisualTestContext::from_window(window_handle, cx);
        visual.settle();
        let unbroken = "A".repeat(160);
        let unicode = "Cafe\u{0301} 👩‍💻 — recording window title 👨‍👩‍👧‍👦".to_owned();
        for (title, scale) in [
            (unbroken.clone(), 1.0),
            (unbroken, 2.0),
            (unicode.clone(), 1.0),
            (unicode, 2.0),
        ] {
            cx.update(|cx| {
                RecordingControl::open(
                    cx,
                    RecordingTarget::Window,
                    ScreenRect {
                        x: 0,
                        y: 0,
                        width: 640,
                        height: 360,
                    },
                    None,
                    Some(1),
                    Some(title.clone()),
                    PreferenceScope::Global,
                );
            });
            visual.simulate_scale_factor_change(scale);
            visual.settle();
            control
                .update(cx, |this, window, cx| window.focus(&this.focus_handle, cx))
                .unwrap();
            visual.settle();
            visual.simulate_mouse_move(
                gpui::point(px(-100.0), px(-100.0)),
                None,
                gpui::Modifiers::none(),
            );
            visual.settle();
            visual.press(if cfg!(windows) { "tab" } else { "tab tab" });

            #[cfg(windows)]
            assert_eq!(
                control
                    .update(cx, |this, _, _| this.tooltip_presenter.active_content())
                    .unwrap()
                    .as_deref(),
                Some(title.as_str()),
            );
            #[cfg(target_os = "linux")]
            {
                let target_chip = visual.expect("recording-target-label");
                let (tip, mask, viewport, actual_scale, count) = visual.update(|window, cx| {
                    let background =
                        gpui::Background::from(crate::theme::vars::active_theme(cx).overlay);
                    let tips = window
                        .painted_quads()
                        .into_iter()
                        .filter(|quad| {
                            quad.background == background
                                && (31.0..=50.0)
                                    .contains(&(quad.bounds.size.height.as_f32() / scale))
                                && quad.bounds.size.width.as_f32() / scale > 35.0
                        })
                        .collect::<Vec<_>>();
                    let tip = tips.first().expect("target tooltip must be painted");
                    (
                        tip.bounds,
                        tip.content_mask.bounds,
                        window.viewport_size(),
                        window.scale_factor(),
                        tips.len(),
                    )
                });
                assert_eq!(actual_scale, scale);
                assert_eq!(count, 1, "only the target title tooltip is open");
                assert_eq!(tip.size.height.as_f32() / scale, 48.0);
                assert!(
                tip.left().as_f32() >= f32::from(target_chip.right()) * scale
                    && tip.top().as_f32() >= 0.0
                    && tip.right().as_f32() <= f32::from(viewport.width) * scale
                    && tip.bottom().as_f32() <= f32::from(viewport.height) * scale
                    && mask.left().as_f32() <= tip.left().as_f32()
                    && mask.top().as_f32() <= tip.top().as_f32()
                    && mask.right().as_f32() >= tip.right().as_f32()
                    && mask.bottom().as_f32() >= tip.bottom().as_f32(),
                "target tooltip is clipped at {scale}x for title {title:?}: {tip:?}, mask {mask:?}"
            );
            }
            assert_eq!(
                control
                    .update(cx, |this, _, _| this
                        .target_name
                        .as_ref()
                        .unwrap()
                        .to_string())
                    .unwrap(),
                title,
                "the input title must remain unchanged"
            );
        }
    }

    #[herogpui::test]
    fn recording_countdown_displays_remaining_seconds(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        config.update(|config| config.recording.start_delay = 5.);
        let toolbar = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            RecordingControl::open(
                cx,
                RecordingTarget::Area,
                ScreenRect {
                    x: 0,
                    y: 0,
                    width: 640,
                    height: 360,
                },
                None,
                None,
                None,
                PreferenceScope::Global,
            );
            let toolbar = registry::handle(RegistryKind::RecordingControl, cx)
                .unwrap()
                .downcast::<RecordingControl>()
                .unwrap();
            toolbar
                .update(cx, |control, window, cx| control.start(window, cx))
                .unwrap();
            toolbar
        });
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(2_250));
        cx.run_until_parked();
        let remaining = |cx: &mut gpui::TestAppContext| {
            toolbar
                .update(cx, |control, _, _| control.countdown_remaining)
                .unwrap()
        };
        assert_eq!(remaining(cx), Some(3));
        cx.executor().advance_clock(Duration::from_millis(750));
        cx.run_until_parked();
        assert_eq!(remaining(cx), Some(2));
        cx.update(|cx| {
            assert!(RecordingControl::cancel_pre_recording(cx));
        });
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn replacing_a_pre_recording_target_preserves_the_toolbar_and_cancels_the_countdown(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|config| config.recording.start_delay = 1.);
        let rect = ScreenRect {
            x: 100,
            y: 200,
            width: 640,
            height: 360,
        };
        let original = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            RecordingControl::open(
                cx,
                RecordingTarget::Area,
                rect,
                None,
                None,
                None,
                PreferenceScope::Global,
            );
            let toolbar = registry::handle(RegistryKind::RecordingControl, cx).unwrap();
            toolbar
                .downcast::<RecordingControl>()
                .unwrap()
                .update(cx, |control, window, cx| {
                    control.microphone = true;
                    control.start(window, cx);
                    assert!(control.countdown_active);
                })
                .unwrap();
            toolbar
        });
        cx.update(|cx| {
            RecordingControl::open(
                cx,
                RecordingTarget::Window,
                rect,
                None,
                Some(7),
                Some("Generated window".into()),
                PreferenceScope::Global,
            );
            assert_eq!(
                registry::handle(RegistryKind::RecordingControl, cx),
                Some(original)
            );
            original
                .downcast::<RecordingControl>()
                .unwrap()
                .update(cx, |control, _, _| {
                    assert_eq!(control.window_id, Some(7));
                    assert!(control.microphone && !control.countdown_active);
                    assert_eq!(control.countdown_remaining, None);
                })
                .unwrap();
        });
        cx.executor().advance_clock(Duration::from_secs(2));
        cx.run_until_parked();
        assert_eq!(recorder::state(), recorder::RecorderState::Idle);
    }

    #[herogpui::test]
    fn pre_recording_preferences_follow_the_entry_scope(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        config.update(|config| {
            config.recording.system_audio = false;
            config.recording.mic_enabled = true;
            config.all_in_one.recording.system_audio = true;
            config.all_in_one.recording.mic_enabled = false;
        });
        cx.update(|cx| {
            crate::state::set_test_state(cx, config.clone());
            let rect = ScreenRect {
                x: 0,
                y: 0,
                width: 640,
                height: 360,
            };
            RecordingControl::open(
                cx,
                RecordingTarget::Area,
                rect,
                None,
                None,
                None,
                PreferenceScope::Global,
            );
            let global = registry::handle(RegistryKind::RecordingControl, cx)
                .unwrap()
                .downcast::<RecordingControl>()
                .unwrap();
            global
                .update(cx, |control, _, cx| {
                    assert!(!control.system_audio);
                    assert!(control.microphone);
                    assert!(control.set_system_audio(true, cx));
                    assert!(control.set_microphone(false, control.selected_mic_id.clone(), cx));
                })
                .unwrap();
            assert!(config.get().recording.system_audio);
            assert!(!config.get().recording.mic_enabled);

            RecordingControl::open(
                cx,
                RecordingTarget::Area,
                rect,
                None,
                None,
                None,
                PreferenceScope::AllInOne,
            );
            let all_in_one = registry::handle(RegistryKind::RecordingControl, cx)
                .unwrap()
                .downcast::<RecordingControl>()
                .unwrap();
            all_in_one
                .update(cx, |control, _, cx| {
                    assert_eq!(control.preference_scope, PreferenceScope::AllInOne);
                    assert!(control.system_audio);
                    assert!(!control.microphone);
                    assert!(control.set_system_audio(false, cx));
                })
                .unwrap();
            assert!(config.get().recording.system_audio);
            assert!(!config.get().all_in_one.recording.system_audio);

            config.update(|config| {
                config.all_in_one.remember_choices = false;
                config.all_in_one.recording.mic_enabled = true;
                config.all_in_one.recording.camera_enabled = true;
            });
            all_in_one
                .update(cx, |control, _, cx| {
                    assert!(control.set_system_audio(true, cx));
                    control.mode = Mode::Recording;
                    control.persist_pre_recording_preference(
                        RecordingPreference::Microphone(false),
                        cx,
                    );
                    control
                        .persist_pre_recording_preference(RecordingPreference::Camera(false), cx);
                    control.mode = Mode::PreRecording;
                })
                .unwrap();
            assert!(!config.get().all_in_one.recording.system_audio);
            assert!(config.get().all_in_one.recording.mic_enabled);
            assert!(config.get().all_in_one.recording.camera_enabled);

            RecordingControl::open(
                cx,
                RecordingTarget::Area,
                rect,
                None,
                None,
                None,
                PreferenceScope::AllInOne,
            );
            all_in_one
                .update(cx, |control, _, _| assert!(control.system_audio))
                .unwrap();
            registry::close(RegistryKind::RecordingControl, cx);
            RecordingControl::open(
                cx,
                RecordingTarget::Area,
                rect,
                None,
                None,
                None,
                PreferenceScope::AllInOne,
            );
            let reopened = registry::handle(RegistryKind::RecordingControl, cx)
                .unwrap()
                .downcast::<RecordingControl>()
                .unwrap();
            reopened
                .update(cx, |control, _, _| {
                    assert!(control.system_audio);
                    assert!(!control.microphone);
                    assert!(!control.camera);
                })
                .unwrap();
            registry::close(RegistryKind::RecordingControl, cx);
        });
    }

    #[test]
    fn camera_bubble_sits_in_the_recording_area_bottom_right() {
        assert_eq!(
            camera_preview_position(ScreenRect {
                x: 100,
                y: 50,
                width: 1920,
                height: 1080,
            }),
            (100 + 1920 - 270 - 32, 50 + 1080 - 270 - 32)
        );
        assert_eq!(
            camera_preview_position(ScreenRect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            }),
            (0, 0)
        );
    }

    #[test]
    fn display_recording_requires_a_native_control_window_id() {
        assert_eq!(
            recording_control_exclusion_ids(Some(41), false),
            Ok(vec![41])
        );
        assert_eq!(
            recording_control_exclusion_ids(None, false),
            Err("Could not protect the recording controls from capture")
        );
        assert_eq!(recording_control_exclusion_ids(None, true), Ok(Vec::new()));
    }

    #[test]
    fn generated_recording_leaves_camera_segments_uninitialized_for_sidecar_mapping() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Generated.poratake");
        std::fs::create_dir(&project).unwrap();
        let state = crate::windows::video_editor::model::VideoEditorState {
            source_duration: Some(2.0),
            segments: vec![crate::windows::video_editor::model::Segment::spanning(2.0)],
            ..Default::default()
        };
        std::fs::write(
            crate::video::project::camera_meta_path(&project),
            r#"{"duration":2.0,"visibleRanges":[{"start":0.25,"end":1.25}]}"#,
        )
        .unwrap();

        save_initial_recording_state(&project, &state);

        let (loaded, camera_segments_were_saved) =
            crate::windows::video_editor::model::load_state_with_camera_segments_presence(&project);
        assert!(!camera_segments_were_saved);
        let meta = crate::video::sidecars::load_camera_meta(&project).unwrap();
        let camera_segments = crate::video::sidecars::map_visible_ranges_to_segments(
            meta.visible_ranges.as_deref(),
            &loaded.segments,
            crate::windows::video_editor::model::total_duration(
                &loaded.segments,
                loaded.source_duration.unwrap(),
            ),
        );
        assert_eq!(
            camera_segments,
            [crate::windows::video_editor::model::CameraSegment {
                id: "camera-0-0".into(),
                start_time: 0.25,
                end_time: 1.25,
            }]
        );
    }

    #[test]
    fn bar_width_follows_mode_and_target_label() {
        assert_eq!(chrome::recording_control_width(false, false), 236.0);
        assert_eq!(chrome::recording_control_width(true, false), 400.0);
        assert_eq!(chrome::recording_control_width(false, true), 376.0);
        assert_eq!(chrome::recording_control_width(true, true), 540.0);
        assert_eq!(chrome::RECORDING_TARGET_LABEL_WIDTH, 140.0);
        assert_eq!(chrome::RECORDING_WINDOW_HEIGHT, 52.0);
        assert_eq!(chrome::RECORDING_BAR_PAD_TOP, 4.0);
        assert_eq!(
            chrome::recording_inner_bar_height(),
            chrome::overlay_bar_height()
        );
        assert_eq!(
            chrome::recording_inner_bar_height(),
            chrome::OVERLAY_BORDER_WIDTH * 2.0
                + chrome::OVERLAY_SURFACE_PADDING * 2.0
                + chrome::OVERLAY_BUTTON_SIZE
        );
        assert_ne!(
            chrome::recording_inner_bar_height(),
            chrome::RECORDING_WINDOW_HEIGHT
        );
        let (x, y) = chrome::recording_bar_origin(0.0, 10.0, 1920.0, 236.0);
        assert_eq!(
            y + chrome::RECORDING_BAR_PAD_TOP,
            10.0 + chrome::RECORDING_TOP_MARGIN
        );
        assert_eq!(x, ((1920.0_f32 - 236.0) / 2.0).round());
    }

    #[test]
    fn escape_only_cancels_before_recording_starts() {
        assert!(can_cancel_with_escape(Mode::PreRecording));
        assert!(!can_cancel_with_escape(Mode::Recording));
    }

    #[test]
    fn recording_target_selection_uses_each_displays_capture_scale() {
        let first = capture_display_geometry(
            Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(1920.0), px(1080.0)),
            },
            1.0,
        );
        let second = capture_display_geometry(
            Bounds {
                origin: point(px(1920.0), px(0.0)),
                size: size(px(1280.0), px(720.0)),
            },
            1.5,
        );
        let displays = [first, second];
        let scale = crate::capture::overlay::capture_coordinate_scale(1.5);

        assert_eq!(first, (0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(second, (1920.0 * scale, 0.0, 1280.0 * scale, 720.0 * scale));
        assert_eq!(
            chrome::display_containing(&displays, 1_920.0 * scale + 1_280.0 * scale / 2.0, 500.0),
            Some(second)
        );
    }

    #[test]
    fn macos_recording_controls_use_the_largest_display_intersection() {
        let displays = [(0.0, 0.0, 1000.0, 1000.0), (1000.0, 300.0, 1000.0, 300.0)];
        let spanning = ScreenRect {
            x: 700,
            y: 100,
            width: 800,
            height: 800,
        };
        assert_eq!(
            chrome::display_containing(&displays, 1100.0, 500.0),
            Some(displays[1])
        );
        assert_eq!(display_matching_rect(&displays, spanning), Some(0));

        let separated = [(0.0, 0.0, 800.0, 800.0), (1000.0, 0.0, 800.0, 800.0)];
        assert_eq!(
            display_matching_rect(
                &separated,
                ScreenRect {
                    x: 700,
                    y: 0,
                    width: 500,
                    height: 200
                }
            ),
            Some(1)
        );
        assert_eq!(
            display_matching_rect(
                &separated,
                ScreenRect {
                    x: 700,
                    y: 0,
                    width: 400,
                    height: 200
                }
            ),
            Some(0)
        );

        let negative_origins = [(-1600.0, -200.0, 1600.0, 900.0), (0.0, 0.0, 1920.0, 1080.0)];
        assert_eq!(
            display_matching_rect(
                &negative_origins,
                ScreenRect {
                    x: -200,
                    y: 100,
                    width: 800,
                    height: 400
                }
            ),
            Some(1)
        );
        assert_eq!(
            display_matching_rect(
                &displays,
                ScreenRect {
                    x: 1900,
                    y: 700,
                    width: 200,
                    height: 200
                }
            ),
            Some(0)
        );
        assert_eq!(display_matching_rect(&[], spanning), None);
    }

    #[test]
    fn recording_window_fits_the_bar_and_expands_for_menus() {
        assert_eq!(
            control_window_metrics(236.0, false, false, false),
            if cfg!(target_os = "macos") {
                (236.0, 0.0, 44.0)
            } else {
                (332.0, 48.0, 52.0)
            }
        );
        assert_eq!(
            control_window_metrics(236.0, false, true, false),
            (332.0, 48.0, 148.0)
        );
        assert_eq!(
            control_window_metrics(540.0, false, false, false),
            if cfg!(target_os = "macos") {
                (540.0, 0.0, 44.0)
            } else {
                (572.0, 16.0, 52.0)
            }
        );
        assert_eq!(
            control_window_metrics(236.0, true, false, false),
            (332.0, 48.0, 300.0)
        );
        assert_eq!(
            control_window_metrics(400.0, true, false, false),
            (432.0, 16.0, 300.0)
        );
        assert_eq!(
            (chrome::OVERLAY_TARGET_TRIGGER_WIDTH - DEVICE_DROPDOWN_WIDTH) / 2.0,
            -104.0
        );
        assert_eq!(
            control_window_metrics(236.0, false, false, true),
            (332.0, 48.0, 52.0)
        );
    }

    #[herogpui::test]
    fn microphone_menu_matches_electron_rows(cx: &mut gpui::TestAppContext) {
        let control = cx.update(|cx| {
            cx.new(|cx| RecordingControl {
                mode: Mode::PreRecording,
                preference_scope: PreferenceScope::Global,
                target: RecordingTarget::Area,
                target_name: None,
                rect: ScreenRect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                },
                display_id: None,
                window_id: None,
                project: None,
                system_audio: true,
                #[cfg(target_os = "macos")]
                system_audio_pending: false,
                #[cfg(target_os = "macos")]
                microphone_pending: false,
                #[cfg(target_os = "macos")]
                camera_pending: false,
                #[cfg(target_os = "macos")]
                pause_pending: false,
                #[cfg(target_os = "macos")]
                camera_protection_pending: false,
                microphone: true,
                camera: false,
                camera_locked: false,
                camera_preview_visible: false,
                camera_preview_position: Arc::default(),
                camera_preview_subscription: None,
                selected_mic_id: Some("mic-1".into()),
                selected_camera_id: None,
                selected_ios_id: None,
                selected_ios_name: None,
                ios_devices: Vec::new(),
                devices: crate::system::devices::MediaDeviceLists {
                    microphones: vec![crate::system::devices::MediaDevice {
                        id: "mic-1".into(),
                        label: "Built-in microphone".into(),
                    }],
                    cameras: Vec::new(),
                    default_microphone_id: Some("mic-1".into()),
                    default_camera_id: None,
                },
                device_request: 0,
                pending_device_menu: None,
                menu: MenuHandle::new(),
                bounds_subscription: None,
                #[cfg(target_os = "macos")]
                activation_subscription: None,
                started_at: None,
                paused_at: None,
                paused_total: Duration::ZERO,
                elapsed: 0,
                countdown_generation: 0,
                countdown_active: false,
                countdown_remaining: None,
                measured_width: None,
                focus_handle: cx.focus_handle(),
                recorder_error_subscription: None,
                recorder_terminal_error: None,
                #[cfg(target_os = "macos")]
                stop_previous_state: None,
                #[cfg(target_os = "macos")]
                stop_target_closed: false,
                #[cfg(target_os = "macos")]
                start_target_closed: false,
                #[cfg(target_os = "macos")]
                start_stop_requested: None,
                recording_error_generation: 0,
                #[cfg(target_os = "macos")]
                pointer_inside: false,
                tooltip_presenter: crate::ui::tooltip::NativeTooltipPresenter::new(
                    "recording-control",
                ),
            })
        });
        let weak = control.downgrade();
        let entries = control.read_with(cx, |control, _| {
            control.device_menu_entries(crate::system::devices::DeviceKind::Microphone, weak)
        });
        let [MenuEntry::Item(toggle), MenuEntry::Separator, MenuEntry::Item(default), MenuEntry::Item(device)] =
            entries.as_slice()
        else {
            panic!("unexpected microphone menu layout");
        };

        assert_eq!(toggle.label.as_ref(), "Microphone");
        assert!(toggle.trailing_check);
        assert_eq!(
            default.label.as_ref(),
            "System Default (Built-in microphone)"
        );
        assert!(!default.trailing_check);
        assert_eq!(device.label.as_ref(), "Built-in microphone");
        assert!(device.trailing_check);
    }

    #[test]
    fn device_refresh_updates_only_the_requested_kind() {
        let mut devices = crate::system::devices::MediaDeviceLists {
            microphones: Vec::new(),
            cameras: vec![crate::system::devices::MediaDevice {
                id: "camera-1".into(),
                label: "Camera".into(),
            }],
            default_microphone_id: None,
            default_camera_id: Some("camera-1".into()),
        };
        let listed = crate::system::devices::MediaDeviceLists {
            microphones: vec![crate::system::devices::MediaDevice {
                id: "mic-1".into(),
                label: "Microphone".into(),
            }],
            cameras: Vec::new(),
            default_microphone_id: Some("mic-1".into()),
            default_camera_id: None,
        };

        assert!(merge_device_refresh(
            &mut devices,
            3,
            3,
            crate::system::devices::DeviceKind::Microphone,
            Ok(listed),
        ));
        assert_eq!(devices.microphones[0].id, "mic-1");
        assert_eq!(devices.cameras[0].id, "camera-1");
        assert_eq!(devices.default_microphone_id.as_deref(), Some("mic-1"));
        assert_eq!(devices.default_camera_id.as_deref(), Some("camera-1"));
    }

    #[test]
    fn unchanged_device_refresh_does_not_reopen_the_menu() {
        use crate::system::devices::{DeviceKind, MediaDevice, MediaDeviceLists};

        for kind in [DeviceKind::Microphone, DeviceKind::Camera] {
            let mut devices = MediaDeviceLists {
                microphones: vec![MediaDevice {
                    id: "mic-1".into(),
                    label: "Built-in microphone".into(),
                }],
                cameras: vec![MediaDevice {
                    id: "camera-1".into(),
                    label: "Built-in camera".into(),
                }],
                default_microphone_id: Some("mic-1".into()),
                default_camera_id: Some("camera-1".into()),
            };
            let mut reply = devices.clone();
            match kind {
                DeviceKind::Microphone => {
                    reply.cameras.clear();
                    reply.default_camera_id = None;
                }
                DeviceKind::Camera => {
                    reply.microphones.clear();
                    reply.default_microphone_id = None;
                }
            }
            let before = devices.clone();
            assert!(
                !merge_device_refresh(&mut devices, 3, 3, kind, Ok(reply.clone())),
                "an identical list must not replace the open menu"
            );
            assert_eq!(devices, before);
            match kind {
                DeviceKind::Microphone => reply.default_microphone_id = None,
                DeviceKind::Camera => reply.default_camera_id = None,
            }
            assert!(merge_device_refresh(&mut devices, 3, 3, kind, Ok(reply)));
        }
    }

    #[test]
    fn failed_device_refresh_preserves_the_last_successful_list() {
        use crate::system::devices::{DeviceKind, MediaDevice, MediaDeviceLists};

        let known = MediaDeviceLists {
            microphones: vec![MediaDevice {
                id: "mic-1".into(),
                label: "Built-in microphone".into(),
            }],
            cameras: vec![MediaDevice {
                id: "camera-1".into(),
                label: "Built-in camera".into(),
            }],
            default_microphone_id: Some("mic-1".into()),
            default_camera_id: Some("camera-1".into()),
        };
        for kind in [DeviceKind::Microphone, DeviceKind::Camera] {
            let mut devices = known.clone();
            let failure = Err(anyhow::anyhow!("enumeration failed"));
            assert!(!merge_device_refresh(&mut devices, 3, 3, kind, failure));
            assert_eq!(devices, known);
            assert!(merge_device_refresh(
                &mut devices,
                4,
                4,
                kind,
                Ok(MediaDeviceLists::default())
            ));
            match kind {
                DeviceKind::Microphone => {
                    assert!(devices.microphones.is_empty());
                    assert!(devices.default_microphone_id.is_none());
                    assert_eq!(devices.cameras, known.cameras);
                }
                DeviceKind::Camera => {
                    assert!(devices.cameras.is_empty());
                    assert!(devices.default_camera_id.is_none());
                    assert_eq!(devices.microphones, known.microphones);
                }
            }
        }
    }

    #[test]
    fn stale_device_refresh_is_ignored() {
        let mut devices = crate::system::devices::MediaDeviceLists::default();
        let listed = crate::system::devices::MediaDeviceLists {
            microphones: vec![crate::system::devices::MediaDevice {
                id: "stale".into(),
                label: "Stale".into(),
            }],
            ..Default::default()
        };

        assert!(!merge_device_refresh(
            &mut devices,
            4,
            3,
            crate::system::devices::DeviceKind::Microphone,
            Ok(listed),
        ));
        assert!(devices.microphones.is_empty());
    }

    #[test]
    fn device_menu_waits_for_the_expanded_gpui_viewport() {
        let mut pending = Some((
            DeviceMenuKind::Media(crate::system::devices::DeviceKind::Camera),
            SharedString::from("recording-camera"),
        ));

        assert!(take_ready_device_menu(&mut pending, px(52.0)).is_none());
        assert!(pending.is_some());
        assert!(take_ready_device_menu(&mut pending, px(300.0)).is_some());
        assert!(pending.is_none());
    }

    #[herogpui::test]
    fn bounds_notification_opens_the_pending_device_menu(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = std::sync::Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let (control, cx) = cx.add_window_view(|window, cx| -> RecordingControl {
            let mut control = RecordingControl {
                mode: Mode::PreRecording,
                preference_scope: PreferenceScope::Global,
                target: RecordingTarget::Area,
                target_name: None,
                rect: ScreenRect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                display_id: None,
                window_id: None,
                project: None,
                system_audio: true,
                #[cfg(target_os = "macos")]
                system_audio_pending: false,
                #[cfg(target_os = "macos")]
                microphone_pending: false,
                #[cfg(target_os = "macos")]
                camera_pending: false,
                #[cfg(target_os = "macos")]
                pause_pending: false,
                #[cfg(target_os = "macos")]
                camera_protection_pending: false,
                microphone: true,
                camera: false,
                camera_locked: false,
                camera_preview_visible: false,
                camera_preview_position: Arc::default(),
                camera_preview_subscription: None,
                selected_mic_id: None,
                selected_camera_id: None,
                selected_ios_id: None,
                selected_ios_name: None,
                ios_devices: Vec::new(),
                devices: crate::system::devices::MediaDeviceLists::default(),
                device_request: 0,
                pending_device_menu: None,
                menu: MenuHandle::new(),
                bounds_subscription: None,
                #[cfg(target_os = "macos")]
                activation_subscription: None,
                started_at: None,
                paused_at: None,
                paused_total: Duration::ZERO,
                elapsed: 0,
                countdown_generation: 0,
                countdown_active: false,
                countdown_remaining: None,
                measured_width: None,
                focus_handle: cx.focus_handle(),
                recorder_error_subscription: None,
                recorder_terminal_error: None,
                #[cfg(target_os = "macos")]
                stop_previous_state: None,
                #[cfg(target_os = "macos")]
                stop_target_closed: false,
                #[cfg(target_os = "macos")]
                start_target_closed: false,
                #[cfg(target_os = "macos")]
                start_stop_requested: None,
                recording_error_generation: 0,
                #[cfg(target_os = "macos")]
                pointer_inside: false,
                tooltip_presenter: crate::ui::tooltip::NativeTooltipPresenter::new(
                    "recording-control",
                ),
            };
            control.bounds_subscription = Some(cx.observe_window_bounds(
                window,
                |this: &mut RecordingControl, window, cx| {
                    this.open_pending_device_menu(window, cx);
                },
            ));
            control
        });
        let toggle_camera = |cx: &mut gpui::VisualTestContext| {
            cx.update(|window, cx| {
                control.update(cx, |control, cx| {
                    control.toggle_device_menu(
                        DeviceMenuKind::Media(crate::system::devices::DeviceKind::Camera),
                        "recording-camera".into(),
                        window,
                        cx,
                    );
                });
            });
        };
        cx.simulate_resize(size(px(332.0), px(52.0)));

        toggle_camera(cx);
        assert!(control.read_with(cx, |control, cx| {
            control.pending_device_menu.is_some() && !control.menu.is_open(cx)
        }));

        toggle_camera(cx);
        cx.simulate_resize(size(px(332.0), px(300.0)));
        assert!(
            control.read_with(cx, |control, cx| {
                control.pending_device_menu.is_none() && !control.menu.is_open(cx)
            }),
            "a second press must cancel the pending menu before the resize arrives"
        );
        assert_eq!(
            control.read_with(cx, |control, _| control.device_request),
            0
        );

        cx.simulate_resize(size(px(332.0), px(52.0)));

        toggle_camera(cx);
        assert!(control.read_with(cx, |control, cx| {
            control.pending_device_menu.is_some() && !control.menu.is_open(cx)
        }));

        cx.update(|window, cx| {
            control.update(cx, |control, cx| {
                assert!(control.dismiss_device_menu(window, cx));
            });
        });
        cx.simulate_resize(size(px(332.0), px(300.0)));
        assert!(control.read_with(cx, |control, cx| {
            control.pending_device_menu.is_none() && !control.menu.is_open(cx)
        }));

        cx.simulate_resize(size(px(332.0), px(52.0)));
        toggle_camera(cx);
        cx.simulate_resize(size(px(332.0), px(300.0)));
        assert!(control.read_with(cx, |control, cx| {
            control.pending_device_menu.is_none()
                && control.menu.is_open_for("recording-camera", cx)
        }));
    }
}

#[cfg(test)]
mod toolbar_tests {
    #[cfg(windows)]
    #[test]
    fn windows_capture_affinity_excludes_an_isolated_native_window() {
        use super::exclude_toolbar_from_capture;
        use windows::core::w;
        use windows::Win32::Foundation::COLORREF;
        use windows::Win32::Graphics::Dwm::DwmIsCompositionEnabled;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, GetWindowDisplayAffinity, SetLayeredWindowAttributes,
            LWA_ALPHA, WDA_EXCLUDEFROMCAPTURE, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_POPUP,
        };

        let composition_enabled =
            unsafe { DwmIsCompositionEnabled() }.expect("query desktop window composition");
        assert!(composition_enabled.as_bool());

        let window = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_LAYERED,
                w!("STATIC"),
                w!("recording-capture-affinity-test"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                None,
                None,
            )
        }
        .expect("create isolated native test window");
        let layered =
            unsafe { SetLayeredWindowAttributes(window, COLORREF(0), u8::MAX, LWA_ALPHA) };
        let applied = exclude_toolbar_from_capture(window);
        let mut affinity = 0;
        let observed = unsafe { GetWindowDisplayAffinity(window, &mut affinity) };
        let _ = unsafe { DestroyWindow(window) };
        let expected_affinity = if crate::capture::overlay::windows_build() >= 19041 {
            WDA_EXCLUDEFROMCAPTURE
        } else {
            windows::Win32::UI::WindowsAndMessaging::WDA_MONITOR
        };

        layered.expect("make isolated native test window layered");
        assert!(applied, "set WDA_EXCLUDEFROMCAPTURE on native test window");
        observed.expect("read native test window capture affinity");
        assert_eq!(affinity, expected_affinity.0);
    }

    #[cfg(windows)]
    #[test]
    fn windows_capture_affinity_accepts_gpui_transparent_window_style() {
        use super::exclude_toolbar_from_capture;
        use windows::core::w;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_POPUP,
        };

        let window = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP,
                w!("STATIC"),
                w!("recording-capture-affinity-style-test"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                None,
                None,
            )
        }
        .expect("create GPUI-style native test window");
        let applied = exclude_toolbar_from_capture(window);
        let _ = unsafe { DestroyWindow(window) };

        assert!(applied, "set capture affinity on GPUI-style native window");
    }

    /// The recording bar's buttons carry their state in the icon, not in a
    /// background. `ToolbarButton` is `variant="ghost"` with only a hover
    /// background, and the system-audio button swaps `Volume2` for `VolumeX`.
    #[test]
    fn the_bar_signals_state_by_icon_and_not_by_a_filled_background() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repository root")
            .to_path_buf();

        let button = std::fs::read_to_string(
            root.join("src/renderer/components/area-overlay/toolbar-button.tsx"),
        )
        .expect("read toolbar-button.tsx");
        assert!(
            button.contains(r#"variant="ghost""#),
            "the toolbar button is a ghost"
        );
        assert!(
            !button.contains("data-selected") && !button.contains("aria-pressed:"),
            "the toolbar button has no pressed styling, so neither should this shell"
        );

        let window =
            std::fs::read_to_string(root.join("src/renderer/windows/recording-control-window.tsx"))
                .expect("read recording-control-window.tsx");
        assert!(
            window.contains("Volume2") && window.contains("VolumeX"),
            "the icon swap is what shows whether system audio is on"
        );

        let here = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/windows/recording_control.rs"),
        )
        .expect("read recording_control.rs");
        // A call, not a mention: this test's own assertion and the comment
        // above the button both name the method they forbid, so match only
        // lines that actually begin with the call.
        let call = here
            .split("#[cfg(test)]")
            .next()
            .unwrap_or_default()
            .lines()
            .find(|line| line.trim_start().starts_with(".selected("));
        assert!(
            call.is_none(),
            "no button in the recording bar may paint a selected background, found `{}`",
            call.unwrap_or_default().trim()
        );
    }
}
