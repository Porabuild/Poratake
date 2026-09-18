use std::sync::OnceLock;

use fontdue::{Font, FontSettings};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Bold,
}

/// Mirrors `FONT_FAMILIES` in `renderer/components/editor/text/text-utils.ts`
/// plus the `system-ui` default the renderer falls back to.
const CANDIDATES: &[(&str, Weight, &[(&str, u32)])] = &[
    (
        "sans",
        Weight::Regular,
        &[
            (r"C:\Windows\Fonts\segoeui.ttf", 0),
            (r"C:\Windows\Fonts\arial.ttf", 0),
            ("/System/Library/Fonts/SFNS.ttf", 0),
            ("/System/Library/Fonts/Supplemental/Arial.ttf", 0),
            ("/System/Library/Fonts/Helvetica.ttc", 0),
            ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
        ],
    ),
    (
        "sans",
        Weight::Bold,
        &[
            (r"C:\Windows\Fonts\segoeuib.ttf", 0),
            (r"C:\Windows\Fonts\arialbd.ttf", 0),
            ("/System/Library/Fonts/Supplemental/Arial Bold.ttf", 0),
            ("/System/Library/Fonts/Helvetica.ttc", 1),
            ("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 0),
        ],
    ),
    (
        "serif",
        Weight::Regular,
        &[
            (r"C:\Windows\Fonts\georgia.ttf", 0),
            (r"C:\Windows\Fonts\times.ttf", 0),
            ("/System/Library/Fonts/Supplemental/Georgia.ttf", 0),
            ("/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf", 0),
        ],
    ),
    (
        "serif",
        Weight::Bold,
        &[
            (r"C:\Windows\Fonts\georgiab.ttf", 0),
            (r"C:\Windows\Fonts\timesbd.ttf", 0),
            ("/System/Library/Fonts/Supplemental/Georgia Bold.ttf", 0),
            ("/usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf", 0),
        ],
    ),
    (
        "mono",
        Weight::Regular,
        &[
            (r"C:\Windows\Fonts\consola.ttf", 0),
            (r"C:\Windows\Fonts\cour.ttf", 0),
            ("/System/Library/Fonts/Menlo.ttc", 0),
            ("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", 0),
        ],
    ),
    (
        "mono",
        Weight::Bold,
        &[
            (r"C:\Windows\Fonts\consolab.ttf", 0),
            (r"C:\Windows\Fonts\courbd.ttf", 0),
            ("/System/Library/Fonts/Menlo.ttc", 1),
            (
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf",
                0,
            ),
        ],
    ),
    (
        "comic",
        Weight::Regular,
        &[
            (r"C:\Windows\Fonts\comic.ttf", 0),
            ("/System/Library/Fonts/Supplemental/Comic Sans MS.ttf", 0),
        ],
    ),
    (
        "comic",
        Weight::Bold,
        &[
            (r"C:\Windows\Fonts\comicbd.ttf", 0),
            (
                "/System/Library/Fonts/Supplemental/Comic Sans MS Bold.ttf",
                0,
            ),
        ],
    ),
];

pub const SANS_UI_FAMILY: &str = if cfg!(target_os = "windows") {
    "Segoe UI"
} else if cfg!(target_os = "macos") {
    ".AppleSystemUIFont"
} else {
    "DejaVu Sans"
};

pub const MONO_UI_FAMILY: &str = if cfg!(target_os = "windows") {
    "Consolas"
} else if cfg!(target_os = "macos") {
    "Menlo"
} else {
    "DejaVu Sans Mono"
};

pub fn ui_family(family: &str) -> &'static str {
    match family {
        "serif" => "Georgia",
        "mono" => MONO_UI_FAMILY,
        "comic" => "Comic Sans MS",
        _ => SANS_UI_FAMILY,
    }
}

struct Loaded {
    family: &'static str,
    weight: Weight,
    font: Font,
}

fn fonts() -> &'static Vec<Loaded> {
    static FONTS: OnceLock<Vec<Loaded>> = OnceLock::new();
    FONTS.get_or_init(|| {
        CANDIDATES
            .iter()
            .filter_map(|(family, weight, paths)| {
                paths.iter().find_map(|(path, index)| {
                    let bytes = std::fs::read(path).ok()?;
                    let settings = FontSettings {
                        collection_index: *index,
                        ..FontSettings::default()
                    };
                    let font = Font::from_bytes(bytes, settings).ok()?;
                    // SFNS.ttf parses but carries CFF2 outlines fontdue cannot
                    // rasterize, yielding empty bitmaps; probe before accepting.
                    let (_, bitmap) = font.rasterize('A', 32.0);
                    if !bitmap.iter().any(|coverage| *coverage > 0) {
                        return None;
                    }
                    Some(Loaded {
                        family,
                        weight: *weight,
                        font,
                    })
                })
            })
            .collect()
    })
}

#[cfg(test)]
fn has_face(family: &str, weight: Weight) -> bool {
    fonts()
        .iter()
        .any(|entry| entry.family == family && entry.weight == weight)
}

fn font_for(family: &str, weight: Weight) -> Option<&'static Font> {
    let loaded = fonts();
    let face = |family: &str, weight: Weight| {
        loaded
            .iter()
            .find(|entry| entry.family == family && entry.weight == weight)
    };
    face(family, weight)
        .or_else(|| face(family, Weight::Regular))
        .or_else(|| face("sans", weight))
        .or_else(|| face("sans", Weight::Regular))
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

pub fn measure(text: &str, family: &str, size: f32, weight: Weight) -> Option<Metrics> {
    let font = font_for(family, weight)?;
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
    weight: Weight,
    size: f32,
    origin_x: f32,
    baseline_y: f32,
    mut plot: impl FnMut(i64, i64, f32),
) -> bool {
    let Some(font) = font_for(family, weight) else {
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
    fn the_live_field_resolves_the_same_families_the_rasterizer_knows() {
        assert_eq!(ui_family("serif"), "Georgia");
        assert_eq!(ui_family("mono"), MONO_UI_FAMILY);
        assert_eq!(ui_family("comic"), "Comic Sans MS");
        assert_eq!(ui_family("sans"), SANS_UI_FAMILY);
        assert_eq!(ui_family("nope"), SANS_UI_FAMILY);
        for family in ["serif", "mono", "comic"] {
            assert!(CANDIDATES.iter().any(|(name, _, _)| *name == family));
        }
    }

    #[test]
    fn falls_back_to_the_sans_family_for_unknown_names() {
        if !is_available() {
            return;
        }
        assert!(font_for("nope", Weight::Regular).is_some());
        assert!(font_for("sans", Weight::Regular).is_some());
        assert!(font_for("nope", Weight::Bold).is_some());
    }

    #[test]
    fn measures_wider_text_as_wider() {
        if !is_available() {
            return;
        }
        let short = measure("i", "sans", 20.0, Weight::Regular).expect("short");
        let long = measure("iiiiii", "sans", 20.0, Weight::Regular).expect("long");
        assert!(long.width > short.width);
        assert!(short.height() > 0.0);
    }

    #[test]
    fn rasterizes_visible_coverage() {
        if !is_available() {
            return;
        }
        let mut covered = 0usize;
        let drawn = rasterize(
            "A",
            "sans",
            Weight::Regular,
            32.0,
            0.0,
            32.0,
            |_, _, coverage| {
                if coverage > 0.5 {
                    covered += 1;
                }
            },
        );
        assert!(drawn);
        assert!(covered > 0);
    }

    #[test]
    fn the_bold_face_covers_more_than_the_regular_one() {
        if !is_available() {
            return;
        }
        let count = |weight| {
            let mut covered = 0usize;
            rasterize("8", "sans", weight, 48.0, 0.0, 48.0, |_, _, coverage| {
                if coverage > 0.5 {
                    covered += 1;
                }
            });
            covered
        };
        if !has_face("sans", Weight::Bold) {
            return;
        }
        assert!(count(Weight::Bold) > count(Weight::Regular));
    }
}
