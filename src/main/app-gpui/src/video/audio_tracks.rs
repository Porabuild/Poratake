use std::path::{Path, PathBuf};

use crate::video::audio::Placement;
use crate::video::composition::segments;
use crate::video::project;
use crate::windows::video_editor::model::{MusicTrack, VideoEditorState};

pub const SYSTEM_TRACK_ID: &str = "system-audio";
pub const MIC_TRACK_ID: &str = "mic-audio";

pub const SYSTEM_SOURCE: &str = "system";
pub const MIC_SOURCE: &str = "mic";
pub const MUSIC_SOURCE: &str = "music";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sources {
    pub system: Option<PathBuf>,
    pub system_is_embedded: bool,
    pub mic: Option<PathBuf>,
    pub music_folder: Option<PathBuf>,
}

impl Sources {
    pub fn resolve(project_path: &Path) -> Self {
        let sidecar = project::system_audio_path(project_path);
        let (system, system_is_embedded) = match sidecar.is_file() {
            true => (Some(sidecar), false),
            false => (project::embedded_audio_path(project_path), true),
        };
        let mic = project::mic_audio_path(project_path);
        Self {
            system_is_embedded: system_is_embedded && system.is_some(),
            system,
            mic: mic.is_file().then_some(mic),
            music_folder: project::music_folder(project_path),
        }
    }

    pub fn path_for(&self, track: &MusicTrack) -> Option<PathBuf> {
        match track.source.as_str() {
            SYSTEM_SOURCE => self.system.clone(),
            MIC_SOURCE => self.mic.clone(),
            _ => {
                if track.file_name.is_empty() {
                    return None;
                }
                Some(self.music_folder.as_ref()?.join(&track.file_name))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum VolumeSource {
    SystemAudio,
    MicAudio,
    Keyboard,
    Track(String),
}

impl VolumeSource {
    pub fn resolve(&self, state: &VideoEditorState) -> f64 {
        let style = &state.audio_style;
        match self {
            Self::SystemAudio => gated(style.system_audio_enabled, style.system_audio_volume),
            Self::MicAudio => gated(style.mic_audio_enabled, style.mic_audio_volume),
            Self::Keyboard => gated(style.keyboard_sound_enabled, style.keyboard_sound_volume),
            Self::Track(id) => state
                .music_tracks
                .iter()
                .find(|track| &track.id == id)
                .map(|track| gated(track.enabled, track.volume))
                .unwrap_or(0.0),
        }
    }
}

fn gated(enabled: bool, volume: f64) -> f64 {
    if !enabled {
        return 0.0;
    }
    volume.max(0.0)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stem {
    pub id: String,
    pub path: PathBuf,
    pub placement: Placement,
    pub volume: VolumeSource,
    pub program: bool,
}

pub fn tracks(
    sources: &Sources,
    state: &VideoEditorState,
    source_duration: f64,
) -> Vec<(MusicTrack, VolumeSource)> {
    let mut result: Vec<(MusicTrack, VolumeSource)> = built_in(sources, state, source_duration)
        .into_iter()
        .filter(|(built_in, _)| {
            !state
                .music_tracks
                .iter()
                .any(|track| track.source == built_in.source)
        })
        .collect();
    result.extend(
        state
            .music_tracks
            .iter()
            .map(|track| (track.clone(), VolumeSource::Track(track.id.clone()))),
    );
    result
}

pub fn stems(sources: &Sources, state: &VideoEditorState, source_duration: f64) -> Vec<Stem> {
    tracks(sources, state, source_duration)
        .into_iter()
        .filter(|(track, _)| track.enabled)
        .filter_map(|(track, volume)| {
            let path = sources.path_for(&track)?;
            Some(Stem {
                id: track.id.clone(),
                program: track.source != MUSIC_SOURCE,
                placement: Placement::of(&track),
                path,
                volume,
            })
        })
        .collect()
}

fn built_in(
    sources: &Sources,
    state: &VideoEditorState,
    source_duration: f64,
) -> Vec<(MusicTrack, VolumeSource)> {
    let duration = built_in_duration(state, source_duration);
    if duration <= 0.0 {
        return Vec::new();
    }
    let style = &state.audio_style;
    let mut result = Vec::new();
    if sources.system.is_some() {
        let name = match sources.system_is_embedded {
            true => "Audio",
            false => "System Audio",
        };
        push_clips(
            &mut result,
            Built {
                id: SYSTEM_TRACK_ID,
                source: SYSTEM_SOURCE,
                name,
                enabled: style.system_audio_enabled,
                volume: style.system_audio_volume,
                volume_source: VolumeSource::SystemAudio,
            },
            state,
            duration,
        );
    }
    if sources.mic.is_some() {
        push_clips(
            &mut result,
            Built {
                id: MIC_TRACK_ID,
                source: MIC_SOURCE,
                name: "Microphone",
                enabled: style.mic_audio_enabled,
                volume: style.mic_audio_volume,
                volume_source: VolumeSource::MicAudio,
            },
            state,
            duration,
        );
    }
    result
}

struct Built {
    id: &'static str,
    source: &'static str,
    name: &'static str,
    enabled: bool,
    volume: f64,
    volume_source: VolumeSource,
}

fn push_clips(
    result: &mut Vec<(MusicTrack, VolumeSource)>,
    built: Built,
    state: &VideoEditorState,
    duration: f64,
) {
    let template = MusicTrack {
        id: built.id.to_string(),
        group_id: built.id.to_string(),
        name: built.name.to_string(),
        source: built.source.to_string(),
        file_name: String::new(),
        volume: built.volume,
        enabled: built.enabled,
        start_time: 0.0,
        end_time: duration,
        original_duration: duration,
        trim_start: 0.0,
        trim_end: 0.0,
        speed: 1.0,
    };

    if state.segments.is_empty() {
        result.push((template, built.volume_source));
        return;
    }

    for (index, segment) in segments::to_video_segments(&state.segments)
        .into_iter()
        .enumerate()
    {
        let speed = if segment.speed <= 0.0 {
            1.0
        } else {
            segment.speed
        };
        let length = (segment.end_time - segment.start_time) / speed;
        if length <= 0.0 {
            continue;
        }
        result.push((
            MusicTrack {
                id: format!("{}-{index}", built.id),
                start_time: segment.timeline_start,
                end_time: segment.timeline_start + length,
                trim_start: segment.start_time,
                trim_end: duration - segment.end_time,
                speed,
                ..template.clone()
            },
            built.volume_source.clone(),
        ));
    }
}

fn built_in_duration(state: &VideoEditorState, source_duration: f64) -> f64 {
    if source_duration > 0.0 {
        return source_duration;
    }
    state
        .segments
        .iter()
        .map(|segment| segment.original_end)
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows::video_editor::model::Segment;

    fn sources() -> Sources {
        Sources {
            system: Some(PathBuf::from("/project/system.m4a")),
            system_is_embedded: false,
            mic: Some(PathBuf::from("/project/mic.m4a")),
            music_folder: Some(PathBuf::from("/project/music")),
        }
    }

    fn state_with_segments(segments: Vec<Segment>) -> VideoEditorState {
        VideoEditorState {
            segments,
            source_duration: Some(10.0),
            ..VideoEditorState::default()
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
    fn an_uncut_project_gets_one_clip_per_built_in_source() {
        let state = state_with_segments(vec![segment(0.0, 10.0)]);
        let built = stems(&sources(), &state, 10.0);
        assert_eq!(built.len(), 2);
        assert!(built.iter().all(|stem| stem.program));
        assert_eq!(built[0].placement.start_time, 0.0);
        assert_eq!(built[0].placement.end_time, 10.0);
        assert_eq!(built[0].placement.trim_start, 0.0);
        assert_eq!(built[0].placement.trim_end, 0.0);
    }

    #[test]
    fn a_cut_timeline_places_one_clip_per_segment() {
        let state = state_with_segments(vec![segment(0.0, 2.0), segment(6.0, 10.0)]);
        let built = stems(&sources(), &state, 10.0);
        let system: Vec<_> = built
            .iter()
            .filter(|stem| stem.path.ends_with("system.m4a"))
            .collect();
        assert_eq!(system.len(), 2);
        assert_eq!(system[0].placement.start_time, 0.0);
        assert_eq!(system[0].placement.end_time, 2.0);
        assert_eq!(system[1].placement.start_time, 2.0);
        assert_eq!(system[1].placement.end_time, 6.0);
        assert_eq!(system[1].placement.trim_start, 6.0);
        assert_eq!(system[1].placement.trim_end, 0.0);
    }

    #[test]
    fn a_muted_built_in_source_renders_no_stem() {
        let mut state = state_with_segments(vec![segment(0.0, 10.0)]);
        state.audio_style.system_audio_enabled = false;
        let built = stems(&sources(), &state, 10.0);
        assert_eq!(built.len(), 1);
        assert!(built[0].path.ends_with("mic.m4a"));
        assert_eq!(VolumeSource::SystemAudio.resolve(&state), 0.0);
    }

    #[test]
    fn a_track_the_timeline_carries_replaces_the_built_in_one() {
        let mut state = state_with_segments(vec![segment(0.0, 10.0)]);
        state.music_tracks = vec![MusicTrack {
            id: "system-audio".into(),
            source: SYSTEM_SOURCE.into(),
            start_time: 1.0,
            end_time: 4.0,
            original_duration: 10.0,
            trim_start: 2.0,
            ..MusicTrack::default()
        }];
        let built = stems(&sources(), &state, 10.0);
        let system: Vec<_> = built
            .iter()
            .filter(|stem| stem.path.ends_with("system.m4a"))
            .collect();
        assert_eq!(system.len(), 1);
        assert_eq!(system[0].placement.start_time, 1.0);
        assert_eq!(system[0].placement.trim_start, 2.0);
        assert_eq!(system[0].volume, VolumeSource::Track("system-audio".into()));
    }

    #[test]
    fn a_disabled_music_track_is_dropped_from_the_stems() {
        let mut state = state_with_segments(vec![segment(0.0, 10.0)]);
        state.music_tracks = vec![MusicTrack {
            id: "music-song".into(),
            file_name: "song.mp3".into(),
            enabled: false,
            ..MusicTrack::default()
        }];
        assert!(stems(&sources(), &state, 10.0)
            .iter()
            .all(|stem| stem.id != "music-song"));
    }

    #[test]
    fn a_recording_without_sidecars_falls_back_to_its_embedded_audio() {
        let directory = tempfile::tempdir().expect("temp directory");
        let project = directory.path().join("Take 1.poratake");
        std::fs::create_dir(&project).expect("project folder");
        std::fs::write(project.join(project::RECORDING), []).expect("recording");

        let sources = Sources::resolve(&project);
        assert!(sources.system_is_embedded);
        assert_eq!(sources.system, Some(project.join(project::RECORDING)));
        assert_eq!(sources.mic, None);

        let state = state_with_segments(vec![segment(0.0, 10.0)]);
        let built = tracks(&sources, &state, 10.0);
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].0.name, "Audio");
    }
}
