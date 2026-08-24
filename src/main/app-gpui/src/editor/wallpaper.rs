//! Wallpaper backdrop — port of `types/editor.ts` `WallpaperSettings` and the
//! geometry in `renderer/utils/wallpaper-render.ts`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GradientOption {
    pub id: String,
    #[serde(default)]
    pub colors: Vec<String>,
    #[serde(default)]
    pub angle: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowFrameSettings {
    #[serde(default = "frame_none")]
    pub style: String,
}

fn frame_none() -> String {
    "none".to_string()
}

impl Default for WindowFrameSettings {
    fn default() -> Self {
        Self {
            style: frame_none(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperSettings {
    #[serde(default)]
    pub gradient: Option<GradientOption>,
    #[serde(default)]
    pub background_image: Option<String>,
    #[serde(default)]
    pub background_blur: f64,
    #[serde(default)]
    pub noise: f64,
    #[serde(default)]
    pub padding: f64,
    #[serde(default)]
    pub inset: f64,
    #[serde(default)]
    pub corners: f64,
    #[serde(default)]
    pub shadow: f64,
    #[serde(default)]
    pub spacing: f64,
    #[serde(default)]
    pub window_frame: WindowFrameSettings,
    #[serde(default)]
    pub balance: bool,
    #[serde(default = "aspect_auto")]
    pub aspect_ratio: String,
}

fn aspect_auto() -> String {
    "auto".to_string()
}

impl Default for WallpaperSettings {
    fn default() -> Self {
        Self {
            gradient: None,
            background_image: None,
            background_blur: 0.0,
            noise: 0.0,
            padding: 0.0,
            inset: 0.0,
            corners: 0.0,
            shadow: 0.0,
            spacing: 0.0,
            window_frame: WindowFrameSettings::default(),
            balance: false,
            aspect_ratio: aspect_auto(),
        }
    }
}

impl WallpaperSettings {
    /// A capture with attached layers is framed as one group, so the wallpaper
    /// is active even without a background or padding.
    pub fn is_active_with_layers(&self, has_layers: bool) -> bool {
        self.is_active() || has_layers
    }

    pub fn is_active(&self) -> bool {
        self.gradient.is_some() || self.background_image.is_some() || self.padding > 0.0
    }

    /// Selecting a gradient with no padding yet opens the frame up, matching
    /// `setGradient` in `useWallpaperState.ts`.
    pub fn set_gradient(&mut self, gradient: Option<GradientOption>) {
        if gradient.is_some() && self.padding == 0.0 {
            self.padding = 50.0;
        }
        self.background_image = None;
        self.gradient = gradient;
    }

    /// The image counterpart of `set_gradient`: the two backgrounds are
    /// exclusive, and picking one opens the frame up the same way.
    pub fn set_background_image(&mut self, source: Option<String>) {
        if source.is_some() && self.padding == 0.0 {
            self.padding = 50.0;
        }
        self.gradient = None;
        self.background_image = source;
    }

    /// Whether a background is set at all, which gates the blur and noise
    /// controls the way the renderer's panel does.
    pub fn has_background(&self) -> bool {
        self.gradient.is_some() || self.background_image.is_some()
    }
}

pub const BLUR_MAX: f64 = 100.0;
pub const NOISE_MAX: f64 = 100.0;
pub const PADDING_MAX: f64 = 300.0;
pub const INSET_MAX: f64 = 200.0;
pub const CORNERS_MAX: f64 = 200.0;
pub const SHADOW_MAX: f64 = 300.0;
pub const SPACING_MAX: f64 = 200.0;
pub const VIDEO_PADDING_MAX: f64 = 300.0;
pub const VIDEO_CORNERS_MAX: f64 = 100.0;
pub const VIDEO_SHADOW_MAX: f64 = 300.0;

pub const ASPECT_RATIOS: [(&str, &str); 10] = [
    ("auto", "Auto"),
    ("1:1", "1:1"),
    ("4:3", "4:3"),
    ("3:2", "3:2"),
    ("16:9", "16:9"),
    ("16:10", "16:10"),
    ("21:9", "21:9"),
    ("9:16", "9:16"),
    ("3:4", "3:4"),
    ("2:3", "2:3"),
];

pub const VIDEO_ASPECT_RATIOS: [(&str, f64, f64); 8] = [
    ("Auto", 0.0, 0.0),
    ("16:9", 16.0, 9.0),
    ("9:16", 9.0, 16.0),
    ("4:3", 4.0, 3.0),
    ("1:1", 1.0, 1.0),
    ("21:9", 21.0, 9.0),
    ("4:5", 4.0, 5.0),
    ("3:2", 3.0, 2.0),
];

pub const WINDOW_FRAMES: [(&str, &str); 5] = [
    ("none", "None"),
    ("macos-light", "macOS Light"),
    ("macos-dark", "macOS Dark"),
    ("windows-light", "Windows Light"),
    ("windows-dark", "Windows Dark"),
];

/// The renderer's built-in gradient presets.
pub const GRADIENT_PRESETS: [(&str, [&str; 2], f64); 8] = [
    ("sunset", ["#f97316", "#db2777"], 135.0),
    ("ocean", ["#0ea5e9", "#2563eb"], 135.0),
    ("forest", ["#22c55e", "#0f766e"], 135.0),
    ("grape", ["#a855f7", "#4f46e5"], 135.0),
    ("ember", ["#f43f5e", "#f59e0b"], 135.0),
    ("slate", ["#64748b", "#1e293b"], 135.0),
    ("mint", ["#34d399", "#06b6d4"], 135.0),
    ("dusk", ["#6366f1", "#0f172a"], 135.0),
];

pub const SVG_PRESETS: [(&str, &str, [&str; 2], f64); 14] = [
    (
        "crimson-wave",
        "Crimson Wave",
        ["#7f1d1d", "#fca5a5"],
        135.0,
    ),
    ("forest-glow", "Forest Glow", ["#4ade80", "#052e16"], 225.0),
    ("violet-dune", "Violet Dune", ["#1e1b4b", "#f472b6"], 135.0),
    ("ocean-depth", "Ocean Depth", ["#0c4a6e", "#0ea5e9"], 180.0),
    ("rose-garden", "Rose Garden", ["#fdf2f8", "#831843"], 180.0),
    ("amber-ridge", "Amber Ridge", ["#0f172a", "#fde68a"], 45.0),
    ("mint-frost", "Mint Frost", ["#f0fdfa", "#0f766e"], 135.0),
    (
        "electric-kite",
        "Electric Kite",
        ["#0f172a", "#22d3ee"],
        135.0,
    ),
    (
        "slate-minimal",
        "Slate Minimal",
        ["#f8fafc", "#1e293b"],
        135.0,
    ),
    (
        "nebula-threads",
        "Nebula Threads",
        ["#1d4ed8", "#020617"],
        225.0,
    ),
    ("golden-hour", "Golden Hour", ["#1c1917", "#fef3c7"], 0.0),
    (
        "lavender-mist",
        "Lavender Mist",
        ["#f5f3ff", "#4c1d95"],
        225.0,
    ),
    ("terra-mosaic", "Terra Mosaic", ["#fef3c7", "#1f2937"], 90.0),
    (
        "arctic-aurora",
        "Arctic Aurora",
        ["#0f172a", "#0c4a6e"],
        180.0,
    ),
];

pub const FRAME_THEMES: [(&str, &str, &str, &str, &str, &str); 4] = [
    (
        "macos-light",
        "#E8E8E8",
        "#D1D1D1",
        "#FFFFFF",
        "#A8A8A8",
        "#262626",
    ),
    (
        "macos-dark",
        "#3A3A3C",
        "#2A2A2C",
        "#1C1C1E",
        "#606064",
        "#F5F5F5",
    ),
    (
        "windows-light",
        "#F3F3F3",
        "#D6D6D6",
        "#FFFFFF",
        "#8A8A8A",
        "#1A1A1A",
    ),
    (
        "windows-dark",
        "#202020",
        "#3A3A3A",
        "#121212",
        "#707070",
        "#FFFFFF",
    ),
];

pub fn preset(id: &str) -> Option<GradientOption> {
    if let Some(gradient) = GRADIENT_PRESETS
        .iter()
        .find(|(preset_id, _, _)| *preset_id == id)
        .map(|(preset_id, colors, angle)| GradientOption {
            id: (*preset_id).to_string(),
            colors: colors.iter().map(|color| (*color).to_string()).collect(),
            angle: *angle,
        })
    {
        return Some(gradient);
    }
    SVG_PRESETS
        .iter()
        .find(|(preset_id, _, _, _)| *preset_id == id)
        .map(|(preset_id, _, colors, angle)| GradientOption {
            id: (*preset_id).to_string(),
            colors: colors.iter().map(|color| (*color).to_string()).collect(),
            angle: *angle,
        })
}

pub fn apply_preset(
    settings: &mut WallpaperSettings,
    preset: &crate::config::schema::WallpaperPreset,
) {
    settings.gradient = preset.gradient.as_ref().map(|gradient| GradientOption {
        id: gradient.id.clone(),
        colors: gradient.colors.clone(),
        angle: gradient.angle,
    });
    settings.background_image = preset.background_image.clone();
    settings.background_blur = preset.background_blur.unwrap_or(0.0);
    settings.noise = preset.noise.unwrap_or(0.0);
    settings.padding = preset.padding;
    settings.corners = preset.corners;
    settings.shadow = preset.shadow;
    if let Some(spacing) = preset.spacing {
        settings.spacing = spacing;
    }
    settings.window_frame.style = preset
        .window_frame
        .as_ref()
        .map(|frame| frame.style.clone())
        .unwrap_or_else(|| "none".to_string());
}

pub fn to_schema_preset(
    settings: &WallpaperSettings,
    id: String,
    name: String,
) -> crate::config::schema::WallpaperPreset {
    crate::config::schema::WallpaperPreset {
        id,
        name,
        gradient: settings.gradient.as_ref().map(|gradient| {
            crate::config::schema::GradientOption {
                id: gradient.id.clone(),
                colors: gradient.colors.clone(),
                angle: gradient.angle,
            }
        }),
        background_image: settings.background_image.clone(),
        background_blur: Some(settings.background_blur),
        noise: Some(settings.noise),
        padding: settings.padding,
        corners: settings.corners,
        shadow: settings.shadow,
        spacing: Some(settings.spacing),
        window_frame: Some(crate::config::schema::WindowFrameSettings {
            style: settings.window_frame.style.clone(),
        }),
    }
}

pub fn parse_aspect_ratio(value: &str) -> Option<(f64, f64)> {
    let (width, height) = value.split_once(':')?;
    let width: f64 = width.trim().parse().ok()?;
    let height: f64 = height.trim().parse().ok()?;
    (width > 0.0 && height > 0.0).then_some((width, height))
}

/// The canvas size and the inner image rect for a wallpaper composition. A
/// window frame adds its title bar above the image, so the frame is part of the
/// content the padding surrounds.
pub fn layout(
    settings: &WallpaperSettings,
    image_width: f64,
    image_height: f64,
) -> ((f64, f64), (f64, f64, f64, f64)) {
    let padding = settings.padding.max(0.0);
    let title_bar =
        crate::render::window_frame::title_bar_height(&settings.window_frame.style, 1.0);
    let content_height = image_height + title_bar;
    let mut canvas_width = image_width + padding * 2.0;
    let mut canvas_height = content_height + padding * 2.0;

    if let Some((ratio_w, ratio_h)) = parse_aspect_ratio(&settings.aspect_ratio) {
        let target = ratio_w / ratio_h;
        let current = canvas_width / canvas_height.max(1.0);
        if current < target {
            canvas_width = canvas_height * target;
        } else {
            canvas_height = canvas_width / target;
        }
    }

    let x = (canvas_width - image_width) / 2.0;
    let y = if settings.balance {
        (canvas_height - content_height) / 2.0
    } else {
        ((canvas_height - content_height) / 2.0).min(padding)
    };

    (
        (canvas_width, canvas_height),
        (x, y, image_width, content_height),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_opens_up_when_a_gradient_is_picked() {
        let mut settings = WallpaperSettings::default();
        settings.set_gradient(preset("ocean"));
        assert_eq!(settings.padding, 50.0);
        assert!(settings.gradient.is_some());

        settings.padding = 12.0;
        settings.set_gradient(preset("sunset"));
        assert_eq!(settings.padding, 12.0);
    }

    #[test]
    fn picking_an_image_clears_the_gradient_and_opens_the_frame() {
        let mut settings = WallpaperSettings::default();
        settings.set_gradient(preset("ocean"));
        settings.padding = 0.0;
        settings.set_background_image(Some("C:/wall.png".into()));
        assert!(settings.gradient.is_none());
        assert_eq!(settings.background_image.as_deref(), Some("C:/wall.png"));
        assert_eq!(settings.padding, 50.0);
        assert!(settings.has_background());

        settings.set_background_image(None);
        assert!(!settings.has_background());
    }

    #[test]
    fn parses_aspect_ratios() {
        assert_eq!(parse_aspect_ratio("16:9"), Some((16.0, 9.0)));
        assert_eq!(parse_aspect_ratio("auto"), None);
        assert_eq!(parse_aspect_ratio("0:1"), None);
    }

    #[test]
    fn auto_layout_is_the_image_plus_padding() {
        let mut settings = WallpaperSettings::default();
        settings.padding = 40.0;
        let ((width, height), (x, y, w, h)) = layout(&settings, 200.0, 100.0);
        assert_eq!((width, height), (280.0, 180.0));
        assert_eq!((x, y, w, h), (40.0, 40.0, 200.0, 100.0));
    }

    #[test]
    fn a_window_frame_adds_its_title_bar_to_the_content() {
        let mut settings = WallpaperSettings::default();
        settings.padding = 10.0;
        settings.window_frame.style = "macos-light".into();
        let ((width, height), (_, _, image_width, content_height)) =
            layout(&settings, 200.0, 100.0);
        assert_eq!((width, height), (220.0, 148.0));
        assert_eq!(image_width, 200.0);
        assert_eq!(content_height, 128.0);
    }

    #[test]
    fn a_fixed_ratio_widens_the_canvas() {
        let mut settings = WallpaperSettings::default();
        settings.padding = 10.0;
        settings.aspect_ratio = "16:9".into();
        let ((width, height), _) = layout(&settings, 100.0, 100.0);
        assert!(width > height);
        assert!((width / height - 16.0 / 9.0).abs() < 0.001);
    }

    #[test]
    fn screenshot_sheet_matches_electron() {
        use crate::ui::chrome;
        assert_eq!(chrome::WALLPAPER_SHEET_WIDTH, 320.0);
        assert_eq!(chrome::WALLPAPER_SHEET_PAD, 20.0);
        assert_eq!(chrome::WALLPAPER_SHEET_GAP, 16.0);
        assert_eq!(chrome::WALLPAPER_SHEET_INNER_GAP, 24.0);
        assert_eq!(chrome::WALLPAPER_SECTION_GAP, 12.0);
        assert_eq!(chrome::WALLPAPER_GRID_COLS, 5);
        assert_eq!(chrome::WALLPAPER_GRID_GAP, 8.0);
        assert_eq!(chrome::WALLPAPER_FRAME_COLS, 3);
        assert_eq!(chrome::WALLPAPER_TILE_RADIUS, chrome::RADIUS_LG);
        assert_eq!(chrome::WALLPAPER_SELECT_WIDTH, 96.0);
        assert_eq!(PADDING_MAX, 300.0);
        assert_eq!(INSET_MAX, 200.0);
        assert_eq!(CORNERS_MAX, 200.0);
        assert_eq!(SHADOW_MAX, 300.0);
        assert_eq!(SPACING_MAX, 200.0);
        assert_eq!(BLUR_MAX, 100.0);
        assert_eq!(NOISE_MAX, 100.0);
        assert_eq!(ASPECT_RATIOS.len(), 10);
        assert_eq!(WINDOW_FRAMES.len(), 5);
        assert_eq!(SVG_PRESETS.len(), 14);
    }

    #[test]
    fn video_wallpaper_panel_matches_electron() {
        use crate::ui::chrome;
        assert_eq!(chrome::VIDEO_SIDEBAR_WIDTH, 288.0);
        assert_eq!(chrome::VIDEO_PANEL_PAD, 16.0);
        assert_eq!(chrome::VIDEO_PANEL_GAP, 16.0);
        assert_eq!(chrome::VIDEO_ASPECT_COLS, 4);
        assert_eq!(VIDEO_PADDING_MAX, 300.0);
        assert_eq!(VIDEO_CORNERS_MAX, 100.0);
        assert_eq!(VIDEO_SHADOW_MAX, 300.0);
        assert_eq!(VIDEO_ASPECT_RATIOS.len(), 8);
        assert_eq!(VIDEO_ASPECT_RATIOS[0], ("Auto", 0.0, 0.0));
    }

    #[test]
    fn svg_preset_lookup_opens_padding() {
        let mut settings = WallpaperSettings::default();
        settings.set_gradient(preset("crimson-wave"));
        assert_eq!(
            settings.gradient.as_ref().map(|g| g.id.as_str()),
            Some("crimson-wave")
        );
        assert_eq!(settings.padding, 50.0);
    }
}
