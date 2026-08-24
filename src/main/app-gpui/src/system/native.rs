use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crate::system::hotkeys::HotkeyRegistry;
use crate::system::tray::{self, Intent, TrayMenuState};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TrayRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeEvent {
    pub intent: Intent,
    pub tray_rect: Option<TrayRect>,
}

pub enum NativeCommand {
    RebuildMenu(TrayMenuState),
    SetTrayVisible(bool),
    SetHotkeys(Vec<(Intent, String)>),
}

pub struct NativeBridge {
    commands: Sender<NativeCommand>,
    events: smol::channel::Receiver<NativeEvent>,
    thread_id: u32,
}

impl NativeBridge {
    pub fn events(&self) -> smol::channel::Receiver<NativeEvent> {
        self.events.clone()
    }

    pub fn send(&self, command: NativeCommand) {
        if self.commands.send(command).is_err() {
            return;
        }
        wake(self.thread_id);
    }
}

pub fn spawn(state: TrayMenuState, hotkeys: Vec<(Intent, String)>) -> NativeBridge {
    let (command_tx, command_rx) = mpsc::channel();
    let (event_tx, event_rx) = smol::channel::unbounded();
    let (id_tx, id_rx) = mpsc::channel();

    thread::Builder::new()
        .name("native-shell".into())
        .spawn(move || run(state, hotkeys, command_rx, event_tx, id_tx))
        .ok();

    let thread_id = id_rx.recv_timeout(Duration::from_secs(5)).unwrap_or(0);
    NativeBridge {
        commands: command_tx,
        events: event_rx,
        thread_id,
    }
}

struct Shell {
    tray: Option<tray_icon::TrayIcon>,
    hotkeys: HotkeyRegistry,
    events: smol::channel::Sender<NativeEvent>,
}

impl Shell {
    fn tray_rect(&self) -> Option<TrayRect> {
        let rect = self.tray.as_ref()?.rect()?;
        Some(TrayRect {
            x: rect.position.x as f32,
            y: rect.position.y as f32,
            width: rect.size.width as f32,
            height: rect.size.height as f32,
        })
    }

    fn emit(&self, intent: Intent) {
        let event = NativeEvent {
            intent,
            tray_rect: self.tray_rect(),
        };
        if self.events.send_blocking(event).is_err() {
            eprintln!("[native] event channel closed");
        }
    }

    fn drain_native_queues(&self) {
        while let Ok(event) = muda::MenuEvent::receiver().try_recv() {
            match Intent::from_id(event.id.as_ref()) {
                Some(intent) => self.emit(intent),
                None => eprintln!("[tray] unknown menu id {:?}", event.id),
            }
        }
        while let Ok(event) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
            if event.state() != global_hotkey::HotKeyState::Pressed {
                continue;
            }
            match self.hotkeys.intent_for(event.id()) {
                Some(intent) => self.emit(intent),
                None => eprintln!("[hotkey] unmapped id {}", event.id()),
            }
        }
    }

    fn apply(&mut self, command: NativeCommand) {
        match command {
            NativeCommand::RebuildMenu(state) => self.rebuild_menu(&state),
            NativeCommand::SetTrayVisible(visible) => self.set_tray_visible(visible),
            NativeCommand::SetHotkeys(bindings) => self.hotkeys.apply(&bindings),
        }
    }

    fn rebuild_menu(&mut self, state: &TrayMenuState) {
        let menu = match tray::build(state) {
            Ok(menu) => menu,
            Err(error) => {
                eprintln!("[tray] menu build failed: {error}");
                return;
            }
        };
        match &self.tray {
            Some(tray) => tray.set_menu(Some(Box::new(menu))),
            None => self.tray = create_tray(menu),
        }
    }

    fn set_tray_visible(&mut self, visible: bool) {
        let Some(tray) = &self.tray else {
            return;
        };
        if let Err(error) = tray.set_visible(visible) {
            eprintln!("[tray] visibility change failed: {error}");
        }
        if !visible {
            self.tray = None;
        }
    }
}

fn create_tray(menu: muda::Menu) -> Option<tray_icon::TrayIcon> {
    let mut builder = tray_icon::TrayIconBuilder::new()
        .with_tooltip("Poratake")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(true);
    if let Some(icon) = tray::tray_icon() {
        builder = builder.with_icon(icon);
    }
    match builder.build() {
        Ok(tray) => Some(tray),
        Err(error) => {
            eprintln!("[tray] creation failed: {error}");
            None
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

    let tray = tray::build(&state)
        .map_err(|error| eprintln!("[tray] menu build failed: {error}"))
        .ok()
        .and_then(create_tray);

    let mut shell = Shell {
        tray,
        hotkeys,
        events,
    };

    pump(&mut shell, &commands);
}

#[cfg(windows)]
fn current_thread_id() -> u32 {
    unsafe { ::windows::Win32::System::Threading::GetCurrentThreadId() }
}

#[cfg(windows)]
fn create_message_queue() {
    use ::windows::Win32::UI::WindowsAndMessaging::{PeekMessageW, MSG, PM_NOREMOVE};

    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
}

#[cfg(not(windows))]
fn create_message_queue() {}

#[cfg(not(windows))]
fn current_thread_id() -> u32 {
    0
}

#[cfg(windows)]
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

#[cfg(not(windows))]
fn wake(_thread_id: u32) {}

#[cfg(windows)]
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

#[cfg(not(windows))]
fn pump(shell: &mut Shell, commands: &Receiver<NativeCommand>) {
    while let Ok(command) = commands.recv() {
        shell.apply(command);
        shell.drain_native_queues();
    }
}
