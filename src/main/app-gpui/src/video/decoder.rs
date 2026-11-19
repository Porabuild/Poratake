//! Video decoding for the editor preview. Media Foundation interfaces are not
//! agile, so the reader lives on one dedicated thread and callers talk to it
//! over a channel; decoded frames are plain byte buffers, which are `Send`.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    pub bgra: Arc<Vec<u8>>,
}

pub(crate) type FrameCancellation = smol::channel::Receiver<()>;

async fn wait_for_cancellation(signal: Option<&FrameCancellation>) {
    if let Some(signal) = signal {
        let _ = signal.recv().await;
    } else {
        std::future::pending().await
    }
}

const STREAM_SEEK_PREROLL: f64 = 2.0;

pub(crate) fn needs_stream_seek(previous: f64, requested: f64) -> bool {
    requested < previous || requested - previous > STREAM_SEEK_PREROLL
}

enum FramePosition {
    First,
    Time(f64),
}

struct Command {
    position: FramePosition,
    reply: Sender<Option<DecodedFrame>>,
    cancelled: Option<FrameCancellation>,
    abandoned: FrameCancellation,
}

struct PendingFrame {
    reply: Receiver<Option<DecodedFrame>>,
    _waiting: smol::channel::Sender<()>,
}

impl PendingFrame {
    fn wait(self) -> Option<DecodedFrame> {
        self.reply
            .recv_timeout(std::time::Duration::from_secs(10))
            .ok()?
    }
}

pub struct VideoDecoder {
    commands: Sender<Command>,
    info: VideoInfo,
}

pub(crate) struct DecoderOpening {
    commands: Sender<Command>,
    info: smol::channel::Receiver<Option<VideoInfo>>,
    deadline: Instant,
}

impl DecoderOpening {
    fn finish(self) -> Option<VideoDecoder> {
        self.finish_interruptible(None, None).ok().flatten()
    }

    pub(crate) fn finish_interruptible(
        self,
        cancelled: Option<&FrameCancellation>,
        superseded: Option<&FrameCancellation>,
    ) -> Result<Option<VideoDecoder>, Self> {
        let ready = smol::future::or(
            async { self.info.recv().await.ok().flatten() },
            async {
                smol::Timer::at(self.deadline).await;
                None
            },
        );
        let info = smol::block_on(smol::future::or(
            async {
                wait_for_cancellation(cancelled).await;
                Err(())
            },
            smol::future::or(async { Ok(ready.await) }, async {
                wait_for_cancellation(superseded).await;
                Err(())
            }),
        ));
        match info {
            Ok(info) => Ok(info.filter(|info| info.is_valid()).map(|info| VideoDecoder {
                commands: self.commands,
                info,
            })),
            Err(()) => Err(self),
        }
    }

    pub(crate) fn request_warmup(
        &self,
        time: f64,
        cancelled: FrameCancellation,
    ) -> Option<Receiver<()>> {
        request_warmup(&self.commands, time, cancelled)
    }
}

impl VideoDecoder {
    /// Opens `path`, returning `None` when the file cannot be decoded on this
    /// platform or has no video stream.
    pub fn open(path: &Path) -> Option<Self> {
        Self::begin_open(path)?.finish()
    }

    pub(crate) fn begin_open(path: &Path) -> Option<DecoderOpening> {
        let (commands, command_rx) = mpsc::channel();
        let (info_tx, info) = smol::channel::bounded(1);
        let owned = path.to_path_buf();
        let deadline = Instant::now() + Duration::from_secs(10);

        std::thread::Builder::new()
            .name("video-decoder".into())
            .spawn(move || backend::run(owned, command_rx, info_tx))
            .ok()?;

        Some(DecoderOpening {
            commands,
            info,
            deadline,
        })
    }

    pub fn info(&self) -> VideoInfo {
        self.info
    }

    /// Decodes the frame at `time` seconds. Blocking, so callers run it on the
    /// background executor.
    pub fn frame_at(&self, time: f64) -> Option<DecodedFrame> {
        self.frame_at_interruptible(time, None)
    }

    pub(crate) fn first_frame(&self) -> Option<DecodedFrame> {
        request_frame(&self.commands, FramePosition::First, None)?.wait()
    }

    pub(crate) fn frame_at_interruptible(
        &self,
        time: f64,
        cancelled: Option<&FrameCancellation>,
    ) -> Option<DecodedFrame> {
        request_frame(
            &self.commands,
            FramePosition::Time(time.max(0.0)),
            cancelled.cloned(),
        )?
        .wait()
    }

    pub(crate) fn request_warmup(
        &self,
        time: f64,
        cancelled: FrameCancellation,
    ) -> Option<Receiver<()>> {
        request_warmup(&self.commands, time, cancelled)
    }
}

fn request_frame(
    commands: &Sender<Command>,
    position: FramePosition,
    cancelled: Option<FrameCancellation>,
) -> Option<PendingFrame> {
    let (reply, received) = mpsc::channel();
    let (waiting, abandoned) = smol::channel::bounded(1);
    commands
        .send(Command {
            position,
            reply,
            cancelled,
            abandoned,
        })
        .ok()?;
    Some(PendingFrame {
        reply: received,
        _waiting: waiting,
    })
}

fn request_warmup(
    commands: &Sender<Command>,
    time: f64,
    cancelled: FrameCancellation,
) -> Option<Receiver<()>> {
    let pending = request_frame(commands, FramePosition::Time(time.max(0.0)), Some(cancelled))?;
    let (reply, completion) = mpsc::channel();
    smol::unblock(move || {
        let _ = pending.wait();
        let _ = reply.send(());
    })
    .detach();
    Some(completion)
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

    pub fn run(
        path: PathBuf,
        commands: Receiver<Command>,
        info_tx: smol::channel::Sender<Option<VideoInfo>>,
    ) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            if MFStartup(MF_VERSION, MFSTARTUP_FULL).is_err() {
                let _ = info_tx.try_send(None);
                CoUninitialize();
                return;
            }

            let reader = match open_reader(&path) {
                Some(reader) => reader,
                None => {
                    let _ = info_tx.try_send(None);
                    let _ = MFShutdown();
                    CoUninitialize();
                    return;
                }
            };

            let info = describe(&reader);
            let _ = info_tx.try_send(Some(info));
            if !info.is_valid() {
                drop(reader);
                let _ = MFShutdown();
                CoUninitialize();
                return;
            }

            let mut cursor = DecodeCursor::default();
            while let Ok(Command {
                position,
                reply,
                cancelled,
                abandoned,
            }) = commands.recv()
            {
                if abandoned.is_closed() || cancelled.as_ref().is_some_and(FrameCancellation::is_closed) {
                    let _ = reply.send(None);
                    continue;
                }
                let time = match position {
                    FramePosition::First => 0.0,
                    FramePosition::Time(time) => time,
                };
                let frame = read_frame(&reader, time, info, &mut cursor);
                let _ = reply.send(frame);
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
        for pixel in bgra.as_chunks_mut::<4>().0 {
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
    use std::io::{BufRead as _, BufReader, Read};
    use std::process::{Child, Command as ProcessCommand, Stdio};

    struct TimedFrame {
        timestamp: f64,
        frame: DecodedFrame,
    }

    #[derive(Clone, Copy, Default)]
    struct MediaInfo {
        video: VideoInfo,
        start_time: f64,
        normalize_pixel_aspect: bool,
    }

    struct DecoderProcess {
        child: Child,
        frames: smol::channel::Receiver<Option<TimedFrame>>,
        spare_frame: smol::channel::Sender<Vec<u8>>,
        seek_start: f64,
        fallback_seek_start: Option<f64>,
        current: Option<TimedFrame>,
        next: Option<TimedFrame>,
    }

    impl DecoderProcess {
        fn advance(&mut self, frame: TimedFrame) {
            if let Some(previous) = self.current.replace(frame) {
                if let Ok(bytes) = Arc::try_unwrap(previous.frame.bgra) {
                    let _ = self.spare_frame.try_send(bytes);
                }
            }
        }
    }

    impl Drop for DecoderProcess {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    pub fn run(
        path: PathBuf,
        commands: Receiver<Command>,
        info_tx: smol::channel::Sender<Option<VideoInfo>>,
    ) {
        let ffmpeg = crate::video::ffmpeg_path();
        let Some(media) = describe(&ffmpeg, &path) else {
            let _ = info_tx.try_send(None);
            return;
        };
        let _ = info_tx.try_send(Some(media.video));
        let mut decoder = None;
        while let Ok(Command {
            position,
            reply,
            cancelled,
            abandoned,
        }) = commands.recv()
        {
            let wait = FrameWait {
                cancelled: cancelled.as_ref(),
                abandoned: Some(&abandoned),
            };
            let frame = match position {
                FramePosition::First => read_first_frame(&ffmpeg, &path, media, &mut decoder, wait),
                FramePosition::Time(time) => {
                    read_frame(&ffmpeg, &path, time, media, &mut decoder, wait)
                }
            };
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
        let mut stream = metadata.lines().skip_while(|line| !line.contains("Video:"));
        let video = stream.next()?;
        let (mut width, mut height) = video
            .split(|character: char| character.is_whitespace() || character == ',')
            .find_map(parse_dimensions)?;
        let aspect = sample_aspect(video).unwrap_or((1, 1));
        let normalize_pixel_aspect = aspect.0 != aspect.1;
        (width, height) = square_pixel_size(width, height, aspect)?;
        let rotation = stream
            .take_while(|line| !line.trim_start().starts_with("Stream #"))
            .find_map(parse_rotation);
        if rotation.is_some_and(rotation_swaps_dimensions) {
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
            normalize_pixel_aspect,
        })
    }

    fn sample_aspect(video: &str) -> Option<(u32, u32)> {
        let (_, ratio) = video.split_once("[SAR ")?;
        let (num, den) = ratio
            .split_whitespace()
            .next()?
            .trim_end_matches(']')
            .split_once(':')?;
        let (num, den) = (num.parse().ok()?, den.parse().ok()?);
        (num > 0 && den > 0).then_some((num, den))
    }

    fn square_pixel_size(width: u32, height: u32, (num, den): (u32, u32)) -> Option<(u32, u32)> {
        if num == den {
            return Some((width, height));
        }
        let expand = |size: u32, num: u32, den: u32| {
            let rounded = (u64::from(size) * u64::from(num) + u64::from(den / 2)) / u64::from(den);
            u32::try_from(rounded).ok()
        };
        let (width, height) = if num > den {
            (expand(width, num, den)?, height)
        } else {
            (width, expand(height, den, num)?)
        };
        let frame_bytes = u64::from(width)
            .checked_mul(u64::from(height))?
            .checked_mul(4)?;
        (frame_bytes <= image::Limits::default().max_alloc.unwrap_or(u64::MAX))
            .then_some((width, height))
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

    #[derive(Clone, Copy, Default)]
    struct FrameWait<'a> {
        cancelled: Option<&'a FrameCancellation>,
        abandoned: Option<&'a FrameCancellation>,
    }

    impl FrameWait<'_> {
        fn is_interrupted(self) -> bool {
            self.cancelled.is_some_and(FrameCancellation::is_closed)
                || self.abandoned.is_some_and(FrameCancellation::is_closed)
        }

        async fn interrupted(self) -> FrameReadError {
            smol::future::or(
                async {
                    wait_for_cancellation(self.abandoned).await;
                    FrameReadError::TimedOut
                },
                async {
                    wait_for_cancellation(self.cancelled).await;
                    FrameReadError::Cancelled
                },
            )
            .await
        }
    }

    fn read_first_frame(
        ffmpeg: &Path,
        path: &Path,
        media: MediaInfo,
        decoder: &mut Option<DecoderProcess>,
        wait: FrameWait<'_>,
    ) -> Option<DecodedFrame> {
        if wait.is_interrupted() {
            return None;
        }
        drop(decoder.take());
        let mut process = start(ffmpeg, path, media, 0.0)?;
        let frame = smol::block_on(wait_for_first_frame(&mut process, wait))?;
        *decoder = Some(process);
        Some(frame)
    }

    async fn wait_for_first_frame(
        decoder: &mut DecoderProcess,
        wait: FrameWait<'_>,
    ) -> Option<DecodedFrame> {
        let frame = smol::future::or(
            async {
                wait.interrupted().await;
                None
            },
            async { decoder.frames.recv().await.ok().flatten() },
        )
        .await?;
        decoder.next = Some(frame);
        selected_frame(decoder)
    }

    fn read_frame(
        ffmpeg: &Path,
        path: &Path,
        time: f64,
        media: MediaInfo,
        decoder: &mut Option<DecoderProcess>,
        wait: FrameWait<'_>,
    ) -> Option<DecodedFrame> {
        if wait.is_interrupted() {
            return None;
        }
        let time = time.max(0.0).next_up();
        if let Some(decoder) = decoder.as_mut() {
            if let Some(next) = decoder.next.take_if(|next| next.timestamp <= time) {
                decoder.advance(next);
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
            let position = decoder
                .current
                .as_ref()
                .or(decoder.next.as_ref())
                .map_or(decoder.seek_start, |frame| frame.timestamp);
            !bracketed && needs_stream_seek(position, time)
        });
        let mut process = if restart {
            drop(decoder.take());
            let preroll_start = (time - STREAM_SEEK_PREROLL).max(0.0);
            let seek_start = if preroll_start > 0.0 { time } else { 0.0 };
            let mut process = start(ffmpeg, path, media, seek_start)?;
            process.fallback_seek_start = (preroll_start > 0.0).then_some(preroll_start);
            process
        } else {
            decoder.take()?
        };
        let mut frame = smol::block_on(wait_for_frame(&mut process, time, wait));
        if process.current.is_some() {
            process.fallback_seek_start = None;
        }
        if let Some(preroll_start) = process.fallback_seek_start.filter(|_| frame.is_ok()) {
            drop(process);
            process = start(ffmpeg, path, media, preroll_start)?;
            frame = smol::block_on(wait_for_frame(&mut process, time, wait));
        }
        if matches!(frame, Ok(Some(_)) | Err(FrameReadError::Cancelled)) {
            *decoder = Some(process);
        }
        frame.ok().flatten()
    }

    #[derive(Debug)]
    enum FrameReadError {
        Cancelled,
        TimedOut,
    }

    async fn wait_for_frame(
        decoder: &mut DecoderProcess,
        time: f64,
        wait: FrameWait<'_>,
    ) -> Result<Option<DecodedFrame>, FrameReadError> {
        if let Some(next) = decoder.next.take_if(|next| next.timestamp <= time) {
            decoder.advance(next);
        }
        if decoder
            .next
            .as_ref()
            .is_some_and(|next| next.timestamp > time)
        {
            return Ok(selected_frame(decoder));
        }

        let mut interrupt = std::pin::pin!(async { Err(wait.interrupted().await) });
        while let Some(frame) = smol::future::or(interrupt.as_mut(), async {
            Ok(decoder.frames.recv().await.ok().flatten())
        })
        .await?
        {
            if frame.timestamp <= time {
                decoder.advance(frame);
                continue;
            }
            decoder.next = Some(frame);
            break;
        }
        Ok(selected_frame(decoder))
    }

    fn selected_frame(decoder: &DecoderProcess) -> Option<DecodedFrame> {
        decoder
            .current
            .as_ref()
            .or(decoder.next.as_ref())
            .map(|frame| frame.frame.clone())
    }

    #[cfg(target_os = "macos")]
    const FRAME_SOCKET_BUFFER: usize = 1024 * 1024;

    #[cfg(target_os = "macos")]
    const MAX_FRAME_SOCKET_SEND_BUFFER: usize = 4 * 1024 * 1024;

    #[cfg(target_os = "macos")]
    fn frame_socket(frame_size: usize) -> Option<(std::os::unix::net::UnixStream, Stdio)> {
        use std::ffi::{c_int, c_void};
        use std::os::fd::{AsRawFd, OwnedFd};
        use std::os::unix::net::UnixStream;

        unsafe extern "C" {
            fn setsockopt(
                socket: c_int,
                level: c_int,
                option: c_int,
                value: *const c_void,
                length: u32,
            ) -> c_int;
        }

        const SOL_SOCKET: c_int = 0xffff;
        const SO_SNDBUF: c_int = 0x1001;
        const SO_RCVBUF: c_int = 0x1002;

        if frame_size <= FRAME_SOCKET_BUFFER {
            return None;
        }
        let (reader, writer) = UnixStream::pair().ok()?;
        let configure_buffer = |fd, option, capacity: usize| {
            let capacity = capacity as c_int;
            unsafe {
                setsockopt(
                    fd,
                    SOL_SOCKET,
                    option,
                    (&capacity as *const c_int).cast(),
                    std::mem::size_of::<c_int>() as u32,
                ) == 0
            }
        };
        if !configure_buffer(reader.as_raw_fd(), SO_RCVBUF, FRAME_SOCKET_BUFFER) {
            return None;
        }
        let capacity = frame_size.min(MAX_FRAME_SOCKET_SEND_BUFFER);
        let writer_fd = writer.as_raw_fd();
        if !configure_buffer(writer_fd, SO_SNDBUF, capacity)
            && !configure_buffer(writer_fd, SO_SNDBUF, FRAME_SOCKET_BUFFER)
        {
            return None;
        }
        Some((reader, Stdio::from(OwnedFd::from(writer))))
    }

    fn spawn_frame_reader(
        mut command: ProcessCommand,
        _frame_size: usize,
    ) -> Option<(Child, Box<dyn Read + Send>)> {
        #[cfg(target_os = "macos")]
        if let Some((reader, stdout)) = frame_socket(_frame_size) {
            if let Ok(child) = command.stdout(stdout).spawn() {
                return Some((child, Box::new(reader)));
            }
        }

        let mut child = command.stdout(Stdio::piped()).spawn().ok()?;
        if let Some(output) = child.stdout.take() {
            return Some((child, Box::new(output)));
        }
        let _ = child.kill();
        let _ = child.wait();
        None
    }

    fn start(
        ffmpeg: &Path,
        path: &Path,
        media: MediaInfo,
        seek_start: f64,
    ) -> Option<DecoderProcess> {
        let MediaInfo {
            video: info,
            start_time,
            normalize_pixel_aspect,
        } = media;
        let frame_size = info.width as usize * info.height as usize * 4;
        let filter = if normalize_pixel_aspect {
            format!(
                "scale={}:{},setsar=1,showinfo=checksum=0",
                info.width, info.height
            )
        } else {
            "showinfo=checksum=0".to_owned()
        };
        let seek_start = seek_start.max(0.0);
        let mut command = ProcessCommand::new(ffmpeg);
        command.args(["-hide_banner", "-loglevel", "info", "-copyts"]);
        if seek_start > 0.0 {
            command.args(["-ss", &format!("{seek_start:.6}"), "-noaccurate_seek"]);
        }
        command
            .arg("-i")
            .arg(path)
            .args([
                "-map",
                "0:v:0",
                "-an",
                "-vf",
                &filter,
                "-fps_mode",
                "passthrough",
                "-threads:v",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "bgra",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stderr(Stdio::piped());
        let (mut child, mut output) = spawn_frame_reader(command, frame_size)?;
        let Some(stderr) = child.stderr.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        };
        let (timestamp_tx, timestamp_rx) = mpsc::sync_channel(1);
        if std::thread::Builder::new()
            .name("ffmpeg-timestamp-reader".into())
            .spawn(move || {
                let mut time_base = None;
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if let Some(parsed) = parse_time_base(&line) {
                        time_base = Some(parsed);
                        continue;
                    }
                    let Some(timestamp) = parse_pts_time(&line, time_base) else {
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
        #[cfg(all(feature = "parity", target_os = "macos"))]
        let trace_pid = std::env::var_os("PORATAKE_NATIVE_SMOKE_VIDEO_PLAYBACK")
            .map(|_| child.id());
        let (frame_tx, frames) = smol::channel::bounded(1);
        let (spare_frame, reusable) = smol::channel::bounded(1);
        if std::thread::Builder::new()
            .name("ffmpeg-frame-reader".into())
            .spawn(move || {
                loop {
                    #[cfg(all(feature = "parity", target_os = "macos"))]
                    let timestamp_started = trace_pid.map(|_| std::time::Instant::now());
                    let Ok(timestamp) = timestamp_rx.recv() else {
                        break;
                    };
                    #[cfg(all(feature = "parity", target_os = "macos"))]
                    let timestamp_received = trace_pid.map(|_| std::time::Instant::now());
                    let bytes = reusable.try_recv();
                    #[cfg(all(feature = "parity", target_os = "macos"))]
                    let reused = bytes.is_ok();
                    let mut bytes = bytes.unwrap_or_else(|_| vec![0; frame_size]);
                    let payload = bytes.as_mut_slice();
                    #[cfg(all(feature = "parity", target_os = "macos"))]
                    let (payload, payload_phases) = if trace_pid.is_some() {
                        let allocated = std::time::Instant::now();
                        let (first, rest) = payload.split_at_mut(1);
                        if output.read_exact(first).is_err() {
                            break;
                        }
                        (rest, Some((allocated, std::time::Instant::now())))
                    } else {
                        (payload, None)
                    };
                    if output.read_exact(payload).is_err() {
                        break;
                    }
                    #[cfg(all(feature = "parity", target_os = "macos"))]
                    if let (
                        Some(pid),
                        Some(started),
                        Some(received),
                        Some((allocated, first_byte)),
                    ) = (
                        trace_pid,
                        timestamp_started,
                        timestamp_received,
                        payload_phases,
                    ) {
                        let finished = std::time::Instant::now();
                        println!(
                            "VIDEO_SMOKE pipe {pid} {:.6} {:.3} {:.3} {:.3} {:.3} {:.3} {reused}",
                            timestamp - start_time,
                            received.duration_since(started).as_secs_f64() * 1000.,
                            finished.duration_since(received).as_secs_f64() * 1000.,
                            allocated.duration_since(received).as_secs_f64() * 1000.,
                            first_byte.duration_since(allocated).as_secs_f64() * 1000.,
                            finished.duration_since(first_byte).as_secs_f64() * 1000.,
                        );
                    }
                    if frame_tx
                        .send_blocking(Some(TimedFrame {
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
                let _ = frame_tx.send_blocking(None);
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
            spare_frame,
            seek_start,
            fallback_seek_start: None,
            current: None,
            next: None,
        })
    }

    fn parse_time_base(line: &str) -> Option<(u64, u64)> {
        let (_, base) = line.split_once("config in time_base: ")?;
        let base = base.split(',').next()?;
        let (numerator, denominator) = base.split_once('/')?;
        let numerator = numerator.trim().parse::<u64>().ok()?;
        let denominator = denominator.trim().parse::<u64>().ok()?;
        (numerator > 0 && denominator > 0).then_some((numerator, denominator))
    }

    fn parse_pts_time(line: &str, time_base: Option<(u64, u64)>) -> Option<f64> {
        if let Some((numerator, denominator)) = time_base {
            let pts = line.split_once(" pts:")?.1.split_whitespace().next()?;
            if let Ok(pts) = pts.parse::<i64>() {
                return Some(pts as f64 * numerator as f64 / denominator as f64);
            }
        }
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
                r#"#!/bin/sh
case "$*" in
  *'-i'*'showinfo=checksum=0'*'-threads:v 1'*'rawvideo'*)
    if [ -f "$0.previous" ]; then
      read -r previous < "$0.previous"
      if kill -0 "$previous" 2>/dev/null; then exit 1; fi
    fi
    printf x >> "$0.count"
    if [ -f "$0.fail-once" ]; then
      rm "$0.fail-once"
      exit 1
    fi
    printf '[Parsed_showinfo] n:0 pts:5 pts_time:5 duration:1\n' >&2
    printf '\003\002\001\377\006\005\004\377'
    printf '[Parsed_showinfo] n:1 pts:15 pts_time:15 duration:1\n' >&2
    printf '\011\010\007\377\014\013\012\377'
    ;;
  *)
    printf 'Duration: 00:00:20.00, start: 5.000000, bitrate: 1 kb/s\nStream #0:0: Video: h264, bgra, 2x1, 1 fps\n' >&2
    exit 1
    ;;
esac
"#,
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
        fn superseded_frame_wait_leaves_the_decoder_available() {
            let (frames_tx, frames) = smol::channel::bounded(1);
            let mut decoder = DecoderProcess {
                child: ProcessCommand::new("/bin/sh").args(["-c", ":"]).spawn().unwrap(),
                frames,
                spare_frame: smol::channel::bounded(1).0,
                seek_start: 0.0,
                fallback_seek_start: None,
                current: None,
                next: None,
            };
            let (cancel, cancelled) = smol::channel::bounded(1);
            smol::block_on(async {
                let wait = FrameWait {
                    cancelled: Some(&cancelled),
                    abandoned: None,
                };
                let mut pending = std::pin::pin!(wait_for_frame(&mut decoder, 1.0, wait));
                assert!(smol::future::poll_once(pending.as_mut()).await.is_none());
                cancel.close();
                assert!(matches!(pending.await, Err(FrameReadError::Cancelled)));
            });

            frames_tx
                .send_blocking(Some(TimedFrame {
                    timestamp: 0.5,
                    frame: DecodedFrame {
                        width: 1,
                        height: 1,
                        bgra: vec![10, 20, 30, 255].into(),
                    },
                }))
                .unwrap();
            let frame = smol::block_on(wait_for_frame(&mut decoder, 0.0, FrameWait::default()))
                .unwrap()
                .unwrap();
            assert_eq!(frame.bgra.as_slice(), [10, 20, 30, 255]);

            let (_, reply) = mpsc::channel();
            let (waiting, abandoned) = smol::channel::bounded(1);
            let caller = PendingFrame {
                reply,
                _waiting: waiting,
            };
            smol::block_on(async {
                let wait = FrameWait {
                    cancelled: None,
                    abandoned: Some(&abandoned),
                };
                let mut pending = std::pin::pin!(wait_for_frame(&mut decoder, 1.0, wait));
                assert!(smol::future::poll_once(pending.as_mut()).await.is_none());
                drop(caller);
                assert!(matches!(pending.await, Err(FrameReadError::TimedOut)));
            });
        }

        #[test]
        fn first_frame_is_available_before_lookahead_and_kept_for_playback() {
            let (frames_tx, frames) = smol::channel::bounded(1);
            let mut decoder = DecoderProcess {
                child: ProcessCommand::new("/usr/bin/true").spawn().unwrap(),
                frames,
                spare_frame: smol::channel::bounded(1).0,
                seek_start: 0.0,
                fallback_seek_start: None,
                current: None,
                next: None,
            };
            frames_tx
                .send_blocking(Some(TimedFrame {
                    timestamp: 0.0,
                    frame: DecodedFrame {
                        width: 1,
                        height: 1,
                        bgra: vec![10, 20, 30, 255].into(),
                    },
                }))
                .unwrap();
            smol::block_on(async {
                let mut first =
                    std::pin::pin!(wait_for_first_frame(&mut decoder, FrameWait::default()));
                let frame = smol::future::poll_once(first.as_mut())
                    .await
                    .expect("poster can be decoded while the second frame is unavailable")
                    .unwrap();
                assert_eq!(frame.bgra.as_slice(), [10, 20, 30, 255]);
            });
            assert!(decoder.current.is_none());
            assert_eq!(decoder.next.as_ref().unwrap().timestamp, 0.0);
            smol::block_on(async {
                let mut pending =
                    std::pin::pin!(wait_for_frame(&mut decoder, 0.0, FrameWait::default()));
                assert!(smol::future::poll_once(pending.as_mut()).await.is_none());
                frames_tx.close();
                let frame = pending.await.unwrap().unwrap();
                assert_eq!(frame.bgra.as_slice(), [10, 20, 30, 255]);
            });
        }

        #[test]
        fn decoder_reuses_only_unshared_frame_storage() {
            let (spare_frame, reusable) = smol::channel::bounded(1);
            let mut decoder = DecoderProcess {
                child: ProcessCommand::new("/usr/bin/true").spawn().unwrap(),
                frames: smol::channel::bounded(1).1,
                spare_frame,
                seek_start: 0.0,
                fallback_seek_start: None,
                current: None,
                next: None,
            };
            let frame = |timestamp, byte| TimedFrame {
                timestamp,
                frame: DecodedFrame {
                    width: 1,
                    height: 1,
                    bgra: vec![byte, 0, 0, 255].into(),
                },
            };
            decoder.advance(frame(0.0, 10));
            let held = selected_frame(&decoder).unwrap();
            decoder.advance(frame(1.0, 20));
            assert!(reusable.try_recv().is_err());
            assert_eq!(held.bgra[0], 10);

            let current = decoder.current.as_ref().unwrap();
            let cached = Arc::downgrade(&current.frame.bgra);
            let address = current.frame.bgra.as_ptr();
            decoder.advance(frame(2.0, 30));
            let bytes = reusable.try_recv().unwrap();
            assert_eq!(bytes.as_ptr(), address);
            assert_eq!(bytes.as_slice(), [20, 0, 0, 255]);
            assert!(cached.upgrade().is_none());
            assert_eq!(selected_frame(&decoder).unwrap().bgra[0], 30);
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn large_frame_reader_delivers_stdout_then_eof() {
            let mut command = ProcessCommand::new("/usr/bin/printf");
            command.arg("frame");
            let (mut child, mut output) =
                spawn_frame_reader(command, MAX_FRAME_SOCKET_SEND_BUFFER).unwrap();
            let mut bytes = Vec::new();
            output.read_to_end(&mut bytes).unwrap();
            assert!(child.wait().unwrap().success());
            assert_eq!(bytes, b"frame");
        }

        #[test]
        fn parses_ffmpeg_dimensions() {
            assert_eq!(parse_dimensions("1920x1080"), Some((1920, 1080)));
            assert_eq!(parse_dimensions("0x1080"), None);
        }

        #[test]
        #[ignore = "requires development FFmpeg"]
        fn sparse_camera_timeline_decodes_at_main_video_times() {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("camera.mov");
            std::fs::write(
                &source,
                include_bytes!("../ui/parity/fixtures/camera-delayed-toggle.mov"),
            )
            .unwrap();
            let ffmpeg = crate::video::ffmpeg_path();
            let media = describe(&ffmpeg, &source).expect("camera metadata");
            assert_eq!(media.start_time, 0.0);
            let mut decoder = None;
            for (time, expected_red) in [
                (10.1, 37_u8),
                (11.1, 71),
                (16.1, 145),
                (17.1, 219),
                (19.0, 108),
            ] {
                let frame = read_frame(
                    &ffmpeg,
                    &source,
                    time,
                    media,
                    &mut decoder,
                    FrameWait::default(),
                )
                .expect("camera frame");
                let red = frame.bgra[(32 * 64 + 32) * 4 + 2];
                assert!(red.abs_diff(expected_red) <= 15, "red={red} at {time}s");
            }
        }

        #[test]
        #[ignore = "requires development FFmpeg"]
        fn sparse_screen_timeline_seeks_through_pause_and_end() {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("screen.mov");
            std::fs::write(
                &source,
                include_bytes!("../ui/parity/fixtures/sparse-screen-pause.mov"),
            )
            .unwrap();
            let ffmpeg = crate::video::ffmpeg_path();
            let media = describe(&ffmpeg, &source).expect("screen metadata");
            assert!((media.video.duration - 31.0).abs() < 0.01);
            let mut decoder = None;
            for (time, expected) in [
                (0.0, 34_u8),
                (12.0, 34),
                (29.5, 34),
                (30.1, 119),
                (30.98, 119),
                (8.0, 34),
            ] {
                let frame = read_frame(
                    &ffmpeg,
                    &source,
                    time,
                    media,
                    &mut decoder,
                    FrameWait::default(),
                )
                .expect("screen frame");
                let channel = frame.bgra[(32 * 64 + 32) * 4 + 2];
                assert!(
                    channel.abs_diff(expected) <= 5,
                    "channel={channel} at {time}s"
                );
            }
        }

        #[test]
        fn parses_ffmpeg_duration() {
            assert_eq!(parse_duration("01:02:03.50"), Some(3723.5));
        }

        #[test]
        fn ffmpeg_integer_pts_preserves_frame_boundary_precision() {
            let base =
                parse_time_base("[Parsed_showinfo] config in time_base: 1/12, frame_rate: 12/1");
            assert_eq!(base, Some((1, 12)));
            let line = "[Parsed_showinfo] n: 2 pts: 2 pts_time:0.166667 duration: 1";
            assert_eq!(parse_pts_time(line, base), Some(2.0 / 12.0));
            assert_eq!(parse_pts_time(line, None), Some(0.166667));
            assert_eq!(
                parse_time_base("config in time_base: 1/0, frame_rate: 12/1"),
                None
            );
        }

        #[test]
        fn unix_frame_selection_uses_precise_pts_at_fractional_boundaries() {
            let directory = tempfile::tempdir().unwrap();
            let ffmpeg = directory.path().join("ffmpeg");
            std::fs::write(
                &ffmpeg,
                "#!/bin/sh\nprintf '[Parsed_showinfo] config in time_base: 1/12, frame_rate: 12/1\\n' >&2\nprintf '[Parsed_showinfo] n:0 pts:0 pts_time:0\\n' >&2\nprintf '\\000\\000\\000\\377'\nprintf '[Parsed_showinfo] n:1 pts:1 pts_time:0.0833333\\n' >&2\nprintf '\\001\\001\\001\\377'\nprintf '[Parsed_showinfo] n:2 pts:2 pts_time:0.166667\\n' >&2\nprintf '\\002\\002\\002\\377'\nprintf '[Parsed_showinfo] n:3 pts:12 pts_time:1\\n' >&2\nprintf '\\014\\014\\014\\377'\n",
            )
            .unwrap();
            let mut permissions = std::fs::metadata(&ffmpeg).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&ffmpeg, permissions).unwrap();
            let info = VideoInfo {
                width: 1,
                height: 1,
                duration: 2.0,
                frame_rate: 12.0,
            };
            let media = MediaInfo {
                video: info,
                ..Default::default()
            };
            let mut decoder = None;
            let source = directory.path().join("source.mov");
            for (time, expected) in [
                (2.0 / 12.0 - 0.000_000_1, 1),
                (2.0 / 12.0, 2),
                ((1.0 + 1.0 / 12.0) - 1.0 / 12.0, 12),
            ] {
                let frame = read_frame(
                    &ffmpeg,
                    &source,
                    time,
                    media,
                    &mut decoder,
                    FrameWait::default(),
                )
                .unwrap();
                assert_eq!(frame.bgra[0], expected, "frame at {time:.17}");
            }
        }

        #[test]
        fn square_pixel_dimensions_expand_the_shortened_axis() {
            assert_eq!(sample_aspect("144x108 [SAR 4:3 DAR 16:9]"), Some((4, 3)));
            assert_eq!(sample_aspect("144x108 [SAR 1:0]"), None);
            assert_eq!(sample_aspect("144x108"), None);
            assert_eq!(square_pixel_size(144, 108, (4, 3)), Some((192, 108)));
            assert_eq!(square_pixel_size(144, 108, (3, 4)), Some((144, 144)));
            assert_eq!(square_pixel_size(146, 108, (5, 4)), Some((183, 108)));
            assert_eq!(square_pixel_size(3840, 2160, (1, 1)), Some((3840, 2160)));
            assert_eq!(square_pixel_size(3840, 2160, (65535, 1)), None);
        }

        #[test]
        #[ignore = "requires development FFmpeg"]
        fn anamorphic_video_decodes_at_square_pixel_dimensions() {
            let ffmpeg = crate::video::ffmpeg_path();
            let directory = tempfile::tempdir().unwrap();
            let upright = directory.path().join("upright.mp4");
            let rotated = directory.path().join("rotated.mp4");
            let encoded = ProcessCommand::new(&ffmpeg)
                .args([
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=size=144x108:rate=12",
                    "-vf",
                    "setsar=4/3",
                    "-frames:v",
                    "12",
                    "-c:v",
                    "mpeg4",
                ])
                .arg(&upright)
                .status()
                .unwrap();
            assert!(encoded.success());
            let rotated_copy = ProcessCommand::new(&ffmpeg)
                .args(["-v", "error", "-display_rotation:v:0", "90", "-i"])
                .arg(&upright)
                .args(["-map", "0:v:0", "-c", "copy"])
                .arg(&rotated)
                .status()
                .unwrap();
            assert!(rotated_copy.success());

            for (path, dimensions) in [(upright, (192, 108)), (rotated, (108, 192))] {
                let decoder = VideoDecoder::open(&path).unwrap();
                let info = decoder.info();
                assert_eq!((info.width, info.height), dimensions);
                let first = decoder.frame_at(0.).unwrap();
                let later = decoder.frame_at(0.5).unwrap();
                let reopened = decoder.frame_at(0.).unwrap();
                for frame in [&first, &later, &reopened] {
                    assert_eq!((frame.width, frame.height), dimensions);
                    assert_eq!(frame.bgra.len(), (dimensions.0 * dimensions.1 * 4) as usize);
                }
                assert_eq!(first.bgra, reopened.bgra);
                assert_ne!(first.bgra, later.bgra);
            }
        }

        #[test]
        fn display_rotation_swaps_video_dimensions() {
            assert_eq!(parse_rotation("rotation of -90.00 degrees"), Some(-90.0));
            assert!(rotation_swaps_dimensions(-90.0));
            assert!(!rotation_swaps_dimensions(180.0));
        }

        #[test]
        fn a_seek_retires_the_old_decoder_before_launching_its_replacement() {
            for (last_frame, requested) in [(Some(10.0), 0.25), (None, 4.5)] {
                let directory = fake_ffmpeg();
                let ffmpeg = directory.path().join("ffmpeg");
                let source = directory.path().join("source.mp4");
                let media = describe(&ffmpeg, &source).expect("video metadata");
                let child = ProcessCommand::new("sleep")
                    .arg("30")
                    .spawn()
                    .expect("stand-in for the active decoder");
                let (_, frames) = smol::channel::bounded(1);
                let mut decoder = Some(DecoderProcess {
                    child,
                    frames,
                    spare_frame: smol::channel::bounded(1).0,
                    seek_start: 0.0,
                    fallback_seek_start: None,
                    current: last_frame.map(|timestamp| TimedFrame {
                        timestamp,
                        frame: DecodedFrame {
                            width: 2,
                            height: 1,
                            bgra: vec![9, 8, 7, 255, 12, 11, 10, 255].into(),
                        },
                    }),
                    next: None,
                });
                let old_pid = decoder.as_ref().unwrap().child.id();
                std::fs::write(
                    directory.path().join("ffmpeg.previous"),
                    old_pid.to_string(),
                )
                .expect("decoder PID for the replacement to check");

                let frame = read_frame(
                    &ffmpeg,
                    &source,
                    requested,
                    media,
                    &mut decoder,
                    FrameWait::default(),
                )
                .expect("replacement decoder must run after the old process exits");

                assert_eq!(frame.bgra.as_slice(), [3, 2, 1, 255, 6, 5, 4, 255]);
            }
        }

        #[test]
        fn direct_seek_retries_if_the_first_frame_is_ahead_of_the_target() {
            let directory = fake_ffmpeg();
            let ffmpeg = directory.path().join("ffmpeg");
            std::fs::write(
                &ffmpeg,
                r#"#!/bin/sh
case "$5 $6" in
  *'-ss 8.000000') printf d >> "$0.calls" ;;
  *'-ss 6.000000')
    printf p >> "$0.calls"
    printf '[Parsed_showinfo] n:0 pts:7 pts_time:7\n' >&2
    printf '\021\000\000\377'
    ;;
  *) exit 1 ;;
esac
printf '[Parsed_showinfo] n:1 pts:9 pts_time:9\n' >&2
printf '\042\000\000\377'
"#,
            )
            .unwrap();
            let mut decoder = None;
            let info = VideoInfo {
                width: 1,
                height: 1,
                duration: 10.0,
                frame_rate: 1.0,
            };
            let frame = read_frame(
                &ffmpeg,
                &directory.path().join("source.mov"),
                8.0,
                MediaInfo {
                    video: info,
                    ..Default::default()
                },
                &mut decoder,
                FrameWait::default(),
            )
            .unwrap();
            assert_eq!(frame.bgra.as_slice(), [17, 0, 0, 255]);
            assert_eq!(
                std::fs::read(directory.path().join("ffmpeg.calls")).unwrap(),
                b"dp"
            );
        }

        #[test]
        fn an_empty_decoder_is_restarted_for_the_next_frame_request() {
            let directory = fake_ffmpeg();
            let ffmpeg = directory.path().join("ffmpeg");
            let source = directory.path().join("source.mp4");
            let media = describe(&ffmpeg, &source).expect("video metadata");
            std::fs::write(directory.path().join("ffmpeg.fail-once"), []).unwrap();
            let mut decoder = None;

            let first = read_frame(
                &ffmpeg,
                &source,
                0.25,
                media,
                &mut decoder,
                FrameWait::default(),
            );
            assert!(first.is_none());
            assert!(decoder.is_none());

            let recovered = read_frame(
                &ffmpeg,
                &source,
                0.25,
                media,
                &mut decoder,
                FrameWait::default(),
            )
            .expect("decoder should restart after delivering no frames");
            assert_eq!(recovered.bgra.as_slice(), [3, 2, 1, 255, 6, 5, 4, 255]);
            assert_eq!(std::fs::read(directory.path().join("ffmpeg.count")).unwrap(), b"xx");
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
                media,
                &mut decoder,
                FrameWait::default(),
            )
            .expect("first video frame");
            assert_eq!(first.bgra.as_slice(), [3, 2, 1, 255, 6, 5, 4, 255]);
            let gap = read_frame(
                &ffmpeg,
                &source,
                5.0,
                media,
                &mut decoder,
                FrameWait::default(),
            )
            .expect("sparse video gap");
            assert_eq!(gap.bgra.as_slice(), first.bgra.as_slice());
            assert_eq!(
                std::fs::read_to_string(directory.path().join("ffmpeg.count"))
                    .expect("FFmpeg launch count"),
                "x"
            );
            let second = read_frame(
                &ffmpeg,
                &source,
                10.0,
                media,
                &mut decoder,
                FrameWait::default(),
            )
            .expect("second video frame");
            assert_eq!(second.bgra.as_slice(), [9, 8, 7, 255, 12, 11, 10, 255]);

            let random = read_frame(
                &ffmpeg,
                &source,
                5.0,
                media,
                &mut None,
                FrameWait::default(),
            )
            .expect("random seek video frame");
            assert_eq!(random.bgra.as_slice(), first.bgra.as_slice());
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod backend {
    use super::*;

    pub fn run(
        _path: PathBuf,
        commands: Receiver<Command>,
        info_tx: smol::channel::Sender<Option<VideoInfo>>,
    ) {
        let _ = info_tx.try_send(None);
        while let Ok(Command { position, reply, .. }) = commands.recv() {
            let _ = position;
            drop(reply);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cancelled_metadata_wait_preserves_the_pending_decoder() {
        let (commands, receiver) = mpsc::channel();
        let (info_tx, info) = smol::channel::bounded(1);
        let opening = DecoderOpening {
            commands,
            info,
            deadline: Instant::now() + Duration::from_secs(10),
        };
        let (cancel, cancelled) = smol::channel::bounded(1);
        let warmup = opening.request_warmup(0.5, cancelled.clone()).unwrap();
        let command = receiver.try_recv().unwrap();
        assert!(matches!(command.position, FramePosition::Time(0.5)));
        drop(command);
        warmup.recv_timeout(Duration::from_secs(1)).unwrap();

        cancel.close();
        let opening = match opening.finish_interruptible(Some(&cancelled), None) {
            Err(opening) => opening,
            Ok(_) => panic!("cancelled wait consumed the opening decoder"),
        };
        let info = VideoInfo {
            width: 1,
            height: 1,
            duration: 2.0,
            frame_rate: 30.0,
        };
        let opening = match opening.finish_interruptible(None, Some(&cancelled)) {
            Err(opening) => opening,
            Ok(_) => panic!("superseded wait consumed the opening decoder"),
        };
        info_tx.try_send(Some(info)).unwrap();
        let ready = opening
            .finish_interruptible(None, Some(&cancelled))
            .ok()
            .flatten()
            .unwrap();
        assert_eq!(ready.info(), info);
    }

    #[cfg(windows)]
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RecordedTooltipProof {
        video_path: std::path::PathBuf,
        reference_path: std::path::PathBuf,
        marker_reference_path: std::path::PathBuf,
        fiducial_reference_path: std::path::PathBuf,
        fiducial_bounds: RecordedTooltipBounds,
        #[serde(default)]
        editor_preview_path: Option<std::path::PathBuf>,
        #[serde(default)]
        editor_preview_seek_path: Option<std::path::PathBuf>,
        #[serde(default)]
        editor_preview_seek_time: Option<f64>,
        marker_move_seconds: f64,
        #[serde(default)]
        expected_duration: Option<f64>,
        paint_proof_path: std::path::PathBuf,
        tooltip_hwnd: i64,
        owner_hwnd: i64,
        tooltip_text: String,
        capture_bounds: RecordedTooltipBounds,
        tooltip_bounds: RecordedTooltipBounds,
        owner_bounds: RecordedTooltipBounds,
        marker_panel_bounds: RecordedTooltipBounds,
        marker_initial_bounds: RecordedTooltipBounds,
        marker_moved_bounds: RecordedTooltipBounds,
        paint_client_bounds: RecordedTooltipClientBounds,
    }

    #[cfg(windows)]
    #[derive(serde::Deserialize)]
    struct RecordedTooltipBounds {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[cfg(windows)]
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RecordedTooltipClientBounds {
        screen_left: i32,
        screen_top: i32,
        width: i32,
        height: i32,
    }

    #[cfg(windows)]
    #[derive(Clone, Copy)]
    struct FiducialPreviewMatch {
        left: u32,
        top: u32,
        width: u32,
        height: u32,
        scale: f64,
        mean_error: f64,
    }

    #[cfg(windows)]
    fn numeric_fiducial() -> image::RgbaImage {
        let cell_size = 12u32;
        let mut image = image::RgbaImage::new(8 * cell_size, 6 * cell_size);
        for index in 0..48u32 {
            let color = match index {
                0 => [236, 36, 36, 255],
                7 => [32, 220, 48, 255],
                40 => [40, 56, 236, 255],
                47 => [236, 220, 32, 255],
                _ => [
                    24 + ((index * 47) % 208) as u8,
                    24 + ((index * 83) % 208) as u8,
                    24 + ((index * 131) % 208) as u8,
                    255,
                ],
            };
            let x = (index % 8) * cell_size;
            let y = (index / 8) * cell_size;
            for row in y..y + cell_size {
                for column in x..x + cell_size {
                    image.put_pixel(column, row, image::Rgba(color));
                }
            }
        }
        image
    }

    #[cfg(windows)]
    fn average_rgb(image: &image::RgbaImage, x: f64, y: f64) -> [f64; 3] {
        let center_x = x.round() as i32;
        let center_y = y.round() as i32;
        let mut channels = [0.0; 3];
        let mut count = 0.0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let sample_x = (center_x + dx).clamp(0, image.width() as i32 - 1) as u32;
                let sample_y = (center_y + dy).clamp(0, image.height() as i32 - 1) as u32;
                let pixel = image.get_pixel(sample_x, sample_y);
                for channel in 0..3 {
                    channels[channel] += f64::from(pixel[channel]);
                }
                count += 1.0;
            }
        }
        channels.map(|channel| channel / count)
    }

    #[cfg(windows)]
    fn average_frame_rgb(frame: &DecodedFrame, x: f64, y: f64, radius: i32) -> [f64; 3] {
        let center_x = x.round() as i32;
        let center_y = y.round() as i32;
        let mut channels = [0.0; 3];
        let mut count = 0.0;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let sample_x = (center_x + dx).clamp(0, frame.width as i32 - 1) as u32;
                let sample_y = (center_y + dy).clamp(0, frame.height as i32 - 1) as u32;
                let offset = ((sample_y * frame.width + sample_x) * 4) as usize;
                channels[0] += f64::from(frame.bgra[offset + 2]);
                channels[1] += f64::from(frame.bgra[offset + 1]);
                channels[2] += f64::from(frame.bgra[offset]);
                count += 1.0;
            }
        }
        channels.map(|channel| channel / count)
    }

    #[cfg(windows)]
    fn marker_preview_sample_errors(
        screenshot: &image::RgbaImage,
        frame: &DecodedFrame,
        capture_bounds: &RecordedTooltipBounds,
        marker_panel_bounds: &RecordedTooltipBounds,
        fiducial_bounds: &RecordedTooltipBounds,
        fiducial_match: FiducialPreviewMatch,
    ) -> Vec<f64> {
        let mut errors = Vec::with_capacity(18);
        let markers = [
            RecordedTooltipBounds {
                left: marker_panel_bounds.left,
                top: marker_panel_bounds.top,
                right: marker_panel_bounds.left + 32,
                bottom: marker_panel_bounds.top + 32,
            },
            RecordedTooltipBounds {
                left: marker_panel_bounds.right - 32,
                top: marker_panel_bounds.top,
                right: marker_panel_bounds.right,
                bottom: marker_panel_bounds.bottom,
            },
        ];
        for marker in markers {
            for y_fraction in [0.25, 0.5, 0.75] {
                for x_fraction in [0.25, 0.5, 0.75] {
                    let source_x =
                        f64::from(marker.left) + f64::from(marker.right - marker.left) * x_fraction;
                    let source_y =
                        f64::from(marker.top) + f64::from(marker.bottom - marker.top) * y_fraction;
                    let preview_x = f64::from(fiducial_match.left)
                        + (source_x - f64::from(fiducial_bounds.left)) * fiducial_match.scale;
                    let preview_y = f64::from(fiducial_match.top)
                        + (source_y - f64::from(fiducial_bounds.top)) * fiducial_match.scale;
                    let preview = average_rgb(screenshot, preview_x, preview_y);
                    let decoded = average_frame_rgb(
                        frame,
                        source_x - f64::from(capture_bounds.left),
                        source_y - f64::from(capture_bounds.top),
                        3,
                    );
                    errors.push(
                        (0..3)
                            .map(|channel| (preview[channel] - decoded[channel]).abs())
                            .sum::<f64>()
                            / 3.0,
                    );
                }
            }
        }
        errors
    }

    #[cfg(windows)]
    #[test]
    fn marker_preview_sample_errors_handles_scaled_mapping_offset_and_stale_polarity() {
        fn marker_frame(marker_left: u32) -> DecodedFrame {
            let mut image = image::RgbaImage::from_pixel(320, 180, image::Rgba([96, 96, 96, 255]));
            for y in 30..62 {
                for x in 30..158 {
                    image.put_pixel(x, y, image::Rgba([255, 255, 255, 255]));
                }
            }
            for y in 30..62 {
                for x in marker_left..marker_left + 32 {
                    image.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
                }
            }
            let bgra = image
                .pixels()
                .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
                .collect::<Vec<_>>();
            DecodedFrame {
                width: 320,
                height: 180,
                bgra: bgra.into(),
            }
        }

        fn preview(frame: &DecodedFrame) -> image::RgbaImage {
            let source = image::RgbaImage::from_fn(frame.width, frame.height, |x, y| {
                let offset = ((y * frame.width + x) * 4) as usize;
                image::Rgba([
                    frame.bgra[offset + 2],
                    frame.bgra[offset + 1],
                    frame.bgra[offset],
                    frame.bgra[offset + 3],
                ])
            });
            let scaled =
                image::imageops::resize(&source, 160, 90, image::imageops::FilterType::Nearest);
            let mut screenshot =
                image::RgbaImage::from_pixel(200, 140, image::Rgba([24, 24, 24, 255]));
            image::imageops::overlay(&mut screenshot, &scaled, 17, 25);
            screenshot
        }

        let moved_frame = marker_frame(126);
        let stale_frame = marker_frame(30);
        let screenshot = preview(&moved_frame);
        let capture_bounds = RecordedTooltipBounds {
            left: -200,
            top: -100,
            right: 120,
            bottom: 80,
        };
        let marker_panel_bounds = RecordedTooltipBounds {
            left: -170,
            top: -70,
            right: -42,
            bottom: -38,
        };
        let fiducial_bounds = RecordedTooltipBounds {
            left: -34,
            top: -70,
            right: 62,
            bottom: 2,
        };
        let fiducial_match = FiducialPreviewMatch {
            left: 101,
            top: 38,
            width: 48,
            height: 36,
            scale: 0.5,
            mean_error: 0.0,
        };
        let moved_errors = marker_preview_sample_errors(
            &screenshot,
            &moved_frame,
            &capture_bounds,
            &marker_panel_bounds,
            &fiducial_bounds,
            fiducial_match,
        );
        let stale_errors = marker_preview_sample_errors(
            &screenshot,
            &stale_frame,
            &capture_bounds,
            &marker_panel_bounds,
            &fiducial_bounds,
            fiducial_match,
        );
        assert!(
            moved_errors.iter().all(|error| *error <= 1.0),
            "scaled moved marker ROI errors: {moved_errors:?}"
        );
        assert!(
            stale_errors.iter().all(|error| *error >= 80.0),
            "stale marker polarity errors: {stale_errors:?}"
        );
    }

    #[cfg(windows)]
    fn fiducial_roi_matches(
        reference: &image::RgbaImage,
        actual: &image::RgbaImage,
    ) -> (bool, f64, f64, u8) {
        let (width, height) = reference.dimensions();
        let cell_width = width / 8;
        let cell_height = height / 6;
        let total_error: u64 = reference
            .pixels()
            .zip(actual.pixels())
            .flat_map(|(expected, actual)| {
                (0..3).map(move |channel| u64::from(expected[channel].abs_diff(actual[channel])))
            })
            .sum();
        let mut interior_error = 0u64;
        let mut max_pixel_error = 0u8;
        let mut interior_pixels = 0u64;
        for row in 0..6 {
            for column in 0..8 {
                for y in row * cell_height + 3..(row + 1) * cell_height - 3 {
                    for x in column * cell_width + 3..(column + 1) * cell_width - 3 {
                        let expected = reference.get_pixel(x, y);
                        let actual = actual.get_pixel(x, y);
                        let difference = [
                            expected[0].abs_diff(actual[0]),
                            expected[1].abs_diff(actual[1]),
                            expected[2].abs_diff(actual[2]),
                        ];
                        max_pixel_error =
                            max_pixel_error.max(difference.into_iter().max().unwrap_or_default());
                        interior_error += difference.into_iter().map(u64::from).sum::<u64>();
                        interior_pixels += 1;
                    }
                }
            }
        }
        let mean_error = total_error as f64 / f64::from(width * height * 3);
        let interior_mean_error = interior_error as f64 / (interior_pixels * 3) as f64;
        (
            mean_error <= 18.0 && interior_mean_error <= 10.0 && max_pixel_error <= 48,
            mean_error,
            interior_mean_error,
            max_pixel_error,
        )
    }

    #[cfg(windows)]
    fn preview_fiducial_match(
        screenshot: &image::RgbaImage,
        template: &image::RgbaImage,
        search: &RecordedTooltipBounds,
    ) -> Option<FiducialPreviewMatch> {
        const COLUMNS: u32 = 8;
        const ROWS: u32 = 6;
        if !template.width().is_multiple_of(COLUMNS) || !template.height().is_multiple_of(ROWS) {
            return None;
        }
        let cell_width = template.width() / COLUMNS;
        let cell_height = template.height() / ROWS;
        if cell_width < 4 || cell_height < 4 {
            return None;
        }
        let search_left = search.left.max(0) as u32;
        let search_top = search.top.max(0) as u32;
        let search_right = search.right.min(screenshot.width() as i32).max(0) as u32;
        let search_bottom = search.bottom.min(screenshot.height() as i32).max(0) as u32;
        if search_right <= search_left || search_bottom <= search_top {
            return None;
        }

        let anchors = [(0, 0), (7, 0), (0, 5), (7, 5)];
        let template_point = |column: u32, row: u32| {
            (
                f64::from(column * cell_width) + f64::from(cell_width) / 2.0,
                f64::from(row * cell_height) + f64::from(cell_height) / 2.0,
            )
        };
        let anchor_template: Vec<_> = anchors
            .iter()
            .map(|(column, row)| {
                let (x, y) = template_point(*column, *row);
                average_rgb(template, x, y)
            })
            .collect();
        let mut sample_points = Vec::with_capacity(212);
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                let (center_x, center_y) = template_point(column, row);
                sample_points.push((center_x, center_y));
                if column + 1 < COLUMNS {
                    let boundary_x = f64::from((column + 1) * cell_width);
                    sample_points.push((boundary_x - 2.0, center_y));
                    sample_points.push((boundary_x + 2.0, center_y));
                }
                if row + 1 < ROWS {
                    let boundary_y = f64::from((row + 1) * cell_height);
                    sample_points.push((center_x, boundary_y - 2.0));
                    sample_points.push((center_x, boundary_y + 2.0));
                }
            }
        }
        let tolerance = 58.0;
        const MAX_ANCHOR_CANDIDATES: usize = 50_000;
        const MAX_VERIFIED_TRANSFORMS: usize = 10_000;
        const MAX_MATCH_CANDIDATES: usize = 1_024;
        let mut anchor_candidates: [Vec<(u32, u32)>; 4] = std::array::from_fn(|_| Vec::new());
        let mut anchor_overflow = [false; 4];
        for screen_y in search_top..search_bottom {
            for screen_x in search_left..search_right {
                let pixel = screenshot.get_pixel(screen_x, screen_y);
                for (index, expected) in anchor_template.iter().enumerate() {
                    if anchor_overflow[index] {
                        continue;
                    }
                    let error = (0..3)
                        .map(|channel| (f64::from(pixel[channel]) - expected[channel]).abs())
                        .fold(0.0, f64::max);
                    if error > tolerance {
                        continue;
                    }
                    if anchor_candidates[index].len() == MAX_ANCHOR_CANDIDATES {
                        anchor_overflow[index] = true;
                        anchor_candidates[index].clear();
                        continue;
                    }
                    anchor_candidates[index].push((screen_x, screen_y));
                }
            }
        }
        let (anchor_index, candidate_points) = anchor_candidates
            .iter()
            .enumerate()
            .filter(|(_, points)| !points.is_empty())
            .min_by_key(|(_, points)| points.len())?;
        let (anchor_column, anchor_row) = anchors[anchor_index];
        let (anchor_point_x, anchor_point_y) = template_point(anchor_column, anchor_row);
        let mut candidates = Vec::<FiducialPreviewMatch>::new();
        let mut candidate_buckets = std::collections::HashMap::<(u32, u32), usize>::new();
        let mut verified_transforms = 0usize;
        for (screen_x, screen_y) in candidate_points {
            for step in 0..=45 {
                let scale = 0.30 + f64::from(step) * 0.01;
                let left = (f64::from(*screen_x) - anchor_point_x * scale).round();
                let top = (f64::from(*screen_y) - anchor_point_y * scale).round();
                let width = (f64::from(template.width()) * scale).round() as u32;
                let height = (f64::from(template.height()) * scale).round() as u32;
                if left < f64::from(search_left)
                    || top < f64::from(search_top)
                    || left + f64::from(width) > f64::from(search_right)
                    || top + f64::from(height) > f64::from(search_bottom)
                {
                    continue;
                }
                let left = left as u32;
                let top = top as u32;
                let mut anchors_match = true;
                for (index, (column, row)) in anchors.iter().enumerate() {
                    let (template_x, template_y) = template_point(*column, *row);
                    let actual = average_rgb(
                        screenshot,
                        f64::from(left) + template_x * scale,
                        f64::from(top) + template_y * scale,
                    );
                    let error = (0..3)
                        .map(|channel| (actual[channel] - anchor_template[index][channel]).abs())
                        .fold(0.0, f64::max);
                    if error > tolerance {
                        anchors_match = false;
                        break;
                    }
                }
                if !anchors_match {
                    continue;
                }
                verified_transforms += 1;
                if verified_transforms > MAX_VERIFIED_TRANSFORMS {
                    return None;
                }
                let mut error = 0.0;
                let mut high_error_samples = 0;
                for (template_x, template_y) in &sample_points {
                    let expected = average_rgb(template, *template_x, *template_y);
                    let actual = average_rgb(
                        screenshot,
                        f64::from(left) + template_x * scale,
                        f64::from(top) + template_y * scale,
                    );
                    let cell_error = (0..3)
                        .map(|channel| (actual[channel] - expected[channel]).abs())
                        .sum::<f64>()
                        / 3.0;
                    if (0..3)
                        .map(|channel| (actual[channel] - expected[channel]).abs())
                        .fold(0.0, f64::max)
                        > 48.0
                    {
                        high_error_samples += 1;
                    }
                    error += cell_error;
                }
                let mean_error = error / sample_points.len() as f64;
                if mean_error > 22.0 || high_error_samples > sample_points.len() / 8 {
                    continue;
                }
                let candidate = FiducialPreviewMatch {
                    left,
                    top,
                    width,
                    height,
                    scale,
                    mean_error,
                };
                let bucket = (candidate.left / 8, candidate.top / 8);
                if let Some(index) = candidate_buckets.get(&bucket).copied() {
                    if candidate.mean_error < candidates[index].mean_error {
                        candidates[index] = candidate;
                    }
                } else {
                    if candidates.len() >= MAX_MATCH_CANDIDATES {
                        return None;
                    }
                    candidate_buckets.insert(bucket, candidates.len());
                    candidates.push(candidate);
                }
            }
        }
        candidates.sort_by(|left, right| left.mean_error.total_cmp(&right.mean_error));
        let best = *candidates.first()?;
        let has_second = candidates.iter().copied().any(|candidate| {
            let left = candidate.left.max(best.left);
            let top = candidate.top.max(best.top);
            let right = (candidate.left + candidate.width).min(best.left + best.width);
            let bottom = (candidate.top + candidate.height).min(best.top + best.height);
            let overlap = right.saturating_sub(left) * bottom.saturating_sub(top);
            let smaller = (candidate.width * candidate.height).min(best.width * best.height);
            f64::from(overlap) / f64::from(smaller) <= 0.75
        });
        if has_second {
            return None;
        }
        Some(best)
    }

    #[cfg(windows)]
    #[test]
    fn numeric_scratch_fiducial_match_is_scaled_unique_and_rejects_rotation() {
        let reference = numeric_fiducial();
        let mut edge_bleed = reference.clone();
        for (x, y, pixel) in edge_bleed.enumerate_pixels_mut() {
            if x % 12 < 2 || x % 12 >= 10 || y % 12 < 2 || y % 12 >= 10 {
                pixel[0] = pixel[0].saturating_add(60);
            }
        }
        assert!(fiducial_roi_matches(&reference, &edge_bleed).0);
        assert!(!fiducial_roi_matches(&reference, &image::imageops::rotate180(&reference)).0);
        let mut shifted_reference = reference.clone();
        for (x, y, pixel) in shifted_reference.enumerate_pixels_mut() {
            *pixel = *reference.get_pixel(x.saturating_sub(3), y);
        }
        assert!(!fiducial_roi_matches(&reference, &shifted_reference).0);
        let mut screenshot = image::RgbaImage::new(984, 680);
        let mut noise = 0x1234_5678u32;
        for pixel in screenshot.pixels_mut() {
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            *pixel = image::Rgba([
                144 + (noise as u8 % 112),
                (noise.rotate_left(7) as u8) % 112,
                (noise.rotate_left(15) as u8) % 88,
                255,
            ]);
        }
        let mut outside_screenshot = screenshot.clone();
        let scaled =
            image::imageops::resize(&reference, 50, 37, image::imageops::FilterType::Triangle);
        image::imageops::overlay(&mut screenshot, &scaled, 510, 160);
        for y in 160..197 {
            for x in 510..560 {
                let noise = ((x * 7 + y * 13) % 9) as i16 - 4;
                let pixel = screenshot.get_pixel_mut(x, y);
                for channel in 0..3 {
                    pixel[channel] = (i16::from(pixel[channel]) + noise).clamp(0, 255) as u8;
                }
            }
        }
        let mut duplicate_screenshot = screenshot.clone();
        image::imageops::overlay(&mut duplicate_screenshot, &scaled, 250, 160);
        let mut shifted_duplicate_screenshot =
            image::RgbaImage::from_pixel(984, 680, image::Rgba([0, 0, 0, 255]));
        image::imageops::overlay(&mut shifted_duplicate_screenshot, &scaled, 100, 60);
        let mut shifted = scaled.clone();
        for pixel in shifted.pixels_mut() {
            for channel in 0..3 {
                pixel[channel] = pixel[channel].saturating_add(10);
            }
        }
        image::imageops::overlay(&mut shifted_duplicate_screenshot, &shifted, 300, 160);
        image::imageops::overlay(&mut outside_screenshot, &scaled, 760, 160);
        let mut negative = reference.clone();
        image::imageops::rotate180_in_place(&mut negative);
        let search = RecordedTooltipBounds {
            left: 0,
            top: (screenshot.height() as f32 * 0.04) as i32,
            right: (screenshot.width() as f32 * 0.68) as i32,
            bottom: (screenshot.height() as f32 * 0.70) as i32,
        };
        let matched = preview_fiducial_match(&screenshot, &reference, &search)
            .expect("positive numeric fiducial match");
        assert!(
            matched.left.abs_diff(510) <= 3,
            "numeric matcher x {}, scale {}, score {}",
            matched.left,
            matched.scale,
            matched.mean_error
        );
        assert!(
            matched.top.abs_diff(160) <= 3,
            "numeric matcher y {}, scale {}, score {}",
            matched.top,
            matched.scale,
            matched.mean_error
        );
        assert!(
            (matched.scale - (50.0 / 96.0)).abs() <= 0.015,
            "numeric matcher scale {}, score {}",
            matched.scale,
            matched.mean_error
        );
        assert!(
            matched.mean_error <= 16.0,
            "numeric matcher mean error {}",
            matched.mean_error
        );
        assert!(
            preview_fiducial_match(&screenshot, &negative, &search).is_none(),
            "rotated numeric negative control matched the screenshot"
        );
        assert!(
            preview_fiducial_match(&outside_screenshot, &reference, &search).is_none(),
            "outside-pane numeric negative control matched the cropped preview search"
        );
        assert!(
            preview_fiducial_match(&duplicate_screenshot, &reference, &search).is_none(),
            "two disjoint numeric fiducials were accepted as a unique preview match"
        );
        assert!(
            preview_fiducial_match(&shifted_duplicate_screenshot, &reference, &search).is_none(),
            "a color-shifted duplicate was accepted as a unique preview match"
        );
        let mut uniform_background =
            image::RgbaImage::from_pixel(984, 680, image::Rgba([236, 36, 36, 255]));
        image::imageops::overlay(&mut uniform_background, &scaled, 510, 160);
        let uniform_match = preview_fiducial_match(&uniform_background, &reference, &search)
            .expect("fiducial remains detectable over a uniform first-anchor background");
        assert!(uniform_match.left.abs_diff(510) <= 3);
        assert!(uniform_match.top.abs_diff(160) <= 3);

        let mut mixed_background = image::RgbaImage::new(984, 680);
        let anchor_colors = [
            image::Rgba([236, 36, 36, 255]),
            image::Rgba([32, 220, 48, 255]),
            image::Rgba([40, 56, 236, 255]),
            image::Rgba([236, 220, 32, 255]),
        ];
        for y in 0..mixed_background.height() {
            for x in 0..mixed_background.width() {
                let index = (if x >= mixed_background.width() / 2 {
                    1
                } else {
                    0
                }) + 2
                    * (if y >= mixed_background.height() / 2 {
                        1
                    } else {
                        0
                    });
                mixed_background.put_pixel(x, y, anchor_colors[index]);
            }
        }
        image::imageops::overlay(&mut mixed_background, &scaled, 510, 160);
        assert!(
            preview_fiducial_match(&mixed_background, &reference, &search).is_none(),
            "ambiguous mixed-anchor background was accepted as a unique fiducial"
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires the hosted native tooltip video fixture"]
    fn recorded_tooltip_is_excluded_from_video_frames() {
        assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"));
        let manifest_path = std::env::var_os("PORATAKE_NATIVE_TOOLTIP_VIDEO_PROOF")
            .map(std::path::PathBuf::from)
            .expect("hosted tooltip video proof manifest");
        let manifest: RecordedTooltipProof = serde_json::from_slice(
            &std::fs::read(&manifest_path).expect("read hosted tooltip video manifest"),
        )
        .expect("parse hosted tooltip video manifest");
        let reference = image::open(&manifest.reference_path)
            .expect("open positive white underlay capture")
            .to_rgba8();
        let marker_reference = image::open(&manifest.marker_reference_path)
            .expect("open white marker underlay capture")
            .to_rgba8();
        let fiducial_reference = image::open(&manifest.fiducial_reference_path)
            .expect("open captured multicolor fiducial reference")
            .to_rgba8();
        let paint_proof = image::open(&manifest.paint_proof_path)
            .expect("open actual WM_PRINTCLIENT tooltip paint proof")
            .to_rgba8();
        let paint_metadata_path = manifest.paint_proof_path.with_extension("json");
        let paint_metadata: serde_json::Value = serde_json::from_slice(
            &std::fs::read(paint_metadata_path).expect("read tooltip paint proof metadata"),
        )
        .expect("parse tooltip paint proof metadata");
        assert_eq!(
            paint_metadata["tooltipHwnd"].as_i64(),
            Some(manifest.tooltip_hwnd)
        );
        assert_eq!(
            paint_metadata["ownerHwnd"].as_i64(),
            Some(manifest.owner_hwnd)
        );
        assert_eq!(
            paint_metadata["text"].as_str(),
            Some(manifest.tooltip_text.as_str())
        );
        let decoder = VideoDecoder::open(&manifest.video_path)
            .expect("open recorded tooltip video with Media Foundation");
        let info = decoder.info();
        assert!(
            info.is_valid(),
            "recording has no decodable video dimensions"
        );
        assert!(
            info.duration >= 1.2,
            "recording is too short: {}s",
            info.duration
        );
        if let Some(expected_duration) = manifest.expected_duration {
            assert!(
                manifest.editor_preview_path.is_some(),
                "UI-export proof must include the actual editor screenshot for fiducial parity"
            );
            assert!(
                manifest.editor_preview_seek_path.is_some()
                    && manifest.editor_preview_seek_time.is_some(),
                "UI-export proof must include an after-seek editor screenshot and timestamp"
            );
            assert!(
                (info.duration - expected_duration).abs() <= 0.100_001,
                "decoded video duration {}s differs by more than one 10-FPS frame from the expected segment {}s",
                info.duration,
                expected_duration
            );
        }

        let tooltip_width =
            u32::try_from(manifest.tooltip_bounds.right - manifest.tooltip_bounds.left)
                .expect("positive tooltip width");
        let tooltip_height =
            u32::try_from(manifest.tooltip_bounds.bottom - manifest.tooltip_bounds.top)
                .expect("positive tooltip height");
        assert_eq!(
            (reference.width(), reference.height()),
            (tooltip_width, tooltip_height)
        );
        let capture_width =
            u32::try_from(manifest.capture_bounds.right - manifest.capture_bounds.left)
                .expect("positive capture width");
        let capture_height =
            u32::try_from(manifest.capture_bounds.bottom - manifest.capture_bounds.top)
                .expect("positive capture height");
        assert_eq!((info.width, info.height), (capture_width, capture_height));
        assert!(manifest.tooltip_bounds.left >= manifest.capture_bounds.left);
        assert!(manifest.tooltip_bounds.top >= manifest.capture_bounds.top);
        assert!(manifest.tooltip_bounds.right <= manifest.capture_bounds.right);
        assert!(manifest.tooltip_bounds.bottom <= manifest.capture_bounds.bottom);
        let late_sample_time = info.duration - 0.1;
        assert!(manifest.marker_move_seconds >= 0.5);
        assert!(
            late_sample_time >= manifest.marker_move_seconds + 0.7,
            "late sample at {late_sample_time:.3}s is too close to marker move at {:.3}s",
            manifest.marker_move_seconds
        );
        let marker_panel_width =
            u32::try_from(manifest.marker_panel_bounds.right - manifest.marker_panel_bounds.left)
                .expect("positive marker panel width");
        let marker_panel_height =
            u32::try_from(manifest.marker_panel_bounds.bottom - manifest.marker_panel_bounds.top)
                .expect("positive marker panel height");
        assert_eq!(
            marker_reference.dimensions(),
            (marker_panel_width, marker_panel_height)
        );
        for marker in [
            &manifest.marker_initial_bounds,
            &manifest.marker_moved_bounds,
        ] {
            assert!(marker.left >= manifest.marker_panel_bounds.left);
            assert!(marker.top >= manifest.marker_panel_bounds.top);
            assert!(marker.right <= manifest.marker_panel_bounds.right);
            assert!(marker.bottom <= manifest.marker_panel_bounds.bottom);
            assert!(marker.left >= manifest.capture_bounds.left);
            assert!(marker.top >= manifest.capture_bounds.top);
            assert!(marker.right <= manifest.capture_bounds.right);
            assert!(marker.bottom <= manifest.capture_bounds.bottom);
            assert!(
                marker.right <= manifest.owner_bounds.left
                    || marker.left >= manifest.owner_bounds.right
                    || marker.bottom <= manifest.owner_bounds.top
                    || marker.top >= manifest.owner_bounds.bottom
            );
            assert!(
                marker.right <= manifest.tooltip_bounds.left
                    || marker.left >= manifest.tooltip_bounds.right
                    || marker.bottom <= manifest.tooltip_bounds.top
                    || marker.top >= manifest.tooltip_bounds.bottom
            );
        }
        assert!(
            manifest.marker_initial_bounds.right <= manifest.marker_moved_bounds.left
                || manifest.marker_moved_bounds.right <= manifest.marker_initial_bounds.left
                || manifest.marker_initial_bounds.bottom <= manifest.marker_moved_bounds.top
                || manifest.marker_moved_bounds.bottom <= manifest.marker_initial_bounds.top
        );
        let fiducial_width =
            u32::try_from(manifest.fiducial_bounds.right - manifest.fiducial_bounds.left)
                .expect("positive fiducial width");
        let fiducial_height =
            u32::try_from(manifest.fiducial_bounds.bottom - manifest.fiducial_bounds.top)
                .expect("positive fiducial height");
        assert_eq!(
            fiducial_reference.dimensions(),
            (fiducial_width, fiducial_height)
        );
        assert_eq!(fiducial_width % 8, 0);
        assert_eq!(fiducial_height % 6, 0);
        assert!(fiducial_width >= 64 && fiducial_height >= 48);
        assert!(manifest.fiducial_bounds.left >= manifest.capture_bounds.left);
        assert!(manifest.fiducial_bounds.top >= manifest.capture_bounds.top);
        assert!(manifest.fiducial_bounds.right <= manifest.capture_bounds.right);
        assert!(manifest.fiducial_bounds.bottom <= manifest.capture_bounds.bottom);
        let intersects = |first: &RecordedTooltipBounds, second: &RecordedTooltipBounds| {
            first.left < second.right
                && first.right > second.left
                && first.top < second.bottom
                && first.bottom > second.top
        };
        assert!(!intersects(
            &manifest.fiducial_bounds,
            &manifest.tooltip_bounds
        ));
        assert!(!intersects(
            &manifest.fiducial_bounds,
            &manifest.owner_bounds
        ));
        assert!(!intersects(
            &manifest.fiducial_bounds,
            &manifest.marker_panel_bounds
        ));
        assert!(!intersects(
            &manifest.fiducial_bounds,
            &manifest.marker_initial_bounds
        ));
        assert!(!intersects(
            &manifest.fiducial_bounds,
            &manifest.marker_moved_bounds
        ));
        let cell_width = fiducial_width / 8;
        let cell_height = fiducial_height / 6;
        let mut reference_cell_colors = std::collections::HashSet::new();
        for row in 0..6 {
            for column in 0..8 {
                let pixel = fiducial_reference.get_pixel(
                    column * cell_width + cell_width / 2,
                    row * cell_height + cell_height / 2,
                );
                reference_cell_colors.insert((pixel[0], pixel[1], pixel[2]));
            }
        }
        assert_eq!(
            reference_cell_colors.len(),
            48,
            "captured fiducial must have a unique color at every cell center"
        );
        let paint_client = &manifest.paint_client_bounds;
        let paint_client_right = paint_client.screen_left + paint_client.width;
        let paint_client_bottom = paint_client.screen_top + paint_client.height;
        assert_eq!(
            paint_proof.dimensions(),
            (paint_client.width as u32, paint_client.height as u32)
        );
        assert!(paint_client.width > 0 && paint_client.height > 0);
        assert!(paint_client.screen_left >= manifest.tooltip_bounds.left);
        assert!(paint_client.screen_top >= manifest.tooltip_bounds.top);
        assert!(paint_client_right <= manifest.tooltip_bounds.right);
        assert!(paint_client_bottom <= manifest.tooltip_bounds.bottom);
        assert!(paint_client.screen_left >= manifest.capture_bounds.left);
        assert!(paint_client.screen_top >= manifest.capture_bounds.top);
        assert!(paint_client_right <= manifest.capture_bounds.right);
        assert!(paint_client_bottom <= manifest.capture_bounds.bottom);

        let analyze_frame = |bgra: &[u8]| {
            let mut compared_pixels = 0usize;
            let mut high_error_pixels = 0usize;
            let mut total_error = 0u64;
            let mut proof = image::RgbaImage::new(tooltip_width, tooltip_height);
            for y in 0..tooltip_height {
                for x in 0..tooltip_width {
                    let screen_x = manifest.tooltip_bounds.left + x as i32;
                    let screen_y = manifest.tooltip_bounds.top + y as i32;
                    let frame_x = (screen_x - manifest.capture_bounds.left) as u32;
                    let frame_y = (screen_y - manifest.capture_bounds.top) as u32;
                    let offset = ((frame_y * info.width + frame_x) * 4) as usize;
                    let pixel = &bgra[offset..offset + 4];
                    proof.put_pixel(x, y, image::Rgba([pixel[2], pixel[1], pixel[0], 255]));
                    if screen_x >= manifest.owner_bounds.left
                        && screen_x < manifest.owner_bounds.right
                        && screen_y >= manifest.owner_bounds.top
                        && screen_y < manifest.owner_bounds.bottom
                    {
                        continue;
                    }
                    let expected = reference.get_pixel(x, y);
                    let differences = [
                        pixel[2].abs_diff(expected[0]),
                        pixel[1].abs_diff(expected[1]),
                        pixel[0].abs_diff(expected[2]),
                    ];
                    if differences.into_iter().max().unwrap_or_default() > 24 {
                        high_error_pixels += 1;
                    }
                    total_error += differences.into_iter().map(u64::from).sum::<u64>();
                    compared_pixels += 1;
                }
            }
            let mean_error = total_error as f64 / (compared_pixels * 3) as f64;
            let allowed_high_error_pixels = 12.max(compared_pixels / 100);
            (
                compared_pixels,
                high_error_pixels,
                mean_error,
                allowed_high_error_pixels,
                proof,
            )
        };

        let marker_crop = |bgra: &[u8]| {
            let mut crop = image::RgbaImage::new(marker_panel_width, marker_panel_height);
            for y in 0..marker_panel_height {
                for x in 0..marker_panel_width {
                    let frame_x = (manifest.marker_panel_bounds.left - manifest.capture_bounds.left)
                        as u32
                        + x;
                    let frame_y =
                        (manifest.marker_panel_bounds.top - manifest.capture_bounds.top) as u32 + y;
                    let offset = ((frame_y * info.width + frame_x) * 4) as usize;
                    let pixel = &bgra[offset..offset + 4];
                    crop.put_pixel(x, y, image::Rgba([pixel[2], pixel[1], pixel[0], 255]));
                }
            }
            crop
        };
        let fiducial_crop = |bgra: &[u8]| {
            let mut crop = image::RgbaImage::new(fiducial_width, fiducial_height);
            for y in 0..fiducial_height {
                for x in 0..fiducial_width {
                    let frame_x =
                        (manifest.fiducial_bounds.left - manifest.capture_bounds.left) as u32 + x;
                    let frame_y =
                        (manifest.fiducial_bounds.top - manifest.capture_bounds.top) as u32 + y;
                    let offset = ((frame_y * info.width + frame_x) * 4) as usize;
                    let pixel = &bgra[offset..offset + 4];
                    crop.put_pixel(x, y, image::Rgba([pixel[2], pixel[1], pixel[0], 255]));
                }
            }
            crop
        };
        let marker_matches_black = |bgra: &[u8], bounds: &RecordedTooltipBounds| {
            let mut black = 0usize;
            let mut compared = 0usize;
            for y in bounds.top + 3..bounds.bottom - 3 {
                for x in bounds.left + 3..bounds.right - 3 {
                    let frame_x = (x - manifest.capture_bounds.left) as u32;
                    let frame_y = (y - manifest.capture_bounds.top) as u32;
                    let offset = ((frame_y * info.width + frame_x) * 4) as usize;
                    let pixel = &bgra[offset..offset + 4];
                    if pixel[0] <= 24 && pixel[1] <= 24 && pixel[2] <= 24 {
                        black += 1;
                    }
                    compared += 1;
                }
            }
            compared > 0 && black * 100 >= compared * 90
        };
        let marker_matches_reference = |bgra: &[u8], bounds: &RecordedTooltipBounds| {
            let mut matching = 0usize;
            let mut compared = 0usize;
            for y in bounds.top + 3..bounds.bottom - 3 {
                for x in bounds.left + 3..bounds.right - 3 {
                    let frame_x = (x - manifest.capture_bounds.left) as u32;
                    let frame_y = (y - manifest.capture_bounds.top) as u32;
                    let offset = ((frame_y * info.width + frame_x) * 4) as usize;
                    let pixel = &bgra[offset..offset + 4];
                    let expected = marker_reference.get_pixel(
                        (x - manifest.marker_panel_bounds.left) as u32,
                        (y - manifest.marker_panel_bounds.top) as u32,
                    );
                    let max_difference = [
                        pixel[2].abs_diff(expected[0]),
                        pixel[1].abs_diff(expected[1]),
                        pixel[0].abs_diff(expected[2]),
                    ]
                    .into_iter()
                    .max()
                    .unwrap_or_default();
                    if max_difference <= 24 {
                        matching += 1;
                    }
                    compared += 1;
                }
            }
            compared > 0 && matching * 100 >= compared * 90
        };

        let sample_times = [0.1, info.duration / 2.0, late_sample_time];
        for (sample_index, time) in sample_times.into_iter().enumerate() {
            let frame = decoder
                .frame_at(time)
                .unwrap_or_else(|| panic!("could not decode video frame at {time:.3}s"));
            assert_eq!((frame.width, frame.height), (info.width, info.height));
            assert_eq!(frame.bgra.len(), (info.width * info.height * 4) as usize);
            if sample_index == 0 {
                assert!(
                    marker_matches_black(&frame.bgra, &manifest.marker_initial_bounds),
                    "initial video sample does not contain the black marker at its starting position"
                );
                assert!(
                    marker_matches_reference(&frame.bgra, &manifest.marker_moved_bounds),
                    "initial video sample does not match the white underlay at the marker's later position"
                );
            }
            if sample_index == 2 {
                assert!(
                    marker_matches_reference(&frame.bgra, &manifest.marker_initial_bounds),
                    "late video sample does not match the white underlay at the marker's starting position"
                );
                assert!(
                    marker_matches_black(&frame.bgra, &manifest.marker_moved_bounds),
                    "late video sample does not contain the black marker at its later position"
                );
            }
            if sample_index == 0 || sample_index == 2 {
                marker_crop(&frame.bgra)
                    .save(
                        manifest_path
                            .with_file_name(format!("tooltip-video-marker-{sample_index:02}.png")),
                    )
                    .expect("save decoded marker-state evidence");
            }
            if sample_index == 0 {
                let fiducial_export = fiducial_crop(&frame.bgra);
                let export_roi_path =
                    manifest_path.with_file_name(if manifest.editor_preview_path.is_some() {
                        "tooltip-video-fiducial-export.png"
                    } else {
                        "tooltip-video-fiducial-recording.png"
                    });
                fiducial_export
                    .save(&export_roi_path)
                    .expect("save decoded fiducial export ROI");
                let (matches, mean_error, interior_mean_error, max_pixel_error) =
                    fiducial_roi_matches(&fiducial_reference, &fiducial_export);
                assert!(
                    matches,
                    "decoded recording did not preserve the multicolor fiducial ROI: mean channel error {mean_error:.2}, interior mean {interior_mean_error:.2}, max interior channel error {max_pixel_error}"
                );
                for row in 0..6 {
                    for column in 0..8 {
                        let x = column * cell_width + cell_width / 2;
                        let y = row * cell_height + cell_height / 2;
                        let expected = fiducial_reference.get_pixel(x, y);
                        let actual = fiducial_export.get_pixel(x, y);
                        let error = (0..3)
                            .map(|channel| expected[channel].abs_diff(actual[channel]))
                            .max()
                            .unwrap_or_default();
                        assert!(
                            error <= 40,
                            "fiducial cell center {column},{row} differs by {error} channel levels"
                        );
                    }
                }
                let rotated_negative = image::imageops::rotate180(&fiducial_reference);
                let negative_mean_error = fiducial_reference
                    .pixels()
                    .zip(rotated_negative.pixels())
                    .map(|(expected, actual)| {
                        (0..3)
                            .map(|channel| u64::from(expected[channel].abs_diff(actual[channel])))
                            .sum::<u64>()
                    })
                    .sum::<u64>() as f64
                    / (fiducial_width as usize * fiducial_height as usize * 3) as f64;
                assert!(
                    negative_mean_error >= 45.0,
                    "fiducial rotated-pattern negative control is not distinct enough: mean error {negative_mean_error:.2}"
                );
                if let Some(editor_preview_path) = &manifest.editor_preview_path {
                    let editor_preview = image::open(editor_preview_path)
                        .expect("open actual GPUI editor preview screenshot")
                        .to_rgba8();
                    let search = RecordedTooltipBounds {
                        left: 0,
                        top: (editor_preview.height() as f32 * 0.04) as i32,
                        right: (editor_preview.width() as f32 * 0.68) as i32,
                        bottom: (editor_preview.height() as f32 * 0.70) as i32,
                    };
                    let preview_match =
                        preview_fiducial_match(&editor_preview, &fiducial_export, &search).expect(
                            "unique multicolor fiducial match in the actual GPUI video preview",
                        );
                    let initial_marker_matches = marker_preview_sample_errors(
                        &editor_preview,
                        &frame,
                        &manifest.capture_bounds,
                        &manifest.marker_panel_bounds,
                        &manifest.fiducial_bounds,
                        preview_match,
                    );
                    let initial_marker_match_count = initial_marker_matches
                        .iter()
                        .filter(|error| **error <= 48.0)
                        .count();
                    assert!(
                        initial_marker_match_count >= 16,
                        "pre-marker editor preview did not show the initial marker state: {initial_marker_match_count}/18 matched"
                    );
                    let preview_roi = image::imageops::crop_imm(
                        &editor_preview,
                        preview_match.left,
                        preview_match.top,
                        preview_match.width,
                        preview_match.height,
                    )
                    .to_image();
                    preview_roi
                        .save(manifest_path.with_file_name("tooltip-video-fiducial-preview.png"))
                        .expect("save matched GPUI preview fiducial ROI");
                    let negative_match = preview_fiducial_match(
                        &editor_preview,
                        &image::imageops::rotate180(&fiducial_export),
                        &search,
                    );
                    assert!(
                        negative_match.is_none(),
                        "rotated numeric fiducial negative control also matched the GPUI preview"
                    );
                    let source_offset_x =
                        (manifest.fiducial_bounds.left - manifest.capture_bounds.left) as f64;
                    let source_offset_y =
                        (manifest.fiducial_bounds.top - manifest.capture_bounds.top) as f64;
                    let full_left =
                        f64::from(preview_match.left) - source_offset_x * preview_match.scale;
                    let full_top =
                        f64::from(preview_match.top) - source_offset_y * preview_match.scale;
                    let full_right = full_left + f64::from(info.width) * preview_match.scale;
                    let full_bottom = full_top + f64::from(info.height) * preview_match.scale;
                    assert!(full_left >= f64::from(search.left));
                    assert!(full_top >= f64::from(search.top));
                    assert!(full_right <= f64::from(search.right));
                    assert!(full_bottom <= f64::from(search.bottom));
                    eprintln!(
                        "GPUI preview matched trimmed export fiducial ROI at {},{} scale {:.3}, mean cell error {:.2}; approximate frame {:.1},{:.1}-{:.1},{:.1}",
                        preview_match.left,
                        preview_match.top,
                        preview_match.scale,
                        preview_match.mean_error,
                        full_left,
                        full_top,
                        full_right,
                        full_bottom
                    );
                }
                if let (Some(seek_path), Some(seek_time)) = (
                    &manifest.editor_preview_seek_path,
                    manifest.editor_preview_seek_time,
                ) {
                    assert!(seek_time > manifest.marker_move_seconds + 0.2);
                    assert!(seek_time < info.duration - 0.25);
                    let seek_frame = decoder.frame_at(seek_time).unwrap_or_else(|| {
                        panic!("could not decode sought preview frame at {seek_time:.3}s")
                    });
                    assert!(
                        marker_matches_reference(&seek_frame.bgra, &manifest.marker_initial_bounds),
                        "after-seek export frame does not show the marker's initial position as white"
                    );
                    assert!(
                        marker_matches_black(&seek_frame.bgra, &manifest.marker_moved_bounds),
                        "after-seek export frame does not show the moved marker as black"
                    );
                    marker_crop(&seek_frame.bgra)
                        .save(manifest_path.with_file_name("tooltip-video-seek-marker-export.png"))
                        .expect("save after-seek decoded marker ROI");
                    let seek_screenshot = image::open(seek_path)
                        .expect("open actual GPUI editor screenshot after ruler seek")
                        .to_rgba8();
                    let seek_fiducial = fiducial_crop(&seek_frame.bgra);
                    let seek_search = RecordedTooltipBounds {
                        left: 0,
                        top: (seek_screenshot.height() as f32 * 0.04) as i32,
                        right: (seek_screenshot.width() as f32 * 0.68) as i32,
                        bottom: (seek_screenshot.height() as f32 * 0.70) as i32,
                    };
                    let seek_match =
                        preview_fiducial_match(&seek_screenshot, &seek_fiducial, &seek_search)
                            .expect("unique fiducial match in the after-seek editor screenshot");
                    let negative_match = preview_fiducial_match(
                        &seek_screenshot,
                        &image::imageops::rotate180(&seek_fiducial),
                        &seek_search,
                    );
                    assert!(
                        negative_match.is_none(),
                        "rotated fiducial also matched the after-seek preview"
                    );
                    let seek_full_left = f64::from(seek_match.left)
                        - (manifest.fiducial_bounds.left - manifest.capture_bounds.left) as f64
                            * seek_match.scale;
                    let seek_full_top = f64::from(seek_match.top)
                        - (manifest.fiducial_bounds.top - manifest.capture_bounds.top) as f64
                            * seek_match.scale;
                    let seek_full_right = seek_full_left + f64::from(info.width) * seek_match.scale;
                    let seek_full_bottom =
                        seek_full_top + f64::from(info.height) * seek_match.scale;
                    assert!(seek_full_left >= f64::from(seek_search.left));
                    assert!(seek_full_top >= f64::from(seek_search.top));
                    assert!(seek_full_right <= f64::from(seek_search.right));
                    assert!(seek_full_bottom <= f64::from(seek_search.bottom));
                    let seek_matches = marker_preview_sample_errors(
                        &seek_screenshot,
                        &seek_frame,
                        &manifest.capture_bounds,
                        &manifest.marker_panel_bounds,
                        &manifest.fiducial_bounds,
                        seek_match,
                    );
                    let stale_frame = decoder
                        .frame_at(0.1)
                        .expect("decode pre-move stale marker negative frame");
                    assert!(marker_matches_black(
                        &stale_frame.bgra,
                        &manifest.marker_initial_bounds
                    ));
                    assert!(marker_matches_reference(
                        &stale_frame.bgra,
                        &manifest.marker_moved_bounds
                    ));
                    let stale_matches = marker_preview_sample_errors(
                        &seek_screenshot,
                        &stale_frame,
                        &manifest.capture_bounds,
                        &manifest.marker_panel_bounds,
                        &manifest.fiducial_bounds,
                        seek_match,
                    );
                    let seek_error = seek_matches.iter().sum::<f64>() / seek_matches.len() as f64;
                    let stale_error =
                        stale_matches.iter().sum::<f64>() / stale_matches.len() as f64;
                    let seek_match_count =
                        seek_matches.iter().filter(|error| **error <= 48.0).count();
                    let stale_match_count =
                        stale_matches.iter().filter(|error| **error <= 48.0).count();
                    assert!(
                        seek_match_count >= 16
                            && stale_match_count <= 2
                            && stale_error >= seek_error + 80.0,
                        "after-seek preview did not match the moved-marker export: seek={seek_match_count}/18 mean={seek_error:.2}, stale={stale_match_count}/18 mean={stale_error:.2}"
                    );
                    eprintln!(
                        "After-seek preview marker ROI matched export at {seek_time:.3}s: {seek_match_count}/18 samples, mean error {seek_error:.2}; stale polarity {stale_match_count}/18, mean error {stale_error:.2}."
                    );
                }
            }
            let (compared_pixels, high_error_pixels, mean_error, allowed_high_error_pixels, proof) =
                analyze_frame(&frame.bgra);
            let evidence_path =
                manifest_path.with_file_name(format!("tooltip-video-frame-{sample_index:02}.png"));
            proof
                .save(&evidence_path)
                .expect("save decoded tooltip evidence");
            assert!(compared_pixels >= 64, "too few outside-owner video pixels");
            assert!(
                mean_error <= 6.0,
                "video underlay differs from the positive white capture at {time:.3}s: mean channel error {mean_error:.2}"
            );
            assert!(
                high_error_pixels <= allowed_high_error_pixels,
                "tooltip-like video pixels at {time:.3}s: {high_error_pixels} of {compared_pixels} pixels exceeded the 24-level H.264 tolerance"
            );

            if sample_index == 0 {
                let mut visible_tooltip_frame = frame.bgra.to_vec();
                for y in 0..paint_client.height as u32 {
                    for x in 0..paint_client.width as u32 {
                        let frame_x =
                            (paint_client.screen_left - manifest.capture_bounds.left) as u32 + x;
                        let frame_y =
                            (paint_client.screen_top - manifest.capture_bounds.top) as u32 + y;
                        let frame_offset = ((frame_y * info.width + frame_x) * 4) as usize;
                        let pixel = paint_proof.get_pixel(x, y);
                        visible_tooltip_frame[frame_offset..frame_offset + 4]
                            .copy_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
                    }
                }
                let (
                    control_pixels,
                    control_outliers,
                    control_mean_error,
                    control_allowed_outliers,
                    visible_tooltip_roi,
                ) = analyze_frame(&visible_tooltip_frame);
                visible_tooltip_roi
                    .save(manifest_path.with_file_name("tooltip-video-visible-control.png"))
                    .expect("save synthetic visible-tooltip control");
                assert!(control_pixels >= 64, "too few positive-control pixels");
                assert!(
                    control_mean_error > 6.0 || control_outliers > control_allowed_outliers,
                    "the video comparator accepted the actual tooltip WM_PRINTCLIENT pixels against the white underlay"
                );
            }
        }
    }

    #[test]
    fn an_unreadable_file_yields_no_decoder() {
        let missing = crate::util::test_paths::unique_temp("poratake-missing-video.mov");
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
