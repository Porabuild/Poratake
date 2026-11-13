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

fn set_pre_recording_escape(enabled: bool, cx: &App) {
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
    microphone: bool,
    camera: bool,
    camera_locked: bool,
    camera_preview_visible: bool,
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
    started_at: Option<Instant>,
    paused_at: Option<Instant>,
    paused_total: Duration,
    elapsed: u64,
    countdown_generation: u64,
    countdown_active: bool,
    countdown_remaining: Option<u32>,
    measured_width: Option<f32>,
    focus_handle: FocusHandle,
    recorder_error_subscription: Option<crate::daemon::EventSubscription>,
    recording_error_generation: u64,
    #[cfg(any(windows, target_os = "macos"))]
    hovered_tooltip: Option<(&'static str, SharedString)>,
    #[cfg(windows)]
    tooltip_generation: u64,
    #[cfg(windows)]
    native_tooltip: Option<crate::system::native_tooltip::WindowsTooltip>,
}

impl RecordingControl {
    #[cfg(feature = "parity")]
    pub(crate) fn parity_rect(&self) -> ScreenRect {
        self.rect
    }

    #[cfg(all(feature = "parity", any(windows, target_os = "macos")))]
    pub(crate) fn parity_hovered_tooltip(&self) -> Option<SharedString> {
        self.hovered_tooltip.as_ref().map(|(_, text)| text.clone())
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
        let bounds = bar_bounds(cx, rect, width, false, false);
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
                        microphone: toggles.mic_enabled,
                        camera: toggles.camera_enabled,
                        camera_locked: false,
                        camera_preview_visible: false,
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
                        started_at: None,
                        paused_at: None,
                        paused_total: Duration::ZERO,
                        elapsed: 0,
                        measured_width: None,
                        countdown_generation: 0,
                        countdown_active: false,
                        countdown_remaining: None,
                        focus_handle: cx.focus_handle(),
                        recorder_error_subscription: None,
                        recording_error_generation: 0,
                        #[cfg(any(windows, target_os = "macos"))]
                        hovered_tooltip: None,
                        #[cfg(windows)]
                        tooltip_generation: 0,
                        #[cfg(windows)]
                        native_tooltip: None,
                    });
                    view.update(cx, |this, cx| {
                        this.bounds_subscription =
                            Some(cx.observe_window_bounds(window, |this, window, cx| {
                                this.open_pending_device_menu(window, cx);
                            }));
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
        for window_handle in cx.windows() {
            let Some(handle) = window_handle.downcast::<Self>() else {
                continue;
            };
            let cancelled = handle
                .update(cx, |this, window, cx| {
                    if this.dismiss_device_menu(window, cx) {
                        return false;
                    }
                    if !can_cancel_with_escape(this.mode) {
                        return false;
                    }
                    this.countdown_generation += 1;
                    this.countdown_active = false;
                    this.countdown_remaining = None;
                    this.hide_camera_preview(cx);
                    window.remove_window();
                    true
                })
                .unwrap_or(false);
            if cancelled {
                set_pre_recording_escape(false, cx);
                registry::forget(RegistryKind::RecordingControl, cx);
                crate::capture::overlay::end_recording_handoff(cx);
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

    #[cfg(any(windows, target_os = "macos"))]
    fn clear_native_tooltip(&mut self, _window: &Window) {
        #[cfg(all(test, windows))]
        if self.hovered_tooltip.is_some() {
            crate::system::native_tooltip::record_test_hide("recording-control");
        }
        self.hovered_tooltip = None;
        #[cfg(windows)]
        {
            self.tooltip_generation = self.tooltip_generation.wrapping_add(1);
            if let Some(tooltip) = &mut self.native_tooltip {
                tooltip.hide();
            }
        }
        #[cfg(target_os = "macos")]
        super::scroll_capture::set_native_tooltip(_window, None);
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn hover_native_tooltip(
        &mut self,
        id: &'static str,
        tooltip: SharedString,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !hovered {
            if self
                .hovered_tooltip
                .as_ref()
                .is_some_and(|(current, _)| *current == id)
            {
                self.clear_native_tooltip(window);
            }
            return;
        }
        self.clear_native_tooltip(window);
        self.hovered_tooltip = Some((id, tooltip.clone()));
        #[cfg(target_os = "macos")]
        {
            let _ = cx;
            super::scroll_capture::set_native_tooltip(window, Some(tooltip.as_ref()));
        }
        #[cfg(windows)]
        {
            let generation = self.tooltip_generation;
            cx.spawn_in(window, async move |entity, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                let _ = entity.update_in(cx, |this, window, _| {
                    if this.tooltip_generation != generation {
                        return;
                    }
                    #[cfg(all(test, windows))]
                    crate::system::native_tooltip::record_test_show(
                        "recording-control",
                        tooltip.as_ref(),
                    );
                    if this.native_tooltip.is_none() {
                        this.native_tooltip =
                            crate::system::native_tooltip::WindowsTooltip::new(window);
                    }
                    if let Some(native) = &mut this.native_tooltip {
                        native.show(window, tooltip.as_ref());
                    }
                });
            })
            .detach();
        }
    }

    fn tooltip(
        id: &'static str,
        tooltip: impl Into<SharedString>,
        child: impl IntoElement,
        max_body_width: Option<f32>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tooltip = tooltip.into();
        let component = match max_body_width {
            Some(width) => {
                let body = tooltip.clone();
                herogpui::components::Tooltip::new(tooltip.clone()).body(move |_, _| {
                    div()
                        .max_w(px(width))
                        .line_clamp(2)
                        .child(body.clone())
                        .into_any_element()
                })
            }
            None => herogpui::components::Tooltip::new(tooltip.clone()),
        }
        .id((gpui::ElementId::from("recording-focus-tooltip"), id));
        let placement = match id {
            "recording-target-tip"
            | "recording-start"
            | "recording-pause"
            | "recording-stop"
            | "camera" => herogpui::components::TooltipPlacement::Right,
            _ => herogpui::components::TooltipPlacement::Left,
        };
        #[cfg(any(windows, target_os = "macos"))]
        {
            let hover_tooltip = tooltip.clone();
            let trigger = div()
                .id((gpui::ElementId::from("recording-native-tooltip"), id))
                .flex()
                .flex_shrink_0()
                .on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                    this.hover_native_tooltip(id, hover_tooltip.clone(), *hovered, window, cx);
                }))
                .capture_any_mouse_down(cx.listener(|this, _, window, _| {
                    this.clear_native_tooltip(window);
                }))
                .child(child);
            component
                .trigger(herogpui::components::TooltipTrigger::Focus)
                .placement(placement)
                .child(trigger)
                .into_any_element()
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = cx;
            component
                .placement(placement)
                .child(child)
                .into_any_element()
        }
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
        #[cfg(not(windows))]
        let bounds = bar_bounds(
            cx,
            rect,
            recording_control_width(false, target_name.is_some()),
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
                        window.remove_window();
                        return false;
                    }
                    #[cfg(not(windows))]
                    if window.bounds().origin != bounds.origin {
                        window.remove_window();
                        return false;
                    }
                    this.target = target;
                    this.rect = rect;
                    this.display_id = display_id;
                    this.window_id = window_id;
                    this.target_name = target_name.clone().map(SharedString::from);
                    this.reposition_camera_preview(cx);
                    this.sync_window_bounds(window, cx, false);
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

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.countdown_active {
            return;
        }
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
        self.countdown_active = true;
        self.countdown_remaining = Some(delay);
        window.focus(&self.focus_handle, cx);
        self.sync_window_bounds(window, cx, false);
        cx.notify();
        cx.spawn_in(window, async move |entity, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let Ok(done) = entity.update_in(cx, |this, window, cx| {
                if !this.countdown_active || this.countdown_generation != generation {
                    return true;
                }
                let remaining = this.countdown_remaining.unwrap_or(1);
                if remaining > 1 {
                    this.countdown_remaining = Some(remaining - 1);
                    cx.notify();
                    return false;
                }
                this.countdown_active = false;
                this.countdown_remaining = None;
                this.begin_recording(window, cx);
                true
            }) else {
                break;
            };
            if done {
                break;
            }
        })
        .detach();
    }

    fn begin_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let service = crate::state::state(cx);
        let recording = service.config.get().recording;
        let ios_recording = self.selected_ios_id.is_some();
        #[cfg(all(target_os = "macos", not(test)))]
        let excluded_window_ids = match recording_control_exclusion_ids(
            recording_control_window_id(window),
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
        let project = match recorder::create_project(&service.config) {
            Ok(path) => path,
            Err(error) => {
                eprintln!("[recorder] failed to create project: {error}");
                return;
            }
        };
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

        if !crate::capture::overlay::release_frozen_screen_in_place(cx) {
            let _ = std::fs::remove_dir_all(&project);
            crate::windows::toast::Toast::show(
                cx,
                "Recording failed",
                "Could not release the frozen screen",
            );
            return;
        }
        if ios_recording {
            crate::capture::overlay::close_all(cx);
        }
        #[cfg(target_os = "macos")]
        if !ios_recording && !crate::capture::overlay::begin_recording_handoff(self.rect, cx) {
            let _ = std::fs::remove_dir_all(&project);
            crate::windows::toast::Toast::show(
                cx,
                "Recording failed",
                "Could not show the recording overlay",
            );
            return;
        }
        self.recording_error_generation = self.recording_error_generation.wrapping_add(1);
        let error_generation = self.recording_error_generation;
        self.watch_recorder_errors(window, cx, error_generation);
        if let Err(error) = recorder::start(&service.daemon, &config) {
            eprintln!("[recorder] {error}");
            self.recording_error_generation = self.recording_error_generation.wrapping_add(1);
            self.recorder_error_subscription = None;
            let _ = std::fs::remove_dir_all(&project);
            #[cfg(target_os = "macos")]
            crate::capture::overlay::end_recording_handoff(cx);
            crate::windows::toast::Toast::show(cx, "Recording failed", error.to_string());
            return;
        }
        if self.camera && !self.camera_preview_visible {
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
        let _ = crate::capture::overlay::begin_recording_handoff(self.rect, cx);

        self.project = Some(project);
        self.mode = Mode::Recording;
        self.camera_locked = self.camera;
        self.started_at = Some(cx.background_executor().now());
        self.paused_at = None;
        self.paused_total = Duration::ZERO;
        self.elapsed = 0;
        self.sync_window_bounds(
            window,
            cx,
            self.pending_device_menu.is_some() || self.menu.is_present(cx),
        );
        crate::intents::refresh_shell(cx);
        self.tick(cx);
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
    fn set_system_audio(&mut self, enabled: bool, cx: &mut Context<Self>) -> bool {
        if self.mode == Mode::Recording {
            let result = recorder::set_system_audio(&crate::state::state(cx).daemon, enabled);
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

    fn set_microphone(&mut self, enabled: bool, cx: &mut Context<Self>) -> bool {
        if enabled
            && !crate::system::permissions::ensure_access(
                crate::system::permissions::Device::Microphone,
            )
        {
            return false;
        }
        if self.mode == Mode::Recording {
            let service = crate::state::state(cx);
            let result =
                recorder::set_microphone(&service.daemon, enabled, self.selected_mic_id.as_deref());
            if let Err(error) = result {
                report_toggle_failure(cx, &error);
                return false;
            }
        }
        self.microphone = enabled;
        self.persist_pre_recording_preference(RecordingPreference::Microphone(enabled), cx);
        cx.notify();
        true
    }

    fn set_camera(&mut self, enabled: bool, cx: &mut Context<Self>) -> bool {
        if enabled
            && !crate::system::permissions::ensure_access(
                crate::system::permissions::Device::Camera,
            )
        {
            return false;
        }
        if enabled {
            if let Err(error) = self.show_camera_preview(cx) {
                report_toggle_failure(cx, &error);
                return false;
            }
        } else {
            self.hide_camera_preview(cx);
        }
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
        self.camera = enabled;
        self.persist_pre_recording_preference(RecordingPreference::Camera(enabled), cx);
        cx.notify();
        true
    }

    fn show_camera_preview(&mut self, cx: &mut App) -> anyhow::Result<()> {
        let service = crate::state::state(cx);
        let camera = service.config.get().recording.camera;
        let (x, y) = camera
            .position
            .map(|position| (position.x as i32, position.y as i32))
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
        service.daemon.camera_preview().show(&request)?;
        self.camera_preview_visible = true;
        Ok(())
    }

    fn hide_camera_preview(&mut self, cx: &mut App) {
        if !self.camera_preview_visible {
            return;
        }
        self.camera_preview_visible = false;
        if let Err(error) = crate::state::state(cx).daemon.camera_preview().hide() {
            eprintln!("[recorder] camera-preview hide failed: {error}");
        }
    }

    fn reposition_camera_preview(&self, cx: &mut App) {
        if !self.camera_preview_visible {
            return;
        }
        let (x, y) = camera_preview_position(self.rect);
        let request = CameraPreviewUpdateRequest {
            x: Some(x),
            y: Some(y),
            ..Default::default()
        };
        if let Err(error) = crate::state::state(cx)
            .daemon
            .camera_preview()
            .update(&request)
        {
            eprintln!("[recorder] camera-preview update failed: {error}");
        }
    }

    fn input_toggles(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut toggles = Vec::new();
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
                self.countdown_active,
                |this, _window, cx| {
                    if this.countdown_active {
                        return;
                    }
                    let next = !this.system_audio;
                    this.set_system_audio(next, cx);
                },
                cx,
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
        let trigger = herogpui::components::Button::new(owner)
            .aria_label(tooltip)
            .variant(herogpui::Variant::Ghost)
            .is_disabled(self.countdown_active)
            .recipe("overlay-trigger")
            .when(self.countdown_active, |button| {
                button.recipe("overlay-disabled")
            })
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
            .child(Self::tooltip(id, tooltip, trigger, None, cx))
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
        let opening = !self.menu.is_open_for(owner.as_ref(), cx);
        if !opening {
            self.pending_device_menu = None;
            self.menu.close(window, cx);
            self.sync_window_bounds(window, cx, false);
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
                                    this.set_microphone(!this.microphone, cx);
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
                .spawn(async move { crate::system::devices::list(&daemon, &[kind]) })
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
            let listed = cx
                .background_executor()
                .spawn(async move { crate::system::devices::list_ios(&daemon) })
                .await;
            let _ = entity.update_in(cx, |this, window, cx| {
                if this.device_request != request {
                    return;
                }
                this.ios_devices = listed;
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

    fn select_device(
        &mut self,
        kind: crate::system::devices::DeviceKind,
        selection: Option<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        let id = selection.as_ref().map(|(id, _)| id.clone());
        let name = selection.as_ref().map(|(_, name)| name.clone());
        match kind {
            crate::system::devices::DeviceKind::Microphone => {
                let previous = self.selected_mic_id.clone();
                self.selected_mic_id = id.clone();
                if !self.set_microphone(true, cx) {
                    self.selected_mic_id = previous;
                    cx.notify();
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

    fn toggle_pause(&mut self, cx: &mut Context<Self>) {
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
            recorder::RecorderState::Idle => {}
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
        let (error_tx, error_rx) = smol::channel::bounded::<(String, String)>(1);
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
                let _ = error_tx.try_send((code, message));
            })));

        let handle = window.window_handle();
        cx.spawn(async move |_, cx| loop {
            let Ok((code, message)) = error_rx.recv().await else {
                break;
            };
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
                        if code == RECORDING_TARGET_CLOSED {
                            !this.finish(false, window, cx)
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

    /// Port of `handleTerminalRecordingFailure`: the daemon already errored
    /// out, so the partial project is deleted, the UI torn down and the
    /// failure surfaced, with no stop call issued.
    fn fail_recording(&mut self, message: String, window: &mut Window, cx: &mut Context<Self>) {
        self.recording_error_generation = self.recording_error_generation.wrapping_add(1);
        self.recorder_error_subscription = None;
        self.hide_camera_preview(cx);
        recorder::force_idle();
        if let Some(project) = self.project.take() {
            let _ = std::fs::remove_dir_all(&project);
        }
        self.started_at = None;
        self.paused_at = None;
        self.paused_total = Duration::ZERO;

        window.remove_window();
        registry::close(RegistryKind::RecordingControl, cx);
        crate::capture::overlay::end_recording_handoff(cx);
        crate::intents::refresh_shell(cx);
        crate::windows::toast::Toast::show(cx, "Recording failed", message);
    }

    pub(crate) fn finish(
        &mut self,
        discard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let duration = self.recording_duration(cx).as_secs_f64();
        let service = crate::state::state(cx);
        let stopped = recorder::stop(&service.daemon);
        if !stopped {
            crate::windows::toast::Toast::show(
                cx,
                "Recording not finalized",
                "Stop failed. The recording controls remain available so you can retry.",
            );
            return false;
        }
        self.hide_camera_preview(cx);
        self.recording_error_generation = self.recording_error_generation.wrapping_add(1);
        self.recorder_error_subscription = None;

        let project = self.project.take();
        self.started_at = None;
        self.paused_at = None;
        self.paused_total = Duration::ZERO;

        window.remove_window();
        registry::close(RegistryKind::RecordingControl, cx);
        crate::capture::overlay::end_recording_handoff(cx);
        crate::intents::refresh_shell(cx);

        let Some(project) = project else {
            return true;
        };
        if discard {
            let _ = std::fs::remove_dir_all(&project);
            return true;
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
        if recording.auto_zoom {
            if let Some(cursor_data) = crate::video::sidecars::load_cursor(&project) {
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
        crate::windows::video_editor::model::save_state(&project, &state);
        crate::history_store::add_item(
            crate::history_store::HistoryItem {
                id: format!(
                    "{}",
                    chrono::Local::now().timestamp_nanos_opt().unwrap_or(0)
                ),
                timestamp: chrono::Local::now().timestamp_millis(),
                original_path: project.to_string_lossy().to_string(),
                r#type: crate::history_store::HistoryItemType::Video,
                editor_state: None,
                duration: Some(duration),
            },
            max_items,
            history_enabled,
        );

        if show_preview {
            let project = project.clone();
            cx.defer(move |cx| {
                crate::windows::capture_preview::CapturePreviewWindow::open_video(cx, project);
            });
        } else {
            let path = project.to_string_lossy().to_string();
            cx.defer(move |cx| {
                crate::windows::video_editor::VideoEditorWindow::open(cx, Some(path));
            });
        }
        true
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == Mode::Recording {
            self.finish(true, window, cx);
            return;
        }
        self.countdown_generation += 1;
        self.countdown_active = false;
        self.countdown_remaining = None;
        self.hide_camera_preview(cx);
        set_pre_recording_escape(false, cx);
        window.remove_window();
        registry::close(RegistryKind::RecordingControl, cx);
        crate::capture::overlay::end_recording_handoff(cx);
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
        let bounds = bar_bounds(
            cx,
            self.rect,
            width,
            device_menu_open,
            self.countdown_active,
        );
        if window.bounds() == bounds {
            return;
        }
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
    listed: crate::system::devices::MediaDeviceLists,
) -> bool {
    if current_request != request {
        return false;
    }
    match kind {
        crate::system::devices::DeviceKind::Microphone => {
            devices.microphones = listed.microphones;
            devices.default_microphone_id = listed.default_microphone_id;
        }
        crate::system::devices::DeviceKind::Camera => {
            devices.cameras = listed.cameras;
            devices.default_camera_id = listed.default_camera_id;
        }
    }
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

#[cfg(all(target_os = "macos", not(test)))]
fn recording_control_window_id(window: &Window) -> Option<i64> {
    use cocoa::base::nil;
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    unsafe {
        let view = appkit.ns_view.as_ptr() as cocoa::base::id;
        let native_window: cocoa::base::id = msg_send![view, window];
        if native_window == nil {
            return None;
        }
        let number: isize = msg_send![native_window, windowNumber];
        (number >= 0).then_some(number as i64)
    }
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
    #[cfg(any(not(windows), test))]
    let _ = window;
}

fn bar_bounds(
    cx: &mut App,
    rect: ScreenRect,
    width: f32,
    device_menu_open: bool,
    countdown_active: bool,
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
    let (bar_x, y) = chrome::recording_bar_origin(work_x, work_y, work_width, width);
    let (window_width, bar_offset, height) =
        control_window_metrics(width, device_menu_open, countdown_active);
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
    let center_x = rect.x as f32 + rect.width as f32 / 2.0;
    let center_y = rect.y as f32 + rect.height as f32 / 2.0;
    let displays = cx.displays();
    let geometries: Vec<_> = displays
        .iter()
        .map(|display| {
            let bounds = crate::system::work_area::display_bounds(display.as_ref());
            let scale = crate::capture::overlay::display_scale_factor(display.as_ref(), cx);
            capture_display_geometry(bounds, scale)
        })
        .collect();
    let selected = chrome::display_containing(&geometries, center_x, center_y)?;
    displays
        .into_iter()
        .zip(geometries)
        .find_map(|(display, geometry)| (geometry == selected).then_some(display))
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
) -> (f32, f32, f32) {
    let window_width = chrome::recording_window_width(width);
    let bar_offset = ((window_width - width) / 2.0).round();
    let height = if device_menu_open {
        DEVICE_MENU_WINDOW_HEIGHT
    } else if countdown_active {
        COUNTDOWN_WINDOW_HEIGHT
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
        cx,
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
        let toggles = self.input_toggles(cx);
        let countdown = self.countdown_remaining;

        let mut bar = toolbar::surface(&theme).h(px(chrome::recording_inner_bar_height()));

        if let Some(name) = &self.target_name {
            use herogpui::a11y::{A11y as _, Name, Role};

            let bar_width = self
                .measured_width
                .unwrap_or_else(|| recording_control_width(self.mode == Mode::Recording, true));
            let window_width = f32::from(window.bounds().size.width);
            let (_, bar_offset, _) = control_window_metrics(bar_width, false, false);
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
                .focus(|style| {
                    style.shadow(crate::ui::primitives::focus_ring(
                        &theme,
                        crate::ui::primitives::FOCUS_RING_OFFSET,
                    ))
                });
            bar = bar
                .child(Self::tooltip(
                    "recording-target-tip",
                    name.clone(),
                    target_label,
                    Some(tooltip_body_width),
                    cx,
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
                &self.focus_handle,
                bar.child(Self::tooltip(
                    "recording-start",
                    start_name,
                    toolbar::button("recording-start", start_name)
                        .is_disabled(self.countdown_active)
                        .child(toolbar::filled_glyph(theme.accent, true))
                        .on_press(cx.listener(|this, _, window, cx| this.start(window, cx))),
                    None,
                    cx,
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
                )),
                &self.menu,
                countdown,
                window,
                cx,
            );
        }

        let stop_name = "Stop recording";
        recording_shell(
            &self.focus_handle,
            bar.child(overlay_icon(
                "recording-pause",
                if paused { "play" } else { "pause" },
                if paused {
                    "Resume recording"
                } else {
                    "Pause recording"
                },
                false,
                |this, _window, cx| this.toggle_pause(cx),
                cx,
            ))
            .child(Self::tooltip(
                "recording-stop",
                stop_name,
                toolbar::button("recording-stop", stop_name)
                    .child(toolbar::filled_glyph(theme.destructive, false))
                    .on_press(cx.listener(|this, _, window, cx| {
                        this.finish(false, window, cx);
                    })),
                None,
                cx,
            ))
            .child(
                div()
                    .min_w(px(64.0))
                    .px(px(4.0))
                    .font_family(crate::ui::colors::MONO_FONT)
                    .text_size(px(12.0))
                    .text_center()
                    .child(crate::util::format::format_elapsed(self.elapsed)),
            )
            .child(toolbar::hairline(&theme))
            .children(toggles)
            .child(toolbar::hairline(&theme))
            .child(overlay_icon(
                "recording-discard",
                "trash-2",
                "Discard recording",
                false,
                |this, window, cx| {
                    this.finish(true, window, cx);
                },
                cx,
            )),
            &self.menu,
            None,
            window,
            cx,
        )
    }
}

fn recording_shell(
    focus: &FocusHandle,
    bar: impl IntoElement,
    menu: &MenuHandle,
    countdown: Option<u32>,
    window: &mut Window,
    cx: &mut Context<RecordingControl>,
) -> impl IntoElement {
    let content = crate::ui::font::root()
        .id("recording-control")
        .track_focus(focus)
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .on_key_down(
            cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                #[cfg(any(windows, target_os = "macos"))]
                this.clear_native_tooltip(window);
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
        .pt(px(chrome::RECORDING_BAR_PAD_TOP))
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
                    )),
            )
        })
        .children(menu.render(cx));
    crate::ui::window_root::focus_root(content, window, cx)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let (width, height) = visual.update(|window, _| {
            (
                window.bounds().size.width.as_f32().ceil() as usize,
                window.bounds().size.height.as_f32(),
            )
        });
        let y = height / 2.0;
        let mut microphone_point = None;
        for x in (0..width).step_by(2) {
            visual.simulate_mouse_move(
                gpui::point(px(x as f32), px(y)),
                None,
                gpui::Modifiers::none(),
            );
            let is_microphone = control
                .update(cx, |this, _, _| {
                    this.hovered_tooltip
                        .as_ref()
                        .is_some_and(|(_, text)| text == "Select microphone")
                })
                .unwrap();
            if is_microphone {
                microphone_point = Some(gpui::point(px(x as f32), px(y)));
                break;
            }
        }
        let microphone_point = microphone_point.expect("microphone control must be hit-testable");

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
        cx.simulate_keystrokes(control.into(), "a");
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
                    this.hovered_tooltip
                        .as_ref()
                        .map(|(_, text)| text.to_string())
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
            let close_delay = visual.update(|_, cx| cx.layout().tooltip_close_delay_ms);
            visual
                .executor()
                .advance_clock(Duration::from_millis(close_delay + 1));
        }
        visual.settle();
        visual.press("tab tab");
        assert!(visual.update(|window, cx| {
            window.focused(cx).is_some() && herogpui::extend::focus_visible(cx)
        }));
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
            visual.press("tab tab");

            let target_chip = visual.expect("recording-target-label");
            let (tip, mask, viewport, actual_scale, count) = visual.update(|window, cx| {
                let background =
                    gpui::Background::from(crate::theme::vars::active_theme(cx).overlay);
                let tips = window
                    .painted_quads()
                    .into_iter()
                    .filter(|quad| {
                        quad.background == background
                            && (31.0..=50.0).contains(&(quad.bounds.size.height.as_f32() / scale))
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
                    assert!(control.set_microphone(false, cx));
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
    fn device_menu_window_matches_electron_bounds() {
        assert_eq!(
            control_window_metrics(236.0, false, false),
            (332.0, 48.0, 52.0)
        );
        assert_eq!(
            control_window_metrics(236.0, false, true),
            (332.0, 48.0, 148.0)
        );
        assert_eq!(
            control_window_metrics(540.0, false, false),
            (572.0, 16.0, 52.0)
        );
        assert_eq!(
            control_window_metrics(236.0, true, false),
            (332.0, 48.0, 300.0)
        );
        assert_eq!(
            control_window_metrics(400.0, true, false),
            (432.0, 16.0, 300.0)
        );
        assert_eq!(
            (chrome::OVERLAY_TARGET_TRIGGER_WIDTH - DEVICE_DROPDOWN_WIDTH) / 2.0,
            -104.0
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
                microphone: true,
                camera: false,
                camera_locked: false,
                camera_preview_visible: false,
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
                recording_error_generation: 0,
                #[cfg(any(windows, target_os = "macos"))]
                hovered_tooltip: None,
                #[cfg(windows)]
                tooltip_generation: 0,
                #[cfg(windows)]
                native_tooltip: None,
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
            listed,
        ));
        assert_eq!(devices.microphones[0].id, "mic-1");
        assert_eq!(devices.cameras[0].id, "camera-1");
        assert_eq!(devices.default_microphone_id.as_deref(), Some("mic-1"));
        assert_eq!(devices.default_camera_id.as_deref(), Some("camera-1"));
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
            listed,
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
                microphone: true,
                camera: false,
                camera_locked: false,
                camera_preview_visible: false,
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
                recording_error_generation: 0,
                #[cfg(any(windows, target_os = "macos"))]
                hovered_tooltip: None,
                #[cfg(windows)]
                tooltip_generation: 0,
                #[cfg(windows)]
                native_tooltip: None,
            };
            control.bounds_subscription = Some(cx.observe_window_bounds(
                window,
                |this: &mut RecordingControl, window, cx| {
                    this.open_pending_device_menu(window, cx);
                },
            ));
            control
        });
        cx.simulate_resize(size(px(332.0), px(52.0)));

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
