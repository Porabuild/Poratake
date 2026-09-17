//! Checking for a newer release.
//!
//! `src/main/update/index.ts` drives `electron-updater` against the GitHub
//! releases of `UPDATE_OWNER/UPDATE_REPOSITORY` with `autoDownload = false`, then
//! calls `downloadUpdate()` from the `update-available` handler. About renders
//! the same states. This shell checks the releases feed, starts the verified
//! download as soon as a newer installer exists, and hands off to that installer
//! from Ready.

use std::sync::{Arc, Mutex};

use crate::system::tray::UpdateStatus;

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
        notes: Option<String>,
    },
    Downloading {
        version: String,
        progress: f32,
        notes: Option<String>,
    },
    Ready {
        version: String,
        installer: std::path::PathBuf,
        notes: Option<String>,
    },
    Error {
        message: String,
    },
    /// `status: 'unsupported'` — Linux, where there is no installer to fetch.
    Unsupported,
}

impl Status {
    /// The status a fresh window starts from: Electron sets `unsupported` at
    /// init on platforms without an updater, and stays `idle` elsewhere.
    pub fn initial() -> Self {
        if cfg!(target_os = "linux") {
            Self::Unsupported
        } else {
            Self::Idle
        }
    }

    /// `getStatusText` in `about-tab.tsx`.
    pub fn text(&self) -> &'static str {
        match self {
            Self::Checking => "Checking for updates...",
            Self::Available { .. } => "Update available",
            Self::Downloading { .. } => "Downloading update...",
            Self::Ready { .. } => "Update ready to install",
            Self::Error { .. } => "Update check failed",
            Self::UpToDate => "You are up to date",
            Self::Unsupported => "Automatic updates are not available on this platform",
            Self::Idle => "Check for updates",
        }
    }

    /// `getStatusIcon`. `Checking` and `Downloading` render as a spinner
    /// instead, so they have no entry here.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Available { .. } => "download",
            Self::UpToDate | Self::Ready { .. } => "check-circle",
            Self::Error { .. } | Self::Unsupported => "alert-circle",
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

    /// The "What's New" text Electron renders for `available` and `ready`.
    pub fn notes(&self) -> Option<&str> {
        match self {
            Self::Available { notes, .. } | Self::Ready { notes, .. } => notes.as_deref(),
            _ => None,
        }
    }
}

/// Shared so the check can run off the UI thread and publish its result.
pub type Shared = Arc<Mutex<UpdateCell>>;

/// The shared updater cell: the latest status plus the tray row it was last
/// published as, so the menu rebuilds only when the row actually changes.
#[derive(Clone, Debug)]
pub struct UpdateCell {
    status: Status,
    published: UpdateStatus,
}

impl Default for UpdateCell {
    fn default() -> Self {
        Self {
            status: Status::initial(),
            published: UpdateStatus::Idle,
        }
    }
}

impl UpdateCell {
    pub fn status(&self) -> Status {
        self.status.clone()
    }

    pub fn publish(&mut self, status: Status) {
        self.status = status;
    }

    /// The tray row for the current status when it differs from the last
    /// published one, marking it published. Mirrors Electron's rebuild rule:
    /// available/ready transitions rebuild, download progress rebuilds on 10%
    /// bucket crossings (the first 10% still shows the available row), and an
    /// error never touches the tray — the stale row keeps the update
    /// discoverable and opens About, which shows the failure.
    pub fn take_tray_refresh(&mut self) -> Option<UpdateStatus> {
        if matches!(self.status, Status::Error { .. }) {
            return None;
        }
        let mapped = UpdateStatus::from_status(&self.status);
        let same = match (&self.published, &mapped) {
            (UpdateStatus::Downloading(previous), UpdateStatus::Downloading(next)) => {
                previous / 10 == next / 10
            }
            (UpdateStatus::Available(_), UpdateStatus::Downloading(percent)) => *percent < 10,
            (previous, next) => previous == next,
        };
        if same {
            return None;
        }
        self.published = mapped.clone();
        Some(mapped)
    }
}

/// The interval skips while a download is running or an installer is ready —
/// `startPeriodicUpdateChecks` — plus while a check is already in flight, so a
/// manual click and the timer never fetch the feed twice.
pub fn should_auto_check(status: &Status) -> bool {
    !matches!(
        status,
        Status::Downloading { .. } | Status::Ready { .. } | Status::Checking
    )
}

/// The status main-thread callers render or map — `initial` without a cell,
/// which only happens in headless tests that never installed one.
pub fn current_status(cx: &gpui::App) -> Status {
    cx.try_global::<crate::state::UpdateState>()
        .and_then(|state| state.0.lock().ok())
        .map(|cell| cell.status())
        .unwrap_or_else(Status::initial)
}

/// Rebuilds the tray menu when the update row changed — the `rebuildTrayMenu`
/// half of Electron's `setStatus`. Silent without a cell or bridge (headless
/// tests).
pub fn sync_tray_status(cx: &mut gpui::App) {
    let status = {
        let Some(state) = cx.try_global::<crate::state::UpdateState>() else {
            return;
        };
        let Ok(mut cell) = state.0.lock() else {
            return;
        };
        if cell.take_tray_refresh().is_none() {
            return;
        }
        cell.status()
    };
    let (Some(service), Some(bridge)) = (crate::state::try_state(cx), crate::state::try_native(cx))
    else {
        return;
    };
    let config = service.config.get();
    bridge.send(crate::system::native::NativeCommand::RebuildMenu(
        crate::system::tray::TrayMenuState::from_config(&config, &status).into(),
    ));
}

const INITIAL_CHECK_DELAY_SECS: u64 = 3;
const CHECK_INTERVAL_SECS: u64 = 30 * 60;

/// `init()` in `main/update/index.ts`: one check shortly after launch, then
/// every 30 minutes. Results publish into the shared cell; the tray follows
/// through `sync_tray_status` and an open About page is repainted.
pub fn spawn_auto_check(cx: &mut gpui::App) {
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(std::time::Duration::from_secs(INITIAL_CHECK_DELAY_SECS))
            .await;
        auto_check_once(cx).await;
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(CHECK_INTERVAL_SECS))
                .await;
            auto_check_once(cx).await;
        }
    })
    .detach();
}

async fn auto_check_once(cx: &mut gpui::AsyncApp) {
    let should: bool = cx.update(|cx| {
        let Some(state) = cx.try_global::<crate::state::UpdateState>() else {
            return false;
        };
        let Ok(mut cell) = state.0.lock() else {
            return false;
        };
        if !should_auto_check(&cell.status()) {
            return false;
        }
        cell.publish(Status::Checking);
        true
    });
    if !should {
        return;
    }
    let result = cx
        .background_executor()
        .spawn(async move { check(crate::product::VERSION) })
        .await;
    cx.update(|cx| {
        if let Some(state) = cx.try_global::<crate::state::UpdateState>() {
            if let Ok(mut cell) = state.0.lock() {
                cell.publish(result);
            }
        }
        sync_tray_status(cx);
        notify_about_status_changed(cx);
        start_download(cx);
    });
}

/// Moves `Available` into `Downloading` at 0% and returns the artifact to
/// fetch. `None` when nothing is waiting or a download is already running.
pub fn begin_download(cell: &mut UpdateCell) -> Option<(String, Artifact, String, Option<String>)> {
    let Status::Available {
        version,
        artifact,
        sha512,
        notes,
    } = cell.status()
    else {
        return None;
    };
    cell.publish(Status::Downloading {
        version: version.clone(),
        progress: 0.0,
        notes: notes.clone(),
    });
    Some((version, artifact, sha512, notes))
}

/// `autoUpdater.downloadUpdate()` after `update-available`. No-op unless the
/// cell is `Available`.
pub fn start_download(cx: &mut gpui::App) {
    let Some(state) = cx.try_global::<crate::state::UpdateState>() else {
        return;
    };
    let shared = state.0.clone();
    let Some((version, artifact, sha512, notes)) = shared
        .lock()
        .ok()
        .and_then(|mut cell| begin_download(&mut cell))
    else {
        return;
    };
    sync_tray_status(cx);
    notify_about_status_changed(cx);
    spawn_download_progress_poll(cx);

    let progress_cell = shared.clone();
    let result_cell = shared;
    cx.spawn(async move |cx| {
        let version_for_progress = version.clone();
        let notes_for_progress = notes.clone();
        let result = cx
            .background_executor()
            .spawn(async move {
                download(&artifact, &sha512, move |fraction| {
                    if let Ok(mut cell) = progress_cell.lock() {
                        cell.publish(Status::Downloading {
                            version: version_for_progress.clone(),
                            progress: fraction,
                            notes: notes_for_progress.clone(),
                        });
                    }
                })
            })
            .await;
        cx.update(|cx| {
            if let Ok(mut cell) = result_cell.lock() {
                cell.publish(match result {
                    Ok(installer) => Status::Ready {
                        version,
                        installer,
                        notes,
                    },
                    Err(message) => Status::Error { message },
                });
            }
            sync_tray_status(cx);
            notify_about_status_changed(cx);
        });
    })
    .detach();
}

/// The `update:download-progress` broadcast: while a download runs, About and
/// the tray row repaint at 10Hz from the shared cell.
fn spawn_download_progress_poll(cx: &mut gpui::App) {
    cx.spawn(async move |cx| loop {
        cx.background_executor()
            .timer(std::time::Duration::from_millis(100))
            .await;
        let downloading = cx.update(|cx| {
            let downloading = matches!(current_status(cx), Status::Downloading { .. });
            if downloading {
                sync_tray_status(cx);
                notify_about_status_changed(cx);
            }
            downloading
        });
        if !downloading {
            break;
        }
    })
    .detach();
}

/// The `update:status-changed` broadcast for the one window that renders the
/// status: repaint About when it is open.
fn notify_about_status_changed(cx: &mut gpui::App) {
    use crate::windows::registry::{self, WindowKind};
    use crate::windows::settings::SettingsWindow;

    if let Some(handle) = registry::handle(WindowKind::Settings, cx) {
        if let Some(settings) = handle.downcast::<SettingsWindow>() {
            let _ = settings.update(cx, |_, _, cx| cx.notify());
        }
    }
}

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
/// Linux has no installer to fetch, so it reports `unsupported` without
/// touching the network — the `!isSupportedPlatform` guard in `checkForUpdate`.
pub fn check(current: &str) -> Status {
    if cfg!(target_os = "linux") {
        return Status::Unsupported;
    }
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

    // `autoDownload = false` on electron-updater: the check still resolves the
    // artifact and digest here so `start_download` has nothing left to discover.
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
        notes: parsed
            .get("body")
            .and_then(serde_json::Value::as_str)
            .map(release_notes_text)
            .filter(|notes| !notes.is_empty()),
    }
}

/// `releaseNotesToText` in `main/update/release-notes.ts`: the GitHub release
/// body is markdown with embedded HTML, and the About card shows it as plain
/// text. Block tags become line breaks, everything else is stripped, entities
/// are decoded, and runs of blank lines collapse to one.
pub fn release_notes_text(notes: &str) -> String {
    let decoded = decode_entities(&strip_html(notes));
    let collapsed: Vec<String> = decoded
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    squash_blank_lines(&collapsed.join("\n")).trim().to_string()
}

fn starts_with_name(inner: &str, name: &str) -> bool {
    inner
        .get(..name.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(name))
}

/// The tag-stripping half: `<br>` becomes a break, `<li>` a bullet, block
/// closers a break, `script`/`style` blocks vanish with their content, and any
/// other `<...>` span is removed. Electron's patterns are prefix matches
/// (`<br[^>]*>` also eats `<breakfast>`), so this is too.
fn strip_html(notes: &str) -> String {
    const BLOCKS: [&str; 15] = [
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "p",
        "div",
        "ul",
        "ol",
        "li",
        "blockquote",
        "pre",
        "table",
        "tr",
    ];
    let mut out = String::with_capacity(notes.len());
    let mut rest = notes;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let tag = &rest[open..];
        let Some(close) = tag.find('>') else {
            out.push_str(tag);
            rest = "";
            break;
        };
        let inner = &tag[1..close];
        rest = &tag[close + 1..];
        let (closing, after_slash) = match inner.strip_prefix('/') {
            Some(after) => (true, after),
            None => (false, inner),
        };
        if closing {
            if BLOCKS
                .iter()
                .any(|block| starts_with_name(after_slash, block))
            {
                out.push('\n');
            }
            continue;
        }
        if after_slash.is_empty() || !after_slash.as_bytes()[0].is_ascii_alphanumeric() {
            continue;
        }
        if starts_with_name(after_slash, "script") || starts_with_name(after_slash, "style") {
            let opener = if starts_with_name(after_slash, "script") {
                "script"
            } else {
                "style"
            };
            if let Some(end) = find_closing_tag(rest, opener) {
                rest = end;
            }
            continue;
        }
        if starts_with_name(after_slash, "br") {
            out.push('\n');
        } else if starts_with_name(after_slash, "li") {
            out.push_str("\n- ");
        }
    }
    out.push_str(rest);
    out
}

/// Finds `</name ...>` for a `script`/`style` opener, case-insensitively, and
/// returns what follows it. `None` leaves the content in place — without its
/// closer the opener is just another stripped tag.
fn find_closing_tag<'a>(rest: &'a str, name: &str) -> Option<&'a str> {
    let mut search = rest;
    loop {
        let open = search.find("</")?;
        let mut tail = &search[open + "</".len()..];
        tail = tail.trim_start();
        if starts_with_name(tail, name) {
            let close = tail.find('>')?;
            return Some(&tail[close + 1..]);
        }
        search = &search[open + "</".len()..];
    }
}

const NAMED_ENTITIES: [(&str, char); 10] = [
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("nbsp", ' '),
    ("mdash", '\u{2014}'),
    ("ndash", '\u{2013}'),
    ("hellip", '\u{2026}'),
    ("copy", '\u{00a9}'),
];

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        let Some(semi) = tail.find(';') else {
            out.push_str(tail);
            rest = "";
            break;
        };
        match decode_entity_body(&tail[1..semi]) {
            Some(decoded) => {
                out.push(decoded);
                rest = &tail[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn decode_entity_body(body: &str) -> Option<char> {
    if let Some(hex) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
        if hex.is_empty() || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        return code_point_to_char(u32::from_str_radix(hex, 16).ok()?);
    }
    if let Some(dec) = body.strip_prefix('#') {
        if dec.is_empty() || !dec.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        return code_point_to_char(dec.parse::<u32>().ok()?);
    }
    if body.is_empty() || !body.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return None;
    }
    NAMED_ENTITIES
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(body))
        .map(|(_, decoded)| *decoded)
}

fn code_point_to_char(code: u32) -> Option<char> {
    if code == 0 {
        return None;
    }
    char::from_u32(code)
}

/// `/\n{3,}/g` becomes a single blank line.
fn squash_blank_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut newlines = 0;
    for ch in text.chars() {
        if ch == '\n' {
            newlines += 1;
            if newlines <= 2 {
                out.push(ch);
            }
        } else {
            newlines = 0;
            out.push(ch);
        }
    }
    out
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
                notes: None,
            },
            Status::Downloading {
                version: "1.0.0".into(),
                progress: 0.5,
                notes: None,
            },
            Status::Ready {
                version: "1.0.0".into(),
                installer: std::path::PathBuf::from("installer.exe"),
                notes: None,
            },
            Status::Error {
                message: "boom".into(),
            },
            Status::Unsupported,
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
        assert!(!Status::Unsupported.shows_check_button());
        assert!(
            !Status::Available {
                version: "1.0.0".into(),
                artifact: Artifact {
                    name: "Poratake-1.0.0-win-x64.exe".into(),
                    url: "https://x".into(),
                    size: 1,
                },
                sha512: "AAA".into(),
                notes: None,
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

    #[test]
    fn release_notes_become_plain_text_like_the_reference() {
        assert_eq!(release_notes_text("a<br>b<BR/>c<br >d"), "a\nb\nc\nd");
        assert_eq!(
            release_notes_text("<ul><li>one</li><li>two</li></ul>"),
            "- one\n\n- two"
        );
        assert_eq!(
            release_notes_text("<h2>Title</h2><p>Body</p>"),
            "Title\nBody"
        );
        assert_eq!(
            release_notes_text("keep<script>var x = '</p>';</script>more"),
            "keepmore"
        );
        assert_eq!(
            release_notes_text("<style type=\"text/css\">.a{}</style>ok"),
            "ok"
        );
        assert_eq!(release_notes_text("a<script>oops"), "aoops");
        assert_eq!(release_notes_text("a</ p>b"), "ab");
        assert_eq!(
            release_notes_text("a &amp; b &LT;tag&gt; &#65;&#x42; &nbsp;x"),
            "a & b <tag> AB x"
        );
        assert_eq!(
            release_notes_text("&bogus; &amp &#0; &#xD800; &#x110000;"),
            "&bogus; &amp &#0; &#xD800; &#x110000;"
        );
        assert_eq!(release_notes_text("<a href=\"x>y\">t</a>"), "y\">t");
        assert_eq!(release_notes_text("a<brr>b"), "a\nb");
        assert_eq!(release_notes_text("a\n\n\n\nb"), "a\n\nb");
        assert_eq!(release_notes_text("5 < 10"), "5 < 10");
        assert_eq!(release_notes_text("5 < 10 > 3"), "5 3");
    }

    #[test]
    fn the_tray_rebuilds_on_the_electron_rule() {
        let available = |version: &str| Status::Available {
            version: version.into(),
            artifact: Artifact {
                name: "a".into(),
                url: "u".into(),
                size: 0,
            },
            sha512: "s".into(),
            notes: None,
        };
        let downloading = |progress: f32| Status::Downloading {
            version: "1.0".into(),
            progress,
            notes: None,
        };

        let mut cell = UpdateCell::default();
        assert_eq!(cell.take_tray_refresh(), None);
        cell.publish(available("1.0"));
        assert_eq!(
            cell.take_tray_refresh(),
            Some(UpdateStatus::Available("1.0".into()))
        );
        assert_eq!(cell.take_tray_refresh(), None);
        cell.publish(downloading(0.05));
        assert_eq!(cell.take_tray_refresh(), None);
        cell.publish(downloading(0.12));
        assert_eq!(
            cell.take_tray_refresh(),
            Some(UpdateStatus::Downloading(12))
        );
        cell.publish(downloading(0.19));
        assert_eq!(cell.take_tray_refresh(), None);
        cell.publish(downloading(0.21));
        assert_eq!(
            cell.take_tray_refresh(),
            Some(UpdateStatus::Downloading(21))
        );
        cell.publish(Status::Ready {
            version: "1.0".into(),
            installer: std::path::PathBuf::from("installer.exe"),
            notes: None,
        });
        assert!(matches!(
            cell.take_tray_refresh(),
            Some(UpdateStatus::Ready(_))
        ));
        cell.publish(Status::Error {
            message: "boom".into(),
        });
        assert_eq!(cell.take_tray_refresh(), None);
    }

    #[test]
    fn the_interval_skips_busy_and_ready_states() {
        assert!(should_auto_check(&Status::Idle));
        assert!(should_auto_check(&Status::UpToDate));
        assert!(should_auto_check(&Status::Unsupported));
        assert!(should_auto_check(&Status::Error {
            message: "boom".into()
        }));
        assert!(!should_auto_check(&Status::Checking));
        assert!(!should_auto_check(&Status::Downloading {
            version: "1.0".into(),
            progress: 0.5,
            notes: None,
        }));
        assert!(!should_auto_check(&Status::Ready {
            version: "1.0".into(),
            installer: std::path::PathBuf::from("installer.exe"),
            notes: None,
        }));
    }

    #[test]
    fn begin_download_only_from_available() {
        let available = Status::Available {
            version: "1.0".into(),
            artifact: Artifact {
                name: "a".into(),
                url: "u".into(),
                size: 0,
            },
            sha512: "s".into(),
            notes: Some("notes".into()),
        };
        let mut cell = UpdateCell::default();
        assert!(begin_download(&mut cell).is_none());
        cell.publish(available);
        let started = begin_download(&mut cell).expect("available starts");
        assert_eq!(started.0, "1.0");
        assert_eq!(started.3.as_deref(), Some("notes"));
        assert!(matches!(
            cell.status(),
            Status::Downloading { progress, .. } if progress == 0.0
        ));
        assert!(begin_download(&mut cell).is_none());
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
