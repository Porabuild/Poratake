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

    pub fn temp_prefix(self) -> &'static str {
        match self {
            Self::Screenshot => "poratake-capture",
            Self::Ocr => "poratake-ocr",
            Self::QrCode => "poratake-qrcode",
            Self::Timer => "poratake-timer",
            Self::ScrollCapture => "poratake-scroll",
            Self::Recording => "poratake-recording",
        }
    }

    /// Only the screenshot flow writes into the library; the analysis flows
    /// work from a temp file they delete afterwards.
    pub fn saves_to_library(self) -> bool {
        matches!(self, Self::Screenshot | Self::Timer | Self::ScrollCapture)
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
    fn temp_prefixes_are_distinct() {
        let mut prefixes = [
            CaptureIntent::Screenshot.temp_prefix(),
            CaptureIntent::Ocr.temp_prefix(),
            CaptureIntent::QrCode.temp_prefix(),
            CaptureIntent::Timer.temp_prefix(),
            CaptureIntent::ScrollCapture.temp_prefix(),
            CaptureIntent::Recording.temp_prefix(),
        ];
        prefixes.sort_unstable();
        let total = prefixes.len();
        let mut unique = prefixes.to_vec();
        unique.dedup();
        assert_eq!(unique.len(), total);
    }
}

#[cfg(test)]
mod prompt_tests {
    /// Every prompt this shell can show has to be a string that exists in the
    /// reference. Six invented per-intent hints shipped here before anyone
    /// compared the two overlays side by side.
    #[test]
    fn every_prompt_is_one_the_reference_actually_uses() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repository root")
            .to_path_buf();

        let mut reference = String::new();
        for relative in [
            "src/renderer/windows/area-overlay-window.tsx",
            "src/main/capture/area-overlay/window-pick-targets.ts",
            "src/main/capture/area-selector/overlay-backend.ts",
        ] {
            reference.push_str(&std::fs::read_to_string(root.join(relative)).expect(relative));
        }

        for prompt in [
            super::DRAG_PROMPT,
            super::WINDOW_PICK_PROMPT,
            super::DISPLAY_PICK_PROMPT,
        ] {
            assert!(
                reference.contains(prompt),
                "`{prompt}` does not appear in the reference overlay sources"
            );
        }

        for intent in [
            super::CaptureIntent::Screenshot,
            super::CaptureIntent::Ocr,
            super::CaptureIntent::QrCode,
            super::CaptureIntent::Timer,
            super::CaptureIntent::ScrollCapture,
            super::CaptureIntent::Recording,
        ] {
            assert!(
                reference.contains(intent.prompt()),
                "the prompt for {intent:?} is not a string the reference uses"
            );
        }
    }
}
