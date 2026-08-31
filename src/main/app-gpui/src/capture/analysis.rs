//! OCR and QR-code recognition — ports of `capture/ocr/index.ts` and
//! `capture/qrcode/index.ts`: capture the selection to a temp file, hand it to
//! the daemon, copy the result and clean the file up.

use std::path::Path;

use crate::daemon::DaemonHandle;

pub struct Outcome {
    pub title: &'static str,
    pub body: String,
    pub clipboard: Option<String>,
}

pub fn recognize_text(daemon: &DaemonHandle, image: &Path) -> Outcome {
    match daemon.ocr().recognize(image) {
        Ok(text) if !text.trim().is_empty() => Outcome {
            title: "Text copied",
            body: "Recognized text has been copied to the clipboard".to_string(),
            clipboard: Some(text.trim().to_string()),
        },
        Ok(_) => Outcome {
            title: "No Text Found",
            body: "No text was detected in the selected area".to_string(),
            clipboard: None,
        },
        Err(error) => {
            eprintln!("[ocr] recognize failed: {error}");
            Outcome {
                title: "OCR Failed",
                body: "Failed to extract text from the image".to_string(),
                clipboard: None,
            }
        }
    }
}

pub fn scan_qr_code(daemon: &DaemonHandle, image: &Path) -> Outcome {
    match daemon.qrcode().detect(image) {
        Ok(payload) if !payload.trim().is_empty() => Outcome {
            title: "QR Code Copied",
            body: "QR code value has been copied to clipboard".to_string(),
            clipboard: Some(payload.trim().to_string()),
        },
        Ok(_) => Outcome {
            title: "No QR Code Found",
            body: "No QR code was detected in the selected area".to_string(),
            clipboard: None,
        },
        Err(error) => {
            eprintln!("[qrcode] detect failed: {error}");
            Outcome {
                title: "Scan Failed",
                body: "Failed to scan QR code from the image".to_string(),
                clipboard: None,
            }
        }
    }
}
