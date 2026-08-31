//! Side tracks for X11 screen recording, mirroring the Windows daemon's
//! recorder output: AAC audio files (`system.m4a` / `mic.m4a`, 48 kHz stereo,
//! 192 kbps) and a camera picture-in-picture H.264 file (`camera.mov`), each
//! beside the main video.
//!
//! Audio sources come from the PulseAudio compatibility API (which PipeWire
//! exposes on modern desktops): the default sink's `.monitor` capture carries
//! the desktop mix, and the default (or requested) source carries the
//! microphone. The camera comes from a V4L2 device. Each track is its own
//! FFmpeg process, so a failing track never takes the video down; pausing
//! suspends the process with SIGSTOP so the track skips the same span the
//! frame-indexed video skips, and stopping sends SIGTERM so FFmpeg writes the
//! container trailer.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use poratake_daemon_common::ffmpeg;

/// Audio encoding parameters, matching the Windows module's constants.
const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u32 = 2;
const BIT_RATE: u32 = 192_000;
const SYSTEM_AUDIO_FILE: &str = "system.m4a";
const MICROPHONE_AUDIO_FILE: &str = "mic.m4a";
const CAMERA_FILE: &str = "camera.mov";
/// The first V4L2 capture device, used when the start request names none.
const DEFAULT_CAMERA_DEVICE: &str = "/dev/video0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrackKind {
    System,
    Microphone,
    Camera,
}

impl TrackKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::System => "system audio",
            Self::Microphone => "microphone",
            Self::Camera => "camera",
        }
    }

    pub(super) fn file_name(self) -> &'static str {
        match self {
            Self::System => SYSTEM_AUDIO_FILE,
            Self::Microphone => MICROPHONE_AUDIO_FILE,
            Self::Camera => CAMERA_FILE,
        }
    }
}

/// One running track encoder process and the file it is writing.
pub(super) struct Track {
    pub kind: TrackKind,
    pub path: PathBuf,
    child: Child,
}

impl Track {
    pub fn pause(&mut self) {
        self.signal(libc::SIGSTOP);
    }

    pub fn resume(&mut self) {
        self.signal(libc::SIGCONT);
    }

    /// Asks FFmpeg to finalize the file; the matching [`Self::wait_finalized`]
    /// happens after every track is signalled so the trailers are written
    /// concurrently. A track left SIGSTOPped by a pause cannot act on
    /// SIGTERM, so it is continued first.
    pub fn request_finalize(&mut self) {
        self.signal(libc::SIGCONT);
        self.signal(libc::SIGTERM);
    }

    /// Waits for a signalled track to exit, falling back to a hard kill so
    /// `stop` never hangs on a wedged encoder.
    pub fn wait_finalized(&mut self) {
        ffmpeg::wait_for_exit(&mut self.child, Duration::from_secs(2));
    }

    /// Best-effort teardown for a session that never produced video; no
    /// finalize wait is owed to a recording the user never sees.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    fn signal(&mut self, sig: i32) {
        unsafe {
            libc::kill(self.child.id() as i32, sig);
        }
    }
}

/// Settings-surface toggles for the running session. They overlay a start
/// request while a session is live and are cleared when it ends, so a toggle
/// never leaks into a later start — the next start reads its configuration
/// purely from the wire, the way the Windows recorder does.
#[derive(Default)]
pub(super) struct TrackPrefs {
    system_override: Mutex<Option<bool>>,
    microphone_override: Mutex<Option<bool>>,
    microphone_device: Mutex<Option<String>>,
    camera_override: Mutex<Option<bool>>,
}

impl TrackPrefs {
    pub fn system_enabled(&self, wire: bool) -> bool {
        self.system_override.lock().unwrap().unwrap_or(wire)
    }

    pub fn microphone_enabled(&self, wire: bool) -> bool {
        self.microphone_override.lock().unwrap().unwrap_or(wire)
    }

    pub fn camera_enabled(&self, wire: bool) -> bool {
        self.camera_override.lock().unwrap().unwrap_or(wire)
    }

    pub fn microphone_device(&self, wire_device: Option<&str>) -> Option<String> {
        self.microphone_device
            .lock()
            .unwrap()
            .clone()
            .or_else(|| wire_device.map(str::to_string))
    }

    pub fn set_microphone(&self, enabled: bool, device: Option<String>) {
        *self.microphone_override.lock().unwrap() = Some(enabled);
        *self.microphone_device.lock().unwrap() = device;
    }

    pub fn set_system(&self, enabled: bool) {
        *self.system_override.lock().unwrap() = Some(enabled);
    }

    pub fn set_camera(&self, enabled: bool) {
        *self.camera_override.lock().unwrap() = Some(enabled);
    }

    pub fn clear(&self) {
        *self.system_override.lock().unwrap() = None;
        *self.microphone_override.lock().unwrap() = None;
        *self.microphone_device.lock().unwrap() = None;
        *self.camera_override.lock().unwrap() = None;
    }
}

/// Spawns the requested side tracks beside the video output. A track whose
/// source cannot be resolved, or whose encoder dies immediately (no
/// Pulse/PipeWire server, missing camera), is dropped with a log line so the
/// recording continues without it.
/// source cannot be resolved, or whose encoder dies immediately (no
/// Pulse/PipeWire server, missing camera), is dropped with a log line so the
/// recording continues without it.
pub(super) fn spawn_tracks(
    ffmpeg_binary: &Path,
    kinds: &[TrackKind],
    microphone_device: Option<&str>,
    camera_device: Option<&str>,
    frame_rate: u32,
    output_directory: &Path,
) -> Vec<Track> {
    let mut spawned = Vec::new();
    for kind in kinds {
        let mut command = Command::new(ffmpeg_binary);
        command.args(ffmpeg::quiet_args());
        match kind {
            TrackKind::System => {
                let Some(monitor) = default_sink_monitor() else {
                    eprintln!("[recorder] could not resolve a Pulse monitor for system audio");
                    continue;
                };
                command
                    .args(["-f", "pulse", "-i", &monitor])
                    .args(aac_args());
            }
            TrackKind::Microphone => {
                let source = microphone_device.unwrap_or("default").to_string();
                command
                    .args(["-f", "pulse", "-i", &source])
                    .args(aac_args());
            }
            TrackKind::Camera => {
                let device = camera_device.unwrap_or(DEFAULT_CAMERA_DEVICE).to_string();
                if !Path::new(&device).exists() {
                    eprintln!("[recorder] no camera device at {device}");
                    continue;
                }
                command
                    .args(["-f", "v4l2", "-framerate", &frame_rate.to_string()])
                    .args(["-i", &device])
                    .args(ffmpeg::h264_encode_args(ffmpeg::VideoRate::Crf(23)));
            }
        }
        let path = output_directory.join(kind.file_name());
        let child = command
            .arg("-y")
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match child {
            Ok(child) => spawned.push((*kind, path, child)),
            Err(error) => eprintln!(
                "[recorder] could not spawn the {} track encoder: {error}",
                kind.label()
            ),
        }
    }
    if spawned.is_empty() {
        return Vec::new();
    }
    // A source that does not exist fails within the first samples; a healthy
    // encoder is still running after this grace period.
    std::thread::sleep(Duration::from_millis(250));
    spawned
        .into_iter()
        .filter_map(|(kind, path, mut child)| {
            if let Ok(Some(_)) = child.try_wait() {
                eprintln!(
                    "[recorder] the {} track encoder exited immediately; recording without it",
                    kind.label()
                );
                return None;
            }
            Some(Track { kind, path, child })
        })
        .collect()
}

/// The AAC encoding parameters, matching the Windows module's audio track.
fn aac_args() -> [String; 8] {
    [
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        format!("{BIT_RATE}"),
        "-ar".into(),
        format!("{SAMPLE_RATE}"),
        "-ac".into(),
        format!("{CHANNELS}"),
    ]
}

/// The default sink's monitor source, read from `pactl` (the PulseAudio
/// compatibility entry point that PipeWire also provides).
fn default_sink_monitor() -> Option<String> {
    let output = Command::new("pactl").arg("info").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let info = String::from_utf8_lossy(&output.stdout);
    let sink = default_sink_from_info(&info)?;
    Some(format!("{sink}.monitor"))
}

/// Parses the `Default Sink:` line of `pactl info` output.
fn default_sink_from_info(info: &str) -> Option<&str> {
    info.lines()
        .find_map(|line| line.strip_prefix("Default Sink: "))
        .map(str::trim)
        .filter(|sink| !sink.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sink_is_parsed_from_pactl_info() {
        let info = "Server String: /run/user/1000/pulse/native\n\
                    Default Sink: alsa_output.pci-0000_00_1f.3.analog-stereo\n\
                    Default Source: alsa_input.usb-mic\n";
        assert_eq!(
            default_sink_from_info(info),
            Some("alsa_output.pci-0000_00_1f.3.analog-stereo")
        );
        assert_eq!(default_sink_from_info("no match here"), None);
        assert_eq!(default_sink_from_info("Default Sink: \n"), None);
    }

    #[test]
    fn track_files_beside_the_video() {
        assert_eq!(TrackKind::System.file_name(), "system.m4a");
        assert_eq!(TrackKind::Microphone.file_name(), "mic.m4a");
        assert_eq!(TrackKind::Camera.file_name(), "camera.mov");
    }
}
