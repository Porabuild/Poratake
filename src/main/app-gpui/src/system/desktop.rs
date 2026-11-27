use std::path::{Path, PathBuf};
#[cfg(not(target_os = "windows"))]
use std::process::Command;

use herogpui::gpui;

#[derive(Clone, Debug, PartialEq, Eq)]
enum CaptureFolderAction {
    Reveal(PathBuf),
    #[cfg(not(target_os = "macos"))]
    OpenDirectory(PathBuf),
}

fn capture_folder_action(path: &Path) -> CaptureFolderAction {
    if !path.is_dir() {
        return CaptureFolderAction::Reveal(path.to_path_buf());
    }
    directory_action(path)
}

#[cfg(target_os = "macos")]
fn directory_action(path: &Path) -> CaptureFolderAction {
    CaptureFolderAction::Reveal(package_child(path))
}

#[cfg(target_os = "macos")]
fn package_child(path: &Path) -> PathBuf {
    let recording = path.join(crate::video::project::RECORDING);
    if recording.is_file() {
        return recording;
    }
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let candidate = entry.path();
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    path.to_path_buf()
}

#[cfg(not(target_os = "macos"))]
fn directory_action(path: &Path) -> CaptureFolderAction {
    CaptureFolderAction::OpenDirectory(path.to_path_buf())
}

pub(crate) fn show_capture_in_folder(cx: &gpui::App, path: &Path) {
    let path = path.to_path_buf();
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        let action = cx
            .background_executor()
            .spawn(async move { capture_folder_action(&path) })
            .await;
        cx.update(|cx| match action {
            CaptureFolderAction::Reveal(path) => cx.reveal_path(&path),
            #[cfg(not(target_os = "macos"))]
            CaptureFolderAction::OpenDirectory(path) => cx.open_with_system(&path),
        });
    })
    .detach();
}

pub fn open_url(url: &str) {
    if !allowed_external_url(url) {
        eprintln!("[desktop] refused unsupported external URL");
        return;
    }

    #[cfg(target_os = "windows")]
    let result = open_url_windows(url);
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(url).spawn().map(|_| ());
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let result = Command::new("xdg-open").arg(url).spawn().map(|_| ());

    if let Err(error) = result {
        eprintln!("[desktop] failed to open {url}: {error}");
    }
}

fn allowed_external_url(url: &str) -> bool {
    let Some((scheme, _)) = url.split_once(':') else {
        return false;
    };
    scheme.eq_ignore_ascii_case("http")
        || scheme.eq_ignore_ascii_case("https")
        || scheme.eq_ignore_ascii_case("ms-settings")
        || scheme.eq_ignore_ascii_case("x-apple.systempreferences")
}

#[cfg(target_os = "windows")]
fn open_url_windows(url: &str) -> Result<(), String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let operation = HSTRING::from("open");
    let target = HSTRING::from(url);
    let result = unsafe {
        ShellExecuteW(
            None,
            &operation,
            &target,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        return Err(format!(
            "ShellExecuteW failed with code {}",
            result.0 as isize
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{capture_folder_action, CaptureFolderAction};
    use std::path::PathBuf;

    fn package_dir(name: &str) -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join(name);
        std::fs::create_dir(&project).unwrap();
        (directory, project)
    }

    #[test]
    fn capture_files_reveal_the_original_path() {
        let directory = tempfile::tempdir().unwrap();
        let image = directory.path().join("capture with spaces.png");
        std::fs::write(&image, [1_u8]).unwrap();

        assert_eq!(
            capture_folder_action(&image),
            CaptureFolderAction::Reveal(image)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_package_prefers_the_recording_over_sidecars() {
        let (_tmp, project) = package_dir("Sample Recording.poratake");
        std::fs::write(project.join(crate::video::project::RECORDING), []).unwrap();
        std::fs::write(project.join("project.json"), "{}").unwrap();
        std::fs::write(project.join("thumbnail.png"), [1_u8]).unwrap();

        assert_eq!(
            capture_folder_action(&project),
            CaptureFolderAction::Reveal(project.join(crate::video::project::RECORDING))
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_package_without_recording_reveals_another_child() {
        let (_tmp, project) = package_dir("Legacy Recording.poratake");
        let child = project.join("preview.png");
        std::fs::write(&child, [1_u8]).unwrap();

        assert_eq!(
            capture_folder_action(&project),
            CaptureFolderAction::Reveal(child)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_empty_package_reveals_the_package_itself() {
        let (_tmp, project) = package_dir("Empty Recording.poratake");

        assert_eq!(
            capture_folder_action(&project),
            CaptureFolderAction::Reveal(project)
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn directories_open_in_the_file_manager() {
        let (_tmp, project) = package_dir("Sample Recording.poratake");

        assert_eq!(
            capture_folder_action(&project),
            CaptureFolderAction::OpenDirectory(project)
        );
    }

    #[test]
    fn external_urls_allow_only_http_and_https() {
        assert!(super::allowed_external_url("https://example.com/a&b|c"));
        assert!(super::allowed_external_url("HTTP://example.com"));
        assert!(!super::allowed_external_url(
            "file:///C:/Windows/System32/calc.exe"
        ));
        assert!(!super::allowed_external_url("javascript:alert(1)"));
        assert!(!super::allowed_external_url("example.com"));
        assert!(super::allowed_external_url("ms-settings:privacy-webcam"));
        assert!(super::allowed_external_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        ));
    }
}
