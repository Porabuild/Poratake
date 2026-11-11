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
    pub trim_bottom: usize,
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
        let identical = self.frames.last().is_some_and(|previous| {
            previous.width == width && previous.height == height && previous.pixels == pixels
        });
        let fixed_footer = if identical {
            None
        } else {
            let known_footer = self
                .frames
                .iter()
                .rev()
                .nth(1)
                .map_or(0, |frame| frame.trim_bottom);
            self.frames.last().and_then(|previous| {
                find_fixed_footer(
                    previous,
                    &pixels,
                    width,
                    height,
                    expected_overlap,
                    known_footer,
                )
            })
        };
        let normal_overlap =
            if !identical && fixed_footer.is_none() && self.last_frame_hash == Some(hash) {
                self.frames.last().and_then(|previous| {
                    let row_bytes = width.checked_mul(4)?;
                    let min_overlap = 8_usize.min(height / 2).min(expected_overlap.max(1));
                    if row_bytes == 0
                        || min_overlap == 0
                        || previous.width != width
                        || previous.height != height
                        || previous.pixels.len() != row_bytes.checked_mul(height)?
                        || pixels.len() != previous.pixels.len()
                    {
                        return None;
                    }
                    let (overlap, alternative) =
                        exact_overlap(&previous.pixels, &pixels, width, min_overlap)?;
                    (alternative < min_overlap
                        && overlap < height
                        && has_scroll_motion(
                            &previous.pixels,
                            &pixels,
                            row_bytes,
                            overlap,
                            height - overlap,
                        ))
                    .then_some(overlap)
                })
            } else {
                None
            };
        if fixed_footer.is_none() && normal_overlap.is_none() && self.last_frame_hash == Some(hash)
        {
            self.duplicate_frame_count += 1;
            return if self.duplicate_frame_count >= MAX_DUPLICATE_FRAMES {
                CaptureOutcome::Ended
            } else {
                CaptureOutcome::Repeated
            };
        }
        if fixed_footer.is_none()
            && self
                .frames
                .iter()
                .rev()
                .nth(1)
                .is_some_and(|frame| frame.trim_bottom > 0)
        {
            return CaptureOutcome::NoOverlap;
        }

        let overlap = if let Some((_, overlap)) = fixed_footer {
            overlap
        } else if let Some(overlap) = normal_overlap {
            overlap
        } else if let Some(previous) = self.frames.last() {
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
        let trim_bottom = fixed_footer.map_or(0, |(footer, _)| footer);
        if let Some(previous) = self.frames.last_mut() {
            previous.trim_bottom = trim_bottom;
        }
        self.estimated_height = if self.frames.is_empty() {
            height
        } else {
            self.estimated_height
                .saturating_sub(trim_bottom)
                .saturating_add(height.saturating_sub(overlap))
        };
        self.frames.push(CapturedFrame {
            width,
            height,
            pixels,
            overlap,
            trim_bottom: 0,
        });
        CaptureOutcome::Added
    }
}

fn find_fixed_footer(
    previous: &CapturedFrame,
    current: &[u8],
    width: usize,
    height: usize,
    expected_overlap: usize,
    known_footer: usize,
) -> Option<(usize, usize)> {
    let row_bytes = width.checked_mul(4)?;
    if row_bytes == 0
        || previous.width != width
        || previous.height != height
        || previous.pixels.len() != row_bytes.checked_mul(height)?
        || current.len() != previous.pixels.len()
    {
        return None;
    }
    let detected_footer = previous
        .pixels
        .chunks_exact(row_bytes)
        .rev()
        .zip(current.chunks_exact(row_bytes).rev())
        .take_while(|(top, bottom)| top == bottom)
        .count();
    let min_overlap = 8_usize.min(height / 2).min(expected_overlap.max(1));
    if min_overlap == 0 {
        return None;
    }
    for footer in [
        known_footer.max(detected_footer),
        known_footer.min(detected_footer),
    ] {
        if footer == 0 || footer >= height.saturating_sub(previous.overlap) {
            continue;
        }
        let content_end = height - footer;
        for ignored_rows in 0..=usize::from(known_footer > 0) {
            let Some(match_end) = content_end.checked_sub(ignored_rows) else {
                continue;
            };
            let Some((matched, alternative)) = exact_overlap(
                &previous.pixels[..match_end * row_bytes],
                current,
                width,
                min_overlap,
            ) else {
                continue;
            };
            let overlap = matched + ignored_rows;
            if overlap >= content_end {
                continue;
            }
            if ignored_rows == 1 {
                let old_row = &previous.pixels[match_end * row_bytes..content_end * row_bytes];
                let new_row = &current[matched * row_bytes..overlap * row_bytes];
                if old_row
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(new_row.as_chunks::<4>().0)
                    .filter(|(old, new)| old != new)
                    .take(2)
                    .count()
                    != 1
                {
                    continue;
                }
            }
            if !has_scroll_motion(
                &previous.pixels,
                current,
                row_bytes,
                overlap,
                content_end - overlap,
            ) {
                continue;
            }
            if alternative >= min_overlap {
                let first_row = &current[..row_bytes];
                if current[..alternative * row_bytes]
                    .chunks_exact(row_bytes)
                    .any(|row| row != first_row)
                    || current[..overlap * row_bytes]
                        .chunks_exact(row_bytes)
                        .all(|row| row == first_row)
                {
                    continue;
                }
            }
            if known_footer == 0
                && exact_overlap(&previous.pixels, current, width, min_overlap).is_some()
            {
                continue;
            }
            return Some((footer, overlap));
        }
    }
    None
}

fn has_scroll_motion(
    previous: &[u8],
    current: &[u8],
    row_bytes: usize,
    overlap: usize,
    shift: usize,
) -> bool {
    let required = overlap / 2 + 1;
    let changed = previous
        .chunks_exact(row_bytes)
        .zip(current.chunks_exact(row_bytes))
        .take(overlap)
        .filter(|(top, bottom)| top != bottom)
        .take(required)
        .count();
    if changed == required {
        return true;
    }
    previous
        .chunks_exact(row_bytes)
        .zip(current.chunks_exact(row_bytes))
        .zip(
            previous
                .chunks_exact(row_bytes)
                .skip(shift)
                .zip(current.chunks_exact(row_bytes).skip(shift)),
        )
        .take(overlap)
        .any(|((old, new), (source, source_after))| {
            old != new && source == new && source != source_after
        })
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
        || width == 0
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
    let acceptable = best_score <= sampled_channels as u64 * MAX_OVERLAP_CHANNEL_DIFFERENCE;
    if acceptable && best_overlap == expected {
        return Some(best_overlap);
    }

    if let Some((exact, alternative)) =
        exact_overlap(&previous.pixels, current_pixels, width, min_overlap)
        && alternative < min_overlap
        && (!acceptable || exact.abs_diff(expected) <= best_overlap.abs_diff(expected))
    {
        return Some(exact);
    }
    acceptable.then_some(best_overlap)
}

fn exact_overlap(
    previous: &[u8],
    current: &[u8],
    width: usize,
    min_overlap: usize,
) -> Option<(usize, usize)> {
    let row_bytes = width * 4;
    let rows: Vec<_> = current.chunks_exact(row_bytes).collect();
    let mut prefix = vec![0; rows.len()];
    for index in 1..rows.len() {
        let mut matched = prefix[index - 1];
        while matched > 0 && rows[index] != rows[matched] {
            matched = prefix[matched - 1];
        }
        if rows[index] == rows[matched] {
            matched += 1;
        }
        prefix[index] = matched;
    }

    let mut matched = 0;
    for row in previous.chunks_exact(row_bytes) {
        while matched > 0 && row != rows[matched] {
            matched = prefix[matched - 1];
        }
        if row == rows[matched] {
            matched += 1;
        }
    }
    if matched < min_overlap {
        return None;
    }
    Some((matched, prefix[matched - 1]))
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
            || frame.trim_bottom > frame.height.saturating_sub(frame.overlap)
    }) {
        return Err("Captured frame dimensions do not match".to_string());
    }

    let total_height = frames
        .iter()
        .skip(1)
        .try_fold(first.height - first.trim_bottom, |height, frame| {
            height
                .checked_add(frame.height.saturating_sub(frame.overlap))?
                .checked_sub(frame.trim_bottom)
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
        (first_row..frame.height - frame.trim_bottom)
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

    fn unique_exact_overlap(
        previous: &[u8],
        current: &[u8],
        width: usize,
        min_overlap: usize,
    ) -> Option<usize> {
        match exact_overlap(previous, current, width, min_overlap) {
            Some((overlap, alternative)) if alternative < min_overlap => Some(overlap),
            _ => None,
        }
    }

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
            trim_bottom: 0,
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
    fn finds_exact_overlap_between_coarse_samples() {
        for (width, height, overlap) in [(1280, 320, 97), (800, 720, 217)] {
            let previous = frame(0, width, height);
            let current = frame(height - overlap, width, height);
            assert_eq!(
                find_overlap(&previous, &current.pixels, width, height, overlap),
                Some(overlap)
            );
        }
    }

    #[test]
    fn prefers_exact_overlap_when_sampled_candidates_are_equally_close() {
        let (width, height, overlap, expected) = (1000, 160, 101, 100);
        let make_frame = |start: usize| {
            let mut pixels = Vec::with_capacity(width * height * 4);
            for y in start..start + height {
                for x in 0..width {
                    let pixel = if x == 0 {
                        [y as u8, (y >> 8) as u8, y.wrapping_mul(17) as u8, 255]
                    } else {
                        [100 + (y % 2) as u8, 0, 0, 255]
                    };
                    pixels.extend_from_slice(&pixel);
                }
            }
            CapturedFrame {
                width,
                height,
                pixels,
                overlap: 0,
                trim_bottom: 0,
            }
        };
        let previous = make_frame(0);
        let current = make_frame(height - overlap);
        assert_eq!(
            find_overlap(&previous, &current.pixels, width, height, expected),
            Some(overlap)
        );
    }

    #[test]
    fn repeated_content_can_hide_independent_edge_motion() {
        let (width, height) = (1000, 160);
        let make_frame = |main_start: usize, edge_start: usize| {
            let mut pixels = Vec::with_capacity(width * height * 4);
            for y in 0..height {
                for x in 0..width {
                    let edge = edge_start + y;
                    let pixel = if x == 0 {
                        [
                            edge as u8,
                            (edge >> 8) as u8,
                            edge.wrapping_mul(17) as u8,
                            255,
                        ]
                    } else {
                        [(((main_start + y) % 8) * 31) as u8, 0, 0, 255]
                    };
                    pixels.extend_from_slice(&pixel);
                }
            }
            CapturedFrame {
                width,
                height,
                pixels,
                overlap: 0,
                trim_bottom: 0,
            }
        };
        let previous = make_frame(0, 0);
        let translated = make_frame(104, 104);
        let independently_moving_edge = make_frame(96, 104);
        assert_eq!(translated.pixels, independently_moving_edge.pixels);
        assert_eq!(
            unique_exact_overlap(&previous.pixels, &translated.pixels, width, 8),
            Some(56)
        );
    }

    #[test]
    fn exact_overlap_requires_a_unique_full_row_match() {
        let previous = [1, 2, 3, 4, 5]
            .into_iter()
            .flat_map(|value| [value, 0, 0, 255])
            .collect::<Vec<_>>();
        let current = [3, 4, 5, 6, 7]
            .into_iter()
            .flat_map(|value| [value, 0, 0, 255])
            .collect::<Vec<_>>();
        assert_eq!(unique_exact_overlap(&previous, &current, 1, 1), Some(3));

        let repeated = [0, 0, 0, 255].repeat(5);
        assert_eq!(unique_exact_overlap(&repeated, &repeated, 1, 1), None);
        assert_eq!(unique_exact_overlap(&previous, &current, 1, 4), None);

        let previous = [1, 2, 1, 2, 1]
            .into_iter()
            .flat_map(|value| [value, 0, 0, 255])
            .collect::<Vec<_>>();
        let current = [1, 2, 1, 3, 4]
            .into_iter()
            .flat_map(|value| [value, 0, 0, 255])
            .collect::<Vec<_>>();
        assert_eq!(unique_exact_overlap(&previous, &current, 1, 1), None);
        assert_eq!(unique_exact_overlap(&previous, &current, 1, 2), Some(3));
    }

    #[test]
    fn exact_overlap_matches_exhaustive_small_sequences() {
        let frames = (0..3_usize.pow(5))
            .map(|mut value| {
                (0..5)
                    .flat_map(|_| {
                        let pixel = [(value % 3) as u8, 0, 0, 255];
                        value /= 3;
                        pixel
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for previous in &frames {
            for current in &frames {
                for minimum in 1..=3 {
                    let mut matches = (minimum..=5)
                        .filter(|overlap| previous[(5 - overlap) * 4..] == current[..overlap * 4]);
                    let first = matches.next();
                    let expected = if matches.next().is_none() {
                        first
                    } else {
                        None
                    };
                    assert_eq!(
                        unique_exact_overlap(previous, current, 1, minimum),
                        expected
                    );
                }
            }
        }
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
    fn stationary_animation_does_not_look_like_a_fixed_footer() {
        let (width, height) = (96, 320);
        let mut previous = frame(0, width, height);
        let row_bytes = width * 4;
        for row in 80..=100 {
            previous.pixels.copy_within(
                (row - 80) * row_bytes..(row - 79) * row_bytes,
                row * row_bytes,
            );
        }
        let mut animated = previous.pixels.clone();
        animated[100 * row_bytes] ^= 255;
        assert_eq!(
            exact_overlap(&previous.pixels[..101 * row_bytes], &animated, width, 8),
            Some((21, 0))
        );
        assert_eq!(
            find_fixed_footer(&previous, &animated, width, height, 240, 0),
            None
        );
        let mut accumulator = FrameAccumulator::default();
        assert_eq!(
            accumulator.submit(previous.pixels, width, height, 240),
            CaptureOutcome::Added
        );
        assert_eq!(
            accumulator.submit(animated, width, height, 240),
            CaptureOutcome::Repeated
        );
    }

    #[test]
    fn sparse_stationary_animation_does_not_look_like_scrolling() {
        let (width, height) = (96, 160);
        let row_bytes = width * 4;
        let mut previous = frame(0, width, height);
        for row in 0..20 {
            let pattern = frame(row % 8, width, 1);
            previous.pixels[row * row_bytes..(row + 1) * row_bytes]
                .copy_from_slice(&pattern.pixels);
        }
        previous.pixels[13 * row_bytes] ^= 255;
        let mut animated = previous.pixels.clone();
        animated[5 * row_bytes] = previous.pixels[13 * row_bytes];
        animated[19 * row_bytes] ^= 255;
        let mut accumulator = FrameAccumulator::default();
        assert_eq!(
            accumulator.submit(previous.pixels, width, height, height - 8),
            CaptureOutcome::Added
        );
        assert_eq!(
            accumulator.submit(animated, width, height, height - 8),
            CaptureOutcome::Repeated
        );
    }

    #[test]
    fn stationary_pixel_after_footer_detection_is_not_scrolling() {
        let (width, height, footer, step) = (96, 160, 40, 70);
        let row_bytes = width * 4;
        let mut first = frame(0, width, height).pixels;
        let mut second = frame(step, width, height).pixels;
        second.copy_within(0..40 * row_bytes, 80 * row_bytes);
        for pixels in [&mut first, &mut second] {
            for row in height - footer..height {
                for pixel in pixels[row * row_bytes..(row + 1) * row_bytes]
                    .as_chunks_mut::<4>()
                    .0
                {
                    pixel.copy_from_slice(&[64, 64, 64, 255]);
                }
            }
        }
        let mut accumulator = FrameAccumulator::default();
        assert_eq!(
            accumulator.submit(first, width, height, height - step),
            CaptureOutcome::Added
        );
        assert_eq!(
            accumulator.submit(second.clone(), width, height, height - step),
            CaptureOutcome::Added
        );
        assert_eq!(accumulator.frames()[0].trim_bottom, footer);
        let mut animated = second.clone();
        animated[39 * row_bytes] ^= 255;
        assert_eq!(
            exact_overlap(&second[..119 * row_bytes], &animated, width, 8),
            Some((39, 0))
        );
        assert_eq!(
            accumulator.submit(animated, width, height, height - step),
            CaptureOutcome::Repeated
        );
        assert_eq!(accumulator.frames().len(), 2);
    }

    #[test]
    fn scrolling_into_a_blank_area_is_not_a_repeated_frame() {
        let (width, height, step) = (96, 160, 80);
        let make_frame = |start: usize| {
            let mut frame = frame(start, width, height);
            for row in 0..height {
                if start + row >= 110 {
                    for pixel in frame.pixels[row * width * 4..(row + 1) * width * 4]
                        .as_chunks_mut::<4>()
                        .0
                    {
                        pixel.copy_from_slice(&[64, 64, 64, 255]);
                    }
                }
            }
            frame.pixels
        };
        let mut accumulator = FrameAccumulator::default();
        assert_eq!(
            accumulator.submit(make_frame(0), width, height, height - step),
            CaptureOutcome::Added
        );
        assert_eq!(
            accumulator.submit(make_frame(step), width, height, height - step),
            CaptureOutcome::Added
        );
        assert_eq!(accumulator.frames()[0].trim_bottom, 0);
        assert_eq!(accumulator.frames()[1].overlap, height - step);
        assert_eq!(accumulator.estimated_height(), height + step);
    }

    #[test]
    fn a_single_scrolling_text_row_is_not_a_repeated_frame() {
        let (width, height, footer, step) = (96, 160, 40, 80);
        let make_frame = |start: usize| {
            let mut pixels = vec![255; width * height * 4];
            for y in 0..height {
                let color = if y >= height - footer {
                    Some([64, 64, 64, 255])
                } else if start + y == 100 {
                    Some([0, 0, 0, 255])
                } else {
                    None
                };
                if let Some(color) = color {
                    for pixel in pixels[y * width * 4..(y + 1) * width * 4]
                        .as_chunks_mut::<4>()
                        .0
                    {
                        pixel.copy_from_slice(&color);
                    }
                }
            }
            pixels
        };
        let mut accumulator = FrameAccumulator::default();
        for start in [0, step] {
            assert_eq!(
                accumulator.submit(make_frame(start), width, height, height - step),
                CaptureOutcome::Added
            );
        }
        assert_eq!(accumulator.estimated_height(), height + step);
        assert_eq!(
            accumulator.frames()[0].trim_bottom + accumulator.frames()[1].overlap,
            80
        );
    }

    #[test]
    fn multiple_changed_overlap_pixels_do_not_get_appended() {
        let (width, height, footer, step) = (96, 160, 40, 80);
        let make_frame = |start: usize| {
            let mut pixels = frame(start, width, height).pixels;
            for row in height - footer..height {
                for pixel in pixels[row * width * 4..(row + 1) * width * 4]
                    .as_chunks_mut::<4>()
                    .0
                {
                    pixel.copy_from_slice(&[64, 64, 64, 255]);
                }
            }
            pixels
        };
        let mut accumulator = FrameAccumulator::default();
        for start in [0, step] {
            assert_eq!(
                accumulator.submit(make_frame(start), width, height, height - step),
                CaptureOutcome::Added
            );
        }
        let mut next = make_frame(step * 2);
        let row = height - footer - step - 1;
        for x in [7, 8] {
            next[(row * width + x) * 4] ^= 255;
        }
        assert_eq!(
            accumulator.submit(next, width, height, height - step),
            CaptureOutcome::NoOverlap
        );
        assert_eq!(accumulator.frames().len(), 2);
    }

    #[test]
    fn fixed_footer_with_sparse_content_accepts_a_changed_later_footer() {
        let (width, height, footer, step) = (96, 160, 40, 80);
        let make_frame = |start: usize, animated_footer: bool| {
            let mut pixels = vec![255; width * height * 4];
            for y in 0..height {
                if y >= height - footer {
                    let value = if animated_footer { 96 } else { 64 };
                    pixels[y * width * 4..(y + 1) * width * 4].fill(value);
                } else {
                    let row = start + y;
                    pixels[y * width * 4..y * width * 4 + 4].copy_from_slice(&[
                        row as u8,
                        (row >> 8) as u8,
                        row.wrapping_mul(17) as u8,
                        255,
                    ]);
                }
            }
            pixels
        };
        let mut accumulator = FrameAccumulator::default();
        for start in [0, step] {
            assert_eq!(
                accumulator.submit(make_frame(start, false), width, height, height - step),
                CaptureOutcome::Added
            );
        }
        assert_eq!(accumulator.frames()[0].trim_bottom, footer);
        assert_eq!(accumulator.frames()[1].overlap, height - footer - step);
        assert_eq!(accumulator.estimated_height(), height + step);
        assert_eq!(
            accumulator.submit(make_frame(step * 2, true), width, height, height - step),
            CaptureOutcome::Added
        );
        assert_eq!(accumulator.frames()[1].trim_bottom, footer);
        assert_eq!(accumulator.frames()[2].overlap, height - footer - step);
        assert_eq!(accumulator.estimated_height(), height + step * 2);
        assert_eq!(
            accumulator.submit(make_frame(10_000, true), width, height, height - step),
            CaptureOutcome::NoOverlap
        );
        assert_eq!(accumulator.frames().len(), 3);
    }

    #[test]
    fn changed_footer_cannot_be_mistaken_for_a_later_body_overlap() {
        let (width, height, footer, step) = (96, 160, 40, 80);
        let make_frame = |start: usize, changed_footer: bool| {
            let mut pixels = frame(start, width, height).pixels;
            for y in 0..height {
                let color = if y >= height - footer {
                    Some([
                        (200 + y - (height - footer)) as u8,
                        u8::from(changed_footer),
                        0,
                        255,
                    ])
                } else if (step * 2..step * 2 + 8).contains(&(start + y)) {
                    Some([(232 + start + y - step * 2) as u8, 0, 0, 255])
                } else {
                    None
                };
                if let Some(color) = color {
                    for pixel in pixels[y * width * 4..(y + 1) * width * 4]
                        .as_chunks_mut::<4>()
                        .0
                    {
                        pixel.copy_from_slice(&color);
                    }
                }
            }
            pixels
        };
        let mut accumulator = FrameAccumulator::default();
        for start in [0, step] {
            assert_eq!(
                accumulator.submit(make_frame(start, false), width, height, height - step),
                CaptureOutcome::Added
            );
        }
        let next = make_frame(step * 2, true);
        assert_eq!(
            exact_overlap(&accumulator.frames()[1].pixels, &next, width, 8),
            Some((8, 0))
        );
        assert_eq!(
            accumulator.submit(next, width, height, height - step),
            CaptureOutcome::Added
        );
        assert_eq!(accumulator.frames()[1].trim_bottom, footer);
        assert_eq!(accumulator.frames()[2].overlap, height - footer - step);
        assert_eq!(accumulator.estimated_height(), height + step * 2);
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
