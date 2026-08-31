use crate::system::hotkeys::HotkeyRegistry;
use crate::system::tray::{self, Intent, TrayMenuState};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
pub fn configure_app() {
    macos::configure_app();
}

#[cfg(not(target_os = "macos"))]
pub fn configure_app() {}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TrayRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<tray_icon::Rect> for TrayRect {
    fn from(rect: tray_icon::Rect) -> Self {
        Self {
            x: rect.position.x as f32,
            y: rect.position.y as f32,
            width: rect.size.width as f32,
            height: rect.size.height as f32,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum NativeEvent {
    Intent {
        intent: Intent,
        tray_rect: Option<TrayRect>,
    },
    #[cfg(target_os = "macos")]
    Hotkey {
        id: u32,
    },
    ToggleTrayMenu {
        tray_rect: Option<TrayRect>,
    },
    CancelPreRecording,
}

pub enum NativeCommand {
    RebuildMenu(Box<TrayMenuState>),
    SetTrayVisible(bool),
    SetHotkeys(Vec<(Intent, String)>),
    SetPreRecordingEscape(bool),
}

pub struct NativeBridge {
    #[cfg(windows)]
    driver: windows::Driver,
    #[cfg(target_os = "linux")]
    commands: Option<linux::CommandSender>,
    events: smol::channel::Receiver<NativeEvent>,
    #[cfg(target_os = "macos")]
    driver: macos::Driver,
}

impl NativeBridge {
    pub fn events(&self) -> smol::channel::Receiver<NativeEvent> {
        self.events.clone()
    }

    pub fn send(&self, command: NativeCommand) {
        #[cfg(target_os = "macos")]
        self.driver.send(command);
        #[cfg(windows)]
        self.driver.send(command);
        #[cfg(target_os = "linux")]
        if let Some(commands) = &self.commands {
            commands.send(command);
        }
    }

    #[cfg(target_os = "macos")]
    pub fn resolve_hotkey(&self, id: u32) -> Option<NativeEvent> {
        self.driver.resolve_hotkey(id)
    }
}

#[cfg(windows)]
pub fn spawn(state: TrayMenuState, hotkeys: Vec<(Intent, String)>) -> NativeBridge {
    let (event_tx, event_rx) = smol::channel::unbounded();
    NativeBridge {
        driver: windows::Driver::spawn(state, hotkeys, event_tx),
        events: event_rx,
    }
}

#[cfg(target_os = "linux")]
pub fn spawn(state: TrayMenuState, hotkeys: Vec<(Intent, String)>) -> NativeBridge {
    let (event_tx, event_rx) = smol::channel::unbounded();
    NativeBridge {
        commands: linux::spawn(state, hotkeys, event_tx),
        events: event_rx,
    }
}

#[cfg(target_os = "macos")]
pub fn spawn(state: TrayMenuState, hotkey_bindings: Vec<(Intent, String)>) -> NativeBridge {
    let (event_tx, event_rx) = smol::channel::unbounded();
    NativeBridge {
        events: event_rx,
        driver: macos::Driver::new(state, hotkey_bindings, event_tx),
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

    #[cfg(target_os = "linux")]
    fn emit_intent(&self, intent: Intent) {
        let event = NativeEvent::Intent {
            intent,
            tray_rect: self.tray_rect(),
        };
        if self.events.send_blocking(event).is_err() {
            eprintln!("[native] event channel closed");
        }
    }

    fn emit_tray_menu(&self, tray_rect: TrayRect) {
        if self
            .events
            .send_blocking(NativeEvent::ToggleTrayMenu {
                tray_rect: Some(tray_rect),
            })
            .is_err()
        {
            eprintln!("[native] event channel closed");
        }
    }

    fn hotkey_event(&self, id: u32) -> Option<NativeEvent> {
        if self.hotkeys.is_pre_recording_escape(id) {
            return Some(NativeEvent::CancelPreRecording);
        }
        self.hotkeys
            .intent_for(id)
            .map(|intent| NativeEvent::Intent {
                intent,
                tray_rect: self.tray_rect(),
            })
    }

    fn emit_hotkey(&self, id: u32) {
        let Some(event) = self.hotkey_event(id) else {
            eprintln!("[hotkey] unmapped id {id}");
            return;
        };
        if self.events.send_blocking(event).is_err() {
            eprintln!("[native] event channel closed");
        }
    }

    fn apply(&mut self, command: NativeCommand) {
        match command {
            NativeCommand::RebuildMenu(state) => self.rebuild_menu(&state),
            NativeCommand::SetTrayVisible(visible) => self.set_tray_visible(visible),
            NativeCommand::SetHotkeys(bindings) => self.hotkeys.apply(&bindings),
            NativeCommand::SetPreRecordingEscape(enabled) => {
                self.hotkeys.set_pre_recording_escape(enabled)
            }
        }
    }

    fn rebuild_menu(&mut self, state: &TrayMenuState) {
        match &self.tray {
            Some(tray) => {
                if let Some(icon) = tray::tray_icon(state.dark_mode) {
                    if let Err(error) = tray.set_icon(Some(icon)) {
                        eprintln!("[tray] icon update failed: {error}");
                    }
                }
                #[cfg(target_os = "linux")]
                tray.set_menu(Some(linux::native_menu(state)));
            }
            None => {
                #[cfg(target_os = "linux")]
                {
                    self.tray = create_tray(state.dark_mode, Some(linux::native_menu(state)));
                }
                #[cfg(not(target_os = "linux"))]
                {
                    self.tray = create_tray(state.dark_mode);
                }
            }
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

#[cfg(target_os = "linux")]
fn create_tray(
    dark_mode: bool,
    menu: Option<Box<dyn tray_icon::menu::ContextMenu>>,
) -> Option<tray_icon::TrayIcon> {
    create_tray_builder(dark_mode)
        .with_menu(menu?)
        .build()
        .map_err(|error| {
            eprintln!("[tray] creation failed: {error}");
            error
        })
        .ok()
}

#[cfg(not(target_os = "linux"))]
fn create_tray(dark_mode: bool) -> Option<tray_icon::TrayIcon> {
    build_tray(create_tray_builder(dark_mode))
}

fn create_tray_builder(dark_mode: bool) -> tray_icon::TrayIconBuilder {
    let mut builder = tray_icon::TrayIconBuilder::new()
        .with_tooltip("Poratake")
        .with_menu_on_left_click(false)
        .with_menu_on_right_click(false);
    #[cfg(target_os = "macos")]
    {
        builder = builder.with_icon_as_template(true);
    }
    if let Some(icon) = tray::tray_icon(dark_mode) {
        builder = builder.with_icon(icon);
    }
    builder
}

#[cfg(not(target_os = "linux"))]
fn build_tray(builder: tray_icon::TrayIconBuilder) -> Option<tray_icon::TrayIcon> {
    match builder.build() {
        Ok(tray) => Some(tray),
        Err(error) => {
            eprintln!("[tray] creation failed: {error}");
            None
        }
    }
}
