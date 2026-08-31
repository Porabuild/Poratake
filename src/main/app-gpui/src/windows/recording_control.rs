//! The recording control bar — port of
//! `capture/video/recording-control-window.tsx`. One frameless always-on-top
//! surface anchored at the top centre of the recorded display, in both the
//! pre-recording and recording modes.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{
    div, point, prelude::*, px, size, AnyElement, App, Bounds, Context, FocusHandle, Pixels,
    Render, SharedString, Styled, Subscription, WeakEntity, Window, WindowBackgroundAppearance,
    WindowBounds, WindowKind, WindowOptions,
};

use crate::capture::overlay::ScreenRect;
use crate::theme::vars::{active_theme, ThemeVars};
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::menu::{MenuBuilder, MenuEntry, MenuHandle, MenuItem, MenuPlacement};
use crate::video::recorder::{self, RecordingConfig, RecordingTarget};
use crate::windows::registry::{self, WindowKind as RegistryKind};

const TARGET_LABEL_WIDTH: f32 = chrome::RECORDING_TARGET_LABEL_WIDTH;
const DEVICE_MENU_WINDOW_WIDTH: f32 = 300.0;
const DEVICE_MENU_WINDOW_HEIGHT: f32 = 300.0;
const CONTROL_WINDOW_HORIZONTAL_GUTTER: f32 = 16.0;
const DEVICE_DROPDOWN_WIDTH: f32 = 256.0;
const DEVICE_DROPDOWN_HEIGHT: f32 = 224.0;
const COUNTDOWN_WINDOW_HEIGHT: f32 = 148.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    PreRecording,
    Recording,
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
    focus_handle: FocusHandle,
}

impl RecordingControl {
    pub fn open(
        cx: &mut App,
        target: RecordingTarget,
        rect: ScreenRect,
        display_id: Option<u32>,
        window_id: Option<i64>,
        target_name: Option<String>,
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
        ) {
            set_pre_recording_escape(true, cx);
            return;
        }
        let config = crate::state::state(cx).config.get().recording;
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
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(window_bounds)),
                    titlebar: None,
                    focus: true,
                    show: true,
                    kind: WindowKind::PopUp,
                    is_movable: false,
                    is_resizable: false,
                    is_minimizable: false,
                    display_id: platform_display_id,
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                |window, cx| {
                    configure_toolbar_window(window);
                    let view = cx.new(|cx| Self {
                        mode: Mode::PreRecording,
                        target,
                        target_name: target_name.map(SharedString::from),
                        rect,
                        display_id,
                        window_id,
                        project: None,
                        system_audio: config.system_audio,
                        microphone: config.mic_enabled,
                        camera: config.camera.enabled,
                        camera_locked: false,
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
                        countdown_generation: 0,
                        countdown_active: false,
                        countdown_remaining: None,
                        focus_handle: cx.focus_handle(),
                    });
                    view.update(cx, |this, cx| {
                        this.bounds_subscription =
                            Some(cx.observe_window_bounds(window, |this, window, cx| {
                                this.open_pending_device_menu(window, cx);
                            }));
                    });
                    window.focus(&view.read(cx).focus_handle);
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
        if !self.menu.is_present() {
            return false;
        }
        self.menu.close(window);
        self.sync_window_bounds(window, cx, false);
        true
    }

    fn update_pre_recording(
        cx: &mut App,
        target: RecordingTarget,
        rect: ScreenRect,
        display_id: Option<u32>,
        window_id: Option<i64>,
        target_name: Option<&str>,
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
        let project = match recorder::create_project(&service.config) {
            Ok(path) => path,
            Err(error) => {
                eprintln!("[recorder] failed to create project: {error}");
                return;
            }
        };
        let recording = service.config.get().recording;
        let config = RecordingConfig {
            x: self.rect.x,
            y: self.rect.y,
            width: self.rect.width,
            height: self.rect.height,
            display_id: self.display_id,
            window_id: self.window_id,
            include_audio: self.system_audio,
            mic_enabled: self.microphone,
            mic_device_id: recording.selected_mic_id.clone(),
            camera_enabled: self.camera,
            camera_device_id: recording.camera.selected_device_id.clone(),
            ios_device_id: self.selected_ios_id.clone(),
            ios_device_name: self.selected_ios_name.clone(),
            keyboard_enabled: false,
            frame_rate: recording.frame_rate,
            output_path: project.clone(),
        };

        if !crate::capture::overlay::release_frozen_for_recording(cx) {
            let _ = std::fs::remove_dir_all(&project);
            crate::windows::toast::Toast::show(
                cx,
                "Recording failed",
                "Could not release the frozen screen",
            );
            return;
        }
        let ios_recording = self.selected_ios_id.is_some();
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
        if let Err(error) = recorder::start(&service.daemon, &config) {
            eprintln!("[recorder] {error}");
            let _ = std::fs::remove_dir_all(&project);
            #[cfg(target_os = "macos")]
            crate::capture::overlay::end_recording_handoff(cx);
            crate::windows::toast::Toast::show(cx, "Recording failed", error.to_string());
            return;
        }
        set_pre_recording_escape(false, cx);
        #[cfg(not(target_os = "macos"))]
        let _ = crate::capture::overlay::begin_recording_handoff(self.rect, cx);

        self.project = Some(project);
        self.mode = Mode::Recording;
        self.camera_locked = self.camera;
        self.started_at = Some(Instant::now());
        self.paused_at = None;
        self.paused_total = Duration::ZERO;
        self.elapsed = 0;
        self.sync_window_bounds(
            window,
            cx,
            self.pending_device_menu.is_some() || self.menu.is_present(),
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
                    this.elapsed = this.recording_duration().as_secs();
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
        if self.mode == Mode::Recording {
            let result = recorder::set_camera(&crate::state::state(cx).daemon, enabled);
            if let Err(error) = result {
                report_toggle_failure(cx, &error);
                return false;
            }
        }
        self.camera = enabled;
        cx.notify();
        true
    }

    fn input_toggles(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut toggles = Vec::new();
        if self.mode == Mode::PreRecording || self.camera_locked {
            toggles.push(self.device_dropdown(
                "camera",
                if self.camera { "video" } else { "video-off" },
                "Select camera",
                DeviceMenuKind::Media(crate::system::devices::DeviceKind::Camera),
                window,
                cx,
            ));
        }
        toggles.extend([
            self.device_dropdown(
                "microphone",
                if self.microphone { "mic" } else { "mic-off" },
                "Select microphone",
                DeviceMenuKind::Media(crate::system::devices::DeviceKind::Microphone),
                window,
                cx,
            ),
            Button::new("recording-system-audio")
                .variant(ButtonVariant::Ghost)
                // Deliberately not `.selected()`: `ToolbarButton` is a plain
                // ghost with `aria-pressed` and no visual pressed state, so the
                // icon swap below is the entire signal. Marking it selected
                // promotes the button to `Secondary` and paints a filled chip
                // the reference never shows.
                .size(ButtonSize::IconSm)
                .radius(px(chrome::OVERLAY_BUTTON_RADIUS))
                .animate_press(false)
                .icon(if self.system_audio {
                    "volume-2"
                } else {
                    "volume-x"
                })
                .tooltip(if self.system_audio {
                    "Turn system sounds off"
                } else {
                    "Turn system sounds on"
                })
                .on_click(cx.listener(|this, _event, _window, cx| {
                    if this.countdown_active {
                        return;
                    }
                    let next = !this.system_audio;
                    this.set_system_audio(next, cx);
                }))
                .into_any_element(),
        ]);
        #[cfg(target_os = "macos")]
        if self.mode == Mode::PreRecording {
            toggles.push(self.device_dropdown(
                "ios-device",
                "smartphone",
                "Select iPhone or iPad",
                DeviceMenuKind::Ios,
                window,
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = format!("recording-{id}");
        let entity = cx.entity().downgrade();
        let key_entity = entity.clone();
        let menu_id = owner.clone();
        let key_menu_id = menu_id.clone();
        let theme = active_theme(cx);
        let focus = crate::ui::primitives::control_focus(&owner, false, window, cx);
        let (trigger_hover, trigger_hovered) =
            crate::ui::primitives::hover_flag(&owner, window, cx);
        div()
            .id(SharedString::from(owner.clone()))
            .track_focus(&focus)
            .focus(move |style| style.shadow(crate::ui::primitives::focus_ring(&theme, 2.0)))
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(4.0))
            .h(px(chrome::OVERLAY_BUTTON_SIZE))
            .w(px(48.0))
            .rounded(px(chrome::OVERLAY_BUTTON_RADIUS))
            .opacity(if self.countdown_active { 0.35 } else { 1.0 })
            .when(trigger_hovered, |el| el.bg(crate::ui::colors::white(0.15)))
            .on_hover({
                let trigger_hover = trigger_hover.clone();
                move |over: &bool, _window, cx| {
                    crate::ui::primitives::track_hover(&trigger_hover, *over, cx);
                }
            })
            .child(icon_element(icon, px(chrome::TOOL_BUTTON_ICON)))
            .child(icon_element("chevron-down", px(12.0)))
            .child(self.menu.render_dropdown(&menu_id))
            .tooltip(move |_window, cx| {
                cx.new(|_| crate::ui::tooltip::Tooltip::new(tooltip)).into()
            })
            .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
                if let Some(entity) = entity.upgrade() {
                    entity.update(cx, |this, cx| {
                        if this.countdown_active {
                            return;
                        }
                        this.toggle_device_menu(kind, menu_id.clone().into(), window, cx);
                    });
                }
                cx.stop_propagation();
            })
            .on_key_down(move |event, window, cx| {
                if !activates_device_menu(event.keystroke.key.as_str()) {
                    return;
                }
                if let Some(entity) = key_entity.upgrade() {
                    entity.update(cx, |this, cx| {
                        if this.countdown_active {
                            return;
                        }
                        this.toggle_device_menu(kind, key_menu_id.clone().into(), window, cx);
                    });
                }
                cx.stop_propagation();
            })
            .into_any_element()
    }

    fn toggle_device_menu(
        &mut self,
        kind: DeviceMenuKind,
        owner: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let opening = !self.menu.is_open_for(owner.as_ref());
        if !opening {
            self.pending_device_menu = None;
            self.menu.close(window);
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
        if self.menu.is_open_for(owner.as_ref()) {
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
                if this.menu.is_open_for(owner.as_ref()) {
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
                if this.menu.is_open_for(owner.as_ref()) {
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
                crate::state::state(cx).config.update(move |config| {
                    config.recording.selected_mic_id = id;
                    config.recording.selected_mic_name = name;
                });
            }
            crate::system::devices::DeviceKind::Camera => {
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
                    self.paused_at = Some(Instant::now());
                }
            }
            recorder::RecorderState::Paused => {
                recorder::resume(&daemon);
                if recorder::state() == recorder::RecorderState::Recording {
                    if let Some(paused_at) = self.paused_at.take() {
                        self.paused_total += paused_at.elapsed();
                    }
                }
            }
            recorder::RecorderState::Idle => {}
        }
        cx.notify();
    }

    fn recording_duration(&self) -> Duration {
        let Some(started_at) = self.started_at else {
            return Duration::from_secs(self.elapsed);
        };
        let current_pause = self
            .paused_at
            .map(|paused_at| paused_at.elapsed())
            .unwrap_or_default();
        started_at
            .elapsed()
            .saturating_sub(self.paused_total + current_pause)
    }

    fn finish(&mut self, discard: bool, window: &mut Window, cx: &mut Context<Self>) {
        let duration = self.recording_duration().as_secs_f64();
        let service = crate::state::state(cx);
        let stopped = recorder::stop(&service.daemon);
        if !stopped {
            crate::windows::toast::Toast::show(
                cx,
                "Recording not finalized",
                "Stop failed. The recording controls remain available so you can retry.",
            );
            return;
        }

        let project = self.project.take();
        self.started_at = None;
        self.paused_at = None;
        self.paused_total = Duration::ZERO;

        window.remove_window();
        registry::close(RegistryKind::RecordingControl, cx);
        crate::capture::overlay::end_recording_handoff(cx);
        crate::intents::refresh_shell(cx);

        let Some(project) = project else {
            return;
        };
        if discard {
            let _ = std::fs::remove_dir_all(&project);
            return;
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
            let path = project.to_string_lossy().to_string();
            cx.defer(move |cx| {
                crate::windows::video_editor::VideoEditorWindow::open(cx, Some(path));
            });
        }
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == Mode::Recording {
            self.finish(true, window, cx);
            return;
        }
        self.countdown_generation += 1;
        self.countdown_active = false;
        self.countdown_remaining = None;
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
        let mut width =
            recording_control_width(self.mode == Mode::Recording, self.target_name.is_some());
        if self.mode == Mode::Recording && !self.camera_locked {
            width -= 48.0;
        }
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
        .min_width(px(DEVICE_DROPDOWN_WIDTH))
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

fn activates_device_menu(key: &str) -> bool {
    matches!(key, "enter" | "space")
}

fn report_toggle_failure(cx: &mut Context<RecordingControl>, error: &anyhow::Error) {
    crate::windows::toast::Toast::show(cx, "Recording control failed", error.to_string());
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
            let bounds = crate::system::work_area::display_bounds(display.as_ref());
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

fn display_for_rect(
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
    let window_width = if device_menu_open {
        width.max(DEVICE_MENU_WINDOW_WIDTH) + CONTROL_WINDOW_HORIZONTAL_GUTTER * 2.0
    } else {
        width
    };
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

fn overlay_hairline(theme: &ThemeVars) -> AnyElement {
    div()
        .mx(px(chrome::OVERLAY_HAIRLINE_INSET))
        .h(px(chrome::OVERLAY_HAIRLINE_HEIGHT))
        .w(px(1.0))
        .flex_none()
        .bg(theme.border.opacity(0.7))
        .into_any_element()
}

fn overlay_icon(
    id: &'static str,
    icon: &'static str,
    tooltip: &'static str,
    on_click: impl Fn(&mut RecordingControl, &mut Window, &mut Context<RecordingControl>) + 'static,
    cx: &mut Context<RecordingControl>,
) -> AnyElement {
    Button::new(id)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::IconSm)
        .radius(px(chrome::OVERLAY_BUTTON_RADIUS))
        .animate_press(false)
        .icon(icon)
        .tooltip(tooltip)
        .on_click(cx.listener(move |this, _event, window, cx| on_click(this, window, cx)))
        .into_any_element()
}

impl Render for RecordingControl {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.pending_device_menu.is_none()
            && !self.menu.is_present()
            && window.bounds().size.height > px(chrome::RECORDING_WINDOW_HEIGHT)
        {
            self.sync_window_bounds(window, cx, false);
        }
        let theme = active_theme(cx);
        let paused = recorder::state() == recorder::RecorderState::Paused;
        let toggles = self.input_toggles(window, cx);
        let countdown = self.countdown_remaining;

        let mut bar = crate::ui::primitives::toolbar_surface(&theme)
            .id("recording-control-bar")
            .h(px(chrome::recording_inner_bar_height()));

        if let Some(name) = &self.target_name {
            bar = bar
                .child(
                    div()
                        .max_w(px(TARGET_LABEL_WIDTH))
                        .truncate()
                        .px(px(4.0))
                        .text_size(px(12.0))
                        .child(name.clone()),
                )
                .child(overlay_hairline(&theme));
        }

        if self.mode == Mode::PreRecording {
            return recording_shell(
                &self.focus_handle,
                bar.child(
                    Button::new("recording-start")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::IconSm)
                        .radius(px(chrome::OVERLAY_BUTTON_RADIUS))
                        .animate_press(false)
                        .disabled(self.countdown_active)
                        // `<Circle className="size-3.5 fill-current" />` -- a
                        // *filled* disc. The lucide icons here are stroke-only,
                        // so an outline circle is the wrong shape; a filled div
                        // is what `fill-current` draws.
                        .child(filled_glyph(theme.accent, true))
                        .tooltip("Start recording")
                        .on_click(cx.listener(|this, _event, window, cx| this.start(window, cx))),
                )
                .child(overlay_hairline(&theme))
                .children(toggles)
                .child(overlay_hairline(&theme))
                .child(overlay_icon(
                    "recording-cancel",
                    "x",
                    "Close",
                    |this, window, cx| this.cancel(window, cx),
                    cx,
                )),
                &self.menu,
                countdown,
                cx,
            );
        }

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
                |this, _window, cx| this.toggle_pause(cx),
                cx,
            ))
            .child(
                Button::new("recording-stop")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::IconSm)
                    .radius(px(chrome::OVERLAY_BUTTON_RADIUS))
                    .animate_press(false)
                    // `<Square className="size-3.5 fill-current text-destructive" />`.
                    .child(filled_glyph(theme.destructive, false))
                    .tooltip("Stop recording")
                    .on_click(
                        cx.listener(|this, _event, window, cx| this.finish(false, window, cx)),
                    ),
            )
            .child(
                div()
                    .min_w(px(64.0))
                    .px(px(4.0))
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_center()
                    .child(recorder::format_elapsed(self.elapsed)),
            )
            .child(overlay_hairline(&theme))
            .children(toggles)
            .child(overlay_hairline(&theme))
            .child(
                Button::new("recording-discard")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::IconSm)
                    .radius(px(chrome::OVERLAY_BUTTON_RADIUS))
                    .animate_press(false)
                    .icon("trash-2")
                    .tooltip("Discard recording")
                    .on_click(
                        cx.listener(|this, _event, window, cx| this.finish(true, window, cx)),
                    ),
            ),
            &self.menu,
            None,
            cx,
        )
    }
}

fn recording_shell(
    focus: &FocusHandle,
    bar: impl IntoElement,
    menu: &MenuHandle,
    countdown: Option<u32>,
    cx: &mut Context<RecordingControl>,
) -> impl IntoElement {
    div()
        .id("recording-control")
        .track_focus(focus)
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .on_key_down(
            cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
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
        .child(bar)
        .when_some(countdown, |el, seconds| {
            el.child(
                crate::ui::primitives::toolbar_surface(&active_theme(cx))
                    .mt(px(8.0))
                    .gap(px(12.0))
                    .px(px(16.0))
                    .py(px(10.0))
                    .child(
                        div()
                            .text_size(px(30.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(seconds.to_string()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
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
                        |this, window, cx| this.cancel(window, cx),
                        cx,
                    )),
            )
        })
        .children(menu.render())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_width_follows_mode_and_target_label() {
        assert_eq!(chrome::recording_control_width(false, false), 236.0);
        assert_eq!(chrome::recording_control_width(true, false), 400.0);
        assert_eq!(chrome::recording_control_width(false, true), 376.0);
        assert_eq!(chrome::recording_control_width(true, true), 540.0);
        assert_eq!(TARGET_LABEL_WIDTH, 140.0);
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
            10.0 + chrome::overlay_toolbar_top()
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
            (236.0, 0.0, 52.0)
        );
        assert_eq!(
            control_window_metrics(236.0, false, true),
            (236.0, 0.0, 148.0)
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

    #[gpui::test]
    fn microphone_menu_matches_electron_rows(cx: &mut gpui::TestAppContext) {
        let control = cx.update(|cx| {
            cx.new(|cx| RecordingControl {
                mode: Mode::PreRecording,
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
                focus_handle: cx.focus_handle(),
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

    #[gpui::test]
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
                focus_handle: cx.focus_handle(),
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
        assert!(control.read_with(cx, |control, _| {
            control.pending_device_menu.is_some() && !control.menu.is_open()
        }));

        cx.update(|window, cx| {
            control.update(cx, |control, cx| {
                assert!(control.dismiss_device_menu(window, cx));
            });
        });
        cx.simulate_resize(size(px(332.0), px(300.0)));
        assert!(control.read_with(cx, |control, _| {
            control.pending_device_menu.is_none() && !control.menu.is_open()
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
        assert!(control.read_with(cx, |control, _| {
            control.pending_device_menu.is_none() && control.menu.is_open_for("recording-camera")
        }));
    }

    #[test]
    fn device_menu_supports_keyboard_activation() {
        assert!(activates_device_menu("enter"));
        assert!(activates_device_menu("space"));
        assert!(!activates_device_menu("escape"));
    }
}

/// `size-3.5 fill-current`: a solid 14px disc for the record button, a solid
/// 14px square for stop. Rounding is the only difference between them.
fn filled_glyph(color: gpui::Hsla, round: bool) -> gpui::AnyElement {
    let glyph = gpui::div().size(px(RECORD_GLYPH_SIZE)).bg(color);
    if round {
        glyph.rounded_full().into_any_element()
    } else {
        // `<Square>` has lucide's own 2px corner, which at this scale reads as
        // square; `rounded-sm` is the nearest token.
        glyph.rounded(px(chrome::RADIUS_SM)).into_any_element()
    }
}

/// `size-3.5`.
const RECORD_GLYPH_SIZE: f32 = 14.0;

#[cfg(test)]
mod toolbar_tests {
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

    #[test]
    fn the_bar_keeps_button_glyphs_stationary_while_pressed() {
        let here = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/windows/recording_control.rs"),
        )
        .expect("read recording_control.rs");
        let production = here.split("#[cfg(test)]").next().unwrap_or_default();

        assert_eq!(production.matches(".animate_press(false)").count(), 5);
    }
}
