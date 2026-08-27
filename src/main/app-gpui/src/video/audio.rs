//! Audio for the video export: decode the recording's tracks, apply the
//! timeline's trims and speeds, and mix them down to one interleaved stream the
//! encoder can take. This replaces the FFmpeg filter graph the Electron shell
//! shells out to, so the native shell needs no external binary.

use std::path::Path;

use crate::video::encoder::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};
use crate::windows::video_editor::model::Segment;

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

/// Slices `samples` to the timeline the editor's segments describe, applying
/// each segment's speed. Gaps outside the source are silence, so a trim past
/// the end of a track does not shorten the mix.
pub fn apply_segments(samples: &[i16], segments: &[Segment]) -> Pcm {
    if segments.is_empty() {
        return samples.to_vec();
    }
    let mut output: Pcm = Vec::new();
    for segment in segments {
        let start = frames_for(segment.original_start);
        let end = frames_for(segment.original_end).max(start);
        let available = frames(samples);

        let mut slice: Pcm = Vec::with_capacity((end - start) * FRAME_SIZE);
        for frame in start..end {
            if frame < available {
                let offset = frame * FRAME_SIZE;
                slice.extend_from_slice(&samples[offset..offset + FRAME_SIZE]);
            } else {
                slice.extend(std::iter::repeat_n(0i16, FRAME_SIZE));
            }
        }

        let speed = segment.speed.unwrap_or(1.0);
        if (speed - 1.0).abs() > f64::EPSILON {
            slice = time_stretch(&slice, speed);
        }
        output.extend_from_slice(&slice);
    }
    output
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

/// Places a music track on the timeline: the trimmed slice of the source is
/// speed-adjusted and laid down at the track's start, with silence around it.
pub fn place_music_track(
    samples: &[i16],
    track: &crate::windows::video_editor::model::MusicTrack,
) -> Pcm {
    let available = frames(samples);
    let start = frames_for(track.trim_start).min(available);
    let end = frames_for(track.original_duration.max(0.0) - track.trim_end).clamp(start, available);

    let mut slice: Pcm = samples[start * FRAME_SIZE..end * FRAME_SIZE].to_vec();
    if (track.speed - 1.0).abs() > f64::EPSILON {
        slice = time_stretch(&slice, track.speed);
    }

    let offset = frames_for(track.start_time);
    let mut placed: Pcm = vec![0; offset * FRAME_SIZE];
    placed.extend_from_slice(&slice);

    // The track's own end wins over the source length, so a shortened clip
    // does not bleed past where the timeline places it.
    let limit = frames_for(track.end_time) * FRAME_SIZE;
    if limit > 0 && placed.len() > limit {
        placed.truncate(limit);
    }
    placed
}

const STRETCH_WINDOW_FRAMES: usize = 1024;
const STRETCH_OVERLAP_FRAMES: usize = 256;

/// Pitch-preserving speed change by overlap-add (SOLA), which is what the
/// Electron export gets from FFmpeg's `atempo`. `speed` above 1 shortens.
pub fn time_stretch(samples: &[i16], speed: f64) -> Pcm {
    let speed = speed.clamp(0.05, 20.0);
    let total = frames(samples);
    if total == 0 || (speed - 1.0).abs() <= f64::EPSILON {
        return samples.to_vec();
    }

    let window = STRETCH_WINDOW_FRAMES.min(total.max(1));
    let overlap = STRETCH_OVERLAP_FRAMES.min(window / 2);
    let hop_out = window - overlap;
    let hop_in = ((hop_out as f64) * speed).round().max(1.0) as usize;
    let output_frames = ((total as f64) / speed).round().max(1.0) as usize;

    let mut output = vec![0.0_f32; output_frames * FRAME_SIZE];
    let mut weights = vec![0.0_f32; output_frames];

    let mut read = 0usize;
    let mut write = 0usize;
    while read < total && write < output_frames {
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
        read += hop_in;
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
                    .chunks_exact(2)
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

#[cfg(not(windows))]
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

    fn segment(start: f64, end: f64, speed: Option<f64>) -> Segment {
        Segment {
            id: format!("{start}-{end}"),
            original_start: start,
            original_end: end,
            trim_min_start: start,
            trim_max_end: end,
            speed,
        }
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
    fn segments_trim_the_source() {
        let samples = tone(AUDIO_SAMPLE_RATE as usize, 8000);
        let trimmed = apply_segments(&samples, &[segment(0.25, 0.75, None)]);
        assert_eq!(frames(&trimmed), AUDIO_SAMPLE_RATE as usize / 2);
    }

    #[test]
    fn a_trim_past_the_end_pads_with_silence() {
        let samples = tone(100, 8000);
        let trimmed = apply_segments(&samples, &[segment(0.0, 0.01, None)]);
        assert_eq!(frames(&trimmed), 480);
        assert_eq!(trimmed[trimmed.len() - 1], 0);
    }

    #[test]
    fn speeding_a_segment_up_shortens_it() {
        let samples = tone(AUDIO_SAMPLE_RATE as usize, 8000);
        let doubled = apply_segments(&samples, &[segment(0.0, 1.0, Some(2.0))]);
        let expected = AUDIO_SAMPLE_RATE as usize / 2;
        assert!(
            (frames(&doubled) as i64 - expected as i64).abs() < 64,
            "{}",
            frames(&doubled)
        );
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
    fn a_unit_speed_stretch_is_the_identity() {
        let samples = tone(1000, 5000);
        assert_eq!(time_stretch(&samples, 1.0), samples);
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
        let placed = place_music_track(&samples, &track);
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
        assert!(decode(&std::env::temp_dir().join("poratake-missing.m4a")).is_none());
    }
}
