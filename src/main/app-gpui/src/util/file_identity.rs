#[cfg(unix)]
use std::fs;
use std::fs::File;
use std::io;
use std::path::Path;

pub(crate) fn from_file(file: &File) -> io::Result<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let metadata = file.metadata()?;
        Ok((metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };

        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        unsafe {
            GetFileInformationByHandle(HANDLE(file.as_raw_handle() as _), &mut information)?;
        }
        Ok(from_windows_information(&information))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "file identity is unavailable on this platform",
        ))
    }
}

#[cfg(unix)]
pub(crate) fn from_path(path: &Path) -> io::Result<(u64, u64)> {
    from_metadata(path)
}

#[cfg(windows)]
pub(crate) fn from_path(path: &Path) -> io::Result<(u64, u64)> {
    from_file(&File::open(path)?)
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn from_path(path: &Path) -> io::Result<(u64, u64)> {
    from_file(&File::open(path)?)
}

#[cfg(unix)]
pub(crate) fn from_metadata(path: &Path) -> io::Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::metadata(path)?;
    Ok((metadata.dev(), metadata.ino()))
}

pub(crate) fn paths_match(left: &Path, right: &Path) -> bool {
    match (from_path(left), from_path(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

#[cfg(windows)]
pub(crate) fn from_windows_information(
    information: &windows::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION,
) -> (u64, u64) {
    (
        u64::from(information.dwVolumeSerialNumber),
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow),
    )
}
