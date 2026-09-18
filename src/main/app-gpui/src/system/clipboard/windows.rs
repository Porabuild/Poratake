use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_HDROP;
use windows::Win32::UI::Shell::DROPFILES;

pub fn write_file(path: &Path) -> bool {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    wide.push(0);
    let header = std::mem::size_of::<DROPFILES>();
    let bytes = header + wide.len() * std::mem::size_of::<u16>();
    unsafe {
        let Ok(memory) = GlobalAlloc(GMEM_MOVEABLE, bytes) else {
            return false;
        };
        if !fill_drop_files(memory, header, &wide) {
            return false;
        }
        if OpenClipboard(None).is_err() {
            return false;
        }
        let written = EmptyClipboard().is_ok()
            && SetClipboardData(CF_HDROP.0 as u32, Some(HANDLE(memory.0))).is_ok();
        let _ = CloseClipboard();
        written
    }
}

unsafe fn fill_drop_files(memory: HGLOBAL, header: usize, wide: &[u16]) -> bool {
    let target = GlobalLock(memory);
    if target.is_null() {
        return false;
    }
    let descriptor = DROPFILES {
        pFiles: header as u32,
        pt: Default::default(),
        fNC: false.into(),
        fWide: true.into(),
    };
    std::ptr::write_unaligned(target.cast::<DROPFILES>(), descriptor);
    std::ptr::copy_nonoverlapping(
        wide.as_ptr(),
        target.byte_add(header).cast::<u16>(),
        wide.len(),
    );
    let _ = GlobalUnlock(memory);
    true
}
