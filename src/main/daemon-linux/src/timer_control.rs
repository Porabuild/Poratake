use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use glib::{ControlFlow, Propagation};
use global_hotkey::hotkey::{Code, HotKey};
use gtk::gdk;
use gtk::prelude::*;
use poratake_daemon_common::contract::{
    TIMER_CONTROL_CANCEL_EVENT, TIMER_CONTROL_COMPLETED_EVENT, TIMER_CONTROL_HEIGHT,
    TIMER_CONTROL_MODULE, TIMER_CONTROL_WIDTH, TimerControlMethod, TimerShowRequest,
};
use poratake_daemon_common::protocol::{Request, Response, params, send_event, send_response};
use poratake_daemon_common::router::{Module, Reply, method_not_found};
use serde_json::json;

use crate::gtk_runtime::GtkRuntime;
use crate::hotkeys;

static COMMAND_SENDER: std::sync::OnceLock<std::sync::Mutex<Option<glib::Sender<TimerCommand>>>> =
    std::sync::OnceLock::new();

pub(crate) fn cancel_from_global_hotkey() {
    if let Some(sender) = COMMAND_SENDER
        .get()
        .and_then(|sender| sender.lock().ok())
        .and_then(|sender| sender.clone())
    {
        let _ = sender.send(TimerCommand::Cancel);
    }
}

enum TimerCommand {
    Show {
        id: String,
        request: TimerShowRequest,
    },
    Hide {
        id: String,
    },
    Cancel,
}

struct TimerState {
    window: Option<gtk::Window>,
    label: Option<gtk::Label>,
    source: Option<glib::SourceId>,
    remaining: u32,
    hovered: bool,
    escape: HotKey,
    escape_registered: bool,
    use_global_hotkey: bool,
}

impl TimerState {
    fn new(escape: HotKey, use_global_hotkey: bool) -> Self {
        Self {
            window: None,
            label: None,
            source: None,
            remaining: 0,
            hovered: false,
            escape,
            escape_registered: false,
            use_global_hotkey,
        }
    }

    fn hide(&mut self) {
        if let Some(source) = self.source.take() {
            source.remove();
        }
        self.label = None;
        if let Some(window) = self.window.take() {
            window.hide();
            window.close();
        }
        if self.escape_registered {
            hotkeys::unregister(self.escape);
            self.escape_registered = false;
        }
    }
}

fn cancel(state: &Rc<RefCell<TimerState>>) {
    if state.borrow().window.is_none() {
        return;
    }
    state.borrow_mut().hide();
    send_event(TIMER_CONTROL_CANCEL_EVENT, None);
}

fn complete(state: &Rc<RefCell<TimerState>>) {
    state.borrow_mut().hide();
    send_event(TIMER_CONTROL_COMPLETED_EVENT, None);
}

fn show(state: &Rc<RefCell<TimerState>>, request: TimerShowRequest) -> Result<(), String> {
    state.borrow_mut().hide();
    {
        let mut timer = state.borrow_mut();
        if timer.use_global_hotkey {
            hotkeys::register(timer.escape)
                .map_err(|error| format!("could not register Escape: {error}"))?;
            timer.escape_registered = true;
        }
    }
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_accept_focus(false);
    window.set_focus_on_map(false);
    window.set_keep_above(true);
    window.set_skip_pager_hint(true);
    window.set_skip_taskbar_hint(true);
    window.set_type_hint(gdk::WindowTypeHint::Utility);
    window.set_default_size(TIMER_CONTROL_WIDTH, TIMER_CONTROL_HEIGHT);

    let button = gtk::Button::new();
    button.set_widget_name("poratake-timer");
    button.set_relief(gtk::ReliefStyle::None);
    button.set_tooltip_text(Some("Cancel timer capture"));
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.set_halign(gtk::Align::Center);
    content.set_valign(gtk::Align::Center);
    let icon = gtk::Image::from_icon_name(Some("alarm-symbolic"), gtk::IconSize::Button);
    icon.set_pixel_size(18);
    content.pack_start(&icon, false, false, 0);
    let duration = request.normalized_duration() as u32;
    let label = gtk::Label::new(Some(&duration.to_string()));
    label.set_widget_name("poratake-timer-label");
    content.pack_start(&label, false, false, 0);
    button.add(&content);

    let css = gtk::CssProvider::new();
    css.load_from_data(
        format!(
            "button#poratake-timer {{ background-image: linear-gradient(to bottom, {0}, shade({0}, 0.88), {0}); color: {1}; border: 1px solid alpha({1}, 0.25); border-radius: 10px; padding: 0 18px; box-shadow: 0 4px 10px alpha(black, 0.22); }} button#poratake-timer:active {{ background-image: linear-gradient(to bottom, shade({0}, 0.9), shade({0}, 0.8), shade({0}, 0.9)); }} label#poratake-timer-label {{ font-family: monospace; font-size: 24px; font-weight: 700; }} label#poratake-timer-label.cancel {{ font-family: sans-serif; font-size: 16px; font-weight: 600; }}",
            request.color, request.foreground_color
        )
        .as_bytes(),
    )
    .map_err(|error| format!("could not style the timer control: {error}"))?;
    button
        .style_context()
        .add_provider(&css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

    let cancel_state = state.clone();
    button.connect_clicked(move |_| cancel(&cancel_state));
    let hover_state = state.clone();
    let hover_label = label.clone();
    button.connect_enter_notify_event(move |_, _| {
        hover_state.borrow_mut().hovered = true;
        hover_label.style_context().add_class("cancel");
        hover_label.set_text("Cancel");
        Propagation::Proceed
    });
    let leave_state = state.clone();
    let leave_label = label.clone();
    button.connect_leave_notify_event(move |_, _| {
        leave_state.borrow_mut().hovered = false;
        leave_label.style_context().remove_class("cancel");
        leave_label.set_text(&leave_state.borrow().remaining.to_string());
        Propagation::Proceed
    });
    let escape_state = state.clone();
    window.connect_key_press_event(move |_, event| {
        if event.keyval() == gdk::keys::constants::Escape {
            cancel(&escape_state);
            return Propagation::Stop;
        }
        Propagation::Proceed
    });
    let close_state = state.clone();
    window.connect_delete_event(move |_, _| {
        cancel(&close_state);
        Propagation::Stop
    });
    window.add(&button);

    {
        let mut timer = state.borrow_mut();
        timer.window = Some(window.clone());
        timer.label = Some(label);
        timer.remaining = duration;
        timer.hovered = false;
    }
    let tick_state = state.clone();
    let source = glib::timeout_add_local(Duration::from_secs(1), move || {
        let remaining = tick_state.borrow().remaining;
        if remaining <= 1 {
            tick_state.borrow_mut().source = None;
            complete(&tick_state);
            return ControlFlow::Break;
        }
        let mut timer = tick_state.borrow_mut();
        timer.remaining -= 1;
        if !timer.hovered
            && let Some(label) = timer.label.as_ref()
        {
            label.set_text(&timer.remaining.to_string());
        }
        ControlFlow::Continue
    });
    state.borrow_mut().source = Some(source);

    window.move_(request.x, request.y);
    window.show_all();
    Ok(())
}

fn handle_command(state: &Rc<RefCell<TimerState>>, command: TimerCommand) {
    match command {
        TimerCommand::Show { id, request } => match show(state, request) {
            Ok(()) => send_response(Response::success(&id, Some(json!({ "visible": true })))),
            Err(error) => send_response(Response::error(&id, "UI_ERROR", &error)),
        },
        TimerCommand::Hide { id } => {
            state.borrow_mut().hide();
            send_response(Response::success(&id, Some(json!({ "visible": false }))));
        }
        TimerCommand::Cancel => cancel(state),
    }
}

#[expect(
    deprecated,
    reason = "GLib channels provide an event-driven GTK source"
)]
fn start_ui(runtime: &GtkRuntime) -> Result<glib::Sender<TimerCommand>, String> {
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
    let backend = runtime.backend();
    runtime.dispatch(move || {
        let (sender, receiver) =
            glib::MainContext::channel::<TimerCommand>(glib::Priority::default());
        if let Ok(mut active_sender) = COMMAND_SENDER.get_or_init(Default::default).lock() {
            *active_sender = Some(sender.clone());
        }
        let escape = HotKey::new(None, Code::Escape);
        if ready_tx.send(Ok(sender.clone())).is_err() {
            return;
        }
        crate::scroll_capture::install_global_hotkey_handler();
        let state = Rc::new(RefCell::new(TimerState::new(
            escape,
            backend == poratake_daemon_common::platform::LinuxBackend::X11,
        )));
        receiver.attach(None, move |command| {
            handle_command(&state, command);
            ControlFlow::Continue
        });
    })?;
    ready_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "timer UI initialization timed out".to_string())?
}

pub struct TimerControlModule {
    runtime: GtkRuntime,
    sender: Option<glib::Sender<TimerCommand>>,
}

impl TimerControlModule {
    pub fn new(runtime: GtkRuntime) -> Self {
        Self {
            runtime,
            sender: None,
        }
    }

    fn send(&mut self, command: TimerCommand) -> Reply {
        if self.sender.is_none() {
            let sender = match start_ui(&self.runtime) {
                Ok(sender) => sender,
                Err(error) => return Reply::Now(Err(("UI_ERROR".into(), error))),
            };
            self.sender = Some(sender);
        }
        let sender = self.sender.as_ref().expect("timer sender initialized");
        match sender.send(command) {
            Ok(()) => Reply::Deferred,
            Err(_) => Reply::Now(Err(("UI_ERROR".into(), "timer UI stopped".into()))),
        }
    }
}

impl Module for TimerControlModule {
    fn name(&self) -> &'static str {
        TIMER_CONTROL_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match TimerControlMethod::parse(&request.method) {
            Some(TimerControlMethod::Show) => {
                let mut params: TimerShowRequest = match params(request) {
                    Ok(params) => params,
                    Err(error) => return Reply::Now(Err(error)),
                };
                let (color, foreground_color) = match params.normalized_theme_colors() {
                    Ok(colors) => colors,
                    Err(error) => return Reply::Now(Err(("INVALID_PARAMS".into(), error))),
                };
                params.color = color;
                params.foreground_color = foreground_color;
                self.send(TimerCommand::Show {
                    id: request.id.clone(),
                    request: params,
                })
            }
            Some(TimerControlMethod::Hide) => {
                if self.sender.is_none() {
                    return Reply::Now(Ok(Some(json!({ "visible": false }))));
                }
                self.send(TimerCommand::Hide {
                    id: request.id.clone(),
                })
            }
            None => method_not_found(&request.method),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module() -> TimerControlModule {
        TimerControlModule::new(GtkRuntime::new(
            poratake_daemon_common::platform::LinuxBackend::Headless,
        ))
    }

    #[test]
    fn rejects_unknown_methods_without_starting_ui_work() {
        let mut module = module();
        let request = Request {
            id: "unknown".into(),
            module: TIMER_CONTROL_MODULE.into(),
            method: "unknown".into(),
            params: None,
        };
        let Reply::Now(result) = module.handle(&request) else {
            panic!("unknown method should respond immediately");
        };
        assert_eq!(result.expect_err("unknown method").0, "METHOD_NOT_FOUND");
    }

    #[test]
    fn rejects_non_hex_theme_colors_before_starting_gtk() {
        for (color, foreground_color) in [("red", "#ffffff"), ("#000000", "rgb(1,2,3)")] {
            let mut module = module();
            let request = Request {
                id: "invalid-color".into(),
                module: TIMER_CONTROL_MODULE.into(),
                method: "show".into(),
                params: serde_json::from_value(serde_json::json!({
                    "x": 10,
                    "y": 20,
                    "duration": 5,
                    "color": color,
                    "foregroundColor": foreground_color
                }))
                .expect("timer params"),
            };
            let Reply::Now(result) = module.handle(&request) else {
                panic!("invalid colors should respond immediately");
            };
            assert_eq!(result.expect_err("invalid color").0, "INVALID_PARAMS");
        }
    }
}
