use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

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
pub enum PngWriteError {
    Cancelled,
    Failed(String),
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
        if let Some((footer, _)) = fixed_footer
            && footer >= expected_overlap
            && self.frames.last().is_some_and(|previous| {
                has_multiple_zero_score_overlaps(previous, &pixels, width, height, expected_overlap)
            })
        {
            return CaptureOutcome::NoOverlap;
        }
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
        if fixed_footer.is_none()
            && normal_overlap.is_none()
            && (self.last_frame_hash == Some(hash)
                || self.frames.last().is_some_and(|previous| {
                    let min_overlap = 8_usize.min(height / 2).min(expected_overlap.max(1));
                    previous.width == width
                        && previous.height == height
                        && previous.pixels.len() == pixels.len()
                        && width != 0
                        && min_overlap != 0
                        && previous
                            .pixels
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .zip(pixels.as_chunks::<4>().0)
                            .filter(|(old, new)| old != new)
                            .take(2)
                            .count()
                            == 1
                        && exact_overlap(&previous.pixels, &pixels, width, min_overlap).is_none()
                }))
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
        if let Some((footer, overlap)) = fixed_footer
            && let Some(previous) = self.frames.last()
        {
            let expected = expected_overlap.saturating_sub(footer);
            let strip_height = 8_usize.min(height / 2).min(expected_overlap.max(1));
            let content_end = height - footer;
            let strip_width = width.min(800);
            if expected >= strip_height
                && expected < content_end
                && expected != overlap
                && (compare_overlap_strip(
                    &previous.pixels,
                    &pixels,
                    width,
                    content_end - expected,
                    0,
                    (width - strip_width) / 2,
                    strip_width,
                    strip_height,
                ) == 0
                    || overlap_matches_except_two_columns(
                        &previous.pixels,
                        &pixels,
                        width,
                        content_end,
                        expected,
                        strip_height,
                    ))
            {
                return CaptureOutcome::NoOverlap;
            }
        }
        if let Some(overlap) = normal_overlap
            && overlap != expected_overlap
            && self.frames.last().is_some_and(|previous| {
                find_overlap_after_footer_arbitration(
                    previous,
                    &pixels,
                    width,
                    height,
                    expected_overlap,
                )
                .is_none()
            })
        {
            return CaptureOutcome::NoOverlap;
        }

        let overlap = if let Some((_, overlap)) = fixed_footer {
            overlap
        } else if let Some(overlap) = normal_overlap {
            overlap
        } else if let Some(previous) = self.frames.last() {
            let Some(overlap) = find_overlap_after_footer_arbitration(
                previous,
                &pixels,
                width,
                height,
                expected_overlap,
            ) else {
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

fn has_multiple_zero_score_overlaps(
    previous: &CapturedFrame,
    current: &[u8],
    width: usize,
    height: usize,
    expected: usize,
) -> bool {
    let strip_height = 8_usize.min(height / 2).min(expected.max(1));
    let strip_width = width.min(800);
    let start_x = (width - strip_width) / 2;
    let mut zero_score_count = 0;
    for overlap in strip_height..=height {
        if overlap == height && previous.pixels != current {
            continue;
        }
        let mut equal = true;
        'pixels: for y in (0..strip_height).step_by(2) {
            for x in (0..strip_width).step_by(2) {
                let previous_offset = ((height - overlap + y) * width + start_x + x) * 4;
                let current_offset = (y * width + start_x + x) * 4;
                if previous.pixels[previous_offset..previous_offset + 3]
                    != current[current_offset..current_offset + 3]
                {
                    equal = false;
                    break 'pixels;
                }
            }
        }
        if equal {
            zero_score_count += 1;
            if zero_score_count > 1 {
                return true;
            }
        }
    }
    false
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
    let footers = [
        known_footer.max(detected_footer),
        known_footer.min(detected_footer),
    ]
    .into_iter()
    .chain(
        std::iter::once_with(|| {
            if detected_footer < min_overlap {
                return [0, 0];
            }
            let tolerant_footer = previous
                .pixels
                .chunks_exact(row_bytes)
                .rev()
                .zip(current.chunks_exact(row_bytes).rev())
                .take_while(|(top, bottom)| {
                    top.as_chunks::<4>()
                        .0
                        .iter()
                        .zip(bottom.as_chunks::<4>().0)
                        .filter(|(old, new)| old != new)
                        .take(width / 64 + 1)
                        .count()
                        <= width / 64
                })
                .count();
            [
                known_footer.max(tolerant_footer),
                known_footer.min(tolerant_footer),
            ]
        })
        .flatten(),
    );
    let mut deferred = Vec::new();
    for approximate_pass in [false, true] {
        for footer in footers.clone() {
            if footer == 0 || footer >= height.saturating_sub(previous.overlap) {
                continue;
            }
            let content_end = height - footer;
            for ignored_rows in 0..=usize::from(known_footer > 0) {
                let Some(match_end) = content_end.checked_sub(ignored_rows) else {
                    continue;
                };
                let matched = if approximate_pass {
                    if ignored_rows != 0 || !deferred.contains(&footer) {
                        continue;
                    }
                    expected_overlap.checked_sub(footer).and_then(|expected| {
                        find_localized_overlap(
                            &previous.pixels,
                            current,
                            width,
                            match_end,
                            expected,
                            min_overlap,
                        )
                        .map(|overlap| (overlap, 0))
                    })
                } else {
                    let result = exact_overlap(
                        &previous.pixels[..match_end * row_bytes],
                        current,
                        width,
                        min_overlap,
                    );
                    if result.is_none() && ignored_rows == 0 {
                        deferred.push(footer);
                    }
                    result
                };
                let Some((matched, alternative)) = matched else {
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
                if approximate_pass
                    && known_footer == 0
                    && expected_overlap > 0
                    && expected_overlap < height
                    && overlap_has_localized_changes(
                        &previous.pixels,
                        current,
                        width,
                        height,
                        expected_overlap,
                    )
                {
                    continue;
                }
                return Some((footer, overlap));
            }
        }
    }
    None
}

fn find_localized_overlap(
    previous: &[u8],
    current: &[u8],
    width: usize,
    previous_end: usize,
    expected: usize,
    min_overlap: usize,
) -> Option<usize> {
    if width < 64 || expected < min_overlap * 2 || expected >= previous_end {
        return None;
    }
    let row_bytes = width * 4;
    for offset in [expected / 2, expected * 3 / 4, expected - min_overlap] {
        let Some((tail, alternative)) = exact_overlap(
            &previous[..previous_end * row_bytes],
            &current[offset * row_bytes..],
            width,
            min_overlap,
        ) else {
            continue;
        };
        if alternative >= min_overlap {
            continue;
        }
        let overlap = offset + tail;
        if overlap >= previous_end {
            continue;
        }
        if overlap_has_localized_changes(previous, current, width, previous_end, overlap) {
            return Some(overlap);
        }
    }
    None
}

fn overlap_has_localized_changes(
    previous: &[u8],
    current: &[u8],
    width: usize,
    previous_end: usize,
    overlap: usize,
) -> bool {
    let row_bytes = width * 4;
    let old = &previous[(previous_end - overlap) * row_bytes..previous_end * row_bytes];
    let new = &current[..overlap * row_bytes];
    let mut diffs = old
        .as_chunks::<4>()
        .0
        .iter()
        .zip(new.as_chunks::<4>().0)
        .enumerate()
        .filter_map(|(index, (old, new))| (old != new).then_some((index % width, index / width)));
    let Some((first_x, first_y)) = diffs.next() else {
        return true;
    };
    let (mut left, mut right, mut top, mut bottom) = (first_x, first_x, first_y, first_y);
    for (x, y) in diffs {
        left = left.min(x);
        right = right.max(x);
        top = top.min(y);
        bottom = bottom.max(y);
        if right - left + 1 > width / 64 || bottom - top + 1 > overlap / 2 {
            return false;
        }
    }
    true
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
    find_overlap_impl(previous, current_pixels, width, height, expected, false)
}

pub(super) fn find_overlap_after_footer_arbitration(
    previous: &CapturedFrame,
    current_pixels: &[u8],
    width: usize,
    height: usize,
    expected: usize,
) -> Option<usize> {
    find_overlap_impl(previous, current_pixels, width, height, expected, true)
}

fn find_overlap_impl(
    previous: &CapturedFrame,
    current_pixels: &[u8],
    width: usize,
    height: usize,
    expected: usize,
    reject_ambiguous_perfect_sample: bool,
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
        let row_bytes = width * 4;
        if previous.pixels[(height - best_overlap) * row_bytes..]
            == current_pixels[..best_overlap * row_bytes]
        {
            return Some(best_overlap);
        }
    }

    let exact_match = exact_overlap(&previous.pixels, current_pixels, width, min_overlap);
    if let Some((exact, _)) = exact_match
        && acceptable
        && best_score == 0
        && exact != best_overlap
    {
        let mut differences = previous.pixels[(height - best_overlap) * width * 4..]
            .as_chunks::<4>()
            .0
            .iter()
            .zip(
                current_pixels[..best_overlap * width * 4]
                    .as_chunks::<4>()
                    .0
                    .iter(),
            )
            .enumerate()
            .filter_map(|(index, (previous, current))| (previous != current).then_some(index));
        if let Some(first) = differences.next() {
            let mut different_rows = false;
            let mut different_columns = false;
            for index in differences {
                different_rows |= index / width != first / width;
                different_columns |= index % width != first % width;
                if different_rows && different_columns {
                    return None;
                }
            }
        }
    }
    if let Some((exact, alternative)) = exact_match
        && alternative < min_overlap
    {
        if exact != expected
            && expected >= min_overlap
            && expected < height
            && ((best_overlap == expected && best_score == 0)
                || overlap_matches_except_two_columns(
                    &previous.pixels,
                    current_pixels,
                    width,
                    height,
                    expected,
                    strip_height,
                ))
        {
            return None;
        }
        if !acceptable || exact.abs_diff(expected) <= best_overlap.abs_diff(expected) {
            return Some(exact);
        }
    }
    if reject_ambiguous_perfect_sample && best_score == 0 && exact_match.is_none() {
        let row_bytes = width * 4;
        let overlap_pixels = best_overlap.checked_mul(width)?;
        let max_changed_pixels = overlap_pixels / 20;
        let previous_overlap =
            &previous.pixels[(height - best_overlap) * row_bytes..height * row_bytes];
        let current_overlap = &current_pixels[..best_overlap * row_bytes];
        let mut changed_pixels = 0;
        for (previous, current) in previous_overlap
            .as_chunks::<4>()
            .0
            .iter()
            .zip(current_overlap.as_chunks::<4>().0)
        {
            if previous[..3] == current[..3] {
                continue;
            }
            changed_pixels += 1;
            if changed_pixels > max_changed_pixels {
                return None;
            }
        }
    }
    acceptable.then_some(best_overlap)
}

fn overlap_matches_except_two_columns(
    previous: &[u8],
    current: &[u8],
    width: usize,
    previous_end: usize,
    overlap: usize,
    strip_height: usize,
) -> bool {
    let strip_width = width.min(800);
    let start_x = (width - strip_width) / 2;
    let sampled_columns = strip_width.div_ceil(2);
    let mut different_columns: [Option<(usize, u8)>; 2] = [None, None];
    for (region, offset) in [0, (overlap - strip_height) / 2, overlap - strip_height]
        .into_iter()
        .enumerate()
    {
        for y in (0..strip_height).step_by(2) {
            for x in (0..strip_width).step_by(2) {
                let previous_offset =
                    ((previous_end - overlap + offset + y) * width + start_x + x) * 4;
                let current_offset = ((offset + y) * width + start_x + x) * 4;
                if previous[previous_offset..previous_offset + 3]
                    != current[current_offset..current_offset + 3]
                {
                    let column = x / 2;
                    let index = match different_columns
                        .iter()
                        .position(|entry| entry.is_some_and(|(existing, _)| existing == column))
                    {
                        Some(index) => index,
                        None => {
                            let Some(index) = different_columns.iter().position(Option::is_none)
                            else {
                                return false;
                            };
                            if (index + 1) * 2 > sampled_columns {
                                return false;
                            }
                            index
                        }
                    };
                    if let Some((_, regions)) = different_columns[index].as_mut() {
                        *regions |= 1 << region;
                    } else {
                        different_columns[index] = Some((column, 1 << region));
                    }
                }
            }
        }
    }
    if overlap < strip_height * 2 || different_columns.iter().all(Option::is_none) {
        return true;
    }
    different_columns
        .iter()
        .flatten()
        .all(|(_, regions)| regions & 0b101 == 0b101)
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
        while matched > 0 && (matched == rows.len() || row != rows[matched]) {
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
    let mut never_cancelled = || false;
    write_png_with_cancel_check(path, frames, &mut never_cancelled).map_err(|error| match error {
        PngWriteError::Cancelled => "PNG write was cancelled unexpectedly".to_string(),
        PngWriteError::Failed(message) => message,
    })
}

pub fn write_png_cancellable(
    path: &Path,
    frames: &[CapturedFrame],
    cancelled: &AtomicBool,
) -> Result<(usize, usize), PngWriteError> {
    let mut is_cancelled = || cancelled.load(Ordering::Acquire);
    write_png_with_cancel_check(path, frames, &mut is_cancelled)
}

fn write_png_with_cancel_check(
    path: &Path,
    frames: &[CapturedFrame],
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(usize, usize), PngWriteError> {
    let (width, height) = stitched_dimensions(frames).map_err(PngWriteError::Failed)?;
    let width_u32 = u32::try_from(width)
        .map_err(|_| PngWriteError::Failed("Image width is too large".to_string()))?;
    let height_u32 = u32::try_from(height)
        .map_err(|_| PngWriteError::Failed("Image height is too large".to_string()))?;
    if is_cancelled() {
        return Err(PngWriteError::Cancelled);
    }
    let file = File::create(path).map_err(|error| PngWriteError::Failed(error.to_string()))?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(&[137, 80, 78, 71, 13, 10, 26, 10])
        .map_err(|error| PngWriteError::Failed(error.to_string()))?;

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width_u32.to_be_bytes());
    header.extend_from_slice(&height_u32.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_png_chunk(&mut writer, b"IHDR", &header).map_err(PngWriteError::Failed)?;
    write_png_pixels(&mut writer, width, height, frames, is_cancelled)?;
    if is_cancelled() {
        return Err(PngWriteError::Cancelled);
    }
    write_png_chunk(&mut writer, b"IEND", &[]).map_err(PngWriteError::Failed)?;
    writer
        .flush()
        .map_err(|error| PngWriteError::Failed(error.to_string()))?;
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
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), PngWriteError> {
    let row_bytes = width
        .checked_mul(4)
        .ok_or_else(|| PngWriteError::Failed("PNG row is too large".to_string()))?;
    let filtered_row_bytes = row_bytes + 1;
    let blocks_per_row = filtered_row_bytes.div_ceil(u16::MAX as usize);
    let raw_length = filtered_row_bytes
        .checked_mul(height)
        .ok_or_else(|| PngWriteError::Failed("PNG data is too large".to_string()))?;
    let block_count = blocks_per_row
        .checked_mul(height)
        .ok_or_else(|| PngWriteError::Failed("PNG data is too large".to_string()))?;
    let payload_length = raw_length
        .checked_add(block_count * 5)
        .and_then(|length| length.checked_add(6))
        .ok_or_else(|| PngWriteError::Failed("PNG data is too large".to_string()))?;
    let payload_length = u32::try_from(payload_length)
        .map_err(|_| PngWriteError::Failed("PNG data is too large".to_string()))?;

    writer
        .write_all(&payload_length.to_be_bytes())
        .and_then(|_| writer.write_all(b"IDAT"))
        .map_err(|error| PngWriteError::Failed(error.to_string()))?;

    let mut crc = crc32_update(crc32_start(), b"IDAT");
    write_crc_bytes(writer, &mut crc, &[0x78, 0x01]).map_err(PngWriteError::Failed)?;
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
        if is_cancelled() {
            return Err(PngWriteError::Cancelled);
        }
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
            write_crc_bytes(writer, &mut crc, &header).map_err(PngWriteError::Failed)?;
            write_crc_bytes(writer, &mut crc, &filtered[offset..offset + block_length])
                .map_err(PngWriteError::Failed)?;
            offset += block_length;
        }
    }

    if is_cancelled() {
        return Err(PngWriteError::Cancelled);
    }
    let adler = (adler_b << 16) | adler_a;
    write_crc_bytes(writer, &mut crc, &adler.to_be_bytes()).map_err(PngWriteError::Failed)?;
    writer
        .write_all(&crc32_finish(crc).to_be_bytes())
        .map_err(|error| PngWriteError::Failed(error.to_string()))
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

    fn periodic_body_frame(index: usize, footer_height: usize) -> Vec<u8> {
        let (width, height, step, side) = (640, 160, 96, 10);
        let mut pixels = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                let mut pixel = if y >= height - footer_height {
                    [
                        62,
                        ((y - (height - footer_height)) % 251) as u8,
                        (x % 251) as u8,
                        255,
                    ]
                } else {
                    let absolute_y = y + index * step;
                    if x == 0 && absolute_y % 256 < 32 {
                        let value = (absolute_y as u32).wrapping_mul(0x45d9f3b);
                        [
                            value as u8,
                            value.rotate_left(9) as u8,
                            value.rotate_left(17) as u8,
                            255,
                        ]
                    } else {
                        [((absolute_y % 8) * 30) as u8, 0, 0, 255]
                    }
                };
                if (1..1 + side).contains(&x) && (32..32 + side).contains(&y) {
                    pixel[0] ^= 51 * index as u8;
                }
                pixels.extend_from_slice(&pixel);
            }
        }
        pixels
    }

    #[test]
    fn stops_on_periodic_body_mistaken_for_a_twenty_pixel_footer() {
        assert_periodic_body_footer_alias_stops(20);
    }

    #[test]
    fn stops_on_periodic_body_mistaken_for_a_forty_pixel_footer() {
        assert_periodic_body_footer_alias_stops(40);
    }

    #[test]
    fn zero_score_overlap_guard_matches_overlap_score_semantics() {
        for (width, height, expected) in [(1, 9, 1), (7, 16, 5), (19, 21, 20)] {
            for pattern in 0..4 {
                let mut previous = Vec::with_capacity(width * height * 4);
                let mut current = Vec::with_capacity(width * height * 4);
                for y in 0..height {
                    for x in 0..width {
                        let value = (if pattern == 1 { y % 3 } else { y * width + x }) as u32;
                        let seed = value
                            .wrapping_mul(0x45d9f3b)
                            .wrapping_add((x as u32).rotate_left(11));
                        let pixel = [
                            seed as u8,
                            seed.rotate_left(9) as u8,
                            seed.rotate_left(17) as u8,
                            (y + x) as u8,
                        ];
                        previous.extend_from_slice(&pixel);
                        let mut next = pixel;
                        match pattern {
                            0 => next[0] ^= 0x5a,
                            2 => next[3] ^= 0x5a,
                            3 if x == 0 && y == 0 => next[1] ^= 0x5a,
                            _ => {}
                        }
                        current.extend_from_slice(&next);
                    }
                }

                let strip_height = 8_usize.min(height / 2).min(expected.max(1));
                let strip_width = width.min(800);
                let start_x = (width - strip_width) / 2;
                let expected_ambiguous = (strip_height..=height)
                    .filter(|&overlap| {
                        overlap_score(
                            &previous,
                            &current,
                            width,
                            height,
                            overlap,
                            start_x,
                            strip_width,
                            strip_height,
                        ) == 0
                    })
                    .take(2)
                    .count()
                    > 1;
                let previous_frame = CapturedFrame {
                    width,
                    height,
                    pixels: previous,
                    overlap: 0,
                    trim_bottom: 0,
                };

                assert_eq!(
                    has_multiple_zero_score_overlaps(
                        &previous_frame,
                        &current,
                        width,
                        height,
                        expected,
                    ),
                    expected_ambiguous,
                    "pattern={pattern} size={width}x{height} expected={expected}"
                );
            }
        }
    }

    fn assert_periodic_body_footer_alias_stops(footer_height: usize) {
        let (width, height, step) = (640, 160, 96);
        let mut accumulator = FrameAccumulator::default();
        assert_eq!(
            accumulator.submit(
                periodic_body_frame(0, footer_height),
                width,
                height,
                height - step
            ),
            CaptureOutcome::Added
        );
        assert_eq!(
            accumulator.submit(
                periodic_body_frame(1, footer_height),
                width,
                height,
                height - step
            ),
            CaptureOutcome::NoOverlap
        );
        assert_eq!(accumulator.frames().len(), 1);
        assert_eq!(accumulator.frames()[0].trim_bottom, 0);
        assert_eq!(accumulator.estimated_height(), height);
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
            exact_overlap(&previous.pixels, &current.pixels, width, 8),
            Some((overlap, 0))
        );
        assert_eq!(
            find_overlap(&previous, &current.pixels, width, height, expected),
            Some(overlap)
        );
        assert_eq!(
            find_overlap_after_footer_arbitration(
                &previous,
                &current.pixels,
                width,
                height,
                expected,
            ),
            Some(overlap)
        );
    }

    #[test]
    fn stops_on_a_sampled_perfect_fixed_footer_alias_and_keeps_prior_frames() {
        let (width, height, footer, step) = (64, 160, 20, 8);
        let expected_overlap = height - step;
        let make_frame = |index: usize| {
            let mut pixels = Vec::with_capacity(width * height * 4);
            for y in 0..height {
                for x in 0..width {
                    let mut pixel = if y >= height - footer {
                        [62, (y - (height - footer)) as u8, x as u8, 255]
                    } else if x == 0 {
                        let value = (y + index * step) as u32;
                        [
                            value as u8,
                            (value >> 8) as u8,
                            value.wrapping_mul(37) as u8,
                            255,
                        ]
                    } else {
                        [235, 235, 235, 255]
                    };
                    if x == 1 && y == height - footer / 2 {
                        pixel[0] ^= 51 * index as u8;
                    }
                    pixels.extend_from_slice(&pixel);
                }
            }
            pixels
        };
        let previous_pixels = make_frame(0);
        let current_pixels = make_frame(1);
        let previous = CapturedFrame {
            width,
            height,
            pixels: previous_pixels.clone(),
            overlap: 0,
            trim_bottom: 0,
        };

        assert_eq!(
            find_fixed_footer(
                &previous,
                &current_pixels,
                width,
                height,
                expected_overlap,
                0
            ),
            None
        );
        assert_eq!(
            overlap_score(
                &previous.pixels,
                &current_pixels,
                width,
                height,
                expected_overlap,
                0,
                width,
                8,
            ),
            0
        );
        assert_eq!(
            exact_overlap(&previous.pixels, &current_pixels, width, 8),
            None
        );
        assert_eq!(
            find_overlap(&previous, &current_pixels, width, height, expected_overlap),
            Some(expected_overlap)
        );
        assert_eq!(
            find_overlap_after_footer_arbitration(
                &previous,
                &current_pixels,
                width,
                height,
                expected_overlap,
            ),
            None
        );

        let mut accumulator = FrameAccumulator::default();
        assert_eq!(
            accumulator.submit(previous_pixels.clone(), width, height, expected_overlap),
            CaptureOutcome::Added
        );
        assert_eq!(
            accumulator.submit(current_pixels, width, height, expected_overlap),
            CaptureOutcome::NoOverlap
        );
        assert_eq!(accumulator.frames().len(), 1);
        assert_eq!(accumulator.estimated_height(), height);
        assert_eq!(accumulator.frames()[0].pixels, previous_pixels);
    }

    #[test]
    fn accepts_a_small_localized_change_when_sampled_overlap_is_perfect() {
        let (width, height, overlap) = (64, 160, 96);
        let previous = frame(0, width, height);
        let mut current = frame(height - overlap, width, height);
        for y in 40..48 {
            current.pixels[(y * width + 1) * 4] ^= 1;
        }

        assert_eq!(
            overlap_score(
                &previous.pixels,
                &current.pixels,
                width,
                height,
                overlap,
                0,
                width,
                8,
            ),
            0
        );
        assert_eq!(
            exact_overlap(&previous.pixels, &current.pixels, width, 8),
            None
        );
        assert_eq!(
            find_overlap(&previous, &current.pixels, width, height, overlap),
            Some(overlap)
        );
        assert_eq!(
            find_overlap_after_footer_arbitration(
                &previous,
                &current.pixels,
                width,
                height,
                overlap,
            ),
            Some(overlap)
        );

        let mut accumulator = FrameAccumulator::default();
        assert_eq!(
            accumulator.submit(previous.pixels.clone(), width, height, overlap),
            CaptureOutcome::Added
        );
        assert_eq!(
            accumulator.submit(current.pixels, width, height, overlap),
            CaptureOutcome::Added
        );
        assert_eq!(accumulator.frames().len(), 2);
        assert_eq!(accumulator.frames()[1].overlap, overlap);
        assert_eq!(accumulator.estimated_height(), height + height - overlap);
    }

    #[test]
    fn accepts_exactly_five_percent_overlap_changes_and_rejects_more() {
        let (width, height, overlap) = (100, 160, 80);
        let previous = frame(0, width, height);
        let make_current = |changed_pixels: usize| {
            let mut current = frame(height - overlap, width, height);
            for index in 0..changed_pixels {
                let y = 40 + index % 8;
                let x = 1 + index / 8;
                current.pixels[(y * width + x) * 4] ^= 1;
            }
            current
        };
        let changed_at_limit = make_current(overlap * width / 20);
        let changed_above_limit = make_current(overlap * width / 20 + 1);

        for current in [&changed_at_limit, &changed_above_limit] {
            assert_eq!(
                overlap_score(
                    &previous.pixels,
                    &current.pixels,
                    width,
                    height,
                    overlap,
                    0,
                    width,
                    8,
                ),
                0
            );
            assert_eq!(
                exact_overlap(&previous.pixels, &current.pixels, width, 8),
                None
            );
        }
        assert_eq!(
            find_overlap_after_footer_arbitration(
                &previous,
                &changed_at_limit.pixels,
                width,
                height,
                overlap,
            ),
            Some(overlap)
        );
        assert_eq!(
            find_overlap_after_footer_arbitration(
                &previous,
                &changed_above_limit.pixels,
                width,
                height,
                overlap,
            ),
            None
        );
    }

    #[test]
    fn stops_when_a_sampled_overlap_conflicts_with_unique_full_rows() {
        let (width, height) = (64, 160);
        let make_frame = |start: usize| {
            let mut pixels = Vec::with_capacity(width * height * 4);
            for y in start..start + height {
                for x in 0..width {
                    let value = if x % 2 == 0 && (80..=88).contains(&y) {
                        40
                    } else {
                        (y * 37) as u8
                    };
                    pixels.extend_from_slice(&[value, value, value, 255]);
                }
            }
            pixels
        };
        let previous = make_frame(0);
        let current = make_frame(80);
        assert_eq!(exact_overlap(&previous, &current, width, 8), Some((80, 0)));

        let mut ambiguous = FrameAccumulator::default();
        assert_eq!(
            ambiguous.submit(previous.clone(), width, height, 72),
            CaptureOutcome::Added
        );
        assert_eq!(
            ambiguous.submit(current.clone(), width, height, 72),
            CaptureOutcome::NoOverlap
        );
        assert_eq!(ambiguous.frames().len(), 1);
        assert_eq!(ambiguous.frames()[0].pixels, previous);
        assert_eq!(ambiguous.estimated_height(), height);

        let mut aligned = FrameAccumulator::default();
        assert_eq!(
            aligned.submit(ambiguous.frames()[0].pixels.clone(), width, height, 80),
            CaptureOutcome::Added
        );
        assert_eq!(
            aligned.submit(current, width, height, 80),
            CaptureOutcome::Added
        );
        assert_eq!(aligned.frames()[1].overlap, 80);
    }

    #[test]
    fn stops_when_a_sampled_overlap_conflicts_with_repeated_full_rows() {
        let (width, height) = (64, 160);
        let make_frame = |start: usize| {
            let mut pixels = Vec::with_capacity(width * height * 4);
            for y in start..start + height {
                for x in 0..width {
                    let value = if x % 2 == 0 {
                        40
                    } else if y < height {
                        ((y % 8) * 30) as u8
                    } else {
                        ((y * 37) % 256) as u8
                    };
                    pixels.extend_from_slice(&[value, value, value, 255]);
                }
            }
            pixels
        };
        let previous = make_frame(0);
        let current = make_frame(80);
        assert_eq!(exact_overlap(&previous, &current, width, 8), Some((80, 72)));

        let mut ambiguous = FrameAccumulator::default();
        assert_eq!(
            ambiguous.submit(previous.clone(), width, height, 79),
            CaptureOutcome::Added
        );
        assert_eq!(
            ambiguous.submit(current.clone(), width, height, 79),
            CaptureOutcome::NoOverlap
        );
        assert_eq!(ambiguous.frames().len(), 1);
        assert_eq!(ambiguous.frames()[0].pixels, previous);
        assert_eq!(ambiguous.estimated_height(), height);

        let mut aligned = FrameAccumulator::default();
        assert_eq!(
            aligned.submit(previous.clone(), width, height, 80),
            CaptureOutcome::Added
        );
        assert_eq!(
            aligned.submit(current, width, height, 80),
            CaptureOutcome::Added
        );
        assert_eq!(aligned.frames().len(), 2);
        assert_eq!(aligned.frames()[1].overlap, 80);
        assert_eq!(aligned.estimated_height(), 240);

        for (vertical, count) in [(false, 1), (false, 16), (true, 16)] {
            let mut animated = make_frame(80);
            for index in 0..count {
                let (x, y) = if vertical {
                    (1, 40 + index)
                } else {
                    (1 + index * 2, 40)
                };
                let animated_pixel = (y * width + x) * 4;
                animated[animated_pixel..animated_pixel + 4].copy_from_slice(&[255, 1, 200, 255]);
            }
            let mut animated_frames = FrameAccumulator::default();
            assert_eq!(
                animated_frames.submit(previous.clone(), width, height, 80),
                CaptureOutcome::Added
            );
            assert_eq!(
                animated_frames.submit(animated, width, height, 80),
                CaptureOutcome::Added
            );
            assert_eq!(animated_frames.frames()[1].overlap, 80);
        }
    }

    #[test]
    fn repeated_content_can_hide_independent_edge_motion() {
        let (width, height) = (1000, 160);
        let make_frame = |main_start: usize,
                          edge_start: usize,
                          edge_x: usize,
                          second_edge: Option<usize>,
                          footer: usize| {
            let mut pixels = Vec::with_capacity(width * height * 4);
            for y in 0..height {
                for x in 0..width {
                    let edge = edge_start + y;
                    let pixel = if y >= height - footer {
                        [51, 60, 70, 255]
                    } else if x == edge_x || second_edge == Some(x) {
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
        for footer in [0, 16] {
            for (edge_x, second_edge) in [
                (0, None),
                (1, None),
                (100, None),
                (101, None),
                (500, None),
                (501, None),
                (100, Some(102)),
                (500, Some(502)),
                (100, Some(500)),
            ] {
                let previous = make_frame(0, 0, edge_x, second_edge, footer);
                let translated = make_frame(104, 104, edge_x, second_edge, footer);
                let independently_moving_edge = make_frame(96, 104, edge_x, second_edge, footer);
                assert_eq!(translated.pixels, independently_moving_edge.pixels);
                assert_eq!(
                    frame_hash(&previous.pixels, width, height)
                        == frame_hash(&translated.pixels, width, height),
                    edge_x % 10 != 0
                );
                assert_eq!(
                    unique_exact_overlap(
                        &previous.pixels[..(height - footer) * width * 4],
                        &translated.pixels,
                        width,
                        8,
                    ),
                    Some(56 - footer)
                );
                if footer == 0 {
                    assert_eq!(
                        find_overlap(&previous, &translated.pixels, width, height, 64),
                        None
                    );
                }
                let mut frames = FrameAccumulator::default();
                assert_eq!(
                    frames.submit(previous.pixels.clone(), width, height, 64),
                    CaptureOutcome::Added
                );
                assert_eq!(
                    frames.submit(translated.pixels, width, height, 64),
                    CaptureOutcome::NoOverlap
                );
                assert_eq!(frames.frames().len(), 1);
                assert_eq!(frames.estimated_height(), height);
                assert_eq!(frames.frames()[0].pixels, previous.pixels);
                assert_eq!(frames.frames()[0].trim_bottom, 0);
            }
        }
    }

    #[test]
    fn repeated_content_can_hide_two_separated_edges_between_samples() {
        let (width, height) = (1000, 160);
        let make_frame = |main_start: usize, edge_start: usize, footer: usize| {
            let mut pixels = Vec::with_capacity(width * height * 4);
            for y in 0..height {
                for x in 0..width {
                    let row = edge_start + y;
                    let pixel = if y >= height - footer {
                        [51, 60, 70, 255]
                    } else if x == 100 || x == 500 {
                        [row as u8, (row >> 8) as u8, 0, 255]
                    } else {
                        [(((main_start + y) % 9) * 28) as u8, 0, 0, 255]
                    };
                    pixels.extend_from_slice(&pixel);
                }
            }
            pixels
        };
        for footer in [0, 16] {
            let previous = make_frame(0, 0, footer);
            let translated = make_frame(105, 105, footer);
            assert_eq!(translated, make_frame(96, 105, footer));
            let mut frames = FrameAccumulator::default();
            assert_eq!(
                frames.submit(previous.clone(), width, height, 64),
                CaptureOutcome::Added
            );
            assert_eq!(
                frames.submit(translated, width, height, 64),
                CaptureOutcome::NoOverlap
            );
            assert_eq!(frames.frames().len(), 1);
            assert_eq!(frames.frames()[0].pixels, previous);
        }
    }

    #[test]
    fn short_expected_overlap_can_hide_two_moving_edges() {
        let (width, height) = (1000, 160);
        let make_frame = |main_start: usize, edge_start: usize, footer: usize| {
            let mut pixels = Vec::with_capacity(width * height * 4);
            for y in 0..height {
                for x in 0..width {
                    let row = edge_start + y;
                    let pixel = if y >= height - footer {
                        [51, 60, 70, 255]
                    } else if x == 100 || x == 500 {
                        [row as u8, (row >> 8) as u8, 0, 255]
                    } else {
                        [(((main_start + y) % 7) * 36) as u8, 0, 0, 255]
                    };
                    pixels.extend_from_slice(&pixel);
                }
            }
            pixels
        };
        for footer in [0, 16] {
            let previous = make_frame(0, 0, footer);
            let main_shift = height - footer - 15;
            let edge_shift = height - footer - 8;
            let translated = make_frame(edge_shift, edge_shift, footer);
            assert_eq!(translated, make_frame(main_shift, edge_shift, footer));
            let mut frames = FrameAccumulator::default();
            assert_eq!(
                frames.submit(previous.clone(), width, height, footer + 15),
                CaptureOutcome::Added
            );
            assert_eq!(
                frames.submit(translated, width, height, footer + 15),
                CaptureOutcome::NoOverlap
            );
            assert_eq!(frames.frames().len(), 1);
            assert_eq!(frames.frames()[0].pixels, previous);
        }
    }

    #[test]
    fn isolated_changes_in_separate_regions_do_not_look_like_moving_columns() {
        let (width, height) = (1000, 160);
        let make_frame = |main_start: usize, marker_start: usize, footer: usize| {
            let mut pixels = Vec::with_capacity(width * height * 4);
            for y in 0..height {
                for x in 0..width {
                    let row = marker_start + y;
                    let pixel = if y >= height - footer {
                        [51, 60, 70, 255]
                    } else if (x == 100 && row == 105) || (x == 500 && row == 133) {
                        [253, 252, 251, 255]
                    } else {
                        [(((main_start + y) % 9) * 28) as u8, 0, 0, 255]
                    };
                    pixels.extend_from_slice(&pixel);
                }
            }
            pixels
        };
        for footer in [0, 16] {
            let translated = make_frame(105, 105, footer);
            assert_eq!(translated, make_frame(96, 105, footer));
            let mut frames = FrameAccumulator::default();
            assert_eq!(
                frames.submit(make_frame(0, 0, footer), width, height, 64),
                CaptureOutcome::Added
            );
            assert_eq!(
                frames.submit(translated, width, height, 64),
                CaptureOutcome::Added
            );
        }
    }

    #[test]
    fn exact_overlap_survives_a_locally_repeated_strip() {
        let (width, height, step) = (1000, 160, 104);
        let make_frame = |start: usize, footer: usize| {
            let mut pixels = Vec::with_capacity(width * height * 4);
            for y in 0..height {
                for x in 0..width {
                    let row = start + y;
                    let pixel = if y >= height - footer {
                        [51, 60, 70, 255]
                    } else if x != 500 && (96..112).contains(&row) {
                        [((row % 8) * 31) as u8, 0, 0, 255]
                    } else {
                        let seed = (row as u32).wrapping_mul(0x45d9f3b)
                            ^ (x as u32).wrapping_mul(0x119de1f3);
                        [
                            seed as u8,
                            seed.rotate_left(9) as u8,
                            seed.rotate_left(17) as u8,
                            255,
                        ]
                    };
                    pixels.extend_from_slice(&pixel);
                }
            }
            pixels
        };
        for footer in [0, 16] {
            let mut frames = FrameAccumulator::default();
            assert_eq!(
                frames.submit(make_frame(0, footer), width, height, 64),
                CaptureOutcome::Added
            );
            assert_eq!(
                frames.submit(make_frame(step, footer), width, height, 64),
                CaptureOutcome::Added
            );
            assert_eq!(frames.frames()[0].trim_bottom, footer);
            assert_eq!(frames.frames()[1].overlap, height - step - footer);
        }
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
    fn exact_overlap_handles_a_shorter_repeated_current_frame() {
        let previous = [0, 0, 0, 255].repeat(100);
        let current = [0, 0, 0, 255].repeat(20);
        assert_eq!(exact_overlap(&previous, &current, 1, 8), Some((20, 19)));
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
    fn stitches_a_footer_with_a_small_animated_block() {
        let (width, height, footer, step) = (1024, 160, 40, 16);
        let row_bytes = width * 4;
        let mut accumulator = FrameAccumulator::default();
        let mut last_footer = Vec::new();
        for index in 0..3 {
            let mut pixels = frame(index * step, width, height).pixels;
            for pixel in pixels[(height - footer) * row_bytes..]
                .as_chunks_mut::<4>()
                .0
            {
                pixel.copy_from_slice(&[64, 64, 64, 255]);
            }
            for y in height - 32..height - 16 {
                for x in 1..17 {
                    pixels[(y * width + x) * 4] ^= (index * 64) as u8;
                }
            }
            last_footer = pixels[(height - footer) * row_bytes..].to_vec();
            assert_eq!(
                accumulator.submit(pixels, width, height, height - step),
                CaptureOutcome::Added
            );
        }
        assert_eq!(accumulator.frames()[0].trim_bottom, footer);
        assert_eq!(accumulator.frames()[1].trim_bottom, footer);
        assert_eq!(accumulator.frames()[1].overlap, height - footer - step);
        assert_eq!(accumulator.frames()[2].overlap, height - footer - step);

        let mut actual = Vec::new();
        for (index, captured) in accumulator.frames().iter().enumerate() {
            let start = if index == 0 { 0 } else { captured.overlap };
            actual.extend_from_slice(
                &captured.pixels[start * row_bytes..(height - captured.trim_bottom) * row_bytes],
            );
        }
        let mut expected = frame(0, width, height + 2 * step).pixels;
        let footer_start = expected.len() - last_footer.len();
        expected[footer_start..].copy_from_slice(&last_footer);
        assert_eq!(actual, expected);
    }

    #[test]
    fn stitches_a_footer_with_a_small_animation_in_scrolling_content() {
        let (width, height, footer, step) = (1024, 160, 40, 16);
        let row_bytes = width * 4;
        let mut accumulator = FrameAccumulator::default();
        for index in 0..3 {
            let mut pixels = frame(index * step, width, height).pixels;
            for pixel in pixels[(height - footer) * row_bytes..]
                .as_chunks_mut::<4>()
                .0
            {
                pixel.copy_from_slice(&[64, 64, 64, 255]);
            }
            for y in 40..56 {
                for x in 1..17 {
                    pixels[(y * width + x) * 4] ^= (index * 64) as u8;
                }
            }
            assert_eq!(
                accumulator.submit(pixels, width, height, height - step),
                CaptureOutcome::Added
            );
        }
        assert_eq!(accumulator.frames()[0].trim_bottom, footer);
        assert_eq!(accumulator.frames()[1].trim_bottom, footer);
        assert_eq!(accumulator.frames()[1].overlap, height - footer - step);
        assert_eq!(accumulator.frames()[2].overlap, height - footer - step);

        let mut actual = Vec::new();
        for (index, captured) in accumulator.frames().iter().enumerate() {
            let start = if index == 0 { 0 } else { captured.overlap };
            actual.extend_from_slice(
                &captured.pixels[start * row_bytes..(height - captured.trim_bottom) * row_bytes],
            );
        }
        let mut expected = frame(0, width, height + 2 * step).pixels;
        let footer_start = expected.len() - footer * row_bytes;
        for pixel in expected[footer_start..].as_chunks_mut::<4>().0 {
            pixel.copy_from_slice(&[64, 64, 64, 255]);
        }
        assert_eq!(actual, expected);
    }

    #[test]
    fn repeated_content_with_animation_does_not_gain_a_footer() {
        let (width, height, step) = (64, 320, 8);
        let row_bytes = width * 4;
        let make_frame = |start: usize, rows: usize, animation: u8| {
            let mut pixels = Vec::with_capacity(width * rows * 4);
            for y in 0..rows {
                for x in 0..width {
                    let source_y = start + y;
                    let seed = (source_y as u32).wrapping_mul(0x45d9f3b)
                        ^ (x as u32).wrapping_mul(0x119de1f3);
                    let mut pixel = if x == 0 && source_y % 256 < 32 {
                        [
                            seed as u8,
                            seed.rotate_left(9) as u8,
                            seed.rotate_left(17) as u8,
                            255,
                        ]
                    } else {
                        [((source_y % 8) * 30) as u8, 0, 0, 255]
                    };
                    if x == 1 && y == 156 {
                        pixel[0] ^= animation;
                    }
                    pixels.extend_from_slice(&pixel);
                }
            }
            pixels
        };
        let mut accumulator = FrameAccumulator::default();
        for index in 0..4 {
            assert_eq!(
                accumulator.submit(
                    make_frame(index * step, height, (index * 51) as u8),
                    width,
                    height,
                    height - step
                ),
                CaptureOutcome::Added
            );
        }
        let mut actual = Vec::new();
        for (index, captured) in accumulator.frames().iter().enumerate() {
            assert_eq!(captured.trim_bottom, 0);
            let start = if index == 0 { 0 } else { captured.overlap };
            actual.extend_from_slice(&captured.pixels[start * row_bytes..]);
        }
        assert_eq!(actual, make_frame(0, height + 3 * step, 0));
    }

    #[test]
    fn preserves_a_fixed_footer_with_sparse_scrolling_content() {
        let (width, height, footer, step) = (64, 160, 20, 8);
        let row_bytes = width * 4;
        let make_frame = |start: usize, rows: usize| {
            let mut pixels = Vec::with_capacity(width * rows * 4);
            for y in 0..rows {
                for x in 0..width {
                    let pixel = if y >= rows - footer {
                        [62, (y - (rows - footer)) as u8, x as u8, 255]
                    } else if x == 0 {
                        let value = start + y;
                        [
                            value as u8,
                            (value >> 8) as u8,
                            value.wrapping_mul(37) as u8,
                            255,
                        ]
                    } else {
                        [235, 235, 235, 255]
                    };
                    pixels.extend_from_slice(&pixel);
                }
            }
            pixels
        };
        let mut accumulator = FrameAccumulator::default();
        for index in 0..4 {
            assert_eq!(
                accumulator.submit(
                    make_frame(index * step, height),
                    width,
                    height,
                    height - step
                ),
                CaptureOutcome::Added
            );
        }

        let mut actual = Vec::new();
        for (index, captured) in accumulator.frames().iter().enumerate() {
            let start = if index == 0 { 0 } else { captured.overlap };
            actual.extend_from_slice(
                &captured.pixels[start * row_bytes..(height - captured.trim_bottom) * row_bytes],
            );
        }
        assert_eq!(actual, make_frame(0, height + 3 * step));
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
    fn stationary_pixel_after_a_scroll_does_not_add_a_frame() {
        let (width, height, step) = (64, 160, 16);
        let make_frame = |start: usize| {
            let mut pixels = Vec::with_capacity(width * height * 4);
            for y in 0..height {
                for x in 0..width {
                    let value = start + y;
                    let pixel = if x == 0 {
                        [
                            value as u8,
                            (value >> 8) as u8,
                            value.wrapping_mul(17) as u8,
                            255,
                        ]
                    } else {
                        [235, 235, 235, 255]
                    };
                    pixels.extend_from_slice(&pixel);
                }
            }
            pixels
        };
        let mut accumulator = FrameAccumulator::default();
        assert_eq!(
            accumulator.submit(make_frame(0), width, height, height - step),
            CaptureOutcome::Added
        );
        let previous = make_frame(step);
        assert_eq!(
            accumulator.submit(previous.clone(), width, height, height - step),
            CaptureOutcome::Added
        );
        let mut animated = previous.clone();
        animated[158 * width * 4] ^= 255;
        assert_ne!(
            frame_hash(&previous, width, height),
            frame_hash(&animated, width, height)
        );
        for expected in [
            CaptureOutcome::Repeated,
            CaptureOutcome::Repeated,
            CaptureOutcome::Ended,
        ] {
            assert_eq!(
                accumulator.submit(animated.clone(), width, height, height - step),
                expected
            );
        }
        assert_eq!(accumulator.frames().len(), 2);
        assert_eq!(accumulator.frames()[1].pixels, previous);
        assert_eq!(accumulator.estimated_height(), height + step);
    }

    #[test]
    fn one_pixel_leaving_a_scrolled_viewport_keeps_its_overlap() {
        let (width, height) = (96, 160);
        let mut first = [255; 4].repeat(width * height);
        let current = first.clone();
        first[120 * width * 4..120 * width * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
        for (expected, overlap) in [(32, 32), (40, 39)] {
            let mut accumulator = FrameAccumulator::default();
            assert_eq!(
                accumulator.submit(first.clone(), width, height, expected),
                CaptureOutcome::Added
            );
            assert_eq!(
                accumulator.submit(current.clone(), width, height, expected),
                CaptureOutcome::Added
            );
            assert_eq!(accumulator.frames()[1].overlap, overlap);
        }
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
    fn cancelled_png_write_leaves_an_existing_output_untouched() {
        let path = std::env::temp_dir().join(format!(
            "poratake-scroll-cancelled-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        std::fs::write(&path, b"existing capture").expect("existing output");
        let cancelled = AtomicBool::new(true);

        assert_eq!(
            write_png_cancellable(&path, &[frame(0, 4, 4)], &cancelled),
            Err(PngWriteError::Cancelled)
        );
        assert_eq!(
            std::fs::read(&path).expect("existing output remains"),
            b"existing capture"
        );
        std::fs::remove_file(path).expect("remove existing output");
    }

    #[test]
    fn cancellation_during_png_rows_leaves_the_caller_path_for_cleanup() {
        let path = std::env::temp_dir().join(format!(
            "poratake-scroll-partial-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        std::fs::write(&path, b"caller-owned output").expect("existing output");
        let mut checks = 0;
        let mut is_cancelled = || {
            checks += 1;
            checks >= 3
        };

        assert_eq!(
            write_png_with_cancel_check(&path, &[frame(0, 4, 4)], &mut is_cancelled),
            Err(PngWriteError::Cancelled)
        );
        assert!(path.is_file());
        std::fs::remove_file(path).expect("caller can clean the partial output");
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
