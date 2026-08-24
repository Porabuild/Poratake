use std::rc::Rc;

use gpui::{App, SharedString, Window};

use crate::ui::colors::Tool;

pub const THICKNESS_OPTIONS: [(f64, f32); 5] = [
    (1.0, 2.0),
    (3.0, 4.0),
    (8.0, 6.0),
    (13.0, 8.0),
    (21.0, 10.0),
];

pub const ARROW_STYLES: [(&str, &str); 4] = [
    ("standard", "Standard"),
    ("curved", "Curved"),
    ("double", "Double"),
    ("double-curved", "Double Curved"),
];

pub const HIGHLIGHT_OPACITIES: [f64; 5] = [0.2, 0.3, 0.4, 0.5, 0.6];

pub const NUMBER_STYLES: [(&str, &str); 4] = [
    ("numeric", "1, 2, 3, 4 ..."),
    ("alpha-upper", "A, B, C, D ..."),
    ("roman", "I, II, III, IV ..."),
    ("alpha-lower", "a, b, c, d ..."),
];

pub const NUMBER_SIZES: [(&str, &str); 3] =
    [("small", "Small"), ("medium", "Medium"), ("large", "Large")];

pub const NUMBER_START_VALUES: [f64; 10] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];

pub const REDACT_STYLES: [(&str, &str, &str); 3] = [
    ("pixelate", "Pixelate", "grid-3x3"),
    ("blur", "Blur", "droplets"),
    ("blackout", "Black Out", "square"),
];

pub const REDACT_INTENSITIES: [f64; 10] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];

pub const SHAPE_FILL_MODES: [(&str, &str); 2] = [("outline", "Outline"), ("filled", "Filled")];

pub const FONT_SIZES: [f64; 11] = [
    12.0, 16.0, 20.0, 28.0, 32.0, 40.0, 48.0, 64.0, 72.0, 84.0, 92.0,
];

pub const FONT_FAMILIES: [(&str, &str); 3] =
    [("serif", "Serif"), ("mono", "Mono"), ("comic", "Comic")];

pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 4.0;
pub const ZOOM_STEP: f32 = 0.1;

#[derive(Clone, Debug, PartialEq)]
pub enum EditorOption {
    Tool(Tool),
    Color(SharedString),
    StrokeWidth(f64),
    ArrowStyle(SharedString),
    HighlightOpacity(f64),
    HighlightColor(SharedString),
    NumberStyle(SharedString),
    NumberSize(SharedString),
    NumberStartValue(f64),
    TextBackground(bool),
    TextFontSize(f64),
    TextFontFamily(SharedString),
    RedactStyle(SharedString),
    RedactIntensity(f64),
    ShapeFillMode(SharedString),
    WallpaperGradient(SharedString),
    WallpaperPadding(f64),
    WallpaperCorners(f64),
    WallpaperShadow(f64),
    WallpaperAspectRatio(SharedString),
    WallpaperFrame(SharedString),
    WallpaperBalance(bool),
    WallpaperBlur(f64),
    WallpaperNoise(f64),
    WallpaperInset(f64),
    /// The gap between the capture and the images attached to its edges.
    WallpaperSpacing(f64),
    /// Removes every attached image.
    ClearAttachedImages,
    /// Loads the desktop wallpaper through the daemon.
    WallpaperUseDesktop,
    /// Opens the image picker.
    WallpaperPickImage,
    /// Clears both the gradient and the background image.
    WallpaperClear,
    WallpaperCustom(SharedString),
    WallpaperDeleteCustom(SharedString),
    WallpaperApplyPreset(SharedString),
    WallpaperSavePreset,
    WallpaperDeletePreset,
    WallpaperToggleDefaultPreset,
    Zoom(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorAction {
    CaptureToggle,
    Undo,
    Redo,
    Copy,
    Save,
    CloudUpload,
    Pin,
}

pub type OptionHandler = Rc<dyn Fn(EditorOption, &mut Window, &mut App)>;
pub type ActionHandler = Rc<dyn Fn(EditorAction, &mut Window, &mut App)>;

#[derive(Clone)]
pub struct EditorHandlers {
    pub on_option: OptionHandler,
    pub on_action: ActionHandler,
}

impl EditorHandlers {
    pub fn option(&self, value: EditorOption) -> impl Fn(&mut Window, &mut App) + 'static {
        let handler = self.on_option.clone();
        move |window, cx| handler(value.clone(), window, cx)
    }

    pub fn action(&self, value: EditorAction) -> impl Fn(&mut Window, &mut App) + 'static {
        let handler = self.on_action.clone();
        move |window, cx| handler(value, window, cx)
    }
}

pub fn thickness_bar_height(stroke_width: f64) -> f32 {
    THICKNESS_OPTIONS
        .iter()
        .find(|(width, _)| (width - stroke_width).abs() < 0.01)
        .map(|(_, height)| *height)
        .unwrap_or(4.0)
}

pub fn label_for(options: &[(&'static str, &'static str)], value: &str) -> &'static str {
    options
        .iter()
        .find(|(candidate, _)| *candidate == value)
        .map(|(_, label)| *label)
        .unwrap_or("")
}

pub fn number_display_value(value: f64, style: &str) -> String {
    let value = value.max(1.0).round() as u32;
    match style {
        "roman" => to_roman(value),
        "alpha-upper" => to_alpha(value, true),
        "alpha-lower" => to_alpha(value, false),
        _ => value.to_string(),
    }
}

fn to_roman(mut value: u32) -> String {
    const NUMERALS: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut result = String::new();
    for (amount, numeral) in NUMERALS {
        while value >= amount {
            result.push_str(numeral);
            value -= amount;
        }
    }
    result
}

fn to_alpha(mut value: u32, uppercase: bool) -> String {
    let mut letters = Vec::new();
    while value > 0 {
        value -= 1;
        letters.push((b'A' + (value % 26) as u8) as char);
        value /= 26;
    }
    letters.reverse();
    let result: String = letters.into_iter().collect();
    if uppercase {
        result
    } else {
        result.to_ascii_lowercase()
    }
}

pub fn number_preview_glyph(style: &str) -> &'static str {
    match style {
        "alpha-upper" => "A",
        "roman" => "I",
        "alpha-lower" => "a",
        _ => "1",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_number_display_values_like_the_renderer() {
        assert_eq!(number_display_value(4.0, "numeric"), "4");
        assert_eq!(number_display_value(4.0, "roman"), "IV");
        assert_eq!(number_display_value(1.0, "alpha-upper"), "A");
        assert_eq!(number_display_value(27.0, "alpha-upper"), "AA");
        assert_eq!(number_display_value(2.0, "alpha-lower"), "b");
        assert_eq!(number_display_value(2024.0, "roman"), "MMXXIV");
    }

    #[test]
    fn maps_stroke_widths_to_preview_heights() {
        assert_eq!(thickness_bar_height(1.0), 2.0);
        assert_eq!(thickness_bar_height(21.0), 10.0);
        assert_eq!(thickness_bar_height(7.5), 4.0);
    }

    #[test]
    fn resolves_labels_from_option_tables() {
        assert_eq!(label_for(&ARROW_STYLES, "double-curved"), "Double Curved");
        assert_eq!(label_for(&NUMBER_SIZES, "large"), "Large");
        assert_eq!(label_for(&SHAPE_FILL_MODES, "nope"), "");
    }
}
