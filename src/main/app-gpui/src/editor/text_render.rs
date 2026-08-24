//! Text rasterization for export. The preview is drawn by GPUI with the
//! platform UI font; the export loads the same family from the OS so a saved
//! image matches what the editor showed. Nothing is bundled, so there is no
//! extra font to license or ship.

use std::sync::OnceLock;

use fontdue::{Font, FontSettings};

/// Mirrors `FONT_FAMILIES` in `renderer/components/editor/text/text-utils.ts`
/// plus the `system-ui` default the renderer falls back to.
const CANDIDATES: &[(&str, &[&str])] = &[
    (
        "sans",
        &[
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\arial.ttf",
            "/System/Library/Fonts/SFNS.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ],
    ),
    (
        "serif",
        &[
            r"C:\Windows\Fonts\georgia.ttf",
            r"C:\Windows\Fonts\times.ttf",
            "/System/Library/Fonts/Supplemental/Georgia.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
        ],
    ),
    (
        "mono",
        &[
            r"C:\Windows\Fonts\consola.ttf",
            r"C:\Windows\Fonts\cour.ttf",
            "/System/Library/Fonts/Menlo.ttc",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        ],
    ),
    (
        "comic",
        &[
            r"C:\Windows\Fonts\comic.ttf",
            "/System/Library/Fonts/Supplemental/Comic Sans MS.ttf",
        ],
    ),
];

struct Loaded {
    family: &'static str,
    font: Font,
}

fn fonts() -> &'static Vec<Loaded> {
    static FONTS: OnceLock<Vec<Loaded>> = OnceLock::new();
    FONTS.get_or_init(|| {
        CANDIDATES
            .iter()
            .filter_map(|(family, paths)| {
                paths.iter().find_map(|path| {
                    let bytes = std::fs::read(path).ok()?;
                    let font = Font::from_bytes(bytes, FontSettings::default()).ok()?;
                    Some(Loaded { family, font })
                })
            })
            .collect()
    })
}

fn font_for(family: &str) -> Option<&'static Font> {
    let loaded = fonts();
    loaded
        .iter()
        .find(|entry| entry.family == family)
        .or_else(|| loaded.iter().find(|entry| entry.family == "sans"))
        .or_else(|| loaded.first())
        .map(|entry| &entry.font)
}

pub struct Metrics {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
}

impl Metrics {
    #[allow(dead_code)]
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }
}

pub fn measure(text: &str, family: &str, size: f32) -> Option<Metrics> {
    let font = font_for(family)?;
    let line = font.horizontal_line_metrics(size)?;
    let width = text
        .chars()
        .map(|character| font.metrics(character, size).advance_width)
        .sum();
    Some(Metrics {
        width,
        ascent: line.ascent,
        descent: -line.descent,
    })
}

/// Calls `plot(x, y, coverage)` for every covered pixel of `text`, with the
/// origin at the text's left baseline.
pub fn rasterize(
    text: &str,
    family: &str,
    size: f32,
    origin_x: f32,
    baseline_y: f32,
    mut plot: impl FnMut(i64, i64, f32),
) -> bool {
    let Some(font) = font_for(family) else {
        return false;
    };
    let mut pen_x = origin_x;
    for character in text.chars() {
        let (metrics, bitmap) = font.rasterize(character, size);
        let left = pen_x + metrics.xmin as f32;
        let top = baseline_y - (metrics.height as f32 + metrics.ymin as f32);
        for row in 0..metrics.height {
            for column in 0..metrics.width {
                let coverage = bitmap[row * metrics.width + column] as f32 / 255.0;
                if coverage <= 0.0 {
                    continue;
                }
                plot(
                    (left + column as f32).round() as i64,
                    (top + row as f32).round() as i64,
                    coverage,
                );
            }
        }
        pen_x += metrics.advance_width;
    }
    true
}

#[allow(dead_code)]
pub fn is_available() -> bool {
    !fonts().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_the_sans_family_for_unknown_names() {
        if !is_available() {
            return;
        }
        assert!(font_for("nope").is_some());
        assert!(font_for("sans").is_some());
    }

    #[test]
    fn measures_wider_text_as_wider() {
        if !is_available() {
            return;
        }
        let short = measure("i", "sans", 20.0).expect("short");
        let long = measure("iiiiii", "sans", 20.0).expect("long");
        assert!(long.width > short.width);
        assert!(short.height() > 0.0);
    }

    #[test]
    fn rasterizes_visible_coverage() {
        if !is_available() {
            return;
        }
        let mut covered = 0usize;
        let drawn = rasterize("A", "sans", 32.0, 0.0, 32.0, |_, _, coverage| {
            if coverage > 0.5 {
                covered += 1;
            }
        });
        assert!(drawn);
        assert!(covered > 0);
    }
}
