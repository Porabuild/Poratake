use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use super::{NativeCommand, NativeEvent, Shell, TrayMenuState};
use crate::system::hotkeys::HotkeyRegistry;
use crate::system::tray::Intent;

pub(super) struct Driver {
    commands: Sender<NativeCommand>,
    thread_id: u32,
}

impl Driver {
    pub(super) fn spawn(
        state: TrayMenuState,
        hotkeys: Vec<(Intent, String)>,
        events: smol::channel::Sender<NativeEvent>,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (id_tx, id_rx) = mpsc::channel();

        thread::Builder::new()
            .name("native-shell".into())
            .spawn(move || run(state, hotkeys, command_rx, events, id_tx))
            .ok();

        Self {
            commands: command_tx,
            thread_id: id_rx.recv_timeout(Duration::from_secs(5)).unwrap_or(0),
        }
    }

    pub(super) fn send(&self, command: NativeCommand) {
        if self.commands.send(command).is_err() {
            return;
        }
        wake(self.thread_id);
    }
}

impl Shell {
    fn drain_native_queues(&self) {
        while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
            if let tray_icon::TrayIconEvent::Click {
                rect,
                button: tray_icon::MouseButton::Left | tray_icon::MouseButton::Right,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = event
            {
                self.emit_tray_menu(rect.into());
            }
        }
        while let Ok(event) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
            if event.state() != global_hotkey::HotKeyState::Pressed {
                continue;
            }
            self.emit_hotkey(event.id());
        }
    }
}

fn run(
    state: TrayMenuState,
    hotkey_bindings: Vec<(Intent, String)>,
    commands: Receiver<NativeCommand>,
    events: smol::channel::Sender<NativeEvent>,
    id_tx: Sender<u32>,
) {
    create_message_queue();
    let _ = id_tx.send(current_thread_id());

    let mut hotkeys = HotkeyRegistry::new();
    hotkeys.apply(&hotkey_bindings);

    let mut shell = Shell {
        tray: super::create_tray(state.dark_mode),
        hotkeys,
        events,
    };

    pump(&mut shell, &commands);
}

fn current_thread_id() -> u32 {
    unsafe { ::windows::Win32::System::Threading::GetCurrentThreadId() }
}

fn create_message_queue() {
    use ::windows::Win32::UI::WindowsAndMessaging::{PeekMessageW, MSG, PM_NOREMOVE};

    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
}

fn wake(thread_id: u32) {
    use ::windows::Win32::Foundation::{LPARAM, WPARAM};
    use ::windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};

    if thread_id == 0 {
        return;
    }
    if unsafe { PostThreadMessageW(thread_id, WM_APP, WPARAM(0), LPARAM(0)) }.is_err() {
        eprintln!("[native] failed to wake the native event thread");
    }
}

fn pump(shell: &mut Shell, commands: &Receiver<NativeCommand>) {
    use ::windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, TranslateMessage, MSG,
    };

    let mut message = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if result.0 == 0 {
            break;
        }
        if result.0 != -1 {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        shell.drain_native_queues();
        while let Ok(command) = commands.try_recv() {
            shell.apply(command);
        }
    }
}
