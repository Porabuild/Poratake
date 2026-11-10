//! What wave 5 of the accessibility contract changed, and what can be checked.
//!
//! Read `a11y_deep.rs` first: it pins the fact that the AccessKit tree itself
//! is *not* observable from the headless platform, and that still holds here.
//! `a11y_overlays_deep.rs` is wave 2 and `a11y_collections_deep.rs` is wave 3.
//! So these tests assert the other half — the half a role change can actually
//! break.
//!
//! # Why a picker is the sharp case
//!
//! An AccessKit node id is a hash of the element's `GlobalElementId`, and that
//! id path is *also* gpui's key for per-element state: the hover slot, the
//! press latch, the scroll offset, the focus handle. Wave 5 gave several
//! calendar and colour surfaces an element id where they had none, because an
//! element with no id produces no node and a role set on it is silently
//! dropped:
//!
//! - each month grid (`{id}-grid-{year}-{month}`), so `Role::Grid` can land,
//! - `DateField` / `TimeField` boxes (`role="group"`), nesting every segment,
//! - `DatePicker` / `DateRangePicker` field boxes and their calendar triggers,
//! - `ColorSlider`'s track (`role="slider"`), `ColorArea`'s group,
//! - `ColorSwatchPicker`'s radio group and each swatch,
//! - `ColorPicker`'s trigger (`aria-expanded`) and panel (`role="dialog"`).
//!
//! Every one of those nests its existing descendants one segment deeper. A
//! day cell, a date segment or a colour thumb that collided would share one
//! press latch and one node while looking completely normal on screen.
//! Overlay-backed pickers make that worse, because the panel mounts detached
//! from the trigger, so nothing on screen says which instance answered.
//!
//! Each test therefore drives *two* independent instances and asserts they
//! answer separately. Keyboard wherever the component has a tab stop, so no
//! assertion depends on a measured coordinate that a layout change would
//! silently move.

use crate::harness;

use gpui::{prelude::*, px, TestAppContext, VisualTestContext};
use herogpui_components::{
    Calendar, CalendarState, ColorChannel, ColorPicker, ColorSlider, ColorSpace, ColorSwatchPicker,
    Date, DateField, DatePicker, InputState, PickerColor, Time, TimeField, TimeState,
};

use harness::{click, events, open_host, press};

/// Centre of a laid-out Calendar day cell, keyed the same way `calendars_deep`
/// reads it. The grid took an id this wave, so a derived (column, row) would
/// silently miss if the path nested; the selector is the cell's own id.
fn click_cal_day(cx: &mut VisualTestContext, entity_id: u64, year: i32, month: u32, day: u32) {
    let key = format!(r#"NamedInteger("cal", {entity_id})-{year}-{month}-d{day}"#);
    let bounds = cx
        .debug_bounds(Box::leak(key.into_boxed_str()))
        .unwrap_or_else(|| panic!("missing calendar cell {year}-{month}-{day}"));
    click(
        cx,
        f32::from(bounds.center().x),
        f32::from(bounds.center().y),
    );
}

/// A pair of pickers side by side, each in its own fixed-width column so a
/// click coordinate is arithmetic rather than a guess. 320px fits a
/// 252px calendar and a 240px colour slider.
fn side_by_side(left: gpui::AnyElement, right: gpui::AnyElement) -> gpui::AnyElement {
    gpui::div()
        .flex()
        .flex_row()
        .child(gpui::div().w(px(COLUMN)).child(left))
        .child(gpui::div().w(px(COLUMN)).child(right))
        .into_any_element()
}

const COLUMN: f32 = 320.;

// ---------------------------------------------------------------------------
// Calendar: the month grid took an id so it can report role="grid"
// ---------------------------------------------------------------------------

/// `Calendar`'s month grid took an id so it can report `role="grid"`, and
/// every day cell now states `role="button"` with `aria-selected`. The cell
/// ids hang off the same `{id}` prefix the grid does, so a second calendar
/// whose id collided would nest its days onto the first one's. Two calendars
/// must select from their own focused cell alone.
#[gpui::test]
fn two_calendars_select_independently(cx: &mut TestAppContext) {
    let changed = events();
    let recorded = changed.clone();
    let left_state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2026, 8, 3)));
    let right_state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2026, 8, 10)));
    let left_id = left_state.entity_id().as_u64();
    let right_id = right_state.entity_id().as_u64();
    let left_view = left_state;
    let right_view = right_state;
    let cx = open_host(cx, move || {
        let left = changed.clone();
        let right = changed.clone();
        side_by_side(
            Calendar::new(left_view.clone())
                .on_change(move |date, _, _| {
                    left.borrow_mut().push(format!(
                        "left:{}",
                        date.as_ref().map(Date::format_iso).unwrap_or_default()
                    ));
                })
                .into_any_element(),
            Calendar::new(right_view.clone())
                .on_change(move |date, _, _| {
                    right.borrow_mut().push(format!(
                        "right:{}",
                        date.as_ref().map(Date::format_iso).unwrap_or_default()
                    ));
                })
                .into_any_element(),
        )
    });

    // A Calendar owns several tab stops (grid, prev, next, year picker), so
    // Tab cannot be asked to walk from one instance to the other. The cells
    // themselves are the nodes this wave added roles to.
    click_cal_day(cx, left_id, 2026, 8, 4);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:2026-08-04"],
        "clicking a day on the left calendar must select only that calendar"
    );

    click_cal_day(cx, right_id, 2026, 8, 11);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:2026-08-04", "right:2026-08-11"],
        "the right calendar must hold its own selection, not the left one's"
    );
}

// ---------------------------------------------------------------------------
// DateField / TimeField: the group took an id so it can report role="group"
// ---------------------------------------------------------------------------

/// `DateField`'s box took an id for `role="group"`, and each segment is a
/// `role="textbox"` node nested under it. Two fields that collided would
/// step together.
#[gpui::test]
fn two_date_fields_step_independently(cx: &mut TestAppContext) {
    let changed = events();
    let recorded = changed.clone();
    let left_state = cx.new(|cx| InputState::with_value(cx, "2025-10-15"));
    let right_state = cx.new(|cx| InputState::with_value(cx, "2025-01-01"));
    let left_view = left_state;
    let right_view = right_state;
    let cx = open_host(cx, move || {
        let left = changed.clone();
        let right = changed.clone();
        side_by_side(
            DateField::new(left_view.clone())
                .on_change(move |date, _, _| {
                    left.borrow_mut().push(format!(
                        "left:{}",
                        date.map(|d| d.format_iso()).unwrap_or_default()
                    ));
                })
                .into_any_element(),
            DateField::new(right_view.clone())
                .on_change(move |date, _, _| {
                    right.borrow_mut().push(format!(
                        "right:{}",
                        date.map(|d| d.format_iso()).unwrap_or_default()
                    ));
                })
                .into_any_element(),
        )
    });

    press(cx, "tab");
    press(cx, "up");
    {
        let events = recorded.borrow();
        assert_eq!(
            events.len(),
            1,
            "the first tab stop is the left field, and Up steps its focused segment"
        );
        assert!(
            events[0].starts_with("left:"),
            "the left field must answer, not the right one: {:?}",
            events[0]
        );
    }

    press(cx, "tab");
    press(cx, "up");
    {
        let events = recorded.borrow();
        assert_eq!(
            events.len(),
            2,
            "the right field must hold its own value, not the left one's"
        );
        assert!(
            events[1].starts_with("right:"),
            "the right field must answer second: {:?}",
            events[1]
        );
    }
}

/// `TimeField`'s box took the same `role="group"` id, and its steppers are
/// named "Increase"/"Decrease" on `role="button"` nodes. Two fields that
/// collided would step together.
#[gpui::test]
fn two_time_fields_step_independently(cx: &mut TestAppContext) {
    let changed = events();
    let recorded = changed.clone();
    let left_state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 30)));
    let right_state = cx.new(|cx| TimeState::with_value(cx, Time::new(14, 0)));
    let left_view = left_state;
    let right_view = right_state;
    let cx = open_host(cx, move || {
        let left = changed.clone();
        let right = changed.clone();
        side_by_side(
            TimeField::new(left_view.clone())
                .on_change(move |time, _, _| {
                    left.borrow_mut().push(format!(
                        "left:{}",
                        time.map(|t| format!("{:02}:{:02}", t.hour, t.minute))
                            .unwrap_or_default()
                    ));
                })
                .into_any_element(),
            TimeField::new(right_view.clone())
                .on_change(move |time, _, _| {
                    right.borrow_mut().push(format!(
                        "right:{}",
                        time.map(|t| format!("{:02}:{:02}", t.hour, t.minute))
                            .unwrap_or_default()
                    ));
                })
                .into_any_element(),
        )
    });

    press(cx, "tab");
    press(cx, "up");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:10:30"],
        "the first tab stop is the left field, and Up steps its focused hour"
    );

    press(cx, "tab");
    press(cx, "up");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:10:30", "right:15:00"],
        "the right field must hold its own value, not the left one's"
    );
}

// ---------------------------------------------------------------------------
// DatePicker: the field group and the trigger both took roles
// ---------------------------------------------------------------------------

/// `DatePicker`'s field box took an id for `role="group"`, and the calendar
/// trigger now states `role="button"` with `aria-expanded`. The composed
/// `Calendar` hangs off the same `{id}` prefix, so a second picker whose id
/// collided would open the first one's panel. Two pickers must open from
/// their own field alone.
#[gpui::test]
fn two_date_pickers_open_independently(cx: &mut TestAppContext) {
    harness::still();
    let opened = events();
    let recorded = opened.clone();
    let left_state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 6, 15)));
    let right_state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 7, 1)));
    let left_view = left_state;
    let right_view = right_state;
    let cx = open_host(cx, move || {
        let left = opened.clone();
        let right = opened.clone();
        side_by_side(
            DatePicker::new(left_view.clone())
                .on_open_change(move |open, _, _| {
                    left.borrow_mut().push(format!("left:{open}"));
                })
                .into_any_element(),
            DatePicker::new(right_view.clone())
                .on_open_change(move |open, _, _| {
                    right.borrow_mut().push(format!("right:{open}"));
                })
                .into_any_element(),
        )
    });

    press(cx, "tab");
    press(cx, "alt-down");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:true"],
        "Alt+ArrowDown on the first field must open only the left picker"
    );

    press(cx, "escape");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:true", "left:false"],
        "Escape must close the left picker without touching the right one"
    );

    // Escape restores the initiating field. The next tab stop is that
    // picker's own calendar trigger, so one more Tab is needed to reach
    // the right field.
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "alt-down");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:true", "left:false", "right:true"],
        "the right picker must hold its own open flag, not the left one's"
    );
}

// ---------------------------------------------------------------------------
// Colour: slider track, swatch radio group, picker dialog
// ---------------------------------------------------------------------------

/// `ColorSlider`'s track took `role="slider"` with `aria-orientation`. Two
/// sliders that shared a path would move together.
#[gpui::test]
fn two_color_sliders_move_independently(cx: &mut TestAppContext) {
    let moved = events();
    let recorded = moved.clone();
    let seed = PickerColor::hsb(180., 1., 1.);
    let cx = open_host(cx, move || {
        let left = moved.clone();
        let right = moved.clone();
        side_by_side(
            ColorSlider::new("left-hue", seed, ColorChannel::Hue)
                .show_label(false)
                .length(px(240.))
                .on_change(move |color, _, _| {
                    left.borrow_mut().push(format!(
                        "left:{}",
                        color.channel_in(ColorChannel::Hue, ColorSpace::Hsb)
                    ));
                })
                .into_any_element(),
            ColorSlider::new("right-hue", seed, ColorChannel::Hue)
                .show_label(false)
                .length(px(240.))
                .on_change(move |color, _, _| {
                    right.borrow_mut().push(format!(
                        "right:{}",
                        color.channel_in(ColorChannel::Hue, ColorSpace::Hsb)
                    ));
                })
                .into_any_element(),
        )
    });

    press(cx, "tab");
    press(cx, "right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:181"],
        "the first tab stop is the left slider, and Right steps hue by one"
    );

    press(cx, "tab");
    press(cx, "right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:181", "right:181"],
        "the right slider must hold its own channel, not the left one's"
    );
}

/// `ColorSwatchPicker`'s root took `role="radiogroup"` and each swatch
/// `role="radio"` with `aria-selected`. Two pickers that collided would
/// report the same swatch.
#[gpui::test]
fn two_color_swatch_pickers_select_independently(cx: &mut TestAppContext) {
    let changed = events();
    let recorded = changed.clone();
    let swatches = vec![
        PickerColor::from_hex("#E52D2D").unwrap(),
        PickerColor::from_hex("#006FEE").unwrap(),
    ];
    let left_swatches = swatches.clone();
    let right_swatches = swatches;
    let cx = open_host(cx, move || {
        let left = changed.clone();
        let right = changed.clone();
        side_by_side(
            ColorSwatchPicker::new("left-swp", left_swatches.clone())
                .default_value(left_swatches[0])
                .on_change(move |c, _, _| left.borrow_mut().push(format!("left:{}", c.to_hex())))
                .into_any_element(),
            ColorSwatchPicker::new("right-swp", right_swatches.clone())
                .default_value(right_swatches[0])
                .on_change(move |c, _, _| right.borrow_mut().push(format!("right:{}", c.to_hex())))
                .into_any_element(),
        )
    });

    // Cells are `size-8` (32px) with an 8px gap, so swatch i's centre is
    // x = 16 + 40i. The right picker starts at COLUMN.
    click(cx, 56., 16.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:#006FEE"],
        "clicking the left picker's second swatch must report only that picker"
    );

    click(cx, COLUMN + 56., 16.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:#006FEE", "right:#006FEE"],
        "the right picker must hold its own selection, not the left one's"
    );
}

/// `ColorPicker`'s trigger took `role="button"` with `aria-expanded`, and the
/// panel took `role="dialog"`. The panel hangs off the same `{id}` prefix the
/// trigger does, so a second picker whose id collided would open the first
/// one's panel.
#[gpui::test]
fn two_color_pickers_open_independently(cx: &mut TestAppContext) {
    harness::still();
    let opened = events();
    let recorded = opened.clone();
    let seed = PickerColor::hsb(210., 0.5, 0.6);
    let cx = open_host(cx, move || {
        let left = opened.clone();
        let right = opened.clone();
        side_by_side(
            ColorPicker::new("left-cp", seed)
                .on_open_change(move |open, _, _| {
                    left.borrow_mut().push(format!("left:{open}"));
                })
                .into_any_element(),
            ColorPicker::new("right-cp", seed)
                .on_open_change(move |open, _, _| {
                    right.borrow_mut().push(format!("right:{open}"));
                })
                .into_any_element(),
        )
    });

    // Trigger: swatch (24px) + hex label, 24px tall at the origin of each
    // column. The right picker starts at COLUMN. The open panel hangs over
    // that column, so the left picker has to close before the right trigger
    // is hittable — that is overlay geometry, not a shared id.
    click(cx, 60., 12.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:true"],
        "the left trigger must open only the left picker"
    );

    press(cx, "escape");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:true", "left:false"],
        "Escape must close the left picker without touching the right one"
    );

    click(cx, COLUMN + 60., 12.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["left:true", "left:false", "right:true"],
        "the right picker must hold its own open flag, not the left one's"
    );
}
