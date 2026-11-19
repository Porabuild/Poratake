//! The video editor's preview pipeline: decode the source frame for a
//! timeline position, composite it with the same engine the export uses, and
//! hand the result to GPUI as an image.

use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tiny_skia::Pixmap;

use crate::video::composition::segments::{self, VideoSegment};
use crate::video::composition::{Config, Engine, Frames};
use crate::video::decoder::{
    needs_stream_seek, DecodedFrame, FrameCancellation, VideoDecoder, VideoInfo,
};
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
    video_frame: Option<CachedFrame>,
    camera_frame: Option<CachedFrame>,
    camera_warmup: Option<CameraWarmup>,
}

struct CameraWarmup {
    time: f64,
    completed: Receiver<()>,
    cancel: smol::channel::Sender<()>,
}

struct CachedFrame {
    time: f64,
    decoded: Weak<Vec<u8>>,
    pixmap: Arc<Pixmap>,
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
            camera_warmup: None,
        })
    }

    pub fn composition_size(&self) -> (f64, f64) {
        let (width, height) = self.engine.dimensions();
        (width as f64, height as f64)
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
    #[cfg(any(test, feature = "parity"))]
    pub fn compose(
        &mut self,
        timeline_time: f64,
        max_dimensions: Option<(u32, u32)>,
    ) -> Option<Pixmap> {
        let mut decode_wait = Duration::ZERO;
        self.compose_measured(timeline_time, max_dimensions, &mut decode_wait, None)
    }

    pub(super) fn compose_measured(
        &mut self,
        timeline_time: f64,
        max_dimensions: Option<(u32, u32)>,
        decode_wait: &mut Duration,
        cancelled: Option<&FrameCancellation>,
    ) -> Option<Pixmap> {
        if cancelled.is_some_and(FrameCancellation::is_closed) {
            return None;
        }
        let first_frame_duration = self.engine.first_frame_duration();
        if first_frame_duration > 0. && timeline_time < first_frame_duration {
            return self.render(
                timeline_time,
                max_dimensions,
                Frames { video: None, camera: None },
            );
        }
        let adjusted = (timeline_time - first_frame_duration).max(0.0);
        let video_time = segments::map_timeline_to_video_time(adjusted, &self.video_segments)
            .or_else(|| self.video_segments.last().map(|segment| segment.end_time))
            .unwrap_or(adjusted);

        if let Some(warmup) = self
            .camera_warmup
            .take_if(|warmup| needs_stream_seek(warmup.time, video_time))
        {
            warmup.cancel.close();
        }

        let video = cached_frame(&mut self.video_frame, video_time, || {
            decode_frame(self.video.as_ref(), video_time, decode_wait, cancelled)
        })?;
        let camera = if self.engine.camera_visible_at(timeline_time) {
            let had_camera_frame = self.camera_frame.is_some();
            let camera = cached_frame(&mut self.camera_frame, video_time, || {
                decode_frame(self.camera.as_ref(), video_time, decode_wait, cancelled)
            });
            if had_camera_frame && camera.is_none() {
                return None;
            }
            camera
        } else {
            let state = &self.engine.config().state;
            if state.camera_style.visible
                && state
                    .camera_segments
                    .iter()
                    .any(|segment| segment.end_time > adjusted)
            {
                self.warm_camera(video_time);
            }
            None
        };

        self.render(
            timeline_time,
            max_dimensions,
            Frames {
                video: Some(video.as_ref().as_ref()),
                camera: camera.as_ref().map(|pixmap| pixmap.as_ref().as_ref()),
            },
        )
    }

    fn warm_camera(&mut self, time: f64) {
        if self.camera_warmup.as_ref().is_some_and(|pending| {
            matches!(pending.completed.try_recv(), Err(TryRecvError::Empty))
        }) {
            return;
        }
        self.camera_warmup = self.camera.as_ref().and_then(|decoder| {
            let (cancel, cancelled) = smol::channel::bounded(1);
            let completed = decoder.request_warmup(time, cancelled)?;
            Some(CameraWarmup {
                time,
                completed,
                cancel,
            })
        });
    }

    fn render(
        &mut self,
        timeline_time: f64,
        max_dimensions: Option<(u32, u32)>,
        frames: Frames<'_>,
    ) -> Option<Pixmap> {
        match max_dimensions {
            Some((width, height)) => {
                self.engine
                    .render_frame_scaled(timeline_time, frames, width, height)
            }
            None => self.engine.render_frame(timeline_time, frames),
        }
    }
}

fn decode_frame(
    decoder: Option<&VideoDecoder>,
    time: f64,
    decode_wait: &mut Duration,
    cancelled: Option<&FrameCancellation>,
) -> Option<DecodedFrame> {
    let decoder = decoder?;
    let started = Instant::now();
    let frame = decoder.frame_at_interruptible(time, cancelled);
    *decode_wait += started.elapsed();
    frame
}

fn cached_frame(
    cached: &mut Option<CachedFrame>,
    time: f64,
    decode: impl FnOnce() -> Option<DecodedFrame>,
) -> Option<Arc<Pixmap>> {
    if let Some(frame) = cached
        .as_ref()
        .filter(|frame| (frame.time - time).abs() < f64::EPSILON)
    {
        return Some(frame.pixmap.clone());
    }
    #[cfg(all(feature = "parity", target_os = "macos"))]
    let decode_started = std::env::var_os("PORATAKE_NATIVE_SMOKE_VIDEO_PLAYBACK")
        .map(|_| std::time::Instant::now());
    let decoded = decode();
    #[cfg(all(feature = "parity", target_os = "macos"))]
    if let Some(started) = decode_started {
        println!(
            "VIDEO_SMOKE decode {time:.6} {:.3} {}",
            started.elapsed().as_secs_f64() * 1000.,
            decoded.is_some(),
        );
    }

    let frame = decoded?;
    let decoded = Arc::downgrade(&frame.bgra);
    if let Some(cached) = cached.as_mut().filter(|cached| {
        cached.decoded.ptr_eq(&decoded)
            && cached.pixmap.width() == frame.width.max(1)
            && cached.pixmap.height() == frame.height.max(1)
    }) {
        cached.time = time;
        return Some(cached.pixmap.clone());
    }
    let pixmap = Arc::new(to_pixmap(&frame)?);
    *cached = Some(CachedFrame {
        time,
        decoded,
        pixmap: pixmap.clone(),
    });
    Some(pixmap)
}

/// A decoded BGRA frame as a tiny-skia RGBA pixmap. Decoded frames are opaque,
/// so premultiplication is a no-op and only the channel order changes.
fn to_pixmap(frame: &DecodedFrame) -> Option<Pixmap> {
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

/// Converts a composed pixmap into the straight-alpha BGRA image GPUI paints.
pub fn to_render_image(pixmap: &Pixmap) -> Option<Arc<gpui::RenderImage>> {
    to_render_image_with_alpha(pixmap, true)
}

pub fn to_render_image_opaque(pixmap: &Pixmap) -> Option<Arc<gpui::RenderImage>> {
    to_render_image_with_alpha(pixmap, false)
}

fn to_render_image_with_alpha(
    pixmap: &Pixmap,
    preserve_alpha: bool,
) -> Option<Arc<gpui::RenderImage>> {
    if !preserve_alpha {
        let buffer = image::RgbaImage::from_raw(
            pixmap.width(),
            pixmap.height(),
            crate::video::composition::opaque::bgra(pixmap),
        )?;
        return Some(Arc::new(gpui::RenderImage::new(smallvec::smallvec![
            image::Frame::new(buffer)
        ])));
    }
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
        pixmap.fill(tiny_skia::Color::from_rgba8(200, 100, 50, 128));
        let image = to_render_image(&pixmap).expect("image");
        assert_eq!(image.size(0).width.0, 1);
        let pixels = image.as_bytes(0).expect("pixels");
        assert_eq!(pixels[3], 128);

        let opaque = to_render_image_opaque(&pixmap).expect("opaque image");
        let opaque_pixels = opaque.as_bytes(0).expect("opaque pixels");
        assert_eq!(opaque_pixels, &[25, 50, 100, 255]);
    }

    #[test]
    fn opening_a_missing_project_yields_no_source() {
        let missing = crate::util::test_paths::unique_temp("poratake-missing.poratake");
        assert!(Source::open(&missing, VideoEditorState::default()).is_none());
    }

    #[test]
    fn a_missing_screen_frame_does_not_produce_a_background_only_preview() {
        let mut state = VideoEditorState::default();
        state.first_frame.enabled = true;
        state.first_frame.image_data = Some("data:,".into());

        let mut intro = Pixmap::new(2, 2).unwrap();
        intro.fill(tiny_skia::Color::from_rgba8(10, 20, 30, 255));
        let mut config = Config::new(2., 2., state);
        config.first_frame_image = Some(intro);

        let mut source = Source {
            engine: Engine::new(config),
            video: None,
            camera: None,
            video_segments: Vec::new(),
            video_frame: None,
            camera_frame: None,
            camera_warmup: None,
        };
        let video_time = 0.5;
        let decoded = DecodedFrame {
            width: 2,
            height: 2,
            bgra: [220, 140, 50, 255].repeat(4).into(),
        };
        let before = cached_frame(&mut source.video_frame, video_time, || Some(decoded.clone())).unwrap();
        let intro_duration = source.engine.first_frame_duration();
        assert!(intro_duration > 0.);
        let rendered_intro = source.compose(0., None).expect("intro preview");
        assert!(rendered_intro.data().chunks_exact(4).any(|pixel| pixel == [10, 20, 30, 255]));
        assert!(Arc::ptr_eq(&before, &source.video_frame.as_ref().unwrap().pixmap));

        assert!(source.compose(intro_duration, None).is_none());
        assert!(source.compose(intro_duration + 0.25, Some((1, 1))).is_none());

        cached_frame(&mut source.video_frame, video_time, || Some(decoded)).unwrap();
        let recovered = source.compose(intro_duration + video_time, None).expect("screen preview");
        assert!(recovered.data().chunks_exact(4).any(|pixel| pixel == [50, 140, 220, 255]));
    }

    #[test]
    fn a_missing_visible_camera_frame_holds_the_last_complete_preview() {
        use crate::windows::video_editor::model::CameraSegment;

        let mut state = VideoEditorState {
            camera_segments: vec![CameraSegment {
                id: "camera".into(),
                start_time: 0.,
                end_time: 1.,
            }],
            ..Default::default()
        };
        state.camera_style.visible = true;
        let mut source = Source {
            engine: Engine::new(Config::new(320., 180., state)),
            video: None,
            camera: None,
            video_segments: Vec::new(),
            video_frame: None,
            camera_frame: None,
            camera_warmup: None,
        };
        let screen = DecodedFrame {
            width: 320,
            height: 180,
            bgra: [220, 140, 50, 255].repeat(320 * 180).into(),
        };
        let camera = DecodedFrame {
            width: 32,
            height: 32,
            bgra: [12, 34, 240, 255].repeat(32 * 32).into(),
        };
        let has_camera_pixels = |frame: &Pixmap| {
            frame.data().chunks_exact(4).any(|pixel| pixel == [240, 34, 12, 255])
        };

        cached_frame(&mut source.video_frame, 0., || Some(screen.clone())).unwrap();
        assert!(!has_camera_pixels(&source.compose(0., None).unwrap()));
        cached_frame(&mut source.camera_frame, 0., || Some(camera.clone())).unwrap();
        assert!(has_camera_pixels(&source.compose(0., None).unwrap()));

        for time in [0.25, 0.5] {
            cached_frame(&mut source.video_frame, time, || Some(screen.clone())).unwrap();
            assert!(source.compose(time, Some((160, 90))).is_none());
        }

        cached_frame(&mut source.video_frame, 0.75, || Some(screen.clone())).unwrap();
        cached_frame(&mut source.camera_frame, 0.75, || Some(camera)).unwrap();
        assert!(has_camera_pixels(&source.compose(0.75, None).unwrap()));

        cached_frame(&mut source.video_frame, 1.25, || Some(screen)).unwrap();
        assert!(!has_camera_pixels(&source.compose(1.25, None).unwrap()));
    }

    #[test]
    fn unchanged_times_reuse_the_composed_source_frame() {
        let decoded = DecodedFrame {
            width: 1,
            height: 1,
            bgra: vec![10, 20, 30, 255].into(),
        };
        let mut cached = None;
        let frame = cached_frame(&mut cached, 2.0, || Some(decoded)).unwrap();
        let reused = cached_frame(&mut cached, 2.0, || panic!("already cached")).unwrap();
        assert!(Arc::ptr_eq(&frame, &reused));
    }

    #[test]
    fn nearby_times_reuse_the_same_decoded_frame_without_retaining_its_buffer() {
        let decoded = DecodedFrame {
            width: 1,
            height: 1,
            bgra: vec![10, 20, 30, 255].into(),
        };
        let mut cached = None;
        let first = cached_frame(&mut cached, 0.0, || Some(decoded.clone())).unwrap();
        let next = cached_frame(&mut cached, 0.01, || Some(decoded.clone())).unwrap();
        assert!(Arc::ptr_eq(&first, &next));
        assert_eq!(next.data(), &[30, 20, 10, 255]);
        assert_eq!(Arc::strong_count(&decoded.bgra), 1);

        let different = DecodedFrame {
            bgra: vec![40, 50, 60, 255].into(),
            ..decoded
        };
        let changed = cached_frame(&mut cached, 0.02, || Some(different)).unwrap();
        assert!(!Arc::ptr_eq(&first, &changed));
        assert_eq!(changed.data(), &[60, 50, 40, 255]);
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[cfg_attr(target_os = "macos", ignore = "requires development FFmpeg")]
    #[test]
    fn preview_only_converts_visible_camera_frames() {
        use crate::windows::video_editor::model::{CameraSegment, Segment};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("camera-preview.poratake");
        std::fs::create_dir(&path).unwrap();
        let video = include_bytes!("../../ui/parity/fixtures/capture.mov");
        std::fs::write(project::recording_video_path(&path), video).unwrap();
        std::fs::write(project::camera_video_path(&path), video).unwrap();

        let mut state = VideoEditorState {
            segments: vec![Segment::spanning(2.)],
            camera_segments: vec![
                CameraSegment {
                    id: "first".into(),
                    start_time: 0.,
                    end_time: 0.5,
                },
                CameraSegment {
                    id: "second".into(),
                    start_time: 1.25,
                    end_time: 2.,
                },
            ],
            ..Default::default()
        };
        state.first_frame.enabled = true;
        state.first_frame.image_data = Some("data:,".into());
        state.camera_style.visible = true;

        let mut source = Source::open(&path, state.clone()).unwrap();
        let still = source.engine.first_frame_duration();
        assert!(still > 0.);
        let dimensions = Some((160, 90));
        let mut decode_wait = Duration::ZERO;
        assert!(source
            .compose_measured(0., dimensions, &mut decode_wait, None)
            .is_some());
        assert_eq!(decode_wait, Duration::ZERO);
        assert!(source.video_frame.is_none());
        assert!(source.camera_frame.is_none());
        assert!(source.compose(still + 0.75, dimensions).is_some());
        assert!(source.camera_frame.is_none());
        assert!(source.camera_warmup.is_some());
        let mut decode_wait = Duration::ZERO;
        assert!(source
            .compose_measured(still + 0.75, dimensions, &mut decode_wait, None)
            .is_some());
        assert_eq!(decode_wait, Duration::ZERO);
        assert!(source.compose(still + 0.25, dimensions).is_some());
        let first = source.camera_frame.as_ref().unwrap().pixmap.clone();

        let mut disabled = state.clone();
        disabled.camera_style.visible = false;
        source.set_state(disabled);
        assert!(source.compose(still + 1.5, dimensions).is_some());
        assert!(Arc::ptr_eq(
            &first,
            &source.camera_frame.as_ref().unwrap().pixmap
        ));

        source.set_state(state);
        assert!(source.compose(still + 1.5, dimensions).is_some());
        let resumed = source.camera_frame.as_ref().unwrap();
        assert!((resumed.time - 1.5).abs() < 0.001);
        assert!(!Arc::ptr_eq(&first, &resumed.pixmap));
    }
}
