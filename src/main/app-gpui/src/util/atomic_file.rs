use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "file has no parent directory")
    })?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "file has no name"))?;
    let (temp, mut file) = loop {
        let mut temp_name = name.to_os_string();
        temp_name.push(format!(
            ".{}.{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let temp = parent.join(temp_name);
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => break (temp, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace(&temp, path)?;
        sync_parent(parent)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(windows)]
pub fn replace(temp: &Path, path: &Path) -> io::Result<()> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    const ERROR_ACCESS_DENIED_HRESULT: i32 = 0x80070005u32 as i32;
    const ERROR_SHARING_VIOLATION_HRESULT: i32 = 0x80070020u32 as i32;

    let temp_h = HSTRING::from(temp.as_os_str());
    let path_h = HSTRING::from(path.as_os_str());
    for attempt in 0..16 {
        let result = unsafe {
            MoveFileExW(
                &temp_h,
                &path_h,
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        match result {
            Ok(()) => return Ok(()),
            Err(error)
                if attempt < 15
                    && matches!(
                        error.code().0,
                        ERROR_ACCESS_DENIED_HRESULT | ERROR_SHARING_VIOLATION_HRESULT
                    ) =>
            {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err(error) => return Err(io::Error::other(error)),
        }
    }
    unreachable!()
}

#[cfg(not(windows))]
pub fn replace(temp: &Path, path: &Path) -> io::Result<()> {
    fs::rename(temp, path)
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> io::Result<()> {
    fs::File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_file_and_removes_the_temporary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        fs::write(&path, b"old").unwrap();
        write(&path, b"new").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_failed_replacement_keeps_the_existing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("keep"), b"original").unwrap();
        assert!(write(&path, b"new").is_err());
        assert_eq!(fs::read(path.join("keep")).unwrap(), b"original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn concurrent_readers_only_see_complete_versions() {
        struct FinishOnDrop<'a>(&'a std::sync::atomic::AtomicBool);
        impl Drop for FinishOnDrop<'_> {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }

        #[cfg(windows)]
        fn read_shared(path: &Path) -> io::Result<Vec<u8>> {
            use std::io::Read;
            use std::os::windows::fs::OpenOptionsExt;
            let mut file = OpenOptions::new().read(true).share_mode(0b111).open(path)?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            Ok(buf)
        }

        #[cfg(not(windows))]
        fn read_shared(path: &Path) -> io::Result<Vec<u8>> {
            fs::read(path)
        }

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let first = vec![b'a'; 131_072];
        let second = vec![b'b'; 131_072];
        write(&path, &first).unwrap();
        let finished = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|scope| {
            let finished = &finished;
            let writer = scope.spawn(|| {
                let _done = FinishOnDrop(finished);
                for index in 0..12 {
                    write(&path, if index % 2 == 0 { &second } else { &first }).unwrap();
                }
            });
            while !finished.load(Ordering::Acquire) {
                let read = read_shared(&path).unwrap();
                assert!(read == first || read == second);
                std::thread::yield_now();
            }
            writer.join().unwrap();
        });
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
