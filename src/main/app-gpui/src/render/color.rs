//! CSS colour parsing. Both shells persist colours as CSS strings, so the
//! rasterizer accepts exactly what the renderer writes: `#rgb`, `#rrggbb`,
//! `#rrggbbaa`, `rgb()`, `rgba()` and the handful of keywords the editor uses.

use tiny_skia::{Color, ColorU8};

pub fn parse(value: &str) -> Option<Color> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    match trimmed {
        "transparent" | "none" => return Some(Color::TRANSPARENT),
        "black" => return Some(Color::from_rgba8(0, 0, 0, 255)),
        "white" => return Some(Color::from_rgba8(255, 255, 255, 255)),
        _ => {}
    }
    if let Some(hex) = trimmed.strip_prefix('#') {
        return parse_hex(hex);
    }
    if trimmed.starts_with("rgb") {
        return parse_rgb(trimmed);
    }
    None
}

/// Parses `value`, falling back to `fallback` so a malformed colour renders as
/// something rather than dropping the shape entirely.
pub fn parse_or(value: &str, fallback: Color) -> Color {
    parse(value).unwrap_or(fallback)
}

fn parse_hex(hex: &str) -> Option<Color> {
    let digits: Vec<u8> = hex
        .chars()
        .map(|character| character.to_digit(16).map(|value| value as u8))
        .collect::<Option<_>>()?;
    let (r, g, b, a) = match digits.len() {
        3 => (digits[0] * 17, digits[1] * 17, digits[2] * 17, 255),
        4 => (
            digits[0] * 17,
            digits[1] * 17,
            digits[2] * 17,
            digits[3] * 17,
        ),
        6 => (
            digits[0] * 16 + digits[1],
            digits[2] * 16 + digits[3],
            digits[4] * 16 + digits[5],
            255,
        ),
        8 => (
            digits[0] * 16 + digits[1],
            digits[2] * 16 + digits[3],
            digits[4] * 16 + digits[5],
            digits[6] * 16 + digits[7],
        ),
        _ => return None,
    };
    Some(Color::from_rgba8(r, g, b, a))
}

fn parse_rgb(value: &str) -> Option<Color> {
    let open = value.find('(')?;
    let close = value.rfind(')')?;
    let parts: Vec<&str> = value[open + 1..close]
        .split(|character| character == ',' || character == '/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() < 3 {
        return None;
    }
    let channel = |part: &str| -> Option<u8> {
        if let Some(percent) = part.strip_suffix('%') {
            let value: f32 = percent.trim().parse().ok()?;
            return Some((value / 100.0 * 255.0).clamp(0.0, 255.0).round() as u8);
        }
        let value: f32 = part.parse().ok()?;
        Some(value.clamp(0.0, 255.0).round() as u8)
    };
    let r = channel(parts[0])?;
    let g = channel(parts[1])?;
    let b = channel(parts[2])?;
    let a = match parts.get(3) {
        Some(part) => {
            if let Some(percent) = part.strip_suffix('%') {
                let value: f32 = percent.trim().parse().ok()?;
                (value / 100.0 * 255.0).clamp(0.0, 255.0).round() as u8
            } else {
                let value: f32 = part.parse().ok()?;
                (value * 255.0).clamp(0.0, 255.0).round() as u8
            }
        }
        None => 255,
    };
    Some(Color::from_rgba8(r, g, b, a))
}

/// Port of `getContrastColor` in `renderer/utils/color.ts`.
pub fn contrast_color(background: &str) -> Color {
    let color = parse_or(background, Color::from_rgba8(0, 0, 0, 255)).to_color_u8();
    if relative_luminance(color) > 0.5 {
        Color::from_rgba8(0, 0, 0, 255)
    } else {
        Color::from_rgba8(255, 255, 255, 255)
    }
}

fn relative_luminance(color: ColorU8) -> f32 {
    let r = color.red() as f32 / 255.0;
    let g = color.green() as f32 / 255.0;
    let b = color.blue() as f32 / 255.0;
    0.299 * r + 0.587 * g + 0.114 * b
}

/// Replaces a colour's alpha, the way `globalAlpha` composes with a fill.
pub fn with_alpha(color: Color, alpha: f32) -> Color {
    let mut scaled = color;
    scaled.set_alpha((color.alpha() * alpha).clamp(0.0, 1.0));
    scaled
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba(color: Color) -> (u8, u8, u8, u8) {
        let value = color.to_color_u8();
        (value.red(), value.green(), value.blue(), value.alpha())
    }

    #[test]
    fn parses_every_hex_length_the_renderer_writes() {
        assert_eq!(rgba(parse("#f00").unwrap()), (255, 0, 0, 255));
        assert_eq!(rgba(parse("#FF3B30").unwrap()), (255, 59, 48, 255));
        assert_eq!(rgba(parse("#00000080").unwrap()), (0, 0, 0, 128));
        assert_eq!(rgba(parse("#0f08").unwrap()), (0, 255, 0, 136));
    }

    #[test]
    fn parses_the_functional_notations() {
        assert_eq!(rgba(parse("rgb(1, 2, 3)").unwrap()), (1, 2, 3, 255));
        assert_eq!(
            rgba(parse("rgba(255, 200, 0, 0.5)").unwrap()),
            (255, 200, 0, 128)
        );
    }

    #[test]
    fn treats_none_and_transparent_alike() {
        assert_eq!(rgba(parse("transparent").unwrap()), (0, 0, 0, 0));
        assert_eq!(rgba(parse("none").unwrap()), (0, 0, 0, 0));
        assert!(parse("chartreuse").is_none());
    }

    #[test]
    fn contrast_follows_the_renderer_threshold() {
        assert_eq!(rgba(contrast_color("#ffffff")), (0, 0, 0, 255));
        assert_eq!(rgba(contrast_color("#000000")), (255, 255, 255, 255));
    }
}
