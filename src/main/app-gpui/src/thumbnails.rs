use std::path::{Path, PathBuf};

use md5::{Digest, Md5};

use crate::config::store::config_dir;
use crate::history_store::HistoryItemType;

const THUMBNAIL_WIDTH: u32 = 300;
const THUMBNAIL_QUALITY: u8 = 80;

pub fn thumbnails_dir() -> PathBuf {
    config_dir().join("thumbnails")
}

pub fn thumbnail_path(original: &Path) -> PathBuf {
    let mut hasher = Md5::new();
    hasher.update(original.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    thumbnails_dir().join(format!("{hex}.jpg"))
}

pub fn cached(original: &Path) -> Option<PathBuf> {
    let path = thumbnail_path(original);
    path.is_file().then_some(path)
}

pub fn ensure(original: &Path, kind: HistoryItemType) -> Option<PathBuf> {
    if let Some(path) = cached(original) {
        return Some(path);
    }
    if kind == HistoryItemType::Video || !original.is_file() {
        return None;
    }
    generate(original)
}

fn generate(original: &Path) -> Option<PathBuf> {
    let target = thumbnail_path(original);
    let parent = target.parent()?;
    if let Err(error) = std::fs::create_dir_all(parent) {
        eprintln!(
            "[thumbnails] failed to create {}: {error}",
            parent.display()
        );
        return None;
    }

    let decoded = match image::open(original) {
        Ok(decoded) => decoded,
        Err(error) => {
            eprintln!(
                "[thumbnails] failed to read {}: {error}",
                original.display()
            );
            return None;
        }
    };
    let resized = decoded
        .thumbnail(THUMBNAIL_WIDTH, THUMBNAIL_WIDTH)
        .to_rgb8();

    let file = match std::fs::File::create(&target) {
        Ok(file) => file,
        Err(error) => {
            eprintln!(
                "[thumbnails] failed to create {}: {error}",
                target.display()
            );
            return None;
        }
    };
    let mut writer = std::io::BufWriter::new(file);
    let mut encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, THUMBNAIL_QUALITY);
    if let Err(error) = encoder.encode_image(&resized) {
        eprintln!(
            "[thumbnails] failed to encode {}: {error}",
            target.display()
        );
        let _ = std::fs::remove_file(&target);
        return None;
    }
    Some(target)
}

pub fn remove(original: &Path) {
    let path = thumbnail_path(original);
    if path.is_file() {
        let _ = std::fs::remove_file(path);
    }
}

pub fn rekey(old: &Path, new: &Path) {
    let old_thumbnail = thumbnail_path(old);
    if !old_thumbnail.is_file() {
        return;
    }
    let new_thumbnail = thumbnail_path(new);
    if let Some(parent) = new_thumbnail.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::rename(old_thumbnail, new_thumbnail);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_thumbnails_by_md5_of_the_source_path() {
        let path = thumbnail_path(Path::new("/tmp/example.png"));
        let name = path.file_name().and_then(|value| value.to_str()).unwrap();
        assert_eq!(name.len(), 36);
        assert!(name.ends_with(".jpg"));
        assert_eq!(path, thumbnail_path(Path::new("/tmp/example.png")));
        assert_ne!(path, thumbnail_path(Path::new("/tmp/other.png")));
    }
}
