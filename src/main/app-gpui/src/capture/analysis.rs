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
    let processed = preprocessed_image(image);
    let source = processed.as_deref().unwrap_or(image);
    let outcome = match daemon.ocr().recognize(source) {
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
    };
    if let Some(processed) = processed.as_deref() {
        let _ = std::fs::remove_file(processed);
    }
    outcome
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

/// `preprocessImageForOcr` (`utils/ffmpeg.ts`): upscale so the longer side
/// reaches 1300px, grayscale, sharpen. Electron runs it through FFmpeg on
/// non-mac only; the `image` crate port keeps this shell FFmpeg-free.
/// Returns the processed path, or `None` when preprocessing is skipped or
/// fails — the caller falls back to the raw capture either way.
fn preprocessed_image(image: &Path) -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let _ = image;
        None
    }
    #[cfg(not(target_os = "macos"))]
    {
        preprocess_for_ocr(image)
    }
}

/// Longer side grows to at least this many pixels, mirroring the FFmpeg
/// `scale=iw*max(1,1300/max(iw,ih))` filter.
#[cfg(any(test, not(target_os = "macos")))]
const OCR_MIN_LONG_SIDE: u32 = 1300;

#[cfg(any(test, not(target_os = "macos")))]
fn ocr_upscale_factor(width: u32, height: u32) -> f64 {
    let long_side = width.max(height).max(1) as f64;
    (f64::from(OCR_MIN_LONG_SIDE) / long_side).max(1.0)
}

#[cfg(not(target_os = "macos"))]
fn preprocess_for_ocr(image: &Path) -> Option<std::path::PathBuf> {
    let source = image::ImageReader::open(image).ok()?.decode().ok()?;
    let factor = ocr_upscale_factor(source.width(), source.height());
    let width = ((source.width() as f64 * factor).round() as u32).max(1);
    let height = ((source.height() as f64 * factor).round() as u32).max(1);
    let resized = source.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
    let gray = resized.grayscale();
    let sharpened = image::imageops::unsharpen(&gray, 1.2, 1);
    let stem = image.file_stem()?.to_str()?;
    let output = std::env::temp_dir().join(format!("{stem}-processed.png"));
    sharpened.save(&output).ok()?;
    output.is_file().then_some(output)
}

#[cfg(test)]
mod tests {
    use super::ocr_upscale_factor;

    #[test]
    fn ocr_upscale_matches_the_ffmpeg_filter() {
        assert_eq!(ocr_upscale_factor(1300, 800), 1.0);
        assert_eq!(ocr_upscale_factor(2000, 1500), 1.0);
        assert_eq!(ocr_upscale_factor(650, 400), 2.0);
        assert_eq!(ocr_upscale_factor(100, 100), 13.0);
        assert_eq!(ocr_upscale_factor(0, 0), 1300.0);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn ocr_preprocess_upscales_grays_and_sharpens() {
        let dir = tempfile::tempdir().expect("temp dir");
        let input = dir.path().join("ocr-input.png");
        let frame = image::RgbImage::from_fn(100, 60, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        frame.save(&input).expect("save input");

        let output = super::preprocess_for_ocr(&input).expect("preprocess");
        let processed = image::ImageReader::open(&output)
            .expect("open output")
            .decode()
            .expect("decode output");
        assert_eq!((processed.width(), processed.height()), (1300, 780));
        assert!(matches!(processed, image::DynamicImage::ImageLuma8(_)));
        let _ = std::fs::remove_file(output);
    }
}
