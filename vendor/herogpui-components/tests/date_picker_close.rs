//! Behaviour tests for the date pickers' close-on-select rule.
//!
//! v3 (via the React Aria pickers it composes) treats the two pickers
//! differently: a single `DatePicker` closes as soon as a day is chosen,
//! while a `DateRangePicker` stays open after the first pick so the end can
//! be chosen, and closes only once the range is complete. The defect was that
//! nothing closed the pickers at all — the calendar cell reported the pick
//! and the open flag never moved. These tests assert the close behaviourally,
//! never by appearance.
//!
//! Geometry is derived from the components' own constants, reusing the
//! DatePicker derivation from `pickers.rs` verbatim:
//!
//! - Each picker composes editable date segments and a separate 24px trigger.
//!   The single trigger centres at (124, 18), and the range trigger at
//!   (300, 18).
//! - The geometry probes below opt into `placement(BottomStart)` so the
//!   calendar cell coordinates stay stable while the default remains HeroUI's
//!   centered `bottom`: top =
//!   36 + 8 = 44, then `picker_panel` padding p-3 (12) brings the calendar to
//!   y = 56. The 24px header, 16px bottom padding, 24px weekday block and
//!   4px body offset put the first Calendar cell at y = 124, centre 142.
//! - Calendar columns: `CALENDAR_WIDTH` (252) split into seven equal
//!   cells; with the panel's 12px padding, the last column's centre is
//!   `12 + 6*cell_w + cell_w/2`.
//! - RangeCalendar adds 2px cell margins, putting its first row centre at 142
//!   and the second at 182; column *c* centres at `12 + 18 + 36c`.
//! - A bare Calendar's first row centres at 86.
//!
//! Closing now retains the popover for the shared 100ms exit animation. The
//! retained calendar is gated by the resolved overlay phase, so a second
//! click cannot select a cell or trigger a duplicate outside dismissal while
//! the panel is leaving.

use crate::harness;

use gpui::{prelude::*, Focusable, TestAppContext};
use herogpui_components::{
    calendar::{CalendarState, Date, CALENDAR_WIDTH},
    Calendar, DateConstraints, DatePicker, DateRangePicker, DateRangeState, Input, InputState,
};

use harness::{click, events, open_host};

/// The single picker's day coordinates, exactly as `pickers.rs` derives them.
fn day_coords() -> (f32, f32) {
    let cell_w = f32::from(CALENDAR_WIDTH) / 7.;
    let day_x = 12. + 6. * cell_w + cell_w / 2.;
    (day_x, 142.)
}

fn input_state(cx: &mut TestAppContext) -> gpui::Entity<InputState> {
    cx.new(|cx| InputState::new(cx))
}

#[gpui::test]
fn date_picker_focus_departure_closes_without_refocusing(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let picker = cx.new(|cx| CalendarState::new(cx));
    let next = input_state(cx);
    let next_for_view = next.clone();

    let cx = open_host(cx, move || {
        let opens = opens.clone();
        gpui::div()
            .child(
                DatePicker::new(picker.clone())
                    .default_open(true)
                    .on_open_change(move |open, _, _| {
                        opens.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .child(Input::new(next_for_view.clone()))
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    cx.update(|window, cx| window.focus(&next.read(cx).focus_handle(cx), cx));
    cx.update(|window, _| window.refresh());

    assert_eq!(opened.borrow().as_slice(), ["open:false"]);
    assert!(cx.update(|window, cx| next.read(cx).focus_handle(cx).is_focused(window)));
}

#[gpui::test]
fn date_range_picker_focus_departure_closes_without_refocusing(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let picker = cx.new(|cx| DateRangeState::new(cx));
    let next = input_state(cx);
    let next_for_view = next.clone();

    let cx = open_host(cx, move || {
        let opens = opens.clone();
        gpui::div()
            .child(
                DateRangePicker::new(picker.clone())
                    .default_open(true)
                    .on_open_change(move |open, _, _| {
                        opens.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .child(Input::new(next_for_view.clone()))
            .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    cx.update(|window, cx| window.focus(&next.read(cx).focus_handle(cx), cx));
    cx.update(|window, _| window.refresh());

    assert_eq!(opened.borrow().as_slice(), ["open:false"]);
    assert!(cx.update(|window, cx| next.read(cx).focus_handle(cx).is_focused(window)));
}

#[gpui::test]
fn date_picker_closes_when_a_day_is_picked(cx: &mut TestAppContext) {
    let picks = events();
    let recorded = picks.clone();
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| CalendarState::new(cx));

    // Column c of the first week holds day `c - lead + 1` once the lead
    // blanks are past, so the last column always holds day `7 - lead` — a
    // real day of this month whatever the month is.
    let today = Date::today();
    let lead = DateConstraints::new().lead_cells(today.year, today.month);
    let expected = Date::new(today.year, today.month, (7 - lead) as u32);
    let (day_x, day_y) = day_coords();

    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let opens = opens.clone();
        DatePicker::new(state_for_view.clone())
            .placement(herogpui_core::Placement::BottomStart)
            .on_change(move |date, _, _| {
                let iso = date.map(|d| d.format_iso()).unwrap_or_default();
                picks.borrow_mut().push(iso);
            })
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 295., 18.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the trigger must open the popover"
    );

    click(cx, day_x, day_y);
    assert_eq!(
        recorded.borrow().as_slice(),
        [expected.format_iso()],
        "clicking the last cell of the first week must pick day {}",
        7 - lead
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "a single date picker must close as soon as a day is chosen"
    );

    // Closed proof by behaviour: the retained exit surface is inert, so the
    // same probe cannot record a second "day" while it fades away.
    click(cx, day_x, day_y);
    assert_eq!(
        recorded.borrow().as_slice(),
        [expected.format_iso()],
        "the popover must be gone after choosing a day"
    );

    // The retained exit still paints its header for the 100ms fade, but its
    // calendar must be inert: a press on the old Previous control cannot page
    // the shared state while a newer overlay or field receives input.
    let anchor_before = cx.update(|_, cx| state.read(cx).anchor());
    click(cx, 24., 68.);
    let anchor_after = cx.update(|_, cx| state.read(cx).anchor());
    assert_eq!(
        anchor_after, anchor_before,
        "a retained DatePicker exit must not accept navigation input"
    );
}

#[gpui::test]
fn date_picker_can_remain_open_after_a_day_is_picked(cx: &mut TestAppContext) {
    let picks = events();
    let recorded = picks.clone();
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| CalendarState::new(cx));

    let today = Date::today();
    let lead = DateConstraints::new().lead_cells(today.year, today.month);
    let expected = Date::new(today.year, today.month, (7 - lead) as u32);
    let (day_x, day_y) = day_coords();

    let state_for_view = state;
    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let opens = opens.clone();
        DatePicker::new(state_for_view.clone())
            .placement(herogpui_core::Placement::BottomStart)
            .should_close_on_select(false)
            .on_change(move |date, _, _| {
                picks
                    .borrow_mut()
                    .push(date.map(|date| date.format_iso()).unwrap_or_default());
            })
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 295., 18.);
    click(cx, day_x, day_y);

    assert_eq!(recorded.borrow().as_slice(), [expected.format_iso()]);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "shouldCloseOnSelect=false keeps the calendar open after selection"
    );
}

#[gpui::test]
fn date_picker_custom_indicator_preserves_trigger_behavior(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        DatePicker::new(state_for_view.clone())
            .placement(herogpui_core::Placement::BottomStart)
            .trigger_indicator(gpui::div().child("custom"))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 295., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);
}

#[gpui::test]
fn date_picker_change_records_the_pick_once(cx: &mut TestAppContext) {
    // No `on_open_change` is wired: closing must not depend on a caller
    // handler, and the change callback must fire exactly once for the pick.
    let picks = events();
    let recorded = picks.clone();
    let state = cx.new(|cx| CalendarState::new(cx));

    let today = Date::today();
    let lead = DateConstraints::new().lead_cells(today.year, today.month);
    let expected = Date::new(today.year, today.month, (7 - lead) as u32);
    let (day_x, day_y) = day_coords();

    let state_for_view = state;
    let cx = open_host(cx, move || {
        let picks = picks.clone();
        DatePicker::new(state_for_view.clone())
            .placement(herogpui_core::Placement::BottomStart)
            .on_change(move |date, _, _| {
                let iso = date.map(|d| d.format_iso()).unwrap_or_default();
                picks.borrow_mut().push(iso);
            })
            .into_any_element()
    });

    click(cx, 295., 18.);
    click(cx, day_x, day_y);

    assert_eq!(
        recorded.borrow().as_slice(),
        [expected.format_iso()],
        "the change callback must record the chosen day exactly once"
    );

    // The pick logically closed the popover even though no open handler
    // exists; its retained exit surface records nothing a second time.
    click(cx, day_x, day_y);
    assert_eq!(
        recorded.borrow().as_slice(),
        [expected.format_iso()],
        "without an open handler the pick must still close the popover"
    );
}

#[gpui::test]
fn date_range_picker_stays_open_until_the_end_is_chosen(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));

    let today = Date::today();
    let lead = DateConstraints::new().lead_cells(today.year, today.month);
    let expected = Date::new(today.year, today.month, 1);
    // Day 8 is a week on: it lands one row down in the same column and is a
    // real, later date whatever the month is.
    let ended = Date::new(today.year, today.month, 8);
    // Day 1 starts at column `lead`; day 8 is 40px below it in the same column.
    let day_x = 30. + 36. * lead as f32;
    let row_one_y = 144.;
    let row_two_y = 184.;

    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        DateRangePicker::new(state.clone())
            .placement(herogpui_core::Placement::BottomStart)
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .on_change(move |_, cx| {
                let st = state.read(cx);
                let s = st.start.map(|d| d.format_iso()).unwrap_or_default();
                let e = st.end.map(|d| d.format_iso()).unwrap_or_default();
                changes.borrow_mut().push(format!("{s}->{e}"));
            })
            .into_any_element()
    });

    click(cx, 300., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // Choosing the start leaves the panel open to choose the end.
    click(cx, day_x, row_one_y);
    assert!(
        changed.borrow().is_empty(),
        "the first pick is an internal anchor, not a changed range value"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the first pick must keep the panel open for the end"
    );
    {
        let (start, end) = cx.update(|_, cx| {
            let st = state.read(cx);
            (st.start, st.end)
        });
        assert_eq!(start, Some(expected), "the start must be remembered");
        assert_eq!(end, None, "the end must still be open");
    }

    // Choosing the end completes the range and closes the panel.
    click(cx, day_x, row_two_y);
    assert_eq!(
        changed.borrow().as_slice(),
        [format!("{}->{}", expected.format_iso(), ended.format_iso())],
        "the second pick must report the complete range, once"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the panel must close once both ends are chosen"
    );
    {
        let (start, end) = cx.update(|_, cx| {
            let st = state.read(cx);
            (st.start, st.end)
        });
        assert_eq!(start, Some(expected), "the start must survive");
        assert_eq!(end, Some(ended), "the end must be remembered");
    }

    // Closed proof: the retained exit surface is inert, so a press where the
    // start row was records nothing.
    click(cx, day_x, row_one_y);
    assert_eq!(
        changed.borrow().len(),
        1,
        "the popover must be gone after the range is complete"
    );
    let anchor_before = cx.update(|_, cx| state.read(cx).anchor());
    click(cx, 24., 68.);
    let anchor_after = cx.update(|_, cx| state.read(cx).anchor());
    assert_eq!(
        anchor_after, anchor_before,
        "a retained DateRangePicker exit must not accept navigation input"
    );
}

#[gpui::test]
fn date_range_picker_can_remain_open_after_completion(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let today = Date::today();
    let lead = DateConstraints::new().lead_cells(today.year, today.month);
    let day_x = 30. + 36. * lead as f32;

    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        DateRangePicker::new(state_for_view.clone())
            .placement(herogpui_core::Placement::BottomStart)
            .should_close_on_select(false)
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 300., 18.);
    click(cx, day_x, 144.);
    click(cx, day_x, 184.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);
}

#[gpui::test]
fn date_range_picker_custom_parts_preserve_trigger_behavior(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        DateRangePicker::new(state_for_view.clone())
            .trigger_indicator(gpui::div().child("calendar"))
            .range_separator(gpui::div().child("to"))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 300., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);
}

#[gpui::test]
fn bare_calendar_still_records_a_chosen_date(cx: &mut TestAppContext) {
    // A Calendar on its own has no popover and must be untouched by the
    // pickers' closing: clicking a day records it, open flag or not.
    let picks = events();
    let recorded = picks.clone();
    let state = cx.new(|cx| CalendarState::new(cx));

    let today = Date::today();
    let lead = DateConstraints::new().lead_cells(today.year, today.month);
    let expected = Date::new(today.year, today.month, (7 - lead) as u32);
    // The bare calendar starts at the window origin: no panel padding, so the
    // last column loses the pickers' leading 12px, and the first row sits at
    // y = 86 (24px header + padding 16 + weekday 24 + offset 4 + half a 36px cell).
    let cell_w = f32::from(CALENDAR_WIDTH) / 7.;
    let day_x = 6. * cell_w + cell_w / 2.;
    let day_y = 86.;

    let state_for_view = state;
    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Calendar::new(state_for_view.clone())
            .on_change(move |date, _, _| {
                let iso = date.map(|d| d.format_iso()).unwrap_or_default();
                picks.borrow_mut().push(iso);
            })
            .into_any_element()
    });

    click(cx, day_x, day_y);
    assert_eq!(
        recorded.borrow().as_slice(),
        [expected.format_iso()],
        "a bare calendar must still record the day it is picked on"
    );
}
