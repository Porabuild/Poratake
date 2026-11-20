#[cfg(not(test))]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(test))]
use std::sync::{Arc, OnceLock};

use gpui::WindowId;
#[cfg(not(test))]
use gpui::{App, Subscription, Window};
#[cfg(not(test))]
use parking_lot::Mutex;

#[cfg(not(test))]
use crate::config::store::ConfigStore;
#[cfg(not(test))]
use crate::daemon::{DaemonHandle, TooltipOwnerFrame};
#[cfg(not(test))]
use crate::theme::presets::{resolve_theme_mode, ThemeMode};

#[derive(Default)]
struct TooltipState {
    active: Option<(WindowId, u32, String)>,
}

impl TooltipState {
    fn show(&mut self, window: WindowId, owner: u32, text: &str) {
        self.active = Some((window, owner, text.to_owned()));
    }

    fn hide(&mut self, window: WindowId) -> Option<u32> {
        if self
            .active
            .as_ref()
            .is_some_and(|(active, _, _)| *active == window)
        {
            return self.active.take().map(|(_, owner, _)| owner);
        }
        None
    }
}

#[cfg(not(test))]
struct TooltipDispatch {
    daemon: DaemonHandle,
    config: Arc<ConfigStore>,
    state: Mutex<TooltipState>,
    sequence: Arc<AtomicU64>,
}

#[cfg(not(test))]
static DISPATCH: OnceLock<TooltipDispatch> = OnceLock::new();

#[cfg(not(test))]
impl TooltipDispatch {
    fn send(&self, owner: u32, text: Option<String>, owner_frame: Option<TooltipOwnerFrame>) {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let daemon = self.daemon.clone();
        let latest = self.sequence.clone();
        let dark = self.config.read(|settings| {
            resolve_theme_mode(ThemeMode::parse(&settings.appearance.mode)) == ThemeMode::Dark
        });
        std::thread::spawn(move || {
            if let Err(error) = daemon.recording_control().set_tooltip(
                owner,
                sequence,
                text.as_deref(),
                dark,
                owner_frame,
                || latest.load(Ordering::Relaxed) == sequence,
            ) {
                eprintln!("[tooltip] {error}");
            }
        });
    }

    fn hide(&self, window: WindowId) {
        if let Some(owner) = self.state.lock().hide(window) {
            self.send(owner, None, None);
        }
    }
}

#[cfg(not(test))]
pub fn init(cx: &mut App, daemon: DaemonHandle, config: Arc<ConfigStore>) -> Subscription {
    let _ = DISPATCH.set(TooltipDispatch {
        daemon,
        config,
        state: Mutex::new(TooltipState::default()),
        sequence: Arc::new(AtomicU64::new(0)),
    });
    cx.on_window_closed(|_, window| {
        if let Some(dispatch) = DISPATCH.get() {
            dispatch.hide(window);
        }
    })
}

#[cfg(not(test))]
pub fn set(window: &Window, text: Option<&str>) {
    let Some(dispatch) = DISPATCH.get() else {
        return;
    };
    let window_id = window.window_handle().window_id();
    let Some(text) = text else {
        dispatch.hide(window_id);
        return;
    };
    let Some(owner) = crate::windows::macos_window_id(window)
        .and_then(|id| u32::try_from(id).ok())
        .filter(|id| *id != 0)
    else {
        return;
    };
    dispatch.state.lock().show(window_id, owner, text);
    dispatch.send(owner, Some(text.to_owned()), owner_frame(window));
}

#[cfg(not(test))]
fn owner_frame(window: &Window) -> Option<TooltipOwnerFrame> {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSRect;
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    unsafe {
        let view = appkit.ns_view.as_ptr() as id;
        let native_window: id = msg_send![view, window];
        if native_window == nil {
            return None;
        }
        let frame: NSRect = msg_send![native_window, frame];
        let owner_frame = TooltipOwnerFrame {
            x: frame.origin.x,
            y: frame.origin.y,
            width: frame.size.width,
            height: frame.size.height,
        };
        (owner_frame.x.is_finite()
            && owner_frame.y.is_finite()
            && owner_frame.width.is_finite()
            && owner_frame.height.is_finite()
            && owner_frame.width > 0.0
            && owner_frame.height > 0.0)
            .then_some(owner_frame)
    }
}

#[cfg(all(not(test), feature = "parity"))]
pub fn status(window: &Window) -> Option<String> {
    DISPATCH
        .get()?
        .state
        .lock()
        .active
        .as_ref()
        .filter(|(id, _, _)| *id == window.window_handle().window_id())
        .map(|(_, _, text)| text.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_current_owner_can_hide_the_tooltip() {
        let mut state = TooltipState::default();
        let first = WindowId::from(1);
        let second = WindowId::from(2);
        state.show(first, 11, "First");
        state.show(second, 22, "Second");
        assert_eq!(state.hide(first), None);
        assert_eq!(
            state.active.as_ref().map(|(_, _, text)| text.as_str()),
            Some("Second")
        );
        assert_eq!(state.hide(second), Some(22));
        assert_eq!(state.hide(second), None);
        assert!(state.active.is_none());
    }
}
