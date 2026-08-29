//! Lucide icon rendering. Icons are stroked (not filled) SVGs, so they are
//! painted through `canvas()` + `PathBuilder::stroke` with round caps and
//! joins — the same look `lucide-react` produces in the Electron renderer.

use gpui::{
    canvas, div, prelude::*, px, App, PathBuilder, Pixels, RenderOnce, StrokeOptions, Window,
};

use crate::ui::svg_path::{parse_path, PathCommand};

mod data {
    include!("icons_data.rs");
}

/// Icon size constants used across the design system (lucide defaults).
#[allow(dead_code)]
pub const ICON_SM: f32 = 14.0;
pub const ICON_MD: f32 = 16.0;
#[allow(dead_code)]
pub const ICON_LG: f32 = 20.0;

const VIEWBOX: f32 = 24.0;
const STROKE_WIDTH: f32 = 2.0;

fn icon_data(name: &str) -> Option<&'static str> {
    if let Some((_, path)) = data::ICONS.iter().find(|(id, _)| *id == name) {
        return Some(path);
    }

    let normalize = |value: &str| -> String {
        value
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .collect()
    };
    let key = normalize(name);
    data::ICONS
        .iter()
        .find(|(id, _)| normalize(id) == key)
        .map(|(_, value)| *value)
}

/// A stroked lucide icon element painted at `size` device pixels.
#[derive(IntoElement)]
pub struct Icon {
    path: &'static str,
    size: Pixels,
    stroke_width: Option<f32>,
    /// Clockwise rotation about the viewBox centre, in turns.
    rotation: f32,
}

pub fn icon(path: &'static str) -> Icon {
    Icon {
        path,
        size: px(ICON_MD),
        stroke_width: None,
        rotation: 0.0,
    }
}

impl Icon {
    /// Looks up a lucide icon by its kebab-case name; returns None for names
    /// outside the generated set.
    pub fn new(name: &str) -> Option<Icon> {
        icon_data(name).map(icon)
    }

    pub fn with_size(name: &str, size: Pixels) -> Option<Icon> {
        Icon::new(name).map(|icon| icon.size(size))
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    #[allow(dead_code)]
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = Some(width);
        self
    }

    /// Half turn about the viewBox centre — the `rotate-180` the renderer puts
    /// on open select indicators and popover chevrons.
    pub fn rotate_180(mut self, rotate: bool) -> Self {
        self.rotation = if rotate { 0.5 } else { 0.0 };
        self
    }

    /// Clockwise rotation about the viewBox centre, in turns, for
    /// `animate-spin`.
    pub fn rotate_turns(mut self, turns: f32) -> Self {
        self.rotation = turns;
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let path_data = self.path;
        let side = self.size;
        let scale = f32::from(self.size) / VIEWBOX;
        let stroke_width = self.stroke_width.unwrap_or(STROKE_WIDTH) * scale;
        let rotation = self.rotation;

        canvas(
            |_, _, _| {},
            move |bounds, _: (), window, _cx| {
                if let Some(path) =
                    build_stroked_path(path_data, bounds.origin, scale, stroke_width, rotation)
                {
                    window.paint_path(path, window.text_style().color);
                }
            },
        )
        .w(side)
        .h(side)
    }
}

fn build_stroked_path(
    path_data: &str,
    origin: gpui::Point<Pixels>,
    scale: f32,
    stroke_width: f32,
    rotation: f32,
) -> Option<gpui::Path<Pixels>> {
    let commands = parse_path(path_data);
    if commands.is_empty() {
        return None;
    }

    let mut builder = PathBuilder::stroke(px(stroke_width));
    if let gpui::PathStyle::Stroke(options) = &mut builder.style {
        let width = options.line_width;
        *options = StrokeOptions::default()
            .with_line_width(width)
            .with_line_cap(lyon::path::LineCap::Round)
            .with_line_join(lyon::path::LineJoin::Round);
    }

    let angle = rotation * std::f32::consts::TAU;
    let (sin, cos) = angle.sin_cos();
    let centre = VIEWBOX / 2.0;
    let at = |x: f32, y: f32| {
        let (x, y) = if rotation == 0.0 {
            (x, y)
        } else {
            let (dx, dy) = (x - centre, y - centre);
            (centre + dx * cos - dy * sin, centre + dx * sin + dy * cos)
        };
        gpui::point(origin.x + px(x * scale), origin.y + px(y * scale))
    };

    for command in commands {
        match command {
            PathCommand::MoveTo { x, y } => builder.move_to(at(x, y)),
            PathCommand::LineTo { x, y } => builder.line_to(at(x, y)),
            PathCommand::CubicTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                builder.cubic_bezier_to(at(x, y), at(x1, y1), at(x2, y2));
            }
            PathCommand::QuadTo { x1, y1, x, y } => builder.curve_to(at(x, y), at(x1, y1)),
            PathCommand::ArcTo {
                rx,
                ry,
                rotation,
                large_arc,
                sweep,
                x,
                y,
            } => builder.arc_to(
                gpui::point(px(rx * scale), px(ry * scale)),
                px(rotation),
                large_arc,
                sweep,
                at(x, y),
            ),
            PathCommand::Close => builder.close(),
        }
    }

    builder.build().ok()
}

/// Renders an icon by name centered in a box of its own size; falls back to an
/// empty element when the name is not in the generated set.
pub fn icon_element(name: &str, size: Pixels) -> gpui::AnyElement {
    match Icon::with_size(name, size) {
        Some(element) => div()
            .flex()
            .items_center()
            .justify_center()
            .child(element)
            .into_any_element(),
        None => div().into_any_element(),
    }
}

/// A `loader-2` glyph spinning once per second, matching Tailwind's
/// `animate-spin` (`1s linear infinite`) on the renderer's spinners.
pub fn spinner_element(id: impl Into<gpui::ElementId>, size: Pixels) -> gpui::AnyElement {
    use gpui::AnimationExt;

    let Some(glyph) = Icon::new("loader-2") else {
        return div().into_any_element();
    };
    div()
        .flex()
        .items_center()
        .justify_center()
        .child(glyph.size(size).with_animation(
            id.into(),
            gpui::Animation::new(std::time::Duration::from_secs(1)).repeat(),
            |icon, delta| icon.rotate_turns(delta),
        ))
        .into_any_element()
}

/// The select / popover indicator: `chevron-down`, carrying the renderer's
/// `rotate-180` while the menu is open.
pub fn chevron_element(size: Pixels, open: bool) -> gpui::AnyElement {
    match Icon::with_size("chevron-down", size) {
        Some(element) => div()
            .flex()
            .items_center()
            .justify_center()
            .child(element.rotate_180(open))
            .into_any_element(),
        None => div().into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every icon the UI asks for by name must exist in the generated set;
    /// a typo would otherwise render as an invisible empty box.
    #[test]
    fn every_referenced_icon_resolves() {
        const NAMES: &[&str] = &[
            "app-window",
            "arrow-up",
            "arrow-up-down",
            "camera",
            "check",
            "chevron-down",
            "chevron-right",
            "circle",
            "cloud-upload",
            "copy",
            "crop",
            "download",
            "droplets",
            "eraser",
            "eye-off",
            "film",
            "folder-open",
            "frame",
            "grid-3x3",
            "help-circle",
            "highlighter",
            "image",
            "image-off",
            "info",
            "keyboard",
            "layout-grid",
            "layout-list",
            "list-ordered",
            "loader-2",
            "maximize-2",
            "mic",
            "minus",
            "monitor",
            "mouse-pointer-2",
            "palette",
            "panel-right-close",
            "panel-right-open",
            "pause",
            "pen-line",
            "pencil",
            "pin",
            "play",
            "plus",
            "refresh-ccw",
            "rotate-ccw",
            "rotate-cw",
            "save",
            "scissors",
            "settings",
            "shuffle",
            "square",
            "square-dashed",
            "subtitles",
            "trash-2",
            "type",
            "video",
            "volume-2",
            "wallpaper",
            "x",
            "zoom-in",
            "aperture",
            "code-2",
            "globe",
            "hard-drive",
            "heart",
            "mic-off",
            "scale",
            "video-off",
            "volume-x",
            "webcam",
            "pipette",
            "scan-text",
            "square-dashed",
        ];
        let missing: Vec<&str> = NAMES
            .iter()
            .copied()
            .filter(|name| Icon::new(name).is_none())
            .collect();
        assert!(missing.is_empty(), "unknown icons: {missing:?}");
    }

    #[test]
    fn the_fast_lookup_still_falls_back_to_the_tolerant_one() {
        let exact = icon_data("trash-2");
        assert!(exact.is_some(), "the table no longer holds `trash-2`");
        assert_eq!(
            icon_data("trash2"),
            exact,
            "punctuation is no longer ignored"
        );
        assert_eq!(icon_data("TRASH-2"), exact, "case is no longer ignored");
        assert!(icon_data("definitely-not-an-icon").is_none());
    }
}
