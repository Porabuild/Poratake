//! Slider paint geometry: the 12px start/end border inset.
//!
//! HeroUI v3.2.4's track draws transparent `border-x-[0.75rem]` (horizontal)
//! or `border-y-[0.75rem]` (vertical) borders — half the 24px inner thumb —
//! so the full border box stays the pointer range while the fill and the
//! thumb percentages resolve against the content box that border leaves.
//! RAC 3.51.0's thumb is `left/top: percent%` + `translate(-50%, -50%)`, so a
//! thumb's center sits at `12px + fraction * (extent - 24px)` from the low
//! end of the border box, not at `fraction * extent`.
//!
//! Tracks are fixed at 600px (vertical 160px), as in the other slider
//! binaries. The thumb render prop carries a canvas that records the laid-out
//! thumb container bounds, which is where the inset shows.

use crate::harness;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{canvas, point, prelude::*, px, Bounds, Modifiers, TestAppContext, VisualTestContext};
use herogpui_components::Slider;

use harness::open_host;

/// Recorded thumb container bounds, one entry per thumb per frame.
type Probe = Rc<RefCell<Vec<Bounds<f32>>>>;

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// A `Slider.Thumb` render prop that reports its container's layout bounds.
fn probe_thumb(probe: &Probe) -> impl Fn(usize, f32) -> gpui::AnyElement + '_ {
    move |_index, _value| {
        let probe = probe.clone();
        canvas(
            move |bounds: Bounds<gpui::Pixels>, _, _| {
                probe.borrow_mut().push(Bounds {
                    origin: point(f32::from(bounds.origin.x), f32::from(bounds.origin.y)),
                    size: gpui::size(f32::from(bounds.size.width), f32::from(bounds.size.height)),
                });
                bounds
            },
            |_, _, _, _| {},
        )
        .size_full()
        .into_any_element()
    }
}

fn center(b: Bounds<f32>) -> (f32, f32) {
    (
        b.origin.x + b.size.width / 2.,
        b.origin.y + b.size.height / 2.,
    )
}

fn assert_center(probe: &Probe, expected: (f32, f32), context: &str) {
    let last = probe
        .borrow()
        .last()
        .copied()
        .unwrap_or_else(|| panic!("{context}: no thumb bounds recorded"));
    let (x, y) = center(last);
    assert!(
        (x - expected.0).abs() < 0.5 && (y - expected.1).abs() < 0.5,
        "{context}: thumb center is ({x}, {y}), expected ({}, {})",
        expected.0,
        expected.1
    );
}

/// Like `assert_center`, but for one thumb of a multi-thumb frame: the probe
/// records one entry per thumb in index order, so a range's last frame ends
/// with its thumbs; `from_end` is 1-based from the newest entry.
fn assert_thumb_center(probe: &Probe, from_end: usize, expected: (f32, f32), context: &str) {
    let frames = probe.borrow();
    let at = frames
        .len()
        .checked_sub(from_end)
        .unwrap_or_else(|| panic!("{context}: no bounds recorded {from_end} entries from the end"));
    let b = frames[at];
    drop(frames);
    let (x, y) = center(b);
    assert!(
        (x - expected.0).abs() < 0.5 && (y - expected.1).abs() < 0.5,
        "{context}: thumb {from_end} entries from the end is ({x}, {y}), expected ({}, {})",
        expected.0,
        expected.1
    );
}

#[gpui::test]
fn thumb_center_uses_the_inset_at_the_low_end(cx: &mut TestAppContext) {
    let probe: Probe = Rc::new(RefCell::new(Vec::new()));
    let for_view = probe.clone();
    let cx = open_host(cx, move || {
        let probe = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("geo-low", 0.)
                    .thumb(move |i, v| probe_thumb(&probe)(i, v))
                    .into_any_element(),
            )
            .into_any_element()
    });
    flush_frame(cx);
    // Old geometry centered on the full border box (x = 0); v3 centers on the
    // content box, 12px in.
    assert_center(&probe, (12., 10.), "value 0 on a 600px track");
}

#[gpui::test]
fn thumb_center_uses_the_inset_at_the_high_end(cx: &mut TestAppContext) {
    let probe: Probe = Rc::new(RefCell::new(Vec::new()));
    let for_view = probe.clone();
    let cx = open_host(cx, move || {
        let probe = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("geo-high", 100.)
                    .thumb(move |i, v| probe_thumb(&probe)(i, v))
                    .into_any_element(),
            )
            .into_any_element()
    });
    flush_frame(cx);
    assert_center(&probe, (588., 10.), "value 100 on a 600px track");
}

#[gpui::test]
fn midpoint_thumb_keeps_the_track_center(cx: &mut TestAppContext) {
    let probe: Probe = Rc::new(RefCell::new(Vec::new()));
    let for_view = probe.clone();
    let cx = open_host(cx, move || {
        let probe = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("geo-mid", 50.)
                    .thumb(move |i, v| probe_thumb(&probe)(i, v))
                    .into_any_element(),
            )
            .into_any_element()
    });
    flush_frame(cx);
    assert_center(&probe, (300., 10.), "value 50 on a 600px track");
}

#[gpui::test]
fn range_thumbs_center_on_the_inset(cx: &mut TestAppContext) {
    let probe: Probe = Rc::new(RefCell::new(Vec::new()));
    let for_view = probe.clone();
    let cx = open_host(cx, move || {
        let probe = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("geo-range", 0.)
                    .values([20., 80.])
                    .thumb(move |i, v| probe_thumb(&probe)(i, v))
                    .into_any_element(),
            )
            .into_any_element()
    });
    flush_frame(cx);
    // 12 + 0.2 * 576 and 12 + 0.8 * 576.
    assert_thumb_center(&probe, 2, (127.2, 10.), "range thumb 0 at 20");
    assert_thumb_center(&probe, 1, (472.8, 10.), "range thumb 1 at 80");
}

#[gpui::test]
fn vertical_thumb_center_mirrors_the_inset(cx: &mut TestAppContext) {
    let probe: Probe = Rc::new(RefCell::new(Vec::new()));
    let for_view = probe.clone();
    let cx = open_host(cx, move || {
        let probe = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("geo-vert", 0.)
                    .orientation(herogpui_core::Orientation::Vertical)
                    .thumb(move |i, v| probe_thumb(&probe)(i, v))
                    .into_any_element(),
            )
            .into_any_element()
    });
    flush_frame(cx);
    // A vertical slider runs bottom to top: value 0 centers the thumb 12px
    // above the track's bottom edge (y = 160 - 12 = 148), x centered on the
    // 20px rail.
    assert_center(&probe, (10., 148.), "value 0 on a 160px vertical track");
}

#[gpui::test]
fn pointer_values_reach_the_full_border_box(cx: &mut TestAppContext) {
    let recorded = harness::events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("geo-pointer", 3.)
                    .default_value(3.)
                    .on_change(move |value, _, _| {
                        recorded.borrow_mut().push(format!("{value}"));
                    })
                    .into_any_element(),
            )
            .into_any_element()
    });

    // The whole border box maps to the value range, including the 12px border
    // zones the thumb never quite reaches: the edges give min/max and the
    // inset edge (x=12) is not dead space.
    cx.simulate_click(point(px(0.5), px(10.)), Modifiers::none());
    flush_frame(cx);
    cx.simulate_click(point(px(12.), px(10.)), Modifiers::none());
    flush_frame(cx);
    cx.simulate_click(point(px(599.5), px(10.)), Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["0", "2", "100"],
        "pointer presses must map over the full border box"
    );
}

#[gpui::test]
fn vertical_pointer_values_reach_the_full_border_box(cx: &mut TestAppContext) {
    let recorded = harness::events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("geo-vert-pointer", 0.)
                    .default_value(0.)
                    .orientation(herogpui_core::Orientation::Vertical)
                    .on_change(move |value, _, _| {
                        recorded.borrow_mut().push(format!("{value}"));
                    })
                    .into_any_element(),
            )
            .into_any_element()
    });

    // y=155 is 5px above the bottom edge: 5/160 of the range snaps to 3. The
    // top edge gives 100.
    cx.simulate_click(point(px(10.), px(155.)), Modifiers::none());
    flush_frame(cx);
    cx.simulate_click(point(px(10.), px(0.5)), Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["3", "100"],
        "vertical pointer presses must map over the full border box"
    );
}

#[gpui::test]
fn slider_label_and_output_keep_twenty_pixel_lines(cx: &mut TestAppContext) {
    for leading in [None, Some(48.)] {
        for label_only in [false, true] {
            let cx = open_host(cx, move || {
                let slider = Slider::new("leading-slider", 50.).label(if label_only {
                    "First\nSecond"
                } else {
                    "Label"
                });
                let slider = if label_only {
                    slider.show_value(false)
                } else {
                    slider.output(|_, _| gpui::div().child("50\npercent").into_any_element())
                };
                gpui::div()
                    .w(px(300.))
                    .when_some(leading, |el, leading| el.line_height(px(leading)))
                    .child(
                        gpui::div()
                            .debug_selector(|| "leading-slider".into())
                            .child(slider),
                    )
                    .into_any_element()
            });
            let bounds = cx
                .debug_bounds("leading-slider")
                .expect("slider must paint");
            assert_eq!(
                bounds.size.height,
                px(64.),
                "two 20px lines, 4px gap and 20px track; label_only={label_only}, host={leading:?}"
            );
        }
    }
}

/// The track's own laid-out height: an unlabelled slider is only its rail, so
/// the wrapper's height is the track's cross axis.
fn track_cross_axis(cx: &mut VisualTestContext) -> gpui::Pixels {
    cx.debug_bounds("sized-slider")
        .expect("slider must paint")
        .size
        .height
}

#[gpui::test]
fn medium_is_the_default_and_keeps_the_pinned_geometry(cx: &mut TestAppContext) {
    for size in [None, Some(herogpui_components::SliderSize::Md)] {
        let probe: Probe = Rc::new(RefCell::new(Vec::new()));
        let for_view = probe.clone();
        let cx = open_host(cx, move || {
            let probe = for_view.clone();
            let slider = Slider::new("geo-md", 0.).thumb(move |i, v| probe_thumb(&probe)(i, v));
            let slider = match size {
                Some(size) => slider.size(size),
                None => slider,
            };
            gpui::div()
                .w(px(600.))
                .child(
                    gpui::div()
                        .debug_selector(|| "sized-slider".into())
                        .child(slider),
                )
                .into_any_element()
        });
        flush_frame(cx);
        assert_eq!(
            track_cross_axis(cx),
            px(20.),
            "Md track cross axis; size={size:?}"
        );
        let thumb = probe.borrow().last().copied().expect("thumb bounds");
        assert_eq!(
            (thumb.size.width, thumb.size.height),
            (28., 20.),
            "Md thumb box; size={size:?}"
        );
        // Value 0 centers the thumb one axis inset in: 12px.
        assert_center(&probe, (12., 10.), "Md axis inset");
    }
}

#[gpui::test]
fn small_shrinks_the_rail_the_inset_and_the_knob(cx: &mut TestAppContext) {
    let probe: Probe = Rc::new(RefCell::new(Vec::new()));
    let for_view = probe.clone();
    let cx = open_host(cx, move || {
        let probe = for_view.clone();
        gpui::div()
            .w(px(600.))
            .child(
                gpui::div().debug_selector(|| "sized-slider".into()).child(
                    Slider::new("geo-sm", 0.)
                        .size(herogpui_components::SliderSize::Sm)
                        .thumb(move |i, v| probe_thumb(&probe)(i, v)),
                ),
            )
            .into_any_element()
    });
    flush_frame(cx);
    assert_eq!(track_cross_axis(cx), px(6.), "Sm track cross axis");
    let thumb = probe.borrow().last().copied().expect("thumb bounds");
    assert_eq!(
        (thumb.size.width, thumb.size.height),
        (12., 12.),
        "Sm knob is a 12x12 square"
    );
    // Value 0 centers the 12px knob one 8px inset in, and the knob straddles
    // the 6px rail: its center stays on the rail center (y = 3).
    assert_center(&probe, (8., 3.), "Sm axis inset and cross-axis centering");
}

/// The `Sm` default thumb is one round node in the foreground token, not
/// `Md`'s pill-around-a-pill, and the `Sm` track and fill are pills. Read off
/// the painted scene: the quads at the knob's bounds, and the corner radii of
/// the rail's quads (fully rounded = half the 6px rail).
#[gpui::test]
fn small_default_thumb_is_a_single_pill_layer(cx: &mut TestAppContext) {
    for (size, layers) in [
        (herogpui_components::SliderSize::Sm, 1usize),
        (herogpui_components::SliderSize::Md, 2),
    ] {
        harness::still();
        let cx = open_host(cx, move || {
            gpui::div()
                .p(px(24.))
                .w(px(600.))
                .child(Slider::new("geo-layers", 40.).size(size))
                .into_any_element()
        });
        let scene = harness::painted(cx);
        let foreground = cx.update(|_, cx| {
            use herogpui_theme::ActiveTheme;
            cx.colors().foreground
        });
        // The knob layers are the quads narrower than the rail but at least
        // 8px tall; the rail is the widest quad.
        let rail = scene
            .quads
            .iter()
            .max_by(|a, b| a.bounds.size.width.0.total_cmp(&b.bounds.size.width.0))
            .map(|q| scene.bounds(q))
            .expect("the rail must paint");
        let knob = scene
            .quads
            .iter()
            .filter(|q| {
                let b = scene.bounds(q);
                f32::from(b.size.width) < 60. && f32::from(b.size.height) >= 8.
            })
            .filter(|q| {
                // not a fill end cap flush with the rail start
                (scene.bounds(q).origin.x - rail.origin.x).abs() > px(0.5)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            knob.len(),
            layers,
            "{size:?}: the default knob paints {layers} layer(s)\n{}",
            scene.describe()
        );
        if layers == 1 {
            assert_eq!(
                knob[0].background.as_solid(),
                Some(foreground),
                "Sm's knob is the foreground token"
            );
            assert!(
                scene.is_uniform(knob[0], f32::MAX),
                "Sm's knob is fully rounded, got {:?}",
                scene.corners(knob[0])
            );
            let rounded_rail = scene
                .quads
                .iter()
                .filter(|q| {
                    let b = scene.bounds(q);
                    (f32::from(b.size.height) - 6.).abs() < 0.5 && f32::from(b.size.width) > 6.
                })
                .all(|q| {
                    scene
                        .corners(q)
                        .iter()
                        .all(|c| (c - 3.).abs() < 0.05 || *c < 0.05)
                });
            assert!(
                rounded_rail
                    && scene.quads.iter().any(|q| {
                        let b = scene.bounds(q);
                        (f32::from(b.size.height) - 6.).abs() < 0.5 && scene.is_uniform(q, 3.)
                    }),
                "Sm's track and fill must be pills\n{}",
                scene.describe()
            );
        }
    }
}

#[gpui::test]
fn md_sx_corners_reach_inner_mark_and_clipped_caps(cx: &mut TestAppContext) {
    for vertical in [false, true] {
        for rounding in 0..3 {
            let cx = open_host(cx, move || {
                let slider = Slider::new("painted-corners", 100.).orientation(if vertical {
                    herogpui_core::Orientation::Vertical
                } else {
                    herogpui_core::Orientation::Horizontal
                });
                let slider = match rounding {
                    1 => slider.sx(|el| el.rounded_full()),
                    2 => slider.sx(|el| el.rounded_tl(px(3.))),
                    _ => slider,
                };
                gpui::div().w(px(600.)).child(slider).into_any_element()
            });
            let (quads, scale, key_radius) = cx.update(|window, cx| {
                (
                    window.painted_quads(),
                    window.scale_factor(),
                    f32::from(herogpui_components::extend::key_radius(cx)),
                )
            });
            let close = |a: f32, b: f32| (a - b).abs() < 0.05;
            let inner = quads
                .iter()
                .find(|quad| {
                    close(
                        quad.bounds.size.width.0 / scale,
                        if vertical { 16. } else { 24. },
                    ) && close(
                        quad.bounds.size.height.0 / scale,
                        if vertical { 24. } else { 16. },
                    )
                })
                .expect("Md inner mark must paint");
            let expected = match rounding {
                1 => [8.; 4],
                2 => [
                    3.,
                    key_radius.min(8.),
                    key_radius.min(8.),
                    key_radius.min(8.),
                ],
                _ => [key_radius.min(8.); 4],
            };
            assert_painted_corners(inner, scale, expected);
            for at_start in [false, true] {
                // The cap paints a square footprint under its clipped 12px
                // coverage so the 20px rail can retain a 10px radius.
                let length = 20.;
                let offset = if vertical == at_start {
                    (if vertical { 160. } else { 600. }) - length
                } else {
                    0.
                };
                let cap = quads
                    .iter()
                    .find(|quad| {
                        close(
                            quad.bounds.size.width.0 / scale,
                            if vertical { 20. } else { length },
                        ) && close(
                            quad.bounds.size.height.0 / scale,
                            if vertical { length } else { 20. },
                        ) && close(
                            (if vertical {
                                quad.bounds.origin.y
                            } else {
                                quad.bounds.origin.x
                            })
                            .0 / scale,
                            offset,
                        )
                    })
                    .expect("cap must paint at the corresponding track edge");
                let mut expected = [0.; 4];
                let indices = match (vertical, at_start) {
                    (false, true) => [0, 3],
                    (false, false) => [1, 2],
                    (true, true) => [2, 3],
                    (true, false) => [0, 1],
                };
                for index in indices {
                    expected[index] = if rounding == 0 || rounding == 1 {
                        10.
                    } else if rounding == 2 && index == 0 {
                        3.
                    } else {
                        6.
                    };
                }
                assert_painted_corners(cap, scale, expected);
                let visible = cap.bounds.intersect(&cap.content_mask.bounds);
                let extent = if vertical {
                    visible.size.height
                } else {
                    visible.size.width
                };
                assert!(
                    close(extent.0 / scale, 12.),
                    "corner expansion must not widen cap coverage"
                );
            }
        }
    }
}

fn assert_painted_corners(quad: &gpui::Quad, scale: f32, expected: [f32; 4]) {
    let corners = quad.corner_radii;
    for (actual, expected) in [
        corners.top_left,
        corners.top_right,
        corners.bottom_right,
        corners.bottom_left,
    ]
    .into_iter()
    .zip(expected)
    {
        assert!(
            (actual.0 / scale - expected).abs() < 0.05,
            "painted corner {} must match {expected}",
            actual.0 / scale
        );
    }
}

#[gpui::test]
fn single_thumb_fill_cap_joins_are_square_only_with_sx_corners(cx: &mut TestAppContext) {
    for vertical in [false, true] {
        for small in [false, true] {
            for range in [false, true] {
                for rounded in [false, true] {
                    for value in [50., 100.] {
                        let cx = open_host(cx, move || {
                            let mut slider = Slider::new("fill-joins", value)
                                .size(if small {
                                    herogpui_components::SliderSize::Sm
                                } else {
                                    herogpui_components::SliderSize::Md
                                })
                                .orientation(if vertical {
                                    herogpui_core::Orientation::Vertical
                                } else {
                                    herogpui_core::Orientation::Horizontal
                                });
                            if range {
                                slider = slider.values([0., value]);
                            }
                            if rounded {
                                slider = slider.sx(|el| el.rounded_full());
                            }
                            gpui::div().w(px(600.)).child(slider).into_any_element()
                        });
                        let (quads, scale) =
                            cx.update(|window, _| (window.painted_quads(), window.scale_factor()));
                        let inset = if small { 8. } else { 12. };
                        let cross = if small { 6. } else { 20. };
                        let extent = if vertical { 160. } else { 600. };
                        let length = (extent - 2. * inset) * value / 100.;
                        let origin = if vertical {
                            extent - inset - length
                        } else {
                            inset
                        };
                        let near = |a: f32, b: f32| (a - b).abs() < 0.05;
                        let fill = quads
                            .iter()
                            .find(|quad| {
                                let b = quad.bounds;
                                near(
                                    b.size.width.0 / scale,
                                    if vertical { cross } else { length },
                                ) && near(
                                    b.size.height.0 / scale,
                                    if vertical { length } else { cross },
                                ) && near(
                                    (if vertical { b.origin.y } else { b.origin.x }).0 / scale,
                                    origin,
                                )
                            })
                            .expect("value fill must paint");
                        let radius = if rounded || small { cross / 2. } else { 0. };
                        let mut expected = [radius; 4];
                        if rounded && !range {
                            for index in if vertical { [2, 3] } else { [0, 3] } {
                                expected[index] = 0.;
                            }
                            if value > 99. {
                                expected = [0.; 4];
                            }
                            // The clipped cap and fill meet at the exact inset,
                            // with full cross-axis coverage and no overlap gap.
                            let cap = quads
                                .iter()
                                .find(|quad| {
                                    let b = quad.bounds.intersect(&quad.content_mask.bounds);
                                    near(
                                        (if vertical {
                                            b.size.height
                                        } else {
                                            b.size.width
                                        })
                                        .0 / scale,
                                        inset,
                                    ) && near(
                                        (if vertical {
                                            b.size.width
                                        } else {
                                            b.size.height
                                        })
                                        .0 / scale,
                                        cross,
                                    ) && near(
                                        (if vertical { b.origin.y } else { b.origin.x }).0 / scale,
                                        if vertical { extent - inset } else { 0. },
                                    )
                                })
                                .expect("start cap must cover the strip adjoining the fill");
                            let cap = cap.bounds.intersect(&cap.content_mask.bounds);
                            let join = if vertical {
                                fill.bounds.origin.y.0 + fill.bounds.size.height.0
                            } else {
                                cap.origin.x.0 + cap.size.width.0
                            };
                            let other = if vertical {
                                cap.origin.y.0
                            } else {
                                fill.bounds.origin.x.0
                            };
                            assert!(near(join, other), "cap/fill bounds must abut");
                        }
                        assert_painted_corners(fill, scale, expected);
                    }
                }
            }
        }
    }
}
