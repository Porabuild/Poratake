//! Port of `composition/wallpaper-canvas-renderer.ts` and the geometry in
//! `types/video-wallpaper.ts`.

use tiny_skia::Color;

use crate::render::canvas::{Canvas, Shadow};
use crate::render::gradient::{self, GradientOption};
use crate::windows::video_editor::styles::VideoWallpaperSettings;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Dimensions {
    pub width: f64,
    pub height: f64,
    pub video_x: f64,
    pub video_y: f64,
}

/// An `AspectRatio` from `types/aspect-ratio.ts` as it is persisted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AspectRatio {
    pub width: f64,
    pub height: f64,
}

pub fn aspect_ratio(settings: &VideoWallpaperSettings) -> Option<AspectRatio> {
    let value = settings.aspect_ratio.as_ref()?;
    let width = value.get("width")?.as_f64()?;
    let height = value.get("height")?.as_f64()?;
    (width != 0.0 || height != 0.0).then_some(AspectRatio { width, height })
}

pub fn gradient_option(settings: &VideoWallpaperSettings) -> Option<GradientOption> {
    let value = settings.gradient.as_ref()?;
    serde_json::from_value(value.clone()).ok()
}

/// Port of `calculateWallpaperDimensions`.
pub fn calculate_dimensions(
    video_width: f64,
    video_height: f64,
    padding: f64,
    ratio: Option<AspectRatio>,
) -> Dimensions {
    let base_width = video_width + padding * 2.0;
    let base_height = video_height + padding * 2.0;

    let Some(ratio) = ratio.filter(|ratio| ratio.width != 0.0 || ratio.height != 0.0) else {
        return Dimensions {
            width: base_width,
            height: base_height,
            video_x: padding,
            video_y: padding,
        };
    };

    let target = ratio.width / ratio.height;
    let current = base_width / base_height;
    let (width, height) = if current < target {
        ((base_height * target).round(), base_height)
    } else {
        (base_width, (base_width / target).round())
    };

    Dimensions {
        width,
        height,
        video_x: ((width - video_width) / 2.0).round(),
        video_y: ((height - video_height) / 2.0).round(),
    }
}

/// Port of `calculateShadowConfig`.
pub fn shadow_config(shadow: f64) -> Option<Shadow> {
    if shadow == 0.0 {
        return None;
    }
    let blur = (shadow * 0.25).round();
    Some(Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, (shadow / 300.0).min(0.5) as f32)
            .unwrap_or(Color::TRANSPARENT),
        blur: blur as f32,
        offset_x: 0.0,
        offset_y: (blur * 0.3).round() as f32,
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layout {
    pub composition_width: f64,
    pub composition_height: f64,
    pub video_x: f64,
    pub video_y: f64,
    pub video_clip_radius: f64,
    pub shadow: Option<Shadow>,
}

/// Port of `renderWallpaper`: clears the surface, paints the background and
/// reports where the video goes.
pub fn render(
    canvas: &mut Canvas,
    wallpaper: Option<&VideoWallpaperSettings>,
    video_width: f64,
    video_height: f64,
    background_image: Option<tiny_skia::PixmapRef<'_>>,
) -> Layout {
    let enabled = wallpaper.is_some_and(|settings| settings.enabled);
    let padding = if enabled {
        wallpaper.map_or(0.0, |settings| settings.padding)
    } else {
        0.0
    };
    let corners = if enabled {
        wallpaper.map_or(0.0, |settings| settings.corners)
    } else {
        0.0
    };
    let shadow = if enabled {
        wallpaper.map_or(0.0, |settings| settings.shadow)
    } else {
        0.0
    };
    let ratio = if enabled {
        wallpaper.and_then(aspect_ratio)
    } else {
        None
    };

    let dimensions = calculate_dimensions(video_width, video_height, padding, ratio);
    canvas.clear();

    if enabled {
        let gradient = wallpaper.and_then(gradient_option);
        match (gradient, background_image) {
            (Some(gradient), _) if gradient.is_renderable() => gradient::fill(
                canvas,
                &gradient,
                dimensions.width as f32,
                dimensions.height as f32,
            ),
            (_, Some(image)) => gradient::fill_image(
                canvas,
                image,
                dimensions.width as f32,
                dimensions.height as f32,
            ),
            _ => {}
        }
    }

    Layout {
        composition_width: dimensions.width,
        composition_height: dimensions.height,
        video_x: dimensions.video_x,
        video_y: dimensions.video_y,
        video_clip_radius: corners,
        shadow: shadow_config(shadow),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_only_grows_the_composition_symmetrically() {
        let dimensions = calculate_dimensions(1920.0, 1080.0, 100.0, None);
        assert_eq!(dimensions.width, 2120.0);
        assert_eq!(dimensions.height, 1280.0);
        assert_eq!(dimensions.video_x, 100.0);
        assert_eq!(dimensions.video_y, 100.0);
    }

    #[test]
    fn a_wider_target_ratio_widens_the_composition() {
        let dimensions = calculate_dimensions(
            1080.0,
            1080.0,
            0.0,
            Some(AspectRatio {
                width: 16.0,
                height: 9.0,
            }),
        );
        assert_eq!(dimensions.height, 1080.0);
        assert_eq!(dimensions.width, 1920.0);
        assert_eq!(dimensions.video_x, 420.0);
        assert_eq!(dimensions.video_y, 0.0);
    }

    #[test]
    fn a_taller_target_ratio_grows_the_height() {
        let dimensions = calculate_dimensions(
            1920.0,
            1080.0,
            0.0,
            Some(AspectRatio {
                width: 9.0,
                height: 16.0,
            }),
        );
        assert_eq!(dimensions.width, 1920.0);
        assert_eq!(dimensions.height, 3413.0);
    }

    #[test]
    fn a_zero_ratio_is_treated_as_no_ratio() {
        let dimensions = calculate_dimensions(
            100.0,
            100.0,
            0.0,
            Some(AspectRatio {
                width: 0.0,
                height: 0.0,
            }),
        );
        assert_eq!((dimensions.width, dimensions.height), (100.0, 100.0));
    }

    #[test]
    fn shadow_strength_follows_the_renderer_formula() {
        assert!(shadow_config(0.0).is_none());
        let shadow = shadow_config(200.0).expect("shadow");
        assert_eq!(shadow.blur, 50.0);
        assert_eq!(shadow.offset_y, 15.0);
        assert!((shadow.color.alpha() - 0.5).abs() < 0.001);

        let softer = shadow_config(60.0).expect("shadow");
        assert_eq!(softer.blur, 15.0);
        assert!((softer.color.alpha() - 0.2).abs() < 0.001);
    }

    #[test]
    fn a_disabled_wallpaper_reports_the_video_size() {
        let mut canvas = Canvas::new(64, 64).expect("canvas");
        let settings = VideoWallpaperSettings {
            enabled: false,
            padding: 100.0,
            corners: 20.0,
            shadow: 50.0,
            ..VideoWallpaperSettings::default()
        };
        let layout = render(&mut canvas, Some(&settings), 40.0, 30.0, None);
        assert_eq!(layout.composition_width, 40.0);
        assert_eq!(layout.composition_height, 30.0);
        assert_eq!(layout.video_clip_radius, 0.0);
        assert!(layout.shadow.is_none());
    }

    #[test]
    fn a_gradient_paints_the_backdrop() {
        let mut canvas = Canvas::new(60, 60).expect("canvas");
        let settings = VideoWallpaperSettings {
            enabled: true,
            padding: 10.0,
            gradient: Some(serde_json::json!({
                "id": "ocean",
                "colors": ["#0ea5e9", "#2563eb"],
                "angle": 135
            })),
            ..VideoWallpaperSettings::default()
        };
        let layout = render(&mut canvas, Some(&settings), 40.0, 40.0, None);
        assert_eq!(layout.composition_width, 60.0);
        let corner = canvas.pixmap().data()[3];
        assert_eq!(corner, 255);
    }
}
