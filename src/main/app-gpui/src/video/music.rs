//! Music tracks — port of `main/capture/video/ipc/music-handlers.ts`. A picked
//! file is copied into the project's `music/` folder so the project stays
//! self-contained, and its duration is probed by decoding it.

use std::path::{Path, PathBuf};

use crate::video::encoder::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};
use crate::video::project;
use crate::windows::video_editor::model::MusicTrack;

/// `SUPPORTED_MUSIC_EXTENSIONS` in `types/music.ts`.
pub const SUPPORTED_EXTENSIONS: [&str; 5] = ["mp3", "m4a", "wav", "aac", "ogg"];

/// `DEFAULT_MUSIC_TRACK_VOLUME`.
pub const DEFAULT_VOLUME: f64 = 0.8;

/// `getUniqueFileName` — an existing name gains a ` (n)` suffix rather than
/// overwriting the file already in the folder.
pub fn unique_file_name(folder: &Path, original: &str) -> String {
    if !folder.join(original).exists() {
        return original.to_string();
    }
    let path = Path::new(original);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("audio");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();

    let mut counter = 1;
    loop {
        let candidate = format!("{stem} ({counter}){extension}");
        if !folder.join(&candidate).exists() {
            return candidate;
        }
        counter += 1;
    }
}

/// Copies `source` into the project's music folder and returns the placed
/// track. `timeline_start` is where it lands on the timeline.
pub fn add(project_path: &Path, source: &Path, timeline_start: f64) -> Result<MusicTrack, String> {
    let folder = project::music_folder(project_path)
        .ok_or_else(|| "this recording has no project folder".to_string())?;
    std::fs::create_dir_all(&folder)
        .map_err(|error| format!("could not create the music folder: {error}"))?;

    let original = source
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "the file has no name".to_string())?;
    let file_name = unique_file_name(&folder, original);
    let destination = folder.join(&file_name);
    std::fs::copy(source, &destination)
        .map_err(|error| format!("could not copy the file: {error}"))?;

    let duration = probe_duration(&destination);
    if duration <= 0.0 {
        let _ = std::fs::remove_file(&destination);
        return Err("Could not determine audio duration".to_string());
    }

    let name = Path::new(&file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(&file_name)
        .to_string();
    let id = format!("music-{}", sanitize_id(&file_name));

    Ok(MusicTrack {
        id: id.clone(),
        group_id: id,
        name,
        source: "music".to_string(),
        file_name,
        volume: DEFAULT_VOLUME,
        enabled: true,
        start_time: timeline_start,
        end_time: timeline_start + duration,
        original_duration: duration,
        trim_start: 0.0,
        trim_end: 0.0,
        speed: 1.0,
    })
}

/// Removes a track's file once no other track still references it.
pub fn remove(project_path: &Path, file_name: &str, still_referenced: bool) {
    if still_referenced || file_name.is_empty() {
        return;
    }
    let Some(folder) = project::music_folder(project_path) else {
        return;
    };
    let _ = std::fs::remove_file(folder.join(file_name));
}

/// `probeAudioDuration` — decoding is the only probe this shell has, and it is
/// exact.
pub fn probe_duration(path: &Path) -> f64 {
    let Some(samples) = crate::video::audio::decode(path) else {
        return 0.0;
    };
    let frames = samples.len() / AUDIO_CHANNELS as usize;
    frames as f64 / AUDIO_SAMPLE_RATE as f64
}

/// Ids end up in `state.json`, so they are kept to characters that read back
/// cleanly rather than whatever the file name happened to contain.
fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

/// Opens the picker `video-editor:music:add` opens.
pub fn pick_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Add Music")
        .add_filter("Audio Files", &SUPPORTED_EXTENSIONS)
        .pick_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_free_name_is_used_as_is() {
        let folder = std::env::temp_dir();
        assert_eq!(
            unique_file_name(&folder, "poratake-definitely-absent.mp3"),
            "poratake-definitely-absent.mp3"
        );
    }

    #[test]
    fn an_existing_name_gains_a_counter() {
        let folder = std::env::temp_dir().join("poratake-music-test");
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("song.mp3"), b"a").expect("write");
        assert_eq!(unique_file_name(&folder, "song.mp3"), "song (1).mp3");

        std::fs::write(folder.join("song (1).mp3"), b"a").expect("write");
        assert_eq!(unique_file_name(&folder, "song.mp3"), "song (2).mp3");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn ids_are_reduced_to_safe_characters() {
        assert_eq!(sanitize_id("My Song!.mp3"), "my-song--mp3");
    }

    #[test]
    fn adding_to_a_loose_video_is_refused() {
        let error = add(Path::new("/tmp/clip.mp4"), Path::new("/tmp/song.mp3"), 0.0).unwrap_err();
        assert!(error.contains("project folder"), "{error}");
    }

    #[test]
    fn a_referenced_file_is_kept() {
        let folder = std::env::temp_dir().join("poratake-music-keep.poratake/music");
        std::fs::create_dir_all(&folder).expect("folder");
        let file = folder.join("song.mp3");
        std::fs::write(&file, b"a").expect("write");

        let project = std::env::temp_dir().join("poratake-music-keep.poratake");
        remove(&project, "song.mp3", true);
        assert!(file.exists());

        remove(&project, "song.mp3", false);
        assert!(!file.exists());
        let _ = std::fs::remove_dir_all(&project);
    }
}
