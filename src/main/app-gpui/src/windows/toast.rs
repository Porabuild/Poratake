//! Transient notifications — the GPUI equivalent of Electron's
//! `showNotification` / `showTransientNotification`.
//!
//! Electron raises the real Windows notification (`new Notification(...).show()`
//! in `src/main/utils/notification.ts`); this shell used to draw its own
//! borderless popup instead, which the Action Center, Focus Assist and per-app
//! notification settings never saw. `Toast::show` now forwards to
//! `crate::system::notification::show`, which delivers the toast through the
//! WinRT `ToastNotificationManager` like Electron does.

use gpui::{App, SharedString};

pub struct Toast;

impl Toast {
    pub fn show(_cx: &mut App, title: impl Into<SharedString>, body: impl Into<SharedString>) {
        crate::system::notification::show(&title.into(), &body.into());
    }
}
