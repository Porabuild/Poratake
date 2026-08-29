//! Port of `components/video-editor/composition/video-composition-engine.ts`.
//! One engine composites a frame for the preview and for the export, so what
//! the editor shows is what the file contains.

pub mod camera;
pub mod cursor;
pub mod cursor_sprites;
pub mod device_frame;
pub mod drawing;
pub mod keyboard;
pub mod segments;
pub mod subtitle;
pub mod wallpaper;
pub mod zoom;

use tiny_skia::{Color, FillRule, Pixmap, PixmapRef};

use crate::render::canvas::{rounded_rect_path, Canvas};
use crate::video::sidecars::{CursorData, KeyboardData, SubtitleData};
use crate::windows::video_editor::model::VideoEditorState;
use crate::windows::video_editor::styles::CursorStyle;

use self::segments::VideoSegment;
use self::zoom::{Transform, ZoomCache};

fn composition_dimensions(config: &Config) -> (u32, u32) {
    let wallpaper = &config.state.wallpaper;
    let enabled = wallpaper.enabled;
    let padding = if enabled { wallpaper.padding } else { 0.0 };
    let ratio = if enabled {
        wallpaper::aspect_ratio(wallpaper)
    } else {
        None
    };

    let (mut width, mut height) = (config.video_width, config.video_height);
    if enabled && wallpaper.device_frame {
        let layout = device_frame::calculate_layout(width, height);
        width = layout.frame_width;
        height = layout.frame_height;
    }

    let dimensions = wallpaper::calculate_dimensions(width, height, padding, ratio);
    (
        dimensions.width.round().max(1.0) as u32,
        dimensions.height.round().max(1.0) as u32,
    )
}

/// Everything the engine needs beyond the editor state: the source size and the
/// sidecars a project carries.
pub struct Config {
    pub video_width: f64,
    pub video_height: f64,
    pub state: VideoEditorState,
    pub cursor_data: Option<CursorData>,
    pub keyboard_data: Option<KeyboardData>,
    pub subtitle_data: Option<SubtitleData>,
    pub background_image: Option<Pixmap>,
    pub first_frame_image: Option<Pixmap>,
    pub fps: f64,
}

impl Config {
    pub fn new(video_width: f64, video_height: f64, state: VideoEditorState) -> Self {
        Self {
            video_width,
            video_height,
            state,
            cursor_data: None,
            keyboard_data: None,
            subtitle_data: None,
            background_image: None,
            first_frame_image: None,
            fps: 60.0,
        }
    }
}

/// The source frames a composition draws from.
#[derive(Clone, Copy)]
pub struct Frames<'a> {
    pub video: Option<PixmapRef<'a>>,
    pub camera: Option<PixmapRef<'a>>,
}

pub struct Engine {
    config: Config,
    zoom_cache: ZoomCache,
    video_segments: Vec<VideoSegment>,
    cached_dimensions: (u32, u32),
}

impl Engine {
    pub fn new(config: Config) -> Self {
        let video_segments = segments::to_video_segments(&config.state.segments);
        let cached_dimensions = composition_dimensions(&config);
        Self {
            config,
            zoom_cache: ZoomCache::default(),
            video_segments,
            cached_dimensions,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Replaces the editor state. The zoom caches are keyed by segment, so they
    /// are dropped whenever the timeline could have changed.
    pub fn set_state(&mut self, state: VideoEditorState) {
        self.video_segments = segments::to_video_segments(&state.segments);
        self.config.state = state;
        self.zoom_cache.clear();
        self.cached_dimensions = composition_dimensions(&self.config);
    }

    pub fn set_background_image(&mut self, image: Option<Pixmap>) {
        self.config.background_image = image;
    }

    pub fn set_first_frame_image(&mut self, image: Option<Pixmap>) {
        self.config.first_frame_image = image;
    }

    pub fn set_frame_rate(&mut self, frame_rate: f64) {
        self.config.fps = frame_rate.max(1.0);
    }

    fn device_frame_enabled(&self) -> bool {
        let wallpaper = &self.config.state.wallpaper;
        wallpaper.enabled && wallpaper.device_frame
    }

    /// `getFirstFrameDuration` — the still holds for exactly one frame.
    pub fn first_frame_duration(&self) -> f64 {
        let first_frame = &self.config.state.first_frame;
        if !first_frame.enabled || first_frame.image_data.is_none() {
            return 0.0;
        }
        1.0 / self.config.fps.max(1.0)
    }

    /// `getCompositionDimensions`.
    pub fn dimensions(&self) -> (u32, u32) {
        self.cached_dimensions
    }

    /// The total timeline duration, including the first-frame still.
    pub fn total_duration(&self) -> f64 {
        self.first_frame_duration()
            + segments::total_duration(
                &self.config.state.segments,
                self.config.state.source_duration.unwrap_or(0.0),
            )
    }

    /// Composes a frame at `timeline_time` onto a fresh surface.
    pub fn render_frame(&mut self, timeline_time: f64, frames: Frames<'_>) -> Option<Pixmap> {
        let (width, height) = self.dimensions();
        let mut canvas = Canvas::new(width, height)?;
        self.render_into(&mut canvas, timeline_time, frames);
        Some(canvas.into_pixmap())
    }

    pub fn render_frame_scaled(
        &mut self,
        timeline_time: f64,
        frames: Frames<'_>,
        max_width: u32,
        max_height: u32,
    ) -> Option<Pixmap> {
        let (width, height) = self.dimensions();
        let scale = (max_width as f32 / width as f32)
            .min(max_height as f32 / height as f32)
            .min(1.0);
        if scale >= 1.0 {
            return self.render_frame(timeline_time, frames);
        }
        let mut canvas = Canvas::new(
            (width as f32 * scale).round().max(1.0) as u32,
            (height as f32 * scale).round().max(1.0) as u32,
        )?;
        canvas.set_shadow_scale(scale);
        canvas.scale(scale, scale);
        self.render_into(&mut canvas, timeline_time, frames);
        Some(canvas.into_pixmap())
    }

    /// Port of `renderFrame`.
    pub fn render_into(&mut self, canvas: &mut Canvas, timeline_time: f64, frames: Frames<'_>) {
        let first_frame_duration = self.first_frame_duration();
        if first_frame_duration > 0.0 && timeline_time < first_frame_duration {
            self.render_first_frame(canvas);
            self.render_drawings(canvas, timeline_time);
            return;
        }

        let adjusted_time = timeline_time - first_frame_duration;
        let is_device_frame = self.device_frame_enabled();
        let device_layout = is_device_frame.then(|| {
            device_frame::calculate_layout(self.config.video_width, self.config.video_height)
        });

        let (effective_width, effective_height) = match device_layout {
            Some(layout) => (layout.frame_width, layout.frame_height),
            None => (self.config.video_width, self.config.video_height),
        };

        let layout = wallpaper::render(
            canvas,
            Some(&self.config.state.wallpaper),
            effective_width,
            effective_height,
            self.config.background_image.as_ref().map(Pixmap::as_ref),
        );

        let transform = zoom::calculate_transform(
            &mut self.zoom_cache,
            &self.config.state.zoom_segments,
            Some(&self.config.state.zoom_settings),
            self.config.cursor_data.as_ref(),
            &self.video_segments,
            adjusted_time,
            self.config.video_width,
            self.config.video_height,
        );

        // The pointer lives inside the device's screen, so its transform is
        // shifted by the bezel when a device frame is on.
        let cursor_transform = match device_layout {
            Some(device) if !transform.is_identity() => Transform {
                translate_x: transform.translate_x + (transform.scale - 1.0) * device.screen_x,
                translate_y: transform.translate_y + (transform.scale - 1.0) * device.screen_y,
                ..transform
            },
            _ => transform,
        };

        if let Some(video) = frames.video {
            match device_layout {
                Some(device) => {
                    self.render_video_with_device_frame(canvas, video, &layout, transform, device)
                }
                None => self.render_video_with_zoom(canvas, video, &layout, transform),
            }
        }

        let cursor_layout = match device_layout {
            Some(device) => wallpaper::Layout {
                video_x: layout.video_x + device.screen_x,
                video_y: layout.video_y + device.screen_y,
                video_clip_radius: 0.0,
                ..layout
            },
            None => layout,
        };
        self.render_cursor(canvas, adjusted_time, &cursor_layout, cursor_transform);

        if let Some(camera) = frames.camera {
            self.render_camera(canvas, adjusted_time, camera, transform);
        }

        let subtitle_bounds = self.active_subtitle_bounds();
        self.render_subtitle(canvas, adjusted_time);
        self.render_keyboard(canvas, adjusted_time, subtitle_bounds);
        self.render_drawings(canvas, timeline_time);
    }

    /// `renderFirstFrame` — an intro still, fitted or stretched.
    fn render_first_frame(&self, canvas: &mut Canvas) {
        let (width, height) = self.dimensions();
        canvas.clear();
        canvas.fill_all(Color::from_rgba8(0, 0, 0, 255));

        let Some(image) = self.config.first_frame_image.as_ref() else {
            return;
        };
        if image.width() == 0 || image.height() == 0 {
            return;
        }

        if self.config.state.first_frame.fit == "stretch" {
            canvas.draw_pixmap(image.as_ref(), 0.0, 0.0, width as f32, height as f32);
            return;
        }

        let image_aspect = image.width() as f64 / image.height() as f64;
        let canvas_aspect = width as f64 / height as f64;
        let (draw_width, draw_height) = if image_aspect > canvas_aspect {
            (height as f64 * image_aspect, height as f64)
        } else {
            (width as f64, width as f64 / image_aspect)
        };
        canvas.draw_pixmap(
            image.as_ref(),
            ((width as f64 - draw_width) / 2.0) as f32,
            ((height as f64 - draw_height) / 2.0) as f32,
            draw_width as f32,
            draw_height as f32,
        );
    }

    /// `renderVideoWithZoom`.
    fn render_video_with_zoom(
        &self,
        canvas: &mut Canvas,
        video: PixmapRef<'_>,
        layout: &wallpaper::Layout,
        transform: Transform,
    ) {
        canvas.save();
        if !transform.is_identity() {
            canvas.translate(layout.video_x as f32, layout.video_y as f32);
            canvas.scale(transform.scale as f32, transform.scale as f32);
            canvas.translate(
                (transform.translate_x / transform.scale) as f32,
                (transform.translate_y / transform.scale) as f32,
            );
            self.draw_video_rect(canvas, video, 0.0, 0.0, layout);
        } else {
            self.draw_video_rect(canvas, video, layout.video_x, layout.video_y, layout);
        }
        canvas.restore();
    }

    /// The clipped, optionally shadowed video rect. A shadow needs an opaque
    /// shape behind the frame, which is what the renderer's offscreen copy is.
    fn draw_video_rect(
        &self,
        canvas: &mut Canvas,
        video: PixmapRef<'_>,
        x: f64,
        y: f64,
        layout: &wallpaper::Layout,
    ) {
        let clip = rounded_rect_path(
            x as f32,
            y as f32,
            self.config.video_width as f32,
            self.config.video_height as f32,
            layout.video_clip_radius as f32,
        );
        canvas.save();
        if let (Some(shadow), Some(path)) = (layout.shadow, clip.as_ref()) {
            canvas.set_shadow(Some(shadow));
            canvas.fill_path(path, Color::from_rgba8(0, 0, 0, 255), FillRule::Winding);
            canvas.set_shadow(None);
        }
        if let Some(path) = clip.as_ref() {
            canvas.clip_path(path, FillRule::Winding);
        }
        canvas.draw_pixmap(
            video,
            x as f32,
            y as f32,
            self.config.video_width as f32,
            self.config.video_height as f32,
        );
        canvas.restore();
    }

    /// `renderVideoWithDeviceFrame`.
    fn render_video_with_device_frame(
        &self,
        canvas: &mut Canvas,
        video: PixmapRef<'_>,
        layout: &wallpaper::Layout,
        transform: Transform,
        device: device_frame::Layout,
    ) {
        canvas.save();
        if !transform.is_identity() {
            canvas.save();
            canvas.translate(layout.video_x as f32, layout.video_y as f32);
            canvas.scale(transform.scale as f32, transform.scale as f32);
            canvas.translate(
                (transform.translate_x / transform.scale) as f32,
                (transform.translate_y / transform.scale) as f32,
            );
            self.draw_screen(canvas, video, device.screen_x, device.screen_y, device);
            canvas.restore();
            device_frame::render(canvas, device, 0.0, 0.0, layout.shadow);
        } else {
            self.draw_screen(
                canvas,
                video,
                layout.video_x + device.screen_x,
                layout.video_y + device.screen_y,
                device,
            );
            device_frame::render(
                canvas,
                device,
                layout.video_x,
                layout.video_y,
                layout.shadow,
            );
        }
        canvas.restore();
    }

    fn draw_screen(
        &self,
        canvas: &mut Canvas,
        video: PixmapRef<'_>,
        x: f64,
        y: f64,
        device: device_frame::Layout,
    ) {
        canvas.save();
        if let Some(path) = rounded_rect_path(
            x as f32,
            y as f32,
            self.config.video_width as f32,
            self.config.video_height as f32,
            device.screen_corner_radius as f32,
        ) {
            canvas.clip_path(&path, FillRule::Winding);
        }
        canvas.draw_pixmap(
            video,
            x as f32,
            y as f32,
            self.config.video_width as f32,
            self.config.video_height as f32,
        );
        canvas.restore();
    }

    /// `renderCursorOverlay`.
    fn render_cursor(
        &self,
        canvas: &mut Canvas,
        timeline_time: f64,
        layout: &wallpaper::Layout,
        transform: Transform,
    ) {
        let Some(cursor_data) = self
            .config
            .cursor_data
            .as_ref()
            .filter(|data| !data.is_empty())
        else {
            return;
        };
        let style: &CursorStyle = &self.config.state.cursor_style;
        if !style.enabled {
            return;
        }

        canvas.save();
        if !transform.is_identity() {
            canvas.translate(layout.video_x as f32, layout.video_y as f32);
            canvas.scale(transform.scale as f32, transform.scale as f32);
            canvas.translate(
                (transform.translate_x / transform.scale) as f32,
                (transform.translate_y / transform.scale) as f32,
            );
            if layout.video_clip_radius > 0.0 {
                if let Some(path) = rounded_rect_path(
                    0.0,
                    0.0,
                    self.config.video_width as f32,
                    self.config.video_height as f32,
                    layout.video_clip_radius as f32,
                ) {
                    canvas.clip_path(&path, FillRule::Winding);
                }
            }
            cursor::render(
                canvas,
                timeline_time,
                &cursor::RenderConfig {
                    cursor_data,
                    cursor_style: style,
                    segments: &self.video_segments,
                    video_width: self.config.video_width,
                    video_height: self.config.video_height,
                    offset_x: 0.0,
                    offset_y: 0.0,
                },
            );
        } else {
            if layout.video_clip_radius > 0.0 {
                if let Some(path) = rounded_rect_path(
                    layout.video_x as f32,
                    layout.video_y as f32,
                    self.config.video_width as f32,
                    self.config.video_height as f32,
                    layout.video_clip_radius as f32,
                ) {
                    canvas.clip_path(&path, FillRule::Winding);
                }
            }
            cursor::render(
                canvas,
                timeline_time,
                &cursor::RenderConfig {
                    cursor_data,
                    cursor_style: style,
                    segments: &self.video_segments,
                    video_width: self.config.video_width,
                    video_height: self.config.video_height,
                    offset_x: layout.video_x,
                    offset_y: layout.video_y,
                },
            );
        }
        canvas.restore();
    }

    /// `renderCameraOverlay`.
    fn render_camera(
        &self,
        canvas: &mut Canvas,
        timeline_time: f64,
        source: PixmapRef<'_>,
        transform: Transform,
    ) {
        let style = &self.config.state.camera_style;
        if !style.visible {
            return;
        }
        let (width, height) = self.dimensions();
        let ranges = (!self.config.state.camera_segments.is_empty())
            .then_some(self.config.state.camera_segments.as_slice());
        camera::render(
            canvas,
            timeline_time,
            source,
            &camera::RenderConfig {
                camera_style: style,
                camera_visible_ranges: ranges,
                cursor_data: self.config.cursor_data.as_ref(),
                segments: &self.video_segments,
                video_width: width as f64,
                video_height: height as f64,
                offset_x: 0.0,
                offset_y: 0.0,
                zoom: Some(camera::ZoomInfo {
                    scale: transform.scale,
                    viewport: transform.viewport,
                }),
            },
        );
    }

    /// `getActiveSubtitleBounds` — only a bottom-anchored caption box pushes the
    /// keyboard pills up.
    fn active_subtitle_bounds(&self) -> Option<subtitle::Bounds> {
        let data = self.config.subtitle_data.as_ref()?;
        if data.segments.is_empty() {
            return None;
        }
        let style = &self.config.state.subtitle_style;
        if !style.visible || style.position != "bottom" {
            return None;
        }
        let (_, height) = self.dimensions();
        Some(subtitle::bounds(style, height as f64))
    }

    fn render_subtitle(&self, canvas: &mut Canvas, timeline_time: f64) {
        let Some(data) = self
            .config
            .subtitle_data
            .as_ref()
            .filter(|data| !data.segments.is_empty())
        else {
            return;
        };
        let style = &self.config.state.subtitle_style;
        if !style.visible {
            return;
        }
        let (width, height) = self.dimensions();
        subtitle::render(
            canvas,
            timeline_time,
            &subtitle::RenderConfig {
                subtitle_data: data,
                subtitle_style: style,
                segments: &self.video_segments,
                video_width: width as f64,
                video_height: height as f64,
            },
        );
    }

    fn render_keyboard(
        &self,
        canvas: &mut Canvas,
        timeline_time: f64,
        subtitle_bounds: Option<subtitle::Bounds>,
    ) {
        let Some(data) = self
            .config
            .keyboard_data
            .as_ref()
            .filter(|data| !data.events.is_empty())
        else {
            return;
        };
        let style = &self.config.state.keyboard_style;
        if !style.visible {
            return;
        }
        let (width, height) = self.dimensions();
        keyboard::render(
            canvas,
            timeline_time,
            &keyboard::RenderConfig {
                keyboard_data: data,
                keyboard_style: style,
                segments: &self.video_segments,
                video_width: width as f64,
                video_height: height as f64,
                subtitle_bounds,
            },
        );
    }

    fn render_drawings(&self, canvas: &mut Canvas, timeline_time: f64) {
        if self.config.state.drawing_segments.is_empty() {
            return;
        }
        let (width, height) = self.dimensions();
        drawing::render(
            canvas,
            &self.config.state.drawing_segments,
            timeline_time,
            width as f64,
            height as f64,
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows::video_editor::model::Segment;
    use crate::windows::video_editor::styles::VideoWallpaperSettings;

    fn state() -> VideoEditorState {
        VideoEditorState {
            segments: vec![Segment {
                id: "a".into(),
                original_start: 0.0,
                original_end: 4.0,
                trim_min_start: 0.0,
                trim_max_end: 4.0,
                speed: None,
            }],
            source_duration: Some(4.0),
            ..VideoEditorState::default()
        }
    }

    fn source(width: u32, height: u32, color: Color) -> Pixmap {
        let mut pixmap = Pixmap::new(width, height).expect("pixmap");
        pixmap.fill(color);
        pixmap
    }

    #[test]
    fn without_a_wallpaper_the_composition_is_the_source_size() {
        let engine = Engine::new(Config::new(1920.0, 1080.0, state()));
        assert_eq!(engine.dimensions(), (1920, 1080));
    }

    #[test]
    fn padding_and_a_ratio_grow_the_composition() {
        let mut config = Config::new(1000.0, 1000.0, state());
        config.state.wallpaper = VideoWallpaperSettings {
            enabled: true,
            padding: 50.0,
            aspect_ratio: Some(serde_json::json!({ "width": 16, "height": 9 })),
            ..VideoWallpaperSettings::default()
        };
        let engine = Engine::new(config);
        assert_eq!(engine.dimensions(), (1956, 1100));
    }

    #[test]
    fn the_device_frame_adds_its_bezel_to_the_composition() {
        let mut config = Config::new(1170.0, 2532.0, state());
        config.state.wallpaper = VideoWallpaperSettings {
            enabled: true,
            device_frame: true,
            ..VideoWallpaperSettings::default()
        };
        let engine = Engine::new(config);
        assert_eq!(engine.dimensions(), (1220, 2582));
    }

    #[test]
    fn a_frame_is_composited_at_the_padded_offset() {
        let mut config = Config::new(40.0, 40.0, state());
        config.state.wallpaper = VideoWallpaperSettings {
            enabled: true,
            padding: 10.0,
            gradient: Some(serde_json::json!({
                "id": "slate",
                "colors": ["#111111", "#222222"],
                "angle": 0
            })),
            ..VideoWallpaperSettings::default()
        };
        let mut engine = Engine::new(config);
        let video = source(40, 40, Color::from_rgba8(255, 0, 0, 255));
        let composed = engine
            .render_frame(
                0.0,
                Frames {
                    video: Some(video.as_ref()),
                    camera: None,
                },
            )
            .expect("frame");

        assert_eq!((composed.width(), composed.height()), (60, 60));
        let pixel = |x: u32, y: u32| {
            let index = ((y * composed.width() + x) * 4) as usize;
            let data = composed.data();
            [
                data[index],
                data[index + 1],
                data[index + 2],
                data[index + 3],
            ]
        };
        assert_eq!(pixel(30, 30), [255, 0, 0, 255]);
        // The padding shows the gradient, not the video.
        assert!(pixel(2, 2)[0] < 255);
        assert_eq!(pixel(2, 2)[3], 255);
    }

    #[test]
    fn scaled_frames_keep_the_composition_aspect_ratio() {
        let mut engine = Engine::new(Config::new(1920.0, 1080.0, state()));
        let video = source(1920, 1080, Color::from_rgba8(255, 0, 0, 255));
        let composed = engine
            .render_frame_scaled(
                0.0,
                Frames {
                    video: Some(video.as_ref()),
                    camera: None,
                },
                640,
                360,
            )
            .expect("frame");
        assert_eq!((composed.width(), composed.height()), (640, 360));
    }

    #[test]
    fn the_first_frame_still_holds_for_one_frame() {
        let mut config = Config::new(40.0, 40.0, state());
        config.state.first_frame.enabled = true;
        config.state.first_frame.image_data = Some("data:,".into());
        config.fps = 25.0;
        let engine = Engine::new(config);
        assert_eq!(engine.first_frame_duration(), 0.04);
        assert_eq!(engine.total_duration(), 4.04);
    }

    #[test]
    fn a_disabled_first_frame_adds_no_duration() {
        let engine = Engine::new(Config::new(40.0, 40.0, state()));
        assert_eq!(engine.first_frame_duration(), 0.0);
        assert_eq!(engine.total_duration(), 4.0);
    }

    #[test]
    fn changing_frame_rate_updates_the_first_frame_duration() {
        let mut state = state();
        state.first_frame.enabled = true;
        state.first_frame.image_data = Some("data:,".into());
        let mut engine = Engine::new(Config::new(40.0, 40.0, state));
        engine.set_frame_rate(24.0);
        assert_eq!(engine.first_frame_duration(), 1.0 / 24.0);
    }

    #[test]
    fn replacing_the_state_reslices_the_timeline() {
        let mut engine = Engine::new(Config::new(40.0, 40.0, state()));
        assert_eq!(engine.total_duration(), 4.0);

        let mut next = state();
        next.segments[0].speed = Some(2.0);
        engine.set_state(next);
        assert_eq!(engine.total_duration(), 2.0);
    }
}
