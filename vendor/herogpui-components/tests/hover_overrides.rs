//! Phase 2 hover overrides: every owner resolves the named fill and its
//! painted consumer uses the resolved value.
//!
//! The field-trigger seams (`Select`/`Autocomplete::trigger_hover_bg`,
//! `Autocomplete::clear_hover_bg`, `NumberField`/`InputGroup::group_hover_bg`)
//! are checked on the painted scene instead, at the bottom of this file: the
//! pointer hovers the part, the fade settles on the executor clock and the frame
//! must hold the override (and a disabled trigger must not).
//!
//! The drawn states are exercised by each owner's own behavior tests; these
//! checks pin the wiring, so a builder that stores the colour without feeding
//! the fill cannot pass. Resolver-style endpoint logic (Button's contract)
//! lives with `util::fade_endpoints`.
//!
//! Both pins are function-scoped through `source_scan::scope_contains`, per
//! the roadmap's source-check rule: a sibling function's paint call must not
//! satisfy an owner's pin, so the function that resolves the override has to
//! be the one whose fill consumes it. Each entry also proves the scanner can
//! fail — with the resolve and the paint split across sibling functions, the
//! pin must error — so a green run is the scanner agreeing, not it reading
//! nothing.

use crate::harness;
use crate::source_scan;

use gpui::{
    point, prelude::*, px, AnyElement, Modifiers, MouseButton, TestAppContext, VisualTestContext,
};
use herogpui_components::{
    Autocomplete, Button, Input, InputGroup, InputState, NumberField, NumberState, Pagination,
    PickerItem, Select, ToggleButton, Variant,
};
use herogpui_theme::ActiveTheme;
use source_scan::{component_src, scope_contains};

#[test]
fn every_hover_override_reaches_its_painted_fill() {
    let mut failures = Vec::new();
    for (file, field, resolved, painted) in [
        (
            "select.rs",
            "row_hover_bg",
            "let row_hover_bg = self.row_hover_bg.unwrap_or(colors.default.color);",
            "s.bg(row_hover_bg)",
        ),
        (
            "combo_box.rs",
            "row_hover_bg",
            "let row_hover_bg = self.row_hover_bg.unwrap_or(colors.default.color);",
            "s.bg(hover_bg)",
        ),
        (
            "autocomplete.rs",
            "row_hover_bg",
            "let row_hover_bg = self.row_hover_bg.unwrap_or(colors.default.color);",
            "s.bg(row_hover_bg)",
        ),
        (
            "list_box.rs",
            "row_hover_bg",
            "let hover_bg = self.row_hover_bg.unwrap_or(colors.default.color);",
            "s.bg(hover_bg)",
        ),
        (
            "dropdown.rs",
            "row_hover_bg",
            "let row_hover_bg = self.row_hover_bg.unwrap_or(colors.default.color);",
            "s.bg(row_hover_bg)",
        ),
        (
            "pagination.rs",
            "hover_bg",
            "let control_hover_bg = self.hover_bg.unwrap_or(colors.default.hover());",
            "let pressed_bg = control_hover_bg;",
        ),
        (
            "tag_group.rs",
            "hover_bg",
            "let hover = self.hover_bg.unwrap_or_else(|| {",
            "hover_fade_with_duration_and_easing(",
        ),
        (
            "tag_group.rs",
            "remove_hover_bg",
            "let hover_bg = self.remove_hover_bg.unwrap_or(colors.default.hover());",
            "s.bg(hover_bg)",
        ),
        (
            "table.rs",
            "row_hover_bg",
            "let hover_bg = (!is_selected && !is_disabled).then(|| {",
            "row_hover_bg.unwrap_or(if self.secondary {",
        ),
        (
            "accordion.rs",
            "hover_bg",
            "let hover_bg = self.hover_bg.unwrap_or_else(|| match self.variant {",
            "(idle_bg, hover_bg),",
        ),
        (
            "time_field.rs",
            "stepper_hover_bg",
            "let hover_bg = self.stepper_hover_bg.unwrap_or(colors.default.color);",
            "s.bg(hover_bg)",
        ),
        (
            "input_otp.rs",
            "slot_hover_bg",
            "let hover_bg = self.slot_hover_bg.unwrap_or(match self.variant {",
            // `.input-otp__slot` hovers through the pinned 150ms shell
            // transition, so the override resolves into the chrome ramp's
            // hovered endpoint rather than an immediate fill.
            "bg: hovered_bg,",
        ),
        (
            "input.rs",
            "clear_hover_bg",
            "let clear_hover_bg = self.clear_hover_bg.unwrap_or(colors.default.hover());",
            "s.bg(clear_hover_bg)",
        ),
        (
            "date_picker/range.rs",
            "trigger_hover_bg",
            "let hover_bg = self.trigger_hover_bg.unwrap_or(colors.field.hover());",
            "s.bg(hover_bg)",
        ),
        (
            "toast.rs",
            "close_hover_bg",
            "let hover_bg = self.t.close_hover_bg.unwrap_or(colors.default.color);",
            "s.bg(hover_bg)",
        ),
        (
            "switch.rs",
            "hover_bg",
            "unwrap_or(if checked { accent_hover } else { default_hover });",
            "let track_motion_frame = track_motion(&self.id, track_target, window, cx);",
        ),
        (
            "checkbox.rs",
            "hover_bg",
            "let fill_hover = self.hover_bg.unwrap_or(accent_hover);",
            "let fill_bg_target = if is_hovered { fill_hover } else { accent_color };",
        ),
        (
            "calendar.rs",
            "day_hover_bg",
            "let hover_bg = self.day_hover_bg.unwrap_or(colors.default.color);",
            "s.bg(hover_bg)",
        ),
        (
            "range_calendar.rs",
            "day_hover_bg",
            "let hover_bg = self.day_hover_bg.unwrap_or(colors.default.color);",
            "s.bg(hover_bg)",
        ),
        (
            "calendar.rs",
            "nav_hover_bg",
            "let hover_bg = self.nav_hover_bg.unwrap_or(colors.default.color);",
            // `.calendar__nav-button` hovers through the pinned 100ms
            // background transition, so the override resolves into the
            // fade's hover endpoint rather than an immediate fill.
            "colors.default.color, 0.0), hover_bg),",
        ),
        (
            "calendar.rs",
            "year_hover_bg",
            "let hover_bg = self.year_hover_bg.unwrap_or(colors.default.color);",
            "s.bg(hover_bg)",
        ),
        (
            "range_calendar.rs",
            "nav_hover_bg",
            "let hover_bg = self.nav_hover_bg.unwrap_or(colors.default.color);",
            "colors.default.color, 0.0), hover_bg),",
        ),
        (
            "range_calendar.rs",
            "year_hover_bg",
            "let hover_bg = self.year_hover_bg.unwrap_or(colors.default.color);",
            "s.bg(hover_bg)",
        ),
    ] {
        let source = component_src(file);
        // The builder named after the field must store the override itself:
        // a store line loose in another function does not wire the builder.
        let builder = format!("pub fn {field}(");
        let stored = format!("self.{field} = Some(color.into());");
        if let Err(err) = scope_contains(&source, &builder, &stored) {
            failures.push(format!("{file} [{field}]: {err}"));
        }
        // The function that resolves the override must be the one painting
        // the resolved value into a fill.
        if let Err(err) = scope_contains(&source, resolved, painted) {
            failures.push(format!("{file} [{field}]: {err}"));
        }
        // The known-negative: with the paint moved into a sibling function,
        // the whole-file truth of both lines must stop satisfying the pin.
        let co_located = fixture(resolved, painted, true);
        assert!(
            scope_contains(&co_located, resolved, painted).is_ok(),
            "{file} [{field}]: the scanner rejected a correct co-located pair"
        );
        let sibling = fixture(resolved, painted, false);
        assert!(
            scope_contains(&sibling, resolved, painted).is_err(),
            "{file} [{field}]: a sibling fn's paint satisfied the scoped pin"
        );
    }
    assert!(
        failures.is_empty(),
        "hover override wiring failed:\n{}",
        failures.join("\n")
    );
}

/// A synthetic owner function holding `resolved` (and `painted`, when
/// `co_located`); otherwise `painted` sits in a sibling function first, the
/// shape a whole-file `contains` wrongly accepts.
fn fixture(resolved: &str, painted: &str, co_located: bool) -> String {
    let mut source = String::new();
    if !co_located {
        source.push_str("fn sibling() {\n    ");
        source.push_str(painted);
        source.push_str("\n}\n\n");
    }
    source.push_str("fn render() {\n    ");
    source.push_str(resolved);
    if co_located {
        source.push_str("\n    ");
        source.push_str(painted);
    }
    source.push_str("\n}\n");
    source
}

// Painted-scene checks for the field-trigger hover seams: the pointer is
// moved onto the control and the 150ms fade is allowed to settle on the real
// clock, then the frame's quads must hold the override colour. The colour is
// one no theme token uses, so finding it proves the seam reached the fill.

const OVERRIDE: gpui::Hsla = gpui::Hsla {
    h: 0.83,
    s: 0.7,
    l: 0.42,
    a: 1.0,
};

fn picker_items() -> Vec<PickerItem> {
    ["Rust", "Go"]
        .iter()
        .map(|l| PickerItem::new(l.to_string(), l.to_string()))
        .collect()
}

fn paints_override(cx: &mut VisualTestContext) -> bool {
    harness::has_color(&harness::painted(cx).solids(), OVERRIDE)
}

/// Steps the executor clock in 100ms increments (two seconds of clock
/// time at most) until `want` holds.
fn eventually(cx: &mut VisualTestContext, want: bool) -> bool {
    for _ in 0..20 {
        if paints_override(cx) == want {
            return true;
        }
        harness::wait_ms(cx, 100);
    }
    paints_override(cx) == want
}

/// Whether hovering (`x`, `y`) settles on a frame that paints `OVERRIDE`.
fn hover_paints_override(cx: &mut VisualTestContext, x: f32, y: f32) -> bool {
    // The first frame is laid out with the pointer at the origin, before the
    // host parks it outside; that crossing's fade runs out first, and the
    // override is a hover endpoint only, so nothing paints it at rest.
    assert!(
        eventually(cx, false),
        "the override is a hover endpoint only; nothing paints it at rest"
    );
    cx.simulate_mouse_move(point(px(x), px(y)), None, Modifiers::none());
    eventually(cx, true)
}

fn open(
    cx: &mut TestAppContext,
    build: impl Fn() -> AnyElement + 'static,
) -> &mut VisualTestContext {
    harness::open_host(cx, build)
}

#[gpui::test]
fn select_trigger_hover_bg_is_the_trigger_hover_endpoint(cx: &mut TestAppContext) {
    let cx = open(cx, || {
        Select::new("hover-select", picker_items())
            .trigger_hover_bg(OVERRIDE)
            .into_any_element()
    });
    assert!(hover_paints_override(cx, 20., 18.));
}

#[gpui::test]
fn a_disabled_select_never_paints_its_trigger_hover_bg(cx: &mut TestAppContext) {
    let cx = open(cx, || {
        Select::new("hover-select-off", picker_items())
            .trigger_hover_bg(OVERRIDE)
            .is_disabled(true)
            .into_any_element()
    });
    assert!(
        !hover_paints_override(cx, 20., 18.),
        "disabled outranks the hover override"
    );
}

#[gpui::test]
fn autocomplete_trigger_hover_bg_is_the_trigger_hover_endpoint(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open(cx, move || {
        Autocomplete::new(state.clone(), picker_items())
            .trigger_hover_bg(OVERRIDE)
            .into_any_element()
    });
    assert!(hover_paints_override(cx, 20., 18.));
}

#[gpui::test]
fn autocomplete_clear_hover_bg_fills_the_hovered_clear_button(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let base = format!("autocomplete-{}", state.entity_id().as_u64());
    let cx = open(cx, move || {
        Autocomplete::new(state.clone(), picker_items())
            .default_value(["Rust"])
            .clear_hover_bg(OVERRIDE)
            .into_any_element()
    });
    cx.update(|window, _| window.refresh());
    let clear: &'static str = Box::leak(format!("{base}-clear-visual").into_boxed_str());
    let bounds = cx
        .debug_bounds(clear)
        .expect("a selected Autocomplete renders its clear button");
    let centre = bounds.center();
    assert!(hover_paints_override(
        cx,
        f32::from(centre.x),
        f32::from(centre.y)
    ));
}

#[gpui::test]
fn number_field_group_hover_bg_is_the_group_hover_endpoint(cx: &mut TestAppContext) {
    let state = cx.new(|cx| NumberState::new(cx, 4.));
    let cx = open(cx, move || {
        NumberField::new(state.clone())
            .group_hover_bg(OVERRIDE)
            .into_any_element()
    });
    assert!(hover_paints_override(cx, 20., 18.));
}

#[gpui::test]
fn input_group_group_hover_bg_is_the_group_hover_endpoint(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open(cx, move || {
        InputGroup::new()
            .group_hover_bg(OVERRIDE)
            .input(Input::new(state.clone()))
            .into_any_element()
    });
    assert!(hover_paints_override(cx, 20., 18.));
}

// ---------------------------------------------------------------------------
// The hover fade's painted contract
// ---------------------------------------------------------------------------
//
// `anim::hover_fade` mounts one absolutely-positioned fill that owns the
// state colour; the element underneath stays transparent. These checks pin
// the three ways that contract was once broken: a translucent endpoint
// composited twice, a mid-flight reversal snapping to the far endpoint, and
// a held press painting its colour under the fill instead of on it.

/// The quads whose non-transparent solid fill is `color`, within
/// [`harness::same_color`]'s float tolerance — `Painted::filled` compares
/// exactly, and a fade that settles through `mix_oklab` can land a hair off
/// the literal endpoint.
fn fill_quads(scene: &harness::Painted, color: gpui::Hsla) -> Vec<&gpui::Quad> {
    scene
        .quads
        .iter()
        .filter(|q| {
            q.background
                .as_solid()
                .is_some_and(|c| c.a > 0. && harness::same_color(c, color))
        })
        .collect()
}

/// DangerSoft's resting fill is translucent, so the element and the fill
/// painting the same endpoint used to composite twice — the resting colour
/// read darker forever after the first hover. Once the fill is mounted the
/// element stays transparent and exactly one quad may carry `soft()`.
#[gpui::test]
fn danger_soft_rest_paints_the_soft_endpoint_exactly_once(cx: &mut TestAppContext) {
    let cx = open(cx, || {
        Button::new("fade-rest")
            .label("Delete")
            .variant(Variant::DangerSoft)
            .into_any_element()
    });
    let (soft, soft_hover) =
        cx.update(|_window, cx| (cx.colors().danger.soft(), cx.colors().danger.soft_hover()));
    cx.simulate_mouse_move(point(px(20.), px(18.)), None, Modifiers::none());
    harness::wait_ms(cx, 200);
    cx.simulate_mouse_move(point(px(-100.), px(-100.)), None, Modifiers::none());
    harness::wait_ms(cx, 200);
    let scene = harness::painted(cx);
    assert_eq!(
        fill_quads(&scene, soft).len(),
        1,
        "the resting DangerSoft fill must be painted exactly once; painted {}",
        scene.describe()
    );
    assert!(
        fill_quads(&scene, soft_hover).is_empty(),
        "no layer may still hold the hover endpoint after the pointer left; painted {}",
        scene.describe()
    );
}

/// A held press resolves to the pressed endpoint on the same single fill.
/// DangerSoft's pressed colour *is* `soft_hover()`, and it must appear once —
/// the variant's `.active` refinement and the press ramp's colour track used
/// to paint it under the fill as well.
#[gpui::test]
fn danger_soft_held_press_paints_the_pressed_endpoint_exactly_once(cx: &mut TestAppContext) {
    let cx = open(cx, || {
        Button::new("fade-press")
            .label("Delete")
            .variant(Variant::DangerSoft)
            .into_any_element()
    });
    let soft_hover = cx.update(|_window, cx| cx.colors().danger.soft_hover());
    cx.simulate_mouse_move(point(px(20.), px(18.)), None, Modifiers::none());
    cx.simulate_mouse_down(
        point(px(20.), px(18.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    harness::wait_ms(cx, 200);
    let scene = harness::painted(cx);
    assert_eq!(
        fill_quads(&scene, soft_hover).len(),
        1,
        "the held press must paint the pressed endpoint exactly once; painted {}",
        scene.describe()
    );
}

/// Reversing mid-fade resumes from the colour on screen: hover part-way into
/// the ease, leave, and the fill's next frame must still be the mid colour —
/// deriving `from`/`to` from the hovered bit used to snap it to `soft_hover()`
/// for a frame, the flash a pointer sweeping a row of buttons produced.
#[gpui::test]
fn a_mid_fade_reversal_resumes_from_the_painted_colour(cx: &mut TestAppContext) {
    let cx = open(cx, || {
        Button::new("fade-reverse")
            .label("Delete")
            .variant(Variant::DangerSoft)
            .into_any_element()
    });
    let soft_hover = cx.update(|_window, cx| cx.colors().danger.soft_hover());
    cx.simulate_mouse_move(point(px(20.), px(18.)), None, Modifiers::none());
    harness::wait_ms(cx, 30);
    cx.simulate_mouse_move(point(px(-100.), px(-100.)), None, Modifiers::none());
    // The reversal's first frames, with the clock held: the fill must still
    // show the mid-fade colour, not the far endpoint.
    harness::settle(cx);
    let scene = harness::painted(cx);
    let fill_color = scene
        .quads
        .iter()
        .filter_map(|q| q.background.as_solid())
        .find(|c| c.a > 0.)
        .expect("the fade's fill must be painting mid-reversal");
    assert!(
        !harness::same_color(fill_color, soft_hover),
        "the reversal must resume from the painted mid-fade colour ({fill_color:?}), \
         not snap to the far endpoint; painted {}",
        scene.describe()
    );
}

/// A keyboard press flips the same pressed bit a pointer press does, so the
/// fill eases to the pressed endpoint even with the pointer elsewhere — the
/// opaque idle fill the pointer left behind must not hide it.
#[gpui::test]
fn a_keyboard_press_reaches_the_painted_fill(cx: &mut TestAppContext) {
    let cx = open(cx, || {
        Button::new("fade-keyboard")
            .label("Go")
            .variant(Variant::Primary)
            .into_any_element()
    });
    let (idle, pressed) =
        cx.update(|_window, cx| (cx.colors().accent.color, cx.colors().accent.hover()));
    // Hover in and back out first: the fill stays mounted, resting on the
    // opaque idle colour — the layer that used to hide the pressed endpoint.
    cx.simulate_mouse_move(point(px(20.), px(18.)), None, Modifiers::none());
    harness::wait_ms(cx, 200);
    cx.simulate_mouse_move(point(px(-100.), px(-100.)), None, Modifiers::none());
    harness::wait_ms(cx, 200);
    let resting = harness::painted(cx);
    let fill = resting
        .quads
        .iter()
        .find(|q| {
            q.background
                .as_solid()
                .is_some_and(|c| harness::same_color(c, idle))
        })
        .expect("the resting fill must be mounted and painting the idle endpoint");
    let centre = resting.bounds(fill).center();
    let centre_bounds = gpui::Bounds {
        origin: centre,
        size: gpui::size(px(0.), px(0.)),
    };
    // Focus and hold Space: the key down begins the press and stays held.
    harness::press(cx, "tab");
    cx.simulate_keystrokes("space");
    harness::wait_ms(cx, 200);
    let scene = harness::painted(cx);
    let topmost = scene
        .around(centre_bounds)
        .into_iter()
        .rev()
        .filter_map(|q| q.background.as_solid())
        .find(|c| c.a > 0.)
        .expect("the button must still paint a fill over its centre");
    assert!(
        harness::same_color(topmost, pressed),
        "the topmost fill over the button must be the pressed endpoint \
         ({pressed:?}), painted {topmost:?}; scene {}",
        scene.describe()
    );
}

/// A selected toggle's pressed endpoint is the translucent
/// `accent.soft_hover()` — the press ramp's colour track used to paint it
/// on the element under the fade fill as well, compositing twice while held.
/// The fade owns the pressed endpoint now, so exactly one quad may carry it.
#[gpui::test]
fn a_selected_toggle_held_press_paints_the_pressed_fill_once(cx: &mut TestAppContext) {
    let cx = open(cx, || {
        ToggleButton::new("fade-toggle")
            .label("Bold")
            .is_selected(true)
            .into_any_element()
    });
    let soft_hover = cx.update(|_window, cx| cx.colors().accent.soft_hover());
    cx.simulate_mouse_move(point(px(20.), px(18.)), None, Modifiers::none());
    cx.simulate_mouse_down(
        point(px(20.), px(18.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    harness::wait_ms(cx, 200);
    let scene = harness::painted(cx);
    assert_eq!(
        fill_quads(&scene, soft_hover).len(),
        1,
        "the held press must paint the pressed endpoint exactly once; painted {}",
        scene.describe()
    );
}

/// `anim::hover_fade` appends its fill as the element's last child, so it
/// paints over any child already present — a page number or nav chevron added
/// before the fade call was hidden under the hover colour. The label now
/// joins after the fill and paints on top of it; this pins the order on the
/// painted scene through `Pagination.Link`'s render prop, which draws a
/// solid quad text runs cannot.
#[gpui::test]
fn a_page_label_paints_over_the_hover_fill(cx: &mut TestAppContext) {
    let cx = open(cx, || {
        Pagination::new("fade-pages", 1, 3)
            .link(|n, _| {
                gpui::div()
                    .size_full()
                    .bg(gpui::Hsla {
                        h: 0.,
                        s: 1.,
                        l: 0.5,
                        a: 1.,
                    })
                    .child(n.to_string())
                    .into_any_element()
            })
            .into_any_element()
    });
    let hover_bg = cx.update(|_window, cx| cx.colors().default.hover());
    let red = gpui::Hsla {
        h: 0.,
        s: 1.,
        l: 0.5,
        a: 1.,
    };
    // Page 2's cell spans x 74..106 (see collections.rs), centre (90, 16).
    cx.simulate_mouse_move(point(px(90.), px(16.)), None, Modifiers::none());
    harness::wait_ms(cx, 200);
    let scene = harness::painted(cx);
    let probe = gpui::Bounds {
        origin: point(px(90.), px(16.)),
        size: gpui::size(px(1.), px(1.)),
    };
    let painted_at = |color: gpui::Hsla| {
        scene
            .quads
            .iter()
            .enumerate()
            .find(|(_, q)| {
                harness::contains(scene.bounds(q), probe)
                    && q.background
                        .as_solid()
                        .is_some_and(|c| c.a > 0. && harness::same_color(c, color))
            })
            .map(|(index, _)| index)
    };
    let fill = painted_at(hover_bg).expect("the hover fill must paint the cell");
    let label = painted_at(red).expect("the page label must paint its own fill");
    assert!(
        label > fill,
        "the label must paint after — over — the fade's fill; scene {}",
        scene.describe()
    );
}
