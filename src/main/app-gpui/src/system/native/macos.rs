use super::{NativeCommand, NativeEvent, Shell, TrayMenuState, TrayRect};
use crate::system::hotkeys::HotkeyRegistry;
use crate::system::tray::Intent;

pub(super) struct Driver {
    shell: parking_lot::Mutex<Shell>,
}

impl Driver {
    pub(super) fn new(
        state: TrayMenuState,
        hotkey_bindings: Vec<(Intent, String)>,
        events: smol::channel::Sender<NativeEvent>,
    ) -> Self {
        install_event_handlers(events.clone());

        let mut hotkeys = HotkeyRegistry::new();
        hotkeys.apply(&hotkey_bindings);

        Self {
            shell: parking_lot::Mutex::new(Shell {
                tray: super::create_tray(state.dark_mode),
                hotkeys,
                events,
            }),
        }
    }

    pub(super) fn send(&self, command: NativeCommand) {
        self.shell.lock().apply(command);
    }

    pub(super) fn resolve_hotkey(&self, id: u32) -> Option<NativeEvent> {
        self.shell.lock().hotkey_event(id)
    }
}

pub(super) fn configure_app() {
    unsafe {
        let application_class = objc_getClass(c"NSApplication".as_ptr());
        if application_class.is_null() {
            return;
        }
        let shared_application: unsafe extern "C" fn(
            *mut std::ffi::c_void,
            *const std::ffi::c_void,
        ) -> *mut std::ffi::c_void = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let application = shared_application(
            application_class,
            sel_registerName(c"sharedApplication".as_ptr()),
        );
        if application.is_null() {
            return;
        }
        let set_policy: unsafe extern "C" fn(
            *mut std::ffi::c_void,
            *const std::ffi::c_void,
            isize,
        ) -> bool = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let _ = set_policy(
            application,
            sel_registerName(c"setActivationPolicy:".as_ptr()),
            1,
        );
    }
}

fn install_event_handlers(events: smol::channel::Sender<NativeEvent>) {
    let tray_events = events.clone();
    tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
        if let tray_icon::TrayIconEvent::Click {
            rect,
            button: tray_icon::MouseButton::Left | tray_icon::MouseButton::Right,
            button_state: tray_icon::MouseButtonState::Up,
            ..
        } = event
        {
            let tray_rect = Some(TrayRect::from(rect));
            let _ = tray_events.send_blocking(NativeEvent::ToggleTrayMenu { tray_rect });
        }
    }));
    global_hotkey::GlobalHotKeyEvent::set_event_handler(Some(
        move |event: global_hotkey::GlobalHotKeyEvent| {
            if event.state() == global_hotkey::HotKeyState::Pressed {
                let _ = events.send_blocking(NativeEvent::Hotkey { id: event.id() });
            }
        },
    ));
}

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
    fn sel_registerName(name: *const std::ffi::c_char) -> *const std::ffi::c_void;
    fn objc_msgSend();
}
