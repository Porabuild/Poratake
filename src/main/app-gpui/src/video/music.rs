//! Music tracks — port of `main/capture/video/ipc/music-handlers.ts`. A picked
//! file is copied into the project's `music/` folder so the project stays
//! self-contained, and its duration is probed by decoding it.

use std::path::Path;

use crate::video::encoder::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};
use crate::video::project;
use crate::windows::video_editor::model::MusicTrack;

/// `SUPPORTED_MUSIC_EXTENSIONS` in `types/music.ts`.
pub const SUPPORTED_EXTENSIONS: [&str; 5] = ["mp3", "m4a", "wav", "aac", "ogg"];

/// `DEFAULT_MUSIC_TRACK_VOLUME`.
pub const DEFAULT_VOLUME: f64 = 0.8;

pub(crate) fn file_path(folder: &Path, file_name: &str) -> Option<std::path::PathBuf> {
    let name = std::ffi::OsStr::new(file_name);
    if Path::new(file_name).file_name() != Some(name) {
        return None;
    }
    match std::fs::symlink_metadata(folder) {
        Ok(metadata) if !metadata.file_type().is_dir() => return None,
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return None,
        _ => {}
    }
    let path = folder.join(name);
    if std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return None;
    }
    Some(path)
}

/// `getUniqueFileName` — an existing name gains a ` (n)` suffix rather than
/// overwriting the file already in the folder.
pub fn unique_file_name(folder: &Path, original: &str) -> String {
    if std::fs::symlink_metadata(folder.join(original)).is_err() {
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
        if std::fs::symlink_metadata(folder.join(&candidate)).is_err() {
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
    if !std::fs::symlink_metadata(&folder).is_ok_and(|metadata| metadata.file_type().is_dir()) {
        return Err("the music folder is not a directory".to_string());
    }

    let original = source
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "the file has no name".to_string())?;
    let mut input =
        std::fs::File::open(source).map_err(|error| format!("could not open the file: {error}"))?;
    let (file_name, destination, mut output) = loop {
        let file_name = unique_file_name(&folder, original);
        let destination = folder.join(&file_name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
        {
            Ok(output) => break (file_name, destination, output),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("could not create the file: {error}")),
        }
    };
    if let Err(error) = std::io::copy(&mut input, &mut output) {
        let _ = std::fs::remove_file(&destination);
        return Err(format!("could not copy the file: {error}"));
    }
    drop(output);

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
        let folder = crate::util::test_paths::unique_temp("poratake-music-test");
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
    fn music_paths_cannot_escape_the_project() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("take.poratake");
        std::fs::create_dir_all(project.join("music")).unwrap();
        let outside = directory.path().join("outside.wav");
        std::fs::write(&outside, b"keep").unwrap();
        assert!(file_path(&project.join("music"), "../../outside.wav").is_none());
        assert!(outside.is_file());
        assert!(file_path(&project.join("music"), outside.to_str().unwrap()).is_none());
        assert!(outside.is_file());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_music_folder_cannot_delete_or_import_files() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("take.poratake");
        let outside = directory.path().join("outside");
        std::fs::create_dir(&project).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, project.join("music")).unwrap();
        let file = outside.join("source.wav");
        std::fs::write(&file, b"keep").unwrap();
        assert!(file_path(&project.join("music"), "source.wav").is_none());
        assert!(file.is_file());
        let source = directory.path().join("new.wav");
        std::fs::write(&source, b"source").unwrap();
        assert!(add(&project, &source, 0.0).is_err());
        assert!(!outside.join("new.wav").exists());
    }

    #[cfg(unix)]
    #[test]
    fn importing_music_cannot_follow_a_dangling_destination_symlink() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("take.poratake");
        let music = project.join("music");
        std::fs::create_dir_all(&music).unwrap();
        let outside = directory.path().join("outside.wav");
        std::os::unix::fs::symlink(&outside, music.join("source.wav")).unwrap();
        assert!(file_path(&music, "source.wav").is_none());
        assert_eq!(unique_file_name(&music, "source.wav"), "source (1).wav");
        let source = directory.path().join("source.wav");
        std::fs::write(&source, b"not an audio file").unwrap();
        let _ = add(&project, &source, 0.0);
        assert!(!outside.exists());
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[cfg_attr(target_os = "macos", ignore = "requires development FFmpeg")]
    #[test]
    fn concurrent_imports_with_the_same_name_do_not_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("take.poratake");
        std::fs::create_dir(&project).unwrap();
        let source = directory.path().join("source.wav");
        let wav = crate::video::transcription::wav_bytes(&vec![5000; 4800], 48000);
        std::fs::write(&source, &wav).unwrap();
        let barrier = std::sync::Barrier::new(3);
        let (first, second) = std::thread::scope(|scope| {
            let first = scope.spawn(|| {
                barrier.wait();
                add(&project, &source, 0.0).unwrap()
            });
            let second = scope.spawn(|| {
                barrier.wait();
                add(&project, &source, 0.0).unwrap()
            });
            barrier.wait();
            (first.join().unwrap(), second.join().unwrap())
        });
        assert_ne!(first.file_name, second.file_name);
        assert_eq!(
            std::fs::read(project.join("music").join(first.file_name)).unwrap(),
            wav
        );
        assert_eq!(
            std::fs::read(project.join("music").join(second.file_name)).unwrap(),
            wav
        );
    }
}
