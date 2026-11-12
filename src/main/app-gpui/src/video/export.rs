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
    should_cancel: &(dyn Fn() -> bool + Sync),
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

    let result = if is_gif {
        run_gif(&request, &mut session, progress, should_cancel)
    } else {
        let result = encode(
            &request,
            &request.output,
            &mut session,
            progress,
            should_cancel,
        );
        if result.is_err() {
            let _ = std::fs::remove_file(&request.output);
        }
        result.map(|()| request.output.clone())
    };
    if result.is_ok() && should_cancel() {
        let _ = std::fs::remove_file(&request.output);
        return Err("the export was cancelled".to_string());
    }
    result
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
            drop(encoder);
            return Err("the export was cancelled".to_string());
        }

        let timeline_time = index as f64 / session.fps as f64;
        let frame = compose_pixmap(session, first_frame_duration, timeline_time)?;
        encoder.write_frame(to_bgra(&frame))?;
        progress((index + 1) as f32 / session.total_frames as f32);
    }

    encoder.finish_cancellable(should_cancel)
}

fn audio_offset(seconds: f64) -> i64 {
    (seconds.max(0.0) * 10_000_000.0).round() as i64
}

#[cfg(any(test, target_os = "macos", target_os = "linux"))]
const GIF_FRAME_PASS_SHARE: f32 = 0.7;
const GIF_DIRECT_ENCODE_SHARE: f32 = 0.95;

fn phase_fraction(start: f32, end: f32, completed: f32) -> f32 {
    start + (end - start) * completed.clamp(0.0, 1.0)
}

fn run_gif(
    request: &Request,
    session: &mut Session<'_>,
    progress: &mut dyn FnMut(f32),
    should_cancel: &(dyn Fn() -> bool + Sync),
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
    should_cancel: &(dyn Fn() -> bool + Sync),
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
            should_cancel,
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
    _should_cancel: &(dyn Fn() -> bool + Sync),
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
    should_cancel: &(dyn Fn() -> bool + Sync),
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
    let result = std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("gif-conversion".into())
            .spawn_scoped(scope, move || {
                let mut command = command;
                crate::video::command_stdout(
                    &mut command,
                    std::time::Duration::from_secs(1800),
                    should_cancel,
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

        worker
            .join()
            .map_err(|_| "the GIF conversion failed".to_string())?
            .map_err(|error| format!("could not run FFmpeg: {error}"))
    })?;
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

    let fps = u64::from(session.fps);
    let mut elapsed_centiseconds = 0;
    let first_frame_duration = session.engine.first_frame_duration();

    for index in 0..session.total_frames {
        if should_cancel() {
            drop(encoder);
            let _ = std::fs::remove_file(&request.output);
            return Err("the export was cancelled".to_string());
        }

        let timeline_time = index as f64 / session.fps as f64;
        let composed = compose_pixmap(session, first_frame_duration, timeline_time)?;
        let next_centiseconds = ((index + 1) * 100 + fps / 2) / fps;
        let delay = image::Delay::from_numer_denom_ms(
            ((next_centiseconds - elapsed_centiseconds) * 10) as u32,
            1,
        );
        elapsed_centiseconds = next_centiseconds;
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

    #[cfg(windows)]
    #[test]
    fn audio_preview_and_export_follow_volume_changes_across_a_cut() {
        use crate::video::audio_tracks::VolumeSource;
        use crate::video::preview_audio::{self, RebuildInputs, StemSignature};
        use crate::windows::video_editor::model;
        use std::collections::HashMap;

        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Audio workflow.poratake");
        std::fs::create_dir(&project).unwrap();
        let mut encoder = Encoder::create(
            &project::recording_video_path(&project),
            Settings {
                width: 320,
                height: 180,
                frame_rate: 12,
                bitrate: 1_000_000,
                has_audio: true,
            },
        )
        .unwrap();
        let samples: Vec<i16> = (0..AUDIO_SAMPLE_RATE * 2)
            .flat_map(|frame| {
                let frequency = if frame < AUDIO_SAMPLE_RATE {
                    440.
                } else {
                    880.
                };
                let phase =
                    frame as f64 * frequency * std::f64::consts::TAU / f64::from(AUDIO_SAMPLE_RATE);
                let mut value = phase.sin() * 5000.;
                for start in [AUDIO_SAMPLE_RATE * 48 / 100, AUDIO_SAMPLE_RATE * 160 / 100] {
                    if (start..start + AUDIO_SAMPLE_RATE / 100).contains(&frame) {
                        let phase = f64::from(frame - start) * 3000. * std::f64::consts::TAU
                            / f64::from(AUDIO_SAMPLE_RATE);
                        value += phase.sin() * 12_000.;
                    }
                }
                let sample = value as i16;
                [sample, sample]
            })
            .collect();
        encoder.write_audio(audio::to_bytes(&samples), 0).unwrap();
        for _ in 0..24 {
            encoder
                .write_frame([32, 64, 128, 255].repeat(320 * 180))
                .unwrap();
        }
        encoder.finish().unwrap();
        std::fs::write(
            project::keys_path(&project),
            serde_json::to_vec(&serde_json::json!({
                "events": [
                    {"timestamp": 0.3, "key": "a", "type": "down"},
                    {"timestamp": 1.0, "key": "b", "type": "down"},
                    {"timestamp": 1.4, "key": "c", "type": "down"}
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let mut state = VideoEditorState {
            source_duration: Some(2.),
            segments: vec![
                model::Segment {
                    original_end: 0.75,
                    speed: Some(1.5),
                    ..model::Segment::spanning(2.)
                },
                model::Segment {
                    id: "after-cut".into(),
                    original_start: 1.25,
                    speed: Some(0.5),
                    ..model::Segment::spanning(2.)
                },
            ],
            ..VideoEditorState::default()
        };
        state.wallpaper.enabled = false;
        state.export_settings.resolution = "original".into();
        state.export_settings.frame_rate = "12".into();
        state.audio_style.system_audio_volume = 0.35;
        state.audio_style.keyboard_sound_enabled = true;
        state.audio_style.keyboard_sound_volume = 0.;
        let built =
            preview_audio::build_stems(RebuildInputs::capture(&project, &state, &HashMap::new()));
        let keyboard = built
            .stems
            .iter()
            .find(|stem| stem.volume == VolumeSource::Keyboard)
            .expect("keyboard clicks must be ready while muted");
        for start in [0.2, 0.8] {
            let start = (start * f64::from(AUDIO_SAMPLE_RATE)) as usize * 2;
            let end = start + AUDIO_SAMPLE_RATE as usize / 10 * 2;
            assert!(keyboard.samples[start..end]
                .iter()
                .any(|sample| *sample != 0));
        }
        assert!(built
            .stems
            .iter()
            .any(|stem| stem.volume == VolumeSource::SystemAudio));

        state.audio_style.keyboard_sound_volume = 0.6;
        assert_eq!(built.signature, StemSignature::capture(&state));
        model::save_state(&project, &state);
        assert_eq!(model::load_state(&project), state);
        let tracks: Vec<audio::Track> = built
            .stems
            .iter()
            .map(|stem| audio::Track {
                samples: stem.samples.as_ref().clone(),
                volume: stem.volume.resolve(&state),
            })
            .collect();
        let preview = audio::mix(&tracks, AUDIO_SAMPLE_RATE as usize * 2);
        let other_tracks: Vec<audio::Track> = built
            .stems
            .iter()
            .filter(|stem| stem.volume != VolumeSource::Keyboard)
            .map(|stem| audio::Track {
                samples: stem.samples.as_ref().clone(),
                volume: stem.volume.resolve(&state),
            })
            .collect();
        let without_keyboard = audio::mix(&other_tracks, AUDIO_SAMPLE_RATE as usize * 2);
        assert_eq!(build_audio(&project, &state, 2., 2.).unwrap(), preview);

        let output = directory.path().join("audio-edited.mp4");
        run(
            Request {
                project,
                output: output.clone(),
                state,
            },
            &mut |_| {},
            &|| false,
        )
        .unwrap();
        let exported = audio::decode(&output).expect("exported audio");
        let duration = exported.len() as f64 / f64::from(AUDIO_SAMPLE_RATE * AUDIO_CHANNELS);
        assert!(
            (duration - 2.).abs() < 0.05,
            "exported audio duration {duration}s"
        );
        for (time, frequency) in [
            (0.05, 440.),
            (0.35, 440.),
            (0.6, 880.),
            (1.4, 880.),
            (1.85, 880.),
        ] {
            let start = (time * f64::from(AUDIO_SAMPLE_RATE)) as usize * 2;
            let end = start + AUDIO_SAMPLE_RATE as usize / 10 * 2;
            for (name, samples) in [("preview", &preview), ("export", &exported)] {
                let strength = audio::tone_strength(&samples[start..end], frequency);
                assert!(
                    strength > 0.5,
                    "{name} has the wrong {frequency} Hz tone at {time}s: {strength}"
                );
            }
        }
        for time in [0.32, 1.2] {
            let center = (time * f64::from(AUDIO_SAMPLE_RATE)) as usize;
            let radius = AUDIO_SAMPLE_RATE as usize / 25;
            let length = AUDIO_SAMPLE_RATE as usize / 100;
            let step = AUDIO_SAMPLE_RATE as usize / 200;
            for (name, samples) in [("preview", &preview), ("export", &exported)] {
                let present = (center - radius..center + radius - length)
                    .step_by(step)
                    .any(|start| {
                        let samples = &samples[start * 2..(start + length) * 2];
                        audio::tone_strength(samples, 3000.) > 0.5
                            && samples.iter().any(|sample| sample.unsigned_abs() > 2500)
                    });
                assert!(present, "{name} lost the short sound near {time}s");
            }
        }
        let count = exported.len().min(preview.len());
        assert!(count >= AUDIO_SAMPLE_RATE as usize * 3);
        let error: f64 = exported[..count]
            .iter()
            .zip(&preview[..count])
            .map(|(actual, expected)| f64::from((*actual as i32 - *expected as i32).abs()))
            .sum::<f64>()
            / count as f64;
        assert!(error < 500., "preview/export audio error {error}");
        for time in [0.2, 0.8] {
            let start = (time * f64::from(AUDIO_SAMPLE_RATE)) as usize * 2;
            let end = start + AUDIO_SAMPLE_RATE as usize / 10 * 2;
            let difference: f64 = preview[start..end]
                .iter()
                .zip(&without_keyboard[start..end])
                .map(|(actual, baseline)| f64::from((*actual as i32 - *baseline as i32).abs()))
                .sum::<f64>()
                / (end - start) as f64;
            let error: f64 = exported[start..end]
                .iter()
                .zip(&preview[start..end])
                .map(|(actual, expected)| f64::from((*actual as i32 - *expected as i32).abs()))
                .sum::<f64>()
                / (end - start) as f64;
            assert!(difference > 100., "keyboard click missing at {time}s");
            assert!(
                error < difference / 2.,
                "keyboard click export error {error} at {time}s"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn cursor_keyboard_and_subtitles_match_export_across_timeline_edits() {
        use crate::windows::video_editor::{model, preview};

        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Overlay workflow.poratake");
        std::fs::create_dir(&project).unwrap();
        let mut encoder = Encoder::create(
            &project::recording_video_path(&project),
            Settings {
                width: 960,
                height: 720,
                frame_rate: 12,
                bitrate: 12_000_000,
                has_audio: false,
            },
        )
        .unwrap();
        for index in 0..24 {
            let color = if index < 12 {
                [176, 96, 32, 255]
            } else {
                [48, 80, 192, 255]
            };
            encoder.write_frame(color.repeat(960 * 720)).unwrap();
        }
        encoder.finish().unwrap();
        for (path, data) in [
            (
                project::cursor_path(&project),
                serde_json::json!({
                    "recordingArea": {"width": 960, "height": 720},
                    "events": [
                        {"timestamp": 0., "x": 0.2, "y": 0.1, "type": "move"},
                        {"timestamp": 1.25, "x": 0.7, "y": 0.1, "type": "move"}
                    ]
                }),
            ),
            (
                project::keys_path(&project),
                serde_json::json!({
                    "events": [
                        {"timestamp": 0.25, "key": "c", "modifiers": ["control"], "type": "down"},
                        {"timestamp": 1.3, "key": "v", "modifiers": ["control"], "type": "down"}
                    ],
                    "meta": {"platform": "windows"}
                }),
            ),
            (
                project::subtitle_path(&project),
                serde_json::json!({
                    "segments": [
                        {"start": 0.2, "end": 0.6, "text": "A"},
                        {"start": 1.3, "end": 1.6, "text": "B"}
                    ]
                }),
            ),
        ] {
            std::fs::write(path, serde_json::to_vec(&data).unwrap()).unwrap();
        }
        let mut state = VideoEditorState {
            source_duration: Some(2.),
            segments: vec![
                model::Segment {
                    original_end: 0.75,
                    speed: Some(1.5),
                    ..model::Segment::spanning(2.)
                },
                model::Segment {
                    id: "after-cut".into(),
                    original_start: 1.25,
                    speed: Some(0.5),
                    ..model::Segment::spanning(2.)
                },
            ],
            ..VideoEditorState::default()
        };
        state.wallpaper.enabled = false;
        state.export_settings.resolution = "original".into();
        state.export_settings.frame_rate = "12".into();
        state.cursor_style.smoothing = 0.;
        state.cursor_style.motion_blur = false;
        state.keyboard_style.visible = true;
        state.keyboard_style.display_duration = 0.3;
        let segments = crate::video::composition::segments::to_video_segments(&state.segments);
        let keys = sidecars::load_keyboard(&project).unwrap();
        let subtitles = sidecars::load_subtitle(&project).unwrap();
        for (time, key, caption) in [
            (0.25, Some("Ctrl+C"), Some("A")),
            (0.75, Some("Ctrl+V"), Some("B")),
            (1.25, None, None),
        ] {
            let active = crate::video::composition::keyboard::active_keys(
                &keys,
                &segments,
                time,
                &state.keyboard_style,
            );
            assert_eq!(active.first().map(|value| value.display_text.as_str()), key);
            assert_eq!(active.len(), usize::from(key.is_some()));
            let active =
                crate::video::composition::subtitle::active_subtitle(&subtitles, &segments, time);
            assert_eq!(
                active
                    .as_ref()
                    .and_then(|value| value.words.first())
                    .map(String::as_str),
                caption
            );
        }
        model::save_state(&project, &state);
        assert_eq!(model::load_state(&project), state);

        let mut plain_state = state.clone();
        plain_state.cursor_style.enabled = false;
        plain_state.keyboard_style.visible = false;
        plain_state.subtitle_style.visible = false;
        let mut cursor_state = plain_state.clone();
        cursor_state.cursor_style.enabled = true;
        let mut keyboard_state = plain_state.clone();
        keyboard_state.keyboard_style.visible = true;
        let mut subtitle_state = plain_state.clone();
        subtitle_state.subtitle_style.visible = true;
        let mut plain = preview::Source::open(&project, plain_state).unwrap();
        let mut cursor = preview::Source::open(&project, cursor_state).unwrap();
        let mut keyboard = preview::Source::open(&project, keyboard_state).unwrap();
        let mut subtitle = preview::Source::open(&project, subtitle_state).unwrap();
        let mut combined = preview::Source::open(&project, model::load_state(&project)).unwrap();
        let mut without_cursor = state.clone();
        without_cursor.cursor_style.enabled = false;
        let mut without_keyboard = state.clone();
        without_keyboard.keyboard_style.visible = false;
        let mut without_subtitle = state.clone();
        without_subtitle.subtitle_style.visible = false;
        let mut without_cursor = preview::Source::open(&project, without_cursor).unwrap();
        let mut without_keyboard = preview::Source::open(&project, without_keyboard).unwrap();
        let mut without_subtitle = preview::Source::open(&project, without_subtitle).unwrap();

        let output = directory.path().join("overlay-edited.mp4");
        run(
            Request {
                project,
                output: output.clone(),
                state,
            },
            &mut |_| {},
            &|| false,
        )
        .unwrap();
        let exported = VideoDecoder::open(&output).unwrap();
        assert!((exported.info().duration - 2.).abs() <= 1. / 12.);
        for (time, overlays_visible, source_color) in [
            (0.25, true, [176_u8, 96, 32]),
            (0.75, true, [48, 80, 192]),
            (1.25, false, [48, 80, 192]),
        ] {
            let baseline = plain.compose(time, None).unwrap();
            for (name, source, visible) in [
                ("cursor", &mut cursor, true),
                ("keyboard", &mut keyboard, overlays_visible),
                ("subtitle", &mut subtitle, overlays_visible),
            ] {
                let frame = source.compose(time, None).unwrap();
                let changed = frame
                    .data()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(baseline.data().as_chunks::<4>().0)
                    .filter(|(actual, baseline)| actual != baseline)
                    .count();
                if visible {
                    assert!(changed > 30, "{name} must be visible at {time}s");
                } else {
                    assert_eq!(changed, 0, "{name} must be hidden at {time}s");
                }
            }
            let expected = to_bgra(&combined.compose(time, None).unwrap());
            let actual = exported.frame_at(time).unwrap();
            assert_eq!(actual.bgra.len(), expected.len());
            let error: u64 = actual
                .bgra
                .iter()
                .zip(&expected)
                .map(|(actual, expected)| u64::from(actual.abs_diff(*expected)))
                .sum();
            let mean = error as f64 / expected.len() as f64;
            assert!(mean < 3., "preview/export channel error {mean} at {time}s");
            for (name, source, visible) in [
                ("cursor", &mut without_cursor, true),
                ("keyboard", &mut without_keyboard, overlays_visible),
                ("subtitle", &mut without_subtitle, overlays_visible),
            ] {
                if !visible {
                    continue;
                }
                let without = to_bgra(&source.compose(time, None).unwrap());
                let (error, count) = expected
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(without.as_chunks::<4>().0)
                    .zip(actual.bgra.as_chunks::<4>().0)
                    .filter(|((expected, without), _)| expected != without)
                    .fold((0_u64, 0_u64), |(error, count), ((expected, _), actual)| {
                        let delta: u64 = expected[..3]
                            .iter()
                            .zip(&actual[..3])
                            .map(|(expected, actual)| u64::from(expected.abs_diff(*actual)))
                            .sum();
                        (error + delta, count + 1)
                    });
                assert!(
                    count > 30,
                    "{name} must affect the combined preview at {time}s"
                );
                let mean = error as f64 / (count * 3) as f64;
                assert!(mean < 8., "{name} preview/export error {mean} at {time}s");
            }
            let pixel = (50 * 960 + 800) * 4;
            for channel in 0..3 {
                assert!(expected[pixel + channel].abs_diff(source_color[channel]) < 8);
                assert!(actual.bgra[pixel + channel].abs_diff(source_color[channel]) < 8);
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn edited_video_exports_the_preview_and_reopens_the_saved_project() {
        use crate::editor::annotations::Annotation;
        use crate::windows::video_editor::{model, preview};

        let directory = tempfile::tempdir().unwrap();
        let recording = directory.path().join("recording.mp4");
        let mut encoder = Encoder::create(
            &recording,
            Settings {
                width: 320,
                height: 180,
                frame_rate: 12,
                bitrate: 12_000_000,
                has_audio: false,
            },
        )
        .unwrap();
        for index in 0..36 {
            let color = match index / 12 {
                0 => [176, 96, 32, 255],
                1 => [64, 160, 64, 255],
                _ => [48, 80, 192, 255],
            };
            let mut frame = color.repeat(320 * 180);
            for y in 60..80 {
                for x in (0..30).chain(160..190).chain(200..230) {
                    let shade = if x % 2 == 0 { 16 } else { 240 };
                    let pixel = (y * 320 + x) * 4;
                    frame[pixel..pixel + 4].copy_from_slice(&[shade, shade, shade, 255]);
                }
            }
            encoder.write_frame(frame).unwrap();
        }
        encoder.finish().unwrap();
        let mut state = VideoEditorState {
            source_duration: Some(3.),
            segments: vec![
                model::Segment {
                    original_start: 0.25,
                    original_end: 1.,
                    speed: Some(2.),
                    ..model::Segment::spanning(3.)
                },
                model::Segment {
                    id: "second".into(),
                    original_start: 2.,
                    original_end: 2.5,
                    speed: Some(0.5),
                    ..model::Segment::spanning(3.)
                },
            ],
            ..VideoEditorState::default()
        };
        state.export_settings.resolution = "original".into();
        state.export_settings.frame_rate = "8".into();
        state.wallpaper.enabled = true;
        state.wallpaper.padding = 12.;
        state.wallpaper.corners = 8.;
        state.wallpaper.gradient = Some(serde_json::json!({
            "id": "translucent-export-test",
            "colors": ["#80402080", "#80402080"],
            "angle": 135.0,
        }));
        state.drawing_segments.push(model::DrawingSegment {
            id: "drawing".into(),
            start_time: 0.,
            end_time: 2.,
            canvas_width: 320.,
            canvas_height: 180.,
            annotations: vec![
                Annotation::Rectangle {
                    id: "shape".into(),
                    x: 24.,
                    y: 24.,
                    width: 72.,
                    height: 48.,
                    stroke: "#ffffff".into(),
                    stroke_width: 3.,
                    fill: Some("#e04040".into()),
                },
                Annotation::Redact {
                    id: "pixelate".into(),
                    x: 160.,
                    y: 60.,
                    width: 30.,
                    height: 20.,
                    style: "pixelate".into(),
                    intensity: 5.,
                },
                Annotation::Redact {
                    id: "blur".into(),
                    x: 200.,
                    y: 60.,
                    width: 30.,
                    height: 20.,
                    style: "blur".into(),
                    intensity: 5.,
                },
                Annotation::Redact {
                    id: "clipped-blur".into(),
                    x: -8.,
                    y: 60.,
                    width: 30.,
                    height: 20.,
                    style: "blur".into(),
                    intensity: 5.,
                },
            ],
        });
        model::save_state(&recording, &state);
        assert_eq!(model::load_state(&recording), state);
        let mut preview = preview::Source::open(&recording, model::load_state(&recording)).unwrap();
        let mut unredacted_state = state.clone();
        unredacted_state.drawing_segments[0].annotations.truncate(1);
        let mut unredacted = preview::Source::open(&recording, unredacted_state).unwrap();
        let output = directory.path().join("edited.mp4");
        let mut progress = 0.;
        run(
            Request {
                project: recording,
                output: output.clone(),
                state,
            },
            &mut |value| progress = value,
            &|| false,
        )
        .unwrap();
        assert_eq!(progress, 1.);
        let exported = VideoDecoder::open(&output).unwrap();
        let info = exported.info();
        assert_eq!((info.width, info.height), (344, 204));
        assert!((info.duration - 1.375).abs() <= 1. / 8., "{info:?}");
        for time in [0., 0.25, 0.5, 1.25] {
            let composed = preview.compose(time, None).unwrap();
            assert_eq!(
                composed.data()[3],
                128,
                "wallpaper padding must retain alpha"
            );
            let preview_image = preview::to_render_image_opaque(&composed).unwrap();
            let expected = preview_image.as_bytes(0).unwrap().to_vec();
            assert_eq!(expected, to_bgra(&composed));
            let original = to_bgra(&unredacted.compose(time, None).unwrap());
            let actual = exported.frame_at(time).unwrap();
            assert_eq!(actual.bgra.len(), expected.len());
            let error: u64 = actual
                .bgra
                .iter()
                .zip(&expected)
                .map(|(actual, expected)| u64::from(actual.abs_diff(*expected)))
                .sum();
            let mean_error = error as f64 / expected.len() as f64;
            assert!(
                mean_error < 3.,
                "preview/export mean channel error {mean_error} at {time}s"
            );
            let mut drawing_error = 0_u64;
            for y in 48..72 {
                for x in 48..88 {
                    let pixel = (y * info.width as usize + x) * 4;
                    for (channel, expected_color) in [64_u8, 64, 224].into_iter().enumerate() {
                        assert!(
                            expected[pixel + channel].abs_diff(expected_color) <= 2,
                            "the preview must contain the drawing at {time}s"
                        );
                        drawing_error += u64::from(
                            actual.bgra[pixel + channel].abs_diff(expected[pixel + channel]),
                        );
                    }
                }
            }
            assert!(
                drawing_error as f64 / (24. * 40. * 3.) < 3.,
                "the exported drawing differs from preview at {time}s"
            );
            for (style, left, top, width, height) in [
                ("pixelate", 172, 72, 30, 20),
                ("blur", 212, 72, 30, 20),
                ("clipped blur", 0, 68, 24, 23),
            ] {
                let mut redaction_change = 0_u64;
                let mut redaction_error = 0_u64;
                for y in top..top + height {
                    for x in left..left + width {
                        let pixel = (y * info.width as usize + x) * 4;
                        for channel in 0..3 {
                            redaction_change += u64::from(
                                expected[pixel + channel].abs_diff(original[pixel + channel]),
                            );
                            redaction_error += u64::from(
                                actual.bgra[pixel + channel].abs_diff(expected[pixel + channel]),
                            );
                        }
                    }
                }
                assert!(
                    redaction_change as f64 / (height as f64 * width as f64 * 3.) > 8.,
                    "{style} must change the preview at {time}s"
                );
                assert!(
                    redaction_error as f64 / (height as f64 * width as f64 * 3.) < 8.,
                    "the exported {style} differs from preview at {time}s"
                );
            }
            for (x, y) in [(0, 80), (23, 80), (10, 68), (10, 90)] {
                let pixel = (y * info.width as usize + x) * 4;
                let effect: u16 = expected[pixel..pixel + 3]
                    .iter()
                    .zip(&original[pixel..pixel + 3])
                    .map(|(actual, original)| u16::from(actual.abs_diff(*original)))
                    .sum();
                assert!(
                    effect > 24,
                    "clipped blur did not affect its bounds at {time}s, {x},{y}: {effect}"
                );
                let deviation = (0..3)
                    .map(|channel| actual.bgra[pixel + channel].abs_diff(expected[pixel + channel]))
                    .max()
                    .unwrap();
                assert!(
                    deviation <= 40,
                    "clipped blur export differs from preview at {time}s, {x},{y}: {deviation}"
                );
            }
            for (x, y) in [(24, 80), (30, 80), (10, 67), (10, 91)] {
                let pixel = (y * info.width as usize + x) * 4;
                assert_eq!(
                    &expected[pixel..pixel + 3],
                    &original[pixel..pixel + 3],
                    "clipped blur changed a preview pixel outside its bounds at {time}s, {x},{y}"
                );
                let deviation = (0..3)
                    .map(|channel| actual.bgra[pixel + channel].abs_diff(expected[pixel + channel]))
                    .max()
                    .unwrap();
                assert!(
                    deviation <= 40,
                    "clipped blur changed an exported pixel outside its bounds at {time}s, {x},{y}: {deviation}"
                );
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn edited_gif_exports_match_the_preview_and_timeline_at_each_frame_rate() {
        use crate::editor::annotations::Annotation;
        use crate::windows::video_editor::{model, preview, styles};
        use image::AnimationDecoder;

        let directory = tempfile::tempdir().unwrap();
        let recording = directory.path().join("recording.mp4");
        let mut encoder = Encoder::create(
            &recording,
            Settings {
                width: 160,
                height: 90,
                frame_rate: 12,
                bitrate: 1_000_000,
                has_audio: false,
            },
        )
        .unwrap();
        for index in 0..24 {
            let color = if index < 12 {
                [176, 96, 32, 255]
            } else {
                [48, 80, 192, 255]
            };
            encoder.write_frame(color.repeat(160 * 90)).unwrap();
        }
        encoder.finish().unwrap();

        let mut state = VideoEditorState {
            source_duration: Some(2.),
            segments: vec![
                model::Segment {
                    original_end: 0.75,
                    speed: Some(1.5),
                    ..model::Segment::spanning(2.)
                },
                model::Segment {
                    id: "after-cut".into(),
                    original_start: 1.25,
                    speed: Some(0.5),
                    ..model::Segment::spanning(2.)
                },
            ],
            ..VideoEditorState::default()
        };
        state.wallpaper.enabled = false;
        state.export_settings.format = "gif".into();
        state.export_settings.resolution = "480p".into();
        state.drawing_segments.push(model::DrawingSegment {
            id: "drawing".into(),
            start_time: 0.,
            end_time: 2.,
            canvas_width: 160.,
            canvas_height: 90.,
            annotations: vec![Annotation::Rectangle {
                id: "shape".into(),
                x: 20.,
                y: 20.,
                width: 40.,
                height: 25.,
                stroke: "#ffffff".into(),
                stroke_width: 3.,
                fill: Some("#e04040".into()),
            }],
        });
        model::save_state(&recording, &state);
        assert_eq!(model::load_state(&recording), state);

        let mut timing_errors = Vec::new();
        for rate in styles::GIF_FRAME_RATES {
            let fps: usize = rate.parse().unwrap();
            let mut state = model::load_state(&recording);
            state.export_settings.frame_rate = (*rate).into();
            let mut preview = preview::Source::open(&recording, state.clone()).unwrap();
            let output = directory.path().join(format!("edited-{fps}.gif"));
            let mut progress = 0.;
            run(
                Request {
                    project: recording.clone(),
                    output: output.clone(),
                    state,
                },
                &mut |value| progress = value,
                &|| false,
            )
            .unwrap();
            assert_eq!(progress, 1.);
            let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
                std::fs::File::open(&output).unwrap(),
            ))
            .unwrap();
            let frames = decoder.into_frames().collect_frames().unwrap();
            assert_eq!(frames.len(), fps * 2, "{fps} fps");
            let mut elapsed_ms = 0.;
            let mut max_error_ms = 0_f64;
            for (index, frame) in frames.iter().enumerate() {
                assert_eq!(frame.buffer().dimensions(), (160, 90));
                for (sample, source_color) in [
                    (fps / 4, [32_u8, 96, 176]),
                    (fps * 3 / 4, [192, 80, 48]),
                    (fps * 3 / 2, [192, 80, 48]),
                ] {
                    if index != sample {
                        continue;
                    }
                    let expected = preview.compose(index as f64 / fps as f64, None).unwrap();
                    let actual = frame.buffer().as_raw();
                    let error: u64 = actual
                        .iter()
                        .zip(expected.data())
                        .map(|(actual, expected)| u64::from(actual.abs_diff(*expected)))
                        .sum();
                    let mean = error as f64 / actual.len() as f64;
                    assert!(mean < 4., "{fps} fps, frame {index}: mean error {mean}");
                    for (x, y, color) in [(120, 60, source_color), (30, 30, [224, 64, 64])] {
                        let pixel = (y * 160 + x) * 4;
                        for (channel, expected_color) in color.into_iter().enumerate() {
                            assert!(expected.data()[pixel + channel].abs_diff(expected_color) < 8);
                            assert!(actual[pixel + channel].abs_diff(expected_color) < 8);
                        }
                    }
                }
                let (numerator, denominator) = frame.delay().numer_denom_ms();
                elapsed_ms += f64::from(numerator) / f64::from(denominator);
                let expected_ms = (index + 1) as f64 * 1000. / fps as f64;
                max_error_ms = max_error_ms.max((elapsed_ms - expected_ms).abs());
            }
            if max_error_ms > 5.01 {
                timing_errors.push(format!(
                    "{fps} fps: {elapsed_ms} ms, max error {max_error_ms} ms"
                ));
            }
        }
        assert!(timing_errors.is_empty(), "{}", timing_errors.join("; "));
    }

    #[cfg(windows)]
    #[test]
    fn cancelling_during_or_at_the_last_frame_removes_the_export() {
        use crate::windows::video_editor::model;
        use image::AnimationDecoder;
        use std::sync::atomic::{AtomicBool, Ordering};

        let directory = tempfile::tempdir().unwrap();
        let recording = directory.path().join("recording.mp4");
        let mut encoder = Encoder::create(
            &recording,
            Settings {
                width: 160,
                height: 90,
                frame_rate: 10,
                bitrate: 1_000_000,
                has_audio: false,
            },
        )
        .unwrap();
        for _ in 0..10 {
            encoder
                .write_frame([176, 96, 32, 255].repeat(160 * 90))
                .unwrap();
        }
        encoder.finish().unwrap();
        let mut state = VideoEditorState {
            source_duration: Some(1.),
            segments: vec![model::Segment::spanning(1.)],
            ..VideoEditorState::default()
        };
        state.wallpaper.enabled = false;
        state.export_settings.resolution = "original".into();
        state.export_settings.frame_rate = "10".into();

        let mut failures = Vec::new();
        for (format, cancel_progress) in [
            ("mp4", 0.1),
            ("mp4", 1.),
            ("gif", 0.1),
            ("gif", GIF_DIRECT_ENCODE_SHARE),
        ] {
            let output = directory
                .path()
                .join(format!("cancelled-{cancel_progress}.{format}"));
            let cancelled = AtomicBool::new(false);
            state.export_settings.format = format.into();
            let result = run(
                Request {
                    project: recording.clone(),
                    output: output.clone(),
                    state: state.clone(),
                },
                &mut |fraction| {
                    if fraction >= cancel_progress {
                        cancelled.store(true, Ordering::Relaxed);
                    }
                },
                &|| cancelled.load(Ordering::Relaxed),
            );
            if !matches!(&result, Err(error) if error.contains("cancelled")) || output.exists() {
                failures.push(format!(
                    "{format} at {cancel_progress}: result={result:?}, exists={}",
                    output.exists()
                ));
                continue;
            }
            run(
                Request {
                    project: recording.clone(),
                    output: output.clone(),
                    state: state.clone(),
                },
                &mut |_| {},
                &|| false,
            )
            .unwrap();
            if format == "mp4" {
                assert!(VideoDecoder::open(&output).unwrap().frame_at(0.).is_some());
            } else {
                let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
                    std::fs::File::open(&output).unwrap(),
                ))
                .unwrap();
                assert!(decoder.into_frames().next().unwrap().is_ok());
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("; "));
    }

    #[cfg(windows)]
    #[test]
    fn camera_ranges_and_zoom_export_the_preview_across_a_cut() {
        use crate::windows::video_editor::{model, preview};

        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("Camera workflow.poratake");
        std::fs::create_dir(&project).unwrap();
        std::fs::write(
            project::recording_video_path(&project),
            include_bytes!("../ui/parity/fixtures/capture.mov"),
        )
        .unwrap();
        let mut camera = Encoder::create(
            &project::camera_video_path(&project),
            Settings {
                width: 160,
                height: 120,
                frame_rate: 12,
                bitrate: 12_000_000,
                has_audio: false,
            },
        )
        .unwrap();
        for index in 0..24 {
            let red = if index < 18 { 224 } else { 64 };
            camera
                .write_frame([48, 192, red, 255].repeat(160 * 120))
                .unwrap();
        }
        camera.finish().unwrap();
        let source_camera = VideoDecoder::open(&project::camera_video_path(&project)).unwrap();
        assert!(source_camera.frame_at(1.).unwrap().bgra[2] > 180);
        assert!(source_camera.frame_at(1.5).unwrap().bgra[2] < 90);
        std::fs::write(
            project::camera_meta_path(&project),
            serde_json::to_vec(&serde_json::json!({
                "videoFile": "camera.mov",
                "meta": {
                    "duration": 2.,
                    "visibleRanges": [{"start": 0.25, "end": 0.65}, {"start": 1.45, "end": 1.8}]
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let mut state = VideoEditorState {
            source_duration: Some(2.),
            segments: vec![
                model::Segment {
                    original_end: 0.75,
                    ..model::Segment::spanning(2.)
                },
                model::Segment {
                    id: "after-cut".into(),
                    original_start: 1.25,
                    ..model::Segment::spanning(2.)
                },
            ],
            ..VideoEditorState::default()
        };
        let meta = sidecars::load_camera_meta(&project).unwrap();
        state.camera_segments = sidecars::map_visible_ranges_to_segments(
            meta.visible_ranges.as_deref(),
            &state.segments,
            1.5,
        );
        assert_eq!(state.camera_segments.len(), 2);
        for (segment, (start, end)) in state
            .camera_segments
            .iter()
            .zip([(0.25, 0.65), (0.95, 1.3)])
        {
            assert!((segment.start_time - start).abs() < 1e-9);
            assert!((segment.end_time - end).abs() < 1e-9);
        }
        state.export_settings.resolution = "original".into();
        state.export_settings.frame_rate = "12".into();
        state.wallpaper.enabled = false;
        state.camera_style.position = "top-left".into();
        state.camera_style.size = "large".into();
        state.camera_style.padding = 0.;
        state.camera_style.border_radius = 0.;
        state.camera_style.shadow = 0.;
        state.zoom_segments.push(model::ZoomSegment {
            id: "zoom".into(),
            start_time: 0.25,
            end_time: 1.25,
            zoom_level: 2.,
            transition_in_duration: Some(0.125),
            transition_out_duration: Some(0.125),
            target_mode: Some("manual".into()),
            focus_point: Some(model::FocusPoint { x: 0.75, y: 0.35 }),
        });
        model::save_state(&project, &state);
        assert_eq!(model::load_state(&project), state);
        let mut preview = preview::Source::open(&project, model::load_state(&project)).unwrap();
        let mut unzoomed = state.clone();
        unzoomed.zoom_segments.clear();
        let unzoomed = preview::Source::open(&project, unzoomed)
            .unwrap()
            .compose(0.75, None)
            .unwrap();
        let zoomed = preview.compose(0.75, None).unwrap();
        let changed = zoomed
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(unzoomed.data().as_chunks::<4>().0)
            .filter(|(zoomed, unzoomed)| zoomed != unzoomed)
            .count();
        assert!(changed > 320 * 180 / 4, "the checker must visibly zoom");
        let output = directory.path().join("camera-edited.mp4");
        run(
            Request {
                project,
                output: output.clone(),
                state,
            },
            &mut |_| {},
            &|| false,
        )
        .unwrap();
        let exported = VideoDecoder::open(&output).unwrap();
        let info = exported.info();
        assert_eq!((info.width, info.height), (320, 180));
        assert!((info.duration - 1.5).abs() <= 1. / 12., "{info:?}");
        for (time, visible) in [
            (0., false),
            (1. / 3., true),
            (0.5, true),
            (0.75, false),
            (1., true),
            (1.25, true),
            (17. / 12., false),
        ] {
            let expected = to_bgra(&preview.compose(time, None).unwrap());
            let actual = exported.frame_at(time).unwrap();
            assert_eq!(actual.bgra.len(), expected.len());
            let error: u64 = actual
                .bgra
                .iter()
                .zip(&expected)
                .map(|(actual, expected)| u64::from(actual.abs_diff(*expected)))
                .sum();
            let mean = error as f64 / expected.len() as f64;
            assert!(mean < 3., "preview/export channel error {mean} at {time}s");
            if visible {
                let (error, channels) = expected
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(actual.bgra.as_chunks::<4>().0)
                    .filter(|(pixel, _)| pixel[0] < 90 && pixel[1] > 170)
                    .fold((0_u64, 0), |(error, channels), (expected, actual)| {
                        let delta: u64 = expected[..3]
                            .iter()
                            .zip(&actual[..3])
                            .map(|(expected, actual)| u64::from(expected.abs_diff(*actual)))
                            .sum();
                        (error + delta, channels + 3)
                    });
                let mean = error as f64 / channels as f64;
                assert!(
                    mean < 3.,
                    "camera preview/export channel error {mean} at {time}s"
                );
            }
            for (name, pixels) in [
                ("preview", expected.as_slice()),
                ("export", actual.bgra.as_ref()),
            ] {
                let camera_pixels = pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|pixel| pixel[0] < 90 && pixel[1] > 170)
                    .count();
                if !visible {
                    assert_eq!(camera_pixels, 0, "{name}: camera must be hidden at {time}s");
                    continue;
                }
                assert!(camera_pixels > 1_000, "{name}: camera missing at {time}s");
                let pixel = (20 * info.width as usize + 20) * 4;
                let red = pixels[pixel + 2];
                assert!(
                    if time < 0.75 { red > 180 } else { red < 90 },
                    "{name}: wrong camera source frame at {time}s"
                );
            }
        }
    }

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

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn cancelling_during_gif_conversion_stops_the_process() {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicBool, Ordering};

        let directory = tempfile::tempdir().unwrap();
        let ffmpeg = directory.path().join("ffmpeg");
        std::fs::write(
            &ffmpeg,
            "#!/bin/sh\nprintf 'out_time_us=500000\\n'\nexec sleep 30\n",
        )
        .unwrap();
        std::fs::set_permissions(&ffmpeg, std::fs::Permissions::from_mode(0o700)).unwrap();
        let cancelled = AtomicBool::new(false);
        let started = std::time::Instant::now();
        let result = convert_to_gif(
            &ffmpeg,
            &directory.path().join("input.mp4"),
            &directory.path().join("output.gif"),
            160,
            30,
            1.,
            &mut |_| cancelled.store(true, Ordering::Relaxed),
            &|| cancelled.load(Ordering::Relaxed),
        );
        assert!(result.unwrap_err().contains("cancelled"));
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
    }

    #[test]
    fn premultiplied_pixels_come_back_as_opaque_bgra() {
        let mut pixmap = Pixmap::new(1, 1).expect("pixmap");
        pixmap.fill(tiny_skia::Color::from_rgba8(10, 20, 30, 255));
        assert_eq!(to_bgra(&pixmap), vec![30, 20, 10, 255]);
    }

    #[test]
    fn translucent_wallpaper_video_preview_matches_encoder_pixels() {
        let mut state = VideoEditorState::default();
        state.wallpaper.enabled = true;
        state.wallpaper.padding = 1.0;
        state.wallpaper.gradient = Some(serde_json::json!({
            "id": "translucent-test",
            "colors": ["#80402080", "#80402080"],
            "angle": 135.0,
        }));
        state.segments = vec![crate::windows::video_editor::model::Segment::spanning(1.0)];
        let mut engine = Engine::new(Config::new(1.0, 1.0, state));
        let mut video = Pixmap::new(1, 1).expect("video frame");
        video.fill(tiny_skia::Color::from_rgba8(10, 20, 30, 255));
        let frame = engine
            .render_frame(
                0.0,
                Frames {
                    video: Some(video.as_ref()),
                    camera: None,
                },
            )
            .expect("composed frame");
        let preview = crate::windows::video_editor::preview::to_render_image_opaque(&frame)
            .expect("preview image");
        assert_eq!(preview.size(0).width.0, 3);
        assert_eq!(preview.size(0).height.0, 3);

        let padding_pixel = &frame.data()[0..4];
        assert_eq!(padding_pixel, &[64, 32, 16, 128]);
        assert_eq!(&frame.data()[4 * 4..4 * 5], &[10, 20, 30, 255]);
        let preview_bytes = preview.as_bytes(0).expect("preview pixels");
        let exported = to_bgra(&frame);
        assert_eq!(&exported[0..4], &[32, 64, 128, 255]);
        assert_eq!(preview_bytes, exported);
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
