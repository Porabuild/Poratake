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
            if !arg.starts_with("--") && Path::new(arg).exists() {
                paths.push(arg.clone());
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
        let Ok(listener) = std::os::unix::net::UnixListener::bind(&socket_path) else {
            return (tx, rx);
        };
        let _ = std::thread::Builder::new()
            .name("single-instance-ipc".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let mut stream = stream;
                    let mut payload = String::new();
                    if stream.read_to_string(&mut payload).is_ok() {
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
                        let _ = file.read_to_string(&mut payload);
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
    let pipe = pipe_name_for(dir);
    let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(pipe) else {
        return false;
    };
    let Ok(payload) = serde_json::to_vec(command) else {
        return false;
    };
    file.write_all(&payload).is_ok()
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
}
