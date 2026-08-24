//! Port of `composition/keyboard-canvas-renderer.ts` — the shortcut pills that
//! appear when a recording captured modifier combinations.

use tiny_skia::{Color, FillRule};

use crate::render::canvas::{rounded_rect_path, Canvas};
use crate::render::text;
use crate::video::composition::segments::{map_timeline_to_video_time, VideoSegment};
use crate::video::composition::subtitle::{
    self, Bounds, CORNER_RADIUS, MARGIN_EDGE, PADDING_HORIZONTAL, PADDING_VERTICAL,
};
use crate::video::sidecars::{KeyboardData, KeyboardKeyEvent};
use crate::windows::video_editor::styles::KeyboardStyle;

const FONT_FAMILY: &str = "sans";
const GAP: f64 = 8.0;
const MAX_VISIBLE_KEYS: usize = 3;

/// `MODIFIER_SYMBOLS`.
fn modifier_symbol(modifier: &str) -> &'static str {
    match modifier {
        "command" => "\u{2318}",
        "control" => "\u{2303}",
        "option" => "\u{2325}",
        "shift" => "\u{21E7}",
        "fn" => "fn",
        "meta" => "\u{229E}",
        "alt" => "Alt",
        _ => "",
    }
}

/// `WINDOWS_MODIFIER_LABELS`.
fn windows_modifier_label(modifier: &str) -> Option<&'static str> {
    match modifier {
        "control" => Some("Ctrl"),
        "alt" => Some("Alt"),
        "shift" => Some("Shift"),
        "meta" => Some("Win"),
        _ => None,
    }
}

/// `KEY_SYMBOLS`.
fn key_symbol(key: &str) -> Option<&'static str> {
    match key {
        "Return" => Some("\u{21A9}"),
        "Tab" => Some("\u{21E5}"),
        "Space" => Some("\u{2423}"),
        "Delete" => Some("\u{232B}"),
        "ForwardDelete" => Some("\u{2326}"),
        "Escape" => Some("\u{238B}"),
        "LeftArrow" => Some("\u{2190}"),
        "RightArrow" => Some("\u{2192}"),
        "UpArrow" => Some("\u{2191}"),
        "DownArrow" => Some("\u{2193}"),
        _ => None,
    }
}

/// `COMMAND_MODIFIERS` — only combinations with one of these are shown, so
/// ordinary typing does not fill the frame with pills.
fn is_shortcut_combo(event: &KeyboardKeyEvent) -> bool {
    event.modifiers.iter().any(|modifier| {
        matches!(
            modifier.as_str(),
            "command" | "control" | "option" | "meta" | "alt"
        )
    })
}

/// `formatKeyDisplay`.
pub fn format_key(event: &KeyboardKeyEvent, platform: Option<&str>) -> String {
    let windows = platform == Some("windows");
    let order: [&str; 4] = if windows {
        ["control", "alt", "shift", "meta"]
    } else {
        ["control", "option", "shift", "command"]
    };

    let mut parts: Vec<String> = Vec::new();
    for modifier in order {
        if !event.modifiers.iter().any(|value| value == modifier) {
            continue;
        }
        let label = if windows {
            windows_modifier_label(modifier).unwrap_or_else(|| modifier_symbol(modifier))
        } else {
            modifier_symbol(modifier)
        };
        parts.push(label.to_string());
    }
    parts.push(
        key_symbol(&event.key)
            .map(str::to_string)
            .unwrap_or_else(|| event.key.to_uppercase()),
    );
    parts.join(if windows { "+" } else { "" })
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveKey {
    pub display_text: String,
    pub expires_at: f64,
}

/// `getActiveKeys`.
pub fn active_keys(
    data: &KeyboardData,
    segments: &[VideoSegment],
    timeline_time: f64,
    style: &KeyboardStyle,
) -> Vec<ActiveKey> {
    let Some(video_time) = map_timeline_to_video_time(timeline_time, segments) else {
        return Vec::new();
    };
    let mut keys: Vec<ActiveKey> = Vec::new();
    for event in &data.events {
        if event.kind != "down" || !is_shortcut_combo(event) {
            continue;
        }
        let since_press = video_time - event.timestamp;
        if since_press < 0.0 || since_press > style.display_duration {
            continue;
        }
        keys.push(ActiveKey {
            display_text: format_key(event, data.meta.platform.as_deref()),
            expires_at: event.timestamp + style.display_duration,
        });
    }
    if keys.len() > MAX_VISIBLE_KEYS {
        keys.drain(..keys.len() - MAX_VISIBLE_KEYS);
    }
    keys
}

pub struct RenderConfig<'a> {
    pub keyboard_data: &'a KeyboardData,
    pub keyboard_style: &'a KeyboardStyle,
    pub segments: &'a [VideoSegment],
    pub video_width: f64,
    pub video_height: f64,
    pub subtitle_bounds: Option<Bounds>,
}

/// Port of `renderKeyboard`.
pub fn render(canvas: &mut Canvas, timeline_time: f64, config: &RenderConfig<'_>) {
    if !config.keyboard_style.visible {
        return;
    }
    let keys = active_keys(
        config.keyboard_data,
        config.segments,
        timeline_time,
        config.keyboard_style,
    );
    if keys.is_empty() {
        return;
    }
    let Some(video_time) = map_timeline_to_video_time(timeline_time, config.segments) else {
        return;
    };

    let size = subtitle::font_size(&config.keyboard_style.font_size);
    let line_height = size * 1.3;
    let widths: Vec<f64> = keys
        .iter()
        .map(|key| {
            text::measure(&key.display_text, FONT_FAMILY, size as f32).width as f64
                + PADDING_HORIZONTAL * 2.0
        })
        .collect();
    let total_width: f64 = widths.iter().sum::<f64>() + GAP * (widths.len() as f64 - 1.0);

    let mut x = (config.video_width - total_width) / 2.0;
    let height = line_height + PADDING_VERTICAL * 2.0;
    let mut y = config.video_height - MARGIN_EDGE - height;

    // Captions own the bottom edge, so the pills stack above them.
    if let Some(bounds) = config.subtitle_bounds {
        const OVERLAP_GAP: f64 = 16.0;
        if y + height > bounds.y - OVERLAP_GAP {
            y = bounds.y - OVERLAP_GAP - height;
        }
    }

    canvas.save();
    let text_y = y + height / 2.0;
    for (key, width) in keys.iter().zip(widths) {
        let center_x = x + width / 2.0;
        let remaining = key.expires_at - video_time;
        let fade_start = config.keyboard_style.display_duration * 0.3;
        let fade = if remaining < fade_start && fade_start > 0.0 {
            (remaining / fade_start).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let text_opacity = (config.keyboard_style.opacity + 0.25).min(1.0) * fade;

        if let Some(path) = rounded_rect_path(
            x as f32,
            y as f32,
            width as f32,
            height as f32,
            CORNER_RADIUS as f32,
        ) {
            canvas.fill_path(&path, Color::from_rgba8(0, 0, 0, 255), FillRule::Winding);
        }
        text::fill_text(
            canvas,
            &key.display_text,
            FONT_FAMILY,
            size as f32,
            center_x as f32,
            text_y as f32,
            Color::from_rgba8(255, 255, 255, (text_opacity * 255.0) as u8),
            text::Align::Center,
            text::Baseline::Middle,
        );

        x += width + GAP;
    }
    canvas.restore();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video::sidecars::KeyboardMeta;

    fn event(timestamp: f64, key: &str, modifiers: &[&str]) -> KeyboardKeyEvent {
        KeyboardKeyEvent {
            timestamp,
            key: key.to_string(),
            key_code: 0,
            modifiers: modifiers.iter().map(|value| value.to_string()).collect(),
            kind: "down".into(),
        }
    }

    fn segments() -> Vec<VideoSegment> {
        vec![VideoSegment {
            start_time: 0.0,
            end_time: 10.0,
            timeline_start: 0.0,
            speed: 1.0,
        }]
    }

    #[test]
    fn windows_combinations_read_as_plus_separated_words() {
        let event = event(0.0, "c", &["control", "shift"]);
        assert_eq!(format_key(&event, Some("windows")), "Ctrl+Shift+C");
    }

    #[test]
    fn mac_combinations_read_as_symbols() {
        let event = event(0.0, "c", &["command", "shift"]);
        // Modifiers are ordered control, option, shift, command.
        assert_eq!(format_key(&event, Some("macos")), "\u{21E7}\u{2318}C");
    }

    #[test]
    fn named_keys_use_their_symbol() {
        let event = event(0.0, "Return", &["command"]);
        assert_eq!(format_key(&event, Some("macos")), "\u{2318}\u{21A9}");
    }

    #[test]
    fn plain_typing_is_not_shown() {
        let data = KeyboardData {
            events: vec![event(0.0, "a", &[]), event(0.1, "b", &["shift"])],
            meta: KeyboardMeta::default(),
        };
        let style = KeyboardStyle {
            visible: true,
            ..KeyboardStyle::default()
        };
        assert!(active_keys(&data, &segments(), 0.1, &style).is_empty());
    }

    #[test]
    fn only_the_last_three_combinations_are_shown() {
        let data = KeyboardData {
            events: (0..6)
                .map(|index| event(index as f64 * 0.05, "a", &["control"]))
                .collect(),
            meta: KeyboardMeta::default(),
        };
        let style = KeyboardStyle {
            visible: true,
            display_duration: 5.0,
            ..KeyboardStyle::default()
        };
        let keys = active_keys(&data, &segments(), 0.3, &style);
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn a_combination_expires_after_the_display_duration() {
        let data = KeyboardData {
            events: vec![event(0.0, "a", &["control"])],
            meta: KeyboardMeta::default(),
        };
        let style = KeyboardStyle {
            visible: true,
            display_duration: 1.0,
            ..KeyboardStyle::default()
        };
        assert_eq!(active_keys(&data, &segments(), 0.5, &style).len(), 1);
        assert!(active_keys(&data, &segments(), 1.5, &style).is_empty());
    }

    #[test]
    fn the_pills_lift_above_the_caption_box() {
        let mut canvas = Canvas::new(1920, 1080).expect("canvas");
        let data = KeyboardData {
            events: vec![event(0.0, "a", &["control"])],
            meta: KeyboardMeta {
                platform: Some("windows".into()),
                ..KeyboardMeta::default()
            },
        };
        let style = KeyboardStyle {
            visible: true,
            font_size: "small".into(),
            display_duration: 2.0,
            ..KeyboardStyle::default()
        };
        render(
            &mut canvas,
            0.1,
            &RenderConfig {
                keyboard_data: &data,
                keyboard_style: &style,
                segments: &segments(),
                video_width: 1920.0,
                video_height: 1080.0,
                subtitle_bounds: Some(Bounds {
                    y: 800.0,
                    height: 200.0,
                }),
            },
        );
        let row_has_pixels = |y: u32| {
            (0..1920).any(|x| {
                let index = ((y * 1920 + x) * 4 + 3) as usize;
                canvas.pixmap().data()[index] > 0
            })
        };
        assert!(row_has_pixels(700));
        assert!(!row_has_pixels(900));
    }
}
