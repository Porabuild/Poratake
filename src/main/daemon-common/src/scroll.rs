use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::OnceLock;

use crate::contract::ScrollSpeed;

pub const FRAME_OVERLAP_PERCENT: usize = 30;
pub const MAX_DUPLICATE_FRAMES: usize = 3;
const MAX_OVERLAP_CHANNEL_DIFFERENCE: u64 = 18;
const WHEEL_LOGICAL_PIXELS: f64 = 48.0;

#[derive(Debug, PartialEq, Eq)]
pub struct ScrollPlan {
    pub target_logical_points: usize,
    pub wheel_detents: usize,
    pub interval_millis: u64,
}

pub fn scroll_interval_millis(speed: ScrollSpeed) -> u64 {
    match speed {
        ScrollSpeed::Slow => 40,
        ScrollSpeed::Medium => 30,
        ScrollSpeed::Fast => 20,
    }
}

pub fn scroll_plan(logical_viewport_height: f64, speed: ScrollSpeed) -> ScrollPlan {
    let target_logical_points = (logical_viewport_height * (100 - FRAME_OVERLAP_PERCENT) as f64
        / 100.0)
        .round()
        .max(1.0) as usize;
    let wheel_detents =
        ((target_logical_points as f64 / WHEEL_LOGICAL_PIXELS).ceil() as usize).max(1);
    let interval_millis = scroll_interval_millis(speed);

    ScrollPlan {
        target_logical_points,
        wheel_detents,
        interval_millis,
    }
}

pub struct CapturedFrame {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub overlap: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CaptureOutcome {
    Added,
    Repeated,
    Ended,
    NoOverlap,
}

#[derive(Default)]
pub struct FrameAccumulator {
    frames: Vec<CapturedFrame>,
    last_frame_hash: Option<u64>,
    duplicate_frame_count: usize,
    estimated_height: usize,
}

impl FrameAccumulator {
    pub fn frames(&self) -> &[CapturedFrame] {
        &self.frames
    }

    pub fn estimated_height(&self) -> usize {
        self.estimated_height
    }

    pub fn take_frames(&mut self) -> Vec<CapturedFrame> {
        let frames = std::mem::take(&mut self.frames);
        *self = Self::default();
        frames
    }

    pub fn submit(
        &mut self,
        pixels: Vec<u8>,
        width: usize,
        height: usize,
        expected_overlap: usize,
    ) -> CaptureOutcome {
        let hash = frame_hash(&pixels, width, height);
        if self.last_frame_hash == Some(hash) {
            self.duplicate_frame_count += 1;
            return if self.duplicate_frame_count >= MAX_DUPLICATE_FRAMES {
                CaptureOutcome::Ended
            } else {
                CaptureOutcome::Repeated
            };
        }

        let overlap = if let Some(previous) = self.frames.last() {
            let Some(overlap) = find_overlap(previous, &pixels, width, height, expected_overlap)
            else {
                return CaptureOutcome::NoOverlap;
            };
            overlap
        } else {
            0
        };
        if !self.frames.is_empty() && overlap == height {
            self.duplicate_frame_count += 1;
            return if self.duplicate_frame_count >= MAX_DUPLICATE_FRAMES {
                CaptureOutcome::Ended
            } else {
                CaptureOutcome::Repeated
            };
        }

        self.duplicate_frame_count = 0;
        self.last_frame_hash = Some(hash);
        self.estimated_height = if self.frames.is_empty() {
            height
        } else {
            self.estimated_height
                .saturating_add(height.saturating_sub(overlap))
        };
        self.frames.push(CapturedFrame {
            width,
            height,
            pixels,
            overlap,
        });
        CaptureOutcome::Added
    }
}

pub fn frame_hash(pixels: &[u8], width: usize, height: usize) -> u64 {
    let sample_width = 100_usize.min(width);
    let sample_height = 50_usize.min(height);
    if sample_width == 0 || sample_height == 0 {
        return 0;
    }
    let sample_top = height.saturating_sub((height / 4).max(sample_height));
    let sample_region_height = height - sample_top;
    let mut hash = 0xcbf29ce484222325_u64;

    for sample_y in 0..sample_height {
        let y = sample_top + sample_y * sample_region_height / sample_height;
        for sample_x in 0..sample_width {
            let x = sample_x * width / sample_width;
            let offset = (y * width + x) * 4;
            for channel in &pixels[offset..offset + 3] {
                hash ^= *channel as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
    }

    hash
}

pub fn find_overlap(
    previous: &CapturedFrame,
    current_pixels: &[u8],
    width: usize,
    height: usize,
    expected: usize,
) -> Option<usize> {
    let strip_height = 8_usize.min(height / 2).min(expected.max(1));
    if strip_height == 0
        || previous.width != width
        || previous.height != height
        || previous.pixels.len() != width.checked_mul(height)?.checked_mul(4)?
        || current_pixels.len() != width.checked_mul(height)?.checked_mul(4)?
    {
        return None;
    }

    let min_overlap = strip_height;
    let max_overlap = height;
    let strip_width = width.min(800);
    let start_x = (width - strip_width) / 2;
    let mut best_overlap = expected.clamp(min_overlap, max_overlap);
    let mut best_score = u64::MAX;

    let mut overlap = min_overlap;
    while overlap <= max_overlap {
        let score = overlap_score(
            &previous.pixels,
            current_pixels,
            width,
            height,
            overlap,
            start_x,
            strip_width,
            strip_height,
        );
        if score < best_score
            || (score == best_score && overlap.abs_diff(expected) < best_overlap.abs_diff(expected))
        {
            best_score = score;
            best_overlap = overlap;
        }
        overlap = overlap.saturating_add(4);
        if overlap == usize::MAX {
            break;
        }
    }

    let fine_start = best_overlap.saturating_sub(3).max(min_overlap);
    let fine_end = (best_overlap + 3).min(max_overlap);
    for overlap in fine_start..=fine_end {
        let score = overlap_score(
            &previous.pixels,
            current_pixels,
            width,
            height,
            overlap,
            start_x,
            strip_width,
            strip_height,
        );
        if score < best_score
            || (score == best_score && overlap.abs_diff(expected) < best_overlap.abs_diff(expected))
        {
            best_score = score;
            best_overlap = overlap;
        }
    }

    let sampled_channels = strip_width.div_ceil(2) * strip_height.div_ceil(2) * 3;
    (best_score <= sampled_channels as u64 * MAX_OVERLAP_CHANNEL_DIFFERENCE).then_some(best_overlap)
}

fn overlap_score(
    previous: &[u8],
    current: &[u8],
    width: usize,
    height: usize,
    overlap: usize,
    start_x: usize,
    strip_width: usize,
    strip_height: usize,
) -> u64 {
    if overlap == height && previous != current {
        return u64::MAX;
    }
    compare_overlap_strip(
        previous,
        current,
        width,
        height - overlap,
        0,
        start_x,
        strip_width,
        strip_height,
    )
}

fn compare_overlap_strip(
    previous: &[u8],
    current: &[u8],
    width: usize,
    previous_y: usize,
    current_y: usize,
    start_x: usize,
    strip_width: usize,
    strip_height: usize,
) -> u64 {
    let mut difference = 0_u64;
    for y in (0..strip_height).step_by(2) {
        for x in (0..strip_width).step_by(2) {
            let previous_offset = ((previous_y + y) * width + start_x + x) * 4;
            let current_offset = ((current_y + y) * width + start_x + x) * 4;
            for channel in 0..3 {
                difference += previous[previous_offset + channel]
                    .abs_diff(current[current_offset + channel])
                    as u64;
            }
        }
    }
    difference
}

pub fn stitched_dimensions(frames: &[CapturedFrame]) -> Result<(usize, usize), String> {
    let Some(first) = frames.first() else {
        return Err("No frames were captured".to_string());
    };

    if frames.iter().any(|frame| {
        frame.width != first.width
            || frame.height != first.height
            || frame.pixels.len()
                != frame
                    .width
                    .checked_mul(frame.height)
                    .and_then(|count| count.checked_mul(4))
                    .unwrap_or(usize::MAX)
    }) {
        return Err("Captured frame dimensions do not match".to_string());
    }

    let total_height = frames
        .iter()
        .skip(1)
        .try_fold(first.height, |height, frame| {
            height.checked_add(frame.height.saturating_sub(frame.overlap))
        })
        .ok_or_else(|| "Stitched image is too large".to_string())?;
    first
        .width
        .checked_mul(total_height)
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| "Stitched image is too large".to_string())?;

    Ok((first.width, total_height))
}

pub fn write_png(path: &Path, frames: &[CapturedFrame]) -> Result<(usize, usize), String> {
    let (width, height) = stitched_dimensions(frames)?;
    let width_u32 = u32::try_from(width).map_err(|_| "Image width is too large".to_string())?;
    let height_u32 = u32::try_from(height).map_err(|_| "Image height is too large".to_string())?;
    let file = File::create(path).map_err(|error| error.to_string())?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(&[137, 80, 78, 71, 13, 10, 26, 10])
        .map_err(|error| error.to_string())?;

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width_u32.to_be_bytes());
    header.extend_from_slice(&height_u32.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_png_chunk(&mut writer, b"IHDR", &header)?;
    write_png_pixels(&mut writer, width, height, frames)?;
    write_png_chunk(&mut writer, b"IEND", &[])?;
    writer.flush().map_err(|error| error.to_string())?;
    Ok((width, height))
}

fn write_png_chunk(
    writer: &mut impl Write,
    chunk_type: &[u8; 4],
    data: &[u8],
) -> Result<(), String> {
    let length = u32::try_from(data.len()).map_err(|_| "PNG chunk is too large".to_string())?;
    writer
        .write_all(&length.to_be_bytes())
        .and_then(|_| writer.write_all(chunk_type))
        .and_then(|_| writer.write_all(data))
        .map_err(|error| error.to_string())?;

    let mut crc = crc32_start();
    crc = crc32_update(crc, chunk_type);
    crc = crc32_update(crc, data);
    writer
        .write_all(&crc32_finish(crc).to_be_bytes())
        .map_err(|error| error.to_string())
}

fn write_png_pixels(
    writer: &mut impl Write,
    width: usize,
    height: usize,
    frames: &[CapturedFrame],
) -> Result<(), String> {
    let row_bytes = width
        .checked_mul(4)
        .ok_or_else(|| "PNG row is too large".to_string())?;
    let filtered_row_bytes = row_bytes + 1;
    let blocks_per_row = filtered_row_bytes.div_ceil(u16::MAX as usize);
    let raw_length = filtered_row_bytes
        .checked_mul(height)
        .ok_or_else(|| "PNG data is too large".to_string())?;
    let block_count = blocks_per_row
        .checked_mul(height)
        .ok_or_else(|| "PNG data is too large".to_string())?;
    let payload_length = raw_length
        .checked_add(block_count * 5)
        .and_then(|length| length.checked_add(6))
        .ok_or_else(|| "PNG data is too large".to_string())?;
    let payload_length =
        u32::try_from(payload_length).map_err(|_| "PNG data is too large".to_string())?;

    writer
        .write_all(&payload_length.to_be_bytes())
        .and_then(|_| writer.write_all(b"IDAT"))
        .map_err(|error| error.to_string())?;

    let mut crc = crc32_update(crc32_start(), b"IDAT");
    write_crc_bytes(writer, &mut crc, &[0x78, 0x01])?;
    let mut adler_a = 1_u32;
    let mut adler_b = 0_u32;

    let rows = frames.iter().enumerate().flat_map(|(index, frame)| {
        let first_row = if index == 0 {
            0
        } else {
            frame.overlap.min(frame.height)
        };
        (first_row..frame.height)
            .map(move |row| &frame.pixels[row * row_bytes..(row + 1) * row_bytes])
    });

    let mut filtered = Vec::with_capacity(filtered_row_bytes);
    for (row, pixels) in rows.enumerate() {
        filtered.clear();
        filtered.push(0);
        filtered.extend_from_slice(pixels);
        update_adler32(&mut adler_a, &mut adler_b, &filtered);

        let mut offset = 0;
        while offset < filtered.len() {
            let block_length = (filtered.len() - offset).min(u16::MAX as usize);
            let is_final = row + 1 == height && offset + block_length == filtered.len();
            let length = block_length as u16;
            let header = [
                u8::from(is_final),
                length as u8,
                (length >> 8) as u8,
                !length as u8,
                (!length >> 8) as u8,
            ];
            write_crc_bytes(writer, &mut crc, &header)?;
            write_crc_bytes(writer, &mut crc, &filtered[offset..offset + block_length])?;
            offset += block_length;
        }
    }

    let adler = (adler_b << 16) | adler_a;
    write_crc_bytes(writer, &mut crc, &adler.to_be_bytes())?;
    writer
        .write_all(&crc32_finish(crc).to_be_bytes())
        .map_err(|error| error.to_string())
}

fn write_crc_bytes(writer: &mut impl Write, crc: &mut u32, bytes: &[u8]) -> Result<(), String> {
    writer.write_all(bytes).map_err(|error| error.to_string())?;
    *crc = crc32_update(*crc, bytes);
    Ok(())
}

fn update_adler32(a: &mut u32, b: &mut u32, bytes: &[u8]) {
    for chunk in bytes.chunks(5_552) {
        for byte in chunk {
            *a += *byte as u32;
            *b += *a;
        }
        *a %= 65_521;
        *b %= 65_521;
    }
}

fn crc32_start() -> u32 {
    u32::MAX
}

fn crc32_update(mut crc: u32, bytes: &[u8]) -> u32 {
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut table = [0_u32; 256];
        for (index, entry) in table.iter_mut().enumerate() {
            let mut value = index as u32;
            for _ in 0..8 {
                let mask = 0_u32.wrapping_sub(value & 1);
                value = (value >> 1) ^ (0xedb88320 & mask);
            }
            *entry = value;
        }
        table
    });

    for byte in bytes {
        crc = table[((crc ^ *byte as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    crc
}

fn crc32_finish(crc: u32) -> u32 {
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(start_row: usize, width: usize, height: usize) -> CapturedFrame {
        let mut pixels = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                let seed = ((start_row + y) as u32).wrapping_mul(0x45d9f3b)
                    ^ (x as u32).wrapping_mul(0x119de1f3);
                pixels.extend_from_slice(&[
                    seed as u8,
                    seed.rotate_left(9) as u8,
                    seed.rotate_left(17) as u8,
                    255,
                ]);
            }
        }
        CapturedFrame {
            width,
            height,
            pixels,
            overlap: 0,
        }
    }

    #[test]
    fn finds_overlap_between_translated_frames() {
        let width = 96;
        let height = 160;
        let overlap = 52;
        let previous = frame(0, width, height);
        let current = frame(height - overlap, width, height);

        assert_eq!(
            find_overlap(&previous, &current.pixels, width, height, 48),
            Some(overlap)
        );
    }

    #[test]
    fn finds_expected_overlap_in_a_short_viewport() {
        let width = 96;
        let height = 80;
        let overlap = 24;
        let previous = frame(0, width, height);
        let current = frame(height - overlap, width, height);

        assert_eq!(
            find_overlap(&previous, &current.pixels, width, height, overlap),
            Some(overlap)
        );
    }

    #[test]
    fn fixed_footer_does_not_make_changed_frames_look_repeated() {
        let width = 96;
        let height = 160;
        let overlap = 52;
        let mut previous = frame(0, width, height);
        let mut current = frame(height - overlap, width, height);
        for pixels in [&mut previous.pixels, &mut current.pixels] {
            for row in pixels.chunks_exact_mut(width * 4).skip(height - 40) {
                row.fill(64);
            }
        }

        assert_eq!(
            find_overlap(&previous, &current.pixels, width, height, overlap),
            Some(overlap)
        );
    }

    #[test]
    fn recognizes_repeated_frame_content() {
        let width = 96;
        let height = 160;
        let previous = frame(0, width, height);
        let repeated = frame(0, width, height);

        assert_eq!(
            frame_hash(&previous.pixels, width, height),
            frame_hash(&repeated.pixels, width, height)
        );
        assert_eq!(
            find_overlap(&previous, &repeated.pixels, width, height, 48),
            Some(height)
        );
    }

    #[test]
    fn rejects_frames_without_confident_overlap() {
        let width = 96;
        let height = 160;
        let previous = frame(0, width, height);
        let unrelated = frame(10_000, width, height);

        assert_eq!(
            find_overlap(&previous, &unrelated.pixels, width, height, 48),
            None
        );
    }

    #[test]
    fn writes_the_stitched_wire_image() {
        let first = frame(0, 4, 4);
        let mut second = frame(2, 4, 4);
        second.overlap = 2;
        let path = std::env::temp_dir().join(format!(
            "poratake-scroll-common-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));

        assert_eq!(write_png(&path, &[first, second]), Ok((4, 6)));
        let bytes = std::fs::read(&path).expect("stitched png");
        std::fs::remove_file(path).expect("remove stitched png");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn plans_equal_displacement_across_speeds() {
        let slow = scroll_plan(1_000.0, ScrollSpeed::Slow);
        let medium = scroll_plan(1_000.0, ScrollSpeed::Medium);
        let fast = scroll_plan(1_000.0, ScrollSpeed::Fast);

        assert_eq!(slow.target_logical_points, 700);
        assert_eq!(slow.wheel_detents, medium.wheel_detents);
        assert_eq!(medium.wheel_detents, fast.wheel_detents);
        assert!(slow.interval_millis > medium.interval_millis);
        assert!(medium.interval_millis > fast.interval_millis);
    }
}
