//! Shipped sources must not read `std::time::Instant` or `SystemTime`: both
//! panic on `wasm32-unknown-unknown`, where the web gallery runs every crate.
//! `web_time` forwards to `std` natively, which is also why clippy's
//! `disallowed-types` cannot hold this line.

use std::path::{Path, PathBuf};

const ROOTS: &[&str] = &[
    "crates/herogpui-core/src",
    "crates/herogpui-theme/src",
    "crates/herogpui-components/src",
    "crates/herogpui/src",
    "crates/herogpui-web/src",
    "gallery/src",
];

const NATIVE_ONLY: &[(&str, &str)] = &[
    (
        "gallery/src/interaction_sweep/mod.rs",
        "the #[cfg(test)] headless sweep times its own shards",
    ),
    (
        "gallery/src/control.rs",
        "a #[test] names its temp directory after the wall clock",
    ),
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|err| panic!("{}: {err}", dir.display())) {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn names_std_time(source: &str) -> bool {
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    if code.contains("std::time::Instant") || code.contains("std::time::SystemTime") {
        return true;
    }
    let mut rest = code.as_str();
    while let Some(at) = rest.find("std::time::{") {
        let group = &rest[at..];
        let end = group.find('}').unwrap_or(group.len());
        let names = &group[..end];
        if names.contains("Instant") || names.contains("SystemTime") {
            return true;
        }
        rest = &group[end..];
    }
    false
}

#[test]
fn names_std_time_reads_paths_and_use_groups() {
    assert!(names_std_time("use std::time::{Duration, Instant};"));
    assert!(names_std_time(
        "use std::time::{\n    Duration,\n    SystemTime,\n};"
    ));
    assert!(names_std_time("let t = std::time::Instant::now();"));
    assert!(!names_std_time(
        "use std::time::Duration;\nuse web_time::Instant;"
    ));
    assert!(!names_std_time("// std::time::Instant panics on wasm"));
}

#[test]
fn shipped_sources_use_web_time_not_std_time() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in ROOTS {
        rust_files(&root.join(dir), &mut files);
    }
    assert!(
        files.len() > 100,
        "the scan must reach the workspace sources"
    );
    let mut offenders = Vec::new();
    let mut allowed_seen = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(&root)
            .expect("under the workspace root")
            .to_string_lossy()
            .replace('\\', "/");
        let source = std::fs::read_to_string(&file).expect("readable source");
        if !names_std_time(&source) {
            continue;
        }
        if NATIVE_ONLY.iter().any(|(path, _)| *path == relative) {
            allowed_seen.push(relative);
        } else {
            offenders.push(relative);
        }
    }
    assert!(
        offenders.is_empty(),
        "use web_time instead of std::time::{{Instant, SystemTime}} in: {offenders:?}"
    );
    for (path, reason) in NATIVE_ONLY {
        assert!(
            allowed_seen.iter().any(|seen| seen == path),
            "stale NATIVE_ONLY entry {path} ({reason}): it no longer names std::time"
        );
    }
}
