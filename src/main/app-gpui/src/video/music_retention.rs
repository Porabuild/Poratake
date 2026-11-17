use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::video::{music, project};
use crate::windows::video_editor::model::MusicTrack;

const FOLDER: &str = ".poratake-music-retention";

fn marker_name(file_name: &str) -> String {
    let hash = Sha256::digest(file_name.as_bytes());
    let mut name = String::with_capacity(69);
    for byte in hash {
        use std::fmt::Write as _;
        write!(name, "{byte:02x}").expect("writing to a string cannot fail");
    }
    name.push_str(".json");
    name
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    file_name: String,
}

pub fn reserved(folder: &Path, file_name: &str) -> io::Result<bool> {
    let parent = folder
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "music folder has no parent"))?;
    let records = parent.join(FOLDER);
    match fs::symlink_metadata(&records) {
        Ok(metadata) if !metadata.file_type().is_dir() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid retention folder",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    }
    match fs::symlink_metadata(records.join(marker_name(file_name))) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub fn record(project_path: &Path, file_name: &str) -> io::Result<()> {
    let folder = project::music_folder(project_path)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a recording project"))?;
    let source = music::file_path(&folder, file_name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid music path"))?;
    if !fs::symlink_metadata(source)?.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a music file",
        ));
    }
    let parent = folder
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "music folder has no parent"))?;
    let records = parent.join(FOLDER);
    match fs::create_dir(&records) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    if !fs::symlink_metadata(&records)?.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid retention folder",
        ));
    }
    let path = records.join(marker_name(file_name));
    let contents = serde_json::to_vec(&Candidate {
        file_name: file_name.to_string(),
    })?;
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(error),
    };
    let result = file.write_all(&contents).and_then(|_| file.sync_all());
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}

pub fn record_removed(project_path: &Path, before: &[MusicTrack], after: &[MusicTrack]) {
    let retained: HashSet<&str> = after
        .iter()
        .filter(|track| track.source == "music")
        .map(|track| track.file_name.as_str())
        .collect();
    let mut recorded = HashSet::new();
    for track in before.iter().filter(|track| track.source == "music") {
        if retained.contains(track.file_name.as_str()) || !recorded.insert(&track.file_name) {
            continue;
        }
        if let Err(error) = record(project_path, &track.file_name) {
            eprintln!("[video-editor] could not record retained music: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_only_files_no_longer_referenced() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        let folder = project.join("music");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("shared.wav"), b"shared").unwrap();
        fs::write(folder.join("removed.wav"), b"removed").unwrap();
        let shared = MusicTrack {
            file_name: "shared.wav".into(),
            ..Default::default()
        };
        let removed = MusicTrack {
            file_name: "removed.wav".into(),
            ..Default::default()
        };
        record_removed(&project, &[shared.clone(), removed], &[shared]);
        let records = fs::read_dir(project.join(FOLDER))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].file_name(),
            "d8fd9d421e70915368b8ec0f69334c92576e067aa4336e4e9537feeaa55862c0.json"
        );
        let candidate: Candidate =
            serde_json::from_slice(&fs::read(records[0].path()).unwrap()).unwrap();
        assert_eq!(candidate.file_name, "removed.wav");
        assert_eq!(fs::read(folder.join("removed.wav")).unwrap(), b"removed");
    }

    #[test]
    fn rejects_paths_outside_the_music_folder() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        fs::create_dir_all(project.join("music")).unwrap();
        fs::write(dir.path().join("outside.wav"), b"outside").unwrap();
        assert!(record(&project, "../../outside.wav").is_err());
        assert!(!project.join(FOLDER).exists());
        assert_eq!(
            fs::read(dir.path().join("outside.wav")).unwrap(),
            b"outside"
        );
    }

    #[test]
    fn an_existing_record_reserves_the_name_even_if_the_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        let folder = project.join("music");
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join("song.wav");
        fs::write(&file, b"song").unwrap();
        record(&project, "song.wav").unwrap();
        fs::remove_file(file).unwrap();
        assert_eq!(
            music::unique_file_name(&folder, "song.wav").unwrap(),
            "song (1).wav"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_retention_and_music_folders() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        let outside = dir.path().join("outside");
        fs::create_dir_all(project.join("music")).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(project.join("music/song.wav"), b"song").unwrap();
        std::os::unix::fs::symlink(&outside, project.join(FOLDER)).unwrap();
        assert!(record(&project, "song.wav").is_err());
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        fs::remove_file(project.join(FOLDER)).unwrap();
        fs::remove_dir_all(project.join("music")).unwrap();
        std::os::unix::fs::symlink(&outside, project.join("music")).unwrap();
        fs::write(outside.join("song.wav"), b"song").unwrap();
        assert!(record(&project, "song.wav").is_err());
        assert!(!project.join(FOLDER).exists());
    }
}
