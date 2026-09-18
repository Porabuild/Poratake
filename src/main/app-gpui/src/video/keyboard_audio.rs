use std::path::PathBuf;

use crate::video::audio::{self, Pcm};
use crate::video::composition::segments;
use crate::video::encoder::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};
use crate::video::sidecars::KeyboardData;
use crate::windows::video_editor::model::Segment;

pub const SAMPLES_PER_TYPE: u32 = 4;

const FRAME_SIZE: usize = AUDIO_CHANNELS as usize;

pub fn sample_index(timestamp: f64, count: u32) -> u32 {
    if count == 0 {
        return 0;
    }
    let hash = (timestamp * 1000.0).round() * 2_654_435_761.0;
    (hash.abs() % count as f64) as u32
}

pub fn sample_path(sound_type: &str, index: u32) -> Option<PathBuf> {
    let relative = PathBuf::from("public")
        .join("sounds")
        .join("keyboard")
        .join(sound_type)
        .join(format!("press-{index}.mp3"));
    if relative.is_file() {
        return Some(relative);
    }
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        roots.push(exe);
    }
    for root in roots {
        for ancestor in root.ancestors() {
            let candidate = ancestor.join(&relative);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn press_times(data: &KeyboardData, timeline: &[Segment], duration: f64) -> Vec<f64> {
    data.events
        .iter()
        .filter(|event| event.kind == "down")
        .map(|event| segments::map_video_to_timeline(event.timestamp, timeline))
        .filter(|time| *time >= 0.0 && *time < duration)
        .collect()
}

pub fn render(
    data: &KeyboardData,
    timeline: &[Segment],
    sound_type: &str,
    duration: f64,
) -> Option<Pcm> {
    if duration <= 0.0 {
        return None;
    }
    let presses = press_times(data, timeline, duration);
    if presses.is_empty() {
        return None;
    }

    let samples: Vec<Option<Pcm>> = (0..SAMPLES_PER_TYPE)
        .map(|index| sample_path(sound_type, index + 1).and_then(|path| audio::decode(&path)))
        .collect();
    if samples.iter().all(Option::is_none) {
        return None;
    }

    let frames = (duration * AUDIO_SAMPLE_RATE as f64).round() as usize;
    let mut mixed = vec![0.0_f32; frames * FRAME_SIZE];
    for press in presses {
        let Some(sample) = samples[sample_index(press, SAMPLES_PER_TYPE) as usize].as_ref() else {
            continue;
        };
        let offset = (press * AUDIO_SAMPLE_RATE as f64).round().max(0.0) as usize * FRAME_SIZE;
        if offset >= mixed.len() {
            continue;
        }
        let length = sample.len().min(mixed.len() - offset);
        for (target, value) in mixed[offset..offset + length]
            .iter_mut()
            .zip(&sample[..length])
        {
            *target += *value as f32;
        }
    }

    Some(
        mixed
            .into_iter()
            .map(|value| value.clamp(i16::MIN as f32, i16::MAX as f32) as i16)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video::sidecars::KeyboardKeyEvent;

    fn event(timestamp: f64, kind: &str) -> KeyboardKeyEvent {
        KeyboardKeyEvent {
            timestamp,
            kind: kind.to_string(),
            ..KeyboardKeyEvent::default()
        }
    }

    fn segment(start: f64, end: f64) -> Segment {
        Segment {
            id: format!("{start}-{end}"),
            original_start: start,
            original_end: end,
            trim_min_start: start,
            trim_max_end: end,
            speed: None,
        }
    }

    #[test]
    fn only_key_downs_inside_the_timeline_are_clicked() {
        let data = KeyboardData {
            events: vec![
                event(0.5, "down"),
                event(0.6, "up"),
                event(7.0, "down"),
                event(9.5, "down"),
            ],
            ..KeyboardData::default()
        };
        let timeline = [segment(0.0, 2.0), segment(6.0, 8.0)];
        assert_eq!(press_times(&data, &timeline, 4.0), vec![0.5, 3.0]);
    }

    #[test]
    fn the_sample_choice_matches_the_electron_hash() {
        assert_eq!(sample_index(0.0, 4), 0);
        assert_eq!(sample_index(1.234, 4), {
            let hash = 1234.0_f64 * 2_654_435_761.0;
            (hash % 4.0) as u32
        });
        assert!(sample_index(12.345, 4) < 4);
    }

    #[test]
    fn a_project_without_presses_renders_no_track() {
        let data = KeyboardData::default();
        assert!(render(&data, &[segment(0.0, 2.0)], "cherry-blue", 2.0).is_none());
    }
}
