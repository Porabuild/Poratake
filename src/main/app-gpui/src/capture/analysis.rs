//! OCR and QR-code recognition — ports of `capture/ocr/index.ts` and
//! `capture/qrcode/index.ts`: capture the selection to a temp file, hand it to
//! the daemon, copy the result and clean the file up.

use std::path::Path;

use serde_json::json;

use crate::daemon::DaemonHandle;

pub struct Outcome {
    pub title: &'static str,
    pub body: String,
}

pub fn recognize_text(daemon: &DaemonHandle, image: &Path) -> Outcome {
    match call_text(daemon, image, AnalysisKind::Ocr) {
        Ok(text) if !text.trim().is_empty() => {
            copy(text.trim());
            Outcome {
                title: "Text copied",
                body: "Recognized text has been copied to the clipboard".to_string(),
            }
        }
        Ok(_) => Outcome {
            title: "No Text Found",
            body: "No text was detected in the selected area".to_string(),
        },
        Err(error) => {
            eprintln!("[ocr] recognize failed: {error}");
            Outcome {
                title: "OCR Failed",
                body: "Failed to extract text from the image".to_string(),
            }
        }
    }
}

pub fn scan_qr_code(daemon: &DaemonHandle, image: &Path) -> Outcome {
    match call_text(daemon, image, AnalysisKind::QrCode) {
        Ok(payload) if !payload.trim().is_empty() => {
            copy(payload.trim());
            Outcome {
                title: "QR Code Copied",
                body: "QR code value has been copied to clipboard".to_string(),
            }
        }
        Ok(_) => Outcome {
            title: "No QR Code Found",
            body: "No QR code was detected in the selected area".to_string(),
        },
        Err(error) => {
            eprintln!("[qrcode] detect failed: {error}");
            Outcome {
                title: "Scan Failed",
                body: "Failed to scan QR code from the image".to_string(),
            }
        }
    }
}

enum AnalysisKind {
    Ocr,
    QrCode,
}

fn call_text(daemon: &DaemonHandle, image: &Path, kind: AnalysisKind) -> anyhow::Result<String> {
    if !daemon.is_running() {
        daemon.start()?;
    }
    let params = Some(json!({ "imagePath": image.to_string_lossy() }));
    let (response, field) = match kind {
        AnalysisKind::Ocr => (
            daemon
                .call("ocr", "recognize", params)
                .map_err(|error| anyhow::anyhow!("ocr recognize failed: {error}"))?,
            "text",
        ),
        AnalysisKind::QrCode => (
            daemon
                .call("qrcode", "detect", params)
                .map_err(|error| anyhow::anyhow!("qrcode detect failed: {error}"))?,
            "payload",
        ),
    };
    Ok(response
        .get(field)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string())
}

fn copy(text: &str) {
    if let Err(error) =
        arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text.to_string()))
    {
        eprintln!("[clipboard] failed to copy recognized text: {error}");
    }
}
