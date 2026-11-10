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
    if pixels[..width * height]
        .iter()
        .all(|&pixel| pixel == pixels[0])
    {
        return String::new();
    }

    let mut candidates = Vec::new();
    let payload = detect_in_region(
        pixels,
        width,
        height,
        &[
            ThresholdPass::Adaptive,
            ThresholdPass::Local(8),
            ThresholdPass::Histogram,
            ThresholdPass::Local(4),
            ThresholdPass::Local(16),
        ],
        Some(&mut candidates),
    );
    if !payload.is_empty() {
        return payload;
    }

    let mut remaining = width * height;
    for (left, top, right, bottom) in candidates {
        let margin = (right - left).max(bottom - top) / 4;
        let left = left.saturating_sub(margin);
        let top = top.saturating_sub(margin);
        let right = right.saturating_add(margin).min(width);
        let bottom = bottom.saturating_add(margin).min(height);
        let region_width = right - left;
        let region_height = bottom - top;
        let size = region_width * region_height;
        if size > remaining {
            continue;
        }
        let mut region = Vec::new();
        if region.try_reserve_exact(size).is_err() {
            continue;
        }
        remaining -= size;
        for y in top..bottom {
            region.extend_from_slice(&pixels[y * width + left..y * width + right]);
        }
        let payload = detect_in_region(
            &region,
            region_width,
            region_height,
            &[
                ThresholdPass::Histogram,
                ThresholdPass::Local(12),
                ThresholdPass::Local(16),
            ],
            None,
        );
        if !payload.is_empty() {
            return payload;
        }
    }
    detect_in_region(
        pixels,
        width,
        height,
        &[
            ThresholdPass::LocalWithPreflight(24),
            ThresholdPass::LocalWithPreflight(10),
        ],
        None,
    )
}

#[cfg(feature = "qrcode-decode")]
#[derive(Clone, Copy)]
enum ThresholdPass {
    Adaptive,
    Histogram,
    Local(usize),
    LocalWithPreflight(usize),
}

#[cfg(feature = "qrcode-decode")]
fn detect_in_region(
    pixels: &[u8],
    width: usize,
    height: usize,
    passes: &[ThresholdPass],
    mut candidates: Option<&mut Vec<(usize, usize, usize, usize)>>,
) -> String {
    for &pass in passes {
        let mut image = match pass {
            ThresholdPass::Adaptive => {
                rqrr::PreparedImage::prepare_from_greyscale(width, height, |x, y| {
                    pixels[y * width + x]
                })
            }
            ThresholdPass::Local(block) | ThresholdPass::LocalWithPreflight(block) => {
                if width < block * 5 || height < block * 5 {
                    continue;
                }
                let thresholds = local_thresholds(pixels, width, height, block);
                let columns = width.div_ceil(block);
                if matches!(pass, ThresholdPass::LocalWithPreflight(_))
                    && !has_finder_transitions(pixels, width, height, block, &thresholds)
                {
                    continue;
                }
                rqrr::PreparedImage::prepare_from_bitmap(width, height, |x, y| {
                    pixels[y * width + x] <= thresholds[(y / block) * columns + x / block]
                })
            }
            ThresholdPass::Histogram => {
                let mut histogram = [0u64; 256];
                for &pixel in &pixels[..width * height] {
                    histogram[pixel as usize] += 1;
                }
                let total = (width * height) as f64;
                let sum: f64 = histogram
                    .iter()
                    .enumerate()
                    .map(|(value, count)| value as f64 * *count as f64)
                    .sum();
                let mut lower_count = 0.0;
                let mut lower_sum = 0.0;
                let mut best = 0.0;
                let mut threshold = None;
                for (value, count) in histogram.iter().enumerate() {
                    lower_count += *count as f64;
                    lower_sum += value as f64 * *count as f64;
                    if lower_count == 0.0 {
                        continue;
                    }
                    let upper_count = total - lower_count;
                    if upper_count == 0.0 {
                        break;
                    }
                    let difference = lower_sum / lower_count - (sum - lower_sum) / upper_count;
                    let variance = lower_count * upper_count * difference * difference;
                    if variance > best {
                        best = variance;
                        threshold = Some(value as u8);
                    }
                }
                let Some(threshold) = threshold else {
                    continue;
                };
                rqrr::PreparedImage::prepare_from_bitmap(width, height, |x, y| {
                    pixels[y * width + x] <= threshold
                })
            }
        };
        for grid in image.detect_grids() {
            if let Ok((_meta, content)) = grid.decode()
                && !content.is_empty()
            {
                return content;
            }
            if let Some(candidates) = candidates.as_deref_mut()
                && candidates.len() < 8
            {
                let left = grid
                    .bounds
                    .iter()
                    .map(|point| point.x.max(0) as usize)
                    .min()
                    .unwrap()
                    .min(width);
                let top = grid
                    .bounds
                    .iter()
                    .map(|point| point.y.max(0) as usize)
                    .min()
                    .unwrap()
                    .min(height);
                let right = grid
                    .bounds
                    .iter()
                    .map(|point| point.x.max(0) as usize)
                    .max()
                    .unwrap()
                    .saturating_add(1)
                    .min(width);
                let bottom = grid
                    .bounds
                    .iter()
                    .map(|point| point.y.max(0) as usize)
                    .max()
                    .unwrap()
                    .saturating_add(1)
                    .min(height);
                if left >= right || top >= bottom {
                    continue;
                }
                let area = (right - left) * (bottom - top);
                let duplicate = candidates.iter().any(|&(x0, y0, x1, y1)| {
                    let overlap = right.min(x1).saturating_sub(left.max(x0))
                        * bottom.min(y1).saturating_sub(top.max(y0));
                    overlap * 4 >= area.min((x1 - x0) * (y1 - y0)) * 3
                });
                if !duplicate {
                    candidates.push((left, top, right, bottom));
                }
            }
        }
    }
    String::new()
}

#[cfg(feature = "qrcode-decode")]
fn has_finder_transitions(
    pixels: &[u8],
    width: usize,
    height: usize,
    block: usize,
    thresholds: &[u8],
) -> bool {
    let columns = width.div_ceil(block);
    for y in 0..height {
        let mut last = pixels[y * width] <= thresholds[(y / block) * columns];
        let mut changes = 0;
        for x in 1..width {
            let black = pixels[y * width + x] <= thresholds[(y / block) * columns + x / block];
            if black != last {
                changes += 1;
                last = black;
                if changes >= 4 {
                    return true;
                }
            }
        }
    }
    false
}

/*
 * Copyright 2009 ZXing authors
 * Modified 2026 for Poratake: adapted local thresholding to Rust.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
#[cfg(feature = "qrcode-decode")]
fn local_thresholds(pixels: &[u8], width: usize, height: usize, block: usize) -> Vec<u8> {
    let columns = width.div_ceil(block);
    let rows = height.div_ceil(block);
    let mut black_points = vec![0u8; columns * rows];
    for block_y in 0..rows {
        let top = (block_y * block).min(height - block);
        for block_x in 0..columns {
            let left = (block_x * block).min(width - block);
            let mut sum = 0u32;
            let mut minimum = 255u8;
            let mut maximum = 0u8;
            for y in top..top + block {
                for x in left..left + block {
                    let pixel = pixels[y * width + x];
                    sum += u32::from(pixel);
                    minimum = minimum.min(pixel);
                    maximum = maximum.max(pixel);
                }
            }
            let mut value = (sum / (block * block) as u32) as u8;
            if maximum - minimum <= 24 {
                value = minimum / 2;
                if block_y > 0 && block_x > 0 {
                    let neighbor = ((u16::from(black_points[(block_y - 1) * columns + block_x])
                        + 2 * u16::from(black_points[block_y * columns + block_x - 1])
                        + u16::from(black_points[(block_y - 1) * columns + block_x - 1]))
                        / 4) as u8;
                    if minimum < neighbor {
                        value = neighbor;
                    }
                }
            }
            black_points[block_y * columns + block_x] = value;
        }
    }
    let mut thresholds = vec![0; columns * rows];
    for block_y in 0..rows {
        let center_y = block_y.clamp(2, rows - 3);
        for block_x in 0..columns {
            let center_x = block_x.clamp(2, columns - 3);
            let mut sum = 0u16;
            for y in center_y - 2..=center_y + 2 {
                for x in center_x - 2..=center_x + 2 {
                    sum += u16::from(black_points[y * columns + x]);
                }
            }
            thresholds[block_y * columns + block_x] = (sum / 25) as u8;
        }
    }
    thresholds
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
    #[cfg(feature = "qrcode-decode")]
    fn finder_transition_preflight_keeps_patterns_at_image_edges() {
        let thresholds = [127; 8];
        for row in [
            [0, 255, 0, 0, 0, 255, 0, 255],
            [255, 0, 255, 0, 0, 0, 255, 0],
            [0, 255, 0, 0, 0, 255, 0, 0],
        ] {
            assert!(has_finder_transitions(&row, 8, 1, 1, &thresholds));
        }
        let rows = [
            0, 255, 0, 255, 255, 255, 255, 255, 255, 0, 255, 0, 0, 0, 0, 0,
        ];
        assert!(!has_finder_transitions(&rows, 8, 2, 1, &[127; 16]));
    }

    #[test]
    #[cfg(feature = "qrcode-decode")]
    fn finder_transition_preflight_uses_each_blocks_threshold() {
        let pixels = [128; 10];
        assert!(has_finder_transitions(
            &pixels,
            10,
            1,
            2,
            &[127, 128, 127, 128, 127]
        ));
        assert!(!has_finder_transitions(&pixels, 10, 1, 2, &[128; 5]));
    }

    #[test]
    #[cfg(feature = "qrcode-decode")]
    fn finder_transition_preflight_skips_a_gradient_without_grids() {
        let (width, height) = (240, 120);
        let pixels: Vec<u8> = (0..width * height)
            .map(|index| ((index % width) * 255 / (width - 1)) as u8)
            .collect();
        for block in [10, 24] {
            let thresholds = local_thresholds(&pixels, width, height, block);
            assert!(!has_finder_transitions(
                &pixels,
                width,
                height,
                block,
                &thresholds
            ));
            let columns = width.div_ceil(block);
            let mut image = rqrr::PreparedImage::prepare_from_bitmap(width, height, |x, y| {
                pixels[y * width + x] <= thresholds[(y / block) * columns + x / block]
            });
            assert!(image.detect_grids().is_empty());
        }
    }

    #[test]
    #[cfg(feature = "qrcode-decode")]
    fn returns_no_payload_for_uniform_images_at_block_boundaries() {
        for (width, height) in [
            (1, 1),
            (19, 21),
            (20, 20),
            (21, 25),
            (39, 65),
            (40, 40),
            (41, 47),
            (79, 81),
            (80, 80),
            (81, 87),
        ] {
            for value in [0, 127, 255] {
                assert_eq!(
                    detect_payload_from_greyscale(&vec![value; width * height], width, height),
                    "",
                    "{width}x{height}, grayscale {value}"
                );
            }
        }
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
