//! Native Windows toast notifications.
//!
//! Electron raises the real OS notification with `new Notification(...).show()`
//! in `src/main/utils/notification.ts`; this is the GPUI shell's equivalent,
//! delivered through the WinRT `ToastNotificationManager` so the toast lands in
//! the Action Center and respects Focus Assist and per-app notification
//! settings instead of being a borderless popup only the shell knows about.
//!
//! This matches `showNotification`. Electron's `showTransientNotification` also
//! closes the toast after 5s via a JS timer; that is not mirrored, because the
//! WinRT equivalent (an `ExpirationTime` that near) silently prevents delivery.

#[cfg(target_os = "macos")]
use block2::RcBlock;
#[cfg(target_os = "macos")]
use objc2::runtime::Bool;
#[cfg(target_os = "macos")]
use objc2_foundation::{NSError, NSString};
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
    if let Err(error) = show_toast(title, body) {
        eprintln!("[notification] failed to show the toast: {error}");
    }
    #[cfg(target_os = "macos")]
    show_macos_notification(title, body);
    #[cfg(target_os = "linux")]
    {
        let title = title.to_owned();
        let body = body.to_owned();
        std::thread::spawn(move || {
            if let Err(error) = show_linux_notification(&title, &body) {
                eprintln!("[notification] failed to show notification: {error}");
            }
        });
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    let _ = (title, body);
}

#[cfg(target_os = "linux")]
fn show_linux_notification(title: &str, body: &str) -> Result<(), dbus::Error> {
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
            -1_i32,
        ),
    )?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn show_macos_notification(title: &str, body: &str) {
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let title = title.to_owned();
    let body = body.to_owned();
    let center_for_authorization = center.clone();
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
        let identifier = NSString::from_str(&format!(
            "poratake-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &identifier,
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
}

#[cfg(windows)]
fn show_toast(title: &str, body: &str) -> windows::core::Result<()> {
    let document = XmlDocument::new()?;
    document.LoadXml(&HSTRING::from(toast_xml(title, body)))?;
    let notification = ToastNotification::CreateToastNotification(&document)?;
    // No `SetExpirationTime` here, deliberately. An expiry only seconds out
    // makes Windows drop the toast outright: `Show` still returns `Ok` and
    // nothing is delivered, which is close to impossible to notice. Verified by
    // watching `wpndatabase.db-wal` -- with the expiry set the file never moved,
    // without it the toast landed every time.
    //
    // Electron does not set an OS expiry either. `showNotification` sets none,
    // and `showTransientNotification` closes the toast from a JS `setTimeout`,
    // which is a different mechanism. `Toast::show` mirrors `showNotification`:
    // Windows dismisses the banner on its own and the entry stays in the Action
    // Center, exactly as Electron's does.
    let notifier =
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_USER_MODEL_ID))?;
    notifier.Show(&notification)
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
