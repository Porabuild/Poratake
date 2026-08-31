//! The video editor's preview pipeline: decode the source frame for a
//! timeline position, composite it with the same engine the export uses, and
//! hand the result to GPUI as an image.

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use tiny_skia::Pixmap;

use crate::video::composition::segments::{self, VideoSegment};
use crate::video::composition::{Config, Engine, Frames};
use crate::video::decoder::{DecodedFrame, VideoDecoder, VideoInfo};
use crate::video::{project, sidecars};
use crate::windows::video_editor::model::VideoEditorState;

/// The decoders and the composition engine for one open project. Everything
/// here is touched only from the background executor, so it lives behind one
/// mutex rather than being split across several.
pub struct Source {
    engine: Engine,
    video: Option<VideoDecoder>,
    camera: Option<VideoDecoder>,
    video_segments: Vec<VideoSegment>,
    video_frame: Option<(f64, Arc<Pixmap>)>,
    camera_frame: Option<(f64, Arc<Pixmap>)>,
}

impl Source {
    /// Opens a project's recording and sidecars. Blocking, so callers run it on
    /// the background executor.
    pub fn open(path: &Path, state: VideoEditorState) -> Option<Self> {
        let video = VideoDecoder::open(&project::recording_video_path(path));
        let info = video.as_ref().map(VideoDecoder::info).unwrap_or_default();
        if !info.is_valid() {
            return None;
        }

        let camera_path = project::camera_video_path(path);
        let camera = camera_path
            .is_file()
            .then(|| VideoDecoder::open(&camera_path))
            .flatten();

        let preview_fps = crate::video::export::frame_rate(&state).max(1) as f64;
        let mut config = Config::new(info.width as f64, info.height as f64, state);
        config.fps = preview_fps;
        config.cursor_data = sidecars::load_cursor(path);
        config.keyboard_data = sidecars::load_keyboard(path);
        config.subtitle_data = sidecars::load_subtitle(path);
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

        let video_segments = segments::to_video_segments(&config.state.segments);
        Some(Self {
            engine: Engine::new(config),
            video,
            camera,
            video_segments,
            video_frame: None,
            camera_frame: None,
        })
    }

    pub fn info(&self) -> VideoInfo {
        self.video
            .as_ref()
            .map(VideoDecoder::info)
            .unwrap_or_default()
    }

    /// Applies edited state to the engine so the next composed frame reflects
    /// it. Backgrounds are reloaded only when their source changed.
    pub fn set_state(&mut self, state: VideoEditorState) {
        let previous_background = self
            .engine
            .config()
            .state
            .wallpaper
            .background_image
            .clone();
        let previous_first_frame = self.engine.config().state.first_frame.image_data.clone();
        let next_background = state.wallpaper.background_image.clone();
        let next_first_frame = state.first_frame.image_data.clone();
        let frame_rate = crate::video::export::frame_rate(&state).max(1) as f64;

        self.video_segments = segments::to_video_segments(&state.segments);
        self.engine.set_state(state);
        self.engine.set_frame_rate(frame_rate);

        if previous_background != next_background {
            self.engine.set_background_image(
                next_background
                    .as_deref()
                    .and_then(crate::render::gradient::load_image),
            );
        }
        if previous_first_frame != next_first_frame {
            self.engine.set_first_frame_image(
                next_first_frame
                    .as_deref()
                    .and_then(crate::render::gradient::load_image),
            );
        }
    }

    /// Composes the frame at `timeline_time`.
    pub fn compose(
        &mut self,
        timeline_time: f64,
        max_dimensions: Option<(u32, u32)>,
    ) -> Option<Pixmap> {
        let first_frame_duration = self.engine.first_frame_duration();
        let adjusted = (timeline_time - first_frame_duration).max(0.0);
        let video_time = segments::map_timeline_to_video_time(adjusted, &self.video_segments)
            .or_else(|| self.video_segments.last().map(|segment| segment.end_time))
            .unwrap_or(adjusted);

        let video = cached_frame(self.video.as_ref(), &mut self.video_frame, video_time);
        let camera = cached_frame(self.camera.as_ref(), &mut self.camera_frame, video_time);

        let frames = Frames {
            video: video.as_ref().map(|pixmap| pixmap.as_ref().as_ref()),
            camera: camera.as_ref().map(|pixmap| pixmap.as_ref().as_ref()),
        };
        match max_dimensions {
            Some((width, height)) => {
                self.engine
                    .render_frame_scaled(timeline_time, frames, width, height)
            }
            None => self.engine.render_frame(timeline_time, frames),
        }
    }
}

fn cached_frame(
    decoder: Option<&VideoDecoder>,
    cached: &mut Option<(f64, Arc<Pixmap>)>,
    time: f64,
) -> Option<Arc<Pixmap>> {
    if let Some(frame) = cached
        .as_ref()
        .filter(|(cached_time, _)| (*cached_time - time).abs() < f64::EPSILON)
        .map(|(_, frame)| frame.clone())
    {
        return Some(frame);
    }
    let frame = decoder
        .and_then(|decoder| decoder.frame_at(time))
        .and_then(|frame| to_pixmap(&frame))
        .map(Arc::new);
    *cached = frame.clone().map(|frame| (time, frame));
    frame
}

/// A decoded BGRA frame as a tiny-skia RGBA pixmap. Decoded frames are opaque,
/// so premultiplication is a no-op and only the channel order changes.
fn to_pixmap(frame: &DecodedFrame) -> Option<Pixmap> {
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

/// Converts a composed pixmap into the straight-alpha BGRA image GPUI paints.
pub fn to_render_image(pixmap: &Pixmap) -> Option<Arc<gpui::RenderImage>> {
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
        // GPUI composites in BGRA.
        buffer.put_pixel(
            x,
            y,
            image::Rgba([
                unpremultiply(pixel[2]),
                unpremultiply(pixel[1]),
                unpremultiply(pixel[0]),
                alpha,
            ]),
        );
    }
    Some(Arc::new(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(buffer)
    ])))
}

/// A handle the window keeps; the engine and decoders live behind it.
pub type Handle = Arc<Mutex<Source>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decoded_frame_becomes_an_rgba_pixmap() {
        let frame = DecodedFrame {
            width: 1,
            height: 1,
            bgra: vec![10, 20, 30, 255].into(),
        };
        let pixmap = to_pixmap(&frame).expect("pixmap");
        assert_eq!(pixmap.data(), &[30, 20, 10, 255]);
    }

    #[test]
    fn a_mismatched_buffer_is_rejected() {
        let frame = DecodedFrame {
            width: 4,
            height: 4,
            bgra: vec![0; 8].into(),
        };
        assert!(to_pixmap(&frame).is_none());
    }

    #[test]
    fn a_composed_pixmap_converts_to_a_bgra_image() {
        let mut pixmap = Pixmap::new(1, 1).expect("pixmap");
        pixmap.fill(tiny_skia::Color::from_rgba8(200, 100, 50, 255));
        let image = to_render_image(&pixmap).expect("image");
        assert_eq!(image.size(0).width.0, 1);
    }

    #[test]
    fn opening_a_missing_project_yields_no_source() {
        let missing = std::env::temp_dir().join("poratake-missing.poratake");
        assert!(Source::open(&missing, VideoEditorState::default()).is_none());
    }

    #[test]
    fn unchanged_times_reuse_the_composed_source_frame() {
        let frame = Arc::new(Pixmap::new(1, 1).expect("pixmap"));
        let mut cached = Some((2.0, frame.clone()));
        let reused = cached_frame(None, &mut cached, 2.0).expect("cached frame");
        assert!(Arc::ptr_eq(&frame, &reused));
    }
}
