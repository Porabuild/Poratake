//! Headless coverage for the `Scrollbar`'s instance-level styling API:
//! track thickness, inset, radius, thumb fills, `auto_hide(false)` and the
//! `sx` slot, asserted against the quads the overlay actually paints
//! (`Window::painted_quads`).
//!
//! One platform fact bounds what the harness can reach: the test platform
//! answers `should_auto_hide_scrollbars()` with `false` and cannot be
//! flipped, so the platform-hiding path of the visibility decision is
//! table-tested inline in `scrollbar.rs`; here `auto_hide(false)` proves the
//! builder keeps the thumb painting with unchanged geometry.

use crate::harness;

use std::{cell::Cell, rc::Rc};

use gpui::{
    hsla, point, prelude::*, px, AnyElement, Hsla, Modifiers, Quad, ScrollHandle, TestAppContext,
    VisualTestContext,
};
use herogpui_components::{Orientation, Scrollbar};
use herogpui_theme::ActiveTheme;

use harness::open_host;

/// A 300x200 viewport over 1000px of content: the vertical thumb is 40px
/// tall with 160px of travel, and the track hugs the right edge.
const TRACK_W: f32 = 300.;
const VIEWPORT: f32 = 200.;
const CONTENT: f32 = 1000.;
/// A 160x40 viewport over 400px of content: the horizontal thumb is 64px
/// wide with 96px of travel, and the track hugs the bottom edge.
const H_TRACK_W: f32 = 160.;
const H_VIEWPORT: f32 = 40.;
const H_CONTENT: f32 = 400.;

fn vertical_bar(handle: ScrollHandle, bar: Scrollbar) -> AnyElement {
    gpui::div()
        .relative()
        .w(px(TRACK_W))
        .h(px(VIEWPORT))
        .child(
            gpui::div()
                .id("api-scroller")
                .track_scroll(&handle)
                .size_full()
                .overflow_y_scroll()
                .child(gpui::div().h(px(CONTENT)).w_full()),
        )
        .child(bar)
        .into_any_element()
}

fn horizontal_bar(handle: ScrollHandle, bar: Scrollbar) -> AnyElement {
    gpui::div()
        .relative()
        .w(px(H_TRACK_W))
        .h(px(H_VIEWPORT))
        .child(
            gpui::div()
                .id("api-h-scroller")
                .track_scroll(&handle)
                .size_full()
                .overflow_x_scroll()
                .child(gpui::div().w(px(H_CONTENT)).h_full()),
        )
        .child(bar)
        .into_any_element()
}

/// Every solid quad in the last rendered frame. Plain scroller chrome paints
/// nothing, so these are the bar's own paint: one thumb quad, or a gutter
/// plus a thumb once an `sx` background is in play.
fn solid_quads(cx: &mut VisualTestContext) -> Vec<Quad> {
    cx.update(|window, _| {
        window
            .painted_quads()
            .into_iter()
            .filter(|quad| quad.background.as_solid().is_some())
            .collect()
    })
}

fn only_quad(cx: &mut VisualTestContext, label: &str) -> Quad {
    let quads = solid_quads(cx);
    assert_eq!(quads.len(), 1, "{label}: expected exactly one painted quad");
    quads[0]
}

/// The window's device scale factor. Painted quads report scaled pixels,
/// while every geometry number in these tests is a CSS pixel.
fn scale_of(cx: &mut VisualTestContext) -> f32 {
    cx.update(|window, _| window.scale_factor())
}

#[allow(clippy::too_many_arguments)] // one parameter per asserted dimension
fn assert_thumb(label: &str, quad: &Quad, scale: f32, x: f32, y: f32, w: f32, h: f32, radius: f32) {
    let close = |what: &str, actual: f32, expected: f32| {
        assert!(
            (actual - expected).abs() < 0.5,
            "{label} {what}: expected {expected}, got {actual}"
        );
    };
    close("origin.x", quad.bounds.origin.x.0, x * scale);
    close("origin.y", quad.bounds.origin.y.0, y * scale);
    close("size.width", quad.bounds.size.width.0, w * scale);
    close("size.height", quad.bounds.size.height.0, h * scale);
    close(
        "radius top_left",
        quad.corner_radii.top_left.0,
        radius * scale,
    );
    close(
        "radius top_right",
        quad.corner_radii.top_right.0,
        radius * scale,
    );
    close(
        "radius bottom_right",
        quad.corner_radii.bottom_right.0,
        radius * scale,
    );
    close(
        "radius bottom_left",
        quad.corner_radii.bottom_left.0,
        radius * scale,
    );
}

fn assert_fill(label: &str, quad: &Quad, color: Hsla) {
    let actual = quad.background.as_solid().expect("a solid thumb fill");
    assert_eq!(actual, color, "{label}: unexpected thumb fill");
}

/// A point inside the 8px vertical track the host draws at the window
/// origin — NOT the parked spot `open_host` starts the pointer at
/// (-100,-100), so a move here is a real crossing onto the track.
const INSIDE_TRACK: (f32, f32) = (TRACK_W - 4., 100.);

#[gpui::test]
fn default_geometry_and_token_fill_are_unchanged(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let scrolled = handle.clone();
    let cx = open_host(cx, move || {
        let scroller = handle.clone();
        vertical_bar(scroller, Scrollbar::new("bar", handle.clone()))
    });
    cx.update(|window, _| window.refresh());
    let scale = scale_of(cx);

    let thumb_len = VIEWPORT * (VIEWPORT / CONTENT);
    let quad = only_quad(cx, "default vertical");
    assert_thumb(
        "default vertical",
        &quad,
        scale,
        TRACK_W - 8.,
        0.,
        8.,
        thumb_len,
        4.,
    );
    let token = cx.update(|_, app| app.colors().scrollbar);
    assert_fill("default vertical", &quad, token);

    // The thumb tracks the handle: 200 of the 800px range is a quarter of
    // the 160px travel.
    scrolled.set_offset(point(px(0.), px(-200.)));
    cx.update(|window, _| window.refresh());
    let quad = only_quad(cx, "default vertical, scrolled");
    assert_thumb(
        "default vertical, scrolled",
        &quad,
        scale,
        TRACK_W - 8.,
        40.,
        8.,
        thumb_len,
        4.,
    );
}

#[gpui::test]
fn horizontal_default_matches_the_vertical_geometry_rules(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let cx = open_host(cx, move || {
        let scroller = handle.clone();
        horizontal_bar(
            scroller,
            Scrollbar::new("h-bar", handle.clone()).orientation(Orientation::Horizontal),
        )
    });
    cx.update(|window, _| window.refresh());
    let scale = scale_of(cx);

    let thumb_len = H_TRACK_W * (H_TRACK_W / H_CONTENT);
    let quad = only_quad(cx, "default horizontal");
    assert_thumb(
        "default horizontal",
        &quad,
        scale,
        0.,
        H_VIEWPORT - 8.,
        thumb_len,
        8.,
        4.,
    );
}

#[gpui::test]
fn track_override_resizes_the_track_and_rederives_the_radius(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let cx = open_host(cx, move || {
        let scroller = handle.clone();
        vertical_bar(
            scroller,
            Scrollbar::new("bar", handle.clone()).track(px(12.)),
        )
    });
    cx.update(|window, _| window.refresh());
    let scale = scale_of(cx);

    let thumb_len = VIEWPORT * (VIEWPORT / CONTENT);
    let quad = only_quad(cx, "track 12");
    assert_thumb(
        "track 12",
        &quad,
        scale,
        TRACK_W - 12.,
        0.,
        12.,
        thumb_len,
        6.,
    );
}

#[gpui::test]
fn horizontal_track_override_rederives_its_radius(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let cx = open_host(cx, move || {
        let scroller = handle.clone();
        horizontal_bar(
            scroller,
            Scrollbar::new("h-bar", handle.clone())
                .orientation(Orientation::Horizontal)
                .track(px(12.)),
        )
    });
    cx.update(|window, _| window.refresh());
    let scale = scale_of(cx);

    let thumb_len = H_TRACK_W * (H_TRACK_W / H_CONTENT);
    let quad = only_quad(cx, "horizontal track 12");
    assert_thumb(
        "horizontal track 12",
        &quad,
        scale,
        0.,
        H_VIEWPORT - 12.,
        thumb_len,
        12.,
        6.,
    );
}

#[gpui::test]
fn radius_override_shapes_the_thumb(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let cx = open_host(cx, move || {
        let scroller = handle.clone();
        vertical_bar(
            scroller,
            Scrollbar::new("bar", handle.clone())
                .track(px(12.))
                .radius(px(2.)),
        )
    });
    cx.update(|window, _| window.refresh());
    let scale = scale_of(cx);

    let thumb_len = VIEWPORT * (VIEWPORT / CONTENT);
    let quad = only_quad(cx, "radius 2");
    assert_thumb(
        "radius 2",
        &quad,
        scale,
        TRACK_W - 12.,
        0.,
        12.,
        thumb_len,
        2.,
    );
}

#[gpui::test]
fn inset_shrinks_the_painted_thumb(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let cx = open_host(cx, move || {
        let scroller = handle.clone();
        vertical_bar(
            scroller,
            Scrollbar::new("bar", handle.clone()).inset(px(2.)),
        )
    });
    cx.update(|window, _| window.refresh());

    // Every edge moves in by 2px; the derived radius still reads the
    // un-inset track, the way a webkit border radius reads the thumb's box.
    let thumb_len = VIEWPORT * (VIEWPORT / CONTENT);
    let scale = scale_of(cx);
    let quad = only_quad(cx, "inset 2");
    assert_thumb(
        "inset 2",
        &quad,
        scale,
        TRACK_W - 6.,
        2.,
        4.,
        thumb_len - 4.,
        4.,
    );
}

#[gpui::test]
fn an_sx_pixel_size_sizes_the_track(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let cx = open_host(cx, move || {
        let scroller = handle.clone();
        vertical_bar(
            scroller,
            Scrollbar::new("bar", handle.clone()).sx(move |s| s.w(px(16.))),
        )
    });
    cx.update(|window, _| window.refresh());

    // The cross-axis `sx` size is the thickness, and the default radius
    // derives from it like a `track` override does.
    let thumb_len = VIEWPORT * (VIEWPORT / CONTENT);
    let scale = scale_of(cx);
    let quad = only_quad(cx, "sx w 16");
    assert_thumb(
        "sx w 16",
        &quad,
        scale,
        TRACK_W - 16.,
        0.,
        16.,
        thumb_len,
        8.,
    );
}

#[gpui::test]
fn sx_radii_shape_the_thumb(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let cx = open_host(cx, move || {
        let scroller = handle.clone();
        vertical_bar(
            scroller,
            Scrollbar::new("bar", handle.clone())
                .track(px(12.))
                .sx(move |s| s.rounded(px(3.))),
        )
    });
    cx.update(|window, _| window.refresh());

    let thumb_len = VIEWPORT * (VIEWPORT / CONTENT);
    let scale = scale_of(cx);
    let quad = only_quad(cx, "sx radius 3");
    assert_thumb(
        "sx radius 3",
        &quad,
        scale,
        TRACK_W - 12.,
        0.,
        12.,
        thumb_len,
        3.,
    );
}

#[gpui::test]
fn an_sx_background_is_the_thumb_resting_fill_and_outranks_thumb_color(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let sx_bg = hsla(0.35, 0.6, 0.5, 1.0);
    let thumb_color = hsla(0.05, 0.9, 0.6, 1.0);
    let cx = open_host(cx, move || {
        let scroller = handle.clone();
        vertical_bar(
            scroller,
            Scrollbar::new("bar", handle.clone())
                .thumb_color(thumb_color)
                .sx(move |s| s.bg(sx_bg)),
        )
    });
    cx.update(|window, _| window.refresh());

    // The override paints the transparent gutter and recolors the thumb with
    // it — two quads, one colour. `thumb_color` is the seam that moves only
    // the thumb, which is why the `sx` background outranks it here.
    let quads = solid_quads(cx);
    assert_eq!(
        quads.len(),
        2,
        "an sx background paints the gutter and the thumb"
    );
    for quad in &quads {
        assert_fill("sx background", quad, sx_bg);
    }
}

#[gpui::test]
fn the_thumb_repaints_when_hovered_and_takes_the_hover_fill(cx: &mut TestAppContext) {
    let resting = hsla(0.3, 0.5, 0.5, 1.0);
    let hover = hsla(0.7, 0.5, 0.5, 1.0);
    let handle = ScrollHandle::new();
    let renders = Rc::new(Cell::new(0usize));
    let counter = renders.clone();
    let cx = open_host(cx, move || {
        counter.set(counter.get() + 1);
        let scroller = handle.clone();
        vertical_bar(
            scroller,
            Scrollbar::new("bar", handle.clone())
                .thumb_color(resting)
                .thumb_hover_color(hover),
        )
    });
    cx.update(|window, _| window.refresh());
    assert_fill("at rest", &only_quad(cx, "at rest"), resting);

    // No `window.refresh()` around the move: a redraw here can only have
    // come from the hover listener's notify, not the harness.
    cx.update(|window, _| window.refresh());
    let before = renders.get();
    cx.simulate_mouse_move(
        point(px(INSIDE_TRACK.0), px(INSIDE_TRACK.1)),
        None,
        Modifiers::none(),
    );
    let after = renders.get();
    assert!(
        after > before,
        "crossing onto the track must repaint the view ({before} renders \
         before, {after} after)"
    );
    cx.update(|window, _| window.refresh());
    assert_fill("hovered", &only_quad(cx, "hovered"), hover);

    cx.simulate_mouse_move(point(px(-100.), px(-100.)), None, Modifiers::none());
    cx.update(|window, _| window.refresh());
    assert_fill("left again", &only_quad(cx, "left again"), resting);
}

#[gpui::test]
fn without_a_hover_color_hovering_changes_nothing(cx: &mut TestAppContext) {
    let handle = ScrollHandle::new();
    let cx = open_host(cx, move || {
        let scroller = handle.clone();
        vertical_bar(scroller, Scrollbar::new("bar", handle.clone()))
    });
    cx.update(|window, _| window.refresh());
    let token = cx.update(|_, app| app.colors().scrollbar);

    cx.simulate_mouse_move(
        point(px(INSIDE_TRACK.0), px(INSIDE_TRACK.1)),
        None,
        Modifiers::none(),
    );
    cx.update(|window, _| window.refresh());
    assert_fill(
        "hovered without a hover color",
        &only_quad(cx, "hovered without a hover color"),
        token,
    );
}

#[gpui::test]
fn auto_hide_false_keeps_the_thumb_painted(cx: &mut TestAppContext) {
    // The test platform never reports an auto-hide preference (and cannot be
    // made to), so on the harness this builder is a no-op by construction;
    // the platform-hiding path of the decision is table-tested inline in
    // `scrollbar.rs`. Here it pins that `auto_hide(false)` still paints the
    // stock thumb, geometry untouched.
    let handle = ScrollHandle::new();
    let cx = open_host(cx, move || {
        let scroller = handle.clone();
        vertical_bar(
            scroller,
            Scrollbar::new("bar", handle.clone()).auto_hide(false),
        )
    });
    cx.update(|window, _| window.refresh());

    let thumb_len = VIEWPORT * (VIEWPORT / CONTENT);
    let scale = scale_of(cx);
    let quad = only_quad(cx, "auto_hide(false)");
    assert_thumb(
        "auto_hide(false)",
        &quad,
        scale,
        TRACK_W - 8.,
        0.,
        8.,
        thumb_len,
        4.,
    );
}
