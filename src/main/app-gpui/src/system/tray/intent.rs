#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Intent {
    AllInOne,
    CaptureScreen,
    CaptureArea,
    CaptureWindow,
    ScrollCapture,
    CaptureText,
    ScanQrCode,
    TimerCapture,
    OpenInEditor,
    OpenClipboardInEditor,
    Pin,
    RecordScreen,
    RecordArea,
    RecordWindow,
    OpenInVideoEditor,
    History,
    ToggleDesktopIcons,
    OpenSettings,
    OpenAbout,
    HideTrayIcon,
    OpenIssues,
    Quit,
}

const INTENT_IDS: &[(Intent, &str)] = &[
    (Intent::AllInOne, "all-in-one"),
    (Intent::CaptureScreen, "capture-screen"),
    (Intent::CaptureArea, "capture-area"),
    (Intent::CaptureWindow, "capture-window"),
    (Intent::ScrollCapture, "scroll-capture"),
    (Intent::CaptureText, "capture-text"),
    (Intent::ScanQrCode, "scan-qr-code"),
    (Intent::TimerCapture, "timer-capture"),
    (Intent::OpenInEditor, "open-in-editor"),
    (Intent::OpenClipboardInEditor, "open-clipboard-in-editor"),
    (Intent::Pin, "pin"),
    (Intent::RecordScreen, "record-screen"),
    (Intent::RecordArea, "record-area"),
    (Intent::RecordWindow, "record-window"),
    (Intent::OpenInVideoEditor, "open-in-video-editor"),
    (Intent::History, "history"),
    (Intent::ToggleDesktopIcons, "toggle-desktop-icons"),
    (Intent::OpenSettings, "open-settings"),
    (Intent::OpenAbout, "open-about"),
    (Intent::HideTrayIcon, "hide-tray-icon"),
    (Intent::OpenIssues, "open-issues"),
    (Intent::Quit, "quit"),
];

impl Intent {
    pub fn id(self) -> &'static str {
        INTENT_IDS
            .iter()
            .find(|(intent, _)| *intent == self)
            .map(|(_, id)| *id)
            .unwrap_or("unknown")
    }

    pub fn from_id(id: &str) -> Option<Self> {
        INTENT_IDS
            .iter()
            .find(|(_, candidate)| *candidate == id)
            .map(|(intent, _)| *intent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_are_unique() {
        let mut ids: Vec<&str> = INTENT_IDS.iter().map(|(_, id)| *id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total);

        for (intent, id) in INTENT_IDS {
            assert_eq!(Intent::from_id(id), Some(*intent));
            assert_eq!(intent.id(), *id);
        }
        assert_eq!(Intent::from_id("nope"), None);
    }
}
