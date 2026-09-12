#[cfg(windows)]
use std::hash::{DefaultHasher, Hash, Hasher};

#[cfg(windows)]
use windows::core::HSTRING;
#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
#[cfg(windows)]
use windows::Win32::System::Threading::CreateMutexW;

/// The profile whose claim this guard represents.
pub fn acquire() -> Option<SingleInstanceGuard> {
    acquire_in(&crate::config::store::config_dir())
}

#[cfg(windows)]
pub struct SingleInstanceGuard {
    handle: HANDLE,
}

#[cfg(not(windows))]
pub struct SingleInstanceGuard {
    path: std::path::PathBuf,
    _file: std::fs::File,
}

/// `acquire` against an explicit profile directory.
///
/// The public entry resolves the process's own profile; this is the seam the
/// test drives. It exists because a test that reached for the ambient profile
/// would both disturb the developer's real state and *fail* whenever the app is
/// already running — the guard would be reported as held, which is the correct
/// answer to the question the test is not asking.
#[cfg(windows)]
fn acquire_in(dir: &std::path::Path) -> Option<SingleInstanceGuard> {
    let mut hasher = DefaultHasher::new();
    dir.hash(&mut hasher);
    let name = HSTRING::from(format!("Local\\Poratake-GPUI-{:016x}", hasher.finish()));
    let handle = unsafe { CreateMutexW(None, false, &name) }.ok()?;
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe { CloseHandle(handle) }.ok();
        return None;
    }
    Some(SingleInstanceGuard { handle })
}

#[cfg(not(windows))]
fn acquire_in(dir: &std::path::Path) -> Option<SingleInstanceGuard> {
    use std::io::Write;

    let path = dir.join("poratake-gpui.lock");
    std::fs::create_dir_all(path.parent()?).ok()?;
    for _ in 0..2 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                writeln!(file, "{}", std::process::id()).ok()?;
                return Some(SingleInstanceGuard { path, _file: file });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let running = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|value| value.trim().parse::<u32>().ok())
                    .is_some_and(process_is_running);
                if running || std::fs::remove_file(&path).is_err() {
                    return None;
                }
            }
            Err(_) => return None,
        }
    }
    None
}

#[cfg(not(windows))]
fn process_is_running(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(windows)]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) }.ok();
    }
}

#[cfg(not(windows))]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    /// Driven against a throwaway directory, not the developer's profile: the
    /// guard is per-profile, so asking about the ambient one would make this
    /// test fail whenever the app happens to be running, and would delete the
    /// running instance's lock on the way out.
    #[test]
    fn a_second_guard_for_the_same_profile_is_rejected() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first = super::acquire_in(dir.path()).expect("first guard");
        assert!(super::acquire_in(dir.path()).is_none());
        drop(first);
        assert!(super::acquire_in(dir.path()).is_some());
    }

    /// Two profiles are independent claims, which is what lets a dev build and
    /// an installed build coexist.
    #[test]
    fn a_guard_for_another_profile_is_not_rejected() {
        let first_dir = tempfile::tempdir().expect("first dir");
        let second_dir = tempfile::tempdir().expect("second dir");
        let _first = super::acquire_in(first_dir.path()).expect("first guard");
        assert!(super::acquire_in(second_dir.path()).is_some());
    }
}
