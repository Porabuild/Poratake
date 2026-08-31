//! Locating the FFmpeg binary and shaping its command lines for everything
//! that records or exports video.
//!
//! The GPUI shell resolves its own copy (`app-gpui/src/video/mod.rs`) with the
//! bundle-relative fallbacks a packaged app needs, and hands the result to the
//! daemon through `PORATAKE_FFMPEG_PATH`; this resolver covers the daemon when
//! it runs standalone (dev sessions) by honoring the same override and then
//! the distribution binary on `PATH`. Recording additionally requires the
//! `libx264` encoder, which distribution builds ship but minimal builds may
//! not — availability is probed, never assumed. The argument builders here
//! are the one definition of the raw-video input, libx264 encode, and quiet
//! logging settings shared by the daemon's recorder, its side tracks, and the
//! editor's export encoder.

use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, Command};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BINARY_NAME: &str = "ffmpeg";

/// The leading arguments every invocation shares: no banner, and errors only
/// on stderr so callers can quote the one line that explains a failure.
pub fn quiet_args() -> Vec<String> {
    vec!["-hide_banner".into(), "-loglevel".into(), "error".into()]
}

/// Raw frame bytes fed to FFmpeg's stdin at a fixed frame rate. `pixel_format`
/// is the layout the caller streams — `rgba` from X11 grabs, `bgra` from
/// composed editor frames.
pub fn raw_video_input_args(pixel_format: &str, width: u32, height: u32, fps: u32) -> Vec<String> {
    vec![
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        pixel_format.into(),
        "-video_size".into(),
        format!("{width}x{height}"),
        "-framerate".into(),
        fps.to_string(),
        "-i".into(),
        "pipe:0".into(),
    ]
}

/// How the H.264 encode is rate-controlled: constant quality for recordings
/// (no bitrate guesswork) and a target bitrate for exports (the quality
/// presets pick one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoRate {
    Crf(u32),
    Bitrate(u32),
}

/// The libx264 encoding settings shared by every video writer: veryfast,
/// square-pixel output, and a fast-start container so playback starts before
/// the file finishes.
pub fn h264_encode_args(rate: VideoRate) -> Vec<String> {
    let (key, value) = match rate {
        VideoRate::Crf(crf) => ("-crf", crf.to_string()),
        VideoRate::Bitrate(bps) => ("-b:v", bps.to_string()),
    };
    vec![
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "veryfast".into(),
        key.into(),
        value,
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-movflags".into(),
        "+faststart".into(),
    ]
}

/// Drains a child's stderr on a background thread, keeping the last
/// non-empty line so failures can quote FFmpeg's own explanation. `on_line`
/// sees every line as it arrives, for callers that also log them live.
pub fn spawn_stderr_tail(
    stderr: ChildStderr,
    mut on_line: impl FnMut(&str) + Send + 'static,
) -> Arc<Mutex<String>> {
    let tail = Arc::new(Mutex::new(String::new()));
    let tail_for_thread = tail.clone();
    let _ = std::thread::Builder::new()
        .name("ffmpeg-stderr-tail".into())
        .spawn(move || {
            use std::io::BufRead;
            for line in std::io::BufReader::new(stderr)
                .lines()
                .map_while(Result::ok)
            {
                if line.is_empty() {
                    continue;
                }
                *tail_for_thread.lock().unwrap() = line.clone();
                on_line(&line);
            }
        });
    tail
}

/// Waits up to `timeout` for a child to exit on its own, killing it past the
/// deadline; returns whether it exited by itself.
pub fn wait_for_exit(child: &mut Child, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => return false,
        }
    }
}

/// Returns the FFmpeg binary to record with, or `None` when none exists.
pub fn resolve() -> Option<PathBuf> {
    resolve_with_override(std::env::var_os("PORATAKE_FFMPEG_PATH").as_deref())
}

/// The override-aware core of [`resolve`], separate so tests can pin the
/// override branch without touching process environment.
pub fn resolve_with_override(override_path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    if let Some(path) = override_path {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|directory| directory.join(BINARY_NAME))
        .find(|candidate| candidate.is_file())
}

/// Whether the binary carries the `libx264` encoder recording needs.
pub fn supports_h264(binary: &Path) -> bool {
    let Ok(output) = Command::new(binary)
        .arg("-hide_banner")
        .arg("-encoders")
        .output()
    else {
        return false;
    };
    output.status.success() && String::from_utf8_lossy(&output.stdout).contains("libx264")
}

/// The binary to record with, provided it can encode H.264. The override is
/// preferred but never disqualifies the distribution binary: a stale
/// `PORATAKE_FFMPEG_PATH` pointing at a build without libx264 must not
/// disable recording when the distro ships a working encoder.
pub fn resolve_h264() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("PORATAKE_FFMPEG_PATH") {
        candidates.push(PathBuf::from(path));
    }
    let path_var = std::env::var_os("PATH")?;
    candidates
        .extend(std::env::split_paths(&path_var).map(|directory| directory.join(BINARY_NAME)));
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file() && supports_h264(candidate))
}

/// Whether recording can encode H.264 right now.
pub fn h264_available() -> bool {
    resolve_h264().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_prefers_the_explicit_override() {
        // Anything that exists beats the PATH search, so a file that surely
        // exists on every test host pins the override branch.
        let override_path = std::env::temp_dir().join("poratake-ffmpeg-override-check");
        std::fs::write(&override_path, b"").expect("write override marker");
        assert_eq!(
            resolve_with_override(Some(override_path.as_os_str())).as_deref(),
            Some(override_path.as_path())
        );
        let _ = std::fs::remove_file(&override_path);
    }

    #[test]
    fn supports_h264_rejects_a_missing_binary() {
        assert!(!supports_h264(Path::new("/nonexistent/poratake-ffmpeg")));
    }

    #[test]
    fn raw_video_input_args_carry_the_stream_shape() {
        assert_eq!(
            raw_video_input_args("rgba", 640, 360, 30),
            vec![
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-video_size",
                "640x360",
                "-framerate",
                "30",
                "-i",
                "pipe:0"
            ]
        );
    }

    #[test]
    fn h264_encode_args_switch_rate_control_by_mode() {
        let crf = h264_encode_args(VideoRate::Crf(23));
        assert!(crf.contains(&"-crf".to_string()) && crf.contains(&"23".to_string()));
        assert!(!crf.contains(&"-b:v".to_string()));
        let bitrate = h264_encode_args(VideoRate::Bitrate(8_000_000));
        assert!(bitrate.contains(&"-b:v".to_string()) && bitrate.contains(&"8000000".to_string()));
        for args in [crf, bitrate] {
            assert!(args.contains(&"libx264".to_string()));
            assert!(args.contains(&"yuv420p".to_string()));
            assert!(args.contains(&"+faststart".to_string()));
        }
    }
}
