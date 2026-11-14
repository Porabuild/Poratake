use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use windows::core::{HRESULT, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GENERIC_WRITE, HANDLE};
use windows::Win32::Security::{
    AclSizeInformation, AddAccessAllowedAceEx, CreateWellKnownSid, GetAclInformation,
    GetFileSecurityW, GetSecurityDescriptorControl, GetSecurityDescriptorDacl, GetTokenInformation,
    InitializeAcl, InitializeSecurityDescriptor, SetSecurityDescriptorControl,
    SetSecurityDescriptorDacl, TokenUser, WinLocalSystemSid, ACL, ACL_REVISION,
    ACL_SIZE_INFORMATION, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, INHERITED_ACE,
    INHERIT_NO_PROPAGATE, INHERIT_ONLY_ACE, OBJECT_INHERIT_ACE, PSECURITY_DESCRIPTOR, PSID,
    SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR, SE_DACL_PROTECTED, TOKEN_QUERY, TOKEN_USER,
};
use windows::Win32::Storage::FileSystem::{
    CreateDirectoryW, EncryptFileW, FileDispositionInfo, GetFileInformationByHandle,
    SetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, DELETE, FILE_ALL_ACCESS,
    FILE_ATTRIBUTE_ENCRYPTED, FILE_DISPOSITION_INFO, FILE_SHARE_READ,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

static NEXT_STAGE_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn write(path: &Path, bytes: &[u8]) -> io::Result<Option<String>> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let (destination, existed) = match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() && !metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "screenshot destination is not a file",
                ));
            }
            let destination = fs::canonicalize(path)?;
            if !fs::metadata(&destination)?.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "screenshot destination does not resolve to a file",
                ));
            }
            (destination, true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let name = path.file_name().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "screenshot destination has no name",
                )
            })?;
            (fs::canonicalize(parent)?.join(name), false)
        }
        Err(error) => return Err(error),
    };

    if existed {
        write_existing(&destination, bytes)
    } else {
        write_new(&destination, bytes)
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<Option<String>> {
    write_new_with(path, bytes, write_content)
}

fn write_new_with(
    path: &Path,
    bytes: &[u8],
    write_content: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
) -> io::Result<Option<String>> {
    let mut file = OpenOptions::new()
        .write(true)
        .access_mode(GENERIC_WRITE.0 | DELETE.0)
        .share_mode(0)
        .create_new(true)
        .open(path)?;
    if let Err(error) = write_content(&mut file, bytes) {
        let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
        if let Err(cleanup_error) = unsafe {
            SetFileInformationByHandle(
                HANDLE(file.as_raw_handle()),
                FileDispositionInfo,
                (&disposition as *const FILE_DISPOSITION_INFO).cast(),
                std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
            )
        } {
            return Err(io::Error::new(
                error.kind(),
                format!("{error}; incomplete new screenshot remains at {} because deletion failed: {cleanup_error}", path.display()),
            ));
        }
        return Err(error);
    }
    Ok(None)
}

fn write_existing(path: &Path, bytes: &[u8]) -> io::Result<Option<String>> {
    write_existing_with(path, bytes, write_content, restore_content)
}

fn write_existing_with(
    path: &Path,
    bytes: &[u8],
    write_content: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    restore_content: impl FnOnce(&mut File, &Path, u64) -> io::Result<()>,
) -> io::Result<Option<String>> {
    let expected_id = file_identity(path)?;
    let mut target = OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(FILE_SHARE_READ.0)
        .open(path)?;
    let mut target_info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe {
        GetFileInformationByHandle(HANDLE(target.as_raw_handle()), &mut target_info)?;
    }
    if file_information_identity(target_info) != expected_id {
        return Err(io::Error::other(
            "screenshot destination changed before it could be locked",
        ));
    }

    let stage_dir = create_backup_directory()?;
    let backup_path = stage_dir.join("original");
    let encrypted = target_info.dwFileAttributes & FILE_ATTRIBUTE_ENCRYPTED.0 != 0;
    let expected_length =
        (u64::from(target_info.nFileSizeHigh) << 32) | u64::from(target_info.nFileSizeLow);
    let backup_result = backup_contents(
        &mut target,
        &backup_path,
        &stage_dir,
        encrypted,
        expected_length,
        |path| {
            unsafe { EncryptFileW(PCWSTR(wide(path).as_ptr())) }.map_err(|error| {
                io::Error::other(format!(
                    "could not encrypt the recovery copy of an EFS file: {error}"
                ))
            })
        },
    );
    if let Err(error) = backup_result {
        return Err(cleanup_stage_error(error, &stage_dir));
    }

    if let Err(error) = write_content(&mut target, bytes) {
        return match restore_content(&mut target, &backup_path, expected_length) {
            Ok(()) => match fs::remove_file(&backup_path) {
                Ok(()) => Err(cleanup_stage_error(error, &stage_dir)),
                Err(cleanup_error) => Err(io::Error::new(
                    error.kind(),
                    format!("{error}; original content was restored, but its backup remains at {}: {cleanup_error}", backup_path.display()),
                )),
            },
            Err(restore_error) => Err(io::Error::new(
                error.kind(),
                format!("{error}; restoring the original failed: {restore_error}; original content remains at {}", backup_path.display()),
            )),
        };
    }

    if let Err(error) = fs::remove_file(&backup_path) {
        return Ok(Some(format!(
            "Screenshot saved successfully, but the previous file is retained at {}: {error}",
            backup_path.display()
        )));
    }
    Ok(cleanup_completed_save(&stage_dir))
}

fn cleanup_stage_error(error: io::Error, stage_dir: &Path) -> io::Error {
    match fs::remove_dir_all(stage_dir) {
        Ok(()) => error,
        Err(cleanup_error) => io::Error::new(
            error.kind(),
            format!(
                "{error}; incomplete recovery data retained at {} because cleanup failed: {cleanup_error}",
                stage_dir.display()
            ),
        ),
    }
}

fn cleanup_completed_save(stage_dir: &Path) -> Option<String> {
    fs::remove_dir_all(stage_dir).err().map(|error| {
        format!(
            "Screenshot saved successfully, but staging data remains at {}: {error}",
            stage_dir.display()
        )
    })
}

fn write_content(file: &mut File, bytes: &[u8]) -> io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn restore_content(file: &mut File, backup_path: &Path, expected_length: u64) -> io::Result<()> {
    let mut backup = File::open(backup_path)?;
    if backup.metadata()?.len() != expected_length {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "recovery backup length does not match the original screenshot",
        ));
    }
    restore_content_from_reader(file, &mut backup, expected_length)
}

fn restore_content_from_reader(
    file: &mut File,
    backup: &mut impl Read,
    expected_length: u64,
) -> io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    copy_expected_length(backup, file, expected_length)?;
    if file.metadata()?.len() != expected_length {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "restored screenshot length does not match the original",
        ));
    }
    file.sync_all()
}

fn backup_contents(
    target: &mut File,
    backup_path: &Path,
    stage_dir: &Path,
    encrypted: bool,
    expected_length: u64,
    encrypt_backup: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let backup = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(backup_path)?;
    drop(backup);
    if encrypted {
        encrypt_backup(backup_path)?;
        let backup = File::open(backup_path)?;
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        unsafe { GetFileInformationByHandle(HANDLE(backup.as_raw_handle()), &mut info)? };
        if info.dwFileAttributes & FILE_ATTRIBUTE_ENCRYPTED.0 == 0 {
            return Err(io::Error::other(
                "encrypted screenshot backup is not encrypted",
            ));
        }
    }
    copy_target_to_backup(target, backup_path, stage_dir, expected_length)
}

fn copy_target_to_backup(
    target: &mut File,
    backup_path: &Path,
    stage_dir: &Path,
    expected_length: u64,
) -> io::Result<()> {
    verify_backup_security(stage_dir, backup_path)?;
    let mut backup = OpenOptions::new().write(true).open(backup_path)?;
    target.seek(SeekFrom::Start(0))?;
    let copied = copy_expected_length(target, &mut backup, expected_length)?;
    backup.sync_all()?;
    if copied != backup.metadata()?.len() || target.metadata()?.len() != expected_length {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "original screenshot size changed while making its recovery copy",
        ));
    }
    Ok(())
}

fn copy_expected_length(
    source: &mut impl Read,
    destination: &mut impl Write,
    expected_length: u64,
) -> io::Result<u64> {
    let copied = io::copy(source, destination)?;
    if copied != expected_length {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("recovery copy contains {copied} of {expected_length} expected bytes"),
        ));
    }
    Ok(copied)
}

fn acl_bytes(acl: *const ACL) -> io::Result<Vec<u8>> {
    let mut info = ACL_SIZE_INFORMATION::default();
    unsafe {
        GetAclInformation(
            acl,
            (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
            std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
        .map_err(io::Error::other)?;
        Ok(std::slice::from_raw_parts(acl.cast::<u8>(), info.AclBytesInUse as usize).to_vec())
    }
}

fn read_dacl(path: &Path) -> io::Result<(Vec<u8>, bool)> {
    let path = wide(path);
    let mut needed = 0;
    unsafe {
        let _ = GetFileSecurityW(
            PCWSTR(path.as_ptr()),
            DACL_SECURITY_INFORMATION.0,
            None,
            0,
            &mut needed,
        );
    }
    if needed == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut descriptor =
        vec![0usize; needed.div_ceil(std::mem::size_of::<usize>() as u32) as usize];
    unsafe {
        GetFileSecurityW(
            PCWSTR(path.as_ptr()),
            DACL_SECURITY_INFORMATION.0,
            Some(PSECURITY_DESCRIPTOR(descriptor.as_mut_ptr().cast())),
            (descriptor.len() * std::mem::size_of::<usize>()) as u32,
            &mut needed,
        )
        .ok()
        .map_err(io::Error::other)?;
        let descriptor = PSECURITY_DESCRIPTOR(descriptor.as_mut_ptr().cast());
        let mut present = false.into();
        let mut defaulted = false.into();
        let mut acl = std::ptr::null_mut();
        GetSecurityDescriptorDacl(descriptor, &mut present, &mut acl, &mut defaulted)
            .map_err(io::Error::other)?;
        if !present.as_bool() || acl.is_null() {
            return Err(io::Error::other("backup object has no explicit DACL"));
        }
        let mut control = 0u16;
        let mut revision = 0;
        GetSecurityDescriptorControl(descriptor, &mut control, &mut revision)
            .map_err(io::Error::other)?;
        Ok((acl_bytes(acl)?, control & SE_DACL_PROTECTED.0 != 0))
    }
}

fn without_inherited_marker(mut acl: Vec<u8>) -> io::Result<Vec<u8>> {
    if acl.len() < 8 {
        return Err(io::Error::other("backup DACL is truncated"));
    }
    let count = u16::from_le_bytes([acl[4], acl[5]]);
    let mut offset = 8;
    for _ in 0..count {
        if offset + 4 > acl.len() {
            return Err(io::Error::other("backup DACL contains a truncated ACE"));
        }
        let size = u16::from_le_bytes([acl[offset + 2], acl[offset + 3]]) as usize;
        if size < 4 || offset + size > acl.len() {
            return Err(io::Error::other("backup DACL contains an invalid ACE size"));
        }
        let inherited_flags = (OBJECT_INHERIT_ACE.0
            | CONTAINER_INHERIT_ACE.0
            | INHERIT_NO_PROPAGATE.0
            | INHERIT_ONLY_ACE.0
            | INHERITED_ACE.0) as u8;
        acl[offset + 1] &= !inherited_flags;
        offset += size;
    }
    acl[2..4].copy_from_slice(&(offset as u16).to_le_bytes());
    Ok(acl)
}

fn verify_backup_directory(path: &Path, expected: &[u8]) -> io::Result<()> {
    let (actual, protected) = read_dacl(path)?;
    let actual_normalized = without_inherited_marker(actual.clone())?;
    let expected_normalized = without_inherited_marker(expected.to_vec())?;
    if !protected || actual_normalized != expected_normalized {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "filesystem did not enforce the private backup directory DACL",
        ));
    }
    Ok(())
}

fn verify_backup_security(directory: &Path, backup: &Path) -> io::Result<()> {
    let (directory_acl, protected) = read_dacl(directory)?;
    if !protected {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "backup directory DACL is not protected",
        ));
    }
    let (backup_acl, _) = read_dacl(backup)?;
    let backup_acl = without_inherited_marker(backup_acl)?;
    let directory_acl = without_inherited_marker(directory_acl)?;
    if backup_acl != directory_acl {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "backup file did not inherit the protected directory DACL",
        ));
    }
    Ok(())
}

fn create_backup_directory() -> io::Result<PathBuf> {
    let parent = std::env::temp_dir();
    let mut user_sid_storage = vec![0usize; 128];
    let mut system_sid_storage = [0usize; 16];
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }?;
    let token_result = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(user_sid_storage.as_mut_ptr().cast()),
            (user_sid_storage.len() * std::mem::size_of::<usize>()) as u32,
            &mut 0,
        )
    };
    let _ = unsafe { CloseHandle(token) };
    token_result?;
    let user = unsafe { &*(user_sid_storage.as_ptr().cast::<TOKEN_USER>()) };

    let mut system_sid_size = (system_sid_storage.len() * std::mem::size_of::<usize>()) as u32;
    unsafe {
        CreateWellKnownSid(
            WinLocalSystemSid,
            None,
            Some(PSID(system_sid_storage.as_mut_ptr().cast())),
            &mut system_sid_size,
        )
    }?;

    let mut acl_storage = [0usize; 128];
    let acl = acl_storage.as_mut_ptr().cast::<ACL>();
    unsafe {
        InitializeAcl(
            acl,
            std::mem::size_of_val(&acl_storage) as u32,
            ACL_REVISION,
        )?;
        AddAccessAllowedAceEx(
            acl,
            ACL_REVISION,
            OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
            FILE_ALL_ACCESS.0,
            user.User.Sid,
        )?;
        AddAccessAllowedAceEx(
            acl,
            ACL_REVISION,
            OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
            FILE_ALL_ACCESS.0,
            PSID(system_sid_storage.as_mut_ptr().cast()),
        )?;
    }
    let expected_dacl = acl_bytes(acl)?;

    let mut descriptor = SECURITY_DESCRIPTOR::default();
    unsafe {
        InitializeSecurityDescriptor(
            PSECURITY_DESCRIPTOR((&mut descriptor as *mut SECURITY_DESCRIPTOR).cast()),
            1,
        )?;
        SetSecurityDescriptorDacl(
            PSECURITY_DESCRIPTOR((&mut descriptor as *mut SECURITY_DESCRIPTOR).cast()),
            true,
            Some(acl),
            false,
        )?;
        SetSecurityDescriptorControl(
            PSECURITY_DESCRIPTOR((&mut descriptor as *mut SECURITY_DESCRIPTOR).cast()),
            SE_DACL_PROTECTED,
            SE_DACL_PROTECTED,
        )?;
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: (&mut descriptor as *mut SECURITY_DESCRIPTOR).cast(),
        bInheritHandle: false.into(),
    };

    loop {
        let id = NEXT_STAGE_ID.fetch_add(1, Ordering::Relaxed);
        let directory = parent.join(format!("poratake-save-{}-{id}", std::process::id()));
        match unsafe {
            CreateDirectoryW(
                PCWSTR(wide(&directory).as_ptr()),
                Some(&attributes as *const SECURITY_ATTRIBUTES),
            )
        } {
            Ok(()) => {
                if let Err(error) = verify_backup_directory(&directory, &expected_dacl) {
                    return Err(match fs::remove_dir(&directory) {
                        Ok(()) => error,
                        Err(cleanup_error) => io::Error::new(
                            error.kind(),
                            format!(
                                "{error}; unverified staging directory retained at {}: {cleanup_error}",
                                directory.display()
                            ),
                        ),
                    });
                }
                return Ok(directory);
            }
            Err(error) if error.code() == HRESULT(0x800700B7u32 as i32) => continue,
            Err(error) => return Err(io::Error::other(error)),
        }
    }
}

fn file_identity(path: &Path) -> io::Result<(u32, u64)> {
    let file = OpenOptions::new().read(true).open(path)?;
    file_identity_from_handle(&file)
}

fn file_identity_from_handle(file: &File) -> io::Result<(u32, u64)> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info)? };
    Ok(file_information_identity(info))
}

fn file_information_identity(info: BY_HANDLE_FILE_INFORMATION) -> (u32, u64) {
    (
        info.dwVolumeSerialNumber,
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    )
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::io::Read;
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Security::{
        AddMandatoryAce, CreateWellKnownSid, GetFileSecurityW, InitializeAcl,
        InitializeSecurityDescriptor, SetFileSecurityW, SetSecurityDescriptorSacl, WinLowLabelSid,
        ACL, ACL_REVISION, DACL_SECURITY_INFORMATION, GROUP_SECURITY_INFORMATION,
        LABEL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSID, SECURITY_DESCRIPTOR,
    };
    use windows::Win32::Storage::FileSystem::{
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    fn file_security(path: &Path) -> Vec<u8> {
        let path = wide(path);
        let information = DACL_SECURITY_INFORMATION.0
            | OWNER_SECURITY_INFORMATION.0
            | GROUP_SECURITY_INFORMATION.0
            | LABEL_SECURITY_INFORMATION.0;
        let mut needed = 0;
        unsafe {
            let _ = GetFileSecurityW(PCWSTR(path.as_ptr()), information, None, 0, &mut needed);
        }
        let mut storage =
            vec![0usize; needed.div_ceil(std::mem::size_of::<usize>() as u32) as usize];
        unsafe {
            GetFileSecurityW(
                PCWSTR(path.as_ptr()),
                information,
                Some(PSECURITY_DESCRIPTOR(storage.as_mut_ptr().cast())),
                (storage.len() * std::mem::size_of::<usize>()) as u32,
                &mut needed,
            )
            .ok()
            .unwrap();
            std::slice::from_raw_parts(storage.as_ptr().cast::<u8>(), needed as usize).to_vec()
        }
    }

    fn copy_file_dacl(source: &Path, destination: &Path) {
        let source = wide(source);
        let mut required = 0;
        unsafe {
            let _ = GetFileSecurityW(
                PCWSTR(source.as_ptr()),
                DACL_SECURITY_INFORMATION.0,
                None,
                0,
                &mut required,
            );
        }
        let mut descriptor =
            vec![0usize; required.div_ceil(std::mem::size_of::<usize>() as u32) as usize];
        unsafe {
            GetFileSecurityW(
                PCWSTR(source.as_ptr()),
                DACL_SECURITY_INFORMATION.0,
                Some(PSECURITY_DESCRIPTOR(descriptor.as_mut_ptr().cast())),
                (descriptor.len() * std::mem::size_of::<usize>()) as u32,
                &mut required,
            )
            .ok()
            .unwrap();
            SetFileSecurityW(
                PCWSTR(wide(destination).as_ptr()),
                DACL_SECURITY_INFORMATION,
                PSECURITY_DESCRIPTOR(descriptor.as_mut_ptr().cast()),
            )
            .ok()
            .unwrap();
        }
    }

    fn set_low_integrity_label(path: &Path) {
        let mut sid_storage = [0usize; 16];
        let mut sid_size = (sid_storage.len() * std::mem::size_of::<usize>()) as u32;
        unsafe {
            CreateWellKnownSid(
                WinLowLabelSid,
                None,
                Some(PSID(sid_storage.as_mut_ptr().cast())),
                &mut sid_size,
            )
            .unwrap();
        }
        let mut acl_storage = [0usize; 32];
        let acl = acl_storage.as_mut_ptr().cast::<ACL>();
        unsafe {
            InitializeAcl(
                acl,
                std::mem::size_of_val(&acl_storage) as u32,
                ACL_REVISION,
            )
            .unwrap();
            AddMandatoryAce(
                acl,
                ACL_REVISION,
                Default::default(),
                1,
                PSID(sid_storage.as_mut_ptr().cast()),
            )
            .unwrap();
        }
        let mut descriptor = SECURITY_DESCRIPTOR::default();
        unsafe {
            InitializeSecurityDescriptor(
                PSECURITY_DESCRIPTOR((&mut descriptor as *mut SECURITY_DESCRIPTOR).cast()),
                1,
            )
            .unwrap();
            SetSecurityDescriptorSacl(
                PSECURITY_DESCRIPTOR((&mut descriptor as *mut SECURITY_DESCRIPTOR).cast()),
                true,
                Some(acl),
                false,
            )
            .unwrap();
            SetFileSecurityW(
                PCWSTR(wide(path).as_ptr()),
                LABEL_SECURITY_INFORMATION,
                PSECURITY_DESCRIPTOR((&mut descriptor as *mut SECURITY_DESCRIPTOR).cast()),
            )
            .ok()
            .unwrap();
        }
    }

    #[test]
    fn overwrite_keeps_same_file_and_security_descriptor() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.png");
        fs::write(&path, b"original capture").unwrap();
        set_low_integrity_label(&path);
        let original_id = file_identity(&path).unwrap();
        let original_security = file_security(&path);

        assert_eq!(write(&path, b"replacement capture").unwrap(), None);

        assert_eq!(fs::read(&path).unwrap(), b"replacement capture");
        assert_eq!(file_identity(&path).unwrap(), original_id);
        assert_eq!(file_security(&path), original_security);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn new_file_inherits_security_and_refuses_existing_destination() {
        let directory = tempfile::tempdir().unwrap();
        let reference = directory.path().join("reference.png");
        let path = directory.path().join("capture.png");
        fs::write(&reference, b"reference").unwrap();

        assert_eq!(write(&path, b"new capture").unwrap(), None);
        assert_eq!(file_security(&path), file_security(&reference));
        assert!(write_new(&path, b"racing output").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"new capture");
    }

    #[test]
    fn partial_write_failure_restores_original_on_same_handle() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.png");
        fs::write(&path, b"original capture").unwrap();
        let original_id = file_identity(&path).unwrap();
        let original_security = file_security(&path);

        let result = write_existing_with(
            &path,
            b"replacement capture",
            |file, bytes| {
                file.set_len(0)?;
                file.seek(SeekFrom::Start(0))?;
                file.write_all(&bytes[..5])?;
                Err(io::Error::other("injected partial write"))
            },
            restore_content,
        );

        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original capture");
        assert_eq!(file_identity(&path).unwrap(), original_id);
        assert_eq!(file_security(&path), original_security);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn efs_backup_failure_aborts_before_target_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.png");
        fs::write(&path, b"original capture").unwrap();
        let source_length = fs::metadata(&path).unwrap().len();
        let mut target = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        let stage_dir = create_backup_directory().unwrap();
        let backup = stage_dir.join("original");

        let result = backup_contents(
            &mut target,
            &backup,
            &stage_dir,
            true,
            source_length,
            |_| Err(io::Error::other("injected EFS setup failure")),
        );

        assert!(result.is_err());
        target.seek(SeekFrom::Start(0)).unwrap();
        let mut contents = Vec::new();
        target.read_to_end(&mut contents).unwrap();
        assert_eq!(contents, b"original capture");
        assert_eq!(fs::metadata(&backup).unwrap().len(), 0);
        fs::remove_dir_all(stage_dir).unwrap();
    }

    #[test]
    fn efs_backup_without_encrypted_attribute_aborts_before_source_read() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        fs::write(&source, b"original capture").unwrap();
        let source_length = fs::metadata(&source).unwrap().len();
        let mut write_only_source = OpenOptions::new().write(true).open(&source).unwrap();
        let stage_dir = create_backup_directory().unwrap();
        let backup = stage_dir.join("original");

        let result = backup_contents(
            &mut write_only_source,
            &backup,
            &stage_dir,
            true,
            source_length,
            |_| Ok(()),
        );

        assert!(result
            .unwrap_err()
            .to_string()
            .contains("backup is not encrypted"));
        assert_eq!(fs::read(&source).unwrap(), b"original capture");
        assert_eq!(fs::metadata(&backup).unwrap().len(), 0);
        fs::remove_dir_all(stage_dir).unwrap();
    }

    #[test]
    fn early_eof_is_rejected_before_target_mutation() {
        let mut source = Cursor::new(b"short source".to_vec());
        let mut backup = Vec::new();

        let result = copy_expected_length(&mut source, &mut backup, 20);

        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(backup, b"short source");
    }

    #[test]
    fn backup_copy_rejects_early_eof_before_target_mutation() {
        let mut source = Cursor::new(b"short source".to_vec());
        let mut backup = Vec::new();

        let result = copy_expected_length(&mut source, &mut backup, 20);

        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(backup, b"short source");
    }

    #[test]
    fn short_restore_read_is_rejected_and_backup_is_retained() {
        let directory = tempfile::tempdir().unwrap();
        let target_path = directory.path().join("capture.png");
        let backup_path = directory.path().join("original");
        fs::write(&target_path, b"partial target").unwrap();
        fs::write(&backup_path, b"original screenshot bytes").unwrap();
        let mut target = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&target_path)
            .unwrap();
        let mut short_reader = Cursor::new(b"short".to_vec());

        let expected_length = fs::metadata(&backup_path).unwrap().len();
        let result = restore_content_from_reader(&mut target, &mut short_reader, expected_length);

        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
        drop(target);
        assert_eq!(
            fs::read(&backup_path).unwrap(),
            b"original screenshot bytes"
        );
        assert_eq!(fs::read(&target_path).unwrap(), b"short");
    }

    #[test]
    fn mismatched_backup_dacl_is_rejected_before_reading_source() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        fs::write(&source, b"original capture").unwrap();
        let stage_dir = create_backup_directory().unwrap();
        let backup = stage_dir.join("original");
        fs::write(&backup, b"").unwrap();
        copy_file_dacl(&source, &backup);
        let mut write_only_source = OpenOptions::new().write(true).open(&source).unwrap();

        let result = copy_target_to_backup(
            &mut write_only_source,
            &backup,
            &stage_dir,
            fs::metadata(&source).unwrap().len(),
        );

        assert!(result
            .unwrap_err()
            .to_string()
            .contains("did not inherit the protected directory DACL"));
        assert_eq!(fs::read(&source).unwrap(), b"original capture");
        assert_eq!(fs::metadata(&backup).unwrap().len(), 0);
        fs::remove_dir_all(stage_dir).unwrap();
    }

    #[test]
    fn failed_restore_retains_original_backup_and_reports_path() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.png");
        fs::write(&path, b"original capture").unwrap();

        let result = write_existing_with(
            &path,
            b"replacement capture",
            |file, bytes| {
                file.set_len(0)?;
                file.seek(SeekFrom::Start(0))?;
                file.write_all(&bytes[..5])?;
                Err(io::Error::other("injected write failure"))
            },
            |_, _, _| Err(io::Error::other("injected restore failure")),
        );

        let message = result.unwrap_err().to_string();
        let backup_path = PathBuf::from(
            message
                .split("original content remains at ")
                .last()
                .unwrap(),
        );
        assert!(message.contains(&backup_path.display().to_string()));
        assert_eq!(fs::read(&path).unwrap(), b"repla");
        assert_eq!(fs::read(&backup_path).unwrap(), b"original capture");
        fs::remove_dir_all(backup_path.parent().unwrap()).unwrap();
    }

    #[test]
    fn compatible_reader_can_remain_open_during_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.png");
        fs::write(&path, b"original capture").unwrap();
        let mut reader = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0 | FILE_SHARE_DELETE.0)
            .open(&path)
            .unwrap();

        assert_eq!(write(&path, b"replacement capture").unwrap(), None);

        reader.seek(SeekFrom::Start(0)).unwrap();
        let mut contents = Vec::new();
        reader.read_to_end(&mut contents).unwrap();
        assert_eq!(contents, b"replacement capture");
    }

    #[test]
    fn incompatible_writer_fails_before_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.png");
        fs::write(&path, b"original capture").unwrap();
        let writer = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(FILE_SHARE_READ.0)
            .open(&path)
            .unwrap();

        assert!(write(&path, b"replacement capture").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original capture");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        drop(writer);
    }

    #[test]
    fn new_file_write_failure_removes_partial_target() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.png");

        let result = write_new_with(&path, b"replacement capture", |file, bytes| {
            file.write_all(&bytes[..5])?;
            Err(io::Error::other("injected write failure"))
        });

        assert!(result.is_err());
        assert!(!path.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}
