use std::path::Path;
use std::process::Command;

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
    scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
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

pub fn reveal_in_file_manager(path: &Path) {
    let target = path.to_string_lossy().to_string();
    let result = if cfg!(target_os = "windows") {
        Command::new("explorer")
            .arg(format!("/select,{target}"))
            .spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").args(["-R", &target]).spawn()
    } else {
        let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
        Command::new("xdg-open").arg(parent).spawn()
    };
    if let Err(error) = result {
        eprintln!("[desktop] failed to reveal {target}: {error}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn external_urls_allow_only_http_and_https() {
        assert!(super::allowed_external_url("https://example.com/a&b|c"));
        assert!(super::allowed_external_url("HTTP://example.com"));
        assert!(!super::allowed_external_url(
            "file:///C:/Windows/System32/calc.exe"
        ));
        assert!(!super::allowed_external_url("javascript:alert(1)"));
        assert!(!super::allowed_external_url("example.com"));
    }
}
