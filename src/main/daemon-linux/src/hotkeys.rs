use std::cell::RefCell;

use global_hotkey::GlobalHotKeyManager;
use global_hotkey::hotkey::HotKey;

thread_local! {
    static MANAGER: RefCell<Option<GlobalHotKeyManager>> = const { RefCell::new(None) };
}

pub fn register(hotkey: HotKey) -> Result<(), String> {
    MANAGER.with(|manager| {
        let mut manager = manager.borrow_mut();
        if manager.is_none() {
            *manager = Some(
                GlobalHotKeyManager::new()
                    .map_err(|error| format!("could not initialize shortcut handling: {error}"))?,
            );
        }
        manager
            .as_ref()
            .expect("hotkey manager initialized")
            .register(hotkey)
            .map_err(|error| error.to_string())
    })
}

pub fn unregister(hotkey: HotKey) {
    MANAGER.with(|manager| {
        if let Some(manager) = manager.borrow().as_ref() {
            let _ = manager.unregister(hotkey);
        }
    });
}
