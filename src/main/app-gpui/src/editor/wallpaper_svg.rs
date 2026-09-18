use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tiny_skia::Pixmap;

const CRIMSON_WAVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stop-color="#7f1d1d" />
            <stop offset="0.5" stop-color="#dc2626" />
            <stop offset="1" stop-color="#fca5a5" />
          </linearGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        <path
          d="M0 320C120 280 240 260 360 280C480 300 600 360 800 340V600H0Z"
          fill="#0f172a" opacity="0.2"
        />
        <path
          d="M0 180C160 140 320 140 480 180C640 220 720 280 800 260V0H0Z"
          fill="#fef2f2" opacity="0.15"
        />
        
      </svg>"##;

const FOREST_GLOW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <radialGradient id="bg" cx="0.3" cy="0.7" r="0.9">
            <stop offset="0" stop-color="#4ade80" />
            <stop offset="0.5" stop-color="#166534" />
            <stop offset="1" stop-color="#052e16" />
          </radialGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        
        <path
          d="M0 480C140 440 280 440 400 470C520 500 680 560 800 540V600H0Z"
          fill="#0f172a" opacity="0.25"
        />
        
      </svg>"##;

const VIOLET_DUNE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stop-color="#1e1b4b" />
            <stop offset="0.5" stop-color="#6366f1" />
            <stop offset="1" stop-color="#f472b6" />
          </linearGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        <path
          d="M0 380C140 320 260 300 360 320C470 346 560 420 800 420V600H0Z"
          fill="#f8fafc" opacity="0.15"
        />
        <path
          d="M0 250C120 190 230 190 330 220C450 258 580 330 800 300V0H0Z"
          fill="#0f172a" opacity="0.2"
        />
        <circle cx="140" cy="160" r="90" fill="#fde68a" opacity="0.2" />
        <circle cx="640" cy="480" r="140" fill="#22d3ee" opacity="0.2" />
      </svg>"##;

const OCEAN_DEPTH: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stop-color="#0c4a6e" />
            <stop offset="0.5" stop-color="#0369a1" />
            <stop offset="1" stop-color="#0ea5e9" />
          </linearGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        <path
          d="M0 200C100 180 200 200 300 220C400 240 500 200 600 180C700 160 800 180 800 180V0H0Z"
          fill="#bae6fd" opacity="0.2"
        />
        
        <path
          d="M60 120C160 100 260 120 360 140"
          stroke="#f8fafc" stroke-width="4" opacity="0.4" fill="none"
        />
      </svg>"##;

const ROSE_GARDEN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <radialGradient id="bg" cx="0.5" cy="0.3" r="0.8">
            <stop offset="0" stop-color="#fdf2f8" />
            <stop offset="0.5" stop-color="#f472b6" />
            <stop offset="1" stop-color="#831843" />
          </radialGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        
        <path
          d="M0 500C160 460 320 460 480 490C640 520 720 580 800 560V600H0Z"
          fill="#0f172a" opacity="0.15"
        />
      </svg>"##;

const AMBER_RIDGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <linearGradient id="bg" x1="0" y1="1" x2="1" y2="0">
            <stop offset="0" stop-color="#0f172a" />
            <stop offset="0.6" stop-color="#ea580c" />
            <stop offset="1" stop-color="#fde68a" />
          </linearGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        <path
          d="M0 420C150 360 260 350 360 370C480 394 590 460 800 460V600H0Z"
          fill="#0f172a" opacity="0.35"
        />
        <path
          d="M0 300C120 260 230 250 330 270C450 300 560 340 800 330"
          stroke="#f8fafc" stroke-width="8" opacity="0.4" fill="none"
        />
        <path
          d="M0 220C140 200 260 210 360 230C470 250 600 270 800 240"
          stroke="#fde68a" stroke-width="6" opacity="0.45" fill="none"
        />
        
      </svg>"##;

const MINT_FROST: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stop-color="#f0fdfa" />
            <stop offset="0.5" stop-color="#5eead4" />
            <stop offset="1" stop-color="#0f766e" />
          </linearGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        
        <path
          d="M640 80C680 120 700 180 680 240"
          stroke="#0f172a" stroke-width="6" opacity="0.15" fill="none"
        />
      </svg>"##;

const ELECTRIC_KITE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stop-color="#0f172a" />
            <stop offset="0.5" stop-color="#2563eb" />
            <stop offset="1" stop-color="#22d3ee" />
          </linearGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        
        <path
          d="M80 520C180 470 280 470 380 500C480 530 600 590 760 560"
          stroke="#f8fafc" stroke-width="6" opacity="0.4" fill="none"
        />
      </svg>"##;

const SLATE_MINIMAL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stop-color="#f8fafc" />
            <stop offset="0.5" stop-color="#94a3b8" />
            <stop offset="1" stop-color="#1e293b" />
          </linearGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        
        <path
          d="M0 480C200 440 400 440 600 470C700 490 800 520 800 520V600H0Z"
          fill="#0f172a" opacity="0.15"
        />
      </svg>"##;

const NEBULA_THREADS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <radialGradient id="bg" cx="0.2" cy="0.8" r="1">
            <stop offset="0" stop-color="#1d4ed8" />
            <stop offset="0.6" stop-color="#0f172a" />
            <stop offset="1" stop-color="#020617" />
          </radialGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        <path
          d="M40 140C140 120 240 140 320 180C400 220 470 300 560 320C660 340 730 300 780 240"
          stroke="#f472b6" stroke-width="6" opacity="0.5" fill="none"
        />
        <path
          d="M20 260C140 240 260 260 350 300C450 344 540 420 660 440C720 450 760 440 790 430"
          stroke="#22d3ee" stroke-width="7" opacity="0.45" fill="none"
        />
        <path
          d="M60 360C180 320 300 330 420 380C520 420 620 500 760 520"
          stroke="#a78bfa" stroke-width="6" opacity="0.4" fill="none"
        />
        
      </svg>"##;

const GOLDEN_HOUR: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <linearGradient id="bg" x1="0" y1="1" x2="0" y2="0">
            <stop offset="0" stop-color="#1c1917" />
            <stop offset="0.4" stop-color="#b45309" />
            <stop offset="0.7" stop-color="#fbbf24" />
            <stop offset="1" stop-color="#fef3c7" />
          </linearGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        
        <path
          d="M0 400C160 360 320 360 480 390C640 420 720 480 800 460V600H0Z"
          fill="#0f172a" opacity="0.3"
        />
        <path
          d="M0 320C140 300 280 300 400 320C520 340 680 380 800 360"
          stroke="#fde68a" stroke-width="4" opacity="0.5" fill="none"
        />
      </svg>"##;

const LAVENDER_MIST: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <radialGradient id="bg" cx="0.7" cy="0.3" r="0.9">
            <stop offset="0" stop-color="#f5f3ff" />
            <stop offset="0.5" stop-color="#c4b5fd" />
            <stop offset="1" stop-color="#4c1d95" />
          </radialGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        
        <path
          d="M80 120C180 100 280 120 380 140"
          stroke="#f8fafc" stroke-width="4" opacity="0.4" fill="none"
        />
      </svg>"##;

const TERRA_MOSAIC: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <linearGradient id="bg" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0" stop-color="#fef3c7" />
            <stop offset="0.5" stop-color="#fb923c" />
            <stop offset="1" stop-color="#1f2937" />
          </linearGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        
      </svg>"##;

const ARCTIC_AURORA: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600">
        <defs>
          <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stop-color="#0f172a" />
            <stop offset="0.5" stop-color="#1e3a5f" />
            <stop offset="1" stop-color="#0c4a6e" />
          </linearGradient>
        </defs>
        <rect width="800" height="600" fill="url(#bg)" />
        <path
          d="M0 200C100 160 200 180 300 200C400 220 500 180 600 160C700 140 800 180 800 180"
          stroke="#4ade80" stroke-width="20" opacity="0.4" fill="none"
        />
        <path
          d="M0 260C120 220 240 240 360 260C480 280 600 240 800 220"
          stroke="#22d3ee" stroke-width="16" opacity="0.35" fill="none"
        />
        <path
          d="M0 320C140 280 280 300 420 320C560 340 700 300 800 280"
          stroke="#a78bfa" stroke-width="12" opacity="0.3" fill="none"
        />
        
      </svg>"##;

pub const PRESETS: [(&str, &str, &str); 14] = [
    ("crimson-wave", "Crimson Wave", CRIMSON_WAVE),
    ("forest-glow", "Forest Glow", FOREST_GLOW),
    ("violet-dune", "Violet Dune", VIOLET_DUNE),
    ("ocean-depth", "Ocean Depth", OCEAN_DEPTH),
    ("rose-garden", "Rose Garden", ROSE_GARDEN),
    ("amber-ridge", "Amber Ridge", AMBER_RIDGE),
    ("mint-frost", "Mint Frost", MINT_FROST),
    ("electric-kite", "Electric Kite", ELECTRIC_KITE),
    ("slate-minimal", "Slate Minimal", SLATE_MINIMAL),
    ("nebula-threads", "Nebula Threads", NEBULA_THREADS),
    ("golden-hour", "Golden Hour", GOLDEN_HOUR),
    ("lavender-mist", "Lavender Mist", LAVENDER_MIST),
    ("terra-mosaic", "Terra Mosaic", TERRA_MOSAIC),
    ("arctic-aurora", "Arctic Aurora", ARCTIC_AURORA),
];

const VIEW_WIDTH: f32 = 800.0;
const VIEW_HEIGHT: f32 = 600.0;
const MAX_DIMENSION: u32 = 8192;
const CACHE_LIMIT: usize = 48;
const TILE_SCALE: f32 = 2.0;

type PixmapCache = HashMap<(&'static str, u32, u32), Option<Arc<Pixmap>>>;
type ImageCache = HashMap<(&'static str, u32), Option<Arc<gpui::RenderImage>>>;

fn entry(id: &str) -> Option<(&'static str, &'static str)> {
    PRESETS
        .iter()
        .find(|(preset_id, _, _)| *preset_id == id)
        .map(|(preset_id, _, source)| (*preset_id, *source))
}

pub fn is_preset(id: &str) -> bool {
    entry(id).is_some()
}

fn dimension(value: f32) -> u32 {
    if !value.is_finite() {
        return 1;
    }
    (value.ceil() as i64).clamp(1, MAX_DIMENSION as i64) as u32
}

pub fn cover_size(width: f32, height: f32) -> (u32, u32) {
    let aspect = VIEW_WIDTH / VIEW_HEIGHT;
    let (width, height) = if aspect > width / height {
        (height * aspect, height)
    } else {
        (width, width / aspect)
    };
    (dimension(width), dimension(height))
}

pub fn pixmap(id: &str, width: u32, height: u32) -> Option<Arc<Pixmap>> {
    static CACHE: Mutex<Option<PixmapCache>> = Mutex::new(None);
    let (id, source) = entry(id)?;
    let width = width.clamp(1, MAX_DIMENSION);
    let height = height.clamp(1, MAX_DIMENSION);
    let mut guard = CACHE.lock().ok()?;
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(cached) = cache.get(&(id, width, height)) {
        return cached.clone();
    }
    if cache.len() >= CACHE_LIMIT {
        cache.clear();
    }
    let rendered = rasterize(source, width, height).map(Arc::new);
    cache.insert((id, width, height), rendered.clone());
    rendered
}

pub fn cover_pixmap(id: &str, width: f32, height: f32) -> Option<Arc<Pixmap>> {
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let (width, height) = cover_size(width, height);
    pixmap(id, width, height)
}

pub fn render_image(id: &str, size: f32) -> Option<Arc<gpui::RenderImage>> {
    static CACHE: Mutex<Option<ImageCache>> = Mutex::new(None);
    let (id, _) = entry(id)?;
    let height = dimension(size * TILE_SCALE);
    let mut guard = CACHE.lock().ok()?;
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(cached) = cache.get(&(id, height)) {
        return cached.clone();
    }
    if cache.len() >= CACHE_LIMIT {
        cache.clear();
    }
    let width = dimension(height as f32 * VIEW_WIDTH / VIEW_HEIGHT);
    let image = pixmap(id, width, height).map(|source| {
        let mut buffer = crate::editor::export::to_rgba(source.as_ref());
        for pixel in buffer.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        Arc::new(gpui::RenderImage::new(smallvec::smallvec![
            image::Frame::new(buffer)
        ]))
    });
    cache.insert((id, height), image.clone());
    image
}

fn rasterize(markup: &str, width: u32, height: u32) -> Option<Pixmap> {
    let tree = usvg::Tree::from_str(markup, &usvg::Options::default()).ok()?;
    let source = tree.size();
    if source.width() <= 0.0 || source.height() <= 0.0 {
        return None;
    }
    let mut pixmap = Pixmap::new(width, height)?;
    let scale = tiny_skia::Transform::from_scale(
        width as f32 / source.width(),
        height as f32 / source.height(),
    );
    resvg::render(&tree, scale, &mut pixmap.as_mut());
    Some(pixmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ELECTRON_PRESETS: [(&str, &str); 14] = [
        ("crimson-wave", "Crimson Wave"),
        ("forest-glow", "Forest Glow"),
        ("violet-dune", "Violet Dune"),
        ("ocean-depth", "Ocean Depth"),
        ("rose-garden", "Rose Garden"),
        ("amber-ridge", "Amber Ridge"),
        ("mint-frost", "Mint Frost"),
        ("electric-kite", "Electric Kite"),
        ("slate-minimal", "Slate Minimal"),
        ("nebula-threads", "Nebula Threads"),
        ("golden-hour", "Golden Hour"),
        ("lavender-mist", "Lavender Mist"),
        ("terra-mosaic", "Terra Mosaic"),
        ("arctic-aurora", "Arctic Aurora"),
    ];

    fn sample(pixmap: &Pixmap, x: u32, y: u32) -> (f32, f32, f32) {
        let pixel = pixmap.pixel(x, y).expect("pixel");
        (
            pixel.red() as f32,
            pixel.green() as f32,
            pixel.blue() as f32,
        )
    }

    fn luminance(color: (f32, f32, f32)) -> f32 {
        0.2126 * color.0 + 0.7152 * color.1 + 0.0722 * color.2
    }

    fn distance(left: (f32, f32, f32), right: (f32, f32, f32)) -> f32 {
        let (dr, dg, db) = (left.0 - right.0, left.1 - right.1, left.2 - right.2);
        (dr * dr + dg * dg + db * db).sqrt()
    }

    #[test]
    fn the_preset_list_matches_electron() {
        assert_eq!(PRESETS.len(), ELECTRON_PRESETS.len());
        for (index, (id, name)) in ELECTRON_PRESETS.iter().enumerate() {
            assert_eq!(PRESETS[index].0, *id);
            assert_eq!(PRESETS[index].1, *name);
        }
    }

    #[test]
    fn every_preset_carries_its_artwork() {
        for (id, _, source) in PRESETS {
            assert!(source.starts_with("<svg"), "{id}");
            assert!(source.contains("viewBox=\"0 0 800 600\""), "{id}");
            assert_eq!(entry(id).map(|(_, found)| found), Some(source));
        }
        assert!(entry("nope").is_none());
        assert!(!is_preset("nope"));
    }

    #[test]
    fn every_preset_rasterizes_at_a_tile_and_an_export_size() {
        for (id, _, _) in PRESETS {
            for (width, height) in [(128u32, 96u32), (1600, 1200)] {
                let rendered = pixmap(id, width, height).expect(id);
                assert_eq!((rendered.width(), rendered.height()), (width, height));
                let first = sample(&rendered, 0, 0);
                let mut opaque = 0usize;
                let mut varied = false;
                for x in (0..width).step_by(7) {
                    for y in (0..height).step_by(7) {
                        let pixel = rendered.pixel(x, y).expect("pixel");
                        if pixel.alpha() == 255 {
                            opaque += 1;
                        }
                        varied |= distance(sample(&rendered, x, y), first) > 8.0;
                    }
                }
                assert!(opaque > 0, "{id}");
                assert!(varied, "{id}");
            }
        }
    }

    #[test]
    fn a_cover_raster_keeps_the_artwork_aspect() {
        assert_eq!(cover_size(800.0, 600.0), (800, 600));
        let (width, height) = cover_size(600.0, 600.0);
        assert_eq!(height, 600);
        assert_eq!(width, 800);
        let (width, height) = cover_size(1600.0, 400.0);
        assert_eq!(width, 1600);
        assert_eq!(height, 1200);
        assert!(cover_pixmap("crimson-wave", 0.0, 10.0).is_none());
        assert!(cover_pixmap("nope", 10.0, 10.0).is_none());
    }

    #[test]
    fn crimson_wave_keeps_its_three_bands_and_its_waves() {
        let rendered = pixmap("crimson-wave", 800, 600).expect("crimson-wave");
        let start = sample(&rendered, 8, 6);
        let middle = sample(&rendered, 400, 300);
        let end = sample(&rendered, 792, 594);
        let two_stop_middle = (
            (start.0 + end.0) / 2.0,
            (start.1 + end.1) / 2.0,
            (start.2 + end.2) / 2.0,
        );
        assert!(
            distance(middle, (220.0, 38.0, 38.0)) < distance(middle, two_stop_middle),
            "{middle:?} vs {two_stop_middle:?}"
        );
        assert!(middle.1 < 70.0, "{middle:?}");

        let mut edges = 0;
        for y in 1..600 {
            let above = sample(&rendered, 400, y - 1);
            let below = sample(&rendered, 400, y);
            if distance(above, below) > 12.0 {
                edges += 1;
            }
        }
        assert!(edges >= 2, "{edges}");
    }

    #[test]
    fn forest_glow_is_brightest_at_its_radial_centre() {
        let rendered = pixmap("forest-glow", 800, 600).expect("forest-glow");
        let centre = (240, 420);
        let mut brightest = (0u32, 0u32);
        let mut best = -1.0f32;
        for x in (4..800).step_by(8) {
            for y in (4..600).step_by(8) {
                let value = luminance(sample(&rendered, x, y));
                if value > best {
                    best = value;
                    brightest = (x, y);
                }
            }
        }
        let dx = brightest.0 as f32 - centre.0 as f32;
        let dy = brightest.1 as f32 - centre.1 as f32;
        assert!((dx * dx + dy * dy).sqrt() < 60.0, "{brightest:?}");
        for corner in [(4u32, 4u32), (796, 4), (4, 596), (796, 596)] {
            assert!(
                luminance(sample(&rendered, corner.0, corner.1)) < best * 0.6,
                "{corner:?}"
            );
        }
    }
}
