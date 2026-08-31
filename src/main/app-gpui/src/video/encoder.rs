//! H.264/AAC encoding for video export, one hardware-native backend per
//! platform: Media Foundation's sink writer on Windows, VideoToolbox on
//! macOS, and a distribution FFmpeg with libx264 on Linux. Each backend
//! lives on one dedicated thread; callers push plain byte buffers to it over
//! a channel.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

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
    commands: Sender<Command>,
    settings: Settings,
    frames_written: u64,
}

impl Encoder {
    /// Creates the output file. Blocking, so callers run it on the background
    /// executor.
    pub fn create(path: &Path, settings: Settings) -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let owned = path.to_path_buf();

        std::thread::Builder::new()
            .name("video-encoder".into())
            .spawn(move || backend::run(owned, settings, command_rx, ready_tx))
            .map_err(|error| format!("could not start the encoder: {error}"))?;

        ready_rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| "the encoder did not start".to_string())??;

        Ok(Self {
            commands: command_tx,
            settings,
            frames_written: 0,
        })
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
            .send(Command::Audio {
                pcm,
                time,
                duration,
            })
            .map_err(|_| "the encoder stopped".to_string())
    }

    /// Finalizes the file. Consuming the encoder guarantees no frame can be
    /// appended after the container is closed.
    pub fn finish(self) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.commands
            .send(Command::Finish { reply: reply_tx })
            .map_err(|_| "the encoder stopped".to_string())?;
        reply_rx
            .recv_timeout(std::time::Duration::from_secs(120))
            .map_err(|_| "the encoder did not finish".to_string())?
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
                    pump(writer, video_stream, audio_stream, commands);
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
    ) {
        let mut failure = None;
        while let Ok(command) = commands.recv() {
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

        let mut child = None;
        let mut input = None;
        let mut failure = None;
        while let Ok(command) = commands.recv() {
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
                    if child.is_none() {
                        match start(
                            &ffmpeg,
                            &path,
                            settings,
                            settings.has_audio.then_some(audio_path.as_path()),
                        ) {
                            Ok((process, stdin)) => {
                                child = Some(process);
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
                    let result = match (failure.take(), child.take()) {
                        (Some(error), _) => Err(error),
                        (None, Some(process)) => finish(process),
                        (None, None) => Err("no video frames were written".to_string()),
                    };
                    let _ = std::fs::remove_file(&audio_path);
                    let _ = reply.send(result);
                    return;
                }
            }
        }
        if let Some(mut process) = child {
            let _ = process.kill();
            let _ = process.wait();
        }
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

    fn finish(process: Child) -> Result<(), String> {
        let output = process
            .wait_with_output()
            .map_err(|error| format!("FFmpeg wait failed: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if detail.is_empty() {
            format!("FFmpeg exited with {}", output.status)
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
    use std::process::{Child, ChildStdin, Stdio};
    use std::sync::{Arc, Mutex};

    use poratake_daemon_common::ffmpeg;

    /// How long the encoders get to flush and finalize after their inputs
    /// close; a wedged FFmpeg is killed past the deadline.
    const FINALIZE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
    /// 128 kbps AAC, matching the rate the other platforms' mixers assume.
    const AUDIO_BIT_RATE: &str = "128k";

    struct Session {
        ffmpeg: PathBuf,
        child: Child,
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
        fn start(ffmpeg: PathBuf, path: &Path, settings: Settings) -> Result<Self, String> {
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
            let mut child = StdCommand::new(&ffmpeg)
                .args(&args)
                .arg(&video_temp)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|error| format!("could not spawn the export encoder: {error}"))?;
            let stdin = child
                .stdin
                .take()
                .ok_or("the export encoder has no stdin")?;
            let stderr = child
                .stderr
                .take()
                .ok_or("the export encoder has no stderr")?;
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

        /// Closes the inputs, muxes audio in when present, and moves the
        /// result onto the output path.
        fn finalize(self) -> Result<(), String> {
            let Session {
                ffmpeg,
                mut child,
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
            ffmpeg::wait_for_exit(&mut child, FINALIZE_TIMEOUT)
                .then_some(())
                .ok_or_else(|| {
                    format!(
                        "the export encoder did not finish: {}",
                        stderr_tail.lock().unwrap()
                    )
                })?;
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
            let mut mux_error = String::new();
            if let Some(stderr) = mux.stderr.take() {
                use std::io::BufRead;
                for line in std::io::BufReader::new(stderr)
                    .lines()
                    .map_while(Result::ok)
                {
                    if !line.is_empty() {
                        mux_error = line;
                    }
                }
            }
            let finished = ffmpeg::wait_for_exit(&mut mux, FINALIZE_TIMEOUT);
            remove_staged_files(&video_temp, &Some(audio_temp));
            if !finished {
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
    ) {
        let Some(ffmpeg) = ffmpeg::resolve_h264() else {
            let _ = ready.send(Err(
                "video export needs FFmpeg with the libx264 encoder on PATH \
                 (or PORATAKE_FFMPEG_PATH)"
                    .to_string(),
            ));
            return;
        };
        let mut session = match Session::start(ffmpeg, &path, settings) {
            Ok(session) => session,
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };
        let _ = ready.send(Ok(()));

        while let Ok(command) = commands.recv() {
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
                    let _ = reply.send(session.finalize());
                    break;
                }
            };
            if outcome.is_err() {
                // The encode is broken; drain the channel so a dropped
                // Encoder cannot deadlock a caller still sending.
                let _ = session.child.kill();
                let _ = session.child.wait();
                while let Ok(command) = commands.recv() {
                    match command {
                        Command::Finish { reply } => {
                            let _ = reply.send(Err(outcome.err().unwrap_or_default()));
                            break;
                        }
                        Command::Video { bgra, .. } => drop(bgra),
                        Command::Audio { pcm, .. } => drop(pcm),
                    }
                }
                return;
            }
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
            commands,
            settings: Settings {
                width: 4,
                height: 4,
                frame_rate: 30,
                bitrate: 1_000_000,
                has_audio: false,
            },
            frames_written: 0,
        };
        assert!(encoder.write_frame(vec![0; 8]).is_err());
        assert!(encoder.write_frame(vec![0; 64]).is_ok());
    }

    #[test]
    fn frame_times_advance_by_the_frame_duration() {
        let (commands, receiver) = mpsc::channel();
        let mut encoder = Encoder {
            commands,
            settings: Settings {
                width: 2,
                height: 2,
                frame_rate: 25,
                bitrate: 1_000_000,
                has_audio: false,
            },
            frames_written: 0,
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
            commands,
            settings: Settings {
                width: 2,
                height: 2,
                frame_rate: 25,
                bitrate: 1_000_000,
                has_audio: true,
            },
            frames_written: 0,
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
            commands,
            settings: Settings {
                width: 2,
                height: 2,
                frame_rate: 25,
                bitrate: 1_000_000,
                has_audio: true,
            },
            frames_written: 0,
        };
        encoder.write_audio(Vec::new(), 0).expect("empty");
        assert!(receiver.try_recv().is_err());
    }
}
