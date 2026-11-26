pub mod accelerator;
pub mod capabilities;
#[cfg(any(target_os = "macos", test, all(windows, feature = "parity")))]
pub mod click_through;
pub mod clipboard;
pub mod desktop;
pub mod device_test;
pub mod devices;
pub mod dialog;
pub mod hotkeys;
#[cfg(target_os = "linux")]
pub mod linux_session;
#[cfg(target_os = "macos")]
pub mod macos_tooltip;
pub mod memory;
pub mod native;
#[cfg(windows)]
pub mod native_tooltip;
pub mod notification;
pub mod permissions;
pub mod reduced_transparency;
pub mod single_instance;
pub mod startup;
#[cfg(all(target_os = "macos", not(test)))]
pub mod tooltip_macos;
pub mod tray;
#[cfg(windows)]
pub mod velopack_hooks;
pub mod window_composition;
pub mod window_surface;
pub mod window_visibility;
pub mod work_area;
