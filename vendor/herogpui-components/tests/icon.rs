//! `IconName` / `Icon` (HeroGPUI extension): every name resolves through
//! `HeroGpuiAssets` to an embedded Lucide SVG that gpui's own renderer
//! rasterizes to visible pixels, the names, paths and files agree one to one,
//! and `Icon` lays out at its size inside a real window.

use crate::harness;

use std::{collections::BTreeSet, sync::Arc};

use gpui::{prelude::*, px, AssetSource as _, SharedString, SvgRenderer, TestAppContext};
use harness::open_host;
use herogpui_components::{
    icons, HeroGpuiAssets, Icon, IconName, IconSize, Sizable, DEFAULT_ICON_SIZE,
    LUCIDE_ICON_PREFIX, LUCIDE_VERSION,
};

const ASSET_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/lucide");

#[test]
fn names_are_sorted_unique_and_round_trip() {
    let names: Vec<&str> = IconName::ALL.iter().map(|icon| icon.name()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        names, sorted,
        "IconName::ALL must be unique and in name order"
    );
    assert!(names.len() >= 100, "only {} icons", names.len());
    for &icon in IconName::ALL {
        assert_eq!(IconName::from_name(icon.name()), Some(icon));
        assert_eq!(IconName::from_path(icon.path()), Some(icon));
        assert_eq!(
            icon.path(),
            format!("{LUCIDE_ICON_PREFIX}/{}.svg", icon.name())
        );
        assert_eq!(SharedString::from(icon).as_ref(), icon.path());
    }
    assert_eq!(IconName::from_name("not-an-icon"), None);
    assert_eq!(IconName::from_path("herogpui/icons/check.svg"), None);
    assert_eq!(IconName::from_path(icons::CHECK), None);
}

/// A variant without a file fails to compile (`include_bytes!`); this is the
/// other direction — a file nobody can name, or a stray upstream copy.
#[test]
fn every_asset_file_has_a_variant() {
    let mut files = BTreeSet::new();
    for entry in std::fs::read_dir(ASSET_DIR).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        if name == "LICENSE" {
            continue;
        }
        let stem = name
            .strip_suffix(".svg")
            .unwrap_or_else(|| panic!("unexpected file {name} in assets/lucide"));
        files.insert(stem.to_owned());
    }
    let names: BTreeSet<String> = IconName::ALL
        .iter()
        .map(|icon| icon.name().to_owned())
        .collect();
    assert_eq!(files, names);
}

#[test]
fn every_icon_is_the_pinned_lucide_file() {
    let license = std::fs::read_to_string(format!("{ASSET_DIR}/LICENSE")).unwrap();
    assert!(license.starts_with("ISC License"));
    assert!(
        license.contains("Feather"),
        "the Feather MIT notice is part of the license"
    );
    let header = format!("<!-- @license lucide-static v{LUCIDE_VERSION} - ISC -->");
    for &icon in IconName::ALL {
        let svg = std::str::from_utf8(icon.svg()).unwrap();
        assert!(
            svg.starts_with(&header),
            "{} is not from v{LUCIDE_VERSION}",
            icon.name()
        );
        assert!(
            svg.contains(&format!("class=\"lucide lucide-{}\"", icon.name())),
            "{} is an alias file, not the canonical icon",
            icon.name()
        );
    }
}

/// Through the asset source a downstream app registers, and through gpui's own
/// SVG renderer: every icon loads, parses and paints at least one pixel.
#[test]
fn every_icon_loads_and_rasterizes() {
    let assets = HeroGpuiAssets;
    let renderer = SvgRenderer::new(Arc::new(HeroGpuiAssets));
    for &icon in IconName::ALL {
        let bytes = assets
            .load(icon.path())
            .unwrap()
            .unwrap_or_else(|| panic!("{} is not served", icon.path()));
        assert_eq!(&*bytes, icon.svg());
        let image = renderer
            .render_single_frame(&bytes, 1.0)
            .unwrap_or_else(|err| panic!("{} does not parse: {err}", icon.name()));
        let pixels = image.as_bytes(0).unwrap();
        assert!(
            pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] > 0),
            "{} rasterizes to nothing",
            icon.name()
        );
    }
    // The fallback wrapper keeps serving them ahead of the app's own source.
    let wrapped = HeroGpuiAssets::with_fallback(());
    assert!(wrapped.load(IconName::Search.path()).unwrap().is_some());
}

#[gpui::test]
fn icons_lay_out_at_their_size(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                gpui::div()
                    .flex()
                    // No cross-axis stretch, so each probe hugs its icon.
                    .items_start()
                    .child(
                        gpui::div()
                            .flex()
                            .debug_selector(|| "default".into())
                            .child(Icon::new(IconName::Search)),
                    )
                    .child(
                        gpui::div()
                            .flex()
                            .debug_selector(|| "sized".into())
                            .child(
                                Icon::new(IconName::Heart)
                                    .size(px(24.))
                                    .color(gpui::red()),
                            ),
                    )
                    .child(
                        gpui::div()
                            .flex()
                            .debug_selector(|| "chrome".into())
                            .child(Icon::from_path(icons::CHECK).size(px(12.))),
                    ),
            )
            // Every icon at once, as a smoke that none of them panics a frame.
            .child(
                gpui::div()
                    .flex()
                    .flex_wrap()
                    .children(IconName::ALL.iter().map(|&icon| Icon::from(icon))),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let default = cx.debug_bounds("default").unwrap();
    assert_eq!(default.size.width, DEFAULT_ICON_SIZE);
    assert_eq!(default.size.height, DEFAULT_ICON_SIZE);
    let sized = cx.debug_bounds("sized").unwrap();
    assert_eq!((sized.size.width, sized.size.height), (px(24.), px(24.)));
    let chrome = cx.debug_bounds("chrome").unwrap();
    assert_eq!((chrome.size.width, chrome.size.height), (px(12.), px(12.)));
}

#[test]
fn icon_size_steps_are_the_tailwind_squares() {
    let steps: Vec<f32> = IconSize::ALL
        .iter()
        .map(|step| f32::from(step.pixels()))
        .collect();
    assert_eq!(steps, [12., 14., 16., 20., 24.]);
    assert_eq!(IconSize::default().pixels(), DEFAULT_ICON_SIZE);
    assert_eq!(gpui::Pixels::from(IconSize::Lg), px(20.));
}

/// Through the asset source: a stroke-width query serves the same Lucide
/// file with only its root `stroke-width` rewritten, and a wider stroke
/// rasterises to more ink.
#[test]
fn a_stroke_width_query_serves_the_rewritten_file() {
    let assets = HeroGpuiAssets;
    let path = IconName::Circle.path_with_stroke_width(1.);
    let served = assets
        .load(&path)
        .unwrap()
        .expect("the stroked path is served");
    let original = std::str::from_utf8(IconName::Circle.svg()).unwrap();
    assert_eq!(
        std::str::from_utf8(&served).unwrap(),
        original.replacen("stroke-width=\"2\"", "stroke-width=\"1\"", 1)
    );
    // Through the fallback wrapper an app registers, too.
    assert!(HeroGpuiAssets::with_fallback(())
        .load(&path)
        .unwrap()
        .is_some());

    let renderer = SvgRenderer::new(Arc::new(HeroGpuiAssets));
    let ink = |width: f32| -> u64 {
        let bytes = assets
            .load(&IconName::Circle.path_with_stroke_width(width))
            .unwrap()
            .unwrap();
        let image = renderer.render_single_frame(&bytes, 1.0).unwrap();
        image
            .as_bytes(0)
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| u64::from(pixel[3]))
            .sum()
    };
    let (thin, normal, thick) = (ink(1.), ink(2.), ink(3.));
    assert!(thin < normal && normal < thick, "{thin} {normal} {thick}");
}

fn sized<T: Sizable<Size = IconSize>>(component: T, size: IconSize) -> T {
    component.size(size)
}

#[gpui::test]
fn icons_take_a_sizable_step(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .items_start()
            .child(
                gpui::div()
                    .flex()
                    .debug_selector(|| "step".into())
                    .child(Icon::new(IconName::Star).size(IconSize::Lg)),
            )
            .child(
                gpui::div()
                    .flex()
                    .debug_selector(|| "trait".into())
                    .child(sized(Icon::new(IconName::Star), IconSize::Xs)),
            )
            .child(
                gpui::div()
                    .flex()
                    .debug_selector(|| "stroked".into())
                    .child(
                        Icon::new(IconName::Star)
                            .size(IconSize::Xl)
                            .stroke_width(1.)
                            .absolute_stroke_width(true),
                    ),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let step = cx.debug_bounds("step").unwrap();
    assert_eq!((step.size.width, step.size.height), (px(20.), px(20.)));
    let via_trait = cx.debug_bounds("trait").unwrap();
    assert_eq!(via_trait.size.width, px(12.));
    let stroked = cx.debug_bounds("stroked").unwrap();
    assert_eq!(stroked.size.width, px(24.));
}
