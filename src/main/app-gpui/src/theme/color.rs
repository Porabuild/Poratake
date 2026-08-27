//! Color utilities replicating the web design system 1:1.
//!
//! The Electron renderer resolves its palette through CSS
//! `color-mix(in oklab, A p%, B)`. Every value the UI reads therefore goes
//! through the same OKLab interpolation here so the GPUI shell renders
//! identical colors.

use gpui::{rgba, Hsla};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Srgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Srgba {
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    pub fn from_hex(hex: &str) -> Self {
        let hex = hex.trim();
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        let (r, g, b, a) = match hex.len() {
            6 => (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
                Ok(255u8),
            ),
            8 => (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
                u8::from_str_radix(&hex[6..8], 16),
            ),
            _ => (Ok(0u8), Ok(0u8), Ok(0u8), Ok(255u8)),
        };
        let u = |v: Result<u8, _>| v.unwrap_or(0) as f32 / 255.0;
        Self {
            r: u(r),
            g: u(g),
            b: u(b),
            a: u(a),
        }
    }

    fn to_oklab(self) -> Oklab {
        let lr = srgb_to_linear(self.r);
        let lg = srgb_to_linear(self.g);
        let lb = srgb_to_linear(self.b);

        let l = 0.412_221_46 * lr + 0.536_332_55 * lg + 0.051_445_995 * lb;
        let m = 0.211_903_5 * lr + 0.680_699_5 * lg + 0.107_396_96 * lb;
        let s = 0.088_302_46 * lr + 0.281_718_85 * lg + 0.629_978_7 * lb;

        let l_ = cbrt(l);
        let m_ = cbrt(m);
        let s_ = cbrt(s);

        Oklab {
            lightness: 0.210_454_26 * l_ + 0.793_617_8 * m_ - 0.004_072_047 * s_,
            a: 1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_,
            b: 0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_,
            alpha: self.a,
        }
    }

    fn from_oklab(oklab: Oklab) -> Self {
        let l_ = oklab.lightness + 0.396_337_78 * oklab.a + 0.215_803_76 * oklab.b;
        let m_ = oklab.lightness - 0.105_561_346 * oklab.a - 0.063_854_17 * oklab.b;
        let s_ = oklab.lightness - 0.089_484_18 * oklab.a - 1.291_485_5 * oklab.b;

        let l = l_ * l_ * l_;
        let m = m_ * m_ * m_;
        let s = s_ * s_ * s_;

        let lr = 4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s;
        let lg = -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s;
        let lb = -0.0041960863 * l - 0.703_418_6 * m + 1.707_614_7 * s;

        let clamp01 = |v: f32| v.clamp(0.0, 1.0);
        Self {
            r: clamp01(linear_to_srgb(lr)),
            g: clamp01(linear_to_srgb(lg)),
            b: clamp01(linear_to_srgb(lb)),
            a: oklab.alpha.clamp(0.0, 1.0),
        }
    }

    /// Parses a CSS color literal used by the design system: hex, the
    /// `oklch(L C H / alpha)` forms found in base.css, and the `rgb()`/`rgba()`
    /// notation annotations are persisted with.
    pub fn parse(value: &str) -> Self {
        let value = value.trim();
        if value.starts_with('#') {
            return Self::from_hex(value);
        }
        if let Some(rest) = value.strip_prefix("oklch(") {
            return Self::from_oklch_str(rest.trim_end_matches(')'));
        }
        if value.starts_with("rgb") {
            return Self::from_rgb_str(value);
        }
        match value {
            "white" => Self::from_hex("#ffffff"),
            "black" => Self::from_hex("#000000"),
            "transparent" | "none" => Self {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
            _ => Self::from_hex("#000000"),
        }
    }

    /// `rgb(r g b)` / `rgba(r, g, b, a)`, with channels in 0-255 and alpha as a
    /// fraction — the shape `TEXT_BG_COLOR` and the cursor highlight use.
    fn from_rgb_str(value: &str) -> Self {
        let body = value
            .split_once('(')
            .map(|(_, rest)| rest.trim_end_matches(')'))
            .unwrap_or_default();
        let parts: Vec<f32> = body
            .split([',', '/', ' '])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse().ok())
            .collect();
        if parts.len() < 3 {
            return Self::from_hex("#000000");
        }
        Self {
            r: (parts[0] / 255.0).clamp(0.0, 1.0),
            g: (parts[1] / 255.0).clamp(0.0, 1.0),
            b: (parts[2] / 255.0).clamp(0.0, 1.0),
            a: parts.get(3).copied().unwrap_or(1.0).clamp(0.0, 1.0),
        }
    }

    fn from_oklch_str(body: &str) -> Self {
        let body = body.split('/').map(str::trim).collect::<Vec<_>>();
        let nums: Vec<f32> = body[0]
            .split_whitespace()
            .filter_map(|token| token.trim_end_matches('%').parse::<f32>().ok())
            .collect();
        if nums.len() < 3 {
            return Self::from_hex("#000000");
        }
        let alpha = body
            .get(1)
            .and_then(|a| a.parse::<f32>().ok())
            .unwrap_or(1.0);
        Self::from_oklch(nums[0], nums[1], nums[2], alpha)
    }

    pub fn from_oklch(lightness: f32, chroma: f32, hue_degrees: f32, alpha: f32) -> Self {
        let radians = hue_degrees.to_radians();
        let oklab = Oklab {
            lightness,
            a: chroma * radians.cos(),
            b: chroma * radians.sin(),
            alpha,
        };
        Self::from_oklab(oklab)
    }

    pub fn to_gpui_rgb(self) -> gpui::Rgba {
        rgba(
            ((self.r * 255.0).round() as u32) << 24
                | ((self.g * 255.0).round() as u32) << 16
                | ((self.b * 255.0).round() as u32) << 8
                | (self.a * 255.0).round() as u32,
        )
    }

    pub fn to_hsla(self) -> Hsla {
        self.to_gpui_rgb().into()
    }

    pub fn from_hsla(color: Hsla) -> Self {
        let rgba: gpui::Rgba = color.into();
        Self {
            r: rgba.r,
            g: rgba.g,
            b: rgba.b,
            a: rgba.a,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Oklab {
    lightness: f32,
    a: f32,
    b: f32,
    alpha: f32,
}

fn cbrt(v: f32) -> f32 {
    if v >= 0.0 {
        v.cbrt()
    } else {
        -(-v).cbrt()
    }
}

fn srgb_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(channel: f32) -> f32 {
    if channel <= 0.0031308 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

/// Replicates CSS `color-mix(in oklab, colorA p%, colorB)` for two
/// already-parsed colors.
pub fn mix_parsed(color_a: Srgba, percentage: f32, color_b: Srgba) -> Srgba {
    mix_parsed_weights(color_a, percentage, color_b, 100.0 - percentage)
}

/// Replicates CSS `color-mix(in oklab, colorA a%, colorB b%)`, where the two
/// percentages need not sum to 100 — CSS normalizes them, which is how tokens
/// like `--field-hover` (`X 90%, Y 2%`) resolve.
pub fn mix_parsed_weights(
    color_a: Srgba,
    percentage_a: f32,
    color_b: Srgba,
    percentage_b: f32,
) -> Srgba {
    let w1_raw = percentage_a / 100.0;
    let w2_raw = percentage_b / 100.0;
    let sum = w1_raw + w2_raw;
    let normalize = if sum <= f32::EPSILON { 0.0 } else { 1.0 / sum };
    let w1 = w1_raw * normalize;
    let w2 = w2_raw * normalize;

    let lab_a = color_a.to_oklab();
    let lab_b = color_b.to_oklab();

    // Premultiplied interpolation.
    let pm_l = lab_a.lightness * lab_a.alpha * w1 + lab_b.lightness * lab_b.alpha * w2;
    let pm_a = lab_a.a * lab_a.alpha * w1 + lab_b.a * lab_b.alpha * w2;
    let pm_b = lab_a.b * lab_a.alpha * w1 + lab_b.b * lab_b.alpha * w2;
    let out_alpha = lab_a.alpha * w1 + lab_b.alpha * w2;

    let mixed = if out_alpha <= f32::EPSILON {
        Oklab {
            lightness: 0.0,
            a: 0.0,
            b: 0.0,
            alpha: 0.0,
        }
    } else {
        Oklab {
            lightness: pm_l / out_alpha,
            a: pm_a / out_alpha,
            b: pm_b / out_alpha,
            alpha: out_alpha,
        }
    };

    Srgba::from_oklab(mixed)
}

/// Replicates CSS `color-mix(in oklab, colorA p%, colorB)`.
///
/// Mixing happens on premultiplied OKLab channels per the CSS Color 5 spec,
/// which keeps blends over `transparent` hue-correct.
pub fn mix_oklab(color_a: &str, percentage: f32, color_b: &str) -> Srgba {
    mix_parsed(Srgba::parse(color_a), percentage, Srgba::parse(color_b))
}

/// Interpolates two colors in sRGB, which is how a CSS
/// `transition: background-color` moves between them.
pub fn lerp_srgb(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0.0, 1.0);
    let a = Srgba::from_hsla(from);
    let b = Srgba::from_hsla(to);
    Srgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
    .to_hsla()
}

/// `color-mix(in oklab, a p%, b)` for two already-resolved theme colors.
pub fn mix_hsla(color_a: gpui::Hsla, percentage: f32, color_b: gpui::Hsla) -> gpui::Hsla {
    mix_parsed(
        Srgba::from_hsla(color_a),
        percentage,
        Srgba::from_hsla(color_b),
    )
    .to_hsla()
}
