//! Checking for a newer release.
//!
//! `src/main/update/index.ts` drives `electron-updater` against the GitHub
//! releases of `UPDATE_OWNER/UPDATE_REPOSITORY` with `autoDownload = false`, and
//! `about-tab.tsx` renders its state. This shell had neither, so the About page
//! was missing the whole row.
//!
//! What is implemented here is the *check*: ask the releases feed for the latest
//! tag and compare it with this build. Downloading and installing an update are
//! not -- that needs a signed artifact and an installer handoff, and a button
//! that pretended to do it would be worse than no button. `Status::Available`
//! therefore offers the release page rather than an in-app download, which is
//! the one honest action available.

use std::sync::{Arc, Mutex};

/// The states `about-tab.tsx` renders.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Status {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available {
        version: String,
        artifact: Artifact,
        sha512: String,
    },
    Downloading {
        version: String,
        progress: f32,
    },
    Ready {
        version: String,
        installer: std::path::PathBuf,
    },
    Error {
        message: String,
    },
}

impl Status {
    /// `getStatusText` in `about-tab.tsx`.
    pub fn text(&self) -> &'static str {
        match self {
            Self::Checking => "Checking for updates...",
            Self::Available { .. } => "Update available",
            Self::Downloading { .. } => "Downloading update...",
            Self::Ready { .. } => "Update ready to install",
            Self::Error { .. } => "Update check failed",
            Self::UpToDate => "You are up to date",
            Self::Idle => "Check for updates",
        }
    }

    /// `getStatusIcon`. `Checking` and `Downloading` render as a spinner
    /// instead, so they have no entry here.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Available { .. } => "download",
            Self::UpToDate | Self::Ready { .. } => "check-circle",
            Self::Error { .. } => "alert-circle",
            Self::Idle | Self::Checking | Self::Downloading { .. } => "refresh-cw",
        }
    }

    /// `Loader2` spins for both of the in-flight states.
    pub fn spins(&self) -> bool {
        matches!(self, Self::Checking | Self::Downloading { .. })
    }

    /// The `Check` button shows for `idle`, `up_to_date` and `error`.
    pub fn shows_check_button(&self) -> bool {
        matches!(self, Self::Idle | Self::UpToDate | Self::Error { .. })
    }

    /// The version card shows for `available` and `ready`.
    pub fn version(&self) -> Option<&str> {
        match self {
            Self::Available { version, .. }
            | Self::Downloading { version, .. }
            | Self::Ready { version, .. } => Some(version),
            _ => None,
        }
    }
}

/// Shared so the check can run off the UI thread and publish its result.
pub type Shared = Arc<Mutex<Status>>;

const OWNER: &str = "Porabuild";
const REPOSITORY: &str = "Poratake";

fn latest_release_url() -> String {
    format!("https://api.github.com/repos/{OWNER}/{REPOSITORY}/releases/latest")
}

/// `tag_name` with a leading `v` removed, which is how the tags are written.
pub fn version_from_tag(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

/// Compares dotted numeric versions, longest-wins on a prefix tie so `1.2.1`
/// beats `1.2`. A non-numeric component compares as zero rather than making the
/// whole comparison fail.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let parse = |value: &str| -> Vec<u64> {
        value
            .split(['.', '-', '+'])
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parse(candidate), parse(current));
    for index in 0..a.len().max(b.len()) {
        let left = a.get(index).copied().unwrap_or(0);
        let right = b.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

/// Runs the check synchronously. Callers put it on a background thread.
pub fn check(current: &str) -> Status {
    let response = ureq::get(&latest_release_url())
        .header("User-Agent", "Poratake")
        .header("Accept", "application/vnd.github+json")
        .call();

    let mut response = match response {
        Ok(response) => response,
        Err(error) => {
            return Status::Error {
                message: error.to_string(),
            }
        }
    };
    let body = match response.body_mut().read_to_string() {
        Ok(body) => body,
        Err(error) => {
            return Status::Error {
                message: error.to_string(),
            }
        }
    };
    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(error) => {
            return Status::Error {
                message: error.to_string(),
            }
        }
    };
    let Some(tag) = parsed.get("tag_name").and_then(serde_json::Value::as_str) else {
        return Status::Error {
            message: "the latest release has no tag".to_string(),
        };
    };

    let latest = version_from_tag(tag);
    if !is_newer(latest, current) {
        return Status::UpToDate;
    }

    // `autoDownload = false`: the check stops here and the user decides. Both
    // the artifact and the digest published beside it are resolved now, so the
    // download has nothing left to discover.
    let assets = parsed.get("assets").cloned().unwrap_or_default();
    let Some(artifact) = find_installer(&assets, installer_suffix()) else {
        return Status::Error {
            message: format!("release {latest} has no installer for this platform"),
        };
    };
    let Some(manifest_url) = find_manifest(&assets) else {
        return Status::Error {
            message: format!("release {latest} publishes no latest.yml to verify against"),
        };
    };
    let sha512 = match fetch_text(&manifest_url) {
        Ok(manifest) => match sha512_for(&manifest, &artifact.name) {
            Some(digest) => digest,
            None => {
                return Status::Error {
                    message: format!("latest.yml has no checksum for {}", artifact.name),
                }
            }
        },
        Err(error) => return Status::Error { message: error },
    };

    Status::Available {
        version: latest.to_string(),
        artifact,
        sha512,
    }
}

/// The `latest.yml` beside the installer. Without it there is nothing to verify
/// against, and an unverified installer is not something to run.
pub fn find_manifest(assets: &serde_json::Value) -> Option<String> {
    assets.as_array()?.iter().find_map(|asset| {
        (asset.get("name")?.as_str()? == "latest.yml")
            .then(|| {
                asset
                    .get("browser_download_url")?
                    .as_str()
                    .map(str::to_string)
            })
            .flatten()
    })
}

fn fetch_text(url: &str) -> Result<String, String> {
    ureq::get(url)
        .header("User-Agent", "Poratake")
        .call()
        .map_err(|error| error.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_is_a_version_without_its_v() {
        assert_eq!(version_from_tag("v1.2.3"), "1.2.3");
        assert_eq!(version_from_tag("1.2.3"), "1.2.3");
    }

    #[test]
    fn versions_compare_component_by_component() {
        assert!(is_newer("0.9.6", "0.9.5"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.9.5", "0.9.5"));
        assert!(!is_newer("0.9.4", "0.9.5"));
        // A shorter version is not newer than a longer one that extends it.
        assert!(!is_newer("1.2", "1.2.1"));
        assert!(is_newer("1.2.1", "1.2"));
        // Junk compares as zero rather than panicking or reading as newest.
        assert!(!is_newer("nightly", "0.9.5"));
    }

    /// The strings and the button rule come straight from `about-tab.tsx`, so a
    /// change there should fail here rather than drift silently.
    #[test]
    fn the_states_match_the_reference() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repository root")
            .to_path_buf();
        let about =
            std::fs::read_to_string(root.join("src/renderer/components/settings/about-tab.tsx"))
                .expect("read about-tab.tsx");

        for status in [
            Status::Idle,
            Status::Checking,
            Status::UpToDate,
            Status::Available {
                version: "1.0.0".into(),
                artifact: Artifact {
                    name: "Poratake-1.0.0-win-x64.exe".into(),
                    url: "https://x".into(),
                    size: 1,
                },
                sha512: "AAA".into(),
            },
            Status::Downloading {
                version: "1.0.0".into(),
                progress: 0.5,
            },
            Status::Ready {
                version: "1.0.0".into(),
                installer: std::path::PathBuf::from("installer.exe"),
            },
            Status::Error {
                message: "boom".into(),
            },
        ] {
            assert!(
                about.contains(status.text()),
                "`{}` is not a string about-tab.tsx renders",
                status.text()
            );
        }

        assert!(Status::Idle.shows_check_button());
        assert!(Status::UpToDate.shows_check_button());
        assert!(Status::Error {
            message: String::new()
        }
        .shows_check_button());
        assert!(!Status::Checking.shows_check_button());
        assert!(
            !Status::Available {
                version: "1.0.0".into(),
                artifact: Artifact {
                    name: "Poratake-1.0.0-win-x64.exe".into(),
                    url: "https://x".into(),
                    size: 1,
                },
                sha512: "AAA".into(),
            }
            .shows_check_button(),
            "the reference hides Check while an update is pending"
        );
    }

    /// The owner and repository have to stay in step with `src/types/product.ts`,
    /// which is what `electron-updater` is pointed at.
    #[test]
    fn the_feed_matches_the_one_electron_updates_from() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repository root")
            .to_path_buf();
        let product =
            std::fs::read_to_string(root.join("src/types/product.ts")).expect("read product.ts");
        assert!(product.contains(&format!("UPDATE_OWNER = '{OWNER}'")));
        assert!(product.contains(&format!("UPDATE_REPOSITORY = '{REPOSITORY}'")));
    }
}

/// The release asset for this platform, and the digest `latest.yml` publishes
/// for it. Both come from the same release, which is what makes the check
/// meaningful: a tampered artifact fails against the manifest beside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact {
    pub name: String,
    pub url: String,
    pub size: u64,
}

/// `artifactName: "${productName}-${version}-win-${arch}.${ext}"` with
/// `target: nsis`, so the installer is the `.exe` for the running architecture.
pub fn installer_suffix() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "-win-arm64.exe"
    } else {
        "-win-x64.exe"
    }
}

/// Picks the installer out of a release's assets. `None` when the release has
/// no artifact for this platform, which is a real case for a partial publish.
pub fn find_installer(assets: &serde_json::Value, suffix: &str) -> Option<Artifact> {
    assets.as_array()?.iter().find_map(|asset| {
        let name = asset.get("name")?.as_str()?;
        if !name.ends_with(suffix) {
            return None;
        }
        Some(Artifact {
            name: name.to_string(),
            url: asset.get("browser_download_url")?.as_str()?.to_string(),
            size: asset
                .get("size")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
        })
    })
}

/// `latest.yml` is electron-builder's manifest: a `files:` list of `- url:` /
/// `sha512:` pairs, plus a top-level `path`/`sha512` for the primary artifact.
/// This reads the digest belonging to `name` without pulling in a YAML parser
/// for four lines of it.
pub fn sha512_for(manifest: &str, name: &str) -> Option<String> {
    let mut current: Option<&str> = None;
    for line in manifest.lines() {
        let trimmed = line.trim_start();
        if let Some(url) = trimmed
            .strip_prefix("- url:")
            .or_else(|| trimmed.strip_prefix("url:"))
            .or_else(|| trimmed.strip_prefix("path:"))
        {
            current = Some(url.trim());
            continue;
        }
        if let Some(digest) = trimmed.strip_prefix("sha512:") {
            if current == Some(name) {
                return Some(digest.trim().to_string());
            }
        }
    }
    None
}

/// Constant-time-ish comparison is not the point here -- this is an integrity
/// check, not a secret -- but trimming and case are, because the manifest and
/// the computed digest are both base64 and must match exactly.
pub fn digest_matches(expected: &str, actual: &str) -> bool {
    expected.trim() == actual.trim()
}

/// The base64 sha512 of `bytes`, in the form `latest.yml` publishes.
#[cfg(test)]
pub fn digest(bytes: &[u8]) -> String {
    use base64::Engine;
    use sha2::Digest;
    let mut hasher = sha2::Sha512::new();
    hasher.update(bytes);
    base64::engine::general_purpose::STANDARD.encode(hasher.finalize())
}

/// Downloads the installer, verifying it against the manifest as it goes, and
/// returns the path it was written to. Refuses to keep a file whose digest does
/// not match -- the whole point of the check is that an unverified binary is
/// never left somewhere it could be run.
pub fn download(
    artifact: &Artifact,
    expected_sha512: &str,
    on_progress: impl Fn(f32),
) -> Result<std::path::PathBuf, String> {
    use std::io::{Read, Write};

    use base64::Engine;
    use sha2::Digest;

    let mut response = ureq::get(&artifact.url)
        .header("User-Agent", "Poratake")
        .call()
        .map_err(|error| error.to_string())?;

    let total = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(artifact.size);

    let mut body = response.body_mut().as_reader();
    let path = installer_path(&artifact.name)?;
    let partial_path = path.with_extension(format!(
        "part-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    ));
    let mut file = std::fs::File::create(&partial_path).map_err(|error| error.to_string())?;
    let mut hasher = sha2::Sha512::new();
    let mut downloaded = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    let result = loop {
        let read = match body.read(&mut buffer) {
            Ok(read) => read,
            Err(error) => break Err(error.to_string()),
        };
        if read == 0 {
            break Ok(());
        }
        if let Err(error) = file.write_all(&buffer[..read]) {
            break Err(error.to_string());
        }
        hasher.update(&buffer[..read]);
        downloaded += read as u64;
        if total > 0 {
            on_progress((downloaded as f32 / total as f32).clamp(0.0, 1.0));
        }
    };
    if let Err(error) = result {
        drop(file);
        let _ = std::fs::remove_file(&partial_path);
        return Err(error);
    }
    if let Err(error) = file.sync_all() {
        drop(file);
        let _ = std::fs::remove_file(&partial_path);
        return Err(error.to_string());
    }
    drop(file);

    let actual = base64::engine::general_purpose::STANDARD.encode(hasher.finalize());
    if !digest_matches(expected_sha512, &actual) {
        let _ = std::fs::remove_file(&partial_path);
        return Err("the downloaded installer does not match the published checksum".to_string());
    }

    if path.exists() {
        std::fs::remove_file(&path).map_err(|error| {
            let _ = std::fs::remove_file(&partial_path);
            error.to_string()
        })?;
    }
    std::fs::rename(&partial_path, &path).map_err(|error| {
        let _ = std::fs::remove_file(&partial_path);
        error.to_string()
    })?;

    Ok(path)
}

fn installer_path(name: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::Path::new(name);
    if name.contains(['/', '\\', ':']) || path.file_name() != Some(std::ffi::OsStr::new(name)) {
        return Err("the release asset has an invalid file name".to_string());
    }
    Ok(std::env::temp_dir().join(path))
}

/// `quitAndInstall`: hand over to the installer and leave. The caller quits
/// afterwards, because NSIS cannot replace a running binary.
pub fn launch_installer(path: &std::path::Path) -> Result<(), String> {
    std::process::Command::new(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod artifact_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_installer_is_the_exe_for_this_architecture() {
        let assets = json!([
            { "name": "Poratake-0.9.6-win-arm64.exe", "browser_download_url": "https://x/arm", "size": 1 },
            { "name": "Poratake-0.9.6-win-x64.exe", "browser_download_url": "https://x/x64", "size": 2 },
            { "name": "latest.yml", "browser_download_url": "https://x/yml", "size": 3 },
        ]);
        let found = find_installer(&assets, "-win-x64.exe").expect("the x64 installer");
        assert_eq!(found.name, "Poratake-0.9.6-win-x64.exe");
        assert_eq!(found.url, "https://x/x64");
        assert_eq!(found.size, 2);

        // A release with no artifact for this platform is a real case.
        assert!(find_installer(&assets, "-win-riscv.exe").is_none());
        assert!(find_installer(&json!([]), "-win-x64.exe").is_none());
    }

    #[test]
    fn installer_downloads_stay_in_the_temp_directory() {
        assert!(installer_path("Poratake-0.9.6-win-x64.exe").is_ok());
        assert!(installer_path("../Poratake.exe").is_err());
        assert!(installer_path("C:\\Poratake.exe").is_err());
    }

    #[test]
    fn the_suffix_follows_the_build_configuration() {
        // `artifactName` in electron-builder.json5 ends `-win-${arch}.${ext}`.
        assert!(installer_suffix().starts_with("-win-"));
        assert!(installer_suffix().ends_with(".exe"));
    }

    #[test]
    fn the_digest_comes_from_the_manifest_entry_for_that_file() {
        let manifest = "version: 0.9.6\n\
                        files:\n\
                        \x20 - url: Poratake-0.9.6-win-x64.exe\n\
                        \x20   sha512: AAAAx64\n\
                        \x20   size: 90000000\n\
                        \x20 - url: Poratake-0.9.6-win-arm64.exe\n\
                        \x20   sha512: BBBBarm\n\
                        path: Poratake-0.9.6-win-x64.exe\n\
                        sha512: AAAAx64\n";
        assert_eq!(
            sha512_for(manifest, "Poratake-0.9.6-win-x64.exe").as_deref(),
            Some("AAAAx64")
        );
        assert_eq!(
            sha512_for(manifest, "Poratake-0.9.6-win-arm64.exe").as_deref(),
            Some("BBBBarm")
        );
        assert_eq!(sha512_for(manifest, "nothing.exe"), None);
    }

    #[test]
    fn a_digest_is_the_base64_sha512_electron_builder_publishes() {
        // Known value: base64(sha512("")) .
        assert_eq!(
            digest(b""),
            "z4PhNX7vuL3xVChQ1m2AB9Yg5AULVxXcg/SpIdNs6c5H0NE8XYXysP+DGNKHfuwvY7kxvUdBeoGlODJ6+SfaPg=="
        );
        assert!(digest_matches(" abc ", "abc"));
        assert!(!digest_matches("abc", "abd"));
    }
}
