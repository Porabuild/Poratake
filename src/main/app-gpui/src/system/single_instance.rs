use std::hash::{DefaultHasher, Hash, Hasher};

use windows::core::HSTRING;
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;

pub struct SingleInstanceGuard {
    handle: HANDLE,
}

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

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) }.ok();
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
