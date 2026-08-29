//! Video decoding for the editor preview. Media Foundation interfaces are not
//! agile, so the reader lives on one dedicated thread and callers talk to it
//! over a channel; decoded frames are plain byte buffers, which are `Send`.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

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
pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
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

            while let Ok(command) = commands.recv() {
                match command {
                    Command::Seek { time, reply } => {
                        let frame = read_frame(&reader, time, info);
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

    unsafe fn read_frame(
        reader: &IMFSourceReader,
        time: f64,
        info: VideoInfo,
    ) -> Option<DecodedFrame> {
        let position = (time * UNITS_PER_SECOND) as i64;
        let mut variant = PROPVARIANT::default();
        {
            let inner = &mut *variant.Anonymous.Anonymous;
            inner.vt = VT_I8;
            inner.Anonymous.hVal = position;
        }
        let _ = reader.SetCurrentPosition(&GUID::zeroed(), &variant);

        // Decoders emit until they land on or past the requested time.
        for _ in 0..64 {
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
                return None;
            }
            let Some(sample) = sample else {
                continue;
            };
            if (timestamp as f64) + 1.0 < position as f64 {
                continue;
            }
            return copy_sample(&sample, info);
        }
        None
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
            bgra,
        })
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
