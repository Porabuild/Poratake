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

use windows::core::HSTRING;
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

/// The AppUserModelID read from the installed Start Menu shortcut's
/// `System.AppUserModel.ID`. It is `electron.app.Poratake`, NOT the
/// electron-builder appId `com.porabuild.poratake`: the notification is
/// registered under the AUMID, and `CreateToastNotifier` with the appId
/// succeeds while the toast silently never appears.
const APP_USER_MODEL_ID: &str = "electron.app.Poratake";

pub fn show(title: &str, body: &str) {
    if let Err(error) = show_toast(title, body) {
        eprintln!("[notification] failed to show the toast: {error}");
    }
}

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
    use windows::core::HSTRING;
    use windows::Data::Xml::Dom::XmlDocument;

    use super::{escape, toast_xml};

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
