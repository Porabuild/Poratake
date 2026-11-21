#[cfg(windows)]
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{Read as _, Write as _};
use std::path::Path;
#[cfg(not(windows))]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
#[cfg(windows)]
use windows::core::HSTRING;
#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
#[cfg(windows)]
use windows::Win32::System::Threading::CreateMutexW;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstanceCommand {
    Activate,
    OpenPaths(Vec<String>),
    Intent { id: String, extra: Option<String> },
}

pub fn decode_file_url(raw: &str) -> Option<String> {
    let stripped = raw.strip_prefix("file://")?;
    let decoded = percent_decode(stripped)?;
    let (host, path) = split_file_authority(&decoded)?;
    if path.split('/').any(|segment| segment == "..") {
        return None;
    }
    if cfg!(windows) {
        return windows_file_path(host, path);
    }
    if !host.is_empty() {
        return None;
    }
    if path.starts_with('/') {
        return Some(path.to_string());
    }
    (!path.is_empty()).then(|| format!("/{path}"))
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(decoded) = u8::from_str_radix(hex, 16) {
                    out.push(decoded);
                    index += 3;
                    continue;
                }
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out).ok()
}

fn split_file_authority(decoded: &str) -> Option<(&str, &str)> {
    if decoded.starts_with('/') {
        return Some(("", decoded));
    }
    let (host, path) = decoded.split_once('/')?;
    if is_windows_drive(host) {
        return Some(("", decoded));
    }
    if host.eq_ignore_ascii_case("localhost") {
        return Some(("", path));
    }
    remote_file_host(host).then_some((host, path))
}

fn is_windows_drive(host: &str) -> bool {
    let bytes = host.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn remote_file_host(host: &str) -> bool {
    !host.is_empty()
        && !host.contains("..")
        && !host.starts_with('.')
        && !host.ends_with('.')
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.')
}

const MAX_IPC_BYTES: u64 = 16 * 1024;

#[cfg(any(windows, test))]
fn named_pipe_should_retry(raw_os_error: Option<i32>, attempt: u32) -> bool {
    raw_os_error == Some(231) && attempt < 2
}

#[cfg(not(windows))]
fn bind_private_socket(path: &Path) -> std::io::Result<std::os::unix::net::UnixListener> {
    struct RestoreUmask(libc::mode_t);
    impl Drop for RestoreUmask {
        fn drop(&mut self) {
            unsafe {
                libc::umask(self.0);
            }
        }
    }

    let previous = unsafe { libc::umask(0o077) };
    let _restore = RestoreUmask(previous);
    let listener = std::os::unix::net::UnixListener::bind(path)?;
    use std::os::unix::fs::PermissionsExt as _;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o600);
    if std::fs::set_permissions(path, permissions).is_err() {
        drop(listener);
        let _ = std::fs::remove_file(path);
        return Err(std::io::Error::other("ipc socket is not private"));
    }
    let mode = std::fs::metadata(path)?.permissions().mode() & 0o777;
    if mode != 0o600 {
        drop(listener);
        let _ = std::fs::remove_file(path);
        return Err(std::io::Error::other("ipc socket is not private"));
    }
    Ok(listener)
}

fn windows_file_path(host: &str, path: &str) -> Option<String> {
    if !host.is_empty() {
        return Some(format!("\\\\{host}\\{}", path.replace('/', "\\")));
    }
    let trimmed = path.strip_prefix('/').unwrap_or(path);
    if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':' {
        return Some(trimmed.replace('/', "\\"));
    }
    trimmed.starts_with('/').then(|| trimmed.replace('/', "\\"))
}

pub fn resolve_cli_path(arg: &str) -> Option<String> {
    if arg.starts_with("--") {
        return None;
    }
    let candidate = decode_file_url(arg).unwrap_or_else(|| arg.to_string());
    Path::new(&candidate).exists().then_some(candidate)
}

impl InstanceCommand {
    pub fn from_args(args: &[String]) -> Self {
        let mut iter = args.iter().skip(1);
        let Some(first) = iter.next() else {
            return Self::Activate;
        };
        if first == "--intent" {
            if let Some(id) = iter.next() {
                return Self::Intent {
                    id: id.clone(),
                    extra: iter.next().cloned(),
                };
            }
            return Self::Activate;
        }
        let mut paths = Vec::new();
        for arg in std::iter::once(first).chain(iter) {
            if let Some(path) = resolve_cli_path(arg) {
                paths.push(path);
            }
        }
        if paths.is_empty() {
            Self::Activate
        } else {
            Self::OpenPaths(paths)
        }
    }
}

pub fn acquire() -> Option<SingleInstanceGuard> {
    acquire_in(&crate::config::store::config_dir())
}

pub fn notify_running(command: &InstanceCommand) -> bool {
    notify_running_in(&crate::config::store::config_dir(), command)
}

#[cfg(windows)]
pub struct SingleInstanceGuard {
    handle: HANDLE,
    pipe_name: String,
}

#[cfg(not(windows))]
pub struct SingleInstanceGuard {
    path: PathBuf,
    socket_path: PathBuf,
    _file: std::fs::File,
}

#[cfg(windows)]
fn pipe_name_for(dir: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    dir.hash(&mut hasher);
    format!("\\\\.\\pipe\\Poratake-GPUI-{:016x}", hasher.finish())
}

#[cfg(windows)]
fn acquire_in(dir: &Path) -> Option<SingleInstanceGuard> {
    let mut hasher = DefaultHasher::new();
    dir.hash(&mut hasher);
    let name = HSTRING::from(format!("Local\\Poratake-GPUI-{:016x}", hasher.finish()));
    let handle = unsafe { CreateMutexW(None, false, &name) }.ok()?;
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe { CloseHandle(handle) }.ok();
        return None;
    }
    Some(SingleInstanceGuard {
        handle,
        pipe_name: pipe_name_for(dir),
    })
}

#[cfg(not(windows))]
fn acquire_in(dir: &Path) -> Option<SingleInstanceGuard> {
    let path = dir.join("poratake-gpui.lock");
    let socket_path = dir.join("poratake-gpui.sock");
    std::fs::create_dir_all(path.parent()?).ok()?;
    for _ in 0..2 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                writeln!(file, "{}", std::process::id()).ok()?;
                return Some(SingleInstanceGuard {
                    path,
                    socket_path,
                    _file: file,
                });
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
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    unsafe {
        libc::kill(pid, 0) == 0
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

impl SingleInstanceGuard {
    #[cfg(not(windows))]
    pub fn start_listener(
        &self,
    ) -> (
        smol::channel::Sender<InstanceCommand>,
        smol::channel::Receiver<InstanceCommand>,
    ) {
        let (tx, rx) = smol::channel::unbounded();
        let ipc_tx = tx.clone();
        let socket_path = self.socket_path.clone();
        let _ = std::fs::remove_file(&socket_path);
        let Ok(listener) = bind_private_socket(&socket_path) else {
            return (tx, rx);
        };
        let _ = std::thread::Builder::new()
            .name("single-instance-ipc".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let stream = stream;
                    let mut payload = String::new();
                    if stream
                        .take(MAX_IPC_BYTES)
                        .read_to_string(&mut payload)
                        .is_ok()
                    {
                        if let Ok(command) = serde_json::from_str::<InstanceCommand>(&payload) {
                            if ipc_tx.send_blocking(command).is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        (tx, rx)
    }

    #[cfg(windows)]
    pub fn start_listener(
        &self,
    ) -> (
        smol::channel::Sender<InstanceCommand>,
        smol::channel::Receiver<InstanceCommand>,
    ) {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::io::FromRawHandle;
        use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows::Win32::Storage::FileSystem::PIPE_ACCESS_INBOUND;
        use windows::Win32::System::Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
            PIPE_TYPE_BYTE, PIPE_WAIT,
        };

        let (tx, rx) = smol::channel::unbounded();
        let ipc_tx = tx.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<()>(1);
        let wide: Vec<u16> = OsStr::new(&self.pipe_name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let _ = std::thread::Builder::new()
            .name("single-instance-ipc".into())
            .spawn(move || {
                let mut ready = Some(ready_tx);
                loop {
                    let handle = unsafe {
                        CreateNamedPipeW(
                            windows::core::PCWSTR(wide.as_ptr()),
                            PIPE_ACCESS_INBOUND,
                            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                            4,
                            0,
                            4096,
                            0,
                            None,
                        )
                    };
                    if let Some(signal) = ready.take() {
                        let _ = signal.send(());
                    }
                    if handle == INVALID_HANDLE_VALUE {
                        break;
                    }
                    let connect_res = unsafe { ConnectNamedPipe(handle, None) };
                    let err = unsafe { GetLastError().0 };
                    let connected = connect_res.is_ok() || err == 535 || err == 232;
                    if connected {
                        let mut file = unsafe { std::fs::File::from_raw_handle(handle.0) };
                        let mut payload = String::new();
                        let _ = (&mut file).take(MAX_IPC_BYTES).read_to_string(&mut payload);
                        unsafe {
                            let _ = DisconnectNamedPipe(handle);
                        }
                        drop(file);
                        if let Ok(command) = serde_json::from_str::<InstanceCommand>(&payload) {
                            if ipc_tx.send_blocking(command).is_err() {
                                break;
                            }
                        }
                    } else {
                        unsafe {
                            let _ = CloseHandle(handle);
                        }
                    }
                }
            });
        let _ = ready_rx.recv();
        (tx, rx)
    }
}

#[cfg(not(windows))]
fn notify_running_in(dir: &Path, command: &InstanceCommand) -> bool {
    let socket_path = dir.join("poratake-gpui.sock");
    let Ok(mut stream) = std::os::unix::net::UnixStream::connect(socket_path) else {
        return false;
    };
    let Ok(payload) = serde_json::to_vec(command) else {
        return false;
    };
    stream.write_all(&payload).is_ok()
}

#[cfg(windows)]
fn notify_running_in(dir: &Path, command: &InstanceCommand) -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::Pipes::WaitNamedPipeW;

    let pipe = pipe_name_for(dir);
    let Ok(payload) = serde_json::to_vec(command) else {
        return false;
    };
    let wide: Vec<u16> = OsStr::new(&pipe)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    for attempt in 0..3 {
        match std::fs::OpenOptions::new().write(true).open(&pipe) {
            Ok(mut file) => return file.write_all(&payload).is_ok(),
            Err(error) if named_pipe_should_retry(error.raw_os_error(), attempt) => unsafe {
                let _ = WaitNamedPipeW(windows::core::PCWSTR(wide.as_ptr()), 500);
            },
            Err(_) => return false,
        }
    }
    false
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
        let _ = std::fs::remove_file(&self.socket_path);
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_guard_for_the_same_profile_is_rejected() {
        let dir = tempfile::tempdir().expect("temp dir");
        let first = acquire_in(dir.path()).expect("first guard");
        assert!(acquire_in(dir.path()).is_none());
        drop(first);
        assert!(acquire_in(dir.path()).is_some());
    }

    #[test]
    fn a_guard_for_another_profile_is_not_rejected() {
        let first_dir = tempfile::tempdir().expect("first dir");
        let second_dir = tempfile::tempdir().expect("second dir");
        let _first = acquire_in(first_dir.path()).expect("first guard");
        assert!(acquire_in(second_dir.path()).is_some());
    }

    #[test]
    fn forwards_open_paths_to_running_instance_over_ipc() {
        let dir = tempfile::tempdir().expect("temp dir");
        let guard = acquire_in(dir.path()).expect("guard");
        let (_tx, rx) = guard.start_listener();
        let command = InstanceCommand::OpenPaths(vec!["/tmp/sample.png".into()]);
        assert!(notify_running_in(dir.path(), &command));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(dir.path().join("poratake-gpui.sock"))
                .expect("ipc socket")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = done_tx.send(rx.recv_blocking().ok());
        });
        let received = done_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .ok()
            .flatten()
            .expect("ipc command within 5s");
        assert_eq!(received, command);
    }

    #[test]
    fn parses_cli_arguments_into_instance_commands() {
        assert_eq!(
            InstanceCommand::from_args(&["poratake".into()]),
            InstanceCommand::Activate
        );
        assert_eq!(
            InstanceCommand::from_args(&[
                "poratake".into(),
                "--intent".into(),
                "open-settings".into(),
                "cloud".into()
            ]),
            InstanceCommand::Intent {
                id: "open-settings".into(),
                extra: Some("cloud".into()),
            }
        );
    }

    #[test]
    fn decodes_file_urls_passed_through_cli_arguments() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("my shot.png");
        std::fs::write(&file, b"png").expect("write file");
        let encoded = format!(
            "file://{}",
            file.to_string_lossy()
                .replace('\\', "/")
                .replace(' ', "%20")
        );
        assert_eq!(
            InstanceCommand::from_args(&["poratake".into(), encoded]),
            InstanceCommand::OpenPaths(vec![file.to_string_lossy().into_owned()])
        );
    }

    #[test]
    fn decodes_file_urls_without_treating_the_host_as_a_relative_path() {
        assert_eq!(decode_file_url("file://localhost../.ssh/id_rsa"), None);
        assert_eq!(decode_file_url("file:///tmp/../secret"), None);
        if cfg!(windows) {
            assert_eq!(
                decode_file_url("file://localhost/C:/tmp/shot.png").as_deref(),
                Some("C:\\tmp\\shot.png")
            );
            assert_eq!(
                decode_file_url("file://server/share/clip.mp4").as_deref(),
                Some("\\\\server\\share\\clip.mp4")
            );
            assert_eq!(
                decode_file_url("file:///C:/Shots/a.png").as_deref(),
                Some("C:\\Shots\\a.png")
            );
        } else {
            assert_eq!(
                decode_file_url("file://localhost/tmp/shot.png").as_deref(),
                Some("/tmp/shot.png")
            );
            assert_eq!(decode_file_url("file://server/share/clip.mp4"), None);
        }
    }

    #[test]
    fn retries_a_busy_named_pipe_only_twice() {
        assert!(named_pipe_should_retry(Some(231), 0));
        assert!(named_pipe_should_retry(Some(231), 1));
        assert!(!named_pipe_should_retry(Some(231), 2));
        assert!(!named_pipe_should_retry(Some(2), 0));
    }
}
