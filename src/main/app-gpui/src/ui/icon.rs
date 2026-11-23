//! Lucide icon rendering. Icons are stroked (not filled) SVGs, so they are
//! painted through `canvas()` + `PathBuilder::stroke` with round caps and
//! joins — the same look `lucide-react` produces in the Electron renderer.

use gpui::{canvas, div, prelude::*, px, App, Pixels, RenderOnce, Window};
use herogpui::gpui;

mod data {
    include!("icons_data.rs");
}

#[allow(dead_code)]
pub const ICON_SM: f32 = 14.0;
pub const ICON_MD: f32 = 16.0;
#[allow(dead_code)]
pub const ICON_LG: f32 = 20.0;

const STROKE_WIDTH: f32 = 2.0;
const SPIN_MS: u64 = 1000;

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
        let stroke_width = self.stroke_width.unwrap_or(STROKE_WIDTH);
        let rotation = self.rotation;

        canvas(
            move |bounds, _, _| {
                let _ = bounds;
                #[cfg(feature = "parity")]
                for (name, icons) in [
                    ("camera-icon", ["video", "video-off"]),
                    ("microphone-icon", ["mic", "mic-off"]),
                    ("system-audio-icon", ["volume-2", "volume-x"]),
                    ("ios-device-icon", ["smartphone", "smartphone"]),
                ] {
                    if icons.iter().any(|icon| Some(path_data) == icon_data(icon)) {
                        crate::ui::parity::record(name, vec![bounds]);
                    }
                }
            },
            move |bounds, _: (), window, cx| {
                if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
                    return;
                }
                let transformation = if rotation == 0.0 {
                    gpui::TransformationMatrix::unit()
                } else {
                    let scale_factor = window.scale_factor();
                    let center = bounds.center();
                    gpui::TransformationMatrix::unit()
                        .translate(center.scale(scale_factor))
                        .rotate(gpui::radians(rotation * std::f32::consts::TAU))
                        .translate(center.scale(-scale_factor))
                };
                let (key, svg) = cached_icon_svg(path_data, stroke_width);
                let _ = window.paint_svg(
                    bounds,
                    key,
                    Some(svg.as_ref()),
                    transformation,
                    window.text_style().color,
                    cx,
                );
            },
        )
        .w(side)
        .h(side)
    }
}

fn cached_icon_svg(
    path_data: &'static str,
    stroke_width: f32,
) -> (gpui::SharedString, std::sync::Arc<[u8]>) {
    use std::collections::HashMap;
    use std::sync::{Arc, OnceLock};

    static CACHE: OnceLock<
        parking_lot::Mutex<HashMap<(usize, u32), (gpui::SharedString, Arc<[u8]>)>>,
    > = OnceLock::new();

    let cache_key = (path_data.as_ptr() as usize, stroke_width.to_bits());
    let mut cache = CACHE
        .get_or_init(|| parking_lot::Mutex::new(HashMap::new()))
        .lock();
    if let Some((key, bytes)) = cache.get(&cache_key) {
        return (key.clone(), Arc::clone(bytes));
    }
    let key = icon_svg_key(path_data, stroke_width);
    let bytes: Arc<[u8]> = Arc::from(icon_svg_document(path_data, stroke_width).into_bytes());
    cache.insert(cache_key, (key.clone(), Arc::clone(&bytes)));
    (key, bytes)
}

fn icon_svg_key(path_data: &str, stroke_width: f32) -> gpui::SharedString {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path_data.hash(&mut hasher);
    stroke_width.to_bits().hash(&mut hasher);
    format!(
        "poratake-icon:{:016x}:{:08x}",
        hasher.finish(),
        stroke_width.to_bits()
    )
    .into()
}

fn icon_svg_document(path_data: &str, stroke_width: f32) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="{stroke_width}" stroke-linecap="round" stroke-linejoin="round"><path d="{path_data}"/></svg>"#
    )
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
    Loader {
        id: id.into(),
        size,
    }
    .into_any_element()
}

#[derive(IntoElement)]
struct Loader {
    id: gpui::ElementId,
    size: Pixels,
}

impl RenderOnce for Loader {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        herogpui::components::Spinner::new(self.id)
            .size_px(self.size)
            .duration_ms(SPIN_MS)
            .current_color(window.text_style().color)
            .glyph(
                gpui::svg()
                    .size(self.size)
                    .flex_shrink_0()
                    .path(herogpui::components::icons::LOADER_2),
            )
    }
}

/// The select / popover indicator: `chevron-down`, carrying the renderer's
/// `rotate-180` while the menu is open. Opening animates a half turn; closing
/// snaps back so a first paint of a closed chevron does not spin from 180.
pub fn chevron_element(size: Pixels, open: bool) -> gpui::AnyElement {
    use gpui::AnimationExt;
    use herogpui::gpui;

    match Icon::with_size("chevron-down", size) {
        Some(element) if open => div()
            .flex()
            .items_center()
            .justify_center()
            .child(
                element.with_animation(
                    gpui::ElementId::Name("chevron-open".into()),
                    gpui::Animation::new(std::time::Duration::from_millis(
                        crate::ui::primitives::OVERLAY_ENTER_MS,
                    ))
                    .with_easing(crate::ui::primitives::ease_out()),
                    |icon, delta| icon.rotate_turns(0.5 * delta),
                ),
            )
            .into_any_element(),
        Some(element) => div()
            .flex()
            .items_center()
            .justify_center()
            .child(element.rotate_180(false))
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

    #[test]
    fn every_icon_produces_a_valid_svg_document() {
        let options = usvg::Options::default();
        for (name, path_data) in data::ICONS {
            let document = icon_svg_document(path_data, STROKE_WIDTH);
            let tree = usvg::Tree::from_str(&document, &options)
                .unwrap_or_else(|error| panic!("invalid SVG for {name}: {error}"));
            assert!(
                !tree.root().children().is_empty(),
                "empty SVG tree for {name}"
            );
        }
    }
}
