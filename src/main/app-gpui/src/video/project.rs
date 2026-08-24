use std::path::{Path, PathBuf};

pub const PROJECT_EXTENSION: &str = ".poratake";

pub const RECORDING: &str = "recording.mov";
pub const SYSTEM_AUDIO: &str = "system.m4a";
pub const MIC_AUDIO: &str = "mic.m4a";
pub const CURSOR: &str = "cursor.json";
pub const CAMERA_VIDEO: &str = "camera.mov";
pub const CAMERA_META: &str = "camera.json";
pub const KEYS: &str = "keys.json";
pub const EDITOR_STATE: &str = "state.json";
pub const SUBTITLE: &str = "subtitle.json";
pub const MUSIC_FOLDER: &str = "music";

pub fn is_project(path: &Path) -> bool {
    path.to_string_lossy().ends_with(PROJECT_EXTENSION)
}

pub fn project_folder(path: &Path) -> Option<PathBuf> {
    if is_project(path) {
        return Some(path.to_path_buf());
    }
    let parent = path.parent()?;
    is_project(parent).then(|| parent.to_path_buf())
}

fn sidecar(path: &Path, project_file: &str, loose_suffix: &str) -> PathBuf {
    match project_folder(path) {
        Some(folder) => folder.join(project_file),
        None => path.with_extension(loose_suffix.trim_start_matches('.')),
    }
}

pub fn recording_video_path(path: &Path) -> PathBuf {
    match project_folder(path) {
        Some(folder) => folder.join(RECORDING),
        None => path.to_path_buf(),
    }
}

pub fn system_audio_path(path: &Path) -> PathBuf {
    sidecar(path, SYSTEM_AUDIO, "system.m4a")
}

pub fn mic_audio_path(path: &Path) -> PathBuf {
    sidecar(path, MIC_AUDIO, "mic.m4a")
}

pub fn cursor_path(path: &Path) -> PathBuf {
    sidecar(path, CURSOR, "cursor.json")
}

pub fn camera_video_path(path: &Path) -> PathBuf {
    sidecar(path, CAMERA_VIDEO, "camera.mov")
}

pub fn camera_meta_path(path: &Path) -> PathBuf {
    sidecar(path, CAMERA_META, "camera.json")
}

pub fn keys_path(path: &Path) -> PathBuf {
    sidecar(path, KEYS, "keys.json")
}

pub fn editor_state_path(path: &Path) -> PathBuf {
    sidecar(path, EDITOR_STATE, "state.json")
}

pub fn subtitle_path(path: &Path) -> PathBuf {
    sidecar(path, SUBTITLE, "subtitle.json")
}

pub fn music_folder(path: &Path) -> Option<PathBuf> {
    project_folder(path).map(|folder| folder.join(MUSIC_FOLDER))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecordingFeatures {
    pub has_mic: bool,
    pub has_system_audio: bool,
    pub has_camera: bool,
    pub has_cursor: bool,
}

pub fn recording_features(path: &Path) -> RecordingFeatures {
    if project_folder(path).is_none() {
        return RecordingFeatures::default();
    }
    RecordingFeatures {
        has_mic: mic_audio_path(path).is_file(),
        has_system_audio: system_audio_path(path).is_file(),
        has_camera: camera_video_path(path).is_file(),
        has_cursor: cursor_path(path).is_file(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_project_sidecars_inside_the_project_folder() {
        let project = PathBuf::from("/tmp/Take 1.poratake");
        assert_eq!(project_folder(&project), Some(project.clone()));
        assert_eq!(mic_audio_path(&project), project.join(MIC_AUDIO));

        let inner = project.join(RECORDING);
        assert_eq!(project_folder(&inner), Some(project.clone()));
        assert_eq!(cursor_path(&inner), project.join(CURSOR));
    }

    #[test]
    fn falls_back_to_sibling_files_for_loose_videos() {
        let loose = PathBuf::from("/tmp/clip.mp4");
        assert_eq!(project_folder(&loose), None);
        assert_eq!(
            camera_meta_path(&loose),
            PathBuf::from("/tmp/clip.camera.json")
        );
        assert_eq!(recording_video_path(&loose), loose);
        assert_eq!(recording_features(&loose), RecordingFeatures::default());
    }
}
