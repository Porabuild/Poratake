//! Native Windows toast notifications.
//!
//! Electron raises the real OS notification with `new Notification(...).show()`
//! in `src/main/utils/notification.ts`; this is the GPUI shell's equivalent,
//! delivered through the WinRT `ToastNotificationManager` so the toast lands in
//! the Action Center and respects Focus Assist and per-app notification
//! settings instead of being a borderless popup only the shell knows about.
//!
//! `show` matches `showNotification`. `show_transient` matches
//! `showTransientNotification`: silent, dismissed after 5s — on Windows by
//! removing the toast's tag from the notification history on a main-thread
//! timer (the JS-timer `close()` equivalent), on macOS by removing the
//! delivered identifier, on Linux through the Notify `expire_timeout`.

#[cfg(target_os = "macos")]
use block2::RcBlock;
#[cfg(target_os = "macos")]
use objc2::runtime::Bool;
#[cfg(target_os = "macos")]
use objc2_foundation::{NSBundle, NSError, NSString};
#[cfg(target_os = "macos")]
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
    UNUserNotificationCenter,
};
#[cfg(windows)]
use windows::core::HSTRING;
#[cfg(windows)]
use windows::Data::Xml::Dom::XmlDocument;
#[cfg(windows)]
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

/// The AppUserModelID read from the installed Start Menu shortcut's
/// `System.AppUserModel.ID`. It is `electron.app.Poratake`, NOT the
/// electron-builder appId `com.porabuild.poratake`: the notification is
/// registered under the AUMID, and `CreateToastNotifier` with the appId
/// succeeds while the toast silently never appears.
#[cfg(windows)]
const APP_USER_MODEL_ID: &str = "electron.app.Poratake";

pub fn show(title: &str, body: &str) {
    #[cfg(windows)]
    if let Err(error) = show_toast(title, body, None) {
        eprintln!("[notification] failed to show the toast: {error}");
    }
    #[cfg(target_os = "macos")]
    show_macos_notification(title, body);
    #[cfg(target_os = "linux")]
    {
        let title = title.to_owned();
        let body = body.to_owned();
        std::thread::spawn(move || {
            if let Err(error) = show_linux_notification(&title, &body, false) {
                eprintln!("[notification] failed to show notification: {error}");
            }
        });
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    let _ = (title, body);
}

/// Electron's `TRANSIENT_NOTIFICATION_DURATION_MS`.
const TRANSIENT_DURATION: std::time::Duration = std::time::Duration::from_secs(5);

/// `showTransientNotification`: silent like `show`, dismissed after 5s. The
/// dismissal runs on the main thread — on Windows the history call needs the
/// COM apartment `show_toast` already runs in.
pub fn show_transient(cx: &mut gpui::App, title: &str, body: &str) {
    #[cfg(windows)]
    {
        let tag = format!(
            "poratake-transient-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        match show_toast(title, body, Some(&tag)) {
            Ok(()) => {
                cx.spawn(async move |cx| dismiss_transient(cx, tag).await)
                    .detach();
            }
            Err(error) => eprintln!("[notification] failed to show the toast: {error}"),
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(identifier) = show_macos_notification(title, body) {
            cx.spawn(async move |cx| dismiss_transient(cx, identifier).await)
                .detach();
        }
    }
    #[cfg(target_os = "linux")]
    {
        let _ = cx;
        let title = title.to_owned();
        let body = body.to_owned();
        std::thread::spawn(move || {
            if let Err(error) = show_linux_notification(&title, &body, true) {
                eprintln!("[notification] failed to show notification: {error}");
            }
        });
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    let _ = (cx, title, body);
}

#[cfg(windows)]
async fn dismiss_transient(cx: &mut gpui::AsyncApp, tag: String) {
    cx.background_executor().timer(TRANSIENT_DURATION).await;
    cx.update(|_| {
        if let Err(error) = remove_toast(&tag) {
            eprintln!("[notification] failed to dismiss the toast: {error}");
        }
    });
}

#[cfg(target_os = "macos")]
async fn dismiss_transient(cx: &mut gpui::AsyncApp, identifier: String) {
    cx.background_executor().timer(TRANSIENT_DURATION).await;
    cx.update(|_| remove_macos_notification(&identifier));
}

#[cfg(target_os = "linux")]
fn show_linux_notification(title: &str, body: &str, transient: bool) -> Result<(), dbus::Error> {
    use std::collections::HashMap;
    use std::time::Duration;

    use dbus::arg::{RefArg, Variant};
    use dbus::blocking::Connection;

    let connection = Connection::new_session()?;
    let proxy = connection.with_proxy(
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        Duration::from_secs(2),
    );
    let _: (u32,) = proxy.method_call(
        "org.freedesktop.Notifications",
        "Notify",
        (
            "Poratake",
            0_u32,
            "",
            title,
            body,
            Vec::<String>::new(),
            HashMap::<String, Variant<Box<dyn RefArg>>>::new(),
            if transient {
                TRANSIENT_DURATION.as_millis() as i32
            } else {
                -1_i32
            },
        ),
    )?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn show_macos_notification(title: &str, body: &str) -> Option<String> {
    // `currentNotificationCenter` throws an Objective-C exception when the
    // process has no bundle identifier (a bare binary, as in `cargo test`),
    // and Rust cannot catch Objective-C exceptions — the process aborts.
    NSBundle::mainBundle().bundleIdentifier()?;
    let identifier = format!(
        "poratake-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let title = title.to_owned();
    let body = body.to_owned();
    let center_for_authorization = center.clone();
    let identifier_for_request = identifier.clone();
    let authorization = RcBlock::new(move |granted: Bool, error: *mut NSError| {
        if !error.is_null() {
            eprintln!("[notification] failed to request notification authorization");
            return;
        }
        if !granted.as_bool() {
            return;
        }

        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&title));
        content.setBody(&NSString::from_str(&body));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(&identifier_for_request),
            &content,
            None,
        );
        let completion = RcBlock::new(|error: *mut NSError| {
            if !error.is_null() {
                eprintln!("[notification] failed to show notification");
            }
        });
        center_for_authorization
            .addNotificationRequest_withCompletionHandler(&request, Some(&completion));
    });
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert,
        &authorization,
    );
    Some(identifier)
}

#[cfg(target_os = "macos")]
fn remove_macos_notification(identifier: &str) {
    use objc2_foundation::NSArray;

    if NSBundle::mainBundle().bundleIdentifier().is_none() {
        return;
    }
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let identifier = NSString::from_str(identifier);
    let identifiers = NSArray::from_slice(&[&*identifier]);
    center.removeDeliveredNotificationsWithIdentifiers(&identifiers);
}

#[cfg(windows)]
fn show_toast(title: &str, body: &str, tag: Option<&str>) -> windows::core::Result<()> {
    let document = XmlDocument::new()?;
    document.LoadXml(&HSTRING::from(toast_xml(title, body)))?;
    let notification = ToastNotification::CreateToastNotification(&document)?;
    if let Some(tag) = tag {
        notification.SetTag(&HSTRING::from(tag))?;
    }
    // No `SetExpirationTime` here, deliberately. An expiry only seconds out
    // makes Windows drop the toast outright: `Show` still returns `Ok` and
    // nothing is delivered, which is close to impossible to notice. Verified by
    // watching `wpndatabase.db-wal` -- with the expiry set the file never moved,
    // without it the toast landed every time.
    //
    // Electron does not set an OS expiry either. `showNotification` sets none,
    // and `showTransientNotification` closes the toast from a JS `setTimeout`,
    // which is a different mechanism. `show_transient` mirrors it by removing
    // the toast's tag from the notification history after 5s; `Toast::show`
    // mirrors `showNotification`: Windows dismisses the banner on its own and
    // the entry stays in the Action Center, exactly as Electron's does.
    let notifier =
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_USER_MODEL_ID))?;
    notifier.Show(&notification)
}

#[cfg(windows)]
fn remove_toast(tag: &str) -> windows::core::Result<()> {
    ToastNotificationManager::History()?.Remove(&HSTRING::from(tag))
}

/// A ToastGeneric toast with the title and body as the two `<text>` nodes and
/// `<audio silent="true"/>` matching Electron's `silent: true`.
#[cfg(windows)]
fn toast_xml(title: &str, body: &str) -> String {
    format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual><audio silent="true"/></toast>"#,
        escape(title),
        escape(body)
    )
}

/// XML-escapes a toast payload. Titles and bodies carry file names and error
/// strings; a bare `&` or `<` makes `LoadXml` fail and the notification
/// silently vanish.
#[cfg(any(windows, test))]
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use windows::core::HSTRING;
    #[cfg(windows)]
    use windows::Data::Xml::Dom::XmlDocument;

    use super::escape;
    #[cfg(windows)]
    use super::toast_xml;

    #[cfg(target_os = "macos")]
    #[test]
    fn show_does_not_abort_without_a_bundle_identifier() {
        super::show("title", "body");
    }

    #[test]
    fn escape_handles_the_xml_metacharacters() {
        assert_eq!(escape("A & B"), "A &amp; B");
        assert_eq!(escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(
            escape("C:\\tmp\\a & b < final>.png"),
            "C:\\tmp\\a &amp; b &lt; final&gt;.png"
        );
        assert_eq!(escape("say \"hi\""), "say &quot;hi&quot;");
        assert_eq!(escape("plain text"), "plain text");
    }

    #[cfg(windows)]
    #[test]
    fn a_title_and_body_with_ampersands_and_angle_brackets_load_as_xml() {
        let xml = toast_xml("A & B", "<error> failed to write &/read");
        let document = XmlDocument::new().expect("create an XmlDocument");
        assert!(
            document.LoadXml(&HSTRING::from(xml)).is_ok(),
            "unescaped metacharacters would make LoadXml fail"
        );
    }
}
