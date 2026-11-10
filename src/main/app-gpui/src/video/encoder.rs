//! H.264/AAC encoding for video export, one hardware-native backend per
//! platform: Media Foundation's sink writer on Windows, VideoToolbox on
//! macOS, and a distribution FFmpeg with libx264 on Linux. Each backend
//! lives on one dedicated thread; callers push plain byte buffers to it over
//! a channel.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// The audio format every mixed track is resampled to before encoding.
pub const AUDIO_SAMPLE_RATE: u32 = 48_000;
pub const AUDIO_CHANNELS: u32 = 2;
pub const AUDIO_BITS_PER_SAMPLE: u32 = 16;
/// 128 kbps AAC, the rate the AAC encoder accepts for stereo at 48 kHz.
#[cfg(any(windows, target_os = "macos"))]
pub const AUDIO_BYTES_PER_SECOND: u32 = 16_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub width: u32,
    pub height: u32,
    pub frame_rate: u32,
    pub bitrate: u32,
    pub has_audio: bool,
}

enum Command {
    Video {
        bgra: Vec<u8>,
        time: i64,
        duration: i64,
    },
    Audio {
        pcm: Vec<u8>,
        time: i64,
        duration: i64,
    },
    Finish {
        reply: Sender<Result<(), String>>,
    },
}

pub struct Encoder {
    commands: Option<Sender<Command>>,
    settings: Settings,
    frames_written: u64,
    cancelled: Arc<AtomicBool>,
    child: ChildSlot,
    worker: Option<std::thread::JoinHandle<()>>,
}

type ChildSlot = Arc<Mutex<Option<std::process::Child>>>;

impl Encoder {
    /// Creates the output file. Blocking, so callers run it on the background
    /// executor.
    pub fn create(path: &Path, settings: Settings) -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let owned = path.to_path_buf();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let child = Arc::new(Mutex::new(None));
        let worker_child = child.clone();

        let worker = std::thread::Builder::new()
            .name("video-encoder".into())
            .spawn(move || {
                backend::run(
                    owned,
                    settings,
                    command_rx,
                    ready_tx,
                    worker_cancelled,
                    worker_child,
                )
            })
            .map_err(|error| format!("could not start the encoder: {error}"))?;

        let encoder = Self {
            commands: Some(command_tx),
            settings,
            frames_written: 0,
            cancelled,
            child,
            worker: Some(worker),
        };
        ready_rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| "the encoder did not start".to_string())??;

        Ok(encoder)
    }

    /// Appends one composed frame. `bgra` must be `width * height * 4` bytes.
    pub fn write_frame(&mut self, bgra: Vec<u8>) -> Result<(), String> {
        let expected = self.settings.width as usize * self.settings.height as usize * 4;
        if bgra.len() != expected {
            return Err(format!(
                "frame is {} bytes, expected {expected}",
                bgra.len()
            ));
        }
        let frame_rate = self.settings.frame_rate.max(1) as i64;
        let duration = UNITS_PER_SECOND / frame_rate;
        let time = self.frames_written as i64 * duration;
        self.frames_written += 1;
        self.commands
            .as_ref()
            .unwrap()
            .send(Command::Video {
                bgra,
                time,
                duration,
            })
            .map_err(|_| "the encoder stopped".to_string())
    }

    /// Appends interleaved 16-bit PCM at [`AUDIO_SAMPLE_RATE`].
    pub fn write_audio(&self, pcm: Vec<u8>, time: i64) -> Result<(), String> {
        if pcm.is_empty() {
            return Ok(());
        }
        let frame_bytes = (AUDIO_CHANNELS * AUDIO_BITS_PER_SAMPLE / 8) as i64;
        let frames = pcm.len() as i64 / frame_bytes;
        let duration = frames * UNITS_PER_SECOND / AUDIO_SAMPLE_RATE as i64;
        self.commands
            .as_ref()
            .unwrap()
            .send(Command::Audio {
                pcm,
                time,
                duration,
            })
            .map_err(|_| "the encoder stopped".to_string())
    }

    /// Finalizes the file. Consuming the encoder guarantees no frame can be
    /// appended after the container is closed.
    #[cfg(all(test, windows))]
    pub fn finish(self) -> Result<(), String> {
        self.finish_cancellable(&|| false)
    }

    pub fn finish_cancellable(self, should_cancel: &dyn Fn() -> bool) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.commands
            .as_ref()
            .unwrap()
            .send(Command::Finish { reply: reply_tx })
            .map_err(|_| "the encoder stopped".to_string())?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            if should_cancel() {
                return Err("the export was cancelled".to_string());
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err("the encoder did not finish".to_string());
            }
            match reply_rx.recv_timeout(remaining.min(std::time::Duration::from_millis(25))) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("the encoder stopped".to_string());
                }
            }
        }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(child) = self.child.lock().unwrap().as_mut() {
            let _ = child.kill();
        }
        self.commands.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn register_child(
    slot: &ChildSlot,
    mut child: std::process::Child,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let mut slot = slot.lock().unwrap();
    if cancelled.load(Ordering::Acquire) {
        let _ = child.kill();
        let _ = child.wait();
        return Err("the export was cancelled".to_string());
    }
    *slot = Some(child);
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn stop_child(slot: &ChildSlot) {
    if let Some(mut child) = slot.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn wait_for_child(
    slot: &ChildSlot,
    timeout: std::time::Duration,
) -> Option<std::process::ExitStatus> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let mut guard = slot.lock().unwrap();
        let child = guard.as_mut()?;
        match child.try_wait() {
            Ok(Some(status)) => {
                guard.take();
                return Some(status);
            }
            Ok(None) if std::time::Instant::now() < deadline => {}
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                guard.take();
                return None;
            }
        }
        drop(guard);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// `MFTIME` counts 100-nanosecond units.
const UNITS_PER_SECOND: i64 = 10_000_000;

#[cfg(windows)]
mod backend {
    use super::*;

    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

    pub fn run(
        path: PathBuf,
        settings: Settings,
        commands: Receiver<Command>,
        ready: Sender<Result<(), String>>,
        cancelled: Arc<AtomicBool>,
        _child: ChildSlot,
    ) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            if MFStartup(MF_VERSION, MFSTARTUP_FULL).is_err() {
                let _ = ready.send(Err("Media Foundation is unavailable".into()));
                CoUninitialize();
                return;
            }

            match open_writer(&path, settings) {
                Ok((writer, video_stream, audio_stream)) => {
                    let _ = ready.send(Ok(()));
                    pump(writer, video_stream, audio_stream, commands, &cancelled);
                }
                Err(error) => {
                    let _ = ready.send(Err(error));
                }
            }

            let _ = MFShutdown();
            CoUninitialize();
        }
    }

    unsafe fn pump(
        writer: IMFSinkWriter,
        video_stream: u32,
        audio_stream: Option<u32>,
        commands: Receiver<Command>,
        cancelled: &AtomicBool,
    ) {
        let mut failure = None;
        while let Ok(command) = commands.recv() {
            if cancelled.load(Ordering::Acquire) {
                break;
            }
            match command {
                Command::Video {
                    bgra,
                    time,
                    duration,
                } => {
                    if failure.is_none() {
                        failure = write_video(&writer, video_stream, &bgra, time, duration).err();
                    }
                }
                Command::Audio {
                    pcm,
                    time,
                    duration,
                } => {
                    let Some(stream) = audio_stream else {
                        continue;
                    };
                    if failure.is_none() {
                        failure = write_audio(&writer, stream, &pcm, time, duration).err();
                    }
                }
                Command::Finish { reply } => {
                    let result = match failure.take() {
                        Some(error) => Err(error),
                        None => writer
                            .Finalize()
                            .map_err(|error| format!("could not finalize the file: {error}")),
                    };
                    drop(writer);
                    let _ = reply.send(result);
                    return;
                }
            }
        }
    }

    unsafe fn open_writer(
        path: &Path,
        settings: Settings,
    ) -> Result<(IMFSinkWriter, u32, Option<u32>), String> {
        let wide = HSTRING::from(path.as_os_str());

        let mut attributes: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attributes, 2).map_err(mf_error)?;
        let attributes = attributes.ok_or_else(|| "no writer attributes".to_string())?;
        // Hardware encoders are preferred; throttling is for live capture and
        // only slows a file export down.
        attributes
            .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
            .map_err(mf_error)?;
        attributes
            .SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)
            .map_err(mf_error)?;

        let writer = MFCreateSinkWriterFromURL(PCWSTR(wide.as_ptr()), None, &attributes)
            .map_err(mf_error)?;

        let video_stream = add_video_stream(&writer, settings)?;
        let audio_stream = if settings.has_audio {
            Some(add_audio_stream(&writer)?)
        } else {
            None
        };

        writer.BeginWriting().map_err(mf_error)?;
        Ok((writer, video_stream, audio_stream))
    }

    unsafe fn add_video_stream(writer: &IMFSinkWriter, settings: Settings) -> Result<u32, String> {
        let output = MFCreateMediaType().map_err(mf_error)?;
        output
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(mf_error)?;
        output
            .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)
            .map_err(mf_error)?;
        output
            .SetUINT32(&MF_MT_AVG_BITRATE, settings.bitrate.max(100_000))
            .map_err(mf_error)?;
        output
            .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .map_err(mf_error)?;
        set_frame_size(&output, settings.width, settings.height)?;
        set_ratio(&output, &MF_MT_FRAME_RATE, settings.frame_rate.max(1), 1)?;
        set_ratio(&output, &MF_MT_PIXEL_ASPECT_RATIO, 1, 1)?;
        let stream = writer.AddStream(&output).map_err(mf_error)?;

        let input = MFCreateMediaType().map_err(mf_error)?;
        input
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(mf_error)?;
        input
            .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)
            .map_err(mf_error)?;
        input
            .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .map_err(mf_error)?;
        // A positive stride marks the buffer as top-down, which is how the
        // composition writes its rows.
        input
            .SetUINT32(&MF_MT_DEFAULT_STRIDE, settings.width * 4)
            .map_err(mf_error)?;
        set_frame_size(&input, settings.width, settings.height)?;
        set_ratio(&input, &MF_MT_FRAME_RATE, settings.frame_rate.max(1), 1)?;
        set_ratio(&input, &MF_MT_PIXEL_ASPECT_RATIO, 1, 1)?;
        writer
            .SetInputMediaType(stream, &input, None)
            .map_err(mf_error)?;

        Ok(stream)
    }

    unsafe fn add_audio_stream(writer: &IMFSinkWriter) -> Result<u32, String> {
        let output = MFCreateMediaType().map_err(mf_error)?;
        output
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .map_err(mf_error)?;
        output
            .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)
            .map_err(mf_error)?;
        output
            .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, AUDIO_BITS_PER_SAMPLE)
            .map_err(mf_error)?;
        output
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, AUDIO_SAMPLE_RATE)
            .map_err(mf_error)?;
        output
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, AUDIO_CHANNELS)
            .map_err(mf_error)?;
        output
            .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, AUDIO_BYTES_PER_SECOND)
            .map_err(mf_error)?;
        let stream = writer.AddStream(&output).map_err(mf_error)?;

        let input = MFCreateMediaType().map_err(mf_error)?;
        input
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .map_err(mf_error)?;
        input
            .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)
            .map_err(mf_error)?;
        input
            .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, AUDIO_BITS_PER_SAMPLE)
            .map_err(mf_error)?;
        input
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, AUDIO_SAMPLE_RATE)
            .map_err(mf_error)?;
        input
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, AUDIO_CHANNELS)
            .map_err(mf_error)?;
        writer
            .SetInputMediaType(stream, &input, None)
            .map_err(mf_error)?;

        Ok(stream)
    }

    unsafe fn set_frame_size(
        media_type: &IMFMediaType,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        media_type
            .SetUINT64(&MF_MT_FRAME_SIZE, ((width as u64) << 32) | height as u64)
            .map_err(mf_error)
    }

    unsafe fn set_ratio(
        media_type: &IMFMediaType,
        key: &windows::core::GUID,
        numerator: u32,
        denominator: u32,
    ) -> Result<(), String> {
        media_type
            .SetUINT64(key, ((numerator as u64) << 32) | denominator as u64)
            .map_err(mf_error)
    }

    unsafe fn write_video(
        writer: &IMFSinkWriter,
        stream: u32,
        bgra: &[u8],
        time: i64,
        duration: i64,
    ) -> Result<(), String> {
        let sample = make_sample(bgra, time, duration)?;
        writer.WriteSample(stream, &sample).map_err(mf_error)
    }

    unsafe fn write_audio(
        writer: &IMFSinkWriter,
        stream: u32,
        pcm: &[u8],
        time: i64,
        duration: i64,
    ) -> Result<(), String> {
        let sample = make_sample(pcm, time, duration)?;
        writer.WriteSample(stream, &sample).map_err(mf_error)
    }

    unsafe fn make_sample(bytes: &[u8], time: i64, duration: i64) -> Result<IMFSample, String> {
        let buffer = MFCreateMemoryBuffer(bytes.len() as u32).map_err(mf_error)?;
        let mut data = std::ptr::null_mut();
        buffer.Lock(&mut data, None, None).map_err(mf_error)?;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        buffer.Unlock().map_err(mf_error)?;
        buffer
            .SetCurrentLength(bytes.len() as u32)
            .map_err(mf_error)?;

        let sample = MFCreateSample().map_err(mf_error)?;
        sample.AddBuffer(&buffer).map_err(mf_error)?;
        sample.SetSampleTime(time).map_err(mf_error)?;
        sample.SetSampleDuration(duration).map_err(mf_error)?;
        Ok(sample)
    }

    fn mf_error(error: windows::core::Error) -> String {
        format!("Media Foundation error: {error}")
    }
}

#[cfg(target_os = "macos")]
mod backend {
    use super::*;
    use std::io::{Read, Write};
    use std::process::{Child, ChildStdin, Command as ProcessCommand, Stdio};

    pub fn run(
        path: PathBuf,
        settings: Settings,
        commands: Receiver<Command>,
        ready: Sender<Result<(), String>>,
        cancelled: Arc<AtomicBool>,
        child: ChildSlot,
    ) {
        let ffmpeg = crate::video::ffmpeg_path();
        if !ffmpeg.is_file() {
            let _ = ready.send(Err(format!("FFmpeg was not found at {}", ffmpeg.display())));
            return;
        }
        let audio_path = std::env::temp_dir().join(format!(
            "poratake-export-audio-{}-{}.pcm",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let _ = ready.send(Ok(()));

        let mut input = None;
        let mut failure = None;
        while let Ok(command) = commands.recv() {
            if cancelled.load(Ordering::Acquire) {
                break;
            }
            match command {
                Command::Audio {
                    pcm,
                    time,
                    duration,
                } => {
                    let _ = duration;
                    if failure.is_none() {
                        failure = write_audio(&audio_path, &pcm, time).err();
                    }
                }
                Command::Video {
                    bgra,
                    time,
                    duration,
                } => {
                    let _ = (time, duration);
                    if failure.is_some() {
                        continue;
                    }
                    if input.is_none() {
                        match start(
                            &ffmpeg,
                            &path,
                            settings,
                            settings.has_audio.then_some(audio_path.as_path()),
                        ) {
                            Ok((process, stdin)) => {
                                if let Err(error) = register_child(&child, process, &cancelled) {
                                    failure = Some(error);
                                    continue;
                                }
                                input = Some(stdin);
                            }
                            Err(error) => {
                                failure = Some(error);
                                continue;
                            }
                        }
                    }
                    if let Err(error) = input
                        .as_mut()
                        .ok_or_else(|| "FFmpeg input is unavailable".to_string())
                        .and_then(|stdin| {
                            stdin
                                .write_all(&bgra)
                                .map_err(|error| format!("FFmpeg input failed: {error}"))
                        })
                    {
                        failure = Some(error);
                    }
                }
                Command::Finish { reply } => {
                    drop(input.take());
                    let has_child = child.lock().unwrap().is_some();
                    let result = match (failure.take(), has_child) {
                        (Some(error), _) => {
                            stop_child(&child);
                            Err(error)
                        }
                        (None, true) => finish(&child),
                        (None, false) => Err("no video frames were written".to_string()),
                    };
                    let _ = std::fs::remove_file(&audio_path);
                    let _ = reply.send(result);
                    return;
                }
            }
        }
        stop_child(&child);
        let _ = std::fs::remove_file(audio_path);
    }

    fn write_audio(path: &Path, pcm: &[u8], time: i64) -> Result<(), String> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| format!("audio staging failed: {error}"))?;
        if time > 0 && file.metadata().map(|metadata| metadata.len()).unwrap_or(0) == 0 {
            let bytes = time as u64 * AUDIO_SAMPLE_RATE as u64 * AUDIO_CHANNELS as u64 * 2
                / UNITS_PER_SECOND as u64;
            std::io::copy(&mut std::io::repeat(0).take(bytes), &mut file)
                .map_err(|error| format!("audio padding failed: {error}"))?;
        }
        file.write_all(pcm)
            .map_err(|error| format!("audio staging failed: {error}"))
    }

    fn start(
        ffmpeg: &Path,
        path: &Path,
        settings: Settings,
        audio_path: Option<&Path>,
    ) -> Result<(Child, ChildStdin), String> {
        let mut command = ProcessCommand::new(ffmpeg);
        command
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "bgra",
                "-video_size",
                &format!("{}x{}", settings.width, settings.height),
                "-framerate",
                &settings.frame_rate.max(1).to_string(),
                "-i",
                "pipe:0",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        if let Some(audio_path) = audio_path {
            command
                .args(["-f", "s16le", "-ar", "48000", "-ac", "2", "-i"])
                .arg(audio_path);
        }
        command.args([
            "-c:v",
            "h264_videotoolbox",
            "-b:v",
            &settings.bitrate.to_string(),
            "-pix_fmt",
            "yuv420p",
            "-force_key_frames",
            "expr:eq(t,0)",
        ]);
        if audio_path.is_some() {
            command.args([
                "-c:a",
                "aac_at",
                "-b:a",
                &format!("{}k", AUDIO_BYTES_PER_SECOND * 8 / 1000),
            ]);
        } else {
            command.arg("-an");
        }
        command.args(["-movflags", "+faststart"]).arg(path);
        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start FFmpeg: {error}"))?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| "FFmpeg input is unavailable".to_string())?;
        Ok((child, input))
    }

    fn finish(child: &ChildSlot) -> Result<(), String> {
        let stderr = child
            .lock()
            .unwrap()
            .as_mut()
            .and_then(|process| process.stderr.take());
        let reader = stderr.map(|mut stderr| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                let _ = stderr.read_to_end(&mut bytes);
                bytes
            })
        });
        let status = wait_for_child(child, std::time::Duration::from_secs(120));
        let stderr = reader
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default();
        let Some(status) = status else {
            return Err("FFmpeg did not finish".to_string());
        };
        if status.success() {
            return Ok(());
        }
        let detail = String::from_utf8_lossy(&stderr).trim().to_string();
        Err(if detail.is_empty() {
            format!("FFmpeg exited with {status}")
        } else {
            detail
        })
    }
}

#[cfg(target_os = "linux")]
mod backend {
    use super::*;

    use std::io::Write;
    use std::process::Command as StdCommand;
    use std::process::{ChildStdin, Stdio};
    use std::sync::{Arc, Mutex};

    use poratake_daemon_common::ffmpeg;

    /// How long the encoders get to flush and finalize after their inputs
    /// close; a wedged FFmpeg is killed past the deadline.
    const FINALIZE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
    /// 128 kbps AAC, matching the rate the other platforms' mixers assume.
    const AUDIO_BIT_RATE: &str = "128k";

    struct Session {
        ffmpeg: PathBuf,
        child: ChildSlot,
        stdin: ChildStdin,
        output: PathBuf,
        video_temp: PathBuf,
        audio_temp: Option<PathBuf>,
        audio_file: Option<std::fs::File>,
        /// The last line FFmpeg printed, surfaced when the encode fails so a
        /// bad pixel format or full disk is diagnosable.
        stderr_tail: Arc<Mutex<String>>,
    }

    impl Session {
        /// Spawns the video-only encode. The composed frames stream through
        /// stdin while audio accumulates beside it; [`Self::finalize`] muxes
        /// the two into the output file.
        fn start(
            ffmpeg: PathBuf,
            path: &Path,
            settings: Settings,
            child: ChildSlot,
            cancelled: &AtomicBool,
        ) -> Result<Self, String> {
            let directory = path.parent().unwrap_or_else(|| Path::new("."));
            let video_temp = staged_path(directory, path, "video");
            let audio_temp = settings
                .has_audio
                .then(|| staged_path(directory, path, "audio"));
            let mut args = ffmpeg::quiet_args();
            args.extend(ffmpeg::raw_video_input_args(
                "bgra",
                settings.width,
                settings.height,
                settings.frame_rate,
            ));
            args.extend(ffmpeg::h264_encode_args(ffmpeg::VideoRate::Bitrate(
                settings.bitrate,
            )));
            args.push("-y".into());
            let mut process = StdCommand::new(&ffmpeg)
                .args(&args)
                .arg(&video_temp)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|error| format!("could not spawn the export encoder: {error}"))?;
            let stdin = process
                .stdin
                .take()
                .ok_or("the export encoder has no stdin")?;
            let stderr = process
                .stderr
                .take()
                .ok_or("the export encoder has no stderr")?;
            register_child(&child, process, cancelled)?;
            let stderr_tail = ffmpeg::spawn_stderr_tail(stderr, |_| {});
            let audio_file = match &audio_temp {
                Some(audio_temp) => Some(
                    std::fs::File::create(audio_temp)
                        .map_err(|error| format!("could not stage export audio: {error}"))?,
                ),
                None => None,
            };
            Ok(Self {
                ffmpeg,
                child,
                stdin,
                output: path.to_path_buf(),
                video_temp,
                audio_temp,
                audio_file,
                stderr_tail,
            })
        }

        fn write_video(&mut self, bgra: &[u8]) -> Result<(), String> {
            self.stdin.write_all(bgra).map_err(|_| {
                format!(
                    "the export encoder stopped accepting frames: {}",
                    self.stderr_tail.lock().unwrap()
                )
            })
        }

        fn write_audio(&mut self, pcm: &[u8]) -> Result<(), String> {
            let Some(audio_file) = self.audio_file.as_mut() else {
                return Ok(());
            };
            audio_file
                .write_all(pcm)
                .map_err(|_| "could not stage export audio".to_string())
        }

        fn abort(self) {
            let Session {
                child,
                stdin,
                video_temp,
                audio_temp,
                audio_file,
                ..
            } = self;
            drop(stdin);
            drop(audio_file);
            stop_child(&child);
            let _ = std::fs::remove_file(video_temp);
            if let Some(audio_temp) = audio_temp {
                let _ = std::fs::remove_file(audio_temp);
            }
        }

        /// Closes the inputs, muxes audio in when present, and moves the
        /// result onto the output path.
        fn finalize(self, cancelled: &AtomicBool) -> Result<(), String> {
            let Session {
                ffmpeg,
                child,
                stdin,
                output,
                video_temp,
                audio_temp,
                audio_file,
                stderr_tail,
            } = self;
            drop(stdin);
            drop(audio_file);
            let remove_staged_files = |video_temp: &Path, audio_temp: &Option<PathBuf>| {
                let _ = std::fs::remove_file(video_temp);
                if let Some(audio_temp) = audio_temp {
                    let _ = std::fs::remove_file(audio_temp);
                }
            };
            let finished = wait_for_child(&child, FINALIZE_TIMEOUT).is_some();
            if !finished || cancelled.load(Ordering::Acquire) {
                remove_staged_files(&video_temp, &audio_temp);
                return Err(format!(
                    "the export encoder did not finish: {}",
                    stderr_tail.lock().unwrap()
                ));
            }
            let Some(audio_temp) = audio_temp else {
                let moved = std::fs::rename(&video_temp, &output)
                    .map_err(|error| format!("could not finalize the export: {error}"));
                remove_staged_files(&video_temp, &None);
                return moved;
            };
            let mux = StdCommand::new(&ffmpeg)
                .args(["-hide_banner", "-loglevel", "error", "-i"])
                .arg(&video_temp)
                .args([
                    "-f",
                    "s16le",
                    "-ar",
                    &AUDIO_SAMPLE_RATE.to_string(),
                    "-ac",
                    &AUDIO_CHANNELS.to_string(),
                    "-i",
                ])
                .arg(&audio_temp)
                .args([
                    "-map",
                    "0:v",
                    "-map",
                    "1:a",
                    "-c:v",
                    "copy",
                    "-c:a",
                    "aac",
                    "-b:a",
                    AUDIO_BIT_RATE,
                    "-movflags",
                    "+faststart",
                    "-y",
                ])
                .arg(&output)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn();
            let mut mux = match mux {
                Ok(mux) => mux,
                Err(error) => {
                    remove_staged_files(&video_temp, &Some(audio_temp));
                    return Err(format!("could not spawn the export muxer: {error}"));
                }
            };
            // The muxer's stderr is the only place a mux failure explains
            // itself; capture it for the error message before waiting.
            let reader = mux.stderr.take().map(|stderr| {
                std::thread::spawn(move || {
                    use std::io::BufRead;
                    std::io::BufReader::new(stderr)
                        .lines()
                        .map_while(Result::ok)
                        .filter(|line| !line.is_empty())
                        .last()
                        .unwrap_or_default()
                })
            });
            if let Err(error) = register_child(&child, mux, cancelled) {
                if let Some(reader) = reader {
                    let _ = reader.join();
                }
                remove_staged_files(&video_temp, &Some(audio_temp));
                return Err(error);
            }
            let finished = wait_for_child(&child, FINALIZE_TIMEOUT).is_some();
            let mux_error = reader
                .and_then(|reader| reader.join().ok())
                .unwrap_or_default();
            remove_staged_files(&video_temp, &Some(audio_temp));
            if !finished || cancelled.load(Ordering::Acquire) {
                return Err(format!("the export muxer did not finish: {mux_error}"));
            }
            Ok(())
        }
    }

    /// `<name>.poratake-video.tmp`-style staging beside the real output, so
    /// the final rename stays on one filesystem.
    fn staged_path(directory: &Path, output: &Path, kind: &str) -> PathBuf {
        let name = output
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("export");
        directory.join(format!(".{name}.poratake-{kind}.tmp"))
    }

    pub fn run(
        path: PathBuf,
        settings: Settings,
        commands: Receiver<Command>,
        ready: Sender<Result<(), String>>,
        cancelled: Arc<AtomicBool>,
        child: ChildSlot,
    ) {
        let Some(ffmpeg) = ffmpeg::resolve_h264() else {
            let _ = ready.send(Err(
                "video export needs FFmpeg with the libx264 encoder on PATH \
                 (or PORATAKE_FFMPEG_PATH)"
                    .to_string(),
            ));
            return;
        };
        let mut session = match Session::start(ffmpeg, &path, settings, child, &cancelled) {
            Ok(session) => session,
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };
        let _ = ready.send(Ok(()));

        while let Ok(command) = commands.recv() {
            if cancelled.load(Ordering::Acquire) {
                break;
            }
            let outcome = match command {
                // Raw-video timestamps come from the frame rate, so the
                // timeline offsets the other platforms use are not needed.
                Command::Video {
                    bgra,
                    time,
                    duration,
                } => {
                    let _ = (time, duration);
                    session.write_video(&bgra)
                }
                Command::Audio {
                    pcm,
                    time,
                    duration,
                } => {
                    let _ = (time, duration);
                    session.write_audio(&pcm)
                }
                Command::Finish { reply } => {
                    let _ = reply.send(session.finalize(&cancelled));
                    return;
                }
            };
            if outcome.is_err() {
                // The encode is broken; drain the channel so a dropped
                // Encoder cannot deadlock a caller still sending.
                stop_child(&session.child);
                while let Ok(command) = commands.recv() {
                    if cancelled.load(Ordering::Acquire) {
                        break;
                    }
                    match command {
                        Command::Finish { reply } => {
                            let _ = reply.send(Err(outcome.err().unwrap_or_default()));
                            break;
                        }
                        Command::Video { bgra, .. } => drop(bgra),
                        Command::Audio { pcm, .. } => drop(pcm),
                    }
                }
                session.abort();
                return;
            }
        }
        session.abort();
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        #[test]
        fn abort_reaps_the_child_and_removes_staged_files() {
            let directory = tempfile::tempdir().unwrap();
            let fake = directory.path().join("ffmpeg");
            std::fs::write(
                &fake,
                "#!/bin/sh\nfor arg in \"$@\"; do output=\"$arg\"; done\nprintf running > \"$output\"\nprintf %s $$ > \"$output.pid\"\nexec cat >/dev/null\n",
            )
            .unwrap();
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
            let output = directory.path().join("output.mp4");
            let video = staged_path(directory.path(), &output, "video");
            let audio = staged_path(directory.path(), &output, "audio");
            let pid_file = directory.path().join(".output.mp4.poratake-video.tmp.pid");
            let mut session = Session::start(
                fake,
                &output,
                Settings {
                    width: 2,
                    height: 2,
                    frame_rate: 25,
                    bitrate: 1_000_000,
                    has_audio: true,
                },
                Arc::new(Mutex::new(None)),
                &AtomicBool::new(false),
            )
            .unwrap();
            session.write_video(&[0; 16]).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            let pid: i32 = loop {
                if let Some(pid) = std::fs::read_to_string(&pid_file)
                    .ok()
                    .and_then(|text| text.parse().ok())
                {
                    break pid;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            };
            assert!(video.is_file());
            assert!(audio.is_file());
            session.abort();
            assert!(!video.exists());
            assert!(!audio.exists());
            assert!(!output.exists());
            unsafe extern "C" {
                fn kill(pid: i32, signal: i32) -> i32;
            }
            assert_eq!(unsafe { kill(pid, 0) }, -1);
        }

        #[test]
        fn cancelling_unblocks_a_child_that_does_not_read_frames() {
            let directory = tempfile::tempdir().unwrap();
            let fake = directory.path().join("ffmpeg");
            std::fs::write(
                &fake,
                "#!/bin/sh\nfor arg in \"$@\"; do output=\"$arg\"; done\nprintf %s $$ > \"$output.pid\"\nexec sleep 30\n",
            )
            .unwrap();
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
            let output = directory.path().join("output.mp4");
            let pid_file = directory.path().join(".output.mp4.poratake-video.tmp.pid");
            let cancelled = Arc::new(AtomicBool::new(false));
            let child = Arc::new(Mutex::new(None));
            let settings = Settings {
                width: 1024,
                height: 1024,
                frame_rate: 25,
                bitrate: 1_000_000,
                has_audio: false,
            };
            let mut session =
                Session::start(fake, &output, settings, child.clone(), &cancelled).unwrap();
            let worker = std::thread::Builder::new()
                .name("cancel-encoder".into())
                .spawn(move || {
                    let _ = session.write_video(&vec![0; 1024 * 1024 * 4]);
                    session.abort();
                })
                .unwrap();
            let (commands, _receiver) = mpsc::channel();
            let encoder = Encoder {
                commands: Some(commands),
                settings,
                frames_written: 0,
                cancelled,
                child,
                worker: Some(worker),
            };
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let pid: i32 = loop {
                if let Some(pid) = std::fs::read_to_string(&pid_file)
                    .ok()
                    .and_then(|text| text.parse().ok())
                {
                    break pid;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            };
            loop {
                let blocked =
                    std::fs::read_dir("/proc/self/task")
                        .unwrap()
                        .flatten()
                        .any(|entry| {
                            let path = entry.path();
                            std::fs::read_to_string(path.join("comm"))
                                .unwrap_or_default()
                                .trim()
                                == "cancel-encoder"
                                && std::fs::read_to_string(path.join("wchan"))
                                    .unwrap_or_default()
                                    .contains("pipe_write")
                        });
                if blocked {
                    break;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            let (done_tx, done_rx) = mpsc::channel();
            let caller = std::thread::spawn(move || {
                drop(encoder);
                done_tx.send(()).unwrap();
            });
            let stopped = done_rx
                .recv_timeout(std::time::Duration::from_secs(3))
                .is_ok();
            unsafe extern "C" {
                fn kill(pid: i32, signal: i32) -> i32;
            }
            if !stopped {
                unsafe { kill(pid, 9) };
            }
            caller.join().unwrap();
            assert!(stopped, "cancellation must stop the blocked child");
            assert!(!output.exists());
        }

        #[test]
        fn cancelling_during_mux_stops_the_child_and_removes_staged_files() {
            let directory = tempfile::tempdir().unwrap();
            let fake = directory.path().join("ffmpeg");
            std::fs::write(
                &fake,
                "#!/bin/sh\nfor arg in \"$@\"; do output=\"$arg\"; done\ncase \"$output\" in\n  *.poratake-video.tmp) cat >/dev/null; printf video > \"$output\" ;;\n  *) printf partial > \"$output\"; printf %s $$ > \"$output.pid\"; exec sleep 30 ;;\nesac\n",
            )
            .unwrap();
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
            let output = directory.path().join("output.mp4");
            let video = staged_path(directory.path(), &output, "video");
            let audio = staged_path(directory.path(), &output, "audio");
            let pid_file = directory.path().join("output.mp4.pid");
            let cancelled = Arc::new(AtomicBool::new(false));
            let child = Arc::new(Mutex::new(None));
            let settings = Settings {
                width: 2,
                height: 2,
                frame_rate: 25,
                bitrate: 1_000_000,
                has_audio: true,
            };
            let mut session =
                Session::start(fake, &output, settings, child.clone(), &cancelled).unwrap();
            session.write_video(&[0; 16]).unwrap();
            session.write_audio(&[0; 8]).unwrap();
            let (commands, receiver) = mpsc::channel();
            let worker_cancelled = cancelled.clone();
            let worker = std::thread::spawn(move || {
                if let Ok(Command::Finish { reply }) = receiver.recv() {
                    let _ = reply.send(session.finalize(&worker_cancelled));
                } else {
                    session.abort();
                }
            });
            let encoder = Encoder {
                commands: Some(commands),
                settings,
                frames_written: 0,
                cancelled,
                child,
                worker: Some(worker),
            };
            let requested = Arc::new(AtomicBool::new(false));
            let worker_requested = requested.clone();
            let (done_tx, done_rx) = mpsc::channel();
            let caller = std::thread::spawn(move || {
                let result =
                    encoder.finish_cancellable(&|| worker_requested.load(Ordering::Acquire));
                done_tx.send(result).unwrap();
            });
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let pid: Option<i32> = loop {
                if let Some(pid) = std::fs::read_to_string(&pid_file)
                    .ok()
                    .and_then(|text| text.parse().ok())
                {
                    break Some(pid);
                }
                if std::time::Instant::now() >= deadline {
                    break None;
                }
                std::thread::yield_now();
            };
            requested.store(true, Ordering::Release);
            let result = done_rx.recv_timeout(std::time::Duration::from_secs(3));
            unsafe extern "C" {
                fn kill(pid: i32, signal: i32) -> i32;
            }
            if result.is_err() {
                if let Some(pid) = pid {
                    unsafe { kill(pid, 9) };
                }
            }
            caller.join().unwrap();
            let pid = pid.expect("mux process started");
            assert!(result.unwrap().unwrap_err().contains("cancelled"));
            assert_eq!(unsafe { kill(pid, 0) }, -1);
            assert!(!video.exists());
            assert!(!audio.exists());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_of_the_wrong_size_is_rejected() {
        // The size check runs before anything is sent, so it does not need a
        // live encoder thread.
        let (commands, _receiver) = mpsc::channel();
        let mut encoder = Encoder {
            commands: Some(commands),
            settings: Settings {
                width: 4,
                height: 4,
                frame_rate: 30,
                bitrate: 1_000_000,
                has_audio: false,
            },
            frames_written: 0,
            cancelled: Arc::new(AtomicBool::new(false)),
            child: Arc::new(Mutex::new(None)),
            worker: None,
        };
        assert!(encoder.write_frame(vec![0; 8]).is_err());
        assert!(encoder.write_frame(vec![0; 64]).is_ok());
    }

    #[test]
    fn frame_times_advance_by_the_frame_duration() {
        let (commands, receiver) = mpsc::channel();
        let mut encoder = Encoder {
            commands: Some(commands),
            settings: Settings {
                width: 2,
                height: 2,
                frame_rate: 25,
                bitrate: 1_000_000,
                has_audio: false,
            },
            frames_written: 0,
            cancelled: Arc::new(AtomicBool::new(false)),
            child: Arc::new(Mutex::new(None)),
            worker: None,
        };
        encoder.write_frame(vec![0; 16]).expect("first");
        encoder.write_frame(vec![0; 16]).expect("second");

        let mut times = Vec::new();
        while let Ok(Command::Video { time, duration, .. }) = receiver.try_recv() {
            times.push((time, duration));
        }
        assert_eq!(times, vec![(0, 400_000), (400_000, 400_000)]);
    }

    #[test]
    fn audio_durations_follow_the_sample_count() {
        let (commands, receiver) = mpsc::channel();
        let encoder = Encoder {
            commands: Some(commands),
            settings: Settings {
                width: 2,
                height: 2,
                frame_rate: 25,
                bitrate: 1_000_000,
                has_audio: true,
            },
            frames_written: 0,
            cancelled: Arc::new(AtomicBool::new(false)),
            child: Arc::new(Mutex::new(None)),
            worker: None,
        };
        // One second of stereo 16-bit at 48 kHz.
        let pcm = vec![0u8; AUDIO_SAMPLE_RATE as usize * 4];
        encoder.write_audio(pcm, 0).expect("audio");
        match receiver.try_recv() {
            Ok(Command::Audio { duration, time, .. }) => {
                assert_eq!(time, 0);
                assert_eq!(duration, UNITS_PER_SECOND);
            }
            _ => panic!("expected an audio command"),
        }
    }

    #[test]
    fn empty_audio_is_dropped() {
        let (commands, receiver) = mpsc::channel();
        let encoder = Encoder {
            commands: Some(commands),
            settings: Settings {
                width: 2,
                height: 2,
                frame_rate: 25,
                bitrate: 1_000_000,
                has_audio: true,
            },
            frames_written: 0,
            cancelled: Arc::new(AtomicBool::new(false)),
            child: Arc::new(Mutex::new(None)),
            worker: None,
        };
        encoder.write_audio(Vec::new(), 0).expect("empty");
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn cancellation_waits_for_the_encoder_worker_to_release_its_output() {
        let (commands, receiver) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (stopping_tx, stopping_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let worker = std::thread::spawn(move || {
            let Command::Finish { reply } = receiver.recv().unwrap() else {
                panic!("expected finish");
            };
            started_tx.send(()).unwrap();
            while !worker_cancelled.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            stopping_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            let _ = reply.send(Ok(()));
        });
        let encoder = Encoder {
            commands: Some(commands),
            settings: Settings {
                width: 2,
                height: 2,
                frame_rate: 25,
                bitrate: 1_000_000,
                has_audio: false,
            },
            frames_written: 0,
            cancelled,
            child: Arc::new(Mutex::new(None)),
            worker: Some(worker),
        };
        let request = Arc::new(AtomicBool::new(false));
        let cancel_request = request.clone();
        let caller = std::thread::spawn(move || {
            let result = encoder.finish_cancellable(&|| cancel_request.load(Ordering::Acquire));
            result_tx.send(result).unwrap();
        });
        started_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        request.store(true, Ordering::Release);
        stopping_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert!(matches!(
            result_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        release_tx.send(()).unwrap();
        assert!(result_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
            .unwrap_err()
            .contains("cancelled"));
        caller.join().unwrap();
    }
}
