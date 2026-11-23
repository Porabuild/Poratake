pub mod audio;
pub mod audio_tracks;
pub mod auto_zoom;
pub mod composition;
pub mod decoder;
pub mod encoder;
pub mod export;
pub mod frame_pool;
pub mod keyboard_audio;
pub mod music;
pub mod music_retention;
pub mod preview_audio;
pub mod preview_budget;
pub mod project;
pub mod recorder;
pub mod sidecars;
pub mod transcription;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn command_output(
    command: &mut std::process::Command,
    timeout: std::time::Duration,
) -> std::io::Result<std::process::Output> {
    let output = command_stdout(command, timeout, None, |mut stdout| {
        use std::io::Read as _;

        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    })?;
    Ok(std::process::Output {
        status: output.status,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) struct CommandOutput<T> {
    pub status: std::process::ExitStatus,
    pub stdout: T,
    pub stderr: Vec<u8>,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn command_stdout<T, F>(
    command: &mut std::process::Command,
    timeout: std::time::Duration,
    should_cancel: Option<&dyn Fn() -> bool>,
    read_stdout: F,
) -> std::io::Result<CommandOutput<T>>
where
    T: Send + 'static,
    F: FnOnce(std::process::ChildStdout) -> std::io::Result<T> + Send + 'static,
{
    use std::io::Read as _;
    use std::os::unix::process::CommandExt as _;
    use std::process::Stdio;

    command.process_group(0);
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("FFmpeg stdout was unavailable"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| std::io::Error::other("FFmpeg stderr was unavailable"))?;
    let stdout = match std::thread::Builder::new()
        .name("ffmpeg-stdout-reader".into())
        .spawn(move || read_stdout(stdout))
    {
        Ok(reader) => reader,
        Err(error) => {
            terminate(&mut child);
            return Err(error);
        }
    };
    let stderr = match std::thread::Builder::new()
        .name("ffmpeg-stderr-reader".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).map(|_| bytes)
        }) {
        Ok(reader) => reader,
        Err(error) => {
            terminate(&mut child);
            let _ = stdout.join();
            return Err(error);
        }
    };
    let status = match wait_for_command(&mut child, timeout, should_cancel) {
        Ok(status) => status,
        Err(error) => {
            terminate(&mut child);
            let _ = stdout.join();
            let _ = stderr.join();
            return Err(error);
        }
    };
    let stdout = stdout
        .join()
        .map_err(|_| std::io::Error::other("FFmpeg stdout reader panicked"))??;
    let stderr = stderr
        .join()
        .map_err(|_| std::io::Error::other("FFmpeg stderr reader panicked"))??;
    Ok(CommandOutput {
        status,
        stdout,
        stderr,
    })
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn wait_for_command(
    child: &mut std::process::Child,
    timeout: std::time::Duration,
    should_cancel: Option<&dyn Fn() -> bool>,
) -> std::io::Result<std::process::ExitStatus> {
    use std::io::{Error, ErrorKind};
    use std::time::{Duration, Instant};

    let started = Instant::now();
    #[cfg(target_os = "macos")]
    let mut exit = should_cancel
        .is_none()
        .then(|| ProcessExit::register(child.id()))
        .flatten();
    loop {
        if should_cancel.is_some_and(|cancel| cancel()) {
            return Err(Error::new(
                ErrorKind::Interrupted,
                "the export was cancelled",
            ));
        }
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(Error::new(ErrorKind::TimedOut, "FFmpeg timed out"));
        }
        #[cfg(target_os = "macos")]
        if let Some(watch) = &exit {
            match watch.wait(remaining) {
                Ok(true) => return child.wait(),
                Ok(false) => continue,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => exit = None,
            }
        }
        std::thread::sleep(remaining.min(Duration::from_millis(10)));
    }
}

#[cfg(target_os = "macos")]
struct ProcessExit(std::os::fd::OwnedFd);

#[cfg(target_os = "macos")]
impl ProcessExit {
    fn register(pid: u32) -> Option<Self> {
        use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
        use std::ptr::{null, null_mut};

        let raw = unsafe { libc::kqueue() };
        if raw == -1 {
            return None;
        }
        let queue = unsafe { OwnedFd::from_raw_fd(raw) };
        let event = libc::kevent {
            ident: pid as usize,
            filter: libc::EVFILT_PROC,
            flags: libc::EV_ADD | libc::EV_ONESHOT,
            fflags: libc::NOTE_EXIT,
            data: 0,
            udata: null_mut(),
        };
        let result = unsafe { libc::kevent(queue.as_raw_fd(), &event, 1, null_mut(), 0, null()) };
        (result == 0).then_some(Self(queue))
    }

    fn wait(&self, timeout: std::time::Duration) -> std::io::Result<bool> {
        use std::os::fd::AsRawFd as _;

        let limit = libc::timespec {
            tv_sec: timeout.as_secs().try_into().unwrap_or(libc::time_t::MAX),
            tv_nsec: timeout.subsec_nanos().into(),
        };
        let mut event = std::mem::MaybeUninit::<libc::kevent>::uninit();
        let count = unsafe {
            libc::kevent(
                self.0.as_raw_fd(),
                std::ptr::null(),
                0,
                event.as_mut_ptr(),
                1,
                &limit,
            )
        };
        if count < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if count == 0 {
            return Ok(false);
        }
        let event = unsafe { event.assume_init() };
        if event.flags & libc::EV_ERROR != 0 {
            return Err(std::io::Error::from_raw_os_error(event.data as i32));
        }
        Ok(true)
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn terminate(child: &mut std::process::Child) {
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }

    let _ = unsafe { kill(-(child.id() as i32), 9) };
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn ffmpeg_path() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("PORATAKE_FFMPEG_PATH") {
        let path = std::path::PathBuf::from(path);
        if path.is_file() {
            return path;
        }
    }
    crate::util::resources::native_binary_path("binaries/ffmpeg", "ffmpeg").unwrap_or_else(|| {
        #[cfg(target_os = "macos")]
        let fallback = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("app-gpui manifest directory")
            .join("binaries/ffmpeg/ffmpeg");
        #[cfg(target_os = "linux")]
        let fallback = std::path::PathBuf::from("ffmpeg");
        fallback
    })
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests {
    use std::process::Command;
    use std::time::{Duration, Instant};

    #[test]
    fn ffmpeg_process_timeout_stops_the_child() {
        let started = std::time::Instant::now();
        let error = super::command_output(
            std::process::Command::new("sh").args(["-c", "sleep 2"]),
            std::time::Duration::from_millis(20),
        )
        .expect_err("process should time out");

        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn ffmpeg_process_preserves_both_streams_and_nonzero_exit() {
        let output = super::command_output(
            Command::new("sh").args(["-c", "printf output; printf error >&2; exit 7"]),
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(output.stdout, b"output");
        assert_eq!(output.stderr, b"error");
        assert_eq!(output.status.code(), Some(7));
    }

    #[test]
    fn cancelling_ffmpeg_stops_the_process_group() {
        let started = Instant::now();
        let error = super::command_stdout(
            Command::new("sh").args(["-c", "sleep 2 & wait"]),
            Duration::from_secs(2),
            Some(&|| started.elapsed() >= Duration::from_millis(20)),
            |_| Ok(()),
        )
        .err()
        .expect("process should be cancelled");
        assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_command_wait_times_out_and_signals_child_exit() {
        use std::io::Write as _;
        use std::process::Stdio;

        let mut child = Command::new("sh")
            .args(["-c", "read value; exit 7"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let exit = super::ProcessExit::register(child.id()).unwrap();
        assert!(!exit.wait(Duration::from_millis(20)).unwrap());
        child.stdin.take().unwrap().write_all(b"exit\n").unwrap();
        assert!(exit.wait(Duration::from_secs(2)).unwrap());
        assert_eq!(child.wait().unwrap().code(), Some(7));
    }
}
