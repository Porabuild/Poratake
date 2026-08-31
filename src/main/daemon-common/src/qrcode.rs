use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::protocol::{Request, Response, params, send_response};
use crate::router::Reply;

pub const MAX_QR_IMAGE_PIXELS: u64 = 64 * 1024 * 1024;

#[derive(Clone, Default)]
pub struct QrDetectionGate {
    busy: Arc<AtomicBool>,
}

struct QrDetectionGuard(Arc<AtomicBool>);

impl Drop for QrDetectionGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl QrDetectionGate {
    fn try_acquire(&self) -> Option<QrDetectionGuard> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| QrDetectionGuard(self.busy.clone()))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrDetectRequest {
    pub image_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct QrDetectResult {
    pub payload: String,
}

pub fn request_path(request: &Request) -> Result<PathBuf, (String, String)> {
    let request: QrDetectRequest = params(request)?;
    if !request.image_path.is_file() {
        return Err((
            "FILE_NOT_FOUND".into(),
            format!("Image file not found: {}", request.image_path.display()),
        ));
    }
    Ok(request.image_path)
}

pub fn spawn_detection(
    gate: &QrDetectionGate,
    request_id: String,
    image_path: PathBuf,
    detect: impl FnOnce(&Path) -> Result<String, String> + Send + 'static,
) -> Reply {
    let Some(guard) = gate.try_acquire() else {
        return Reply::Now(Err((
            "BUSY".into(),
            "QR code detection is already in progress".into(),
        )));
    };
    match std::thread::Builder::new()
        .name("qr-detection".into())
        .spawn(move || {
            let response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                detect(&image_path)
            })) {
                Ok(Ok(payload)) => {
                    Response::success(&request_id, Some(json!(QrDetectResult { payload })))
                }
                Ok(Err(error)) => Response::error(
                    &request_id,
                    "QR_DETECTION_FAILED",
                    &format!("QR code detection failed: {error}"),
                ),
                Err(_) => Response::error(
                    &request_id,
                    "QR_DETECTION_FAILED",
                    "QR code detection worker panicked",
                ),
            };
            drop(guard);
            send_response(response);
        }) {
        Ok(_) => Reply::Deferred,
        Err(error) => Reply::Now(Err((
            "QR_DETECTION_FAILED".into(),
            format!("Failed to start QR code detection: {error}"),
        ))),
    }
}

#[cfg(feature = "qrcode-decode")]
pub fn detect_payload_from_greyscale(pixels: &[u8], width: usize, height: usize) -> String {
    if width == 0 || height == 0 || pixels.len() < width.saturating_mul(height) {
        return String::new();
    }

    let mut image =
        rqrr::PreparedImage::prepare_from_greyscale(width, height, |x, y| pixels[y * width + x]);
    for grid in image.detect_grids() {
        if let Ok((_meta, content)) = grid.decode()
            && !content.is_empty()
        {
            return content;
        }
    }
    String::new()
}

pub fn image_buffer_size(width: u32, height: u32) -> Result<usize, &'static str> {
    let pixel_count = u64::from(width) * u64::from(height);
    if pixel_count > MAX_QR_IMAGE_PIXELS {
        return Err("Image dimensions exceed the QR detection limit");
    }
    usize::try_from(pixel_count).map_err(|_| "Failed to decode image data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_and_result_use_the_shared_wire_shape() {
        let request = serde_json::to_value(QrDetectRequest {
            image_path: PathBuf::from("capture.png"),
        })
        .expect("serialize request");
        let result = serde_json::to_value(QrDetectResult {
            payload: "value".into(),
        })
        .expect("serialize result");

        assert_eq!(request["imagePath"], "capture.png");
        assert_eq!(result["payload"], "value");
    }

    #[test]
    fn detection_gate_allows_only_one_worker() {
        let gate = QrDetectionGate::default();
        let guard = gate.try_acquire().expect("first worker");
        assert!(gate.try_acquire().is_none());
        drop(guard);
        assert!(gate.try_acquire().is_some());
    }

    #[test]
    fn spawn_detection_rejects_concurrent_workers() {
        let gate = QrDetectionGate::default();
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let (finished_tx, finished_rx) = std::sync::mpsc::sync_channel(1);
        let first = spawn_detection(
            &gate,
            "first".into(),
            PathBuf::from("unused.png"),
            move |_| {
                started_tx.send(()).expect("signal start");
                release_rx.recv().expect("release worker");
                finished_tx.send(()).expect("signal finish");
                Ok(String::new())
            },
        );
        assert!(matches!(first, Reply::Deferred));
        started_rx.recv().expect("worker started");

        let second = spawn_detection(&gate, "second".into(), PathBuf::from("unused.png"), |_| {
            panic!("busy worker must not start")
        });
        let Reply::Now(result) = second else {
            panic!("concurrent detection should respond immediately");
        };
        assert_eq!(result.expect_err("busy detection").0, "BUSY");

        release_tx.send(()).expect("release first worker");
        finished_rx.recv().expect("first worker finished");
        while gate.busy.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        assert!(gate.try_acquire().is_some());
    }

    #[test]
    #[cfg(feature = "qrcode-decode")]
    fn rejects_invalid_greyscale_dimensions() {
        let pixels = vec![0u8; 4];
        assert_eq!(detect_payload_from_greyscale(&pixels, 0, 0), "");
        assert_eq!(detect_payload_from_greyscale(&pixels, 10, 10), "");
    }

    #[test]
    fn accepts_the_pixel_limit_and_rejects_larger_images() {
        assert_eq!(image_buffer_size(8192, 8192), Ok(64 * 1024 * 1024));
        assert_eq!(
            image_buffer_size(8193, 8192),
            Err("Image dimensions exceed the QR detection limit")
        );
    }
}
