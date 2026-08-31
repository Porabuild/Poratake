use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use glib::{MainContext, MainLoop};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};

use super::{NativeCommand, NativeEvent, Shell, TrayMenuState, TrayRect};
use crate::system::tray::{native_entries, Intent, NativeMenuEntry};

enum DriverMessage {
    Command(NativeCommand),
    Hotkey(u32),
    Intent(Intent),
    TrayMenu(TrayRect),
}

pub(super) struct CommandSender(smol::channel::Sender<DriverMessage>);

impl CommandSender {
    pub(super) fn send(&self, command: NativeCommand) {
        if self
            .0
            .send_blocking(DriverMessage::Command(command))
            .is_err()
        {
            eprintln!("[native] Linux command channel closed");
        }
    }
}

pub(super) fn spawn(
    state: TrayMenuState,
    hotkeys: Vec<(Intent, String)>,
    events: smol::channel::Sender<NativeEvent>,
) -> Option<CommandSender> {
    let (ready_tx, ready_rx) = mpsc::channel();
    thread::Builder::new()
        .name("native-shell".into())
        .spawn(move || run(state, hotkeys, events, ready_tx))
        .ok()?;
    ready_rx.recv_timeout(Duration::from_secs(5)).ok()
}

fn run(
    state: TrayMenuState,
    hotkey_bindings: Vec<(Intent, String)>,
    events: smol::channel::Sender<NativeEvent>,
    ready: mpsc::Sender<CommandSender>,
) {
    let context = MainContext::new();
    let main_loop = MainLoop::new(Some(&context), false);
    let _ = context.with_thread_default(|| {
        if gtk::init().is_err() {
            eprintln!("[native] GTK initialization failed");
            return;
        }

        let (sender, receiver) = smol::channel::unbounded();
        if ready.send(CommandSender(sender.clone())).is_err() {
            return;
        }

        let tray_messages = sender.clone();
        let menu_messages = sender.clone();
        tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
            if let tray_icon::TrayIconEvent::Click {
                rect,
                button: tray_icon::MouseButton::Left | tray_icon::MouseButton::Right,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = event
            {
                let _ = tray_messages.send_blocking(DriverMessage::TrayMenu(rect.into()));
            }
        }));
        global_hotkey::GlobalHotKeyEvent::set_event_handler(Some(
            move |event: global_hotkey::GlobalHotKeyEvent| {
                if event.state() == global_hotkey::HotKeyState::Pressed {
                    let _ = sender.send_blocking(DriverMessage::Hotkey(event.id()));
                }
            },
        ));
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let Some(intent) = Intent::from_id(event.id().as_ref()) else {
                return;
            };
            let _ = menu_messages.send_blocking(DriverMessage::Intent(intent));
        }));

        let mut hotkeys = crate::system::hotkeys::HotkeyRegistry::new();
        hotkeys.apply(&hotkey_bindings);
        let mut shell = Shell {
            tray: super::create_tray(state.dark_mode, Some(native_menu(&state))),
            hotkeys,
            events,
        };

        context.spawn_local(async move {
            while let Ok(message) = receiver.recv().await {
                match message {
                    DriverMessage::Command(command) => shell.apply(command),
                    DriverMessage::Hotkey(id) => shell.emit_hotkey(id),
                    DriverMessage::Intent(intent) => shell.emit_intent(intent),
                    DriverMessage::TrayMenu(rect) => shell.emit_tray_menu(rect),
                }
            }
        });
        main_loop.run();
    });
}

pub(super) fn native_menu(state: &TrayMenuState) -> Box<Menu> {
    let native = Menu::new();
    for entry in native_entries(state) {
        match entry {
            NativeMenuEntry::Item {
                intent,
                label,
                enabled,
            } => {
                let item = MenuItem::with_id(intent.id(), label, enabled, None);
                if let Err(error) = native.append(&item) {
                    eprintln!("[tray] menu item creation failed: {error}");
                }
            }
            NativeMenuEntry::Separator => {
                if let Err(error) = native.append(&PredefinedMenuItem::separator()) {
                    eprintln!("[tray] menu separator creation failed: {error}");
                }
            }
        }
    }
    Box::new(native)
}
