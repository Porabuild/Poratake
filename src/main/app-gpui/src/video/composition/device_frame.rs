//! Port of `composition/device-frame-canvas-renderer.ts` — the iPhone/iPad
//! bezel an iOS recording is composited inside.

use tiny_skia::{Color, FillRule, LineJoin, Stroke};

use crate::render::canvas::{rounded_rect_path, Canvas, Shadow};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceType {
    Iphone,
    Ipad,
}

#[derive(Clone, Copy, Debug)]
struct DeviceConfig {
    bezel_ratio: f64,
    device_corner_ratio: f64,
    screen_corner_ratio: f64,
    side_button_width_ratio: f64,
    dynamic_island_width_ratio: f64,
    dynamic_island_height_ratio: f64,
    dynamic_island_top_ratio: f64,
    has_dynamic_island: bool,
}

const IPHONE: DeviceConfig = DeviceConfig {
    bezel_ratio: 0.01,
    device_corner_ratio: 0.065,
    screen_corner_ratio: 0.055,
    side_button_width_ratio: 0.006,
    dynamic_island_width_ratio: 0.25,
    dynamic_island_height_ratio: 0.03,
    dynamic_island_top_ratio: 0.02,
    has_dynamic_island: true,
};

const IPAD: DeviceConfig = DeviceConfig {
    bezel_ratio: 0.008,
    device_corner_ratio: 0.035,
    screen_corner_ratio: 0.025,
    side_button_width_ratio: 0.004,
    dynamic_island_width_ratio: 0.0,
    dynamic_island_height_ratio: 0.0,
    dynamic_island_top_ratio: 0.0,
    has_dynamic_island: false,
};

fn device_color() -> Color {
    Color::from_rgba8(0x1a, 0x1a, 0x1a, 255)
}

fn device_edge_color() -> Color {
    Color::from_rgba8(0x2a, 0x2a, 0x2a, 255)
}

fn dynamic_island_color() -> Color {
    Color::from_rgba8(0, 0, 0, 255)
}

const IPHONE_MAX_ASPECT_RATIO: f64 = 0.55;
const IPAD_MIN_ASPECT_RATIO: f64 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub frame_width: f64,
    pub frame_height: f64,
    pub screen_x: f64,
    pub screen_y: f64,
    pub screen_width: f64,
    pub screen_height: f64,
    pub screen_corner_radius: f64,
    pub device_type: DeviceType,
}

fn config_for(video_width: f64, video_height: f64) -> (DeviceConfig, DeviceType) {
    let aspect = video_width / video_height;
    if aspect <= IPHONE_MAX_ASPECT_RATIO {
        return (IPHONE, DeviceType::Iphone);
    }
    if aspect >= IPAD_MIN_ASPECT_RATIO {
        return (IPAD, DeviceType::Ipad);
    }
    if video_width >= 1400.0 {
        (IPAD, DeviceType::Ipad)
    } else {
        (IPHONE, DeviceType::Iphone)
    }
}

/// Port of `calculateDeviceFrameLayout`.
pub fn calculate_layout(video_width: f64, video_height: f64) -> Layout {
    let (config, device_type) = config_for(video_width, video_height);
    let bezel = (video_width.max(video_height) * config.bezel_ratio).round();
    let frame_width = video_width + bezel * 2.0;
    let frame_height = video_height + bezel * 2.0;
    let screen_corner_radius = (frame_width.max(frame_height) * config.screen_corner_ratio).round();

    Layout {
        frame_width,
        frame_height,
        screen_x: bezel,
        screen_y: bezel,
        screen_width: video_width,
        screen_height: video_height,
        screen_corner_radius,
        device_type,
    }
}

/// Port of `renderDeviceFrame`. The body is drawn into an offscreen so the
/// screen can be punched out of it before it is composited with its shadow.
pub fn render(
    canvas: &mut Canvas,
    layout: Layout,
    offset_x: f64,
    offset_y: f64,
    shadow: Option<Shadow>,
) {
    let config = match layout.device_type {
        DeviceType::Ipad => IPAD,
        DeviceType::Iphone => IPHONE,
    };

    let padded_width = (layout.frame_width + layout.frame_width * config.side_button_width_ratio)
        .ceil()
        .max(1.0) as u32;
    let padded_height = layout.frame_height.max(1.0) as u32;
    let Some(mut frame) = Canvas::new(padded_width, padded_height) else {
        return;
    };

    let device_corner_radius =
        (layout.frame_width.max(layout.frame_height) * config.device_corner_ratio).round();
    let screen_corner_radius =
        (layout.frame_width.max(layout.frame_height) * config.screen_corner_ratio).round();
    let button_pad = (layout.frame_width * config.side_button_width_ratio).round() / 2.0;

    frame.save();
    frame.translate(button_pad as f32, 0.0);
    draw_body(
        &mut frame,
        layout.frame_width,
        layout.frame_height,
        device_corner_radius,
    );
    draw_side_buttons(
        &mut frame,
        layout.frame_width,
        layout.frame_height,
        device_corner_radius,
        &config,
    );
    punch_screen(
        &mut frame,
        layout.screen_x,
        layout.screen_y,
        layout.screen_width,
        layout.screen_height,
        screen_corner_radius,
    );
    if config.has_dynamic_island {
        draw_dynamic_island(&mut frame, layout.frame_width, layout.frame_height, &config);
    }
    frame.restore();

    let body = frame.into_pixmap();
    canvas.save();
    canvas.set_shadow(shadow);
    canvas.draw_pixmap(
        body.as_ref(),
        (offset_x - button_pad) as f32,
        offset_y as f32,
        padded_width as f32,
        padded_height as f32,
    );
    canvas.restore();
}

fn draw_body(canvas: &mut Canvas, width: f64, height: f64, corner_radius: f64) {
    let Some(path) = rounded_rect_path(0.0, 0.0, width as f32, height as f32, corner_radius as f32)
    else {
        return;
    };
    canvas.save();
    canvas.fill_path(&path, device_color(), FillRule::Winding);
    canvas.stroke_path(
        &path,
        device_edge_color(),
        &Stroke {
            width: 2.0,
            line_join: LineJoin::Miter,
            ..Stroke::default()
        },
    );
    canvas.restore();
}

fn draw_side_buttons(
    canvas: &mut Canvas,
    width: f64,
    height: f64,
    corner_radius: f64,
    config: &DeviceConfig,
) {
    let button_width = (width * config.side_button_width_ratio).round();
    let button_radius = button_width / 2.0;
    canvas.save();

    let mut button = |x: f64, y: f64, height: f64| {
        if let Some(path) = rounded_rect_path(
            x as f32,
            y as f32,
            button_width as f32,
            height as f32,
            button_radius as f32,
        ) {
            canvas.fill_path(&path, device_edge_color(), FillRule::Winding);
        }
    };

    let power_y = corner_radius + height * 0.15;
    button(width - button_width / 2.0, power_y, height * 0.1);

    let mute_y = corner_radius + height * 0.1;
    let mute_height = height * 0.03;
    button(-button_width / 2.0, mute_y, mute_height);

    let volume_up_y = mute_y + mute_height + height * 0.02;
    let volume_height = height * 0.06;
    button(-button_width / 2.0, volume_up_y, volume_height);

    let volume_down_y = volume_up_y + volume_height + height * 0.01;
    button(-button_width / 2.0, volume_down_y, volume_height);

    canvas.restore();
}

/// `drawScreenCutout` — `destination-out` in the renderer, a hole in the body
/// that the already-drawn video shows through.
fn punch_screen(canvas: &mut Canvas, x: f64, y: f64, width: f64, height: f64, corner_radius: f64) {
    let Some(path) = rounded_rect_path(
        x as f32,
        y as f32,
        width as f32,
        height as f32,
        corner_radius as f32,
    ) else {
        return;
    };
    canvas.save();
    canvas.fill_path_blended(
        &path,
        Color::from_rgba8(0, 0, 0, 255),
        FillRule::Winding,
        tiny_skia::BlendMode::DestinationOut,
    );
    canvas.restore();
}

fn draw_dynamic_island(
    canvas: &mut Canvas,
    frame_width: f64,
    frame_height: f64,
    config: &DeviceConfig,
) {
    let longest = frame_width.max(frame_height);
    let island_width = (frame_width * config.dynamic_island_width_ratio).round();
    let island_height = (longest * config.dynamic_island_height_ratio).round();
    let island_x = ((frame_width - island_width) / 2.0).round();
    let top_bezel = (longest * config.bezel_ratio).round();
    let island_top_offset = (longest * config.dynamic_island_top_ratio).round();
    let island_y = top_bezel + island_top_offset;

    canvas.save();
    if let Some(path) = rounded_rect_path(
        island_x as f32,
        island_y as f32,
        island_width as f32,
        island_height as f32,
        (island_height / 2.0) as f32,
    ) {
        canvas.fill_path(&path, dynamic_island_color(), FillRule::Winding);
    }
    canvas.restore();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tall_recording_is_framed_as_an_iphone() {
        let layout = calculate_layout(1170.0, 2532.0);
        assert_eq!(layout.device_type, DeviceType::Iphone);
        // 1% of the longest side, on each edge.
        assert_eq!(layout.screen_x, 25.0);
        assert_eq!(layout.frame_width, 1220.0);
        assert_eq!(layout.frame_height, 2582.0);
    }

    #[test]
    fn a_landscape_recording_is_framed_as_an_ipad() {
        let layout = calculate_layout(2360.0, 1640.0);
        assert_eq!(layout.device_type, DeviceType::Ipad);
        assert_eq!(layout.screen_x, 19.0);
    }

    #[test]
    fn an_ambiguous_ratio_falls_back_to_width() {
        assert_eq!(
            calculate_layout(1500.0, 2600.0).device_type,
            DeviceType::Ipad
        );
        assert_eq!(
            calculate_layout(1200.0, 2080.0).device_type,
            DeviceType::Iphone
        );
    }

    #[test]
    fn the_screen_cutout_is_transparent_in_the_body() {
        let mut canvas = Canvas::new(240, 480).expect("canvas");
        let layout = calculate_layout(200.0, 440.0);
        render(&mut canvas, layout, 0.0, 0.0, None);

        let alpha_at = |x: u32, y: u32| {
            let index = ((y * canvas.width() + x) * 4 + 3) as usize;
            canvas.pixmap().data()[index]
        };
        // The middle of the screen is punched out; the bezel is not.
        assert_eq!(alpha_at(120, 240), 0);
        assert!(alpha_at(120, 2) > 0);
    }
}
