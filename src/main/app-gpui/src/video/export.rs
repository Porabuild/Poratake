//! Video export. Every frame goes through the same composition engine the
//! preview uses, so the file matches what the editor showed; audio is decoded,
//! trimmed to the timeline and mixed here rather than by an external tool.

use std::path::{Path, PathBuf};

use tiny_skia::Pixmap;

use crate::video::composition::segments::{self, VideoSegment};
use crate::video::composition::{Config, Engine, Frames};
use crate::video::decoder::VideoDecoder;
use crate::video::encoder::{Encoder, Settings, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};
use crate::video::{audio, audio_tracks, keyboard_audio, project, sidecars};
use crate::windows::video_editor::model::VideoEditorState;

/// `MAX_H264_DIMENSION` / `MAX_H264_PIXELS` in `export/export-types.ts`.
const MAX_H264_DIMENSION: u32 = 4096;
const MAX_H264_PIXELS: u32 = 8_847_360;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

/// `RESOLUTION_MAP`.
fn resolution_height(resolution: &str) -> Option<u32> {
    match resolution {
        "4k" => Some(2160),
        "1080p" => Some(1080),
        "720p" => Some(720),
        "480p" => Some(480),
        _ => None,
    }
}

/// Port of `calculateExportDimensions`: pick the target height, keep the
/// composition's aspect, round to even, and stay inside H.264's limits.
pub fn export_dimensions(
    composition_width: u32,
    composition_height: u32,
    resolution: &str,
) -> Dimensions {
    let composition_width = composition_width.max(1);
    let composition_height = composition_height.max(1);
    let aspect = composition_width as f64 / composition_height as f64;

    let mut height = resolution_height(resolution).unwrap_or(composition_height);
    let mut width = (height as f64 * aspect).round() as u32;

    if !width.is_multiple_of(2) {
        width += 1;
    }
    if !height.is_multiple_of(2) {
        height += 1;
    }

    if width > MAX_H264_DIMENSION
        || height > MAX_H264_DIMENSION
        || width.saturating_mul(height) > MAX_H264_PIXELS
    {
        let factor = (MAX_H264_DIMENSION as f64 / width as f64)
            .min(MAX_H264_DIMENSION as f64 / height as f64)
            .min((MAX_H264_PIXELS as f64 / (width as f64 * height as f64)).sqrt());
        width = (width as f64 * factor).floor() as u32;
        height = (height as f64 * factor).floor() as u32;
        if !width.is_multiple_of(2) {
            width -= 1;
        }
        if !height.is_multiple_of(2) {
            height -= 1;
        }
    }

    Dimensions {
        width: width.max(2),
        height: height.max(2),
        scale: height as f64 / composition_height as f64,
    }
}

/// `getGifScaleWidth` — a GIF is capped much lower than the video export, so
/// the file stays sendable.
pub fn gif_width(resolution: &str) -> u32 {
    match resolution {
        "1080p" => 1920,
        "480p" => 854,
        _ => 1280,
    }
}

/// The GIF's frame size for a composition, keeping its aspect ratio.
pub fn gif_dimensions(
    composition_width: u32,
    composition_height: u32,
    resolution: &str,
) -> Dimensions {
    let composition_width = composition_width.max(1);
    let composition_height = composition_height.max(1);
    let width = gif_width(resolution).min(composition_width.max(1));
    let height = ((width as f64 * composition_height as f64 / composition_width as f64).round()
        as u32)
        .max(1);
    Dimensions {
        width,
        height,
        scale: height as f64 / composition_height as f64,
    }
}

/// Port of `calculateBitrate`.
pub fn bitrate(width: u32, height: u32, fps: u32, quality: &str, has_camera: bool) -> u32 {
    let (factor, min_mbps, max_mbps) = match quality {
        "social" => (0.07, 8.0, 16.0),
        "web" => (0.028, 1.5, 4.0),
        "web-low" => (0.018, 0.6, 1.5),
        _ => (0.15, 12.0, 100.0),
    };
    let content_factor = if has_camera { 1.15 } else { 1.1 };
    let pixels = width as f64 * height as f64;
    let mbps = (pixels * fps as f64 * factor * content_factor) / 1_000_000.0;
    (mbps.clamp(min_mbps, max_mbps) * 1_000_000.0).round() as u32
}

pub fn frame_rate(state: &VideoEditorState) -> u32 {
    state.export_settings.frame_rate.parse().unwrap_or(60)
}

/// The name the exported file gets next to the project.
pub fn default_output_path(project_or_video: &Path, state: &VideoEditorState) -> PathBuf {
    let extension = if state.export_settings.format == "gif" {
        "gif"
    } else {
        "mp4"
    };
    let name = format!(
        "{}-exported",
        crate::windows::video_editor::model::project_display_name(project_or_video)
    );
    let folder = project::project_folder(project_or_video)
        .and_then(|folder| folder.parent().map(Path::to_path_buf))
        .or_else(|| project_or_video.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    folder.join(format!("{name}.{extension}"))
}

pub struct Request {
    pub project: PathBuf,
    pub output: PathBuf,
    pub state: VideoEditorState,
}

/// Runs the whole export. `progress` is called with a 0..1 fraction; returning
/// `false` from `should_cancel` stops the run and removes the partial file.
pub fn run(
    request: Request,
    progress: &mut dyn FnMut(f32),
    should_cancel: &dyn Fn() -> bool,
) -> Result<PathBuf, String> {
    let is_gif = request.state.export_settings.format == "gif";

    let video_path = project::recording_video_path(&request.project);
    let decoder = VideoDecoder::open(&video_path)
        .ok_or_else(|| "the recording could not be decoded".to_string())?;
    let info = decoder.info();

    let camera_path = project::camera_video_path(&request.project);
    let camera = camera_path
        .is_file()
        .then(|| VideoDecoder::open(&camera_path))
        .flatten();

    let mut config = Config::new(info.width as f64, info.height as f64, request.state.clone());
    config.fps = frame_rate(&request.state).max(1) as f64;
    config.cursor_data = sidecars::load_cursor(&request.project);
    config.keyboard_data = sidecars::load_keyboard(&request.project);
    config.subtitle_data = sidecars::load_subtitle(&request.project);
    config.background_image = config
        .state
        .wallpaper
        .background_image
        .as_deref()
        .and_then(crate::render::gradient::load_image);
    config.first_frame_image = config
        .state
        .first_frame
        .image_data
        .as_deref()
        .and_then(crate::render::gradient::load_image);

    let fps = config.fps as u32;
    let video_segments = segments::to_video_segments(&config.state.segments);
    let mut engine = Engine::new(config);

    let (composition_width, composition_height) = engine.dimensions();
    let dimensions = export_dimensions(
        composition_width,
        composition_height,
        &request.state.export_settings.resolution,
    );

    let total_duration = engine.total_duration();
    if total_duration <= 0.0 {
        return Err("the timeline is empty".to_string());
    }

    let mut session = Session {
        engine: &mut engine,
        decoder: &decoder,
        camera: camera.as_ref(),
        video_segments: &video_segments,
        dimensions,
        fps,
        total_frames: ((total_duration * fps as f64).ceil() as u64).max(1),
        total_duration,
        source_duration: request.state.source_duration.unwrap_or(info.duration),
    };

    if is_gif {
        return run_gif(&request, &mut session, progress, should_cancel);
    }

    let result = encode(
        &request,
        &request.output,
        &mut session,
        progress,
        should_cancel,
    );
    if let Err(error) = result {
        let _ = std::fs::remove_file(&request.output);
        return Err(error);
    }
    Ok(request.output)
}

struct Session<'a> {
    engine: &'a mut Engine,
    decoder: &'a VideoDecoder,
    camera: Option<&'a VideoDecoder>,
    video_segments: &'a [VideoSegment],
    dimensions: Dimensions,
    fps: u32,
    total_frames: u64,
    total_duration: f64,
    source_duration: f64,
}

fn encode(
    request: &Request,
    output: &Path,
    session: &mut Session<'_>,
    progress: &mut dyn FnMut(f32),
    should_cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    let first_frame_duration = session.engine.first_frame_duration();
    let audio_track = build_audio(
        &request.project,
        &request.state,
        session.source_duration,
        (session.total_duration - first_frame_duration).max(0.0),
    );
    let mut encoder = Encoder::create(
        output,
        Settings {
            width: session.dimensions.width,
            height: session.dimensions.height,
            frame_rate: session.fps,
            bitrate: bitrate(
                session.dimensions.width,
                session.dimensions.height,
                session.fps,
                &request.state.export_settings.quality_preset,
                session.camera.is_some(),
            ),
            has_audio: audio_track.is_some(),
        },
    )?;

    if let Some(samples) = audio_track {
        encoder.write_audio(
            audio::to_bytes(&samples),
            audio_offset(first_frame_duration),
        )?;
    }

    for index in 0..session.total_frames {
        if should_cancel() {
            return Err("the export was cancelled".to_string());
        }

        let timeline_time = index as f64 / session.fps as f64;
        let frame = compose_pixmap(session, first_frame_duration, timeline_time)?;
        encoder.write_frame(to_bgra(&frame))?;
        progress((index + 1) as f32 / session.total_frames as f32);
    }

    encoder.finish()
}

fn audio_offset(seconds: f64) -> i64 {
    (seconds.max(0.0) * 10_000_000.0).round() as i64
}

const GIF_FRAME_PASS_SHARE: f32 = 0.7;
const GIF_DIRECT_ENCODE_SHARE: f32 = 0.95;

fn phase_fraction(start: f32, end: f32, completed: f32) -> f32 {
    start + (end - start) * completed.clamp(0.0, 1.0)
}

fn run_gif(
    request: &Request,
    session: &mut Session<'_>,
    progress: &mut dyn FnMut(f32),
    should_cancel: &dyn Fn() -> bool,
) -> Result<PathBuf, String> {
    if let Some(result) = run_gif_with_ffmpeg(request, session, progress, should_cancel) {
        return result;
    }
    run_gif_direct(request, session, progress, should_cancel)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn run_gif_with_ffmpeg(
    request: &Request,
    session: &mut Session<'_>,
    progress: &mut dyn FnMut(f32),
    should_cancel: &dyn Fn() -> bool,
) -> Option<Result<PathBuf, String>> {
    let ffmpeg = crate::video::ffmpeg_path();
    if !ffmpeg.is_file() {
        return None;
    }
    let intermediate = request.output.with_extension("gif-temp.mp4");
    let duration = session.total_duration;
    let width = gif_width(&request.state.export_settings.resolution);
    let fps = session.fps;
    let result = (|| -> Result<(), String> {
        encode(
            request,
            &intermediate,
            session,
            &mut |fraction| progress(phase_fraction(0.0, GIF_FRAME_PASS_SHARE, fraction)),
            should_cancel,
        )?;
        if should_cancel() {
            return Err("the export was cancelled".to_string());
        }
        convert_to_gif(
            &ffmpeg,
            &intermediate,
            &request.output,
            width,
            fps,
            duration,
            progress,
        )
    })();
    let _ = std::fs::remove_file(&intermediate);
    if let Err(error) = result {
        let _ = std::fs::remove_file(&request.output);
        return Some(Err(error));
    }
    progress(1.0);
    Some(Ok(request.output.clone()))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn run_gif_with_ffmpeg(
    _request: &Request,
    _session: &mut Session<'_>,
    _progress: &mut dyn FnMut(f32),
    _should_cancel: &dyn Fn() -> bool,
) -> Option<Result<PathBuf, String>> {
    None
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn ffmpeg_progress_seconds(line: &str) -> Option<f64> {
    let line = line.trim();
    let value = line
        .strip_prefix("out_time_us=")
        .or_else(|| line.strip_prefix("out_time_ms="))?;
    value.parse::<f64>().ok().map(|micros| micros / 1_000_000.0)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn convert_to_gif(
    ffmpeg: &Path,
    input: &Path,
    output: &Path,
    width: u32,
    fps: u32,
    duration: f64,
    progress: &mut dyn FnMut(f32),
) -> Result<(), String> {
    let filter = format!(
        "[0:v]fps={fps},scale={width}:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=256:stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle[out]"
    );
    let mut command = std::process::Command::new(ffmpeg);
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostats",
            "-progress",
            "pipe:1",
            "-y",
            "-i",
        ])
        .arg(input)
        .args(["-filter_complex", &filter, "-map", "[out]", "-loop", "0"])
        .arg(output)
        .stdin(std::process::Stdio::null());

    let (sender, receiver) = std::sync::mpsc::channel();
    let worker = std::thread::Builder::new()
        .name("gif-conversion".into())
        .spawn(move || {
            let mut command = command;
            crate::video::command_stdout(
                &mut command,
                std::time::Duration::from_secs(1800),
                move |stdout| {
                    use std::io::BufRead as _;

                    for line in std::io::BufReader::new(stdout).lines() {
                        let Some(seconds) = ffmpeg_progress_seconds(&line?) else {
                            continue;
                        };
                        if sender.send(seconds).is_err() {
                            break;
                        }
                    }
                    Ok(())
                },
            )
        })
        .map_err(|error| format!("could not run FFmpeg: {error}"))?;

    while let Ok(seconds) = receiver.recv() {
        let completed = match duration > 0.0 {
            true => (seconds / duration) as f32,
            false => 0.0,
        };
        progress(phase_fraction(GIF_FRAME_PASS_SHARE, 1.0, completed));
    }

    let result = worker
        .join()
        .map_err(|_| "the GIF conversion failed".to_string())?
        .map_err(|error| format!("could not run FFmpeg: {error}"))?;
    if result.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&result.stderr).trim().to_string();
    Err(match detail.is_empty() {
        true => "the GIF could not be written".to_string(),
        false => detail,
    })
}

fn run_gif_direct(
    request: &Request,
    session: &mut Session<'_>,
    progress: &mut dyn FnMut(f32),
    should_cancel: &dyn Fn() -> bool,
) -> Result<PathBuf, String> {
    use image::codecs::gif::{GifEncoder, Repeat};

    let (composition_width, composition_height) = session.engine.dimensions();
    session.dimensions = gif_dimensions(
        composition_width,
        composition_height,
        &request.state.export_settings.resolution,
    );

    let file = std::fs::File::create(&request.output)
        .map_err(|error| format!("could not create the file: {error}"))?;
    let mut encoder = GifEncoder::new_with_speed(std::io::BufWriter::new(file), 1);
    encoder
        .set_repeat(Repeat::Infinite)
        .map_err(|error| format!("could not start the GIF: {error}"))?;

    let delay = image::Delay::from_numer_denom_ms(1000, session.fps.max(1));
    let first_frame_duration = session.engine.first_frame_duration();

    for index in 0..session.total_frames {
        if should_cancel() {
            let _ = std::fs::remove_file(&request.output);
            return Err("the export was cancelled".to_string());
        }

        let timeline_time = index as f64 / session.fps as f64;
        let composed = compose_pixmap(session, first_frame_duration, timeline_time)?;
        let frame = image::Frame::from_parts(to_rgba_buffer(&composed), 0, 0, delay);
        encoder
            .encode_frame(frame)
            .map_err(|error| format!("could not write a frame: {error}"))?;
        progress(phase_fraction(
            0.0,
            GIF_DIRECT_ENCODE_SHARE,
            (index + 1) as f32 / session.total_frames as f32,
        ));
    }

    drop(encoder);
    progress(1.0);
    Ok(request.output.clone())
}

/// Straight-alpha RGBA, which the GIF encoder quantizes from.
fn to_rgba_buffer(pixmap: &Pixmap) -> image::RgbaImage {
    let mut buffer = image::RgbaImage::new(pixmap.width(), pixmap.height());
    for (index, pixel) in pixmap.data().as_chunks::<4>().0.iter().enumerate() {
        let alpha = pixel[3];
        let unpremultiply = |value: u8| -> u8 {
            if alpha == 0 {
                0
            } else {
                ((value as u32 * 255 + alpha as u32 / 2) / alpha as u32).min(255) as u8
            }
        };
        let x = index as u32 % pixmap.width();
        let y = index as u32 / pixmap.width();
        buffer.put_pixel(
            x,
            y,
            image::Rgba([
                unpremultiply(pixel[0]),
                unpremultiply(pixel[1]),
                unpremultiply(pixel[2]),
                alpha,
            ]),
        );
    }
    buffer
}

fn compose_pixmap(
    session: &mut Session<'_>,
    first_frame_duration: f64,
    timeline_time: f64,
) -> Result<Pixmap, String> {
    let adjusted = (timeline_time - first_frame_duration).max(0.0);
    let video_time = segments::map_timeline_to_video_time(adjusted, session.video_segments)
        .or_else(|| {
            session
                .video_segments
                .last()
                .map(|segment| segment.end_time)
        })
        .unwrap_or(adjusted);

    let video = session
        .decoder
        .frame_at(video_time)
        .and_then(|frame| to_pixmap(&frame));
    let camera_frame = session
        .camera
        .and_then(|decoder| decoder.frame_at(video_time))
        .and_then(|frame| to_pixmap(&frame));

    session
        .engine
        .render_frame_at(
            timeline_time,
            Frames {
                video: video.as_ref().map(Pixmap::as_ref),
                camera: camera_frame.as_ref().map(Pixmap::as_ref),
            },
            session.dimensions.width,
            session.dimensions.height,
        )
        .ok_or_else(|| "the frame could not be composed".to_string())
}

fn to_pixmap(frame: &crate::video::decoder::DecodedFrame) -> Option<Pixmap> {
    let mut pixmap = Pixmap::new(frame.width.max(1), frame.height.max(1))?;
    let target = pixmap.data_mut();
    if target.len() != frame.bgra.len() {
        return None;
    }
    for (out, pixel) in target
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(frame.bgra.as_chunks::<4>().0)
    {
        out[0] = pixel[2];
        out[1] = pixel[1];
        out[2] = pixel[0];
        out[3] = pixel[3];
    }
    Some(pixmap)
}

/// Unpremultiplies into the BGRA layout Media Foundation's RGB32 input expects.
fn to_bgra(pixmap: &Pixmap) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(pixmap.data().len());
    for pixel in pixmap.data().as_chunks::<4>().0 {
        let alpha = pixel[3];
        let unpremultiply = |value: u8| -> u8 {
            if alpha == 0 {
                0
            } else {
                ((value as u32 * 255 + alpha as u32 / 2) / alpha as u32).min(255) as u8
            }
        };
        bytes.push(unpremultiply(pixel[2]));
        bytes.push(unpremultiply(pixel[1]));
        bytes.push(unpremultiply(pixel[0]));
        bytes.push(255);
    }
    bytes
}

fn build_audio(
    project_path: &Path,
    state: &VideoEditorState,
    source_duration: f64,
    audio_duration: f64,
) -> Option<audio::Pcm> {
    let sources = audio_tracks::Sources::resolve(project_path);
    let mut tracks: Vec<audio::Track> = Vec::new();

    for stem in audio_tracks::stems(&sources, state, source_duration) {
        let volume = stem.volume.resolve(state);
        if volume <= 0.0 {
            continue;
        }
        let Some(samples) = audio::decode(&stem.path) else {
            continue;
        };
        tracks.push(audio::Track {
            samples: audio::place(&samples, &stem.placement),
            volume,
        });
    }

    let keyboard_volume = audio_tracks::VolumeSource::Keyboard.resolve(state);
    if keyboard_volume > 0.0 {
        if let Some(data) = sidecars::load_keyboard(project_path) {
            if let Some(samples) = keyboard_audio::render(
                &data,
                &state.segments,
                &state.audio_style.keyboard_sound_type,
                audio_duration,
            ) {
                tracks.push(audio::Track {
                    samples,
                    volume: keyboard_volume,
                });
            }
        }
    }

    if tracks.is_empty() {
        return None;
    }
    let frames = (audio_duration * AUDIO_SAMPLE_RATE as f64).round() as usize;
    let mixed = audio::mix(&tracks, frames);
    (mixed.len() >= AUDIO_CHANNELS as usize).then_some(mixed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_original_resolution_keeps_the_composition_size() {
        let dimensions = export_dimensions(1920, 1080, "original");
        assert_eq!((dimensions.width, dimensions.height), (1920, 1080));
        assert_eq!(dimensions.scale, 1.0);
    }

    #[test]
    fn a_named_resolution_scales_by_height() {
        let dimensions = export_dimensions(3840, 2160, "1080p");
        assert_eq!((dimensions.width, dimensions.height), (1920, 1080));
        assert_eq!(dimensions.scale, 0.5);
    }

    #[test]
    fn odd_sizes_are_rounded_up_to_even() {
        let dimensions = export_dimensions(1001, 1001, "original");
        assert_eq!(dimensions.width % 2, 0);
        assert_eq!(dimensions.height % 2, 0);
    }

    #[test]
    fn oversized_compositions_are_clamped_into_h264_limits() {
        let dimensions = export_dimensions(8000, 6000, "original");
        assert!(dimensions.width <= MAX_H264_DIMENSION);
        assert!(dimensions.height <= MAX_H264_DIMENSION);
        assert!(dimensions.width * dimensions.height <= MAX_H264_PIXELS);
        assert_eq!(dimensions.width % 2, 0);
    }

    #[test]
    fn bitrates_stay_inside_each_preset_band() {
        let studio = bitrate(1920, 1080, 60, "studio", false);
        assert!((12_000_000..=100_000_000).contains(&studio), "{studio}");

        let web_low = bitrate(1920, 1080, 60, "web-low", false);
        assert!((600_000..=1_500_000).contains(&web_low), "{web_low}");

        // A tiny frame still gets the preset's floor.
        assert_eq!(bitrate(160, 90, 24, "social", false), 8_000_000);
    }

    #[test]
    fn a_camera_raises_the_bitrate() {
        assert!(bitrate(1280, 720, 30, "web", true) >= bitrate(1280, 720, 30, "web", false));
    }

    #[test]
    fn the_frame_rate_comes_from_the_export_settings() {
        let mut state = VideoEditorState::default();
        assert_eq!(frame_rate(&state), 30);
        state.export_settings.frame_rate = "24".into();
        assert_eq!(frame_rate(&state), 24);
        state.export_settings.frame_rate = "nonsense".into();
        assert_eq!(frame_rate(&state), 60);
    }

    #[test]
    fn the_default_output_sits_beside_the_project_with_an_exported_stem() {
        let mut state = VideoEditorState::default();
        let path = default_output_path(Path::new("/tmp/Take 1.poratake"), &state);
        assert_eq!(path, PathBuf::from("/tmp/Take 1-exported.mp4"));

        state.export_settings.format = "gif".into();
        assert_eq!(
            default_output_path(Path::new("/tmp/Take 1.poratake"), &state),
            PathBuf::from("/tmp/Take 1-exported.gif")
        );
    }

    fn gif_progress_sequence(frames: u64, conversion_samples: u64) -> Vec<f32> {
        let mut values = Vec::new();
        for index in 0..frames {
            values.push(phase_fraction(
                0.0,
                GIF_FRAME_PASS_SHARE,
                (index + 1) as f32 / frames as f32,
            ));
        }
        for sample in 0..conversion_samples {
            values.push(phase_fraction(
                GIF_FRAME_PASS_SHARE,
                1.0,
                (sample + 1) as f32 / conversion_samples as f32,
            ));
        }
        values
    }

    #[test]
    fn the_gif_progress_never_goes_backwards_and_reaches_the_end_only_at_the_end() {
        let values = gif_progress_sequence(60, 20);

        assert!(
            values.windows(2).all(|pair| pair[1] >= pair[0]),
            "{values:?}"
        );
        assert!(values[0] < 0.05, "{}", values[0]);
        assert_eq!(values.last().copied(), Some(1.0));
        assert!(values[..values.len() - 1].iter().all(|value| *value < 1.0));
    }

    #[test]
    fn the_gif_progress_advances_in_small_steps_through_the_conversion() {
        let values = gif_progress_sequence(60, 20);
        let biggest_step = values
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .fold(0.0_f32, f32::max);
        assert!(biggest_step < 0.05, "{biggest_step}");

        let during_conversion = values
            .iter()
            .filter(|value| **value > GIF_FRAME_PASS_SHARE && **value < 1.0)
            .count();
        assert!(during_conversion >= 10, "{during_conversion}");
    }

    #[test]
    fn a_phase_fraction_stays_inside_its_own_band() {
        assert_eq!(phase_fraction(0.0, GIF_FRAME_PASS_SHARE, -1.0), 0.0);
        assert_eq!(phase_fraction(0.0, GIF_FRAME_PASS_SHARE, 2.0), 0.7);
        assert_eq!(phase_fraction(GIF_FRAME_PASS_SHARE, 1.0, 0.0), 0.7);
        assert_eq!(phase_fraction(GIF_FRAME_PASS_SHARE, 1.0, 1.0), 1.0);
    }

    #[test]
    fn the_in_process_gif_encoder_leaves_room_for_the_flush() {
        let last_frame = phase_fraction(0.0, GIF_DIRECT_ENCODE_SHARE, 1.0);
        assert!(last_frame < 1.0, "{last_frame}");
        assert!(last_frame > GIF_FRAME_PASS_SHARE, "{last_frame}");
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn ffmpeg_progress_lines_are_read_as_seconds() {
        assert_eq!(ffmpeg_progress_seconds("out_time_us=1500000"), Some(1.5));
        assert_eq!(ffmpeg_progress_seconds("out_time_ms=2000000"), Some(2.0));
        assert_eq!(ffmpeg_progress_seconds("out_time_us=N/A"), None);
        assert_eq!(ffmpeg_progress_seconds("frame=12"), None);
        assert_eq!(ffmpeg_progress_seconds("progress=end"), None);
    }

    #[test]
    fn premultiplied_pixels_come_back_as_opaque_bgra() {
        let mut pixmap = Pixmap::new(1, 1).expect("pixmap");
        pixmap.fill(tiny_skia::Color::from_rgba8(10, 20, 30, 255));
        assert_eq!(to_bgra(&pixmap), vec![30, 20, 10, 255]);
    }

    #[test]
    fn the_audio_offset_is_the_first_frame_in_media_units() {
        assert_eq!(audio_offset(0.0), 0);
        assert_eq!(audio_offset(1.0 / 60.0), 166_667);
    }

    #[test]
    fn gif_dimensions_cap_the_width_and_keep_the_aspect() {
        let dimensions = gif_dimensions(1920, 1080, "original");
        assert_eq!((dimensions.width, dimensions.height), (1280, 720));

        let hd = gif_dimensions(1920, 1080, "1080p");
        assert_eq!((hd.width, hd.height), (1920, 1080));

        // A composition narrower than the cap is never upscaled.
        let small = gif_dimensions(640, 480, "1080p");
        assert_eq!((small.width, small.height), (640, 480));
    }

    #[test]
    fn an_export_with_no_recording_fails_before_writing_anything() {
        let output = crate::util::test_paths::unique_temp("poratake-export-test.gif");
        let _ = std::fs::remove_file(&output);
        let mut state = VideoEditorState::default();
        state.export_settings.format = "gif".into();
        let error = run(
            Request {
                project: crate::util::test_paths::unique_temp("poratake-missing.poratake"),
                output: output.clone(),
                state,
            },
            &mut |_| {},
            &|| false,
        )
        .unwrap_err();
        assert!(error.contains("decoded"), "{error}");
        assert!(!output.exists());
    }
}
