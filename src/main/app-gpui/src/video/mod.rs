pub mod audio;
pub mod auto_zoom;
pub mod composition;
pub mod decoder;
pub mod encoder;
pub mod export;
pub mod music;
pub mod project;
pub mod recorder;
pub mod sidecars;
pub mod transcription;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn command_output(
    command: &mut std::process::Command,
    timeout: std::time::Duration,
) -> std::io::Result<std::process::Output> {
    let output = command_stdout(command, timeout, |mut stdout| {
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
    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                terminate(&mut child);
                let _ = stdout.join();
                let _ = stderr.join();
                return Err(error);
            }
        }
        if std::time::Instant::now() >= deadline {
            terminate(&mut child);
            let _ = stdout.join();
            let _ = stderr.join();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "FFmpeg timed out",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
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
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            #[cfg(target_os = "macos")]
            let packaged = directory
                .parent()
                .map(|contents| contents.join("Resources/binaries/ffmpeg/ffmpeg"));
            #[cfg(target_os = "macos")]
            if let Some(packaged) = packaged.filter(|path| path.is_file()) {
                return packaged;
            }
            #[cfg(target_os = "linux")]
            let packaged = directory.join("binaries/ffmpeg/ffmpeg");
            #[cfg(target_os = "linux")]
            if packaged.is_file() {
                return packaged;
            }
            for ancestor in directory.ancestors() {
                let candidate = ancestor.join("src/main/binaries/ffmpeg/ffmpeg");
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        return std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("app-gpui manifest directory")
            .join("binaries/ffmpeg/ffmpeg");
    }
    #[cfg(target_os = "linux")]
    std::path::PathBuf::from("ffmpeg")
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests {
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
}
