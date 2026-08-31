//! Video decoding for the editor preview. Media Foundation interfaces are not
//! agile, so the reader lives on one dedicated thread and callers talk to it
//! over a channel; decoded frames are plain byte buffers, which are `Send`.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub frame_rate: f64,
}

impl VideoInfo {
    pub fn is_valid(self) -> bool {
        self.width > 0 && self.height > 0
    }

    pub fn frame_rate(self) -> f64 {
        if self.frame_rate.is_finite() && self.frame_rate > 0.0 {
            return self.frame_rate;
        }
        60.0
    }
}

/// A decoded frame in BGRA, the layout GPUI composites in.
#[derive(Clone)]
pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    pub bgra: Arc<[u8]>,
}

enum Command {
    Seek {
        time: f64,
        reply: Sender<Option<DecodedFrame>>,
    },
}

pub struct VideoDecoder {
    commands: Sender<Command>,
    info: VideoInfo,
}

impl VideoDecoder {
    /// Opens `path`, returning `None` when the file cannot be decoded on this
    /// platform or has no video stream.
    pub fn open(path: &Path) -> Option<Self> {
        let (command_tx, command_rx) = mpsc::channel();
        let (info_tx, info_rx) = mpsc::channel();
        let owned = path.to_path_buf();

        std::thread::Builder::new()
            .name("video-decoder".into())
            .spawn(move || backend::run(owned, command_rx, info_tx))
            .ok()?;

        let info = info_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .ok()??;
        if !info.is_valid() {
            return None;
        }
        Some(Self {
            commands: command_tx,
            info,
        })
    }

    pub fn info(&self) -> VideoInfo {
        self.info
    }

    /// Decodes the frame at `time` seconds. Blocking, so callers run it on the
    /// background executor.
    pub fn frame_at(&self, time: f64) -> Option<DecodedFrame> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.commands
            .send(Command::Seek {
                time: time.max(0.0),
                reply: reply_tx,
            })
            .ok()?;
        reply_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .ok()?
    }
}

#[cfg(windows)]
mod backend {
    use super::*;

    use windows::core::{Interface, GUID, HSTRING, PCWSTR};
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
    use windows::Win32::System::Variant::VT_I8;

    /// `MFTIME` counts 100-nanosecond units.
    const UNITS_PER_SECOND: f64 = 10_000_000.0;

    pub fn run(path: PathBuf, commands: Receiver<Command>, info_tx: Sender<Option<VideoInfo>>) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            if MFStartup(MF_VERSION, MFSTARTUP_FULL).is_err() {
                let _ = info_tx.send(None);
                CoUninitialize();
                return;
            }

            let reader = match open_reader(&path) {
                Some(reader) => reader,
                None => {
                    let _ = info_tx.send(None);
                    let _ = MFShutdown();
                    CoUninitialize();
                    return;
                }
            };

            let info = describe(&reader);
            let _ = info_tx.send(Some(info));
            if !info.is_valid() {
                drop(reader);
                let _ = MFShutdown();
                CoUninitialize();
                return;
            }

            let mut cursor = DecodeCursor::default();
            while let Ok(command) = commands.recv() {
                match command {
                    Command::Seek { time, reply } => {
                        let frame = read_frame(&reader, time, info, &mut cursor);
                        let _ = reply.send(frame);
                    }
                }
            }

            drop(reader);
            let _ = MFShutdown();
            CoUninitialize();
        }
    }

    unsafe fn open_reader(path: &Path) -> Option<IMFSourceReader> {
        let attributes = {
            let mut attributes: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes, 2).ok()?;
            let attributes = attributes?;
            // Lets the reader insert a converter so it can hand back RGB32.
            attributes
                .SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)
                .ok()?;
            attributes
                .SetUINT32(&MF_SOURCE_READER_DISABLE_DXVA, 1)
                .ok()?;
            attributes
        };

        let wide = HSTRING::from(path.as_os_str());
        let reader = MFCreateSourceReaderFromURL(PCWSTR(wide.as_ptr()), &attributes).ok()?;

        reader
            .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
            .ok()?;
        reader
            .SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, true)
            .ok()?;

        let output = MFCreateMediaType().ok()?;
        output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).ok()?;
        output.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).ok()?;
        reader
            .SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, None, &output)
            .ok()?;

        Some(reader)
    }

    unsafe fn describe(reader: &IMFSourceReader) -> VideoInfo {
        let mut info = VideoInfo::default();

        if let Ok(media_type) =
            reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
        {
            if let Ok(packed) = media_type.GetUINT64(&MF_MT_FRAME_SIZE) {
                info.width = (packed >> 32) as u32;
                info.height = (packed & 0xFFFF_FFFF) as u32;
            }
            if let Ok(packed) = media_type.GetUINT64(&MF_MT_FRAME_RATE) {
                let numerator = (packed >> 32) as u32;
                let denominator = (packed & 0xFFFF_FFFF) as u32;
                if denominator > 0 {
                    info.frame_rate = numerator as f64 / denominator as f64;
                }
            }
        }

        if let Ok(value) =
            reader.GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
        {
            let units = value.Anonymous.Anonymous.Anonymous.uhVal;
            info.duration = units as f64 / UNITS_PER_SECOND;
        }

        info
    }

    struct TimedFrame {
        timestamp: i64,
        frame: DecodedFrame,
    }

    #[derive(Default)]
    struct DecodeCursor {
        current: Option<TimedFrame>,
        next: Option<TimedFrame>,
    }

    impl DecodeCursor {
        fn needs_seek(&self, position: i64) -> bool {
            self.current.as_ref().is_none_or(|current| {
                position < current.timestamp
                    || position.saturating_sub(current.timestamp) > UNITS_PER_SECOND as i64 / 2
            })
        }

        fn frame(&self) -> Option<DecodedFrame> {
            self.current
                .as_ref()
                .or(self.next.as_ref())
                .map(|frame| frame.frame.clone())
        }
    }

    fn bounded_time(time: f64, info: VideoInfo) -> f64 {
        if info.duration <= 0.0 {
            return time.max(0.0);
        }
        let last_frame_time = (info.duration - 1.0 / info.frame_rate()).max(0.0);
        time.clamp(0.0, last_frame_time)
    }

    unsafe fn read_frame(
        reader: &IMFSourceReader,
        time: f64,
        info: VideoInfo,
        cursor: &mut DecodeCursor,
    ) -> Option<DecodedFrame> {
        let position = (bounded_time(time, info) * UNITS_PER_SECOND) as i64;
        if cursor.needs_seek(position) {
            let mut variant = PROPVARIANT::default();
            {
                let inner = &mut *variant.Anonymous.Anonymous;
                inner.vt = VT_I8;
                inner.Anonymous.hVal = position;
            }
            reader.SetCurrentPosition(&GUID::zeroed(), &variant).ok()?;
            *cursor = DecodeCursor::default();
        }

        if cursor
            .next
            .as_ref()
            .is_some_and(|next| next.timestamp <= position)
        {
            cursor.current = cursor.next.take();
        }
        if cursor
            .next
            .as_ref()
            .is_some_and(|next| next.timestamp > position)
            || cursor
                .current
                .as_ref()
                .is_some_and(|current| current.timestamp == position)
        {
            return cursor.frame();
        }

        let mut candidate = None;
        loop {
            let mut stream_flags = 0u32;
            let mut timestamp = 0i64;
            let mut sample: Option<IMFSample> = None;
            reader
                .ReadSample(
                    MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                    0,
                    None,
                    Some(&mut stream_flags),
                    Some(&mut timestamp),
                    Some(&mut sample),
                )
                .ok()?;

            if stream_flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                if let Some((timestamp, sample)) = candidate {
                    cursor.current = Some(TimedFrame {
                        timestamp,
                        frame: copy_sample(&sample, info)?,
                    });
                    cursor.next = None;
                }
                return cursor.frame();
            }
            let Some(sample) = sample else {
                continue;
            };
            if timestamp > position {
                if let Some((timestamp, sample)) = candidate {
                    cursor.current = Some(TimedFrame {
                        timestamp,
                        frame: copy_sample(&sample, info)?,
                    });
                }
                cursor.next = Some(TimedFrame {
                    timestamp,
                    frame: copy_sample(&sample, info)?,
                });
                return cursor.frame();
            }
            if timestamp == position {
                cursor.current = Some(TimedFrame {
                    timestamp,
                    frame: copy_sample(&sample, info)?,
                });
                cursor.next = None;
                return cursor.frame();
            }
            candidate = Some((timestamp, sample));
        }
    }

    unsafe fn copy_sample(sample: &IMFSample, info: VideoInfo) -> Option<DecodedFrame> {
        let buffer = sample.ConvertToContiguousBuffer().ok()?;
        let stride = (info.width * 4) as i32;

        // RGB32 frames are bottom-up unless the buffer reports otherwise, so
        // the 2D interface is preferred when the decoder offers it.
        if let Ok(two_d) = buffer.cast::<IMF2DBuffer2>() {
            let mut scanline = std::ptr::null_mut();
            let mut pitch = 0i32;
            let mut start = std::ptr::null_mut();
            let mut length = 0u32;
            two_d
                .Lock2DSize(
                    MF2DBuffer_LockFlags_Read,
                    &mut scanline,
                    &mut pitch,
                    &mut start,
                    &mut length,
                )
                .ok()?;
            let frame = copy_rows(scanline, pitch, info);
            let _ = two_d.Unlock2D();
            return frame;
        }

        let mut data = std::ptr::null_mut();
        let mut max_length = 0u32;
        let mut current_length = 0u32;
        buffer
            .Lock(&mut data, Some(&mut max_length), Some(&mut current_length))
            .ok()?;
        let frame = copy_rows(data, stride, info);
        let _ = buffer.Unlock();
        frame
    }

    /// Copies `info.height` rows, honouring a negative pitch (bottom-up).
    unsafe fn copy_rows(start: *mut u8, pitch: i32, info: VideoInfo) -> Option<DecodedFrame> {
        if start.is_null() || info.width == 0 || info.height == 0 {
            return None;
        }
        let row_bytes = (info.width * 4) as usize;
        let mut bgra = vec![0u8; row_bytes * info.height as usize];

        for row in 0..info.height as usize {
            let source = start.offset(pitch as isize * row as isize);
            let target = &mut bgra[row * row_bytes..(row + 1) * row_bytes];
            std::ptr::copy_nonoverlapping(source, target.as_mut_ptr(), row_bytes);
        }
        // RGB32 has no alpha channel; force it opaque.
        for pixel in bgra.chunks_exact_mut(4) {
            pixel[3] = 255;
        }

        Some(DecodedFrame {
            width: info.width,
            height: info.height,
            bgra: bgra.into(),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn timed_frame(timestamp: i64, value: u8) -> TimedFrame {
            TimedFrame {
                timestamp,
                frame: DecodedFrame {
                    width: 1,
                    height: 1,
                    bgra: vec![value; 4].into(),
                },
            }
        }

        #[test]
        fn forward_playback_keeps_the_frame_before_the_next_timestamp() {
            let cursor = DecodeCursor {
                current: Some(timed_frame(28_000_000, 1)),
                next: Some(timed_frame(30_000_000, 2)),
            };
            assert!(!cursor.needs_seek(29_000_000));
            assert_eq!(cursor.frame().expect("frame").bgra[0], 1);
        }

        #[test]
        fn backward_and_large_forward_jumps_seek() {
            let cursor = DecodeCursor {
                current: Some(timed_frame(10_000_000, 1)),
                next: None,
            };
            assert!(cursor.needs_seek(9_000_000));
            assert!(cursor.needs_seek(16_000_000));
        }

        #[test]
        fn unknown_duration_does_not_clamp_playback_to_zero() {
            let info = VideoInfo {
                duration: 0.0,
                frame_rate: 60.0,
                ..Default::default()
            };
            assert_eq!(bounded_time(2.9, info), 2.9);
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod backend {
    use super::*;
    use std::io::{BufRead as _, BufReader, Read as _};
    use std::process::{Child, Command as ProcessCommand, Stdio};

    struct TimedFrame {
        timestamp: f64,
        frame: DecodedFrame,
    }

    struct MediaInfo {
        video: VideoInfo,
        start_time: f64,
    }

    struct DecoderProcess {
        child: Child,
        frames: Receiver<Option<TimedFrame>>,
        current: Option<TimedFrame>,
        next: Option<TimedFrame>,
    }

    impl Drop for DecoderProcess {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    pub fn run(path: PathBuf, commands: Receiver<Command>, info_tx: Sender<Option<VideoInfo>>) {
        let ffmpeg = crate::video::ffmpeg_path();
        let Some(media) = describe(&ffmpeg, &path) else {
            let _ = info_tx.send(None);
            return;
        };
        let info = media.video;
        let _ = info_tx.send(Some(info));
        let mut decoder = None;
        while let Ok(Command::Seek { time, reply }) = commands.recv() {
            let frame = read_frame(&ffmpeg, &path, time, info, media.start_time, &mut decoder);
            let _ = reply.send(frame);
        }
    }

    fn describe(ffmpeg: &Path, path: &Path) -> Option<MediaInfo> {
        let output = crate::video::command_output(
            ProcessCommand::new(ffmpeg)
                .args(["-hide_banner", "-i"])
                .arg(path)
                .stdin(Stdio::null()),
            std::time::Duration::from_secs(10),
        )
        .ok()?;
        let metadata = String::from_utf8_lossy(&output.stderr);
        let video = metadata.lines().find(|line| line.contains("Video:"))?;
        let (mut width, mut height) = video
            .split(|character: char| character.is_whitespace() || character == ',')
            .find_map(parse_dimensions)?;
        if parse_rotation(&metadata).is_some_and(rotation_swaps_dimensions) {
            std::mem::swap(&mut width, &mut height);
        }
        let words: Vec<_> = video.split_whitespace().collect();
        let frame_rate = words
            .windows(2)
            .find(|pair| pair[1].trim_matches(',') == "fps")
            .and_then(|pair| pair[0].parse::<f64>().ok())
            .unwrap_or(60.0);
        let timing = metadata
            .lines()
            .find_map(|line| line.split_once("Duration: ").map(|(_, value)| value));
        let duration = timing
            .and_then(|value| value.split(',').next())
            .and_then(parse_duration)
            .unwrap_or(0.0);
        let start_time = timing
            .and_then(|value| value.split_once("start: ").map(|(_, value)| value))
            .and_then(|value| value.split(',').next())
            .and_then(|value| value.parse().ok())
            .unwrap_or(0.0);
        Some(MediaInfo {
            video: VideoInfo {
                width,
                height,
                duration,
                frame_rate,
            },
            start_time,
        })
    }

    fn parse_dimensions(value: &str) -> Option<(u32, u32)> {
        let value = value.trim_matches(|character: char| !character.is_ascii_alphanumeric());
        let (width, height) = value.split_once('x')?;
        let width = width.parse().ok()?;
        let height = height.parse().ok()?;
        (width > 0 && height > 0).then_some((width, height))
    }

    fn parse_duration(value: &str) -> Option<f64> {
        let mut parts = value.trim().split(':');
        let hours = parts.next()?.parse::<f64>().ok()?;
        let minutes = parts.next()?.parse::<f64>().ok()?;
        let seconds = parts.next()?.parse::<f64>().ok()?;
        Some(hours * 3600.0 + minutes * 60.0 + seconds)
    }

    fn parse_rotation(metadata: &str) -> Option<f64> {
        metadata.lines().find_map(|line| {
            let (_, rotation) = line.split_once("rotation of ")?;
            rotation.split_whitespace().next()?.parse().ok()
        })
    }

    fn rotation_swaps_dimensions(rotation: f64) -> bool {
        (rotation.abs().rem_euclid(180.0) - 90.0).abs() < 0.5
    }

    fn read_frame(
        ffmpeg: &Path,
        path: &Path,
        time: f64,
        info: VideoInfo,
        start_time: f64,
        decoder: &mut Option<DecoderProcess>,
    ) -> Option<DecodedFrame> {
        let time = time.max(0.0);
        if let Some(decoder) = decoder.as_mut() {
            if decoder
                .next
                .as_ref()
                .is_some_and(|next| next.timestamp <= time)
            {
                decoder.current = decoder.next.take();
            }
        }
        let restart = decoder.as_ref().is_none_or(|decoder| {
            let bracketed = decoder.current.as_ref().is_some_and(|current| {
                current.timestamp <= time
                    && decoder
                        .next
                        .as_ref()
                        .is_some_and(|next| time < next.timestamp)
            });
            !bracketed
                && decoder
                    .current
                    .as_ref()
                    .or(decoder.next.as_ref())
                    .is_some_and(|current| {
                        time < current.timestamp || time - current.timestamp > 2.0
                    })
        });
        let mut process = if restart {
            start(ffmpeg, path, info, start_time, (time - 2.0).max(0.0))?
        } else {
            decoder.take()?
        };
        let frame = read_from_process(
            &mut process,
            time,
            std::time::Instant::now() + std::time::Duration::from_secs(10),
        );
        match frame {
            Ok(frame) => {
                *decoder = Some(process);
                frame
            }
            Err(()) => None,
        }
    }

    fn read_from_process(
        decoder: &mut DecoderProcess,
        time: f64,
        deadline: std::time::Instant,
    ) -> Result<Option<DecodedFrame>, ()> {
        if decoder
            .next
            .as_ref()
            .is_some_and(|next| next.timestamp <= time)
        {
            decoder.current = decoder.next.take();
        }
        if decoder
            .next
            .as_ref()
            .is_some_and(|next| next.timestamp > time)
        {
            return Ok(selected_frame(decoder));
        }
        loop {
            let timeout = deadline
                .checked_duration_since(std::time::Instant::now())
                .ok_or(())?;
            let frame = match decoder.frames.recv_timeout(timeout) {
                Ok(Some(frame)) => frame,
                Ok(None) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Ok(selected_frame(decoder));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => return Err(()),
            };
            if frame.timestamp <= time {
                decoder.current = Some(frame);
                continue;
            }
            decoder.next = Some(frame);
            return Ok(selected_frame(decoder));
        }
    }

    fn selected_frame(decoder: &DecoderProcess) -> Option<DecodedFrame> {
        decoder
            .current
            .as_ref()
            .or(decoder.next.as_ref())
            .map(|frame| frame.frame.clone())
    }

    fn start(
        ffmpeg: &Path,
        path: &Path,
        info: VideoInfo,
        start_time: f64,
        seek_start: f64,
    ) -> Option<DecoderProcess> {
        let frame_size = info.width as usize * info.height as usize * 4;
        let seek_start = seek_start.max(0.0);
        let mut command = ProcessCommand::new(ffmpeg);
        command.args(["-hide_banner", "-loglevel", "info", "-copyts"]);
        if seek_start > 0.0 {
            command.args(["-ss", &format!("{seek_start:.6}"), "-noaccurate_seek"]);
        }
        let mut child = command
            .arg("-i")
            .arg(path)
            .args([
                "-map", "0:v:0", "-an", "-vf", "showinfo", "-vsync", "0", "-f", "rawvideo",
                "-pix_fmt", "bgra", "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .ok()?;
        let Some(mut output) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        };
        let Some(stderr) = child.stderr.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        };
        let (timestamp_tx, timestamp_rx) = mpsc::sync_channel(1);
        if std::thread::Builder::new()
            .name("ffmpeg-timestamp-reader".into())
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let Some(timestamp) = parse_pts_time(&line) else {
                        continue;
                    };
                    if timestamp_tx.send(timestamp).is_err() {
                        return;
                    }
                }
            })
            .is_err()
        {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        let (frame_tx, frames) = mpsc::sync_channel(1);
        if std::thread::Builder::new()
            .name("ffmpeg-frame-reader".into())
            .spawn(move || {
                while let Ok(timestamp) = timestamp_rx.recv() {
                    let mut bytes = vec![0; frame_size];
                    if output.read_exact(&mut bytes).is_err() {
                        break;
                    }
                    if frame_tx
                        .send(Some(TimedFrame {
                            timestamp: timestamp - start_time,
                            frame: DecodedFrame {
                                width: info.width,
                                height: info.height,
                                bgra: bytes.into(),
                            },
                        }))
                        .is_err()
                    {
                        return;
                    }
                }
                let _ = frame_tx.send(None);
            })
            .is_err()
        {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        Some(DecoderProcess {
            child,
            frames,
            current: None,
            next: None,
        })
    }

    fn parse_pts_time(line: &str) -> Option<f64> {
        let (_, timestamp) = line.split_once("pts_time:")?;
        timestamp.split_whitespace().next()?.parse().ok()
    }

    #[cfg(test)]
    mod tests {
        use std::os::unix::fs::PermissionsExt as _;

        use super::*;

        fn fake_ffmpeg() -> tempfile::TempDir {
            let directory = tempfile::tempdir().expect("temp directory");
            let executable = directory.path().join("ffmpeg");
            std::fs::write(
                &executable,
                "#!/bin/sh\ncase \"$*\" in\n  *'showinfo'*'rawvideo'*) printf x >> \"$0.count\"; printf '[Parsed_showinfo] n:0 pts:5 pts_time:5 duration:1\\n' >&2; printf '\\003\\002\\001\\377\\006\\005\\004\\377'; printf '[Parsed_showinfo] n:1 pts:15 pts_time:15 duration:1\\n' >&2; printf '\\011\\010\\007\\377\\014\\013\\012\\377' ;;\n  *) printf 'Duration: 00:00:20.00, start: 5.000000, bitrate: 1 kb/s\\nStream #0:0: Video: h264, bgra, 2x1, 1 fps\\n' >&2; exit 1 ;;\nesac\n",
            )
            .expect("fake FFmpeg");
            let mut permissions = std::fs::metadata(&executable)
                .expect("fake FFmpeg metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(executable, permissions).expect("fake FFmpeg permissions");
            directory
        }

        #[test]
        fn parses_ffmpeg_dimensions() {
            assert_eq!(parse_dimensions("1920x1080"), Some((1920, 1080)));
            assert_eq!(parse_dimensions("0x1080"), None);
        }

        #[test]
        fn parses_ffmpeg_duration() {
            assert_eq!(parse_duration("01:02:03.50"), Some(3723.5));
        }

        #[test]
        fn display_rotation_swaps_video_dimensions() {
            assert_eq!(parse_rotation("rotation of -90.00 degrees"), Some(-90.0));
            assert!(rotation_swaps_dimensions(-90.0));
            assert!(!rotation_swaps_dimensions(180.0));
        }

        #[test]
        fn unix_ffmpeg_process_describes_and_decodes_bgra() {
            let directory = fake_ffmpeg();
            let ffmpeg = directory.path().join("ffmpeg");
            let source = directory.path().join("source.mp4");
            std::fs::write(&source, []).expect("source file");
            let media = describe(&ffmpeg, &source).expect("video metadata");
            assert_eq!(
                media.video,
                VideoInfo {
                    width: 2,
                    height: 1,
                    duration: 20.0,
                    frame_rate: 1.0,
                }
            );
            assert_eq!(media.start_time, 5.0);

            let mut decoder = None;
            let first = read_frame(
                &ffmpeg,
                &source,
                0.25,
                media.video,
                media.start_time,
                &mut decoder,
            )
            .expect("first video frame");
            assert_eq!(first.bgra.as_ref(), [3, 2, 1, 255, 6, 5, 4, 255]);
            let gap = read_frame(
                &ffmpeg,
                &source,
                5.0,
                media.video,
                media.start_time,
                &mut decoder,
            )
            .expect("sparse video gap");
            assert_eq!(gap.bgra.as_ref(), first.bgra.as_ref());
            assert_eq!(
                std::fs::read_to_string(directory.path().join("ffmpeg.count"))
                    .expect("FFmpeg launch count"),
                "x"
            );
            let second = read_frame(
                &ffmpeg,
                &source,
                10.0,
                media.video,
                media.start_time,
                &mut decoder,
            )
            .expect("second video frame");
            assert_eq!(second.bgra.as_ref(), [9, 8, 7, 255, 12, 11, 10, 255]);

            let random = read_frame(
                &ffmpeg,
                &source,
                5.0,
                media.video,
                media.start_time,
                &mut None,
            )
            .expect("random seek video frame");
            assert_eq!(random.bgra.as_ref(), first.bgra.as_ref());
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod backend {
    use super::*;

    pub fn run(_path: PathBuf, commands: Receiver<Command>, info_tx: Sender<Option<VideoInfo>>) {
        let _ = info_tx.send(None);
        while let Ok(Command::Seek { time, reply }) = commands.recv() {
            let _ = time;
            drop(reply);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unreadable_file_yields_no_decoder() {
        let missing = std::env::temp_dir().join("poratake-missing-video.mov");
        assert!(VideoDecoder::open(&missing).is_none());
    }

    #[test]
    fn info_needs_both_dimensions() {
        assert!(!VideoInfo::default().is_valid());
        assert!(VideoInfo {
            width: 1920,
            height: 1080,
            duration: 3.0,
            frame_rate: 60.0,
        }
        .is_valid());
        assert!(!VideoInfo {
            width: 1920,
            height: 0,
            duration: 3.0,
            frame_rate: 60.0,
        }
        .is_valid());
    }

    #[test]
    fn missing_frame_rate_defaults_to_sixty() {
        assert_eq!(VideoInfo::default().frame_rate(), 60.0);
        assert_eq!(
            VideoInfo {
                frame_rate: 120.0,
                ..Default::default()
            }
            .frame_rate(),
            120.0
        );
    }
}
