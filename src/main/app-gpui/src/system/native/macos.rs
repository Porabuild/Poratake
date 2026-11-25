use std::collections::HashSet;

use herogpui::gpui;

use super::{NativeCommand, NativeEvent, Shell, TrayMenuState, TrayRect};
use crate::system::hotkeys::HotkeyRegistry;
use crate::system::tray::Intent;

#[derive(Default)]
struct DockRegistry {
    windows: HashSet<gpui::WindowId>,
}

impl DockRegistry {
    fn insert(&mut self, id: gpui::WindowId) -> bool {
        let was_empty = self.windows.is_empty();
        self.windows.insert(id) && was_empty
    }

    fn remove(&mut self, id: gpui::WindowId) -> bool {
        self.windows.remove(&id) && self.windows.is_empty()
    }
}

#[derive(Default)]
struct DockState {
    registry: DockRegistry,
    subscription: Option<gpui::Subscription>,
}

impl gpui::Global for DockState {}

pub(super) struct Driver {
    shell: parking_lot::Mutex<Shell>,
}

impl Driver {
    pub(super) fn new(
        state: TrayMenuState,
        hotkey_bindings: Vec<(Intent, String)>,
        events: smol::channel::Sender<NativeEvent>,
    ) -> Self {
        install_event_handlers(events);

        let mut hotkeys = HotkeyRegistry::new();
        hotkeys.apply(&hotkey_bindings);

        Self {
            shell: parking_lot::Mutex::new(Shell {
                tray: super::create_tray(state.dark_mode, state.is_recording),
                hotkeys,
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

pub(super) fn configure_app(cx: &mut gpui::App) {
    cx.set_activation_policy(gpui::ActivationPolicy::Accessory);
    cx.set_cursor_hide_mode(gpui::CursorHideMode::OnTyping);
}

pub(super) fn register_dock_window(window_id: gpui::WindowId, cx: &mut gpui::App) {
    if crate::state::try_native(cx).is_none() {
        return;
    }
    if cx.try_global::<DockState>().is_none() {
        let subscription = cx.on_window_closed(|cx, closed| {
            if cx.try_global::<DockState>().is_none() {
                return;
            }
            if cx.global_mut::<DockState>().registry.remove(closed) {
                cx.set_activation_policy(gpui::ActivationPolicy::Accessory);
            }
        });
        cx.set_global(DockState {
            registry: DockRegistry::default(),
            subscription: Some(subscription),
        });
    }
    let show_dock = {
        let state = cx.global_mut::<DockState>();
        let _ = &state.subscription;
        state.registry.insert(window_id)
    };
    if show_dock {
        cx.set_activation_policy(gpui::ActivationPolicy::Regular);
        cx.activate(true);
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
            let tray_rect = Some(logical_tray_rect(rect, screen_scale_at_pointer()));
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

pub(super) fn logical_tray_rect(rect: tray_icon::Rect, scale: f64) -> TrayRect {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    TrayRect {
        x: (rect.position.x / scale) as f32,
        y: (rect.position.y / scale) as f32,
        width: (rect.size.width as f64 / scale) as f32,
        height: (rect.size.height as f64 / scale) as f32,
    }
}

pub(super) fn tray_scale_factor(tray: Option<&tray_icon::TrayIcon>) -> f64 {
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl};

    if let Some(status_item) = tray.and_then(|tray| tray.ns_status_item()) {
        unsafe {
            let item = (&**status_item) as *const _ as id;
            if item != nil {
                let button: id = msg_send![item, button];
                if button != nil {
                    let window: id = msg_send![button, window];
                    if window != nil {
                        let scale: f64 = msg_send![window, backingScaleFactor];
                        if scale.is_finite() && scale > 0.0 {
                            return scale;
                        }
                    }
                }
            }
        }
    }
    screen_scale_at_pointer()
}

fn screen_scale_at_pointer() -> f64 {
    use cocoa::appkit::NSScreen;
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSArray, NSPoint, NSRect};
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let point: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        let screens = NSScreen::screens(nil);
        let count = NSArray::count(screens);
        for index in 0..count {
            let screen: id = screens.objectAtIndex(index);
            let frame: NSRect = msg_send![screen, frame];
            if point.x >= frame.origin.x
                && point.x < frame.origin.x + frame.size.width
                && point.y >= frame.origin.y
                && point.y < frame.origin.y + frame.size.height
            {
                let scale = NSScreen::backingScaleFactor(screen);
                if scale.is_finite() && scale > 0.0 {
                    return scale;
                }
            }
        }
        let main = NSScreen::mainScreen(nil);
        if main != nil {
            let scale = NSScreen::backingScaleFactor(main);
            if scale.is_finite() && scale > 0.0 {
                return scale;
            }
        }
    }
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tray_icon::dpi::{PhysicalPosition, PhysicalSize};

    #[test]
    fn dock_registry_transitions_on_first_open_and_last_close() {
        let mut registry = DockRegistry::default();
        let first = gpui::WindowId::from(1);
        let second = gpui::WindowId::from(2);

        assert!(registry.insert(first));
        assert!(!registry.insert(first));
        assert!(!registry.insert(second));
        assert!(!registry.remove(first));
        assert!(registry.remove(second));
        assert!(!registry.remove(second));
    }

    #[test]
    fn retina_tray_rect_scales_back_to_logical_points() {
        let rect = tray_icon::Rect {
            position: PhysicalPosition::new(2800.0, 4.0),
            size: PhysicalSize::new(64, 48),
        };
        let logical = logical_tray_rect(rect, 2.0);
        assert_eq!(logical.x, 1400.0);
        assert_eq!(logical.y, 2.0);
        assert_eq!(logical.width, 32.0);
        assert_eq!(logical.height, 24.0);
    }

    #[test]
    fn invalid_tray_scale_falls_back_to_unit_scale() {
        let rect = tray_icon::Rect {
            position: PhysicalPosition::new(1400.0, 2.0),
            size: PhysicalSize::new(32, 24),
        };
        assert_eq!(logical_tray_rect(rect, 0.0).x, 1400.0);
        assert_eq!(logical_tray_rect(rect, f64::NAN).width, 32.0);
    }
}
