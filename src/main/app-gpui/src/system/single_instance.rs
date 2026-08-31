#[cfg(windows)]
use std::hash::{DefaultHasher, Hash, Hasher};

#[cfg(windows)]
use windows::core::HSTRING;
#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
#[cfg(windows)]
use windows::Win32::System::Threading::CreateMutexW;

#[cfg(windows)]
pub struct SingleInstanceGuard {
    handle: HANDLE,
}

#[cfg(not(windows))]
pub struct SingleInstanceGuard {
    path: std::path::PathBuf,
    _file: std::fs::File,
}

#[cfg(windows)]
pub fn acquire() -> Option<SingleInstanceGuard> {
    let mut hasher = DefaultHasher::new();
    crate::config::store::config_dir().hash(&mut hasher);
    let name = HSTRING::from(format!("Local\\Poratake-GPUI-{:016x}", hasher.finish()));
    let handle = unsafe { CreateMutexW(None, false, &name) }.ok()?;
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe { CloseHandle(handle) }.ok();
        return None;
    }
    Some(SingleInstanceGuard { handle })
}

#[cfg(not(windows))]
pub fn acquire() -> Option<SingleInstanceGuard> {
    use std::io::Write;

    let path = crate::config::store::config_dir().join("poratake-gpui.lock");
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
    #[test]
    fn a_second_guard_for_the_same_profile_is_rejected() {
        let first = super::acquire().expect("first guard");
        assert!(super::acquire().is_none());
        drop(first);
        assert!(super::acquire().is_some());
    }
}
