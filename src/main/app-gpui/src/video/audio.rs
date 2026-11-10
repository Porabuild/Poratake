//! Audio for the video export: decode the recording's tracks, apply the
//! timeline's trims and speeds, and mix them down to one interleaved stream the
//! encoder can take. This replaces the FFmpeg filter graph the Electron shell
//! uses for timeline composition.

use std::path::Path;

use crate::video::encoder::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};

/// Interleaved stereo 16-bit samples at [`AUDIO_SAMPLE_RATE`].
pub type Pcm = Vec<i16>;

const FRAME_SIZE: usize = AUDIO_CHANNELS as usize;

/// One decoded source and how loudly it should sit in the mix.
pub struct Track {
    pub samples: Pcm,
    pub volume: f64,
}

/// Decodes any file Media Foundation can read into the encoder's format.
/// Returns `None` when the file is missing or has no audio.
pub fn decode(path: &Path) -> Option<Pcm> {
    if !path.is_file() {
        return None;
    }
    backend::decode(path)
}

fn frames(samples: &[i16]) -> usize {
    samples.len() / FRAME_SIZE
}

fn frames_for(seconds: f64) -> usize {
    (seconds.max(0.0) * AUDIO_SAMPLE_RATE as f64).round() as usize
}

/// Sums tracks, scaling each by its volume and clipping at the 16-bit range.
/// The result is `length_frames` long, padded with silence.
pub fn mix(tracks: &[Track], length_frames: usize) -> Pcm {
    let mut accumulator = vec![0.0_f32; length_frames * FRAME_SIZE];
    for track in tracks {
        let volume = track.volume.clamp(0.0, 4.0) as f32;
        if volume == 0.0 {
            continue;
        }
        let count = accumulator.len().min(track.samples.len());
        for (accumulator, sample) in accumulator[..count].iter_mut().zip(&track.samples[..count]) {
            *accumulator += *sample as f32 * volume;
        }
    }
    accumulator
        .into_iter()
        .map(|value| value.clamp(i16::MIN as f32, i16::MAX as f32) as i16)
        .collect()
}

/// Little-endian bytes, the layout the encoder's PCM input type expects.
pub fn to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placement {
    pub start_time: f64,
    pub end_time: f64,
    pub original_duration: f64,
    pub trim_start: f64,
    pub trim_end: f64,
    pub speed: f64,
}

impl Placement {
    pub fn of(track: &crate::windows::video_editor::model::MusicTrack) -> Self {
        Self {
            start_time: track.start_time,
            end_time: track.end_time,
            original_duration: track.original_duration,
            trim_start: track.trim_start,
            trim_end: track.trim_end,
            speed: track.speed,
        }
    }
}

pub fn place(samples: &[i16], placement: &Placement) -> Pcm {
    let available = frames(samples);
    let start = frames_for(placement.trim_start).min(available);
    let end = frames_for(placement.original_duration.max(0.0) - placement.trim_end)
        .clamp(start, available);

    let mut slice: Pcm = samples[start * FRAME_SIZE..end * FRAME_SIZE].to_vec();
    if (placement.speed - 1.0).abs() > f64::EPSILON {
        slice = time_stretch(&slice, placement.speed);
    }

    let offset = frames_for(placement.start_time);
    let mut placed: Pcm = vec![0; offset * FRAME_SIZE];
    placed.extend_from_slice(&slice);

    // does not bleed past where the timeline places it.
    let limit = frames_for(placement.end_time) * FRAME_SIZE;
    if limit > 0 && placed.len() > limit {
        placed.truncate(limit);
    }
    placed
}

const STRETCH_WINDOW_FRAMES: usize = 1024;
const STRETCH_OVERLAP_FRAMES: usize = 768;
const STRETCH_SEARCH_FRAMES: usize = AUDIO_SAMPLE_RATE as usize / 100;

fn dot_product(a: &[i16], b: &[i16]) -> i64 {
    a.iter()
        .zip(b)
        .map(|(a, b)| i64::from(*a) * i64::from(*b))
        .sum()
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn dot_product_avx2(a: &[i16], b: &[i16]) -> i64 {
    use std::arch::x86_64::*;

    let mut lo = _mm256_setzero_si256();
    let mut hi = _mm256_setzero_si256();
    let min = _mm256_set1_epi16(i16::MIN);
    let two = _mm256_set1_epi32(2);
    let correction = _mm256_set1_epi64x(1_i64 << 32);
    let mut i = 0;
    while i + 16 <= a.len() {
        let x = _mm256_loadu_si256(a.as_ptr().add(i).cast());
        let y = _mm256_loadu_si256(b.as_ptr().add(i).cast());
        let pairs = _mm256_madd_epi16(x, y);
        let both = _mm256_and_si256(_mm256_cmpeq_epi16(x, min), _mm256_cmpeq_epi16(y, min));
        let overflow = _mm256_cmpeq_epi32(_mm256_madd_epi16(both, both), two);
        let lower = _mm256_add_epi64(
            _mm256_cvtepi32_epi64(_mm256_castsi256_si128(pairs)),
            _mm256_and_si256(
                _mm256_cvtepi32_epi64(_mm256_castsi256_si128(overflow)),
                correction,
            ),
        );
        let upper = _mm256_add_epi64(
            _mm256_cvtepi32_epi64(_mm256_extracti128_si256::<1>(pairs)),
            _mm256_and_si256(
                _mm256_cvtepi32_epi64(_mm256_extracti128_si256::<1>(overflow)),
                correction,
            ),
        );
        lo = _mm256_add_epi64(lo, lower);
        hi = _mm256_add_epi64(hi, upper);
        i += 16;
    }
    let mut lanes = [0_i64; 4];
    _mm256_storeu_si256(lanes.as_mut_ptr().cast(), _mm256_add_epi64(lo, hi));
    lanes.into_iter().sum::<i64>() + dot_product(&a[i..], &b[i..])
}

fn aligned_start(
    samples: &[i16],
    reference: usize,
    expected: usize,
    overlap: usize,
    minimum: usize,
    previous: usize,
) -> usize {
    let total = frames(samples);
    if overlap == 0 || reference + overlap > total || expected + overlap > total {
        return expected.max(previous).min(total - 1);
    }
    let reference = &samples[reference * FRAME_SIZE..(reference + overlap) * FRAME_SIZE];
    if reference.iter().all(|sample| *sample == 0) {
        return expected.max(previous).min(total - 1);
    }
    let first = expected.saturating_sub(STRETCH_SEARCH_FRAMES).max(minimum);
    let last = expected
        .saturating_add(STRETCH_SEARCH_FRAMES)
        .min(total - overlap);
    if first > last {
        return expected.max(minimum).min(total - 1);
    }
    let mut energy: i64 = samples[first * FRAME_SIZE..(first + overlap) * FRAME_SIZE]
        .iter()
        .map(|sample| i64::from(*sample).pow(2))
        .sum();
    let mut best = expected.clamp(first, last);
    let mut best_score = f64::NEG_INFINITY;
    #[cfg(target_arch = "x86_64")]
    let vectorized = std::is_x86_feature_detected!("avx2");
    for candidate in first..=last {
        if energy > 0 {
            let input = &samples[candidate * FRAME_SIZE..(candidate + overlap) * FRAME_SIZE];
            #[cfg(target_arch = "x86_64")]
            let dot = if vectorized {
                unsafe { dot_product_avx2(reference, input) }
            } else {
                dot_product(reference, input)
            };
            #[cfg(not(target_arch = "x86_64"))]
            let dot = dot_product(reference, input);
            let score = dot as f64 / (energy as f64).sqrt();
            if score > best_score
                || (score == best_score && candidate.abs_diff(expected) < best.abs_diff(expected))
            {
                best = candidate;
                best_score = score;
            }
        }
        if candidate < last {
            for channel in 0..FRAME_SIZE {
                let leaving = i64::from(samples[candidate * FRAME_SIZE + channel]);
                let entering = i64::from(samples[(candidate + overlap) * FRAME_SIZE + channel]);
                energy += entering * entering - leaving * leaving;
            }
        }
    }
    best
}

/// Pitch-preserving speed change by overlap-add (SOLA), which is what the
/// Electron export gets from FFmpeg's `atempo`. `speed` above 1 shortens.
pub fn time_stretch(samples: &[i16], speed: f64) -> Pcm {
    let speed = speed.clamp(0.05, 20.0);
    let total = frames(samples);
    if total == 0 || (speed - 1.0).abs() <= f64::EPSILON {
        return samples.to_vec();
    }

    let window = STRETCH_WINDOW_FRAMES.min(total.max(1));
    let overlap = STRETCH_OVERLAP_FRAMES.min(window.saturating_sub(1));
    let hop_out = window - overlap;
    let output_frames = ((total as f64) / speed).round().max(1.0) as usize;

    let mut output = vec![0.0_f32; output_frames * FRAME_SIZE];
    let mut weights = vec![0.0_f32; output_frames];

    let mut previous = None;
    let mut write = 0usize;
    while write < output_frames {
        let expected = (write as f64 * speed).round() as usize;
        if expected >= total {
            break;
        }
        let read = previous.map_or(expected, |previous| {
            let minimum = if speed >= 1.0 { previous } else { 0 };
            aligned_start(
                samples,
                previous + hop_out,
                expected,
                overlap,
                minimum,
                previous,
            )
        });
        let length = window.min(total - read).min(output_frames - write);
        for frame in 0..length {
            // A Hann window makes the overlapping copies sum to a flat gain.
            let envelope = if length > 1 {
                let phase = frame as f32 / (length - 1) as f32;
                0.5 - 0.5 * (std::f32::consts::TAU * phase).cos()
            } else {
                1.0
            };
            let source = (read + frame) * FRAME_SIZE;
            let target = (write + frame) * FRAME_SIZE;
            for channel in 0..FRAME_SIZE {
                output[target + channel] += samples[source + channel] as f32 * envelope;
            }
            weights[write + frame] += envelope;
        }
        previous = Some(read);
        write += hop_out;
    }

    let mut result: Pcm = Vec::with_capacity(output_frames * FRAME_SIZE);
    for frame in 0..output_frames {
        let weight = weights[frame].max(0.0001);
        for channel in 0..FRAME_SIZE {
            let value = output[frame * FRAME_SIZE + channel] / weight;
            result.push(value.clamp(i16::MIN as f32, i16::MAX as f32) as i16);
        }
    }
    result
}

#[cfg(test)]
pub(crate) fn tone_strength(samples: &[i16], frequency: f64) -> f64 {
    let (real, imaginary, power) = samples.as_chunks::<2>().0.iter().enumerate().fold(
        (0_f64, 0_f64, 0_f64),
        |(real, imaginary, power), (frame, sample)| {
            let value = f64::from(sample[0]);
            let phase =
                frame as f64 * frequency * std::f64::consts::TAU / f64::from(AUDIO_SAMPLE_RATE);
            (
                real + value * phase.cos(),
                imaginary + value * phase.sin(),
                power + value.powi(2),
            )
        },
    );
    ((real * real + imaginary * imaginary) * 2. / (power * frames(samples) as f64)).sqrt()
}

#[cfg(windows)]
mod backend {
    use std::path::Path;

    use windows::core::{Interface, HSTRING, PCWSTR};
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

    use super::Pcm;
    use crate::video::encoder::{AUDIO_BITS_PER_SAMPLE, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};

    /// Media Foundation interfaces are not agile, so decoding runs entirely on
    /// one short-lived thread and only the samples cross back.
    pub fn decode(path: &Path) -> Option<Pcm> {
        let owned = path.to_path_buf();
        std::thread::Builder::new()
            .name("audio-decoder".into())
            .spawn(move || unsafe { decode_on_thread(&owned) })
            .ok()?
            .join()
            .ok()?
    }

    unsafe fn decode_on_thread(path: &Path) -> Option<Pcm> {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        if MFStartup(MF_VERSION, MFSTARTUP_FULL).is_err() {
            CoUninitialize();
            return None;
        }
        let samples = read_all(path);
        let _ = MFShutdown();
        CoUninitialize();
        samples
    }

    unsafe fn read_all(path: &Path) -> Option<Pcm> {
        let wide = HSTRING::from(path.as_os_str());
        let reader = MFCreateSourceReaderFromURL(PCWSTR(wide.as_ptr()), None).ok()?;

        reader
            .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
            .ok()?;
        reader
            .SetStreamSelection(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, true)
            .ok()?;

        let output = MFCreateMediaType().ok()?;
        output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).ok()?;
        output.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM).ok()?;
        output
            .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, AUDIO_BITS_PER_SAMPLE)
            .ok()?;
        output
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, AUDIO_SAMPLE_RATE)
            .ok()?;
        output
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, AUDIO_CHANNELS)
            .ok()?;
        reader
            .SetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, None, &output)
            .ok()?;

        let mut samples: Pcm = Vec::new();
        loop {
            let mut flags = 0u32;
            let mut sample: Option<IMFSample> = None;
            reader
                .ReadSample(
                    MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32,
                    0,
                    None,
                    Some(&mut flags),
                    None,
                    Some(&mut sample),
                )
                .ok()?;
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }
            let Some(sample) = sample else {
                continue;
            };
            let buffer = sample.ConvertToContiguousBuffer().ok()?;
            let mut data = std::ptr::null_mut();
            let mut length = 0u32;
            buffer.Lock(&mut data, None, Some(&mut length)).ok()?;
            let bytes = std::slice::from_raw_parts(data, length as usize);
            samples.extend(
                bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| i16::from_le_bytes([pair[0], pair[1]])),
            );
            let _ = buffer.Unlock();
        }

        (!samples.is_empty()).then_some(samples)
    }

    /// Keeps the `Interface` import meaningful on every build configuration.
    #[allow(dead_code)]
    fn _assert_interface_in_scope(sample: &IMFSample) -> bool {
        sample.as_raw().is_null()
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod backend {
    use std::io::{BufReader, Read as _};
    use std::path::Path;
    use std::process::{Command, Stdio};

    use super::Pcm;

    pub fn decode(path: &Path) -> Option<Pcm> {
        decode_with_ffmpeg(&crate::video::ffmpeg_path(), path)
    }

    fn decode_with_ffmpeg(ffmpeg: &Path, path: &Path) -> Option<Pcm> {
        let output = crate::video::command_stdout(
            Command::new(ffmpeg)
                .args(["-hide_banner", "-loglevel", "error", "-i"])
                .arg(path)
                .args([
                    "-vn",
                    "-f",
                    "s16le",
                    "-acodec",
                    "pcm_s16le",
                    "-ar",
                    "48000",
                    "-ac",
                    "2",
                    "pipe:1",
                ])
                .stdin(Stdio::null()),
            std::time::Duration::from_secs(120),
            |stdout| {
                let mut stdout = BufReader::new(stdout);
                let mut samples = Vec::new();
                let mut bytes = [0; 2];
                loop {
                    match stdout.read_exact(&mut bytes) {
                        Ok(()) => samples.push(i16::from_le_bytes(bytes)),
                        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
                        Err(error) => return Err(error),
                    }
                }
                Ok(samples)
            },
        )
        .ok()?;
        if !output.status.success() || output.stdout.is_empty() {
            return None;
        }
        Some(output.stdout)
    }

    #[cfg(test)]
    mod tests {
        use std::os::unix::fs::PermissionsExt as _;

        use super::*;

        #[test]
        fn unix_ffmpeg_process_decodes_stereo_pcm() {
            let directory = tempfile::tempdir().expect("temp directory");
            let ffmpeg = directory.path().join("ffmpeg");
            std::fs::write(&ffmpeg, "#!/bin/sh\nprintf '\\001\\000\\377\\377'\n")
                .expect("fake FFmpeg");
            let mut permissions = std::fs::metadata(&ffmpeg)
                .expect("fake FFmpeg metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&ffmpeg, permissions).expect("fake FFmpeg permissions");
            let source = directory.path().join("source.m4a");
            std::fs::write(&source, []).expect("source file");

            assert_eq!(decode_with_ffmpeg(&ffmpeg, &source), Some(vec![1, -1]));
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod backend {
    use std::path::Path;

    use super::Pcm;

    pub fn decode(_path: &Path) -> Option<Pcm> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(frames: usize, amplitude: i16) -> Pcm {
        (0..frames)
            .flat_map(|frame| {
                let phase = frame as f32 / 48.0 * std::f32::consts::TAU;
                let value = (phase.sin() * amplitude as f32) as i16;
                [value, value]
            })
            .collect()
    }

    #[test]
    fn mixing_sums_and_clips() {
        let loud = Track {
            samples: vec![20_000, 20_000],
            volume: 1.0,
        };
        let louder = Track {
            samples: vec![20_000, 20_000],
            volume: 1.0,
        };
        let mixed = mix(&[loud, louder], 1);
        assert_eq!(mixed, vec![i16::MAX, i16::MAX]);
    }

    #[test]
    fn a_silent_track_is_skipped() {
        let track = Track {
            samples: vec![1000, 1000],
            volume: 0.0,
        };
        assert_eq!(mix(&[track], 1), vec![0, 0]);
    }

    #[test]
    fn the_mix_is_padded_to_the_requested_length() {
        let track = Track {
            samples: vec![100, 100],
            volume: 1.0,
        };
        let mixed = mix(&[track], 3);
        assert_eq!(mixed.len(), 6);
        assert_eq!(&mixed[2..], &[0, 0, 0, 0]);
    }

    #[test]
    fn stretching_preserves_the_signal_level() {
        let samples = tone(AUDIO_SAMPLE_RATE as usize, 8000);
        let stretched = time_stretch(&samples, 0.5);
        assert!(frames(&stretched) > frames(&samples));

        let peak = stretched.iter().map(|value| value.abs()).max().unwrap_or(0);
        assert!((6000..=9000).contains(&peak), "{peak}");
    }

    #[test]
    fn stretching_preserves_stereo_pitch_across_speeds() {
        for frequency in [50., 100., 440., 880., 12_000.] {
            let source: Pcm = (0..AUDIO_SAMPLE_RATE)
                .flat_map(|frame| {
                    let phase = f64::from(frame) * frequency * std::f64::consts::TAU
                        / f64::from(AUDIO_SAMPLE_RATE);
                    let sample = (phase.sin() * 5000.) as i16;
                    [sample, -sample]
                })
                .collect();
            for speed in [0.5, 0.75, 1.25, 1.5, 2., 3., 4.] {
                let stretched = time_stretch(&source, speed);
                assert_eq!(
                    frames(&stretched),
                    (f64::from(AUDIO_SAMPLE_RATE) / speed).round() as usize
                );
                assert!(stretched
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .all(|frame| frame[0] == -frame[1]));
                let start = AUDIO_SAMPLE_RATE as usize / 10 * FRAME_SIZE;
                let end = AUDIO_SAMPLE_RATE as usize / 5 * FRAME_SIZE;
                let strength = tone_strength(&stretched[start..end], frequency);
                assert!(strength > 0.9, "{frequency} Hz at {speed}x: {strength}");
            }
        }
    }

    #[test]
    fn stretching_keeps_short_sounds_near_their_timeline_positions() {
        let rate = AUDIO_SAMPLE_RATE as usize;
        let mut source: Pcm = (0..rate * 5)
            .flat_map(|frame| {
                let phase = frame as f64 * 20. * std::f64::consts::TAU / rate as f64;
                let sample = (phase.sin() * 3000.) as i16;
                [sample, sample]
            })
            .collect();
        for index in 1..=9 {
            let start = index * rate / 2;
            for frame in 0..rate / 100 {
                let phase = frame as f64 * 440. * std::f64::consts::TAU / rate as f64;
                let sample = (phase.sin() * 15_000.) as i16;
                source[(start + frame) * FRAME_SIZE] += sample;
                source[(start + frame) * FRAME_SIZE + 1] += sample;
            }
        }
        for speed in [0.5, 0.75, 1.25, 1.5, 2., 3., 4.] {
            let stretched = time_stretch(&source, speed);
            for index in 1..=9 {
                let center = (index as f64 * 0.5 / speed * rate as f64) as usize;
                let start = center.saturating_sub(rate / 25);
                let end = (center + rate / 25).min(frames(&stretched));
                let peak = stretched[start * FRAME_SIZE..end * FRAME_SIZE]
                    .iter()
                    .map(|sample| sample.unsigned_abs())
                    .max()
                    .unwrap_or(0);
                assert!(peak > 6000, "sound {index} at {speed}x: peak {peak}");
            }
        }
    }

    #[test]
    fn slowing_a_short_sound_at_the_end_or_before_silence_does_not_split_it() {
        let rate = AUDIO_SAMPLE_RATE as usize;
        for (tone_end, start) in [(rate * 5, rate * 498 / 100), (rate, rate * 99 / 100)] {
            let mut source: Pcm = (0..rate * 5)
                .flat_map(|frame| {
                    let phase = frame as f64 * 20. * std::f64::consts::TAU / rate as f64;
                    let sample = if frame < tone_end {
                        (phase.sin() * 3000.) as i16
                    } else {
                        0
                    };
                    [sample, sample]
                })
                .collect();
            for frame in 0..rate / 100 {
                let phase = frame as f64 * 440. * std::f64::consts::TAU / rate as f64;
                let sample = (phase.sin() * 15_000.) as i16;
                source[(start + frame) * FRAME_SIZE] += sample;
                source[(start + frame) * FRAME_SIZE + 1] += sample;
            }
            let stretched = time_stretch(&source, 0.5);
            let mut groups = 0;
            let mut last = None;
            let end = (start * 2 + rate / 10).min(frames(&stretched));
            for frame in start * 2 - rate / 10..end {
                if stretched[frame * FRAME_SIZE].unsigned_abs() <= 6000 {
                    continue;
                }
                if last.is_none_or(|previous| frame - previous > rate / 200) {
                    groups += 1;
                }
                last = Some(frame);
            }
            assert_eq!(groups, 1, "short sound at frame {start}");
        }
    }

    #[test]
    fn a_unit_speed_stretch_is_the_identity() {
        let samples = tone(1000, 5000);
        assert_eq!(time_stretch(&samples, 1.0), samples);
    }

    #[test]
    fn stretching_short_silence_keeps_the_expected_duration() {
        for length in [0, 1, 128, 1024] {
            let samples = vec![0; length * FRAME_SIZE];
            for speed in [0.5, 1.5, 2., 4.] {
                let stretched = time_stretch(&samples, speed);
                let expected = if length == 0 {
                    0
                } else {
                    (length as f64 / speed).round().max(1.) as usize
                };
                assert_eq!(frames(&stretched), expected);
                assert!(stretched.iter().all(|sample| *sample == 0));
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn vectorized_alignment_matches_scalar_at_signed_extremes() {
        if !std::is_x86_feature_detected!("avx2") {
            return;
        }
        let mut seed = 1_u64;
        for length in [0, 1, 2, 15, 16, 17, 31, 32, 255, 768, 1536] {
            let mut a = Vec::with_capacity(length);
            let mut b = Vec::with_capacity(length);
            for _ in 0..length {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                a.push((seed >> 32) as i16);
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                b.push((seed >> 32) as i16);
            }
            assert_eq!(unsafe { dot_product_avx2(&a, &b) }, dot_product(&a, &b));
        }
        for a in [i16::MIN, i16::MIN + 1, i16::MAX] {
            for b in [i16::MIN, i16::MIN + 1, i16::MAX] {
                let a = vec![a; 1536];
                let b = vec![b; 1536];
                assert_eq!(unsafe { dot_product_avx2(&a, &b) }, dot_product(&a, &b));
            }
        }
    }

    #[test]
    fn bytes_are_little_endian_pairs() {
        assert_eq!(to_bytes(&[1, -1]), vec![0x01, 0x00, 0xff, 0xff]);
    }

    #[test]
    fn a_music_track_is_trimmed_and_placed_at_its_start() {
        use crate::windows::video_editor::model::MusicTrack;

        let samples = tone(AUDIO_SAMPLE_RATE as usize, 8000);
        let track = MusicTrack {
            start_time: 1.0,
            end_time: 1.5,
            original_duration: 1.0,
            trim_start: 0.25,
            trim_end: 0.25,
            ..MusicTrack::default()
        };
        let placed = place(&samples, &Placement::of(&track));
        assert_eq!(frames(&placed), frames_for(1.5));
        // Everything before the start is silence.
        assert!(placed[..frames_for(1.0) * FRAME_SIZE]
            .iter()
            .all(|value| *value == 0));
        assert!(placed[frames_for(1.1) * FRAME_SIZE..]
            .iter()
            .any(|value| *value != 0));
    }

    #[test]
    fn a_missing_file_decodes_to_nothing() {
        assert!(decode(&crate::util::test_paths::unique_temp(
            "poratake-missing.m4a"
        ))
        .is_none());
    }
}
