use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn unique_temp(stem: &str) -> PathBuf {
    let nth = COUNTER.fetch_add(1, Ordering::Relaxed);
    let (name, extension) = match stem.split_once('.') {
        Some((name, extension)) => (name, format!(".{extension}")),
        None => (stem, String::new()),
    };
    std::env::temp_dir().join(format!("{name}-{}-{nth}{extension}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_calls_never_collide() {
        assert_ne!(unique_temp("poratake-x.png"), unique_temp("poratake-x.png"));
    }

    #[test]
    fn the_extension_survives_the_suffix() {
        let path = unique_temp("poratake-x.png");
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("png"));
        assert!(path
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|stem| stem.starts_with("poratake-x-")));
    }

    #[test]
    fn a_stemless_name_still_gets_a_suffix() {
        let path = unique_temp("poratake-folder");
        assert!(path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|name| name.starts_with("poratake-folder-")));
        assert_eq!(path.extension(), None);
    }
}
