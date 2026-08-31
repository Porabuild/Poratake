use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::editor::annotations::Annotation;
use crate::video::project;
use crate::windows::video_editor::styles::{
    AudioStyle, CameraStyle, CursorStyle, ExportSettings, FirstFrameSettings, KeyboardStyle,
    SubtitleStyle, VideoWallpaperSettings, ZoomSettings,
};

pub const EDITOR_STATE_VERSION: u32 = 2;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub id: String,
    pub original_start: f64,
    pub original_end: f64,
    #[serde(default)]
    pub trim_min_start: f64,
    #[serde(default)]
    pub trim_max_end: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
}

impl Segment {
    /// `defaultSegments` in `video-editor-window.tsx`: one segment covering the
    /// whole recording, seeded as soon as the duration is known and there is no
    /// saved project. Without it the timeline has no clip to lay out -- the
    /// ruler is drawn and the lane is empty.
    pub fn spanning(duration: f64) -> Self {
        Self {
            id: format!("segment-{}", duration.to_bits()),
            original_start: 0.0,
            original_end: duration,
            trim_min_start: 0.0,
            trim_max_end: duration,
            speed: None,
        }
    }

    pub fn source_duration(&self) -> f64 {
        (self.original_end - self.original_start).max(0.0)
    }

    pub fn timeline_duration(&self) -> f64 {
        self.source_duration() / self.speed.unwrap_or(1.0).max(0.01)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct FocusPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomSegment {
    pub id: String,
    pub start_time: f64,
    pub end_time: f64,
    #[serde(default = "default_zoom_level")]
    pub zoom_level: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_in_duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_out_duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_point: Option<FocusPoint>,
}

fn default_zoom_level() -> f64 {
    1.2
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraSegment {
    pub id: String,
    pub start_time: f64,
    pub end_time: f64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawingSegment {
    pub id: String,
    pub start_time: f64,
    pub end_time: f64,
    #[serde(default)]
    pub canvas_width: f64,
    #[serde(default)]
    pub canvas_height: f64,
    #[serde(default)]
    pub annotations: Vec<Annotation>,
}

impl DrawingSegment {
    /// The timeline label comes from the first annotation's tool, the way
    /// `getAnnotationLabel` in `timeline/drawing-track.tsx` picks it.
    pub fn kind(&self) -> &'static str {
        self.annotations
            .first()
            .map(Annotation::kind)
            .unwrap_or("pen")
    }
}

/// Port of `MusicTrack` in `types/music.ts`. A track is a placed slice of an
/// audio file: `start_time`/`end_time` are timeline positions and
/// `trim_start`/`trim_end` are the source offsets they play from.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicTrack {
    pub id: String,
    #[serde(default)]
    pub group_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "music_source")]
    pub source: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default = "default_volume")]
    pub volume: f64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub start_time: f64,
    #[serde(default)]
    pub end_time: f64,
    #[serde(default)]
    pub original_duration: f64,
    #[serde(default)]
    pub trim_start: f64,
    #[serde(default)]
    pub trim_end: f64,
    #[serde(default = "default_speed")]
    pub speed: f64,
}

fn music_source() -> String {
    "music".to_string()
}

/// `DEFAULT_MUSIC_TRACK_VOLUME`.
fn default_volume() -> f64 {
    0.8
}

fn default_speed() -> f64 {
    1.0
}

impl Default for MusicTrack {
    fn default() -> Self {
        Self {
            id: String::new(),
            group_id: String::new(),
            name: String::new(),
            source: music_source(),
            file_name: String::new(),
            volume: default_volume(),
            enabled: true,
            start_time: 0.0,
            end_time: 0.0,
            original_duration: 0.0,
            trim_start: 0.0,
            trim_end: 0.0,
            speed: 1.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorUiState {
    #[serde(default = "default_true")]
    pub sidebar_open: bool,
    #[serde(default = "default_sidebar_tab")]
    pub sidebar_tab: String,
    #[serde(default)]
    pub scrub_audio_enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_sidebar_tab() -> String {
    "cursor".to_string()
}

impl Default for EditorUiState {
    fn default() -> Self {
        Self {
            sidebar_open: true,
            sidebar_tab: default_sidebar_tab(),
            scrub_audio_enabled: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoEditorState {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub saved_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_duration: Option<f64>,
    #[serde(default)]
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub cursor_style: CursorStyle,
    #[serde(default)]
    pub camera_style: CameraStyle,
    #[serde(default)]
    pub keyboard_style: KeyboardStyle,
    #[serde(default)]
    pub subtitle_style: SubtitleStyle,
    #[serde(default)]
    pub audio_style: AudioStyle,
    #[serde(default)]
    pub zoom_settings: ZoomSettings,
    #[serde(default)]
    pub wallpaper: VideoWallpaperSettings,
    #[serde(default)]
    pub first_frame: FirstFrameSettings,
    #[serde(default)]
    pub export_settings: ExportSettings,
    #[serde(default)]
    pub zoom_segments: Vec<ZoomSegment>,
    #[serde(default)]
    pub camera_segments: Vec<CameraSegment>,
    #[serde(default)]
    pub drawing_segments: Vec<DrawingSegment>,
    #[serde(default)]
    pub music_tracks: Vec<MusicTrack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline_zoom: Option<f64>,
    #[serde(default)]
    pub ui: EditorUiState,
}

fn default_version() -> u32 {
    EDITOR_STATE_VERSION
}

impl Default for VideoEditorState {
    fn default() -> Self {
        Self {
            version: EDITOR_STATE_VERSION,
            saved_at: String::new(),
            recording_type: None,
            source_duration: None,
            segments: Vec::new(),
            cursor_style: CursorStyle::default(),
            camera_style: CameraStyle::default(),
            keyboard_style: KeyboardStyle::default(),
            subtitle_style: SubtitleStyle::default(),
            audio_style: AudioStyle::default(),
            zoom_settings: ZoomSettings::default(),
            wallpaper: VideoWallpaperSettings::default(),
            first_frame: FirstFrameSettings::default(),
            export_settings: ExportSettings::default(),
            zoom_segments: Vec::new(),
            camera_segments: Vec::new(),
            drawing_segments: Vec::new(),
            music_tracks: Vec::new(),
            timeline_zoom: None,
            ui: EditorUiState::default(),
        }
    }
}

pub fn load_state(project_or_video: &Path) -> VideoEditorState {
    let path = project::editor_state_path(project_or_video);
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return VideoEditorState::default();
    };
    match serde_json::from_str(&contents) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("[video-editor] failed to parse {}: {error}", path.display());
            VideoEditorState::default()
        }
    }
}

pub fn save_state(project_or_video: &Path, state: &VideoEditorState) {
    let path = project::editor_state_path(project_or_video);
    let Some(parent) = path.parent() else {
        return;
    };
    if !parent.is_dir() {
        return;
    }
    match serde_json::to_string_pretty(state) {
        Ok(contents) => {
            if let Err(error) = std::fs::write(&path, contents) {
                eprintln!("[video-editor] failed to save {}: {error}", path.display());
            }
        }
        Err(error) => eprintln!("[video-editor] failed to serialize state: {error}"),
    }
}

/// `getFileNameFromPath` in `video-editor/utils.ts`: a recording inside a
/// `.poratake` directory is named after the directory, and anything else is the
/// file name with its extension removed. This used to strip only the project
/// extension, so a plain recording kept its `.mp4` in the title bar.
pub fn project_display_name(path: &Path) -> String {
    // `parts[parts.length - 2]` -- the containing directory.
    if let Some(parent) = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
    {
        if let Some(stem) = parent.strip_suffix(project::PROJECT_EXTENSION) {
            return stem.to_string();
        }
    }

    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if let Some(stem) = name.strip_suffix(project::PROJECT_EXTENSION) {
        return stem.to_string();
    }
    // `lastDot > 0`: a leading dot is part of the name, not an extension.
    match name.rfind('.') {
        Some(index) if index > 0 => name[..index].to_string(),
        _ => name.to_string(),
    }
}

pub fn poster_frame(project_or_video: &Path) -> Option<PathBuf> {
    crate::thumbnails::cached(project_or_video)
}

pub fn total_duration(segments: &[Segment], fallback: f64) -> f64 {
    if segments.is_empty() {
        return fallback.max(0.0);
    }
    segments.iter().map(Segment::timeline_duration).sum()
}

/// Port of `formatTime` in `video-editor/utils.ts`.
pub fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0);
    let minutes = (total / 60.0).floor() as i64;
    let secs = (total % 60.0).floor() as i64;
    format!("{minutes}:{secs:02}")
}

/// Port of `formatDuration` in `video-editor/utils.ts`.
pub fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        return format!("{}s", (seconds * 10.0).round() / 10.0);
    }
    let minutes = (seconds / 60.0).floor() as i64;
    let secs = (seconds % 60.0).round() as i64;
    format!("{minutes}m{secs}s")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_times_like_the_renderer() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(9.4), "0:09");
        assert_eq!(format_time(61.0), "1:01");
        assert_eq!(format_time(600.0), "10:00");
    }

    #[test]
    fn formats_durations_like_the_renderer() {
        assert_eq!(format_duration(3.25), "3.3s");
        assert_eq!(format_duration(90.0), "1m30s");
    }

    #[test]
    fn timeline_duration_accounts_for_speed() {
        let segment = Segment {
            id: "a".into(),
            original_start: 0.0,
            original_end: 10.0,
            trim_min_start: 0.0,
            trim_max_end: 10.0,
            speed: Some(2.0),
        };
        assert_eq!(segment.source_duration(), 10.0);
        assert_eq!(segment.timeline_duration(), 5.0);
        assert_eq!(total_duration(&[segment], 0.0), 5.0);
        assert_eq!(total_duration(&[], 12.0), 12.0);
    }

    #[test]
    fn strips_the_project_extension_from_display_names() {
        assert_eq!(
            project_display_name(Path::new("/tmp/Take 1.poratake")),
            "Take 1"
        );
        // This used to assert `clip.mp4`, which was the bug rather than the
        // contract: `getFileNameFromPath` strips whatever follows the last dot,
        // so a recording's extension never reaches the title bar.
        assert_eq!(project_display_name(Path::new("/tmp/clip.mp4")), "clip");
    }
}

#[cfg(test)]
mod duration_tests {
    use super::*;

    /// `handleBootstrapMetadata` fills the duration in from the decoded media,
    /// and a project with no saved duration has to end up with the decoded one
    /// or the timeline lays out against zero.
    #[test]
    fn a_project_without_a_saved_duration_has_nothing_to_lay_out() {
        let state = VideoEditorState::default();
        assert_eq!(state.source_duration, None);
        assert_eq!(
            total_duration(&state.segments, state.source_duration.unwrap_or(0.0)),
            0.0,
            "which is exactly why the decoded duration has to be adopted"
        );
    }

    /// And once it is adopted, the total follows it.
    #[test]
    fn adopting_the_decoded_duration_gives_the_timeline_its_length() {
        let state = VideoEditorState {
            source_duration: Some(3.0),
            ..VideoEditorState::default()
        };
        assert_eq!(
            total_duration(&state.segments, state.source_duration.unwrap_or(0.0)),
            3.0
        );
        assert_eq!(format_time(3.0), "0:03");
    }
}

#[cfg(test)]
mod display_name_tests {
    use super::*;

    #[test]
    fn a_recording_is_named_without_its_extension() {
        #[cfg(windows)]
        assert_eq!(
            project_display_name(Path::new(r"C:\clips\parity-clip.mp4")),
            "parity-clip"
        );
        assert_eq!(
            project_display_name(Path::new("/clips/Screen Recording 2026.mov")),
            "Screen Recording 2026"
        );
    }

    #[test]
    fn a_recording_inside_a_project_takes_the_project_name() {
        let path = format!("/clips/Demo{}/source.mp4", project::PROJECT_EXTENSION);
        assert_eq!(project_display_name(Path::new(&path)), "Demo");
    }

    #[test]
    fn a_project_path_itself_loses_only_the_project_extension() {
        let path = format!("/clips/Demo{}", project::PROJECT_EXTENSION);
        assert_eq!(project_display_name(Path::new(&path)), "Demo");
    }

    #[test]
    fn a_leading_dot_is_part_of_the_name_and_not_an_extension() {
        // `lastDot > 0` in the reference.
        assert_eq!(project_display_name(Path::new("/clips/.hidden")), ".hidden");
        assert_eq!(project_display_name(Path::new("/clips/noext")), "noext");
    }
}
