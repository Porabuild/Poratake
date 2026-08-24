//! H.264/AAC encoding through Media Foundation's sink writer. Media Foundation
//! interfaces are not agile, so the writer lives on one dedicated thread and
//! callers push plain byte buffers to it over a channel.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

/// The audio format every mixed track is resampled to before encoding.
pub const AUDIO_SAMPLE_RATE: u32 = 48_000;
pub const AUDIO_CHANNELS: u32 = 2;
pub const AUDIO_BITS_PER_SAMPLE: u32 = 16;
/// 128 kbps AAC, the rate the AAC encoder accepts for stereo at 48 kHz.
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

#[cfg(not(windows))]
mod backend {
    use super::*;

    pub fn run(
        _path: PathBuf,
        _settings: Settings,
        _commands: Receiver<Command>,
        ready: Sender<Result<(), String>>,
    ) {
        let _ = ready.send(Err("video export is only supported on Windows".into()));
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
