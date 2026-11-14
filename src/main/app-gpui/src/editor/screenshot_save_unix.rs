use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_BACKUP_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(target_os = "linux")]
const OPEN_NONBLOCK: i32 = 0x800;
#[cfg(target_os = "macos")]
const OPEN_NONBLOCK: i32 = 0x4;
#[cfg(target_os = "linux")]
const OPEN_DIRECTORY: i32 = 0x10000;
#[cfg(target_os = "macos")]
const OPEN_DIRECTORY: i32 = 0x100000;
#[cfg(target_os = "linux")]
const OPEN_NOFOLLOW: i32 = 0x20000;
#[cfg(target_os = "macos")]
const OPEN_NOFOLLOW: i32 = 0x100;
#[cfg(target_os = "linux")]
const OPEN_READ_WRITE: i32 = 0x2;
#[cfg(target_os = "macos")]
const OPEN_READ_WRITE: i32 = 0x2;
#[cfg(target_os = "linux")]
const OPEN_CREATE: i32 = 0x40;
#[cfg(target_os = "macos")]
const OPEN_CREATE: i32 = 0x200;
#[cfg(target_os = "linux")]
const OPEN_EXCLUSIVE: i32 = 0x80;
#[cfg(target_os = "macos")]
const OPEN_EXCLUSIVE: i32 = 0x800;
#[cfg(target_os = "linux")]
const OPEN_CLOEXEC: i32 = 0x80000;
#[cfg(target_os = "macos")]
const OPEN_CLOEXEC: i32 = 0x1000000;
#[cfg(target_os = "macos")]
const MACOS_ACL_TYPE_EXTENDED: i32 = 0x00000100;
#[cfg(target_os = "macos")]
const MACOS_ACL_EXTENDED_ALLOW: i32 = 1;
#[cfg(target_os = "macos")]
const MACOS_ACL_DELETE_CHILD: i32 = 1 << 6;
#[cfg(target_os = "macos")]
const MACOS_ACL_ENTRY_ONLY_INHERIT: i32 = 1 << 8;
#[cfg(target_os = "macos")]
const MACOS_EINVAL: i32 = 22;

#[cfg(target_os = "linux")]
#[link(name = "c")]
unsafe extern "C" {
    fn fchmod(fd: i32, mode: u32) -> i32;
    fn openat(dirfd: i32, path: *const i8, flags: i32, mode: u32) -> i32;
    fn unlinkat(dirfd: i32, path: *const i8, flags: i32) -> i32;
    fn geteuid() -> u32;
}

#[cfg(target_os = "macos")]
#[link(name = "System")]
unsafe extern "C" {
    fn fchmod(fd: i32, mode: u32) -> i32;
    fn openat(dirfd: i32, path: *const i8, flags: i32, mode: u32) -> i32;
    fn unlinkat(dirfd: i32, path: *const i8, flags: i32) -> i32;
    fn geteuid() -> u32;
}

pub(crate) fn write(path: &Path, bytes: &[u8]) -> io::Result<Option<String>> {
    match open_existing(path) {
        Ok(target) => write_existing(path, target, bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let destination = resolve_destination(path)?;
            match open_existing(&destination) {
                Ok(target) => write_existing(path, target, bytes),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    write_new(&destination, path, bytes)
                }
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

fn open_existing(path: &Path) -> io::Result<File> {
    let selected = fs::metadata(path)?;
    if !selected.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "screenshot destination is not a regular file",
        ));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(OPEN_NONBLOCK)
        .open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || (opened.dev(), opened.ino()) != (selected.dev(), selected.ino()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "screenshot destination changed to a non-file",
        ));
    }
    Ok(file)
}

fn resolve_destination(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut candidate = absolute;
    for _ in 0..40 {
        let parent = candidate.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "screenshot destination has no parent",
            )
        })?;
        let name = candidate.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "screenshot destination has no name",
            )
        })?;
        let candidate_parent = fs::canonicalize(parent)?;
        candidate = candidate_parent.join(name);
        match fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let target = fs::read_link(&candidate)?;
                candidate = if target.is_absolute() {
                    target
                } else {
                    candidate_parent.join(target)
                };
            }
            Ok(_) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(candidate),
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "screenshot destination has too many symbolic links",
    ))
}

fn write_new(path: &Path, selected_path: &Path, bytes: &[u8]) -> io::Result<Option<String>> {
    write_new_with(path, selected_path, bytes, write_content)
}

fn write_new_with(
    path: &Path,
    selected_path: &Path,
    bytes: &[u8],
    write_content: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
) -> io::Result<Option<String>> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let identity = match file_identity(&file) {
        Ok(identity) => identity,
        Err(error) => {
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "{error}; empty screenshot file remains at {} because its identity could not be verified",
                    path.display()
                ),
            ));
        }
    };
    if let Err(error) = write_content(&mut file, bytes) {
        let cleanup = file.set_len(0).and_then(|()| file.sync_all());
        return Err(match cleanup {
            Ok(()) => io::Error::new(
                error.kind(),
                format!(
                    "{error}; the newly created file remains empty at {}",
                    path.display()
                ),
            ),
            Err(cleanup_error) => io::Error::new(
                error.kind(),
                format!(
                    "{error}; incomplete output may remain at {} because clearing it failed: {cleanup_error}",
                    path.display()
                ),
            ),
        });
    }
    let current_identity =
        fs::metadata(selected_path).map(|metadata| (metadata.dev(), metadata.ino()))?;
    if current_identity != identity {
        return Err(io::Error::other(format!(
            "screenshot destination changed while saving; completed output may remain at {}",
            selected_path.display()
        )));
    }
    Ok(None)
}

fn write_existing(path: &Path, target: File, bytes: &[u8]) -> io::Result<Option<String>> {
    write_existing_with(
        path,
        target,
        bytes,
        copy_snapshot,
        write_content,
        restore_content,
    )
}

fn write_existing_with(
    path: &Path,
    mut target: File,
    bytes: &[u8],
    copy_snapshot: impl FnOnce(&mut File, &mut File) -> io::Result<u64>,
    write_content: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    restore_content: impl FnOnce(&mut File, &mut File, u64) -> io::Result<()>,
) -> io::Result<Option<String>> {
    verify_selected_path(path, &target)?;
    let identity = file_identity(&target)?;
    let target_path = fs::canonicalize(path)?;
    verify_selected_path(path, &target)?;
    let parent = target_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "screenshot destination has no parent",
        )
    })?;
    let expected_length = target.metadata()?.len();
    let expected_parent = fs::metadata(parent)?;
    let parent_handle = open_backup_parent(parent)?;
    let opened_parent = parent_handle.metadata()?;
    if (expected_parent.dev(), expected_parent.ino()) != (opened_parent.dev(), opened_parent.ino())
    {
        return Err(io::Error::other(
            "screenshot directory changed while preparing recovery data",
        ));
    }
    let (backup_path, mut backup) = create_private_backup(&parent_handle, parent, identity.0)?;
    if let Err(error) = copy_snapshot(&mut target, &mut backup) {
        return Err(cleanup_backup_error(error, &parent_handle, &backup_path));
    }
    let backup_length = match backup.metadata() {
        Ok(metadata) => metadata.len(),
        Err(error) => return Err(cleanup_backup_error(error, &parent_handle, &backup_path)),
    };
    let target_length = match target.metadata() {
        Ok(metadata) => metadata.len(),
        Err(error) => return Err(cleanup_backup_error(error, &parent_handle, &backup_path)),
    };
    if backup_length != expected_length || target_length != expected_length {
        return Err(cleanup_backup_error(
            io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "screenshot recovery copy length does not match the original",
            ),
            &parent_handle,
            &backup_path,
        ));
    }
    verify_selected_path(path, &target)
        .map_err(|error| cleanup_backup_error(error, &parent_handle, &backup_path))?;

    if let Err(error) = write_content(&mut target, bytes) {
        return match restore_content(&mut target, &mut backup, expected_length) {
            Ok(()) => Err(cleanup_restored_failure(
                error,
                &parent_handle,
                &backup_path,
            )),
            Err(restore_error) => Err(io::Error::new(
                error.kind(),
                format!(
                    "{error}; restoring the original failed: {restore_error}; original content remains at {}",
                    backup_path.display()
                ),
            )),
        };
    }
    if let Err(error) = verify_selected_path(path, &target) {
        drop(backup);
        return Err(io::Error::new(
            error.kind(),
            format!(
                "{error}; the completed screenshot is on the opened file and the original snapshot remains at {}",
                backup_path.display()
            ),
        ));
    }
    drop(backup);
    cleanup_completed_save(&parent_handle, &backup_path)
}

fn open_backup_parent(path: &Path) -> io::Result<File> {
    let parent = OpenOptions::new()
        .read(true)
        .custom_flags(OPEN_DIRECTORY | OPEN_NOFOLLOW)
        .open(path)?;
    let metadata = parent.metadata()?;
    if !metadata.is_dir()
        || !backup_parent_is_private(metadata.uid(), metadata.mode(), unsafe { geteuid() })
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "screenshot directory permits other users to replace recovery files",
        ));
    }
    #[cfg(target_os = "macos")]
    verify_macos_parent_acl(&parent)?;
    Ok(parent)
}

fn backup_parent_is_private(owner: u32, mode: u32, current_user: u32) -> bool {
    let sticky = mode & 0o1000 != 0;
    (mode & 0o022 == 0 || sticky) && (owner == current_user || (sticky && owner == 0))
}

fn create_private_backup(
    parent: &File,
    parent_path: &Path,
    expected_device: u64,
) -> io::Result<(PathBuf, File)> {
    for _ in 0..=128 {
        let id = NEXT_BACKUP_ID.fetch_add(1, Ordering::Relaxed);
        let name = format!(".poratake-save-{}-{id}", std::process::id());
        let backup = match open_backup_file(parent, &name, 0o600) {
            Ok(backup) => backup,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let backup_path = parent_path.join(&name);
        if let Err(error) = restrict_private_file(&backup) {
            return Err(cleanup_backup_error(error, parent, &backup_path));
        }
        let metadata = match backup.metadata() {
            Ok(metadata) => metadata,
            Err(error) => return Err(cleanup_backup_error(error, parent, &backup_path)),
        };
        if metadata.dev() != expected_device {
            return Err(cleanup_backup_error(
                io::Error::other("screenshot recovery copy is not on the destination filesystem"),
                parent,
                &backup_path,
            ));
        }
        return Ok((backup_path, backup));
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not reserve a private screenshot recovery file",
    ))
}

fn open_backup_file(parent: &File, name: &str, mode: u32) -> io::Result<File> {
    let name_c = std::ffi::CString::new(name)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "recovery name contains nul"))?;
    let descriptor = unsafe {
        openat(
            parent.as_raw_fd(),
            name_c.as_ptr(),
            OPEN_READ_WRITE | OPEN_CREATE | OPEN_EXCLUSIVE | OPEN_NOFOLLOW | OPEN_CLOEXEC,
            mode,
        )
    };
    if descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

fn restrict_private_file(file: &File) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    clear_macos_acl(file)?;
    if unsafe { fchmod(file.as_raw_fd(), 0o600) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let metadata = file.metadata()?;
    if metadata.permissions().mode() & 0o077 != 0 || metadata.uid() != unsafe { geteuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "filesystem did not enforce private screenshot recovery permissions",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_macos_parent_acl(parent: &File) -> io::Result<()> {
    let acl = unsafe { macos_acl::acl_get_fd_np(parent.as_raw_fd(), MACOS_ACL_TYPE_EXTENDED) };
    if acl.is_null() {
        return match io::Error::last_os_error().kind() {
            io::ErrorKind::NotFound => Ok(()),
            _ => Err(io::Error::last_os_error()),
        };
    }
    let mut owner_uuid = [0u8; 16];
    if unsafe { macos_acl::mbr_uid_to_uuid(geteuid(), owner_uuid.as_mut_ptr()) } != 0 {
        unsafe { macos_acl::acl_free(acl) };
        return Err(io::Error::last_os_error());
    }
    let mut entry = std::ptr::null_mut();
    let mut entry_id = 0;
    let mut reached_end = false;
    for _ in 0..=128 {
        let result = unsafe { macos_acl::acl_get_entry(acl, entry_id, &mut entry) };
        if result != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(MACOS_EINVAL) {
                reached_end = true;
                break;
            }
            unsafe { macos_acl::acl_free(acl) };
            return Err(error);
        }
        entry_id = -1;
        let mut flags = std::ptr::null_mut();
        if unsafe { macos_acl::acl_get_flagset_np(entry, &mut flags) } != 0 {
            unsafe { macos_acl::acl_free(acl) };
            return Err(io::Error::last_os_error());
        }
        let only_inherited =
            unsafe { macos_acl::acl_get_flag_np(flags, MACOS_ACL_ENTRY_ONLY_INHERIT) };
        if only_inherited < 0 {
            unsafe { macos_acl::acl_free(acl) };
            return Err(io::Error::last_os_error());
        }
        if only_inherited == 1 {
            continue;
        }
        let mut tag = 0;
        if unsafe { macos_acl::acl_get_tag_type(entry, &mut tag) } != 0 {
            unsafe { macos_acl::acl_free(acl) };
            return Err(io::Error::last_os_error());
        }
        if tag != MACOS_ACL_EXTENDED_ALLOW {
            continue;
        }
        let mut permissions = std::ptr::null_mut();
        if unsafe { macos_acl::acl_get_permset(entry, &mut permissions) } != 0 {
            unsafe { macos_acl::acl_free(acl) };
            return Err(io::Error::last_os_error());
        }
        let delete_child =
            unsafe { macos_acl::acl_get_perm_np(permissions, MACOS_ACL_DELETE_CHILD) };
        if delete_child < 0 {
            unsafe { macos_acl::acl_free(acl) };
            return Err(io::Error::last_os_error());
        }
        if delete_child == 0 {
            continue;
        }
        let qualifier = unsafe { macos_acl::acl_get_qualifier(entry) };
        if qualifier.is_null() {
            unsafe { macos_acl::acl_free(acl) };
            return Err(io::Error::last_os_error());
        }
        let is_owner = unsafe {
            std::slice::from_raw_parts(qualifier.cast::<u8>(), owner_uuid.len()) == owner_uuid
        };
        unsafe { macos_acl::acl_free(qualifier) };
        if !is_owner {
            unsafe { macos_acl::acl_free(acl) };
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "screenshot directory ACL lets another user replace recovery files",
            ));
        }
    }
    if !reached_end {
        unsafe { macos_acl::acl_free(acl) };
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "screenshot directory ACL could not be fully checked",
        ));
    }
    unsafe { macos_acl::acl_free(acl) };
    Ok(())
}

#[cfg(target_os = "macos")]
fn clear_macos_acl(file: &File) -> io::Result<()> {
    let acl = unsafe { macos_acl::acl_from_text(c"!#acl 1".as_ptr()) };
    if acl.is_null() {
        return Err(io::Error::last_os_error());
    }
    let set_result =
        unsafe { macos_acl::acl_set_fd_np(file.as_raw_fd(), acl, MACOS_ACL_TYPE_EXTENDED) };
    let set_error = (set_result != 0).then(io::Error::last_os_error);
    let free_result = unsafe { macos_acl::acl_free(acl.cast()) };
    if let Some(error) = set_error {
        return Err(error);
    }
    if free_result != 0 {
        return Err(io::Error::last_os_error());
    }
    let acl = unsafe { macos_acl::acl_get_fd_np(file.as_raw_fd(), MACOS_ACL_TYPE_EXTENDED) };
    if acl.is_null() {
        return match io::Error::last_os_error().kind() {
            io::ErrorKind::NotFound => Ok(()),
            _ => Err(io::Error::last_os_error()),
        };
    }
    unsafe { macos_acl::acl_free(acl) };
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "filesystem retained an extended ACL on screenshot recovery data",
    ))
}

#[cfg(target_os = "macos")]
mod macos_acl {
    use std::ffi::{c_char, c_int, c_void};

    #[link(name = "System")]
    unsafe extern "C" {
        pub(super) fn acl_from_text(text: *const c_char) -> *mut c_void;
        pub(super) fn acl_set_fd_np(fd: c_int, acl: *mut c_void, kind: c_int) -> c_int;
        pub(super) fn acl_get_fd_np(fd: c_int, kind: c_int) -> *mut c_void;
        pub(super) fn acl_get_entry(
            acl: *mut c_void,
            entry_id: c_int,
            entry: *mut *mut c_void,
        ) -> c_int;
        pub(super) fn acl_get_tag_type(entry: *mut c_void, tag: *mut c_int) -> c_int;
        pub(super) fn acl_get_permset(entry: *mut c_void, permissions: *mut *mut c_void) -> c_int;
        pub(super) fn acl_get_perm_np(permissions: *mut c_void, permission: c_int) -> c_int;
        pub(super) fn acl_get_qualifier(entry: *mut c_void) -> *mut c_void;
        pub(super) fn acl_get_flagset_np(object: *mut c_void, flags: *mut *mut c_void) -> c_int;
        pub(super) fn acl_get_flag_np(flags: *mut c_void, flag: c_int) -> c_int;
        pub(super) fn mbr_uid_to_uuid(uid: u32, uuid: *mut u8) -> c_int;
        pub(super) fn acl_free(object: *mut c_void) -> c_int;
        #[cfg(test)]
        pub(super) fn acl_set_file(path: *const c_char, kind: c_int, acl: *mut c_void) -> c_int;
    }
}

fn remove_backup(parent: &File, backup_path: &Path) -> io::Result<()> {
    let name = backup_path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "recovery file has no name"))?;
    let name = std::ffi::CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "recovery name contains nul"))?;
    if unsafe { unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn copy_snapshot(source: &mut File, backup: &mut File) -> io::Result<u64> {
    source.seek(SeekFrom::Start(0))?;
    let expected_length = source.metadata()?.len();
    let copied = io::copy(source, backup)?;
    backup.sync_all()?;
    if copied != expected_length || backup.metadata()?.len() != expected_length {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "screenshot recovery copy was incomplete",
        ));
    }
    Ok(copied)
}

fn write_content(file: &mut File, bytes: &[u8]) -> io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn restore_content(target: &mut File, backup: &mut File, expected_length: u64) -> io::Result<()> {
    if backup.metadata()?.len() != expected_length {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "screenshot recovery copy length does not match the original",
        ));
    }
    backup.seek(SeekFrom::Start(0))?;
    target.set_len(0)?;
    target.seek(SeekFrom::Start(0))?;
    let copied = io::copy(backup, target)?;
    if copied != expected_length || target.metadata()?.len() != expected_length {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "restored screenshot length does not match the original",
        ));
    }
    target.sync_all()
}

fn verify_selected_path(path: &Path, target: &File) -> io::Result<()> {
    let target_identity = file_identity(target)?;
    let path_identity = fs::metadata(path).map(|metadata| (metadata.dev(), metadata.ino()))?;
    if target_identity != path_identity {
        return Err(io::Error::other(
            "screenshot destination changed during save; no overwrite was made",
        ));
    }
    Ok(())
}

fn file_identity(file: &File) -> io::Result<(u64, u64)> {
    let metadata = file.metadata()?;
    Ok((metadata.dev(), metadata.ino()))
}

fn cleanup_backup_error(error: io::Error, parent: &File, backup_path: &Path) -> io::Error {
    match remove_backup(parent, backup_path) {
        Ok(()) => error,
        Err(cleanup_error) => io::Error::new(
            error.kind(),
            format!(
                "{error}; recovery data remains at {} because cleanup failed: {cleanup_error}",
                backup_path.display()
            ),
        ),
    }
}

fn cleanup_restored_failure(error: io::Error, parent: &File, backup_path: &Path) -> io::Error {
    match remove_backup(parent, backup_path) {
        Ok(()) => error,
        Err(cleanup_error) => io::Error::new(
            error.kind(),
            format!(
                "{error}; original content was restored, but its recovery copy remains at {}: {cleanup_error}",
                backup_path.display()
            ),
        ),
    }
}

fn cleanup_completed_save(parent: &File, backup_path: &Path) -> io::Result<Option<String>> {
    match remove_backup(parent, backup_path) {
        Ok(()) => Ok(None),
        Err(error) => Ok(Some(format!(
            "Screenshot saved successfully, but recovery data remains at {}: {error}",
            backup_path.display()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            for _ in 0..128 {
                let id = NEXT_BACKUP_ID.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir()
                    .join(format!("poratake-save-test-{}-{id}", std::process::id()));
                match fs::create_dir(&path) {
                    Ok(()) => {
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
                        return Self(path);
                    }
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("could not create test directory: {error}"),
                }
            }
            panic!("could not reserve test directory");
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn partial_overwrite_failure_restores_same_inode_and_permissions() {
        let directory = TestDir::new();
        let path = directory.path().join("capture.png");
        fs::write(&path, b"original screenshot").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let original = fs::metadata(&path).unwrap();

        let result = write_existing_with(
            &path,
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .unwrap(),
            b"replacement screenshot",
            copy_snapshot,
            |file, bytes| {
                file.set_len(0)?;
                file.seek(SeekFrom::Start(0))?;
                file.write_all(&bytes[..5])?;
                Err(io::Error::other("injected partial write"))
            },
            restore_content,
        );

        assert!(result.is_err());
        let restored = fs::metadata(&path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"original screenshot");
        assert_eq!(
            (restored.dev(), restored.ino()),
            (original.dev(), original.ino())
        );
        assert_eq!(restored.permissions().mode() & 0o777, 0o640);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn backup_failure_leaves_original_unchanged() {
        let directory = TestDir::new();
        let path = directory.path().join("capture.png");
        fs::write(&path, b"original screenshot").unwrap();
        let target = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();

        let result = write_existing_with(
            &path,
            target,
            b"replacement screenshot",
            |_, _| Err(io::Error::other("injected backup failure")),
            write_content,
            restore_content,
        );

        assert!(result
            .unwrap_err()
            .to_string()
            .contains("injected backup failure"));
        assert_eq!(fs::read(&path).unwrap(), b"original screenshot");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn restore_failure_retains_and_reports_original_snapshot() {
        let directory = TestDir::new();
        let path = directory.path().join("capture.png");
        fs::write(&path, b"original screenshot").unwrap();

        let result = write_existing_with(
            &path,
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .unwrap(),
            b"replacement screenshot",
            copy_snapshot,
            |file, bytes| {
                file.set_len(0)?;
                file.seek(SeekFrom::Start(0))?;
                file.write_all(&bytes[..5])?;
                Err(io::Error::other("injected partial write"))
            },
            |_, _, _| Err(io::Error::other("injected restore failure")),
        );

        let message = result.unwrap_err().to_string();
        assert!(message.contains("injected restore failure"));
        let backup_path = PathBuf::from(
            message
                .split("original content remains at ")
                .last()
                .unwrap(),
        );
        assert_eq!(fs::read(&backup_path).unwrap(), b"original screenshot");
        assert_eq!(fs::read(&path).unwrap(), b"repla");
        fs::remove_file(backup_path).unwrap();
    }

    #[test]
    fn symlink_and_hardlink_overwrites_keep_their_targets() {
        let directory = TestDir::new();
        let target = directory.path().join("target.png");
        let symlink = directory.path().join("selected.png");
        let hardlink = directory.path().join("linked.png");
        fs::write(&target, b"original screenshot").unwrap();
        std::os::unix::fs::symlink(&target, &symlink).unwrap();
        fs::hard_link(&target, &hardlink).unwrap();
        let identity = fs::metadata(&target).unwrap();

        write(&symlink, b"replacement screenshot").unwrap();

        assert!(fs::symlink_metadata(&symlink)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(&target).unwrap(), b"replacement screenshot");
        assert_eq!(fs::read(&hardlink).unwrap(), b"replacement screenshot");
        let replaced = fs::metadata(&target).unwrap();
        assert_eq!(
            (replaced.dev(), replaced.ino()),
            (identity.dev(), identity.ino())
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 3);
    }

    #[test]
    fn new_file_partial_write_failure_clears_partial_bytes_without_unlinking() {
        let directory = TestDir::new();
        let path = directory.path().join("capture.png");

        let result = write_new_with(&path, &path, b"replacement screenshot", |file, bytes| {
            file.write_all(&bytes[..5])?;
            Err(io::Error::other("injected partial write"))
        });

        let message = result.unwrap_err().to_string();
        assert!(message.contains("injected partial write"));
        assert!(message.contains("newly created file remains empty"));
        assert_eq!(fs::metadata(&path).unwrap().len(), 0);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn new_file_path_replacement_after_write_is_reported() {
        let directory = TestDir::new();
        let path = directory.path().join("capture.png");
        let replacement = directory.path().join("replacement.png");
        fs::write(&replacement, b"concurrent replacement").unwrap();

        let result = write_new_with(&path, &path, b"new screenshot", |file, bytes| {
            file.write_all(bytes)?;
            fs::remove_file(&path)?;
            fs::rename(&replacement, &path)
        });

        assert!(result
            .unwrap_err()
            .to_string()
            .contains("destination changed while saving"));
        assert_eq!(fs::read(&path).unwrap(), b"concurrent replacement");
    }

    #[test]
    fn dangling_symlink_retarget_after_write_is_reported() {
        let directory = TestDir::new();
        let selected = directory.path().join("selected.png");
        let destination = directory.path().join("new-target.png");
        let competitor = directory.path().join("competitor.png");
        fs::write(&competitor, b"concurrent replacement").unwrap();
        std::os::unix::fs::symlink(&destination, &selected).unwrap();

        let result = write_new_with(&destination, &selected, b"new screenshot", |file, bytes| {
            file.write_all(bytes)?;
            fs::remove_file(&selected)?;
            std::os::unix::fs::symlink(&competitor, &selected)
        });

        assert!(result
            .unwrap_err()
            .to_string()
            .contains("destination changed while saving"));
        assert_eq!(fs::read(&competitor).unwrap(), b"concurrent replacement");
        assert_eq!(fs::read(&destination).unwrap(), b"new screenshot");
    }

    #[test]
    fn destination_identity_change_aborts_before_overwrite() {
        let directory = TestDir::new();
        let path = directory.path().join("capture.png");
        let replacement = directory.path().join("replacement.png");
        fs::write(&path, b"selected original").unwrap();
        fs::write(&replacement, b"concurrent replacement").unwrap();
        let target = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        fs::remove_file(&path).unwrap();
        fs::rename(&replacement, &path).unwrap();

        let result = write_existing_with(
            &path,
            target,
            b"new screenshot",
            copy_snapshot,
            write_content,
            restore_content,
        );

        assert!(result
            .unwrap_err()
            .to_string()
            .contains("destination changed"));
        assert_eq!(fs::read(&path).unwrap(), b"concurrent replacement");
    }

    #[test]
    fn recovery_data_is_private_and_stays_on_the_target_filesystem() {
        let directory = TestDir::new();
        let path = directory.path().join("capture.png");
        fs::write(&path, b"original screenshot").unwrap();
        let target = File::open(&path).unwrap();
        let target_device = target.metadata().unwrap().dev();
        let parent = open_backup_parent(directory.path()).unwrap();
        let (backup_path, backup) =
            create_private_backup(&parent, directory.path(), target_device).unwrap();
        assert_eq!(
            backup.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(backup.metadata().unwrap().dev(), target_device);
        drop(backup);
        remove_backup(&parent, &backup_path).unwrap();
    }

    #[test]
    fn sticky_parent_only_allows_its_owner_or_root() {
        assert!(backup_parent_is_private(1000, 0o700, 1000));
        assert!(backup_parent_is_private(0, 0o1777, 1000));
        assert!(!backup_parent_is_private(2000, 0o1777, 1000));
        assert!(!backup_parent_is_private(1000, 0o777, 1000));
    }

    #[test]
    fn shared_replaceable_directory_fails_before_original_mutation() {
        let directory = TestDir::new();
        let path = directory.path().join("capture.png");
        fs::write(&path, b"original screenshot").unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o777)).unwrap();

        let result = write(&path, b"replacement screenshot");

        assert!(result
            .unwrap_err()
            .to_string()
            .contains("permits other users to replace recovery files"));
        assert_eq!(fs::read(&path).unwrap(), b"original screenshot");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn path_replaced_after_snapshot_aborts_and_keeps_new_target() {
        let directory = TestDir::new();
        let path = directory.path().join("capture.png");
        let replacement = directory.path().join("replacement.png");
        fs::write(&path, b"selected original").unwrap();
        fs::write(&replacement, b"concurrent replacement").unwrap();
        let target = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();

        let result = write_existing_with(
            &path,
            target,
            b"new screenshot",
            |target, backup| {
                copy_snapshot(target, backup)?;
                fs::remove_file(&path)?;
                fs::rename(&replacement, &path)?;
                Ok(backup.metadata()?.len())
            },
            write_content,
            restore_content,
        );

        assert!(result
            .unwrap_err()
            .to_string()
            .contains("destination changed"));
        assert_eq!(fs::read(&path).unwrap(), b"concurrent replacement");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn path_replaced_during_write_retains_original_snapshot() {
        let directory = TestDir::new();
        let path = directory.path().join("capture.png");
        let replacement = directory.path().join("replacement.png");
        fs::write(&path, b"selected original").unwrap();
        fs::write(&replacement, b"concurrent replacement").unwrap();
        let target = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();

        let result = write_existing_with(
            &path,
            target,
            b"new screenshot",
            copy_snapshot,
            |target, bytes| {
                write_content(target, bytes)?;
                fs::remove_file(&path)?;
                fs::rename(&replacement, &path)
            },
            restore_content,
        );

        let message = result.unwrap_err().to_string();
        assert!(message.contains("completed screenshot is on the opened file"));
        let backup_path = PathBuf::from(
            message
                .split("original snapshot remains at ")
                .last()
                .unwrap(),
        );
        assert_eq!(fs::read(&backup_path).unwrap(), b"selected original");
        assert_eq!(fs::read(&path).unwrap(), b"concurrent replacement");
        fs::remove_file(backup_path).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn inherited_extended_acl_is_removed_before_snapshot_bytes_are_copied() {
        let directory = TestDir::new();
        let target_path = directory.path().join("capture.png");
        fs::write(&target_path, b"original screenshot").unwrap();
        set_test_acl(
            directory.path(),
            "!#acl 1\ngroup:everyone deny delete\ngroup:everyone allow read,readattr,readextattr,file_inherit,directory_inherit",
        );
        let parent = open_backup_parent_without_acl_gate(directory.path()).unwrap();
        let target = File::open(&target_path).unwrap();
        let device = target.metadata().unwrap().dev();
        let inherited_name = "acl-inheritance-probe";
        let inherited_path = directory.path().join(inherited_name);
        let inherited_probe = open_backup_file(&parent, inherited_name, 0o600).unwrap();
        let inherited_acl = unsafe {
            macos_acl::acl_get_fd_np(inherited_probe.as_raw_fd(), MACOS_ACL_TYPE_EXTENDED)
        };
        assert!(
            !inherited_acl.is_null(),
            "fixture ACL did not inherit to the backup file"
        );
        unsafe { macos_acl::acl_free(inherited_acl) };
        drop(inherited_probe);
        remove_backup(&parent, &inherited_path).unwrap();
        let (backup_path, mut backup) =
            create_private_backup(&parent, directory.path(), device).unwrap();

        assert!(verify_macos_parent_acl(&parent).is_ok());
        assert!(verify_macos_parent_acl(&backup).is_ok());
        backup.write_all(b"sensitive snapshot").unwrap();
        backup.sync_all().unwrap();
        assert!(verify_macos_parent_acl(&backup).is_ok());
        assert_eq!(fs::read(&backup_path).unwrap(), b"sensitive snapshot");
        drop(backup);
        remove_backup(&parent, &backup_path).unwrap();

        fn open_backup_parent_without_acl_gate(path: &Path) -> io::Result<File> {
            OpenOptions::new()
                .read(true)
                .custom_flags(OPEN_DIRECTORY | OPEN_NOFOLLOW)
                .open(path)
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn everyone_delete_child_allow_is_rejected() {
        let directory = TestDir::new();
        set_test_acl(
            directory.path(),
            "!#acl 1\ngroup:everyone allow delete_child",
        );
        let parent = OpenOptions::new()
            .read(true)
            .custom_flags(OPEN_DIRECTORY | OPEN_NOFOLLOW)
            .open(directory.path())
            .unwrap();

        assert!(verify_macos_parent_acl(&parent).is_err());
    }

    #[cfg(target_os = "macos")]
    fn set_test_acl(path: &Path, acl_text: &str) {
        let path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        let acl_text = std::ffi::CString::new(acl_text).unwrap();
        let acl = unsafe { macos_acl::acl_from_text(acl_text.as_ptr()) };
        assert!(!acl.is_null(), "could not build test ACL");
        let result =
            unsafe { macos_acl::acl_set_file(path.as_ptr(), MACOS_ACL_TYPE_EXTENDED, acl) };
        unsafe { macos_acl::acl_free(acl) };
        assert_eq!(result, 0, "could not set test ACL");
    }
}
