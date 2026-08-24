//! Port of `composition/cursor-svg-data.ts`. The renderer builds an SVG per
//! cursor type and colour pair and rasterizes it; the same SVG markup is used
//! here and rendered with `resvg`, so the drawn pointer is identical.

use std::collections::HashMap;
use std::sync::Mutex;

use tiny_skia::Pixmap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hotspot {
    pub x: f64,
    pub y: f64,
}

struct Definition {
    view_box: &'static str,
    hotspot: Hotspot,
    body: &'static str,
}

/// `{fill}` and `{stroke}` stand in for the two colour placeholders the
/// renderer interpolates.
const ARROW: &str = r#"
      <g stroke-linejoin="round" stroke-linecap="round">
        <polygon fill="{stroke}" stroke="{stroke}" stroke-width="1.1" points="8.2,20.9 8.2,4.9 19.8,16.5 13,16.5 12.6,16.6"/>
        <polygon fill="{stroke}" stroke="{stroke}" stroke-width="1.1" points="17.3,21.6 13.7,23.1 9,12 12.7,10.5"/>
        <rect x="12.5" y="13.6" transform="matrix(0.9221 -0.3871 0.3871 0.9221 -5.7605 6.5909)" width="2" height="8" fill="{fill}"/>
        <polygon fill="{fill}" points="9.2,7.3 9.2,18.5 12.2,15.6 12.6,15.5 17.4,15.5"/>
      </g>
    "#;

const POINTING_HAND: &str = r#"
      <path fill="{fill}" stroke="{stroke}" stroke-width="0.5" stroke-linecap="round" stroke-linejoin="round" d="M11.3,20.4c-0.3-0.4-0.6-1.1-1.2-2c-0.3-0.5-1.2-1.5-1.5-1.9c-0.2-0.4-0.2-0.6-0.1-1c0.1-0.6,0.7-1.1,1.4-1.1c0.5,0,1,0.4,1.4,0.7c0.2,0.2,0.5,0.6,0.7,0.8c0.2,0.2,0.2,0.3,0.4,0.5c0.2,0.3,0.3,0.5,0.2,0.1c-0.1-0.5-0.2-1.3-0.4-2.1c-0.1-0.6-0.2-0.7-0.3-1.1c-0.1-0.5-0.2-0.8-0.3-1.3c-0.1-0.3-0.2-1.1-0.3-1.5c-0.1-0.5-0.1-1.4,0.3-1.8c0.3-0.3,0.9-0.4,1.3-0.2c0.5,0.3,0.8,1,0.9,1.3c0.2,0.5,0.4,1.2,0.5,2c0.2,1,0.5,2.5,0.5,2.8c0-0.4-0.1-1.1,0-1.5c0.1-0.3,0.3-0.7,0.7-0.8c0.3-0.1,0.6-0.1,0.9-0.1c0.3,0.1,0.6,0.3,0.8,0.5c0.4,0.6,0.4,1.9,0.4,1.8c0.1-0.4,0.1-1.2,0.3-1.6c0.1-0.2,0.5-0.4,0.7-0.5c0.3-0.1,0.7-0.1,1,0c0.2,0,0.6,0.3,0.7,0.5c0.2,0.3,0.3,1.3,0.4,1.7c0,0.1,0.1-0.4,0.3-0.7c0.4-0.6,1.8-0.8,1.9,0.6c0,0.7,0,0.6,0,1.1c0,0.5,0,0.8,0,1.2c0,0.4-0.1,1.3-0.2,1.7c-0.1,0.3-0.4,1-0.7,1.4c0,0-1.1,1.2-1.2,1.8c-0.1,0.6-0.1,0.6-0.1,1c0,0.4,0.1,0.9,0.1,0.9s-0.8,0.1-1.2,0c-0.4-0.1-0.9-0.8-1-1.1c-0.2-0.3-0.5-0.3-0.7,0c-0.2,0.4-0.7,1.1-1.1,1.1c-0.7,0.1-2.1,0-3.1,0c0,0,0.2-1-0.2-1.4c-0.3-0.3-0.8-0.8-1.1-1.1L11.3,20.4z"/>
    "#;

const OPEN_HAND: &str = r#"
      <path fill="{fill}" stroke="{stroke}" stroke-width="0.5" stroke-linecap="round" stroke-linejoin="round" d="M12.6,16.6c-0.1-0.4-0.2-0.8-0.4-1.6c-0.2-0.6-0.3-0.9-0.5-1.2c-0.2-0.5-0.3-0.7-0.5-1.2c-0.1-0.3-0.4-1-0.5-1.4c-0.1-0.5,0-0.9,0.2-1.2c0.3-0.3,1-0.5,1.4-0.4c0.4,0.1,0.7,0.5,0.9,0.8c0.3,0.5,0.4,0.6,0.7,1.5c0.4,1,0.6,1.9,0.6,2.2l0.1,0.5c0,0,0-1.1,0-1.2c0-1-0.1-1.8,0-2.9c0-0.1,0.1-0.6,0.1-0.7c0.1-0.5,0.3-0.8,0.7-1c0.4-0.2,0.9-0.2,1.4,0c0.4,0.2,0.6,0.5,0.7,1c0,0.1,0.1,1,0.1,1.1c0,1,0,1.6,0,2.2c0,0.2,0,1.6,0,1.5c0.1-0.7,0.1-3.2,0.3-3.9c0.1-0.4,0.4-0.7,0.8-0.9c0.4-0.2,1.1-0.1,1.4,0.2c0.3,0.3,0.4,0.7,0.5,1.2c0,0.4,0,0.9,0,1.2c0,0.9,0,1.3,0,2.1c0,0,0,0.3,0,0.2c0.1-0.3,0.2-0.5,0.3-0.7c0-0.1,0.2-0.6,0.4-0.9c0.1-0.2,0.2-0.4,0.4-0.7c0.2-0.3,0.4-0.4,0.7-0.6c0.5-0.2,1.1,0.1,1.3,0.6c0.1,0.2,0,0.7,0,1.1c-0.1,0.6-0.3,1.3-0.4,1.6c-0.1,0.4-0.3,1.2-0.3,1.6c-0.1,0.4-0.2,1.4-0.4,1.8c-0.1,0.3-0.4,1-0.7,1.4c0,0-1.1,1.2-1.2,1.8c-0.1,0.6-0.1,0.6-0.1,1c0,0.4,0.1,0.9,0.1,0.9s-0.8,0.1-1.2,0c-0.4-0.1-0.9-0.8-1-1.1c-0.2-0.3-0.5-0.3-0.7,0c-0.2,0.4-0.7,1.1-1.1,1.1c-0.7,0.1-2.1,0-3.1,0c0,0,0.2-1-0.2-1.4c-0.3-0.3-0.8-0.8-1.1-1.1l-0.8-0.9c-0.3-0.4-0.6-1.1-1.2-2c-0.3-0.5-1-1.1-1.3-1.6c-0.2-0.4-0.3-1-0.2-1.3c0.2-0.6,0.7-0.9,1.4-0.8c0.5,0,0.8,0.2,1.2,0.5c0.2,0.2,0.6,0.5,0.8,0.7c0.2,0.2,0.2,0.3,0.4,0.5C12.6,16.8,12.6,16.9,12.6,16.6"/>
    "#;

const CLOSED_HAND: &str = r#"
      <path fill="{fill}" stroke="{stroke}" stroke-width="0.5" stroke-linejoin="round" d="M12.6,13c0.5-0.2,1.4-0.1,1.7,0.5c0.2,0.5,0.4,1.2,0.4,1.1c0-0.4,0-1.2,0.1-1.6c0.1-0.3,0.3-0.6,0.7-0.7c0.3-0.1,0.6-0.1,0.9-0.1c0.3,0.1,0.6,0.3,0.8,0.5c0.4,0.6,0.4,1.9,0.4,1.8c0.1-0.3,0.1-1.2,0.3-1.6c0.1-0.2,0.5-0.4,0.7-0.5c0.3-0.1,0.7-0.1,1,0c0.2,0,0.6,0.3,0.7,0.5c0.2,0.3,0.3,1.3,0.4,1.7c0,0.1,0.1-0.4,0.3-0.7c0.4-0.6,1.8-0.8,1.9,0.6c0,0.7,0,0.6,0,1.1c0,0.5,0,0.8,0,1.2c0,0.4-0.1,1.3-0.2,1.7c-0.1,0.3-0.4,1-0.7,1.4c0,0-1.1,1.2-1.2,1.8c-0.1,0.6-0.1,0.6-0.1,1c0,0.4,0.1,0.9,0.1,0.9s-0.8,0.1-1.2,0c-0.4-0.1-0.9-0.8-1-1.1c-0.2-0.3-0.5-0.3-0.7,0c-0.2,0.4-0.7,1.1-1,1.1c-0.7,0.1-2.1,0-3.1,0c0,0,0.2-1-0.2-1.4c-0.3-0.3-0.8-0.8-1.1-1.1l-0.8-0.9c-0.3-0.4-1-0.9-1.2-2c-0.2-0.9-0.2-1.4,0-1.8c0.2-0.4,0.7-0.6,0.9-0.6c0.2,0,0.7,0,0.9,0.1c0.2,0.1,0.3,0.2,0.5,0.4c0.2,0.3,0.3,0.5,0.2,0.1c-0.1-0.3-0.3-0.6-0.4-1c-0.1-0.4-0.4-0.9-0.4-1.5C11.7,13.9,11.8,13.3,12.6,13z"/>
    "#;

const I_BEAM: &str = r#"
      <g transform="translate(13 8)">
        <path fill="{fill}" stroke="{stroke}" stroke-linejoin="round" d="m6.12306605-.48331676c.43304536-.02942018.89723494-.01586641 1.23506765.01110645l.09836607 2.00031187c-.52088553-.02633116-.86402421-.03615261-1.16297111-.01823278-.57216322.1246759-.83397559.26379885-1.13476879.47262866-.20678677.14251521-.54543639.60542479-.68837291.9244994v4.53970853h.998v1.984h-.998v3.57686113c.14285978.3186299.48159131.7805976.69278827.9256073.28177652.1964385.52739561.3374074.74486623.4121252.92617851.1117241.86141186.0439655 1.38886347.0608526l.24148845 1.9771822c-.63869922.0316331-1.03914186.0381475-1.41606129.0122618-.31198863-.0214264-.57343006-.0643378-.77405544-.129216-.41684626-.1296585-.85258908-.3604995-1.32295099-.6884424-.16852556-.1156905-.3571101-.2906327-.54285865-.4981462-.17040902.2017941-.33955796.3725205-.48392771.4855461-.40405946.3138676-.86631905.544191-1.35971316.6990619-.21164232.068207-.47249574.1108318-.78353769.1322303-.43450358.0298922-.90002831.0163041-1.23810501-.010835l-.09691742-2.0002571c.51770616.0263348.86168069.0362399 1.16109487.0181487.6186734-.1394818.87678125-.2519735 1.08726671-.4154673.19712987-.1543365.58456002-.6802379.70170954-.9838289l.01232275-3.57368433h-1.00027293v-1.984h1.002v-4.53315423c-.13203427-.30699537-.51655024-.83390163-.71128328-.98567507-.2013222-.15578504-.43877755-.27368925-.70166256-.36109174-.92774648-.11104657-.86334532-.04378063-1.3908609-.06035307l-.24301648-1.97729129c.64057546-.03160079 1.04058994-.03810767 1.41699023-.01249118.31067274.02114331.57118123.06339129.7795926.13006754.50016024.15848101.96025701.38783055 1.36565801.70154178.14340254.11176212.31252725.28238091.4832615.48453047.18451625-.20650555.37147758-.38041205.53791304-.49511426.47467546-.32956039.90822271-.55967002 1.31895768-.68980909.21133301-.06776711.4720386-.11004715.78312925-.13118199z"/>
      </g>
    "#;

const CROSSHAIR: &str = r#"
      <path d="M5 16h22M16 5v22" stroke="{stroke}" stroke-width="3" stroke-linecap="round"/>
      <path d="M5 16h22M16 5v22" stroke="{fill}" stroke-width="1" stroke-linecap="round"/>
    "#;

const RESIZE_UP_DOWN: &str = r#"
      <g transform="translate(8 7)" stroke-linejoin="round" stroke-linecap="round">
        <path fill="{stroke}" stroke="{stroke}" stroke-width="1.1" d="m7.988 0-5.461 5.962h3.478v1.038h-6.005v.019 3.942.02h6.005v1.058h-3.466l5.472 5.961 5.462-5.961h-3.478v-1.058h6.005v-3.981h-6.006v-1.038h3.467z"/>
        <path fill="{fill}" d="m14.961 8.02h-5.981v-3.039h2.26l-3.251-3.32-3.223 3.32h2.254v3.039h-5.98-.02v1.96h.02 6v3.04h-2.28l3.251 3.32 3.223-3.32h-2.253v-3.04h5.98.02v-1.96z"/>
      </g>
    "#;

fn definition(cursor_type: &str) -> &'static Definition {
    // `resizeLeftRight` reuses the arrow artwork, as it does in the renderer.
    const ARROW_DEF: Definition = Definition {
        view_box: "0 0 28 28",
        hotspot: Hotspot { x: 0.29, y: 0.18 },
        body: ARROW,
    };
    const POINTING_HAND_DEF: Definition = Definition {
        view_box: "0 0 32 32",
        hotspot: Hotspot { x: 0.35, y: 0.15 },
        body: POINTING_HAND,
    };
    const OPEN_HAND_DEF: Definition = Definition {
        view_box: "0 0 32 32",
        hotspot: Hotspot { x: 0.5, y: 0.4 },
        body: OPEN_HAND,
    };
    const CLOSED_HAND_DEF: Definition = Definition {
        view_box: "0 0 32 32",
        hotspot: Hotspot { x: 0.5, y: 0.45 },
        body: CLOSED_HAND,
    };
    const I_BEAM_DEF: Definition = Definition {
        view_box: "0 0 32 32",
        hotspot: Hotspot { x: 0.5, y: 0.5 },
        body: I_BEAM,
    };
    const CROSSHAIR_DEF: Definition = Definition {
        view_box: "0 0 32 32",
        hotspot: Hotspot { x: 0.5, y: 0.5 },
        body: CROSSHAIR,
    };
    const RESIZE_LEFT_RIGHT_DEF: Definition = Definition {
        view_box: "0 0 32 32",
        hotspot: Hotspot { x: 0.5, y: 0.5 },
        body: ARROW,
    };
    const RESIZE_UP_DOWN_DEF: Definition = Definition {
        view_box: "0 0 32 32",
        hotspot: Hotspot { x: 0.5, y: 0.5 },
        body: RESIZE_UP_DOWN,
    };

    match cursor_type {
        "pointingHand" => &POINTING_HAND_DEF,
        "openHand" => &OPEN_HAND_DEF,
        "closedHand" => &CLOSED_HAND_DEF,
        "iBeam" => &I_BEAM_DEF,
        "crosshair" => &CROSSHAIR_DEF,
        "resizeLeftRight" => &RESIZE_LEFT_RIGHT_DEF,
        "resizeUpDown" => &RESIZE_UP_DOWN_DEF,
        _ => &ARROW_DEF,
    }
}

/// `getCursorHotspot` — the fraction of the sprite the pointer tip sits at.
pub fn hotspot(cursor_type: &str) -> Hotspot {
    definition(cursor_type).hotspot
}

/// `generateCursorSvg`.
pub fn svg(cursor_type: &str, fill: &str, stroke: &str) -> String {
    let definition = definition(cursor_type);
    let body = definition
        .body
        .replace("{fill}", &escape(fill))
        .replace("{stroke}", &escape(stroke));
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{}">{body}</svg>"#,
        definition.view_box
    )
}

/// Colours come from user settings, so they are escaped before being spliced
/// into markup.
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Rasterizes a cursor sprite at `size` pixels square, caching by type, colour
/// pair and size the way the renderer caches its `Image` objects.
pub fn sprite(cursor_type: &str, fill: &str, stroke: &str, size: u32) -> Option<Pixmap> {
    static CACHE: Mutex<Option<HashMap<String, Option<Pixmap>>>> = Mutex::new(None);

    let size = size.clamp(1, 1024);
    let key = format!("{cursor_type}:{fill}:{stroke}:{size}");
    let mut guard = CACHE.lock().ok()?;
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(cached) = cache.get(&key) {
        return cached.clone();
    }
    let rendered = rasterize(&svg(cursor_type, fill, stroke), size);
    cache.insert(key, rendered.clone());
    rendered
}

fn rasterize(markup: &str, size: u32) -> Option<Pixmap> {
    let tree = usvg::Tree::from_str(markup, &usvg::Options::default()).ok()?;
    let source = tree.size();
    if source.width() <= 0.0 || source.height() <= 0.0 {
        return None;
    }
    let mut pixmap = Pixmap::new(size, size)?;
    let scale = tiny_skia::Transform::from_scale(
        size as f32 / source.width(),
        size as f32 / source.height(),
    );
    resvg::render(&tree, scale, &mut pixmap.as_mut());
    Some(pixmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cursor_type_has_a_definition() {
        for cursor_type in [
            "arrow",
            "pointingHand",
            "openHand",
            "closedHand",
            "iBeam",
            "crosshair",
            "resizeLeftRight",
            "resizeUpDown",
        ] {
            let markup = svg(cursor_type, "#000000", "#ffffff");
            assert!(markup.starts_with("<svg"), "{cursor_type}");
            assert!(!markup.contains("{fill}"), "{cursor_type}");
            assert!(!markup.contains("{stroke}"), "{cursor_type}");
        }
    }

    #[test]
    fn an_unknown_type_falls_back_to_the_arrow() {
        assert_eq!(hotspot("nope"), hotspot("arrow"));
        assert!(svg("nope", "#000", "#fff").contains("viewBox=\"0 0 28 28\""));
    }

    #[test]
    fn colours_are_escaped_before_they_reach_the_markup() {
        let markup = svg("arrow", "\"/><script>x", "#fff");
        assert!(!markup.contains("<script>"));
        assert!(markup.contains("&lt;script&gt;"));
    }

    #[test]
    fn rasterizing_produces_visible_coverage() {
        let sprite = sprite("arrow", "#000000", "#ffffff", 49).expect("sprite");
        let covered = sprite
            .data()
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 0)
            .count();
        assert!(covered > 100, "{covered}");
    }

    #[test]
    fn sprites_are_cached_per_key() {
        let first = sprite("crosshair", "#ff0000", "#00ff00", 32).expect("first");
        let second = sprite("crosshair", "#ff0000", "#00ff00", 32).expect("second");
        assert_eq!(first.data(), second.data());
    }
}
