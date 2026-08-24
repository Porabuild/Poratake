//! Video export. Every frame goes through the same composition engine the
//! preview uses, so the file matches what the editor showed; audio is decoded,
//! trimmed to the timeline and mixed here rather than by an external tool.

use std::path::{Path, PathBuf};

use tiny_skia::Pixmap;

use crate::render::canvas::Canvas;
use crate::video::composition::segments::{self, VideoSegment};
use crate::video::composition::{Config, Engine, Frames};
use crate::video::decoder::VideoDecoder;
use crate::video::encoder::{Encoder, Settings, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};
use crate::video::{audio, project, sidecars};
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

    if width % 2 != 0 {
        width += 1;
    }
    if height % 2 != 0 {
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
        if width % 2 != 0 {
            width -= 1;
        }
        if height % 2 != 0 {
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
    let name = crate::windows::video_editor::model::project_display_name(project_or_video);
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
    let dimensions = if is_gif {
        gif_dimensions(
            composition_width,
            composition_height,
            &request.state.export_settings.resolution,
        )
    } else {
        export_dimensions(
            composition_width,
            composition_height,
            &request.state.export_settings.resolution,
        )
    };

    let total_duration = engine.total_duration();
    if total_duration <= 0.0 {
        return Err("the timeline is empty".to_string());
    }
    let total_frames = ((total_duration * fps as f64).ceil() as u64).max(1);

    if is_gif {
        return run_gif(
            &request,
            &mut engine,
            &decoder,
            camera.as_ref(),
            &video_segments,
            dimensions,
            fps,
            total_frames,
            progress,
            should_cancel,
        );
    }

    let result = (|| -> Result<(), String> {
        let audio_track = build_audio(&request.project, &request.state, total_duration);
        let mut encoder = Encoder::create(
            &request.output,
            Settings {
                width: dimensions.width,
                height: dimensions.height,
                frame_rate: fps,
                bitrate: bitrate(
                    dimensions.width,
                    dimensions.height,
                    fps,
                    &request.state.export_settings.quality_preset,
                    camera.is_some(),
                ),
                has_audio: audio_track.is_some(),
            },
        )?;

        if let Some(samples) = audio_track {
            encoder.write_audio(audio::to_bytes(&samples), 0)?;
        }

        let first_frame_duration = engine.first_frame_duration();
        for index in 0..total_frames {
            if should_cancel() {
                return Err("the export was cancelled".to_string());
            }

            let timeline_time = index as f64 / fps as f64;
            let frame = compose_pixmap(
                &mut engine,
                &decoder,
                camera.as_ref(),
                &video_segments,
                first_frame_duration,
                timeline_time,
                dimensions,
            )?;
            encoder.write_frame(to_bgra(&frame))?;
            progress((index + 1) as f32 / total_frames as f32);
        }

        encoder.finish()
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&request.output);
        return Err(error);
    }
    Ok(request.output)
}

/// Writes an animated GIF straight from the composed frames. The Electron
/// shell renders an MP4 and converts it with FFmpeg; going direct keeps the
/// frames the preview showed and needs no external binary.
#[allow(clippy::too_many_arguments)]
fn run_gif(
    request: &Request,
    engine: &mut Engine,
    decoder: &VideoDecoder,
    camera: Option<&VideoDecoder>,
    video_segments: &[VideoSegment],
    dimensions: Dimensions,
    fps: u32,
    total_frames: u64,
    progress: &mut dyn FnMut(f32),
    should_cancel: &dyn Fn() -> bool,
) -> Result<PathBuf, String> {
    use image::codecs::gif::{GifEncoder, Repeat};

    let file = std::fs::File::create(&request.output)
        .map_err(|error| format!("could not create the file: {error}"))?;
    let mut encoder = GifEncoder::new_with_speed(std::io::BufWriter::new(file), 10);
    encoder
        .set_repeat(Repeat::Infinite)
        .map_err(|error| format!("could not start the GIF: {error}"))?;

    let delay = image::Delay::from_numer_denom_ms(1000, fps.max(1));
    let first_frame_duration = engine.first_frame_duration();

    for index in 0..total_frames {
        if should_cancel() {
            let _ = std::fs::remove_file(&request.output);
            return Err("the export was cancelled".to_string());
        }

        let timeline_time = index as f64 / fps as f64;
        let composed = compose_pixmap(
            engine,
            decoder,
            camera,
            video_segments,
            first_frame_duration,
            timeline_time,
            dimensions,
        )?;
        let frame = image::Frame::from_parts(to_rgba_buffer(&composed), 0, 0, delay);
        encoder
            .encode_frame(frame)
            .map_err(|error| format!("could not write a frame: {error}"))?;
        progress((index + 1) as f32 / total_frames as f32);
    }

    drop(encoder);
    Ok(request.output.clone())
}

/// Straight-alpha RGBA, which the GIF encoder quantizes from.
fn to_rgba_buffer(pixmap: &Pixmap) -> image::RgbaImage {
    let mut buffer = image::RgbaImage::new(pixmap.width(), pixmap.height());
    for (index, pixel) in pixmap.data().chunks_exact(4).enumerate() {
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

/// Composes one frame at export size.
#[allow(clippy::too_many_arguments)]
fn compose_pixmap(
    engine: &mut Engine,
    decoder: &VideoDecoder,
    camera: Option<&VideoDecoder>,
    video_segments: &[VideoSegment],
    first_frame_duration: f64,
    timeline_time: f64,
    dimensions: Dimensions,
) -> Result<Pixmap, String> {
    let adjusted = (timeline_time - first_frame_duration).max(0.0);
    let video_time = segments::map_timeline_to_video_time(adjusted, video_segments)
        .or_else(|| video_segments.last().map(|segment| segment.end_time))
        .unwrap_or(adjusted);

    let video = decoder
        .frame_at(video_time)
        .and_then(|frame| to_pixmap(&frame));
    let camera_frame = camera
        .and_then(|decoder| decoder.frame_at(video_time))
        .and_then(|frame| to_pixmap(&frame));

    let composed = engine
        .render_frame(
            timeline_time,
            Frames {
                video: video.as_ref().map(Pixmap::as_ref),
                camera: camera_frame.as_ref().map(Pixmap::as_ref),
            },
        )
        .ok_or_else(|| "the frame could not be composed".to_string())?;

    scale_to(&composed, dimensions.width, dimensions.height)
        .ok_or_else(|| "the frame could not be scaled".to_string())
}

fn to_pixmap(frame: &crate::video::decoder::DecodedFrame) -> Option<Pixmap> {
    let mut pixmap = Pixmap::new(frame.width.max(1), frame.height.max(1))?;
    let target = pixmap.data_mut();
    if target.len() != frame.bgra.len() {
        return None;
    }
    for (out, pixel) in target.chunks_exact_mut(4).zip(frame.bgra.chunks_exact(4)) {
        out[0] = pixel[2];
        out[1] = pixel[1];
        out[2] = pixel[0];
        out[3] = pixel[3];
    }
    Some(pixmap)
}

/// Resamples a composed frame to the export size, on an opaque backdrop so a
/// transparent wallpaper does not encode as black fringing.
fn scale_to(source: &Pixmap, width: u32, height: u32) -> Option<Pixmap> {
    if source.width() == width && source.height() == height {
        return Some(source.clone());
    }
    let mut canvas = Canvas::new(width, height)?;
    canvas.fill_all(tiny_skia::Color::from_rgba8(0, 0, 0, 255));
    canvas.draw_pixmap(source.as_ref(), 0.0, 0.0, width as f32, height as f32);
    Some(canvas.into_pixmap())
}

/// Unpremultiplies into the BGRA layout Media Foundation's RGB32 input expects.
fn to_bgra(pixmap: &Pixmap) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(pixmap.data().len());
    for pixel in pixmap.data().chunks_exact(4) {
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

/// Decodes, trims and mixes every enabled audio track, or `None` when the
/// project has no audible source.
fn build_audio(
    project_path: &Path,
    state: &VideoEditorState,
    total_duration: f64,
) -> Option<audio::Pcm> {
    let mut tracks: Vec<audio::Track> = Vec::new();

    let mut add = |path: PathBuf, volume: f64, enabled: bool| {
        if !enabled || volume <= 0.0 {
            return;
        }
        if let Some(samples) = audio::decode(&path) {
            tracks.push(audio::Track {
                samples: audio::apply_segments(&samples, &state.segments),
                volume,
            });
        }
    };

    add(
        project::system_audio_path(project_path),
        state.audio_style.system_audio_volume,
        state.audio_style.system_audio_enabled,
    );
    add(
        project::mic_audio_path(project_path),
        state.audio_style.mic_audio_volume,
        state.audio_style.mic_audio_enabled,
    );

    if let Some(folder) = project::music_folder(project_path) {
        for track in &state.music_tracks {
            if !track.enabled || track.file_name.is_empty() {
                continue;
            }
            let Some(samples) = audio::decode(&folder.join(&track.file_name)) else {
                continue;
            };
            tracks.push(audio::Track {
                samples: audio::place_music_track(&samples, track),
                volume: track.volume,
            });
        }
    }

    if tracks.is_empty() {
        return None;
    }
    let frames = (total_duration * AUDIO_SAMPLE_RATE as f64).round() as usize;
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
        assert_eq!(frame_rate(&state), 60);
        state.export_settings.frame_rate = "24".into();
        assert_eq!(frame_rate(&state), 24);
        state.export_settings.frame_rate = "nonsense".into();
        assert_eq!(frame_rate(&state), 60);
    }

    #[test]
    fn the_default_output_sits_beside_the_project() {
        let state = VideoEditorState::default();
        let path = default_output_path(Path::new("/tmp/Take 1.poratake"), &state);
        assert_eq!(path, PathBuf::from("/tmp/Take 1.mp4"));
    }

    #[test]
    fn premultiplied_pixels_come_back_as_opaque_bgra() {
        let mut pixmap = Pixmap::new(1, 1).expect("pixmap");
        pixmap.fill(tiny_skia::Color::from_rgba8(10, 20, 30, 255));
        assert_eq!(to_bgra(&pixmap), vec![30, 20, 10, 255]);
    }

    #[test]
    fn scaling_returns_the_requested_size() {
        let mut source = Pixmap::new(4, 4).expect("pixmap");
        source.fill(tiny_skia::Color::from_rgba8(255, 0, 0, 255));
        let scaled = scale_to(&source, 8, 8).expect("scaled");
        assert_eq!((scaled.width(), scaled.height()), (8, 8));
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
        let output = std::env::temp_dir().join("poratake-export-test.gif");
        let _ = std::fs::remove_file(&output);
        let mut state = VideoEditorState::default();
        state.export_settings.format = "gif".into();
        let error = run(
            Request {
                project: std::env::temp_dir().join("poratake-missing.poratake"),
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
