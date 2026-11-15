//! Button's instance-level geometry and text-metric builders: `width`,
//! `min_width`, `height`, `padding_x`, `text_size`, `font_weight` and `grow`.
//!
//! The measurements ride the footprint, the way `sx_slot.rs` probes the `sx`
//! box: a fixed 10px canvas marker composed *after* the button records where
//! the button's box ends — `origin.x` in a flex row, `origin.y` in a flex
//! column — so every assertion reads flex-flow packing, never a percentage
//! child. (A `size_full` probe inside the button collapses in taffy: a
//! percentage width against a content-sized parent resolves to zero, and a
//! couple of rows even disagreed with themselves.)
//!
//! The one builder this suite cannot see is `font_weight`: the platform's
//! system font shapes bold and medium to the same advance width, so no
//! footprint can move. Its wiring is pinned by source scan instead, the
//! `text_size_knobs.rs` convention.
//!
//! Every builder is additive: each test pins the untouched default next to
//! the override where a mix could hide a changed fallback.

use crate::harness;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{prelude::*, px, Bounds, FontWeight, Pixels, TestAppContext, VisualTestContext};
use herogpui_components::Button;
use herogpui_core::Size;

type Sink = Rc<RefCell<Option<Bounds<Pixels>>>>;

fn sink() -> Sink {
    Rc::new(RefCell::new(None))
}

/// A fixed 10px marker: composed next to the button it records where the
/// button's footprint ends.
fn marker(sink: &Sink) -> gpui::AnyElement {
    gpui::canvas(|_, _, _| {}, {
        let sink = sink.clone();
        move |bounds, _, _, _| *sink.borrow_mut() = Some(bounds)
    })
    .w(px(10.))
    .h(px(10.))
    .into_any_element()
}

fn origin_x(sink: &Sink) -> f32 {
    f32::from(sink.borrow().expect("the marker painted").origin.x)
}

fn origin_y(sink: &Sink) -> f32 {
    f32::from(sink.borrow().expect("the marker painted").origin.y)
}

/// Footprints land on device pixels, so a content-derived width snaps to the
/// enclosing pixel (99.2 reads as 100). One pixel of slack keeps the text
/// assertions honest without loosening the exact-pixel ones.
fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0
}

/// The advance width of `text` shaped the way the button shapes its label:
/// gpui's default `.SystemUIFont` stack at `size` px and `weight`, laid out by
/// the window's own `WindowTextSystem` (the same helper `buttons.rs` uses).
fn text_width(system: &gpui::WindowTextSystem, text: &str, size: f32, weight: FontWeight) -> f32 {
    let run = gpui::TextRun {
        len: text.len(),
        font: gpui::Font {
            family: ".SystemUIFont".into(),
            features: gpui::FontFeatures::default(),
            weight,
            style: gpui::FontStyle::default(),
            fallbacks: None,
        },
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = system.shape_line(text.to_owned().into(), px(size), &[run], None);
    f32::from(line.width)
}

/// Forces a frame so canvas paint bounds describe the current build.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

const LABEL: &str = "Geometry";
/// `.button` is `px-4` at `Size::Md`.
const MD_PADDING_X: f32 = 32.;
/// The marker's own 10px box, which a height sandwich must subtract.
const MARKER_H: f32 = 10.;

#[gpui::test]
fn default_geometry_stays_the_size_ladder(cx: &mut TestAppContext) {
    let width_sink = sink();
    let before_sink = sink();
    let after_sink = sink();
    let (row_marker, before, after) = (width_sink.clone(), before_sink.clone(), after_sink.clone());
    let cx = open_host_geometry(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(0.))
            .child(
                gpui::div()
                    .flex()
                    .child(Button::new("geo-default").label(LABEL))
                    .child(marker(&row_marker)),
            )
            // The before/after sandwich cancels the column's own offset, so
            // the difference is exactly this button's box.
            .child(
                gpui::div()
                    .flex()
                    .flex_col()
                    .child(marker(&before))
                    .child(Button::new("geo-default-col").label(LABEL))
                    .child(marker(&after)),
            )
            .into_any_element()
    });
    flush_frame(cx);
    flush_frame(cx);

    let label_w =
        cx.update(|window, _| text_width(window.text_system(), LABEL, 14.0, FontWeight::MEDIUM));
    assert!(
        close(origin_x(&width_sink), MD_PADDING_X + label_w),
        "an untouched button keeps `px-4` around its 14px medium label: \
         {} vs {}",
        origin_x(&width_sink),
        MD_PADDING_X + label_w
    );
    assert!(
        (origin_y(&after_sink) - origin_y(&before_sink) - MARKER_H - 36.).abs() < f32::EPSILON,
        "an untouched button keeps `Size::Md`'s 36px control height"
    );
}

#[gpui::test]
fn width_fixes_the_box_and_beats_full_width(cx: &mut TestAppContext) {
    let plain_sink = sink();
    let both_sink = sink();
    let full_sink = sink();
    let (plain, both, full) = (plain_sink.clone(), both_sink.clone(), full_sink.clone());
    let cx = open_host_geometry(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                gpui::div()
                    .flex()
                    .child(Button::new("geo-width").label(LABEL).width(px(200.)))
                    .child(marker(&plain)),
            )
            .child(
                gpui::div()
                    .flex()
                    .child(
                        Button::new("geo-width-beats-full")
                            .label(LABEL)
                            .full_width(true)
                            .width(px(200.)),
                    )
                    .child(marker(&both)),
            )
            .child(
                gpui::div()
                    .flex()
                    .child(
                        Button::new("geo-full-still-works")
                            .label(LABEL)
                            .full_width(true),
                    )
                    .child(marker(&full)),
            )
            .into_any_element()
    });
    flush_frame(cx);
    flush_frame(cx);

    assert!(
        (origin_x(&plain_sink) - 200.).abs() < f32::EPSILON,
        "an explicit pixel width fixes the box at 200px"
    );
    assert!(
        (origin_x(&both_sink) - 200.).abs() < f32::EPSILON,
        "the explicit pixel width must win over full_width"
    );
    assert!(
        (origin_x(&full_sink) - 1920.).abs() < f32::EPSILON,
        "full_width alone must still fill the full-width row"
    );
}

#[gpui::test]
fn min_width_floors_the_box(cx: &mut TestAppContext) {
    let floored_sink = sink();
    let bare_sink = sink();
    let (floored, bare) = (floored_sink.clone(), bare_sink.clone());
    let cx = open_host_geometry(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                gpui::div()
                    .flex()
                    .child(Button::new("geo-min").label("Hi").min_width(px(150.)))
                    .child(marker(&floored)),
            )
            .child(
                gpui::div()
                    .flex()
                    .child(Button::new("geo-min-bare").label("Hi"))
                    .child(marker(&bare)),
            )
            .into_any_element()
    });
    flush_frame(cx);
    flush_frame(cx);

    let label_w =
        cx.update(|window, _| text_width(window.text_system(), "Hi", 14.0, FontWeight::MEDIUM));
    assert!(
        (origin_x(&floored_sink) - 150.).abs() < f32::EPSILON,
        "the floor must hold under a label narrower than it"
    );
    assert!(
        close(origin_x(&bare_sink), MD_PADDING_X + label_w),
        "without the floor the button keeps its content width: {} vs {}",
        origin_x(&bare_sink),
        MD_PADDING_X + label_w
    );
}

#[gpui::test]
fn height_and_padding_x_beat_the_size_ladder(cx: &mut TestAppContext) {
    let (tall_before, tall_after) = (sink(), sink());
    let (short_before, short_after) = (sink(), sink());
    let (ladder_before, ladder_after) = (sink(), sink());
    let padded_sink = sink();
    let bare_padded_sink = sink();
    let (padded, bare_padded) = (padded_sink.clone(), bare_padded_sink.clone());
    let cx = open_host_geometry(cx, {
        let (tall_before, tall_after) = (tall_before.clone(), tall_after.clone());
        let (short_before, short_after) = (short_before.clone(), short_after.clone());
        let (ladder_before, ladder_after) = (ladder_before.clone(), ladder_after.clone());
        move || {
            gpui::div()
            .flex()
            .flex_col()
            .gap(px(0.))
            // Heights: the before/after sandwich per button cancels the
            // column offsets, so the marker difference is that button's box.
            .child(
                gpui::div()
                    .flex()
                    .flex_col()
                    .child(marker(&tall_before))
                    .child(Button::new("geo-tall").size(Size::Lg).height(px(56.)))
                    .child(marker(&tall_after)),
            )
            .child(
                gpui::div()
                    .flex()
                    .flex_col()
                    .child(marker(&short_before))
                    .child(Button::new("geo-short").height(px(24.)))
                    .child(marker(&short_after)),
            )
            .child(
                gpui::div()
                    .flex()
                    .flex_col()
                    .child(marker(&ladder_before))
                    .child(Button::new("geo-ladder").size(Size::Lg).label(LABEL))
                    .child(marker(&ladder_after)),
            )
            // Padding: the marker beside each button reads the outer width.
            .child(
                gpui::div()
                    .flex()
                    .child(
                        Button::new("geo-padded")
                            .size(Size::Sm)
                            .padding_x(px(40.))
                            .label("X"),
                    )
                    .child(marker(&padded)),
            )
            .child(
                gpui::div()
                    .flex()
                    .child(Button::new("geo-padded-bare").size(Size::Sm).label("X"))
                    .child(marker(&bare_padded)),
            )
            .into_any_element()
        }
    });
    flush_frame(cx);
    flush_frame(cx);

    assert!(
        (origin_y(&tall_after) - origin_y(&tall_before) - MARKER_H - 56.).abs() < f32::EPSILON,
        "an explicit height must beat the Lg ladder's 40px"
    );
    assert!(
        (origin_y(&short_after) - origin_y(&short_before) - MARKER_H - 24.).abs() < f32::EPSILON,
        "an explicit height beats the 36px Md ladder"
    );
    assert!(
        (origin_y(&ladder_after) - origin_y(&ladder_before) - MARKER_H - 40.).abs() < f32::EPSILON,
        "the Lg ladder stays the fallback when no height is set"
    );
    let x_w =
        cx.update(|window, _| text_width(window.text_system(), "X", 14.0, FontWeight::MEDIUM));
    assert!(
        close(origin_x(&padded_sink), 80. + x_w),
        "an explicit padding_x must beat the Sm ladder's px-3: {} vs {}",
        origin_x(&padded_sink),
        80. + x_w
    );
    assert!(
        close(origin_x(&bare_padded_sink), 24. + x_w),
        "the Sm ladder's px-3 stays the fallback without the builder: {} vs {}",
        origin_x(&bare_padded_sink),
        24. + x_w
    );
}

#[gpui::test]
fn text_size_resizes_the_label_and_beats_the_size_ladder(cx: &mut TestAppContext) {
    let big_sink = sink();
    let small_sink = sink();
    let (big, small) = (big_sink.clone(), small_sink.clone());
    let cx = open_host_geometry(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                gpui::div()
                    .flex()
                    .child(Button::new("geo-text").label(LABEL).text_size(px(18.)))
                    .child(marker(&big)),
            )
            .child(
                gpui::div()
                    .flex()
                    .child(
                        Button::new("geo-text-vs-size")
                            .size(Size::Lg)
                            .label(LABEL)
                            .text_size(px(12.)),
                    )
                    .child(marker(&small)),
            )
            .into_any_element()
    });
    flush_frame(cx);
    flush_frame(cx);

    let mut at = |size: f32| {
        cx.update(|window, _| text_width(window.text_system(), LABEL, size, FontWeight::MEDIUM))
    };
    assert!(
        close(origin_x(&big_sink), MD_PADDING_X + at(18.)),
        "the label must be shaped at the explicit 18px: {} vs {}",
        origin_x(&big_sink),
        MD_PADDING_X + at(18.)
    );
    assert!(
        close(origin_x(&small_sink), MD_PADDING_X + at(12.)),
        "an explicit text_size must beat the Lg ladder's 16px: {} vs {}",
        origin_x(&small_sink),
        MD_PADDING_X + at(12.)
    );
}

/// The platform's system font shapes bold and medium to the same advance
/// width, so no footprint can pin this; a style probe composed into the
/// button reads the weight its label draws with instead: `.button`'s
/// `font-medium` by default, replaced by `font_weight`.
#[gpui::test]
fn font_weight_builder_replaces_the_medium_default(cx: &mut TestAppContext) {
    for (weight, want) in [
        (None, FontWeight::MEDIUM),
        (Some(FontWeight::BOLD), FontWeight::BOLD),
    ] {
        let style = harness::style_sink();
        let seen = style.clone();
        harness::still();
        let cx = harness::open_host(cx, move || {
            let button = Button::new("fw")
                .label("Save")
                .child(harness::style_probe(&style));
            match weight {
                Some(w) => button.font_weight(w),
                None => button,
            }
            .into_any_element()
        });
        harness::settle(cx);
        assert_eq!(
            harness::seen_style(&seen, "button label").font_weight,
            want,
            "font_weight {weight:?}"
        );
    }
}

#[gpui::test]
fn grow_fills_the_row_free_width_and_can_compress(cx: &mut TestAppContext) {
    let fill_sink = sink();
    let squeezed_sink = sink();
    let hug_sink = sink();
    let disabled_sink = sink();
    let (fill, squeezed, hug, disabled) = (
        fill_sink.clone(),
        squeezed_sink.clone(),
        hug_sink.clone(),
        disabled_sink.clone(),
    );
    let cx = open_host_geometry(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                // Free width: 400 - 100 - 10 = 290 for the grown button. The
                // marker rides between the button and the spacer, so its
                // origin is the button's own box.
                gpui::div()
                    .flex()
                    .w(px(400.))
                    .child(Button::new("geo-grow-fill").label(LABEL).grow(true))
                    .child(marker(&fill))
                    .child(gpui::div().w(px(100.)).h(px(36.))),
            )
            .child(
                // No free width: the grown button compresses below its own
                // content width — the `min-w-0` half of the pair.
                gpui::div()
                    .flex()
                    .w(px(120.))
                    .child(
                        Button::new("geo-grow-squeezed")
                            .label("A Long Button Label")
                            .grow(true),
                    )
                    .child(marker(&squeezed))
                    .child(gpui::div().w(px(40.)).h(px(36.))),
            )
            .child(
                // The control: without `grow` the same row keeps the button at
                // its content width and lets it overflow.
                gpui::div()
                    .flex()
                    .w(px(120.))
                    .child(Button::new("geo-grow-control").label("A Long Button Label"))
                    .child(marker(&hug))
                    .child(gpui::div().w(px(40.)).h(px(36.))),
            )
            .child(
                // A disabled button takes no press wrapper, so `grow` must
                // reach the row through the root alone.
                gpui::div()
                    .flex()
                    .w(px(400.))
                    .child(
                        Button::new("geo-grow-disabled")
                            .label(LABEL)
                            .grow(true)
                            .is_disabled(true),
                    )
                    .child(marker(&disabled))
                    .child(gpui::div().w(px(100.)).h(px(36.))),
            )
            .into_any_element()
    });
    flush_frame(cx);
    flush_frame(cx);

    let long_w = cx.update(|window, _| {
        text_width(
            window.text_system(),
            "A Long Button Label",
            14.0,
            FontWeight::MEDIUM,
        )
    });
    assert!(
        (origin_x(&fill_sink) - 290.).abs() < f32::EPSILON,
        "the grown button must take the row's free width"
    );
    assert!(
        (origin_x(&squeezed_sink) - 70.).abs() < f32::EPSILON,
        "the grown button must compress below its content width"
    );
    assert!(
        70. < MD_PADDING_X + long_w,
        "the compression case only proves min-w-0 when the content is wider"
    );
    assert!(
        close(origin_x(&hug_sink), MD_PADDING_X + long_w),
        "without grow the button keeps its content width: {} vs {}",
        origin_x(&hug_sink),
        MD_PADDING_X + long_w
    );
    assert!(
        (origin_x(&disabled_sink) - 290.).abs() < f32::EPSILON,
        "a disabled grown button fills its row without a press wrapper"
    );
}

#[gpui::test]
fn sx_refines_over_the_instance_values(cx: &mut TestAppContext) {
    let height_sink = sink();
    let width_sink = sink();
    let (height_marker_sink, width_marker_sink) = (height_sink.clone(), width_sink.clone());
    let cx = open_host_geometry(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(0.))
            .child(
                // The instance height moves the skin and the footprint; the
                // `sx` height refines the footprint's root last.
                Button::new("geo-sx-height")
                    .label(LABEL)
                    .height(px(56.))
                    .sx(|el| el.h(px(80.))),
            )
            .child(marker(&height_marker_sink))
            .child(
                gpui::div()
                    .flex()
                    .child(
                        Button::new("geo-sx-width")
                            .label(LABEL)
                            .width(px(200.))
                            .sx(|el| el.w(px(120.))),
                    )
                    .child(marker(&width_marker_sink)),
            )
            .into_any_element()
    });
    flush_frame(cx);
    flush_frame(cx);

    assert!(
        (origin_y(&height_sink) - 80.).abs() < f32::EPSILON,
        "the sx height must refine over the instance 56px"
    );
    assert!(
        (origin_x(&width_sink) - 120.).abs() < f32::EPSILON,
        "the sx width must refine over the instance 200px"
    );
}

/// The geometry suite's host: the theme is installed once and the same
/// element tree is rebuilt every frame from the `Fn` closure.
fn open_host_geometry(
    cx: &mut TestAppContext,
    content: impl Fn() -> gpui::AnyElement + 'static,
) -> &mut VisualTestContext {
    harness::open_host(cx, content)
}
