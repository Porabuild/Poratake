//! Per-corner `sx` reconciliation for painted parts: an explicit `sx` corner
//! refines a slider track/fill/thumb and a switch track/fill/thumb
//! individually, while an unnamed corner keeps the component's own radius.
//! The per-corner precedence itself is unit-tested on
//! `util::round_sx_corners`.
//!
//! Read off the painted scene: each part is rendered once without `sx` and
//! once with `sx(|s| s.rounded_tl(7))`, the quads are matched by their bounds,
//! and every rounded part must take the 7px top-left corner while its other
//! three corners stay what the component painted.

use crate::harness;

use gpui::{prelude::*, px, AnyElement, TestAppContext};
use harness::{open_host, painted, still, Painted};
use herogpui_components::{Slider, Switch, TabItem, Tabs};

const TL: f32 = 7.;

/// One part's corners, without and with the `sx` corner.
type Pair = ([f32; 4], [f32; 4]);

fn render(cx: &mut TestAppContext, build: fn(bool) -> AnyElement, sx: bool) -> Painted {
    still();
    let vcx = open_host(cx, move || {
        gpui::div()
            .p(px(24.))
            .w(px(300.))
            .child(build(sx))
            .into_any_element()
    });
    painted(vcx)
}

/// The quads that paint a rounded shape by default, each paired with the
/// quad painted at the same bounds under the `sx` corner.
fn refined_parts(cx: &mut TestAppContext, build: fn(bool) -> AnyElement) -> (Painted, Vec<Pair>) {
    let stock = render(cx, build, false);
    let refined = render(cx, build, true);
    let pairs = stock
        .quads
        .iter()
        .filter(|q| stock.corners(q).iter().any(|c| *c > 0.05))
        .map(|q| {
            let twin = refined
                .quads
                .iter()
                .find(|r| r.bounds == q.bounds && r.background == q.background)
                .unwrap_or_else(|| {
                    panic!(
                        "the part at {:?} must paint under sx too\n{}",
                        stock.bounds(q),
                        refined.describe()
                    )
                });
            (stock.corners(q), refined.corners(twin))
        })
        .collect();
    (refined, pairs)
}

fn assert_refined(name: &str, parts: &[Pair], expected: usize) {
    assert_eq!(
        parts.len(),
        expected,
        "{name}: expected {expected} rounded parts, got {parts:?}"
    );
    for (stock, refined) in parts {
        assert!(
            (refined[0] - TL).abs() < 0.05,
            "{name}: every part must take the sx top-left corner ({stock:?} -> {refined:?})"
        );
        // A uniformly rounded part keeps its own radius on the corners sx
        // did not name.
        if stock.iter().all(|c| (c - stock[0]).abs() < 0.05) {
            for i in 1..4 {
                assert!(
                    (refined[i] - stock[i]).abs() < 0.05,
                    "{name}: an unnamed corner must keep the part's own radius \
                     ({stock:?} -> {refined:?})"
                );
            }
        }
    }
}

#[gpui::test]
fn slider_refines_track_fill_and_both_thumb_layers(cx: &mut TestAppContext) {
    let (_, parts) = refined_parts(cx, |sx| {
        let s = Slider::new("sl", 40.);
        if sx {
            s.sx(|d| d.rounded_tl(px(TL)))
        } else {
            s
        }
        .into_any_element()
    });
    assert_refined("Slider", &parts, 4);
}

#[gpui::test]
fn switch_refines_track_fill_and_thumb_inside_its_clip(cx: &mut TestAppContext) {
    let (scene, parts) = refined_parts(cx, |sx| {
        let s = Switch::new("sw").is_selected(true);
        if sx {
            s.sx(|d| d.rounded_tl(px(TL)))
        } else {
            s
        }
        .into_any_element()
    });
    assert_refined("Switch", &parts, 3);

    // The track clips its animated fill, the thumb (and its shadow) and the
    // icons to its bounds: every part inside it carries the track's box as
    // its content mask.
    let track = scene
        .quads
        .iter()
        .max_by(|a, b| a.bounds.size.width.0.total_cmp(&b.bounds.size.width.0))
        .map(|q| scene.bounds(q))
        .expect("the switch must paint its track");
    let inner = scene
        .quads
        .iter()
        .filter(|q| harness::contains(track, scene.bounds(q)))
        .filter(|q| scene.mask(q) == track)
        .count();
    assert!(
        inner >= 2,
        "the fill and thumb must be clipped to the track's bounds\n{}",
        scene.describe()
    );
}

#[gpui::test]
fn tabs_indicator_refines_its_corners(cx: &mut TestAppContext) {
    let stock = render(cx, tabs, false);
    let refined = render(cx, tabs, true);
    let colors = |p: &Painted| {
        p.quads
            .iter()
            .map(|q| (q.bounds, q.background.as_solid()))
            .collect::<Vec<_>>()
    };
    assert_eq!(colors(&stock), colors(&refined), "sx must not move a part");
    // The indicator is the selected tab's pill, the second of the two quads
    // (the tray paints first).
    let indicator = |p: &Painted| p.corners(p.quads.last().expect("the indicator must paint"));
    let (before, after) = (indicator(&stock), indicator(&refined));
    assert!(
        (after[0] - TL).abs() < 0.05 && (after[1] - before[1]).abs() < 0.05,
        "the Tabs indicator must take the sx corner and keep the rest: \
         {before:?} -> {after:?}"
    );
}

fn tabs(sx: bool) -> AnyElement {
    let t = Tabs::new(
        "tb",
        vec![TabItem::new("a", "A"), TabItem::new("b", "B")],
        "a",
    );
    if sx {
        t.sx(|d| d.rounded_tl(px(TL)))
    } else {
        t
    }
    .into_any_element()
}
