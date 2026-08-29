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

#[cfg(not(windows))]
mod backend {
    use super::*;

    pub fn run(_path: PathBuf, _commands: Receiver<Command>, info_tx: Sender<Option<VideoInfo>>) {
        let _ = info_tx.send(None);
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
