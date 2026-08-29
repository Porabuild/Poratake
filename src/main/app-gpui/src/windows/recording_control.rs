//! The recording control bar — port of
//! `capture/video/recording-control-window.tsx`. One frameless always-on-top
//! surface anchored at the top centre of the recorded display, in both the
//! pre-recording and recording modes.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{
    div, prelude::*, px, size, AnyElement, App, Bounds, Context, FocusHandle, Render, SharedString,
    Styled, Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};

use crate::capture::overlay::ScreenRect;
use crate::theme::vars::{active_theme, ThemeVars};
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::menu::{MenuBuilder, MenuHandle, MenuItem, MenuPlacement};
use crate::video::recorder::{self, RecordingConfig, RecordingTarget};
use crate::windows::registry::{self, WindowKind as RegistryKind};

const TARGET_LABEL_WIDTH: f32 = chrome::RECORDING_TARGET_LABEL_WIDTH;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    PreRecording,
    Recording,
}

fn can_cancel_with_escape(mode: Mode) -> bool {
    mode == Mode::PreRecording
}

pub struct RecordingControl {
    mode: Mode,
    #[allow(dead_code)]
    target: RecordingTarget,
    target_name: Option<SharedString>,
    rect: ScreenRect,
    window_id: Option<i64>,
    project: Option<PathBuf>,
    system_audio: bool,
    microphone: bool,
    camera: bool,
    selected_mic_id: Option<String>,
    selected_camera_id: Option<String>,
    menu: MenuHandle,
    started_at: Option<Instant>,
    elapsed: u64,
    focus_handle: FocusHandle,
}

impl RecordingControl {
    pub fn open(
        cx: &mut App,
        target: RecordingTarget,
        rect: ScreenRect,
        window_id: Option<i64>,
        target_name: Option<String>,
    ) {
        if recorder::is_recording() {
            return;
        }
        if Self::update_pre_recording(cx, target, rect, window_id, target_name.as_deref()) {
            return;
        }
        let config = crate::state::state(cx).config.get().recording;
        let width = chrome::recording_control_width(false, target_name.is_some());
        let bounds = bar_bounds(cx, rect, width);

        registry::open_or_activate(RegistryKind::RecordingControl, cx, move |cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: None,
                    focus: true,
                    show: true,
                    kind: WindowKind::PopUp,
                    is_movable: false,
                    is_resizable: false,
                    is_minimizable: false,
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
                        window_id,
                        project: None,
                        system_audio: config.system_audio,
                        microphone: config.mic_enabled,
                        camera: config.camera.enabled,
                        selected_mic_id: config.selected_mic_id.clone(),
                        selected_camera_id: config.camera.selected_device_id.clone(),
                        menu: MenuHandle::new(),
                        started_at: None,
                        elapsed: 0,
                        focus_handle: cx.focus_handle(),
                    });
                    window.focus(&view.read(cx).focus_handle);
                    view
                },
            )
            .ok()
            .map(Into::into)
        });
    }

    pub fn cancel_pre_recording(cx: &mut App) -> bool {
        for window_handle in cx.windows() {
            let Some(handle) = window_handle.downcast::<Self>() else {
                continue;
            };
            let cancelled = handle
                .update(cx, |this, window, _cx| {
                    if !can_cancel_with_escape(this.mode) {
                        return false;
                    }
                    window.remove_window();
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

    fn update_pre_recording(
        cx: &mut App,
        target: RecordingTarget,
        rect: ScreenRect,
        window_id: Option<i64>,
        target_name: Option<&str>,
    ) -> bool {
        let width = chrome::recording_control_width(false, target_name.is_some());
        let bounds = bar_bounds(cx, rect, width);
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
                    this.window_id = window_id;
                    this.target_name = target_name.clone().map(SharedString::from);
                    window.resize(bounds.size);
                    #[cfg(windows)]
                    if let Some(hwnd) = crate::windows::window_hwnd(window) {
                        crate::system::window_composition::apply_window_bounds(
                            hwnd,
                            bounds,
                            window.scale_factor(),
                        );
                    }
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
            window_id: self.window_id,
            include_audio: self.system_audio,
            mic_enabled: self.microphone,
            mic_device_id: recording.selected_mic_id.clone(),
            camera_enabled: self.camera,
            camera_device_id: recording.camera.selected_device_id.clone(),
            keyboard_enabled: false,
            frame_rate: 60,
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
        #[cfg(target_os = "macos")]
        if !crate::capture::overlay::begin_recording_handoff(self.rect, cx) {
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
        #[cfg(not(target_os = "macos"))]
        let _ = crate::capture::overlay::begin_recording_handoff(self.rect, cx);

        self.project = Some(project);
        self.mode = Mode::Recording;
        self.started_at = Some(Instant::now());
        self.elapsed = 0;
        let width = chrome::recording_control_width(true, self.target_name.is_some());
        window.resize(size(px(width), px(chrome::RECORDING_WINDOW_HEIGHT)));
        crate::intents::refresh_shell(cx);
        self.tick(cx);
        cx.notify();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |entity, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let running = entity.update(cx, |this, cx| {
                let Some(started_at) = this.started_at else {
                    return false;
                };
                if recorder::state() == recorder::RecorderState::Recording {
                    this.elapsed = started_at.elapsed().as_secs();
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

    fn input_toggles(&self, window: &mut Window, cx: &mut Context<Self>) -> [AnyElement; 3] {
        [
            self.device_dropdown(
                "camera",
                if self.camera { "video" } else { "video-off" },
                "Select camera",
                self.camera,
                crate::system::devices::DeviceKind::Camera,
                window,
                cx,
            ),
            self.device_dropdown(
                "microphone",
                if self.microphone { "mic" } else { "mic-off" },
                "Select microphone",
                self.microphone,
                crate::system::devices::DeviceKind::Microphone,
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
                    let next = !this.system_audio;
                    this.set_system_audio(next, cx);
                }))
                .into_any_element(),
        ]
    }

    fn device_dropdown(
        &self,
        id: &'static str,
        icon: &'static str,
        _tooltip: &'static str,
        enabled: bool,
        kind: crate::system::devices::DeviceKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let handle = self.menu.clone();
        let owner = format!("recording-{id}");
        let selected = match kind {
            crate::system::devices::DeviceKind::Microphone => self.selected_mic_id.clone(),
            crate::system::devices::DeviceKind::Camera => self.selected_camera_id.clone(),
        };
        let lists = crate::system::devices::list(&crate::state::state(cx).daemon, &[kind]);
        let devices = match kind {
            crate::system::devices::DeviceKind::Microphone => lists.microphones,
            crate::system::devices::DeviceKind::Camera => lists.cameras,
        };
        let mut builder = MenuBuilder::new().item(
            MenuItem::new(if enabled { "Turn off" } else { "Turn on" }).on_select({
                let entity = cx.entity().downgrade();
                move |_window, cx| {
                    if let Some(entity) = entity.upgrade() {
                        entity.update(cx, |this, cx| match kind {
                            crate::system::devices::DeviceKind::Microphone => {
                                let next = !this.microphone;
                                this.set_microphone(next, cx);
                            }
                            crate::system::devices::DeviceKind::Camera => {
                                let next = !this.camera;
                                this.set_camera(next, cx);
                            }
                        });
                    }
                }
            }),
        );
        for device in &devices {
            let device_id = device.id.clone();
            let label = if device.label.trim().is_empty() {
                device.id.clone()
            } else {
                device.label.clone()
            };
            builder = builder.item(
                MenuItem::new(label)
                    .trailing_check(selected.as_deref() == Some(device_id.as_str()))
                    .on_select({
                        let entity = cx.entity().downgrade();
                        move |_window, cx| {
                            if let Some(entity) = entity.upgrade() {
                                entity.update(cx, |this, cx| {
                                    this.select_device(kind, &device_id, cx)
                                });
                            }
                        }
                    }),
            );
        }
        let entries = builder.build();
        let menu_id = owner.clone();
        // Gated hover flag instead of a `.hover()` style, which gpui paints
        // against the window's last mouse position and so survives the
        // pointer leaving the window.
        let (trigger_hover, trigger_hovered) =
            crate::ui::primitives::hover_flag(&owner, window, cx);
        div()
            .id(SharedString::from(owner.clone()))
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(4.0))
            .h(px(chrome::OVERLAY_BUTTON_SIZE))
            .w(px(48.0))
            .rounded(px(chrome::OVERLAY_BUTTON_RADIUS))
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
            .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
                handle.toggle(
                    MenuPlacement::below(menu_id.clone()),
                    entries.clone(),
                    window,
                    cx,
                );
                cx.stop_propagation();
            })
            .into_any_element()
    }

    fn select_device(
        &mut self,
        kind: crate::system::devices::DeviceKind,
        id: &str,
        cx: &mut Context<Self>,
    ) {
        match kind {
            crate::system::devices::DeviceKind::Microphone => {
                let previous = self.selected_mic_id.clone();
                self.selected_mic_id = Some(id.to_string());
                if !self.set_microphone(true, cx) {
                    self.selected_mic_id = previous;
                    cx.notify();
                    return;
                }
                crate::state::state(cx).config.update(|config| {
                    config.recording.selected_mic_id = Some(id.to_string());
                });
            }
            crate::system::devices::DeviceKind::Camera => {
                let previous = self.selected_camera_id.clone();
                self.selected_camera_id = Some(id.to_string());
                if !self.set_camera(true, cx) {
                    self.selected_camera_id = previous;
                    cx.notify();
                    return;
                }
                crate::state::state(cx).config.update(|config| {
                    config.recording.camera.selected_device_id = Some(id.to_string());
                });
            }
        }
    }

    fn toggle_pause(&mut self, cx: &mut Context<Self>) {
        let daemon = crate::state::state(cx).daemon;
        match recorder::state() {
            recorder::RecorderState::Recording => recorder::pause(&daemon),
            recorder::RecorderState::Paused => recorder::resume(&daemon),
            recorder::RecorderState::Idle => {}
        }
        cx.notify();
    }

    fn finish(&mut self, discard: bool, window: &mut Window, cx: &mut Context<Self>) {
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

        let (max_items, show_preview) = {
            let config = service.config.get();
            (
                config.history.max_items as usize,
                config.recording.show_preview,
            )
        };
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
                duration: Some(self.elapsed as f64),
            },
            max_items,
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
        window.remove_window();
        registry::close(RegistryKind::RecordingControl, cx);
        crate::capture::overlay::end_recording_handoff(cx);
    }
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

fn bar_bounds(cx: &mut App, rect: ScreenRect, width: f32) -> Bounds<gpui::Pixels> {
    let scale = crate::capture::overlay::app_scale_factor(cx).max(0.01);
    let displays: Vec<(f32, f32, f32, f32)> = cx
        .displays()
        .into_iter()
        .map(|display| {
            let bounds = display.bounds();
            (
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height),
            )
        })
        .collect();
    let center_x = rect.x as f32 / scale + rect.width as f32 / scale / 2.0;
    let center_y = rect.y as f32 / scale + rect.height as f32 / scale / 2.0;
    let (work_x, work_y, work_width, _) = chrome::display_containing(&displays, center_x, center_y)
        .unwrap_or((0.0, 0.0, 1920.0, 1080.0));
    let (x, y) = chrome::recording_bar_origin(work_x, work_y, work_width, width);
    Bounds {
        origin: gpui::point(px(x), px(y)),
        size: size(px(width), px(chrome::RECORDING_WINDOW_HEIGHT)),
    }
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
        .icon(icon)
        .tooltip(tooltip)
        .on_click(cx.listener(move |this, _event, window, cx| on_click(this, window, cx)))
        .into_any_element()
}

impl Render for RecordingControl {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        let paused = recorder::state() == recorder::RecorderState::Paused;
        let toggles = self.input_toggles(window, cx);

        let mut bar = div()
            .id("recording-control-bar")
            .h(px(chrome::recording_inner_bar_height()))
            .flex()
            .flex_row()
            .items_center()
            // The recording bar and the all-in-one toolbar are the same
            // `ToolbarSurface` in Electron -- `gap-0.5 p-1 rounded-4xl border-2`
            // -- so they take the same metrics here. Bespoke ones made this bar
            // 12px/16px instead of 2px/4px, which pushed its contents wider than
            // its own window and clipped the record dot and the close button.
            .gap(px(chrome::OVERLAY_SURFACE_GAP))
            .rounded(px(chrome::OVERLAY_SURFACE_RADIUS))
            .border_2()
            .border_color(theme.muted_foreground.opacity(0.35))
            .bg(theme.muted_background.opacity(0.95))
            .text_color(theme.foreground)
            .shadow_2xl()
            .p(px(chrome::OVERLAY_SURFACE_PADDING));

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
                    .icon("trash-2")
                    .tooltip("Discard recording")
                    .on_click(
                        cx.listener(|this, _event, window, cx| this.finish(true, window, cx)),
                    ),
            ),
            &self.menu,
            cx,
        )
    }
}

fn recording_shell(
    focus: &FocusHandle,
    bar: impl IntoElement,
    menu: &MenuHandle,
    cx: &mut Context<RecordingControl>,
) -> impl IntoElement {
    div()
        .id("recording-control")
        .track_focus(focus)
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" && can_cancel_with_escape(this.mode) {
                this.cancel(window, cx);
            }
        }))
        .pt(px(chrome::RECORDING_BAR_PAD_TOP))
        .child(bar)
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
        assert_eq!(chrome::RECORDING_TOP_MARGIN, 24.0);
        let (x, y) = chrome::recording_bar_origin(0.0, 10.0, 1920.0, 236.0);
        assert_eq!(y, 34.0);
        assert_eq!(x, ((1920.0_f32 - 236.0) / 2.0).round());
    }

    #[test]
    fn escape_only_cancels_before_recording_starts() {
        assert!(can_cancel_with_escape(Mode::PreRecording));
        assert!(!can_cancel_with_escape(Mode::Recording));
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
}
