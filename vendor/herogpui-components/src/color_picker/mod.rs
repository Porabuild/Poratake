//! Colors — port of `@heroui/color-area`, `color-field`, `color-picker`,
//! `color-slider`, `color-swatch` and `color-swatch-picker` (v3).
//!
//! All six components share the [`PickerColor`] value type and the
//! [`ColorChannel`] / [`ColorSpace`] vocabulary that React Aria uses.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use gpui::{
    div, prelude::*, px, Animation, AnimationExt, App, Bounds, ElementId, Entity, Hsla,
    InteractiveElement, IntoElement, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    RenderOnce, SharedString, Styled, Window,
};
#[allow(unused_imports)] // children import these through `use super::*`
use herogpui_core::{element_id, FieldVariant, Placement, SizeXl};
use herogpui_theme::ActiveTheme;

use crate::{
    a11y::{self, A11y as _},
    input::Input,
    util,
};

/// HeroUI's color surfaces use an 8px checker cell pair (a 16px repeating
/// tile) beneath translucent values. GPUI has no repeating-conic background,
/// so the shared color controls compose the same tile from clipped child
/// squares. Keeping this helper here makes swatches and alpha sliders use one
/// palette and one phase at every size and orientation.
pub(super) const CHECKER_CELL: f32 = 8.0;
pub(super) const CHECKER_LIGHT: u32 = 0xefefef;
pub(super) const CHECKER_DARK: u32 = 0xf7f7f7;

/// The dark half of the checkerboard as one clipped monochrome silhouette.
///
/// Vanilla GPUI clips `overflow_hidden()` to the rectangle, so the square
/// cells of [`transparency_checker`] bleed through rounded corners wherever
/// a translucent fill reveals them. GPUI's `Svg` paints through an alpha
/// mask (`Window::paint_svg` → `render_alpha_mask`), which rules out a
/// multicolor checker SVG -- but a single tint works: the light cells stay a
/// rounded div background (an element's own `bg` always follows its radius)
/// while the dark cells become one SVG under a `clipPath` curve, tinted dark
/// through `text_color`. Pair with a light rounded base -- a sketch of the
/// layering, not a compilable example: this helper is `pub(super)`, so
/// rustdoc never builds a fence here whatever the fence claims, and
/// `.shots/fence_audit.py` exists to stop a block claiming a check that does
/// not happen.
///
/// ```text
/// div().absolute().inset_0().rounded(radius).bg(LIGHT)
///     .child(transparency_checker_cells(width, height, radius, 1.0))
/// ```
///
/// A multicolor SVG (one document, two grays, one clip) renders as a single
/// silhouette instead -- verified against the pinned `gpui-pre` sources -- so the
/// two layers are structural, not stylistic.
pub(super) fn transparency_checker_cells(
    width: Pixels,
    height: Pixels,
    radius: Pixels,
    opacity: f32,
) -> gpui::Svg {
    use std::fmt::Write as _;
    const CELL: f32 = CHECKER_CELL;
    let w = f32::from(width);
    let h = f32::from(height);
    let r = f32::from(radius);
    let columns = (w / CELL).ceil().max(1.0) as usize;
    let rows = (h / CELL).ceil().max(1.0) as usize;
    // Same phase as [`transparency_checker`]: dark where row+column is odd.
    let mut cells = String::new();
    for row in 0..rows {
        for column in 0..columns {
            if (row + column) % 2 == 0 {
                continue;
            }
            let _ = write!(
                cells,
                "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{CELL:.2}\" height=\"{CELL:.2}\"/>",
                column as f32 * CELL,
                row as f32 * CELL,
            );
        }
    }
    let doc = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.2}\" height=\"{h:.2}\" \
         viewBox=\"0 0 {w:.2} {h:.2}\">\
         <defs><clipPath id=\"c\"><rect width=\"{w:.2}\" height=\"{h:.2}\" rx=\"{r:.2}\"/></clipPath></defs>\
         <g clip-path=\"url(#c)\" fill=\"#000000\">{cells}</g></svg>"
    );
    let bytes = doc.into_bytes();
    // Explicit size like every icon svg (zero bounds paint nothing), and the
    // dark tint: `Svg::data` only paints with a text color set, which becomes
    // the silhouette fill. An ancestor's `opacity()` does not reach the
    // silhouette (only quads inherit it), so a dimmed owner passes its
    // opacity here and the tint carries it instead.
    gpui::svg()
        .data(&bytes)
        .absolute()
        .top_0()
        .left_0()
        .w(width)
        .h(height)
        .text_color(gpui::rgb(CHECKER_DARK).alpha(opacity.clamp(0.0, 1.0)))
}

/// HeroUI's color surfaces use a one-pixel translucent inset edge rather than
/// a semantic theme border. GPUI's shadow primitive needs a small blur to
/// produce a visible raster edge, so the one-pixel spread is kept exact while
/// the blur stays at the smallest drawable value.
pub(super) fn color_inner_shadow() -> gpui::BoxShadow {
    gpui::BoxShadow {
        color: gpui::black().alpha(0.1),
        offset: gpui::point(px(0.), px(0.)),
        blur_radius: px(1.),
        spread_radius: px(1.),
        inset: true,
    }
}

/// The ColorArea/ColorSlider thumb depth treatment: a one-pixel outer hairline
/// plus the matching inset hairline inside the white ring.
pub(super) fn color_thumb_shadows() -> Vec<gpui::BoxShadow> {
    let edge = gpui::black().alpha(0.1);
    vec![
        gpui::BoxShadow {
            color: edge,
            offset: gpui::point(px(0.), px(0.)),
            blur_radius: px(1.),
            spread_radius: px(1.),
            inset: false,
        },
        gpui::BoxShadow {
            color: edge,
            offset: gpui::point(px(0.), px(0.)),
            blur_radius: px(1.),
            spread_radius: px(1.),
            inset: true,
        },
    ]
}

/// HeroUI shades the track's two long edges and each cap's outer edge with
/// unblurred one-pixel inset shadows. One complete capsule has all four edges.
pub(super) fn color_track_shadows() -> Vec<gpui::BoxShadow> {
    let edge = gpui::black().alpha(0.1);
    let offsets = [
        (px(1.), px(0.)),
        (px(-1.), px(0.)),
        (px(0.), px(1.)),
        (px(0.), px(-1.)),
    ];
    offsets
        .into_iter()
        .map(|(x, y)| gpui::BoxShadow {
            color: edge,
            offset: gpui::point(x, y),
            blur_radius: px(0.),
            spread_radius: px(0.),
            inset: true,
        })
        .collect()
}

/// Tailwind's placement-specific `slide-in-from-*` offsets for the color
/// picker popover. The floating engine may later flip a requested placement;
/// the caller records that remaining resolved-placement limitation in its
/// reference metadata while still matching every requested side.
pub(super) fn color_picker_entry_offset(placement: Placement) -> (f32, f32) {
    if placement.is_above() {
        (0.0, 4.0)
    } else if placement.is_side() {
        if placement.is_start_side() {
            (4.0, 0.0)
        } else {
            (-4.0, 0.0)
        }
    } else {
        (0.0, -4.0)
    }
}

/// HeroUI transitions a color thumb's focus-ring shadow over 150ms. GPUI's
/// shared `with_focus_ring` helper resolves the shadow list immediately, so
/// color controls keep a small keyed opacity tween for the focus layers while
/// preserving their component-specific depth shadows.
const COLOR_FOCUS_RING_TRANSITION_MS: u64 = 150;

#[derive(Clone)]
struct ColorFocusRingMotion {
    focused: bool,
    generation: usize,
    from: f32,
    opacity: Rc<Cell<f32>>,
}

pub(super) struct ColorFocusRingMotionFrame {
    base: ElementId,
    generation: usize,
    from: f32,
    to: f32,
    opacity: Rc<Cell<f32>>,
    animate: bool,
}

impl ColorFocusRingMotionFrame {
    fn render<T>(
        self,
        element: T,
        base_shadows: Vec<gpui::BoxShadow>,
        radius: Pixels,
        offset: bool,
        cx: &App,
    ) -> gpui::AnyElement
    where
        T: Styled + ParentElement + IntoElement + 'static,
    {
        // The ring is an overlay child, not a shadow: `focus_ring_shadows`
        // keeps the element's corner radius under the spread, so on the round
        // thumb and swatch it squared off. The element keeps only its own
        // (non-ring) shadows -- `shadow()` replaces the list, which is also
        // what drops any static ring an earlier pass painted.
        let element = element.shadow(base_shadows);
        let ring = util::focus_ring_overlay(radius, offset, cx);
        if !self.animate {
            self.opacity.set(self.to);
            return element.child(ring.opacity(self.to)).into_any_element();
        }

        let opacity = self.opacity;
        let from = self.from;
        let to = self.to;
        let ring = ring
            .with_animation(
                element_id::indexed(&self.base, "focus-ring", self.generation),
                Animation::new(Duration::from_millis(COLOR_FOCUS_RING_TRANSITION_MS))
                    .with_easing(crate::anim::ease_out()),
                move |ring, delta| {
                    let next = from + (to - from) * delta;
                    opacity.set(next);
                    ring.opacity(next)
                },
            )
            .into_any_element();
        element.child(ring).into_any_element()
    }
}

pub(super) fn color_focus_ring_motion(
    id: &ElementId,
    focused: bool,
    window: &mut Window,
    cx: &mut App,
) -> ColorFocusRingMotionFrame {
    let state = window.use_keyed_state(element_id::scoped(id, "focus-ring-motion"), cx, |_, _| {
        ColorFocusRingMotion {
            focused,
            generation: 0,
            from: if focused { 1.0 } else { 0.0 },
            opacity: Rc::new(Cell::new(if focused { 1.0 } else { 0.0 })),
        }
    });
    let mut current = state.read(cx).clone();
    let to = if focused { 1.0 } else { 0.0 };
    if current.focused != focused {
        current.focused = focused;
        current.generation = current.generation.wrapping_add(1);
        current.from = current.opacity.get();
        state.update(cx, |stored, _| *stored = current.clone());
    }
    let reduced_motion = ActiveTheme::reduce_motion(cx);
    if reduced_motion && (current.opacity.get() - to).abs() > f32::EPSILON {
        current.from = to;
        current.opacity.set(to);
        state.update(cx, |stored, _| *stored = current.clone());
    }
    ColorFocusRingMotionFrame {
        base: id.clone(),
        generation: current.generation,
        from: current.from,
        to,
        opacity: current.opacity,
        animate: current.generation != 0
            && !reduced_motion
            && (current.from - to).abs() > f32::EPSILON,
    }
}

// ---------------------------------------------------------------------------
// Value model
// ---------------------------------------------------------------------------

/// The color space a channel belongs to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorSpace {
    /// Hue, saturation and brightness.
    #[default]
    Hsb,
    /// Hue, saturation and lightness.
    Hsl,
    /// Red, green and blue.
    Rgb,
}

impl ColorSpace {
    /// Every color space, in display order.
    pub const ALL: [ColorSpace; 3] = [ColorSpace::Hsb, ColorSpace::Hsl, ColorSpace::Rgb];

    /// The uppercase name of this color space.
    pub fn label(self) -> &'static str {
        match self {
            ColorSpace::Hsb => "HSB",
            ColorSpace::Hsl => "HSL",
            ColorSpace::Rgb => "RGB",
        }
    }

    /// The two channels a colour area edits in this space.
    pub fn area_channels(self) -> (ColorChannel, ColorChannel) {
        match self {
            ColorSpace::Hsb => (ColorChannel::Saturation, ColorChannel::Brightness),
            ColorSpace::Hsl => (ColorChannel::Saturation, ColorChannel::Lightness),
            ColorSpace::Rgb => (ColorChannel::Red, ColorChannel::Green),
        }
    }
}

/// A single editable channel of a color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorChannel {
    /// Hue, in degrees.
    Hue,
    /// Saturation.
    Saturation,
    /// HSB value / "brightness".
    Brightness,
    /// HSL lightness.
    Lightness,
    /// Opacity.
    Alpha,
    /// Red component.
    Red,
    /// Green component.
    Green,
    /// Blue component.
    Blue,
}

impl ColorChannel {
    /// The human-readable name of this channel.
    pub fn label(self) -> &'static str {
        match self {
            ColorChannel::Hue => "Hue",
            ColorChannel::Saturation => "Saturation",
            ColorChannel::Brightness => "Brightness",
            ColorChannel::Lightness => "Lightness",
            ColorChannel::Alpha => "Alpha",
            ColorChannel::Red => "Red",
            ColorChannel::Green => "Green",
            ColorChannel::Blue => "Blue",
        }
    }

    /// The channel's name in the active chrome locale (React Stately's
    /// colour dictionary; [`ColorChannel::label`] is the en-US name).
    pub fn localized_label(self, cx: &App) -> SharedString {
        use crate::i18n::{ui_string, UiString};
        ui_string(
            match self {
                ColorChannel::Hue => UiString::Hue,
                ColorChannel::Saturation => UiString::Saturation,
                ColorChannel::Brightness => UiString::Brightness,
                ColorChannel::Lightness => UiString::Lightness,
                ColorChannel::Alpha => UiString::Alpha,
                ColorChannel::Red => UiString::Red,
                ColorChannel::Green => UiString::Green,
                ColorChannel::Blue => UiString::Blue,
            },
            cx,
        )
    }

    /// The inclusive value range of this channel.
    pub fn range(self) -> (f32, f32) {
        match self {
            ColorChannel::Hue => (0.0, 360.0),
            ColorChannel::Saturation
            | ColorChannel::Brightness
            | ColorChannel::Lightness
            | ColorChannel::Alpha => (0.0, 1.0),
            ColorChannel::Red | ColorChannel::Green | ColorChannel::Blue => (0.0, 255.0),
        }
    }
}

/// The color value shared by every picker component — HSB plus alpha, matching
/// React Aria's default working space.
///
/// HSB coordinates remain readable through dereferencing. Mutations go through
/// `with_channel_in`, which preserves the selected color model's channels even
/// at achromatic endpoints where converting through RGB/HSB would lose them.
#[derive(Clone, Copy, Debug)]
pub struct PickerColor {
    coordinates: HsbCoordinates,
    model: ColorModel,
}

/// The HSB representation of a `PickerColor`, with hue in degrees and the other components normalised to `0..1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HsbCoordinates {
    /// Hue in degrees, `0..360`.
    pub hue: f32,
    /// Saturation, `0..1`.
    pub saturation: f32,
    /// Brightness (HSB value), `0..1`.
    pub brightness: f32,
    /// Alpha, `0..1`.
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ColorModel {
    Hsb,
    Hsl { saturation: f32, lightness: f32 },
}

impl std::ops::Deref for PickerColor {
    type Target = HsbCoordinates;

    fn deref(&self) -> &Self::Target {
        &self.coordinates
    }
}

impl PartialEq for PickerColor {
    fn eq(&self, other: &Self) -> bool {
        self.coordinates == other.coordinates
    }
}

impl Default for PickerColor {
    fn default() -> Self {
        // React Aria's documented default working color.
        Self::hsb(210.0, 1.0, 1.0)
    }
}

impl PickerColor {
    /// Creates an opaque color from `hue` (wrapped to `0..360`), `saturation` and `brightness` (clamped to `0..1`).
    pub fn hsb(hue: f32, saturation: f32, brightness: f32) -> Self {
        Self {
            coordinates: HsbCoordinates {
                hue: normalize_hue(hue),
                saturation: saturation.clamp(0.0, 1.0),
                brightness: brightness.clamp(0.0, 1.0),
                alpha: 1.0,
            },
            model: ColorModel::Hsb,
        }
    }

    /// Returns the color with its alpha replaced, clamped to `0..1`.
    pub fn with_alpha(mut self, alpha: f32) -> Self {
        self.coordinates.alpha = alpha.clamp(0.0, 1.0);
        self
    }

    /// Parses `#rgb`, `#rrggbb` or `#rrggbbaa`.
    pub fn from_hex(text: &str) -> Option<Self> {
        let hex = text.trim().trim_start_matches('#');
        let expand = |c: char| {
            let d = c.to_digit(16)? as f32;
            Some(d * 17.0 / 255.0)
        };
        let (r, g, b, a) = match hex.len() {
            3 => {
                let mut it = hex.chars();
                (
                    expand(it.next()?)?,
                    expand(it.next()?)?,
                    expand(it.next()?)?,
                    1.0,
                )
            }
            6 | 8 => {
                let byte = |i: usize| {
                    u8::from_str_radix(hex.get(i..i + 2)?, 16)
                        .ok()
                        .map(|v| v as f32 / 255.0)
                };
                (
                    byte(0)?,
                    byte(2)?,
                    byte(4)?,
                    if hex.len() == 8 { byte(6)? } else { 1.0 },
                )
            }
            _ => return None,
        };
        Some(Self::from_rgb(r, g, b).with_alpha(a))
    }

    /// Builds a color from normalised sRGB components.
    // `max` is `r.max(g).max(b)`, so `max == r` asks which channel won, not
    // whether two computed floats are near each other. An epsilon here would
    // make two equally-large channels both match.
    #[allow(clippy::float_cmp)]
    pub fn from_rgb(r: f32, g: f32, b: f32) -> Self {
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        // `max` is by construction one of `r`/`g`/`b`, so these comparisons
        // select a branch rather than test a quantity; comparing within a
        // tolerance would pick the wrong one when two channels are merely close.
        #[allow(clippy::float_cmp)]
        let hue = if delta <= f32::EPSILON {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        Self {
            coordinates: HsbCoordinates {
                hue: normalize_hue(hue),
                saturation: if max <= f32::EPSILON {
                    0.0
                } else {
                    delta / max
                },
                brightness: max,
                alpha: 1.0,
            },
            model: ColorModel::Hsb,
        }
    }

    /// Normalised sRGB components.
    pub fn to_rgb(self) -> (f32, f32, f32) {
        let c = self.brightness * self.saturation;
        let h = self.hue / 60.0;
        let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
        let (r, g, b) = match h as u32 % 6 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = self.brightness - c;
        (r + m, g + m, b + m)
    }

    /// The gpui color for this value.
    pub fn to_hsla(self) -> Hsla {
        let (r, g, b) = self.to_rgb();
        Hsla::from(gpui::Rgba {
            r,
            g,
            b,
            a: self.alpha,
        })
    }

    /// `#rrggbb`, or `#rrggbbaa` when the color is translucent.
    pub fn to_hex(self) -> String {
        let (r, g, b) = self.to_rgb();
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        if self.alpha >= 1.0 {
            format!("#{:02X}{:02X}{:02X}", q(r), q(g), q(b))
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", q(r), q(g), q(b), q(self.alpha))
        }
    }

    /// Reads one channel in its own units.
    /// HSL saturation, which is a different quantity from the stored HSB
    /// saturation for every colour that is not fully saturated or achromatic.
    pub fn hsl_saturation(self) -> f32 {
        if let ColorModel::Hsl { saturation, .. } = self.model {
            return saturation;
        }
        let l = self.brightness * (1.0 - self.saturation / 2.0);
        let denom = l.min(1.0 - l);
        if denom <= f32::EPSILON {
            0.0
        } else {
            ((self.brightness - l) / denom).clamp(0.0, 1.0)
        }
    }

    /// Replaces the HSL saturation, holding hue and HSL lightness.
    pub fn with_hsl_saturation(self, s: f32) -> Self {
        let s = s.clamp(0.0, 1.0);
        let l = self.hsl_lightness();
        self.with_hsl_channels(s, l)
    }

    pub(super) fn with_hsl_lightness(self, l: f32) -> Self {
        self.with_hsl_channels(self.hsl_saturation(), l.clamp(0.0, 1.0))
    }

    pub(super) fn with_hsl_channels(self, s: f32, l: f32) -> Self {
        let v = l + s * l.min(1.0 - l);
        let sv = if v <= f32::EPSILON {
            0.0
        } else {
            (2.0 * (1.0 - l / v)).clamp(0.0, 1.0)
        };
        Self {
            coordinates: HsbCoordinates {
                saturation: sv,
                brightness: v,
                ..self.coordinates
            },
            model: ColorModel::Hsl {
                saturation: s,
                lightness: l,
            },
        }
    }

    pub(super) fn hsl_lightness(self) -> f32 {
        match self.model {
            ColorModel::Hsl { lightness, .. } => lightness,
            ColorModel::Hsb => self.brightness * (1.0 - self.saturation / 2.0),
        }
    }

    /// [`PickerColor::channel`] read in `space`.
    pub fn channel_in(self, channel: ColorChannel, space: ColorSpace) -> f32 {
        match (channel, space) {
            (ColorChannel::Saturation, ColorSpace::Hsl) => self.hsl_saturation(),
            (ColorChannel::Lightness, ColorSpace::Hsl) => self.hsl_lightness(),
            _ => self.channel(channel),
        }
    }

    /// [`PickerColor::with_channel`] written in `space`.
    pub fn with_channel_in(self, channel: ColorChannel, space: ColorSpace, value: f32) -> Self {
        match (channel, space) {
            (ColorChannel::Saturation, ColorSpace::Hsl) => self.with_hsl_saturation(value),
            (ColorChannel::Lightness, ColorSpace::Hsl) => self.with_hsl_lightness(value),
            (ColorChannel::Hue, ColorSpace::Hsl) => Self {
                coordinates: HsbCoordinates {
                    hue: normalize_hue(value),
                    ..self.coordinates
                },
                ..self
            },
            (ColorChannel::Alpha, ColorSpace::Hsl) => self.with_alpha(value),
            _ => self.with_channel(channel, value),
        }
    }

    /// The value of `channel`: hue in degrees, red/green/blue on `0..255`, the others on `0..1`.
    pub fn channel(self, channel: ColorChannel) -> f32 {
        let (r, g, b) = self.to_rgb();
        match channel {
            ColorChannel::Hue => self.hue,
            ColorChannel::Saturation => self.saturation,
            ColorChannel::Brightness => self.brightness,
            // HSB -> HSL lightness.
            ColorChannel::Lightness => self.hsl_lightness(),
            ColorChannel::Alpha => self.alpha,
            ColorChannel::Red => r * 255.0,
            ColorChannel::Green => g * 255.0,
            ColorChannel::Blue => b * 255.0,
        }
    }

    /// Returns a copy with one channel replaced.
    pub fn with_channel(self, channel: ColorChannel, value: f32) -> Self {
        let (min, max) = channel.range();
        let value = value.clamp(min, max);
        let (r, g, b) = self.to_rgb();
        match channel {
            ColorChannel::Hue => Self {
                coordinates: HsbCoordinates {
                    hue: normalize_hue(value),
                    ..self.coordinates
                },
                model: ColorModel::Hsb,
            },
            ColorChannel::Saturation => Self {
                coordinates: HsbCoordinates {
                    saturation: value,
                    ..self.coordinates
                },
                model: ColorModel::Hsb,
            },
            ColorChannel::Brightness => Self {
                coordinates: HsbCoordinates {
                    brightness: value,
                    ..self.coordinates
                },
                model: ColorModel::Hsb,
            },
            ColorChannel::Lightness => self.with_hsl_lightness(value),
            ColorChannel::Alpha => Self {
                coordinates: HsbCoordinates {
                    alpha: value,
                    ..self.coordinates
                },
                ..self
            },
            ColorChannel::Red => Self::from_rgb(value / 255.0, g, b).with_alpha(self.alpha),
            ColorChannel::Green => Self::from_rgb(r, value / 255.0, b).with_alpha(self.alpha),
            ColorChannel::Blue => Self::from_rgb(r, g, value / 255.0).with_alpha(self.alpha),
        }
    }
}

// React Stately preserves the exact endpoint 360 as a distinct slider
// position even though it renders the same color as 0.
#[allow(clippy::float_cmp)]
pub(super) fn normalize_hue(hue: f32) -> f32 {
    if hue == 360.0 {
        hue
    } else {
        hue.rem_euclid(360.0)
    }
}

pub(super) type OnColorChange = Arc<dyn Fn(&PickerColor, &mut Window, &mut App) + 'static>;

pub(super) fn color_swatch_indicator_color(swatch: PickerColor) -> Hsla {
    let (r, g, b) = swatch.to_rgb();
    let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if luminance > 0.5 {
        gpui::black()
    } else {
        gpui::white()
    }
}

/// `ColorField`'s `onChange`, which reports `None` when the text is not a
/// colour -- v3 types it `(color: Color | null) => void`.
pub(super) type OnColorFieldChange =
    Arc<dyn Fn(&Option<PickerColor>, &mut Window, &mut App) + 'static>;

/// Shape of a swatch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwatchShape {
    /// A circular swatch.
    #[default]
    Circle,
    /// A square swatch.
    Square,
}

impl SwatchShape {
    /// Both swatch shapes, in display order.
    pub const ALL: [SwatchShape; 2] = [SwatchShape::Circle, SwatchShape::Square];

    /// The human-readable name of this shape.
    pub fn label(self) -> &'static str {
        match self {
            SwatchShape::Circle => "Circle",
            SwatchShape::Square => "Square",
        }
    }
}

mod area;
mod field;
mod picker;
mod slider;
mod swatch;
mod swatch_picker;

pub use area::*;
#[allow(unused_imports)]
pub(super) use area::*;
pub use field::*;
#[allow(unused_imports)]
pub(super) use field::*;
pub use picker::*;
#[allow(unused_imports)]
pub(super) use picker::*;
pub use slider::*;
#[allow(unused_imports)]
pub(super) use slider::*;
pub use swatch::*;
#[allow(unused_imports)]
pub(super) use swatch::*;
pub use swatch_picker::*;
#[allow(unused_imports)]
pub(super) use swatch_picker::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Every colour-picker ring is the shared offset overlay: a spread shadow
    /// keeps the element's corner radius, so on the round slider thumb and
    /// swatches it squared the ring off. The motion frame animates the
    /// overlay child's opacity, and each caller passes its part's radius.
    #[test]
    fn color_focus_rings_use_the_shared_overlay_geometry() {
        let source = include_str!("mod.rs");
        let implementation = source
            .split("#[cfg(test)]")
            .next()
            .expect("the render implementation must precede this test module");
        assert!(implementation.contains("util::focus_ring_overlay(radius, offset, cx)"));
        assert!(
            !implementation.contains("util::focus_ring_shadows"),
            "the colour rings moved to the overlay; a shadow ring squares off round parts"
        );

        let swatch = include_str!("swatch_picker.rs");
        assert!(swatch.contains("focus_motion.render(cell, Vec::new(), item_radius, true, cx)"));
        let slider = include_str!("slider.rs");
        let slider_render = slider
            .split("focus_motion.render(")
            .nth(1)
            .expect("the slider thumb must render through the shared motion frame");
        assert!(slider_render.contains("px(16.)"));
        let picker = include_str!("picker.rs");
        let trigger_render = picker
            .split("trigger_focus_motion.render(")
            .nth(1)
            .expect("the trigger must render through the shared motion frame");
        assert!(trigger_render.contains("util::hairline_radius(cx)"));
    }

    #[test]
    fn hsl_and_hsb_saturation_differ_and_round_trip() {
        // Mid-brightness, half-saturated: the two spaces disagree here, which
        // is exactly the case a colorSpace-unaware slider got wrong.
        let c = PickerColor::hsb(210.0, 0.5, 0.6);
        let hsl_s = c.hsl_saturation();
        assert!(
            (hsl_s - c.saturation).abs() > 0.05,
            "expected the two saturations to differ, got {hsl_s} vs {}",
            c.saturation
        );

        // Writing an HSL saturation and reading it back is stable.
        for target in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let out = c.with_hsl_saturation(target);
            assert!(
                (out.hsl_saturation() - target).abs() < 1e-3,
                "hsl saturation {target} round-tripped to {}",
                out.hsl_saturation()
            );
            // Hue and HSL lightness are held.
            assert!((out.hue - c.hue).abs() < 1e-3);
        }
    }

    #[test]
    fn equality_uses_public_hsb_coordinates_not_color_model_history() {
        let hsl_black = PickerColor::hsb(40.0, 0.5, 0.5)
            .with_channel_in(ColorChannel::Saturation, ColorSpace::Hsl, 0.75)
            .with_channel_in(ColorChannel::Lightness, ColorSpace::Hsl, 0.0);
        let hsb_black = PickerColor::hsb(hsl_black.hue, 0.0, 0.0);
        assert_eq!(hsl_black, hsb_black);
        assert_eq!(hsl_black.to_hex(), hsb_black.to_hex());
    }

    #[test]
    fn hue_gradients_have_seven_stops_and_reverse_on_the_vertical_axis() {
        let stops = hue_stop_colors(PickerColor::hsb(0.0, 1.0, 1.0), ColorSpace::Hsb);
        assert_eq!(stops[0], stops[6]);
        for pair in stops.windows(2).take(5) {
            assert_ne!(pair[0], pair[1]);
        }
        assert!((hue_band_offset(0, false, 0.0) - 0.0).abs() < f32::EPSILON);
        assert!((hue_band_offset(5, false, 0.0) - 5.0 / 6.0).abs() < f32::EPSILON);
        assert!((hue_band_offset(0, true, 0.0) - 5.0 / 6.0).abs() < f32::EPSILON);
        assert!((hue_band_offset(5, true, 0.0) - 0.0).abs() < f32::EPSILON);
        for index in 0..6 {
            assert!((hue_band_extent(index, 0.0) - 1.0 / 6.0).abs() < 1e-6);
        }
    }

    #[test]
    fn inset_hue_bands_stretch_only_the_two_end_bands() {
        // A 240px slider with 10px caps.
        let inset = 10.0 / 240.0;
        let width = (1.0 - inset * 2.0) / 6.0;
        // The end bands reach the box edges; the interior ones keep their
        // travel width, so the ramp is unchanged over the thumb's travel.
        assert!((hue_band_offset(0, false, inset) - 0.0).abs() < 1e-6);
        assert!((hue_band_extent(0, inset) - (width + inset)).abs() < 1e-6);
        assert!((hue_band_extent(5, inset) - (width + inset)).abs() < 1e-6);
        for index in 1..5 {
            assert!((hue_band_extent(index, inset) - width).abs() < 1e-6);
            assert!(
                (hue_band_offset(index, false, inset) - (inset + index as f32 * width)).abs()
                    < 1e-6
            );
        }
        // Vertical mirrors: band 0 is flush with the bottom, band 5 the top.
        assert!((hue_band_offset(5, true, inset) - 0.0).abs() < 1e-6);
        assert!(
            (hue_band_offset(0, true, inset) - (1.0 - width - inset)).abs() < 1e-6,
            "band 0 must end at the bottom edge"
        );
        let total: f32 = (0..6).map(|i| hue_band_extent(i, inset)).sum();
        assert!((total - 1.0).abs() < 1e-5, "the bands must tile the box");
    }

    #[test]
    fn corner_arc_inset_is_zero_outside_the_corner_and_full_at_the_edge() {
        let r = 16.0;
        assert!((corner_arc_inset(0.0, r) - r).abs() < 1e-6);
        assert!((corner_arc_inset(r, r) - 0.0).abs() < f32::EPSILON);
        assert!((corner_arc_inset(r + 5.0, r) - 0.0).abs() < f32::EPSILON);
        assert!((corner_arc_inset(1.0, 0.0) - 0.0).abs() < f32::EPSILON);
        let mut previous = f32::INFINITY;
        for step in 0..=32 {
            let d = r * step as f32 / 32.0;
            let inset = corner_arc_inset(d, r);
            assert!(inset <= previous + 1e-6, "must decrease with distance");
            assert!((0.0..=r).contains(&inset));
            // Strictly inside the arc: the point (d, inset) is on the circle
            // centred at (r, r), so the strip never crosses the curve.
            let dx = r - d;
            let dy = r - inset;
            assert!((dx * dx + dy * dy).sqrt() <= r + 1e-3);
            previous = inset;
        }
        assert!((previous - 0.0).abs() < 1e-5);
    }

    #[test]
    fn the_slider_ramp_is_one_full_length_element_over_nothing_but_the_checkerboard() {
        let source = include_str!("slider.rs");
        // The stop-percentage design replaced the end bases, the alpha end
        // piece and the short-track fallback caps: with `opacity()` applied
        // per element, anything under the ramp shows through when disabled.
        for gone in [
            "END_BASE_PX",
            "wide_ends",
            "wide_opaque_ends",
            "start_cap",
            "end_cap",
        ] {
            assert!(
                !source.contains(gone),
                "`{gone}` must not come back: it paints under or over the ramp"
            );
        }
        // Exactly one child is added to the clip besides the ramp, and it is
        // the alpha checkerboard.
        let painted: Vec<&str> = source
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("layers = layers"))
            .collect();
        assert_eq!(
            painted,
            vec!["layers = layers", "layers = layers.child(ramp);"],
            "only the checkerboard and the ramp may be painted into the clip"
        );
        // The ramp is full-length, so its r10 corners are never clamped.
        assert!(source.contains("let ramp = div().absolute().inset_0().rounded(track_r);"));
        assert!(source.contains("gpui::linear_color_stop(start_color, inset)"));
        assert!(source.contains("gpui::linear_color_stop(end_color, 1.0 - inset)"));
    }

    #[test]
    fn lightness_midpoint_preserves_the_current_hue() {
        let value = PickerColor::hsb(0.0, 1.0, 1.0);
        let (start, middle, end) = lightness_gradient_colors(value, ColorSpace::Hsl, 0.0, 1.0);
        assert_eq!(start, gpui::black());
        assert_eq!(middle, value.to_hsla());
        assert_eq!(end, gpui::white());
    }

    #[test]
    #[allow(clippy::float_cmp)] // the two spaces must agree bit for bit
    fn channel_in_only_diverges_for_saturation() {
        let c = PickerColor::hsb(30.0, 0.4, 0.8);
        for ch in [
            ColorChannel::Hue,
            ColorChannel::Brightness,
            ColorChannel::Lightness,
            ColorChannel::Alpha,
            ColorChannel::Red,
            ColorChannel::Green,
            ColorChannel::Blue,
        ] {
            let (hsl, hsb) = (
                c.channel_in(ch, ColorSpace::Hsl),
                c.channel_in(ch, ColorSpace::Hsb),
            );
            assert!(
                (hsl - hsb).abs() < 1e-6,
                "{ch:?} should not depend on the colour space"
            );
        }
        assert_ne!(
            c.channel_in(ColorChannel::Saturation, ColorSpace::Hsl),
            c.channel_in(ColorChannel::Saturation, ColorSpace::Hsb)
        );
    }

    /// The values `ColorSlider` exposes on its `slider` node: the channel
    /// range is the min/max and the label is the fallback accessible name.
    #[test]
    fn slider_channel_range_and_name_match_react_stately() {
        let cases = [
            (ColorChannel::Hue, (0.0, 360.0), "Hue"),
            (ColorChannel::Saturation, (0.0, 1.0), "Saturation"),
            (ColorChannel::Brightness, (0.0, 1.0), "Brightness"),
            (ColorChannel::Lightness, (0.0, 1.0), "Lightness"),
            (ColorChannel::Alpha, (0.0, 1.0), "Alpha"),
            (ColorChannel::Red, (0.0, 255.0), "Red"),
            (ColorChannel::Green, (0.0, 255.0), "Green"),
            (ColorChannel::Blue, (0.0, 255.0), "Blue"),
        ];
        for (channel, range, label) in cases {
            assert_eq!(channel.range(), range, "{channel:?} range");
            assert_eq!(channel.label(), label, "{channel:?} label");
        }
    }

    #[test]
    fn area_channels_match_the_space() {
        assert_eq!(
            ColorSpace::Hsl.area_channels(),
            (ColorChannel::Saturation, ColorChannel::Lightness)
        );
        assert_eq!(
            ColorSpace::Hsb.area_channels(),
            (ColorChannel::Saturation, ColorChannel::Brightness)
        );
        assert_eq!(
            ColorSpace::Rgb.area_channels(),
            (ColorChannel::Red, ColorChannel::Green)
        );
    }

    #[test]
    fn hex_round_trips() {
        for hex in [
            "#FF0000", "#00FF00", "#0000FF", "#123456", "#FFFFFF", "#000000",
        ] {
            let c = PickerColor::from_hex(hex).expect(hex);
            assert_eq!(c.to_hex(), hex, "{hex}");
        }
    }

    #[test]
    fn short_hex_expands() {
        assert_eq!(PickerColor::from_hex("#f00").unwrap().to_hex(), "#FF0000");
    }

    #[test]
    fn hex_with_alpha_round_trips() {
        let c = PickerColor::from_hex("#11223344").unwrap();
        assert_eq!(c.to_hex(), "#11223344");
    }

    #[test]
    fn rejects_bad_hex() {
        assert!(PickerColor::from_hex("#12345").is_none());
        assert!(PickerColor::from_hex("nope").is_none());
    }

    #[test]
    fn color_surfaces_keep_the_pinned_depth_layers() {
        let inner = color_inner_shadow();
        assert!(inner.inset);
        assert_eq!(inner.offset, gpui::point(px(0.), px(0.)));
        assert_eq!(inner.spread_radius, px(1.));
        assert!((inner.color.a - 0.1).abs() < f32::EPSILON);

        let thumb = color_thumb_shadows();
        assert_eq!(thumb.len(), 2);
        assert!(!thumb[0].inset);
        assert!(thumb[1].inset);
        assert!(thumb.iter().all(|shadow| {
            shadow.offset == gpui::point(px(0.), px(0.))
                && shadow.spread_radius == px(1.)
                && (shadow.color.a - 0.1).abs() < f32::EPSILON
        }));

        let track = color_track_shadows();
        assert_eq!(track.len(), 4);
        assert!(track.iter().all(|shadow| shadow.inset
            && shadow.blur_radius == px(0.)
            && shadow.spread_radius == px(0.)));

        assert_eq!(
            color_slider_thumb_transition_offset(0.25, 0.75, px(200.), false,),
            px(-100.)
        );
        assert_eq!(
            color_slider_thumb_transition_offset(0.25, 0.75, px(200.), true,),
            px(100.)
        );
    }

    #[test]
    fn color_picker_entry_offsets_follow_the_requested_side() {
        assert_eq!(
            color_picker_entry_offset(Placement::BottomStart),
            (0.0, -4.0)
        );
        assert_eq!(color_picker_entry_offset(Placement::TopEnd), (0.0, 4.0));
        assert_eq!(color_picker_entry_offset(Placement::Left), (4.0, 0.0));
        assert_eq!(color_picker_entry_offset(Placement::Right), (-4.0, 0.0));
    }

    #[test]
    fn channel_edits_are_isolated() {
        let c = PickerColor::hsb(120.0, 0.5, 0.5);
        let hue = c.with_channel(ColorChannel::Hue, 240.0);
        assert!((hue.hue - 240.0).abs() < 1e-3);
        assert!((hue.saturation - c.saturation).abs() < 1e-3);
        assert!((hue.brightness - c.brightness).abs() < 1e-3);
    }

    #[test]
    #[allow(clippy::float_cmp)] // the clamp bounds are exact
    fn channel_values_stay_in_range() {
        let c = PickerColor::default();
        assert!((c.with_channel(ColorChannel::Alpha, 5.0).alpha - 1.0).abs() < 1e-6);
        assert_eq!(c.with_channel(ColorChannel::Alpha, -1.0).alpha, 0.0);
        assert!(
            c.with_channel(ColorChannel::Red, 999.0)
                .channel(ColorChannel::Red)
                <= 255.0
        );
    }

    #[test]
    fn rgb_channel_edit_matches_readback() {
        let c = PickerColor::from_hex("#204060").unwrap();
        let next = c.with_channel(ColorChannel::Green, 128.0);
        assert!((next.channel(ColorChannel::Green) - 128.0).abs() < 1.0);
    }

    #[test]
    fn dark_saturated_blue_uses_a_white_selected_indicator() {
        let blue = PickerColor::from_hex("#0000FF").unwrap();
        assert_eq!(color_swatch_indicator_color(blue), gpui::white());
    }

    #[test]
    fn mid_gray_uses_a_black_selected_indicator() {
        let gray = PickerColor::from_hex("#808080").unwrap();
        assert_eq!(color_swatch_indicator_color(gray), gpui::black());
    }
}
