//! What a confirmed area selection is for. The overlay is one surface shared
//! by the screenshot, OCR, QR and timer flows, exactly as the Electron
//! `area-overlay` is.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CaptureIntent {
    #[default]
    Screenshot,
    Ocr,
    QrCode,
    Timer,
    ScrollCapture,
    Recording,
    EditorAttach,
}

/// `'Drag to select an area \u{b7} Esc to cancel'`.
pub const DRAG_PROMPT: &str = "Drag to select an area \u{b7} Esc to cancel";

/// `window-pick-targets.ts`.
pub const WINDOW_PICK_PROMPT: &str = "Click a window to select it \u{b7} Esc to cancel";

pub const DISPLAY_PICK_PROMPT: &str = "Click a display to select it \u{b7} Esc to cancel";

impl CaptureIntent {
    /// `area-overlay-window.tsx` hard-codes one line for dragging, whatever the
    /// capture is for -- there is no per-intent wording in Electron, and the
    /// hint names the key that cancels. This used to return six invented
    /// strings, none of which appear anywhere in the reference.
    pub fn prompt(self) -> &'static str {
        DRAG_PROMPT
    }

    pub fn shows_prompt(self) -> bool {
        !matches!(self, Self::Timer)
    }

    pub fn temp_prefix(self) -> &'static str {
        match self {
            Self::Screenshot => "poratake-capture",
            Self::Ocr => "poratake-ocr",
            Self::QrCode => "poratake-qrcode",
            Self::Timer => "poratake-timer",
            Self::ScrollCapture => "poratake-scroll",
            Self::Recording => "poratake-recording",
            Self::EditorAttach => "poratake-editor",
        }
    }

    /// Only the screenshot flow writes into the library; the analysis flows
    /// work from a temp file they delete afterwards.
    pub fn saves_to_library(self) -> bool {
        matches!(self, Self::Screenshot | Self::Timer | Self::ScrollCapture)
    }

    pub fn allows_freeze(self) -> bool {
        !matches!(self, Self::ScrollCapture | Self::Recording)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_capture_flows_write_to_the_library() {
        assert!(CaptureIntent::Screenshot.saves_to_library());
        assert!(CaptureIntent::Timer.saves_to_library());
        assert!(CaptureIntent::ScrollCapture.saves_to_library());
        assert!(!CaptureIntent::Ocr.saves_to_library());
        assert!(!CaptureIntent::QrCode.saves_to_library());
    }

    #[test]
    fn only_the_timer_flow_suppresses_the_prompt() {
        assert!(!CaptureIntent::Timer.shows_prompt());
        assert!(CaptureIntent::Screenshot.shows_prompt());
        assert!(CaptureIntent::ScrollCapture.shows_prompt());
        assert!(CaptureIntent::Recording.shows_prompt());
    }

    #[test]
    fn scroll_and_direct_recording_selection_use_live_pixels() {
        assert!(!CaptureIntent::ScrollCapture.allows_freeze());
        assert!(!CaptureIntent::Recording.allows_freeze());
        assert!(CaptureIntent::Screenshot.allows_freeze());
        assert!(CaptureIntent::EditorAttach.allows_freeze());
    }

    #[test]
    fn temp_prefixes_are_distinct() {
        let mut prefixes = [
            CaptureIntent::Screenshot.temp_prefix(),
            CaptureIntent::Ocr.temp_prefix(),
            CaptureIntent::QrCode.temp_prefix(),
            CaptureIntent::Timer.temp_prefix(),
            CaptureIntent::ScrollCapture.temp_prefix(),
            CaptureIntent::Recording.temp_prefix(),
            CaptureIntent::EditorAttach.temp_prefix(),
        ];
        prefixes.sort_unstable();
        let total = prefixes.len();
        let mut unique = prefixes.to_vec();
        unique.dedup();
        assert_eq!(unique.len(), total);
    }
}
