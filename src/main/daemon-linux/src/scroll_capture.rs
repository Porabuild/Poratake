use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use glib::ControlFlow;
use global_hotkey::hotkey::{Code, HotKey};
use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use gtk::gdk;
use gtk::prelude::*;
use poratake_daemon_common::contract::{
    SCROLL_CAPTURE_CANCELLED_EVENT, SCROLL_CAPTURE_DONE_EVENT, SCROLL_CAPTURE_FRAME_EVENT,
    SCROLL_CAPTURE_MODULE, SCROLL_CAPTURE_SCROLL_ENDED_EVENT, ScrollCaptureAutoScrollRequest,
    ScrollCaptureFinishRequest, ScrollCaptureFinishResult, ScrollCaptureMethod,
    ScrollCaptureStartRequest,
};
use poratake_daemon_common::geometry::CaptureRect;
use poratake_daemon_common::platform::LinuxBackend;
use poratake_daemon_common::protocol::{Request, Response, params, send_event, send_response};
use poratake_daemon_common::router::{Module, Reply, method_not_found};
use poratake_daemon_common::scroll::{CaptureOutcome, FrameAccumulator, scroll_plan, write_png};
use serde_json::json;
use xcb::x;

use crate::capture::capture_area_pixels;
use crate::gtk_runtime::GtkRuntime;
use crate::hotkeys;

enum ScrollCommand {
    Start {
        id: String,
        request: ScrollCaptureStartRequest,
    },
    StartAuto {
        id: String,
        request: ScrollCaptureAutoScrollRequest,
    },
    StopAuto {
        id: String,
    },
    Finish {
        id: String,
        path: PathBuf,
    },
    Cancel {
        id: String,
    },
    HotkeyCancel,
    HotkeyDone,
}

static COMMAND_SENDER: OnceLock<Mutex<Option<glib::Sender<ScrollCommand>>>> = OnceLock::new();

pub(crate) fn cancel_from_global_hotkey() {
    if let Some(sender) = COMMAND_SENDER
        .get()
        .and_then(|sender| sender.lock().ok())
        .and_then(|sender| sender.clone())
    {
        let _ = sender.send(ScrollCommand::HotkeyCancel);
    }
}

pub(crate) fn install_global_hotkey_handler() {
    let escape_id = HotKey::new(None, Code::Escape).id();
    let enter_id = HotKey::new(None, Code::Enter).id();
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.state != HotKeyState::Pressed {
            return;
        }
        if event.id == escape_id {
            crate::timer_control::cancel_from_global_hotkey();
            cancel_from_global_hotkey();
        } else if event.id == enter_id
            && let Some(sender) = COMMAND_SENDER
                .get()
                .and_then(|sender| sender.lock().ok())
                .and_then(|sender| sender.clone())
        {
            let _ = sender.send(ScrollCommand::HotkeyDone);
        }
    }));
}

struct X11Input {
    connection: xcb::Connection,
    root: x::Window,
}

impl X11Input {
    fn connect() -> Result<Self, String> {
        let (connection, screen_number) =
            xcb::Connection::connect_with_extensions(None, &[xcb::Extension::Test], &[])
                .map_err(|error| format!("could not connect to X11 input: {error}"))?;
        let root = connection
            .get_setup()
            .roots()
            .nth(screen_number as usize)
            .ok_or_else(|| "X11 did not expose a screen".to_string())?
            .root();
        Ok(Self { connection, root })
    }

    fn move_to(&self, x: i16, y: i16) -> Result<(), String> {
        self.connection.send_request(&xcb::xtest::FakeInput {
            r#type: 6,
            detail: 0,
            time: 0,
            root: self.root,
            root_x: x,
            root_y: y,
            deviceid: 0,
        });
        self.connection
            .flush()
            .map_err(|error| format!("could not move the pointer: {error}"))
    }

    fn pointer_inside(&self, rect: CaptureRect) -> bool {
        self.connection
            .wait_for_reply(
                self.connection
                    .send_request(&x::QueryPointer { window: self.root }),
            )
            .is_ok_and(|pointer| {
                let x = i32::from(pointer.root_x());
                let y = i32::from(pointer.root_y());
                x >= rect.x && y >= rect.y && x < rect.x + rect.width && y < rect.y + rect.height
            })
    }

    fn scroll_down(&self) -> Result<(), String> {
        for event_type in [4, 5] {
            self.connection.send_request(&xcb::xtest::FakeInput {
                r#type: event_type,
                detail: 5,
                time: 0,
                root: self.root,
                root_x: 0,
                root_y: 0,
                deviceid: 0,
            });
        }
        self.connection
            .flush()
            .map_err(|error| format!("could not inject scroll input: {error}"))
    }
}

struct ScrollState {
    request: Option<ScrollCaptureStartRequest>,
    accumulator: FrameAccumulator,
    scroll_step_points: usize,
    scroll_steps_per_frame: usize,
    current_scroll_step: usize,
    capture_on_next_tick: bool,
    source: Option<glib::SourceId>,
    boundary: Vec<gtk::Window>,
    panel: Option<gtk::Window>,
    auto_button: Option<gtk::Button>,
    escape_hotkey: HotKey,
    enter_hotkey: HotKey,
    hotkeys_registered: bool,
    input: X11Input,
}

impl ScrollState {
    fn new(input: X11Input) -> Self {
        Self {
            request: None,
            accumulator: FrameAccumulator::default(),
            scroll_step_points: 0,
            scroll_steps_per_frame: 0,
            current_scroll_step: 0,
            capture_on_next_tick: false,
            source: None,
            boundary: Vec::new(),
            panel: None,
            auto_button: None,
            escape_hotkey: HotKey::new(None, Code::Escape),
            enter_hotkey: HotKey::new(None, Code::Enter),
            hotkeys_registered: false,
            input,
        }
    }

    fn stop_auto(&mut self) {
        if let Some(source) = self.source.take() {
            source.remove();
        }
        self.current_scroll_step = 0;
        self.capture_on_next_tick = false;
        if let Some(button) = self.auto_button.as_ref() {
            button.set_label("Auto");
        }
    }

    fn hide_ui(&mut self) {
        for window in self.boundary.drain(..) {
            window.hide();
            unsafe {
                window.destroy();
            }
        }
        if let Some(window) = self.panel.take() {
            window.hide();
            unsafe {
                window.destroy();
            }
        }
        self.auto_button = None;
        if self.hotkeys_registered {
            hotkeys::unregister(self.escape_hotkey);
            hotkeys::unregister(self.enter_hotkey);
            self.hotkeys_registered = false;
        }
    }

    fn reset(&mut self) {
        self.stop_auto();
        self.hide_ui();
        self.request = None;
        self.accumulator = FrameAccumulator::default();
        self.scroll_step_points = 0;
        self.scroll_steps_per_frame = 0;
    }

    fn capture(&mut self) -> Result<CaptureOutcome, String> {
        let request = self
            .request
            .as_ref()
            .ok_or_else(|| "Not in capture mode".to_string())?;
        let rect = request.capture.rect;
        let visible: Vec<bool> = self
            .boundary
            .iter()
            .map(gtk::prelude::WidgetExt::is_visible)
            .chain(self.panel.iter().map(gtk::prelude::WidgetExt::is_visible))
            .collect();
        for window in self.boundary.iter().chain(self.panel.iter()) {
            window.hide();
        }
        if let Some(display) = gdk::Display::default() {
            display.flush();
        }
        let image = capture_area_pixels(LinuxBackend::X11, rect).map_err(|error| error.to_string());
        for (window, was_visible) in self.boundary.iter().chain(self.panel.iter()).zip(visible) {
            if was_visible {
                window.show_all();
            }
        }
        if let Some(display) = gdk::Display::default() {
            display.flush();
        }
        let pixels = image?.into_raw();
        let width = request.capture.rect.width as usize;
        let height = request.capture.rect.height as usize;
        let expected = height
            .saturating_sub(self.scroll_step_points)
            .clamp(1, height);
        Ok(self.accumulator.submit(pixels, width, height, expected))
    }
}

fn window(x: i32, y: i32, width: i32, height: i32, name: &str) -> gtk::Window {
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title(name);
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_accept_focus(false);
    window.set_focus_on_map(false);
    window.set_keep_above(true);
    window.set_skip_pager_hint(true);
    window.set_skip_taskbar_hint(true);
    window.set_type_hint(gdk::WindowTypeHint::Utility);
    window.set_default_size(width.max(1), height.max(1));
    window.move_(x, y);
    window
}

fn show_ui(state: &Rc<RefCell<ScrollState>>) -> Result<(), String> {
    let request = state
        .borrow()
        .request
        .clone()
        .ok_or_else(|| "Not in capture mode".to_string())?;
    let capture = request.capture;
    let scale = capture.scale_factor;
    let ui_x = (capture.rect.x as f64 / scale).round() as i32;
    let ui_y = (capture.rect.y as f64 / scale).round() as i32;
    let ui_width = (capture.rect.width as f64 / scale).round().max(1.0) as i32;
    let ui_height = (capture.rect.height as f64 / scale).round().max(1.0) as i32;
    {
        let mut state = state.borrow_mut();
        hotkeys::register(state.escape_hotkey)
            .map_err(|error| format!("could not register Escape: {error}"))?;
        if let Err(error) = hotkeys::register(state.enter_hotkey) {
            hotkeys::unregister(state.escape_hotkey);
            return Err(format!("could not register Enter: {error}"));
        }
        state.hotkeys_registered = true;
    }
    install_global_hotkey_handler();
    let thickness = 2;
    let edges = [
        (ui_x - thickness, ui_y - thickness, ui_width + 4, thickness),
        (ui_x - thickness, ui_y + ui_height, ui_width + 4, thickness),
        (ui_x - thickness, ui_y, thickness, ui_height),
        (ui_x + ui_width, ui_y, thickness, ui_height),
    ];
    let css = gtk::CssProvider::new();
    css.load_from_data(
        b"#poratake-scroll-boundary { background: #007aff; } #poratake-scroll-panel { background: #202124; border-radius: 12px; padding: 8px; } #poratake-scroll-panel button { border-radius: 9px; padding: 6px 16px; }",
    )
    .map_err(|error| format!("could not style scroll capture: {error}"))?;
    let mut boundary = Vec::new();
    for (x, y, width, height) in edges {
        let edge = window(x, y, width, height, "Poratake Scroll Capture Boundary");
        let area = gtk::EventBox::new();
        area.set_widget_name("poratake-scroll-boundary");
        area.style_context()
            .add_provider(&css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        edge.add(&area);
        edge.show_all();
        boundary.push(edge);
    }

    let panel = window(0, 0, 280, 52, "Poratake Scroll Capture Controls");
    panel.set_widget_name("poratake-scroll-panel");
    panel
        .style_context()
        .add_provider(&css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    controls.set_halign(gtk::Align::Center);
    controls.set_valign(gtk::Align::Center);
    let auto = gtk::Button::with_label("Auto");
    let done = gtk::Button::with_label("Done");
    let cancel = gtk::Button::with_label("Cancel");
    controls.pack_start(&auto, true, true, 0);
    controls.pack_start(&done, true, true, 0);
    controls.pack_start(&cancel, true, true, 0);
    panel.add(&controls);
    let panel_x = ui_x + (ui_width - 280) / 2;
    let panel_y = if ui_y >= 64 {
        ui_y - 64
    } else {
        ui_y + ui_height + 12
    };
    panel.move_(panel_x, panel_y);

    let auto_state = state.clone();
    auto.connect_clicked(move |_| {
        if auto_state.borrow().source.is_some() {
            auto_state.borrow_mut().stop_auto();
        } else if let Err((_, error)) = start_auto(&auto_state) {
            send_event(
                SCROLL_CAPTURE_SCROLL_ENDED_EVENT,
                Some(json!({ "reason": "capture-error", "message": error })),
            );
        }
    });
    let done_state = state.clone();
    done.connect_clicked(move |_| done_capture(&done_state));
    let cancel_state = state.clone();
    cancel.connect_clicked(move |_| cancel_capture(&cancel_state, true));
    panel.show_all();

    let mut state = state.borrow_mut();
    state.boundary = boundary;
    state.panel = Some(panel);
    state.auto_button = Some(auto);
    Ok(())
}

fn start_auto(state: &Rc<RefCell<ScrollState>>) -> Result<(), (String, String)> {
    if state.borrow().source.is_some() {
        return Ok(());
    }
    let (rect, interval) = {
        let mut state = state.borrow_mut();
        let request = state
            .request
            .clone()
            .ok_or_else(|| ("NOT_CAPTURING".into(), "Not in capture mode".into()))?;
        let plan = scroll_plan(
            request.capture.rect.height as f64 / request.capture.scale_factor,
            request.auto_scroll_speed,
        );
        state.scroll_step_points =
            (plan.target_logical_points as f64 * request.capture.scale_factor).round() as usize;
        state.scroll_steps_per_frame = plan.wheel_detents;
        state.current_scroll_step = 0;
        state.capture_on_next_tick = false;
        match state
            .capture()
            .map_err(|error| ("CAPTURE_FAILED".into(), error))?
        {
            CaptureOutcome::Added | CaptureOutcome::Repeated => {}
            CaptureOutcome::Ended => {
                return Err(("SCROLL_ENDED".into(), "Scrollable content has ended".into()));
            }
            CaptureOutcome::NoOverlap => {
                return Err((
                    "NO_OVERLAP".into(),
                    "The captured frame does not overlap the previous frame".into(),
                ));
            }
        }
        if let Some(button) = state.auto_button.as_ref() {
            button.set_label("Stop");
        }
        (request.capture.rect, plan.interval_millis)
    };
    state
        .borrow()
        .input
        .move_to(
            i16::try_from(rect.x + rect.width / 2).map_err(|_| {
                (
                    "CAPTURE_FAILED".into(),
                    "capture center is outside X11".into(),
                )
            })?,
            i16::try_from(rect.y + rect.height / 2).map_err(|_| {
                (
                    "CAPTURE_FAILED".into(),
                    "capture center is outside X11".into(),
                )
            })?,
        )
        .map_err(|error| ("CAPTURE_FAILED".into(), error))?;

    let tick_state = state.clone();
    let source = glib::timeout_add_local(Duration::from_millis(interval), move || {
        let mut state = tick_state.borrow_mut();
        let Some(request) = state.request.clone() else {
            state.source = None;
            return ControlFlow::Break;
        };
        let rect = request.capture.rect;
        let mut ended = None;
        if state.capture_on_next_tick {
            state.capture_on_next_tick = false;
            ended = match state.capture() {
                Ok(CaptureOutcome::Added) => {
                    let estimated_height = (state.accumulator.estimated_height() as f64
                        / request.capture.scale_factor)
                        .ceil() as usize;
                    if estimated_height >= request.normalized_max_height() {
                        Some(json!({
                            "reason": "max-height",
                            "frameCount": state.accumulator.frames().len(),
                            "estimatedHeight": estimated_height,
                        }))
                    } else {
                        send_event(
                            SCROLL_CAPTURE_FRAME_EVENT,
                            Some(json!({
                                "frameCount": state.accumulator.frames().len(),
                                "estimatedHeight": estimated_height,
                            })),
                        );
                        None
                    }
                }
                Ok(CaptureOutcome::Repeated) => None,
                Ok(CaptureOutcome::Ended) => Some(
                    json!({ "reason": "duplicate", "frameCount": state.accumulator.frames().len() }),
                ),
                Ok(CaptureOutcome::NoOverlap) => Some(
                    json!({ "reason": "no-overlap", "frameCount": state.accumulator.frames().len() }),
                ),
                Err(error) => Some(json!({
                    "reason": "capture-error",
                    "message": error,
                    "frameCount": state.accumulator.frames().len(),
                })),
            };
        } else if state.input.pointer_inside(rect) {
            if let Err(error) = state.input.scroll_down() {
                ended = Some(json!({
                    "reason": "input-error",
                    "message": error,
                    "frameCount": state.accumulator.frames().len(),
                }));
            } else {
                state.current_scroll_step += 1;
                if state.current_scroll_step >= state.scroll_steps_per_frame {
                    state.current_scroll_step = 0;
                    state.capture_on_next_tick = true;
                }
            }
        }
        if let Some(event) = ended {
            state.source = None;
            state.current_scroll_step = 0;
            state.capture_on_next_tick = false;
            if let Some(button) = state.auto_button.as_ref() {
                button.set_label("Auto");
            }
            send_event(SCROLL_CAPTURE_SCROLL_ENDED_EVENT, Some(event));
            return ControlFlow::Break;
        }
        ControlFlow::Continue
    });
    state.borrow_mut().source = Some(source);
    Ok(())
}

fn cancel_capture(state: &Rc<RefCell<ScrollState>>, emit: bool) {
    let active = state.borrow().request.is_some();
    state.borrow_mut().reset();
    if emit && active {
        send_event(SCROLL_CAPTURE_CANCELLED_EVENT, None);
    }
}

fn done_capture(state: &Rc<RefCell<ScrollState>>) {
    if state.borrow().request.is_none() {
        return;
    }
    let mut state = state.borrow_mut();
    state.stop_auto();
    state.hide_ui();
    send_event(SCROLL_CAPTURE_DONE_EVENT, None);
}

fn handle_command(state: &Rc<RefCell<ScrollState>>, command: ScrollCommand) {
    match command {
        ScrollCommand::Start { id, request } => {
            if state.borrow().request.is_some() {
                send_response(Response::error(
                    &id,
                    "ALREADY_CAPTURING",
                    "Scroll capture is already active",
                ));
                return;
            }
            let native_controls = request.native_controls.unwrap_or(false);
            state.borrow_mut().request = Some(request);
            let shown = if native_controls {
                show_ui(state)
            } else {
                Ok(())
            };
            match shown {
                Ok(()) => send_response(Response::success(&id, Some(json!({ "started": true })))),
                Err(error) => {
                    state.borrow_mut().reset();
                    send_response(Response::error(&id, "UI_ERROR", &error));
                }
            }
        }
        ScrollCommand::StartAuto { id, request } => {
            if let Some(speed) = request.speed
                && let Some(active) = state.borrow_mut().request.as_mut()
            {
                active.auto_scroll_speed = speed;
            }
            match start_auto(state) {
                Ok(()) => send_response(Response::success(
                    &id,
                    Some(json!({ "autoScrolling": true })),
                )),
                Err((code, error)) => send_response(Response::error(&id, &code, &error)),
            }
        }
        ScrollCommand::StopAuto { id } => {
            state.borrow_mut().stop_auto();
            send_response(Response::success(
                &id,
                Some(json!({ "autoScrolling": false })),
            ));
        }
        ScrollCommand::Finish { id, path } => {
            let frames = {
                let mut state = state.borrow_mut();
                if state.request.is_none() {
                    send_response(Response::error(&id, "NOT_CAPTURING", "Not in capture mode"));
                    return;
                }
                let frames = state.accumulator.take_frames();
                state.reset();
                frames
            };
            std::thread::spawn(move || match write_png(&path, &frames) {
                Ok((width, height)) => {
                    let result = ScrollCaptureFinishResult {
                        success: true,
                        output_path: path,
                        width,
                        height,
                        frame_count: frames.len(),
                    };
                    match serde_json::to_value(result) {
                        Ok(result) => {
                            send_response(Response::success(&id, Some(result)));
                        }
                        Err(error) => {
                            send_response(Response::error(&id, "STITCH_ERROR", &error.to_string()));
                        }
                    }
                }
                Err(error) => {
                    send_response(Response::error(&id, "STITCH_ERROR", &error));
                }
            });
        }
        ScrollCommand::Cancel { id } => {
            cancel_capture(state, true);
            send_response(Response::success(&id, Some(json!({ "cancelled": true }))));
        }
        ScrollCommand::HotkeyCancel => cancel_capture(state, true),
        ScrollCommand::HotkeyDone => done_capture(state),
    }
}

#[expect(
    deprecated,
    reason = "GLib channels provide an event-driven GTK source"
)]
fn start_ui(runtime: &GtkRuntime) -> Result<glib::Sender<ScrollCommand>, String> {
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
    runtime.dispatch(move || {
        let input = match X11Input::connect() {
            Ok(input) => input,
            Err(error) => {
                let _ = ready_tx.send(Err(error));
                return;
            }
        };
        let (sender, receiver) =
            glib::MainContext::channel::<ScrollCommand>(glib::Priority::default());
        if let Ok(mut active_sender) = COMMAND_SENDER.get_or_init(Default::default).lock() {
            *active_sender = Some(sender.clone());
        }
        if ready_tx.send(Ok(sender.clone())).is_err() {
            return;
        }
        let state = Rc::new(RefCell::new(ScrollState::new(input)));
        receiver.attach(None, move |command| {
            handle_command(&state, command);
            ControlFlow::Continue
        });
    })?;
    ready_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "scroll capture UI initialization timed out".to_string())?
}

pub struct ScrollCaptureModule {
    runtime: GtkRuntime,
    sender: Option<glib::Sender<ScrollCommand>>,
}

impl ScrollCaptureModule {
    pub fn new(runtime: GtkRuntime) -> Self {
        Self {
            runtime,
            sender: None,
        }
    }

    fn send(&mut self, command: ScrollCommand) -> Reply {
        if self.sender.is_none() {
            let sender = match start_ui(&self.runtime) {
                Ok(sender) => sender,
                Err(error) => return Reply::Now(Err(("UI_ERROR".into(), error))),
            };
            self.sender = Some(sender);
        }
        match self
            .sender
            .as_ref()
            .and_then(|sender| sender.send(command).ok())
        {
            Some(()) => Reply::Deferred,
            None => Reply::Now(Err(("UI_ERROR".into(), "scroll capture UI stopped".into()))),
        }
    }
}

impl Module for ScrollCaptureModule {
    fn name(&self) -> &'static str {
        SCROLL_CAPTURE_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match ScrollCaptureMethod::parse(&request.method) {
            Some(ScrollCaptureMethod::Start) => {
                let request_params: ScrollCaptureStartRequest = match params(request) {
                    Ok(request_params) => request_params,
                    Err(error) => return Reply::Now(Err(error)),
                };
                if let Err(error) = request_params.validate() {
                    return Reply::Now(Err(("INVALID_PARAMS".into(), error)));
                }
                if self.runtime.backend() != LinuxBackend::X11 {
                    return Reply::Now(Err((
                        "UNSUPPORTED_SESSION".into(),
                        "scroll capture is only available in X11 sessions".into(),
                    )));
                }
                self.send(ScrollCommand::Start {
                    id: request.id.clone(),
                    request: request_params,
                })
            }
            Some(ScrollCaptureMethod::StartAutoScroll) => {
                let request_params: ScrollCaptureAutoScrollRequest = match params(request) {
                    Ok(request_params) => request_params,
                    Err(error) => return Reply::Now(Err(error)),
                };
                self.send(ScrollCommand::StartAuto {
                    id: request.id.clone(),
                    request: request_params,
                })
            }
            Some(ScrollCaptureMethod::StopAutoScroll) => self.send(ScrollCommand::StopAuto {
                id: request.id.clone(),
            }),
            Some(ScrollCaptureMethod::Finish) => {
                let request_params: ScrollCaptureFinishRequest = match params(request) {
                    Ok(request_params) => request_params,
                    Err(error) => return Reply::Now(Err(error)),
                };
                self.send(ScrollCommand::Finish {
                    id: request.id.clone(),
                    path: request_params.output_path,
                })
            }
            Some(ScrollCaptureMethod::Cancel) => self.send(ScrollCommand::Cancel {
                id: request.id.clone(),
            }),
            None => method_not_found(&request.method),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module() -> ScrollCaptureModule {
        ScrollCaptureModule::new(GtkRuntime::new(LinuxBackend::Headless))
    }

    #[test]
    fn rejects_invalid_geometry_before_starting_gtk() {
        let mut module = module();
        let request = Request {
            id: "invalid".into(),
            module: SCROLL_CAPTURE_MODULE.into(),
            method: ScrollCaptureMethod::Start.id().into(),
            params: serde_json::from_value(json!({
                "x": 0,
                "y": 0,
                "width": 0,
                "height": 100,
                "scaleFactor": 1.0,
                "autoScrollSpeed": "medium",
                "maxHeight": 20000
            }))
            .expect("scroll params"),
        };
        let Reply::Now(result) = module.handle(&request) else {
            panic!("invalid geometry should respond immediately");
        };
        assert_eq!(result.expect_err("invalid geometry").0, "INVALID_PARAMS");
    }

    #[test]
    fn headless_session_rejects_scroll_capture() {
        let mut module = module();
        let request = Request {
            id: "headless".into(),
            module: SCROLL_CAPTURE_MODULE.into(),
            method: ScrollCaptureMethod::Start.id().into(),
            params: serde_json::from_value(json!({
                "x": 0,
                "y": 0,
                "width": 100,
                "height": 100,
                "scaleFactor": 1.0,
                "autoScrollSpeed": "medium",
                "maxHeight": 20000
            }))
            .expect("scroll params"),
        };
        let Reply::Now(result) = module.handle(&request) else {
            panic!("headless session should respond immediately");
        };
        assert_eq!(
            result.expect_err("headless scroll").0,
            "UNSUPPORTED_SESSION"
        );
    }
}
