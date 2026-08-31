mod icons;
mod intent;
mod menu;

pub use icons::tray_icon;
pub use intent::Intent;
pub use menu::{entries, TrayMenuState};
#[cfg(target_os = "linux")]
pub(crate) use menu::{native_entries, NativeMenuEntry};
