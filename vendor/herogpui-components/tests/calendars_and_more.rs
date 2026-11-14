//! Behaviour tests for the calendar family and the remaining un-driven
//! controls: Calendar, RangeCalendar, ColorPicker, Select and
//! Disclosure / DisclosureGroup / Toolbar.
//!
//! The `.shots/*.py` audits measure props and pixels; these tests drive the
//! controls with simulated input and assert on recorded callbacks, state
//! entities the test owns, and probe clicks that must record nothing.
//!
//! Geometry is derived from the components' own constants, reusing the
//! derivations in `pickers.rs` and `date_picker_close.rs`:
//!
//! - Bare Calendar at the window origin: `CALENDAR_WIDTH` (252) split into seven equal
//!   cells fixes the column centres, and the first cell row
//!   sits at y = 74 (24px nav header + gap 8 + ~16px weekday line + gap 8 +
//!   half a 36px cell). Rows step 38px (36 cell + 2 gap). Day *d* of a month
//!   with `lead` leading blanks sits at `idx = d + lead - 1`, row `idx / 7`,
//!   column `idx % 7`. The lead follows the system locale, just like the
//!   component. Only the weekday line height is a text metric, and the click
//!   tolerates it the same way `date_picker_close.rs` does.
//! - Nav buttons live in the header row: `.calendar__header` is `px-0.5`, the
//!   buttons are `size-6`, so previous centres at (14, 12) and next at
//!   (238, 12) inside the 252px-wide column.
//! - Bare RangeCalendar: cells are 38px with no column gaps (row `flex_row`
//!   with no gap), so column *c* centres at `19 + 38c`. The first row centres
//!   at y = 75 (the same header stack as the Calendar, half of a 38px cell)
//!   and rows step 40px (38 cell + 2 gap).
//! - Select: the trigger is the 36px field at the origin (centre (60, 18));
//!   the popover hangs from `placed_field_panel(BottomStart, 6px)` at top 42
//!   with `py(6)`, so option row *i* centres at `66 + 36i`. A `section_before`
//!   heading is drawn *above* its option inside the same slot: the header
//!   occupies the top ~24px and the option the bottom 36px.
//! - ColorPicker: the trigger is the swatch + hex label row, 24px tall
//!   (`SizeXl::Sm` swatch), so the panel hangs at top 30 and the `ColorArea`
//!   (240x160) spans x 8..248, y 38..198 after its 8px top/inline inset.
//! - Disclosure / DisclosureGroup: each trigger is a 36px Button.
//! - Toolbar: driven entirely through the keyboard (Tab, arrows, Enter), so
//!   no geometry is needed — the window's tab order is what moves.
//!
//! ColorPicker retains its panel for the pinned 100ms exit. A closed-proof
//! probe against its uncontrolled path advances the deterministic clock first;
//! Select and the bare calendars have no retained exit surface here.

use crate::harness;

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    process::Command,
    rc::Rc,
    time::Duration,
};

use gpui::{
    point, prelude::*, px, Focusable, KeyDownEvent, Keystroke, Modifiers, MouseButton,
    SharedString, TestAppContext, VisualTestContext,
};
use harness::{click, events, open_host, press};
use herogpui_components::{
    add_days,
    calendar::{Date, CALENDAR_WIDTH},
    calendar_view::{
        aligned_anchor, anchor_following_focus, linear_cells, week_start, SelectionAlignment,
    },
    Button, Calendar, CalendarState, ColorPicker, DateConstraints, DateRangeState, Disclosure,
    DisclosureGroup, Input, InputState, PageBehavior, PickerColor, PickerItem, RangeCalendar,
    Select, SelectionMode, Toolbar, VisibleDuration, Weekday,
};

/// Select options whose labels are unique, so the key can be the label itself.
/// Tests that need duplicate labels or a key distinct from the label build
/// explicit `PickerItem`s instead.
fn keyed(labels: &[&str]) -> Vec<PickerItem> {
    labels
        .iter()
        .map(|l| PickerItem::new(l.to_string(), l.to_string()))
        .collect()
}

/// Drives the shared measured Disclosure panel to its settled endpoint before
/// asserting the sibling's reflow. Since gpui-pre 0.3.11 GPUI animations and
/// the retained exit timer both use the test executor clock.
fn settle_collapsible(cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_millis(220));
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    cx.executor().advance_clock(Duration::from_millis(210));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
}

/// Column *c*'s centre in a bare Calendar: seven cells across `CALENDAR_WIDTH`
/// with no horizontal gaps.
fn cal_col_x(col: usize) -> f32 {
    let cell_w = f32::from(CALENDAR_WIDTH) / 7.;
    col as f32 * cell_w + cell_w / 2.
}

/// Row *r*'s centre in a bare Calendar: first row at y = 86, then 36px per row.
fn cal_row_y(row: usize) -> f32 {
    86. + row as f32 * 36.
}

/// The centre of the cell holding `day` of `(year, month)` in a bare
/// Calendar, derived from the same locale-sensitive `DateConstraints` the
/// test's calendars use.
fn cal_day(year: i32, month: u32, day: u32) -> (f32, f32) {
    let lead = DateConstraints::new().lead_cells(year, month);
    let idx = day as usize + lead - 1;
    (cal_col_x(idx % 7), cal_row_y(idx / 7))
}

/// Column *c*'s centre in a bare RangeCalendar: 36px cells, no column gaps.
fn range_col_x(col: usize) -> f32 {
    18. + 36. * col as f32
}

/// Row *r*'s centre in a bare RangeCalendar: first row at y = 88, then a
/// 36px cell plus the 4px combined vertical margins per row.
fn range_row_y(row: usize) -> f32 {
    88. + 40. * row as f32
}

/// The centre of the cell holding `day` of `(year, month)` in a bare
/// RangeCalendar.
fn range_day(year: i32, month: u32, day: u32) -> (f32, f32) {
    let lead = DateConstraints::new().lead_cells(year, month);
    let idx = day as usize + lead - 1;
    (range_col_x(idx % 7), range_row_y(idx / 7))
}

fn week_home_end_expectations(focused: Date, grid_first_day: Weekday) -> (Date, Date, Date, Date) {
    let duration = VisibleDuration::Weeks(2);
    let mut anchor = aligned_anchor(
        duration,
        SelectionAlignment::Center,
        grid_first_day,
        focused,
    );
    let cells = linear_cells(duration, grid_first_day, anchor);
    let home = week_start(focused, Weekday::default());
    anchor = anchor_following_focus(
        duration,
        grid_first_day,
        anchor,
        cells[0],
        cells[cells.len() - 1],
        home,
    );
    let anchor_after_home = anchor;
    let cells = linear_cells(duration, grid_first_day, anchor);
    let end = add_days(&home, 6);
    anchor = anchor_following_focus(
        duration,
        grid_first_day,
        anchor,
        cells[0],
        cells[cells.len() - 1],
        end,
    );
    (home, end, anchor_after_home, anchor)
}

/// One forced redraw, so a keyed flag change is visible to the next probe.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

// ---------------------------------------------------------------------------
// Calendar & RangeCalendar
// ---------------------------------------------------------------------------

#[gpui::test]
fn calendar_selection_keys_consume_browser_defaults_and_ignore_repeats(cx: &mut TestAppContext) {
    for key in ["enter", "space"] {
        let state = cx.new(|cx| CalendarState::new(cx));
        let view_state = state.clone();
        let changes = events();
        let recorded = changes.clone();
        let cx = open_host(cx, move || {
            let changes = changes.clone();
            Calendar::new(view_state.clone())
                .on_change(move |_, _, _| changes.borrow_mut().push(String::new()))
                .into_any_element()
        });
        press(cx, "tab");
        for is_held in [false, true] {
            let result = cx.update(|window, cx| {
                window.dispatch_event(
                    gpui::PlatformInput::KeyDown(KeyDownEvent {
                        keystroke: Keystroke::parse(key).unwrap(),
                        is_held,
                        prefer_character_input: false,
                    }),
                    cx,
                )
            });
            assert!(!result.propagate, "{key} must consume the browser default");
            flush_frame(cx);
        }
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: Keystroke::parse(key).unwrap(),
        });
        assert_eq!(recorded.borrow().len(), 1);
    }
}

#[gpui::test]
fn range_calendar_selection_keys_consume_browser_defaults_and_ignore_repeats(
    cx: &mut TestAppContext,
) {
    for key in ["enter", "space"] {
        let state = cx.new(|cx| DateRangeState::new(cx));
        let view_state = state.clone();
        let changes = events();
        let recorded = changes.clone();
        let cx = open_host(cx, move || {
            let changes = changes.clone();
            RangeCalendar::new(view_state.clone())
                .on_change(move |_, _, _, _| changes.borrow_mut().push(String::new()))
                .into_any_element()
        });
        press(cx, "tab");
        for is_held in [false, true] {
            let result = cx.update(|window, cx| {
                window.dispatch_event(
                    gpui::PlatformInput::KeyDown(KeyDownEvent {
                        keystroke: Keystroke::parse(key).unwrap(),
                        is_held,
                        prefer_character_input: false,
                    }),
                    cx,
                )
            });
            assert!(!result.propagate, "{key} must consume the browser default");
            flush_frame(cx);
        }
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: Keystroke::parse(key).unwrap(),
        });
        assert!(cx.update(|_, cx| state.read(cx).end.is_none()));
        assert!(recorded.borrow().is_empty());
        press(cx, key);
        assert_eq!(recorded.borrow().len(), 1);
    }
}

#[gpui::test]
fn calendar_indian_locale_grid_selects_gregorian_dates(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let changed = events();
    let recorded = changed.clone();
    let cx = open_host(cx, move || {
        let changed = changed.clone();
        Calendar::new(state.clone())
            .locale("hi-IN-u-ca-indian")
            .first_day_of_week(Weekday::Mon)
            .default_value(Date::new(2026, 1, 15))
            .on_change(move |date, _, _| {
                changed.borrow_mut().push(date.unwrap().format_iso());
            })
            .into_any_element()
    });
    // Pausha 1947 starts on Monday, 22 December 2025.
    click(cx, cal_col_x(0), cal_row_y(0));
    assert_eq!(recorded.borrow().as_slice(), ["2025-12-22"]);
}

#[gpui::test]
fn calendar_indian_locale_keyboard_uses_displayed_months(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let focused = events();
    let recorded = focused.clone();
    let cx = open_host(cx, move || {
        let focused = focused.clone();
        Calendar::new(state.clone())
            .locale("hi-IN-u-ca-indian")
            .default_value(Date::new(2026, 1, 15))
            .on_focus_change(move |date, _, _| focused.borrow_mut().push(date.format_iso()))
            .into_any_element()
    });
    press(cx, "tab");
    press(cx, "home");
    press(cx, "end");
    press(cx, "pagedown");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2025-12-22", "2026-01-20", "2026-02-19"]
    );
}

#[gpui::test]
fn range_calendar_indian_locale_keyboard_uses_displayed_months(cx: &mut TestAppContext) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let focused = events();
    let recorded = focused.clone();
    let cx = open_host(cx, move || {
        let focused = focused.clone();
        RangeCalendar::new(state.clone())
            .locale("hi-IN-u-ca-indian")
            .default_value((Date::new(2026, 1, 15), Date::new(2026, 1, 16)))
            .on_focus_change(move |date, _, _| focused.borrow_mut().push(date.format_iso()))
            .into_any_element()
    });
    press(cx, "tab");
    press(cx, "home");
    press(cx, "end");
    press(cx, "pagedown");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2025-12-22", "2026-01-20", "2026-02-19"]
    );
}

#[gpui::test]
fn range_calendar_indian_locale_end_alignment_keeps_selection_visible(cx: &mut TestAppContext) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        RangeCalendar::new(state_for_view.clone())
            .locale("hi-IN-u-ca-indian")
            .first_day_of_week(Weekday::Mon)
            .visible_duration(VisibleDuration::Months(2))
            .selection_alignment(SelectionAlignment::End)
            .default_value((Date::new(2026, 1, 21), Date::new(2026, 1, 22)))
            .into_any_element()
    });
    // Magha 1 must remain in the second grid; the first grid is Pausha.
    let key = format!(
        r#"NamedInteger("range-cal", {})-1947-10-day-1"#,
        state.entity_id().as_u64()
    );
    let bounds = cx.debug_bounds(Box::leak(key.into_boxed_str())).unwrap();
    click(
        cx,
        f32::from(bounds.center().x),
        f32::from(bounds.center().y),
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).start),
        Some(Date::new(2025, 12, 22))
    );
}

#[gpui::test]
fn calendar_indian_locale_bounds_disable_both_month_controls(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone())
            .locale("hi-IN-u-ca-indian")
            .default_value(Date::new(2026, 1, 15))
            .min_value(Date::new(2025, 12, 22))
            .max_value(Date::new(2026, 1, 20))
            .into_any_element()
    });
    click(cx, 14., 12.);
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        Date::new(2026, 1, 15)
    );
    click(cx, f32::from(CALENDAR_WIDTH) - 14., 12.);
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        Date::new(2026, 1, 15)
    );
}

#[gpui::test]
fn calendar_hebrew_year_picker_preserves_the_month_code(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let focused = events();
    let recorded = focused.clone();
    let cx = open_host(cx, move || {
        let focused = focused.clone();
        Calendar::new(state_for_view.clone())
            .locale("en-US-u-ca-hebrew")
            .default_value(Date::new(2024, 3, 25))
            .default_year_picker_open(true)
            .visible_years(3)
            .on_focus_change(move |date, _, _| focused.borrow_mut().push(date.format_iso()))
            .into_any_element()
    });
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(recorded.borrow().as_slice(), ["2025-03-15"]);
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        Date::new(2025, 3, 15)
    );
}

#[gpui::test]
fn range_calendar_hebrew_year_picker_preserves_the_month_code(cx: &mut TestAppContext) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let focused = events();
    let recorded = focused.clone();
    let cx = open_host(cx, move || {
        let focused = focused.clone();
        RangeCalendar::new(state_for_view.clone())
            .locale("en-US-u-ca-hebrew")
            .default_value((Date::new(2024, 3, 25), Date::new(2024, 3, 26)))
            .default_year_picker_open(true)
            .visible_years(3)
            .on_focus_change(move |date, _, _| focused.borrow_mut().push(date.format_iso()))
            .into_any_element()
    });
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(recorded.borrow().as_slice(), ["2025-03-15"]);
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        Date::new(2025, 3, 15)
    );
}

/// The grid is one tab stop (`util::tab_stop_handle`), so Tab from the host
/// root puts the keyboard inside it; then the arrows walk a day and a week,
/// and Enter and Space both take the ring's date. Asserted through
/// `onFocusChange` and `onChange`, never through the ring drawing.
#[gpui::test]
fn calendar_arrows_walk_days_and_weeks(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let changes = changes.clone();
        // `defaultValue` seeds the selection and the view month, so the ring
        // starts exactly on 2026-08-15 whatever day the tests run.
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .on_change(move |date, _, _| {
                let iso = date.map(|d| d.format_iso()).unwrap_or_default();
                changes.borrow_mut().push(iso);
            })
            .into_any_element()
    });

    // Tab moves the focus from the host root into the calendar grid.
    press(cx, "tab");
    press(cx, "right");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-08-16"],
        "Right must walk the cursor a day forward"
    );
    press(cx, "enter");
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-16"],
        "Enter must select the date the ring is on"
    );
    press(cx, "left");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-08-16", "2026-08-15"],
        "Left must walk a day back"
    );
    press(cx, "down");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-08-16", "2026-08-15", "2026-08-22"],
        "Down must walk a week forward"
    );
    press(cx, "up");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-08-16", "2026-08-15", "2026-08-22", "2026-08-15"],
        "Up must walk a week back"
    );
    press(cx, "space");
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-16", "2026-08-15"],
        "Space must select like Enter"
    );
}

/// PageUp / PageDown step a month (a year with shift) and Home / End jump to
/// the ends of the month, per React Aria's `useCalendarGrid` mapping that v3
/// inherits: `End -> focusSectionEnd()`, `Home -> focusSectionStart()`,
/// `PageUp/Down -> focusPrevious/NextSection()`, and `larger` for shift. The
/// nav buttons move the visible month on their own, read back through the
/// state entity the test owns.
#[gpui::test]
fn calendar_page_keys_change_month(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();

    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "pagedown");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-09-15"],
        "PageDown must step a month forward"
    );
    {
        let (year, month) =
            cx.update(|_, cx| (state.read(cx).view_year, state.read(cx).view_month));
        assert_eq!((year, month), (2026, 9), "the view must follow the cursor");
    }

    // gpui's keystroke syntax joins modifiers with `-`, so a shifted page key
    // is `shift-pagedown`, not `shift+pagedown` (which parses as key "+pagedown").
    press(cx, "shift-pagedown");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-09-15", "2027-09-15"],
        "shift+PageDown must step a year forward"
    );
    press(cx, "shift-pageup");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-09-15", "2027-09-15", "2026-09-15"],
        "shift+PageUp must step a year back"
    );
    press(cx, "pageup");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-09-15", "2027-09-15", "2026-09-15", "2026-08-15"],
        "PageUp must step a month back"
    );
    press(cx, "home");
    assert_eq!(
        focused.borrow().as_slice(),
        [
            "2026-09-15",
            "2027-09-15",
            "2026-09-15",
            "2026-08-15",
            "2026-08-01"
        ],
        "Home must jump to the first day of the month"
    );
    press(cx, "end");
    assert_eq!(
        focused.borrow().as_slice(),
        [
            "2026-09-15",
            "2027-09-15",
            "2026-09-15",
            "2026-08-15",
            "2026-08-01",
            "2026-08-31"
        ],
        "End must jump to the last day of the month"
    );

    // The nav buttons move the visible month directly: previous at (14, 12)
    // and next at (238, 12) inside the 252px header row.
    click(cx, 14., 12.);
    {
        let (year, month) =
            cx.update(|_, cx| (state.read(cx).view_year, state.read(cx).view_month));
        assert_eq!((year, month), (2026, 7), "previous must page back a month");
    }
    click(cx, 238., 12.);
    {
        let (year, month) =
            cx.update(|_, cx| (state.read(cx).view_year, state.read(cx).view_month));
        assert_eq!((year, month), (2026, 8), "next must page forward a month");
    }
}

#[gpui::test]
fn calendar_week_page_keys_move_one_week_and_shift_moves_one_month(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .visible_duration(VisibleDuration::Weeks(2))
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "pagedown");
    press(cx, "shift-pagedown");
    press(cx, "pageup");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-08-22", "2026-09-22", "2026-09-15"],
        "week sections move seven days, while shifted paging moves one month"
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        aligned_anchor(
            VisibleDuration::Weeks(2),
            SelectionAlignment::End,
            Weekday::default(),
            Date::new(2026, 9, 15),
        ),
        "the two-week window must realign at the edge focus crossed"
    );
}

#[gpui::test]
fn calendar_day_page_keys_honor_the_visible_day_count_even_with_shift(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .visible_duration(VisibleDuration::Days(3))
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "pagedown");
    press(cx, "shift-pagedown");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-08-18", "2026-08-21"],
        "day sections use pageBehavior and ignore the shift modifier"
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        Date::new(2026, 8, 20),
        "the rolling three-day window must advance with each page"
    );
}

#[gpui::test]
fn range_calendar_single_day_pages_one_day(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        RangeCalendar::new(state_for_view.clone())
            .default_value((Date::new(2026, 8, 15), Date::new(2026, 8, 16)))
            .visible_duration(VisibleDuration::Days(3))
            .page_behavior(PageBehavior::Single)
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "pagedown");
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        Date::new(2026, 8, 15),
        "single-day paging must advance the range calendar window"
    );
    press(cx, "shift-pageup");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-08-16", "2026-08-15"],
        "single day paging moves one day and ignores shift"
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        Date::new(2026, 8, 14),
        "the range calendar day window must follow both page keys"
    );
}

#[gpui::test]
fn range_calendar_week_page_keys_realign_at_the_visible_boundary(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        RangeCalendar::new(state_for_view.clone())
            .default_value((Date::new(2026, 8, 15), Date::new(2026, 8, 16)))
            .visible_duration(VisibleDuration::Weeks(2))
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "pagedown");
    press(cx, "shift-pagedown");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-08-22", "2026-09-22"],
        "range week sections share the pinned week and shifted-month steps"
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        aligned_anchor(
            VisibleDuration::Weeks(2),
            SelectionAlignment::Start,
            Weekday::default(),
            Date::new(2026, 9, 22),
        ),
        "focus beyond the two-week range must realign it at the start"
    );
}

#[gpui::test]
fn calendar_week_home_end_use_the_locale_week(cx: &mut TestAppContext) {
    let grid_first_day = if Weekday::default() == Weekday::Mon {
        Weekday::Sun
    } else {
        Weekday::Mon
    };
    let (home, end, anchor_after_home, anchor_after_end) =
        week_home_end_expectations(Date::new(2026, 8, 15), grid_first_day);
    let focuses = events();
    let focused = focuses.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .visible_duration(VisibleDuration::Weeks(2))
            .first_day_of_week(grid_first_day)
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "home");
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        anchor_after_home,
        "a non-locale grid override must realign so the locale week start remains visible"
    );
    press(cx, "end");
    assert_eq!(
        focused.borrow().as_slice(),
        [home.format_iso(), end.format_iso()],
        "week section bounds use the system locale even when the grid overrides its first day"
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        anchor_after_end,
        "End must realign the grid forward after the locale week crosses its visible edge"
    );
}

#[gpui::test]
fn calendar_week_home_end_follow_a_non_sunday_time_locale(cx: &mut TestAppContext) {
    const CHILD: &str = "HEROGPUI_NON_SUNDAY_TIME_LOCALE_TEST";
    if std::env::var_os(CHILD).is_none() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "calendar_week_home_end_follow_a_non_sunday_time_locale",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env_remove("LC_ALL")
            .env("LC_TIME", "de_DE.UTF-8")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "non-Sunday locale child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return;
    }

    assert_eq!(Weekday::default(), Weekday::Mon);
    let focuses = events();
    let focused = focuses.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .visible_duration(VisibleDuration::Weeks(2))
            .first_day_of_week(Weekday::Sun)
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "home");
    press(cx, "end");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-08-10", "2026-08-16"],
        "LC_TIME must determine the week bounds independently of the Sunday grid override"
    );
}

#[gpui::test]
fn calendar_day_home_end_use_the_visible_window(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        Calendar::new(state.clone())
            .default_value(Date::new(2026, 8, 15))
            .visible_duration(VisibleDuration::Days(3))
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "home");
    press(cx, "end");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-08-14", "2026-08-16"],
        "day section bounds are the already-visible dates, not the focused date's month"
    );
}

#[gpui::test]
fn range_calendar_week_home_end_use_the_locale_week(cx: &mut TestAppContext) {
    let grid_first_day = if Weekday::default() == Weekday::Mon {
        Weekday::Sun
    } else {
        Weekday::Mon
    };
    let (home, end, anchor_after_home, anchor_after_end) =
        week_home_end_expectations(Date::new(2026, 8, 15), grid_first_day);
    let focuses = events();
    let focused = focuses.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        RangeCalendar::new(state_for_view.clone())
            .default_value((Date::new(2026, 8, 15), Date::new(2026, 8, 16)))
            .visible_duration(VisibleDuration::Weeks(2))
            .first_day_of_week(grid_first_day)
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "home");
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        anchor_after_home,
        "RangeCalendar must keep the locale-week Home target visible"
    );
    press(cx, "end");
    assert_eq!(
        focused.borrow().as_slice(),
        [home.format_iso(), end.format_iso()],
        "RangeCalendar must share Calendar's locale-week section bounds"
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        anchor_after_end,
        "RangeCalendar must realign again when End crosses the visible edge"
    );
}

/// A day outside `minValue`/`maxValue` is not selectable by either input
/// path: a click on it records nothing, and Enter with the ring on it is
/// stopped by the same `constraints.allows` gate (`calendar.rs`'s key
/// handler). The in-range neighbours all still answer.
#[gpui::test]
fn calendar_min_max_blocks_out_of_range_days(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .min_value(Date::new(2026, 8, 10))
            .max_value(Date::new(2026, 8, 20))
            .on_change(move |date, _, _| {
                let iso = date.map(|d| d.format_iso()).unwrap_or_default();
                changes.borrow_mut().push(iso);
            })
            .into_any_element()
    });

    // August 2026: day 5 is before min (10) in the same grid.
    let (out_x, out_y) = cal_day(2026, 8, 5);
    click(cx, out_x, out_y);
    assert!(
        changed.borrow().is_empty(),
        "clicking an out-of-range day must not select it"
    );

    let (in_x, in_y) = cal_day(2026, 8, 15);
    click(cx, in_x, in_y);
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-15"],
        "an in-range day must still answer the pointer"
    );

    // The pointer leaves the grid focused on its seeded selection (15); five
    // Rights walk to 20 (inside), six to 21 (outside). Enter on 21 is blocked.
    press(cx, "right right right right right");
    press(cx, "enter");
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-15", "2026-08-20"],
        "Enter on the in-range end of the walk must select it"
    );
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-15", "2026-08-20"],
        "Enter on an out-of-range day must not select it"
    );
}

/// A RangeCalendar's range is a two-step anchor/extend interaction: the first
/// The first click holds an internal anchor and the second reports the sorted
/// completed range in either direction. A click after a complete range starts
/// a new anchor.
/// The hover preview between the two ends must drive the drawing and never a
/// callback.
#[gpui::test]
fn range_calendar_click_start_then_end_reports_completed_ranges(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    // Pin the visible month so the click arithmetic is independent of the
    // day the suite runs on.
    state.update(cx, |s, _| {
        s.view_year = 2026;
        s.view_month = 8;
        s.view_day = 1;
        s.user_navigated = true;
    });
    let state_for_view = state.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        RangeCalendar::new(state_for_view.clone())
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    // First click: the anchor remains internal selection state.
    let (day5_x, day5_y) = range_day(2026, 8, 5);
    click(cx, day5_x, day5_y);
    assert!(changed.borrow().is_empty());

    // Hover a later day: the preview paints the range 5..8 (its render reads
    // `state.hovered`), but no callback may fire on its own.
    let (day8_x, day8_y) = range_day(2026, 8, 8);
    cx.simulate_mouse_move(
        point(px(day8_x), px(day8_y)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    {
        let hovered = cx.update(|_, cx| state.read(cx).hovered);
        assert_eq!(
            hovered,
            Some(Date::new(2026, 8, 8)),
            "the hover must reach the cell it passed over"
        );
    }
    assert_eq!(
        changed.borrow().len(),
        0,
        "the hover preview must not report anything on its own"
    );

    // A second click on an earlier day completes the range in reverse. React
    // Aria sorts the two endpoints rather than discarding the first anchor.
    let (day2_x, day2_y) = range_day(2026, 8, 2);
    click(cx, day2_x, day2_y);
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-02->2026-08-05"],
        "a second pick earlier than the anchor must complete the sorted range"
    );

    // A complete range starts over on the next click, then a later click
    // completes that new range.
    click(cx, day2_x, day2_y);
    let (day12_x, day12_y) = range_day(2026, 8, 12);
    click(cx, day12_x, day12_y);
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-02->2026-08-05", "2026-08-02->2026-08-12",],
        "a later pick must complete and report the whole range"
    );
}

// ---------------------------------------------------------------------------
// ColorPicker (ColorSwatch is a static display; see the report)
// ---------------------------------------------------------------------------

/// The trigger opens the panel, a press on the colour area reports a colour,
/// and Escape closes it again. React Aria resolves a pointer against the
/// area's own bounds, so the expected hex subtracts the area origin before
/// deriving the fractions and compares strings — never floats (`float_cmp`
/// is denied).
#[gpui::test]
fn color_picker_trigger_opens_and_area_reports(cx: &mut TestAppContext) {
    harness::still();
    let colors = events();
    let reported = colors.clone();
    let opens = events();
    let opened = opens.clone();
    let open = Rc::new(RefCell::new(false));

    // The component's recorded border-box turns this panel-relative point into
    // the pinned local-coordinate colour. The direct offset tests derive the
    // fractions independently; this integration keeps the resulting hex.
    let expected_hex = "#6490BD";

    let cx = open_host(cx, move || {
        let colors = colors.clone();
        let opens = opens.clone();
        let open = open.clone();
        let is_open = *open.borrow();
        ColorPicker::new("cp-area", PickerColor::hsb(210.0, 0.5, 0.6))
            .default_value(PickerColor::hsb(210.0, 0.5, 0.6))
            .is_open(is_open)
            .on_open_change(move |v, window, _| {
                *open.borrow_mut() = *v;
                opens.borrow_mut().push(format!("open:{v}"));
                window.refresh();
            })
            .on_change(move |color, _, _| {
                colors.borrow_mut().push(color.to_hex());
            })
            .into_any_element()
    });

    // Trigger: swatch (24px) + hex label, 24px tall at the origin.
    click(cx, 60., 12.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the trigger must open the popover"
    );

    // The area: panel top (24 trigger + 8) + pt-2 (8) puts the 160px-tall
    // area at y 40..200; the 12px viewport inset plus px-2 puts it at x 20..260. The press reports its
    // local fractions within those bounds.
    click(cx, 132., 82.);
    assert_eq!(
        reported.borrow().as_slice(),
        [expected_hex],
        "a press on the area must report the colour it derives from the position"
    );

    // Tab moves the focus trigger -> area; Escape is read on the picker root
    // (`dismiss_on_escape`), which the focused area's key event bubbles to.
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "escape");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "Escape must close the popover"
    );

    // Closed proof: where the area was, nothing answers now.
    click(cx, 132., 82.);
    assert_eq!(
        reported.borrow().as_slice(),
        [expected_hex],
        "the popover must be gone after escape"
    );
}

#[gpui::test]
fn color_picker_default_trigger_owns_open_state_without_callback(cx: &mut TestAppContext) {
    harness::still();
    let colors = events();
    let reported = colors.clone();
    let cx = open_host(cx, move || {
        let reported = reported.clone();
        ColorPicker::new("cp-default-open", PickerColor::hsb(210.0, 0.5, 0.6))
            .on_change(move |color, _, _| reported.borrow_mut().push(color.to_hex()))
            .into_any_element()
    });

    click(cx, 60., 12.);
    click(cx, 132., 82.);
    assert_eq!(
        colors.borrow().as_slice(),
        ["#6490BD"],
        "the default trigger must open its own popover and expose the color controls"
    );

    press(cx, "tab tab escape");
    cx.executor().advance_clock(Duration::from_millis(150));
    click(cx, 132., 82.);
    assert_eq!(
        colors.borrow().as_slice(),
        ["#6490BD"],
        "Escape must close the uncontrolled popover without an owner callback"
    );
}

#[gpui::test]
fn pointer_open_color_picker_closes_when_focus_moves_elsewhere(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let open = Rc::new(RefCell::new(false));
    let next = cx.new(|cx| InputState::new(cx));
    let next_for_view = next.clone();

    let cx = open_host(cx, move || {
        let opens = opens.clone();
        let open = open.clone();
        let is_open = *open.borrow();
        gpui::div()
            .child(
                ColorPicker::new("cp-blur", PickerColor::hsb(210.0, 0.5, 0.6))
                    .is_open(is_open)
                    .on_open_change(move |value, window, _| {
                        *open.borrow_mut() = *value;
                        opens.borrow_mut().push(format!("open:{value}"));
                        window.refresh();
                    }),
            )
            .child(Input::new(next_for_view.clone()))
            .into_any_element()
    });

    click(cx, 60., 12.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    cx.update(|window, cx| window.focus(&next.read(cx).focus_handle(cx), cx));
    cx.update(|window, _| window.refresh());

    assert_eq!(opened.borrow().as_slice(), ["open:true", "open:false"]);
    assert!(cx.update(|window, cx| next.read(cx).focus_handle(cx).is_focused(window)));
}

// ---------------------------------------------------------------------------
// Select
// ---------------------------------------------------------------------------

/// `selectionMode="multiple"` keeps the popover open between picks, and each
/// pick is reported as the caller's selection plus the clicked row (`select.rs`
/// hands its `selected_keys` back and stores nothing of its own, so the
/// caller owns the growing set). This test replays that ownership the way a
/// gallery page would — through an `Rc` the render closure reads and a
/// `window.refresh()` in the callback — and the outside-press dismissal still
/// works when the caller finally walks away.
#[gpui::test]
fn select_multiple_accumulates_and_keeps_the_panel_open(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();
    let opens = events();
    let opened = opens.clone();
    let selection = Rc::new(RefCell::new(Vec::<SharedString>::new()));

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let opens = opens.clone();
        let selection = selection.clone();
        let selection_now = selection.borrow().clone();
        Select::new("sel-multi", keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .selected_keys(selection_now)
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .on_selection_change_all(move |keys, window, _| {
                *selection.borrow_mut() = keys.to_vec();
                // The port reports the merged set without storing it, so the next
                // report accumulates only if the caller renders it back in.
                window.refresh();
                let joined = keys
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                picks.borrow_mut().push(joined);
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // Row *i* centres at y = 66 + 36i inside the popover.
    click(cx, 60., 66.);
    assert_eq!(picked.borrow().as_slice(), ["Alpha"]);

    click(cx, 60., 138.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha", "Alpha,Gamma"],
        "the second report must still contain the first pick"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "a multiple select must keep the panel open between picks"
    );

    click(cx, 60., 102.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha", "Alpha,Gamma", "Alpha,Beta,Gamma"],
        "the third pick must join the accumulated set"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the panel must still be open after the third pick"
    );

    // Pressing the bare page outside the popover dismisses it: the panel is
    // 320 wide and about 150 tall, so (600, 300) is clear of it.
    click(cx, 600., 300.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "an outside press must dismiss the popover even in multiple mode"
    );
}

/// Pinned Select accepts a `Key[]` as `defaultValue`. In multiple mode that
/// seed belongs to the Select: later picks toggle against the held set even
/// when the caller only observes `onChange` and never feeds a controlled value
/// back.
#[gpui::test]
fn select_uncontrolled_multiple_default_accumulates_and_toggles(cx: &mut TestAppContext) {
    let picks = events();
    let recorded = picks.clone();
    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new("sel-multi-default", keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .default_selected_keys(["Alpha".into(), "Gamma".into()])
            .default_open(true)
            .on_selection_change_all(move |keys, _, _| {
                picks.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    // Row *i* centres at y = 66 + 36i inside the already-open popover.
    click(cx, 60., 102.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha,Beta,Gamma"],
        "the first pick must extend the uncontrolled default array"
    );

    click(cx, 60., 66.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha,Beta,Gamma", "Beta,Gamma"],
        "a later pick must toggle against the Select-owned current set"
    );
}

#[gpui::test]
fn select_uncontrolled_multiple_keyboard_toggles_and_stays_open(cx: &mut TestAppContext) {
    let picks = events();
    let recorded = picks.clone();
    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new("sel-multi-default-keys", keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .default_selected_keys(["Alpha".into()])
            .on_selection_change_all(move |keys, _, _| {
                picks.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    press(cx, "tab");
    // The opening Down is consumed by the trigger and seats the cursor on the
    // selected Alpha; one more Down reaches Beta for Enter to toggle.
    press(cx, "down down enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha,Beta"],
        "Enter must toggle the highlighted row against the uncontrolled default"
    );

    // The multiple popover remains open after keyboard activation, so its
    // third row still answers a pointer press.
    click(cx, 60., 138.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha,Beta", "Alpha,Beta,Gamma"],
        "keyboard activation must not close a multiple Select"
    );
}

#[gpui::test]
fn select_controlled_multiple_waits_for_owner_acceptance(cx: &mut TestAppContext) {
    let picks = events();
    let recorded = picks.clone();
    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new(
            "sel-multi-controlled-reject",
            keyed(&["Alpha", "Beta", "Gamma"]),
        )
        .selection_mode(SelectionMode::Multiple)
        .selected_keys(["Alpha".into()])
        .default_open(true)
        .on_selection_change_all(move |keys, _, _| {
            picks.borrow_mut().push(
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .into_any_element()
    });

    click(cx, 60., 102.);
    click(cx, 60., 138.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Alpha,Beta", "Alpha,Gamma"],
        "controlled proposals must keep starting from the owner-supplied set"
    );
}

/// React Aria's typeahead, which `list_nav::typeahead` implements, works in
/// both shapes: typed on a *closed* select it picks the matching option where
/// it stands (no popover), and typed on an *open* one it moves the highlight,
/// which Enter then activates. Two selects with distinct ids so the two halves
/// do not share a typeahead buffer.
#[gpui::test]
fn select_typeahead_moves_the_highlight(cx: &mut TestAppContext) {
    let picked = events();
    let changes = picked.clone();
    let opened_closed = events();
    let closed_opens = opened_closed.clone();
    let opened_open = events();
    let open_opens = opened_open.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let open_pick_changes = changes.clone();
        let closed_opens = closed_opens.clone();
        let open_opens = open_opens.clone();
        // Two selects 600px apart so the lower one's popover never overlaps
        // the upper trigger.
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(600.))
            .child(
                Select::new("sel-ta-closed", keyed(&["Alpha", "Astra", "Go", "Zig"]))
                    .on_change(move |key, _, _| {
                        changes.borrow_mut().push(format!("{key:?}"));
                    })
                    .on_open_change(move |open, _, _| {
                        closed_opens.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .child(
                Select::new("sel-ta-open", keyed(&["Alpha", "Rust", "Go", "Zig"]))
                    .on_selection_change(move |key, _, _| {
                        open_pick_changes.borrow_mut().push(format!("{key:?}"));
                    })
                    .on_open_change(move |open, _, _| {
                        open_opens.borrow_mut().push(format!("open:{open}"));
                    }),
            )
            .into_any_element()
    });

    // Closed half: Tab reaches the upper trigger without opening it. A letter
    // picks the matching option where it stands; repeating it walks to the
    // *next* row with that initial (React Aria's `aa` behaviour, which the
    // port's `Typeahead` reproduces), so the second pick lands on Astra only
    // if the first one (Alpha) was remembered as the cursor.
    press(cx, "tab");
    press(cx, "a");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"Alpha\")"],
        "a letter on the closed select must pick the matching option"
    );
    assert!(
        opened_closed.borrow().is_empty(),
        "the closed typeahead must not open the popover"
    );
    press(cx, "a");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"Alpha\")", "Some(\"Astra\")"],
        "repeating the letter must walk to the next row with that initial"
    );

    // Open half: the trigger at y 636..672 after the 600px gap. "r" moves the
    // highlight to Rust (index 1), and Enter activates the highlighted row
    // and closes (gpui fires the trigger's click listener on the same key,
    // which is the close).
    click(cx, 60., 654.);
    assert_eq!(opened_open.borrow().as_slice(), ["open:true"]);
    press(cx, "r");
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"Alpha\")", "Some(\"Astra\")", "Some(\"Rust\")"],
        "Enter must activate the row the typeahead highlighted"
    );
    assert_eq!(
        opened_open.borrow().as_slice(),
        ["open:true", "open:false"],
        "the Enter pick must dismiss the open popover"
    );
}

/// A `ListBox.Section` heading is decoration: a click on it records nothing
/// (it has no handlers), the popover does not close, and the arrows land on
/// options — the cursor's stops are the option indices, so the section never
/// consumes one. The option under the heading still answers both paths.
///
/// A single-mode pick also *reports* the close it performs: a caller who
/// drives `isOpen` from `onOpenChange` has to hear `open:false`. This was a
/// divergence — the row flipped its own keyed open flag without the callback,
/// so a controlled caller kept believing the panel was open and the next
/// render reopened it — so the panel's closure is asserted here through the
/// callback, exactly once, rather than with a probe click.
#[gpui::test]
fn select_row_pick_reports_the_close_once(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();
    let opens = events();
    let opened = opens.clone();
    let open = Rc::new(RefCell::new(false));
    let open_for_view = open.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let opens = opens.clone();
        let open = open_for_view.clone();
        let is_open = *open.borrow();
        Select::new(
            "sel-sections",
            keyed(&["Apple", "Banana", "Cherry", "Durian", "Fig"]),
        )
        .section_before("Durian", "Tropical")
        .is_open(is_open)
        .on_selection_change(move |key, _, _| {
            picks.borrow_mut().push(format!("{key:?}"));
        })
        .on_open_change(move |v, window, _| {
            *open.borrow_mut() = *v;
            opens.borrow_mut().push(format!("open:{v}"));
            window.refresh();
        })
        .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // Rows 0..2 occupy y 48..156; the row-3 slot (156..216) holds the
    // `pt-6 pb-2` heading across its top ~24px and the option's 36px below.
    click(cx, 60., 165.);
    assert!(
        picked.borrow().is_empty(),
        "a section heading must not be selectable"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "a press on the heading must not dismiss the popover either"
    );

    // The option under the heading is a normal pick: the selection is
    // reported, and the close is reported exactly once — the caller's flag
    // flips to false, so the next render keeps the panel shut.
    click(cx, 60., 195.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"Durian\")"],
        "the option a section announces must still be clickable"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the pick must report the close exactly once"
    );
    assert!(
        !*open.borrow(),
        "the caller driving isOpen must now read closed"
    );

    // The panel is gone, so a press where row 0 was records nothing either
    // way — and nothing else reports a close.
    click(cx, 60., 66.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"Durian\")"],
        "after the pick the popover must be gone, so the old row answers nothing"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "a closed panel must not report anything further"
    );

    // Keyboard: the pointer reopen seats the cursor on the selected Durian,
    // past the heading. Walking Up to the top and back Down crosses the
    // `pt-6 pb-2` "Tropical" heading in both directions without stopping on
    // it, so the option it announces is what Enter commits. The heading is
    // never a stop, so it cannot be activated, and this close is the
    // trigger's own click listener (gpui fires a focused element's click on
    // Enter), which reports through the same callback — a second `open:false`,
    // one per pick, never two for the same one.
    click(cx, 60., 18.);
    press(cx, "up up up down down down");
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"Durian\")", "Some(\"Durian\")"],
        "the arrows must skip the section heading, stopping on its option"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false", "open:true", "open:false"],
        "the Enter pick must close through the trigger's own click, once"
    );
}

/// Multiple mode keeps the panel open for the next pick, so a pick must not
/// report `open:false` — a caller driving `isOpen` from `onOpenChange` would
/// close the panel between picks. The picks accumulate through the caller's
/// own set (the port hands the merged selection back and stores nothing), and
/// the panel is still answering after each one.
#[gpui::test]
fn select_multiple_picks_report_no_close(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();
    let opens = events();
    let opened = opens.clone();
    let open = Rc::new(RefCell::new(false));
    let selection = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let open_for_view = open.clone();
    let selection_for_view = selection;

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let opens = opens.clone();
        let open = open_for_view.clone();
        let selection = selection_for_view.clone();
        // Pre-extracted so no `Ref` borrow survives into the builder chain,
        // which moves `open` into the `on_open_change` closure.
        let is_open = *open.borrow();
        let selection_now = selection.borrow().clone();
        Select::new("sel-multi-close", keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .is_open(is_open)
            .selected_keys(selection_now)
            .on_open_change(move |v, window, _| {
                *open.borrow_mut() = *v;
                opens.borrow_mut().push(format!("open:{v}"));
                window.refresh();
            })
            .on_selection_change_all(move |keys, window, _| {
                *selection.borrow_mut() = keys.to_vec();
                window.refresh();
                let joined = keys
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                picks.borrow_mut().push(joined);
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // Row *i* centres at y = 66 + 36i inside the popover.
    click(cx, 60., 66.);
    assert_eq!(picked.borrow().as_slice(), ["Alpha"]);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "a multiple pick must not report a close"
    );

    // The panel is still open and answering, so the next pick lands — and it
    // still reports nothing about the open state.
    click(cx, 60., 138.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha", "Alpha,Gamma"],
        "the second pick must join the accumulated set"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the panel must stay open across picks, with no close reports"
    );
    assert!(
        *open.borrow(),
        "the caller driving isOpen must still read open"
    );
}

/// PageUp/PageDown belong to the *open* list: pinned `useSelectableCollection`
/// binds them through the collection's keyboard handling, which a closed
/// Select never runs. The proof is not vacuous: the list is opened once and
/// closed with Escape, which leaves the focus on the trigger, so the page keys
/// land on the very handler that would answer them on an open list -- and a
/// following Down still opens. A page key on the closed trigger must not open
/// the list and must not move a selection.
#[gpui::test]
fn select_page_keys_ignore_a_closed_trigger(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();
    let opens = events();
    let opened = opens.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let opens = opens.clone();
        Select::new("sel-page-closed", keyed(&["Alpha", "Beta", "Gamma"]))
            .on_selection_change(move |key, _, _| {
                picks.borrow_mut().push(format!("{key:?}"));
            })
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    // Open, then close with Escape: the trigger keeps the focus. On a
    // never-opened trigger the presses could be lost before the component and
    // prove nothing.
    click(cx, 60., 18.);
    press(cx, "escape");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the probe must begin from a list the Escape closed onto its trigger"
    );

    press(cx, "pagedown");
    press(cx, "pageup");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "a page key on the closed trigger must not open the list"
    );
    assert!(
        picked.borrow().is_empty(),
        "a page key on the closed trigger must not move the selection"
    );

    // The presses were delivered: the same focused handler still opens on
    // Down, and the select then picks as before the page keys arrived.
    press(cx, "down");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false", "open:true"],
        "the page keys must have reached a live handler, not a dead one"
    );
    // The reopening Down seats the cursor on the first row; Enter picks it.
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"Alpha\")"],
        "the page keys must have left the closed select answering as before"
    );
}

/// Pinned HeroUI v3.2.4 scrolls the *popover*, with the ListBox element itself
/// `overflow-clip`, so pinned React Aria 3.51.0 never sees a scrollable list
/// behind a Select: page keys take the enabled ends whatever the panel could
/// have shown. Those handlers require a focused key, though -- a mouse-opened,
/// selection-less Select has a null cursor, so its page keys are inert; a real
/// Down (or Down on the closed trigger, which opens) establishes the cursor,
/// and paging from there reaches the first and last enabled rows, never the
/// disabled rows at the ends. Paging only moves the highlight until Enter
/// commits it.
#[gpui::test]
fn select_page_keys_reach_enabled_ends_on_a_short_panel(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new("sel-page-short", keyed(&["0", "1", "2", "3", "4", "5"]))
            .disabled_keys(["0".into(), "5".into()])
            .on_selection_change(move |key, _, _| {
                picks.borrow_mut().push(format!("{key:?}"));
            })
            .into_any_element()
    });

    // Six 36px rows (216px) fit the capped panel. Mouse-open with no
    // selection: the cursor is null, so both page keys must be inert. Down
    // from a null cursor enters the first enabled row (1); had either page
    // key created a cursor, Down would hold on that end or step off it --
    // Some("2") from PageUp's first stop, Some("4") from PageDown's last --
    // and the pick would betray the unconditional cursor creation.
    click(cx, 60., 18.);
    press(cx, "pagedown");
    press(cx, "pageup");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"1\")"],
        "page keys on a mouse-opened, cursor-less list must be inert: \
         Down must still enter the first enabled row"
    );

    // Enter closed the list; reopen and press Down so keyboard navigation
    // establishes the cursor (on 2), then PageDown must take the last
    // enabled row (4), never the disabled 5.
    click(cx, 60., 18.);
    press(cx, "down");
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"1\")", "Some(\"4\")"],
        "PageDown with a cursor must reach the last enabled row"
    );

    // Reopen once more; the cursor stands where PageDown left it, and
    // PageUp must walk back to the first enabled row (1), never the
    // disabled 0.
    click(cx, 60., 18.);
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"1\")", "Some(\"4\")", "Some(\"1\")"],
        "PageUp with a cursor must reach the first enabled row"
    );
}

/// The same ends answer the virtualized list: `rowHeight` projects the rows
/// into a fixed 280px viewport, and pinned React Aria still treats Select's
/// list as non-scrollable. Its page handlers still require a focused key, so
/// the mouse-opened cursor-less list ignores them, and only after keyboard
/// navigation establishes the cursor does PageDown land on the last enabled
/// row and PageUp walk back to the first -- no viewport step to preserve.
#[gpui::test]
fn select_page_keys_reach_enabled_ends_on_a_virtual_list(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();
    let options: Vec<PickerItem> = (0..30)
        .map(|i| {
            let label = format!("Option {i:02}");
            PickerItem::new(label.clone(), label)
        })
        .collect();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new("sel-page-virtual", options.clone())
            .row_height(px(36.))
            .on_selection_change(move |key, _, _| {
                picks.borrow_mut().push(format!("{key:?}"));
            })
            .into_any_element()
    });

    // Mouse-open with no selection: both page keys must be inert. Down from a
    // null cursor enters row 0; had a page key created a cursor at an end,
    // Down would hold on 29 or step to 1 and the pick would expose it.
    click(cx, 60., 18.);
    press(cx, "pagedown");
    press(cx, "pageup");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"Option 00\")"],
        "page keys on a mouse-opened, cursor-less virtual list must be inert: \
         Down must still enter the first row"
    );

    // Enter closed the list; reopen and press Down so keyboard navigation
    // establishes the cursor (on 1), then PageDown must take the last
    // enabled row (29).
    click(cx, 60., 18.);
    press(cx, "down");
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"Option 00\")", "Some(\"Option 29\")"],
        "PageDown with a cursor must reach the last enabled row of the virtual list"
    );

    // Reopen once more; the cursor stands where PageDown left it, and PageUp
    // must return to the first row.
    click(cx, 60., 18.);
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        [
            "Some(\"Option 00\")",
            "Some(\"Option 29\")",
            "Some(\"Option 00\")"
        ],
        "PageUp with a cursor must return to the first enabled row of the virtual list"
    );
}

/// A long plain list (no `rowHeight`, every option laid out for real inside
/// the capped 280px panel) scrolls, but the popover owns that scrolling and
/// the page keys never consult it. They still require a focused key first:
/// the mouse-opened cursor-less list ignores them, and only after keyboard
/// navigation establishes the cursor does PageDown land on the last enabled
/// row and PageUp back on the first, skipping the disabled rows at both ends.
#[gpui::test]
fn select_page_keys_reach_enabled_ends_on_a_scrolled_list(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();
    let options: Vec<PickerItem> = (0..20)
        .map(|i| {
            let label = format!("Option {i:02}");
            PickerItem::new(label.clone(), label)
        })
        .collect();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new("sel-page-plain", options.clone())
            .disabled_keys(["Option 00".into(), "Option 19".into()])
            .on_selection_change(move |key, _, _| {
                picks.borrow_mut().push(format!("{key:?}"));
            })
            .into_any_element()
    });

    // Mouse-open with no selection: both page keys must be inert. Down from a
    // null cursor enters the first enabled row (1); had a page key created a
    // cursor at an end, Down would hold on 18 or step to 2 and the pick would
    // expose it.
    click(cx, 60., 18.);
    press(cx, "pagedown");
    press(cx, "pageup");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"Option 01\")"],
        "page keys on a mouse-opened, cursor-less scrolled list must be inert: \
         Down must still enter the first enabled row"
    );

    // Enter closed the list; reopen and press Down so keyboard navigation
    // establishes the cursor (on 2), then PageDown must take the last
    // enabled row (18), never the disabled 19.
    click(cx, 60., 18.);
    press(cx, "down");
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"Option 01\")", "Some(\"Option 18\")"],
        "PageDown on a scrolled list must reach the last enabled row"
    );

    // Reopen once more; the cursor stands where PageDown left it, and PageUp
    // must walk back to the first enabled row (1), never the disabled 0.
    click(cx, 60., 18.);
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        [
            "Some(\"Option 01\")",
            "Some(\"Option 18\")",
            "Some(\"Option 01\")"
        ],
        "PageUp on a scrolled list must reach the first enabled row"
    );
}

/// The platform Mod (`Ctrl` on Windows and Linux, `Cmd` on macOS), so the
/// select-all tests prove the chord their host actually sends.
fn press_mod_a(cx: &mut VisualTestContext) {
    if cfg!(target_os = "macos") {
        press(cx, "cmd-a");
    } else {
        press(cx, "ctrl-a");
    }
}

/// Pinned React Stately's `extendSelection` replaces the anchor..current range
/// with anchor..target: Shift+Down grows the range from the anchor, a reverse
/// Shift+Up shrinks it again. The anchor is seated by the Enter toggle that
/// added the first key.
#[gpui::test]
fn select_shift_arrows_extend_and_reverse_shrink(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new(
            "sel-range-arrows",
            keyed(&["Alpha", "Beta", "Gamma", "Delta"]),
        )
        .selection_mode(SelectionMode::Multiple)
        .default_open(true)
        .on_selection_change_all(move |keys, _, _| {
            picks.borrow_mut().push(
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .into_any_element()
    });

    press(cx, "tab down enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha"],
        "the Enter toggle must seat the anchor on the added key"
    );

    press(cx, "shift-down shift-down");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha", "Alpha,Beta", "Alpha,Beta,Gamma"],
        "Shift+Down must extend the anchor's range forward"
    );

    press(cx, "shift-up");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha", "Alpha,Beta", "Alpha,Beta,Gamma", "Alpha,Beta"],
        "a reverse Shift+Up must shrink the old anchor..cursor range"
    );

    press(cx, "shift-up");
    assert_eq!(
        picked.borrow().as_slice(),
        [
            "Alpha",
            "Alpha,Beta",
            "Alpha,Beta,Gamma",
            "Alpha,Beta",
            "Alpha"
        ],
        "the shrink must replace the range, not toggle keys off"
    );

    press(cx, "shift-up");
    assert_eq!(
        picked.borrow().as_slice(),
        [
            "Alpha",
            "Alpha,Beta",
            "Alpha,Beta,Gamma",
            "Alpha,Beta",
            "Alpha"
        ],
        "the held Shift+Up at the boundary must run no extension and report \
         nothing: the pinned arrow delegate returns null there"
    );
}

/// A Shift click extends from the seated anchor through `extendSelection`, and
/// disabled keys never join the range; a reverse Shift click shrinks. An
/// ordinary click toggles against the uncontrolled set and re-anchors on the
/// add.
#[gpui::test]
fn select_shift_click_extends_and_disabled_keys_stay_out(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new(
            "sel-shift-click",
            keyed(&["Alpha", "Beta", "Held", "Delta", "Echo"]),
        )
        .selection_mode(SelectionMode::Multiple)
        .disabled_keys(["Held".into()])
        .default_open(true)
        .on_selection_change_all(move |keys, _, _| {
            picks.borrow_mut().push(
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .into_any_element()
    });

    let mut shift = Modifiers::none();
    shift.shift = true;

    // Row *i* centres at y = 66 + 36i inside the popover.
    click(cx, 60., 66.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha"],
        "the plain click must seat the anchor on the added key"
    );

    cx.simulate_click(point(px(60.), px(210.)), shift);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha", "Alpha,Beta,Delta,Echo"],
        "Shift+Click must extend across the disabled row without selecting it"
    );

    cx.simulate_click(point(px(60.), px(102.)), shift);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha", "Alpha,Beta,Delta,Echo", "Alpha,Beta"],
        "a reverse Shift+Click must shrink the old anchor..cursor range"
    );

    click(cx, 60., 66.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha", "Alpha,Beta,Delta,Echo", "Alpha,Beta", "Beta"],
        "an ordinary click must toggle against the current set, not extend"
    );
}

/// Pinned `useSelectableCollection` registers Home and End per platform:
/// Windows and Linux install none, Shift, Control, and Control+Shift, and
/// only Control+Shift extends; macOS installs none, Shift, Alt, and Alt+Shift
/// only, so every Control-bearing chord is entirely inert. Each branch drives
/// its own host's real chords; the cfg-free unit truth tables in `select.rs`
/// prove both maps everywhere.
#[gpui::test]
fn select_shift_home_end_follow_the_registered_chords(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new(
            "sel-shift-home-end",
            keyed(&["Alpha", "Beta", "Gamma", "Delta", "Held"]),
        )
        .selection_mode(SelectionMode::Multiple)
        .disabled_keys(["Held".into()])
        .default_open(true)
        .on_selection_change_all(move |keys, _, _| {
            picks.borrow_mut().push(
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .into_any_element()
    });

    press(cx, "tab down enter");
    assert_eq!(picked.borrow().as_slice(), ["Alpha"]);

    press(cx, "ctrl-shift-end");
    if cfg!(target_os = "macos") {
        assert_eq!(
            picked.borrow().as_slice(),
            ["Alpha"],
            "macOS must leave Control-bearing Home/End entirely inert"
        );
    } else {
        assert_eq!(
            picked.borrow().as_slice(),
            ["Alpha", "Alpha,Beta,Gamma,Delta"],
            "Control+Shift+End must extend the range to the last enabled option"
        );
    }

    press(cx, "shift-home");
    assert_eq!(
        picked.borrow().as_slice(),
        if cfg!(target_os = "macos") {
            &["Alpha"][..]
        } else {
            &["Alpha", "Alpha,Beta,Gamma,Delta"][..]
        },
        "plain Shift+Home must only move the cursor, on every platform"
    );

    if cfg!(target_os = "macos") {
        // Alt+Shift *is* registered on macOS: the cursor walks, extends nothing.
        press(cx, "alt-shift-end");
        assert_eq!(
            picked.borrow().as_slice(),
            ["Alpha"],
            "Alt+Shift+End must walk the cursor without extending"
        );
        // The Enter target betrays where the cursor was left: on Delta, and
        // the selection still only holds Alpha, so the toggle adds it.
        press(cx, "enter");
        assert_eq!(
            picked.borrow().as_slice(),
            ["Alpha", "Alpha,Delta"],
            "Alt+Shift+End must have walked the cursor without extending and \
             the inert Control chord must never have selected the range"
        );
    } else {
        // Alt-bearing chords sit outside the Windows/Linux registration: the
        // event is entirely inert, not even the cursor moves.
        press(cx, "alt-shift-end");
        assert_eq!(
            picked.borrow().as_slice(),
            ["Alpha", "Alpha,Beta,Gamma,Delta"],
            "an unregistered Alt-bearing chord must leave Home and End inert"
        );
        // The cursor stayed on Alpha through the inert chord, so Enter
        // toggles it.
        press(cx, "enter");
        assert_eq!(
            picked.borrow().as_slice(),
            ["Alpha", "Alpha,Beta,Gamma,Delta", "Beta,Gamma,Delta"],
            "the inert chord must have left the cursor where Shift+Home put it"
        );
    }
}

/// A registered extending Home/End chord may resolve the end the cursor
/// already holds: the pinned `end` handler calls `getLastKey` again and the
/// repeated `extendSelection` still reports through Select's
/// `allowDuplicateSelectionEvents`. Windows and Linux register Control+Shift
/// as the extending chord; macOS registers no extending Home/End chord at
/// all -- the cfg-free helper truth table in `select.rs` proves that map --
/// so a macOS host only proves the Control-bearing chord inert there.
#[gpui::test]
fn select_registered_shift_home_end_reports_when_the_end_is_already_held(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new(
            "sel-shift-end-held",
            keyed(&["Alpha", "Beta", "Gamma", "Delta"]),
        )
        .selection_mode(SelectionMode::Multiple)
        .default_open(true)
        .on_selection_change_all(move |keys, _, _| {
            picks.borrow_mut().push(
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .into_any_element()
    });

    press(cx, "tab down down down down enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Delta"],
        "the walk must end with the cursor holding the last option"
    );

    press(cx, "ctrl-shift-end");
    if cfg!(target_os = "macos") {
        assert_eq!(
            picked.borrow().as_slice(),
            ["Delta"],
            "macOS registers no extending Home/End chord: the Control-bearing \
             chord is entirely inert and must not report"
        );
        press(cx, "up enter");
        assert_eq!(
            picked.borrow().as_slice(),
            ["Delta", "Gamma,Delta"],
            "the inert chord must have left the cursor holding the end: Up \
             stepped to Gamma and Enter toggled it beside Delta"
        );
    } else {
        assert_eq!(
            picked.borrow().as_slice(),
            ["Delta", "Delta"],
            "the pinned End handler resolves the end the cursor already \
             holds, so the registered extending chord must report the \
             unchanged set"
        );
        press(cx, "ctrl-shift-end");
        assert_eq!(
            picked.borrow().as_slice(),
            ["Delta", "Delta", "Delta"],
            "the repeated same-key extension must report again"
        );
    }
}

/// Pinned `useSelectableCollection` answers `Mod+A` with `selectAll` in
/// multiple mode, but pinned SelectState drops the symbolic `all`: the
/// uncontrolled set becomes every *enabled* key while `onSelectionChange`
/// stays silent, a repeat over a complete selection is not a toggle, and a
/// controlled owner's state is not touched at all. The click that follows
/// proves what the silent select-all had done: toggling row 0 off leaves the
/// rest of the enabled keys selected.
#[gpui::test]
fn select_mod_a_selects_every_enabled_key_without_a_toggle_or_callback(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new(
            "sel-select-all",
            keyed(&["Alpha", "Held", "Gamma", "Delta"]),
        )
        .selection_mode(SelectionMode::Multiple)
        .disabled_keys(["Held".into()])
        .default_open(true)
        .on_selection_change_all(move |keys, _, _| {
            picks.borrow_mut().push(
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .into_any_element()
    });

    press(cx, "tab");
    press_mod_a(cx);
    assert!(
        picked.borrow().is_empty(),
        "the symbolic `all` must not report through onSelectionChange"
    );

    press_mod_a(cx);
    assert!(
        picked.borrow().is_empty(),
        "a repeat Mod+A over a complete selection must not be a toggle"
    );

    click(cx, 60., 66.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Gamma,Delta"],
        "toggling the first row must expose the enabled-only set the silent \
         select-all installed"
    );
}

/// A controlled Select's owner state is not the select's to mutate: Mod+A
/// reports nothing and leaves the owner-supplied set standing, so the next
/// toggle still starts from it.
#[gpui::test]
fn select_mod_a_leaves_a_controlled_selection_to_its_owner(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new(
            "sel-select-all-controlled",
            keyed(&["Alpha", "Beta", "Gamma"]),
        )
        .selection_mode(SelectionMode::Multiple)
        .selected_keys(["Alpha".into()])
        .default_open(true)
        .on_selection_change_all(move |keys, _, _| {
            picks.borrow_mut().push(
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .into_any_element()
    });

    press(cx, "tab");
    press_mod_a(cx);
    assert!(picked.borrow().is_empty());

    click(cx, 60., 66.);
    assert_eq!(
        picked.borrow().as_slice(),
        [""],
        "the toggle must have started from the owner-supplied set, proving \
         Mod+A never mutated it"
    );
}

/// Pinned `useSelect` returns `menuProps` with `disallowEmptySelection: true`,
/// so the generic collection's Escape-clear is unreachable for Select: Escape
/// closes the panel on the first press and the selection survives it. A
/// closed-proof click where row 0 was records nothing once the panel is gone.
#[gpui::test]
fn select_escape_closes_on_the_first_press_and_keeps_the_selection(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();
    let opens = events();
    let opened = opens.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let opens = opens.clone();
        Select::new("sel-escape-keeps", keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .default_selected_keys(["Alpha".into(), "Gamma".into()])
            .default_open(true)
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .on_selection_change_all(move |keys, _, _| {
                picks.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "escape");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:false"],
        "the first Escape must close the panel"
    );
    assert!(
        picked.borrow().is_empty(),
        "the closing Escape must not touch the selection or report one"
    );

    click(cx, 60., 66.);
    assert!(
        picked.borrow().is_empty(),
        "the closed panel's old row must answer nothing"
    );

    click(cx, 60., 18.);
    flush_frame(cx);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:false", "open:true"],
        "the trigger must reopen the panel the Escape closed"
    );

    click(cx, 60., 66.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Gamma"],
        "toggling Alpha off must leave Gamma selected, proving the Escape \
         preserved the pre-close selection"
    );
}

/// The Shift-range anchor is Select-owned state beside the cursor, so it
/// survives closing and reopening the popover — and it survives a deselect
/// that only ends a raw `all`, the way pinned `useMultipleSelectionState`
/// keeps the anchor. The reopened Shift+Up must extend from the pre-close
/// anchor, not seat a fresh one on the moved-to key.
#[gpui::test]
fn select_multiple_anchor_persists_across_close_and_reopen(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();
    let opens = events();
    let opened = opens.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let opens = opens.clone();
        Select::new(
            "sel-anchor-persists",
            keyed(&["Alpha", "Beta", "Gamma", "Delta"]),
        )
        .selection_mode(SelectionMode::Multiple)
        .default_open(true)
        .on_open_change(move |open, _, _| {
            opens.borrow_mut().push(format!("open:{open}"));
        })
        .on_selection_change_all(move |keys, _, _| {
            picks.borrow_mut().push(
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .into_any_element()
    });

    // Seat the anchor on Gamma, then deselect it: the anchor stays behind
    // with an empty selection, exactly where closing finds it.
    press(cx, "tab down enter down down enter enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha", "Alpha,Gamma", "Alpha"],
        "the probe must end with the anchor seated on deselected Gamma"
    );

    click(cx, 600., 300.);
    click(cx, 60., 18.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:false", "open:true"],
        "the probe must close and reopen the panel around the anchor"
    );

    // The pointer reopen seats the cursor on the first selected key, Alpha,
    // while the Shift anchor stays parked on the deselected Gamma. Shift+Down
    // moves the cursor to Beta and extends the range from that pre-close
    // anchor, so Gamma rejoins the set.
    press(cx, "shift-down");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha", "Alpha,Gamma", "Alpha", "Alpha,Beta,Gamma"],
        "the reopened extension must reach from the pre-close anchor at \
         Gamma; a fresh anchor on the moved-to Beta would leave the \
         surviving Alpha out and report only Alpha,Beta"
    );
}

/// A multiple Select answers no typeahead on its closed trigger -- the closed
/// pick would report through the single-key callback a set-valued selection
/// has no use for -- but the open RAC ListBox keeps its type-select: a letter
/// moves the cursor to the exact match without selecting, and Enter then
/// toggles that row.
#[gpui::test]
fn select_multiple_typeahead_is_inert_on_the_closed_trigger_alone(cx: &mut TestAppContext) {
    let single = events();
    let singled = single.clone();
    let all = events();
    let reported = all.clone();
    let opens = events();
    let opened = opens.clone();

    let cx = open_host(cx, move || {
        let single = single.clone();
        let all = all.clone();
        let opens = opens.clone();
        Select::new("sel-multi-typeahead", keyed(&["Alpha", "Beta", "Gamma"]))
            .selection_mode(SelectionMode::Multiple)
            .on_selection_change(move |key, _, _| {
                single.borrow_mut().push(format!("{key:?}"));
            })
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .on_selection_change_all(move |keys, _, _| {
                all.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    press(cx, "tab g");
    assert!(
        singled.borrow().is_empty(),
        "the closed trigger's typeahead must not report through the \
         single-key callback"
    );
    assert!(
        reported.borrow().is_empty(),
        "the closed trigger's typeahead must not report through the plural \
         callback"
    );
    assert!(
        opened.borrow().is_empty(),
        "the closed trigger's typeahead must not open the popover either"
    );

    press(cx, "down g enter");
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);
    assert_eq!(
        reported.borrow().as_slice(),
        ["Gamma"],
        "the open list's typeahead must move the cursor to the exact match \
         (Gamma), and Enter must toggle it"
    );
    assert!(
        singled.borrow().is_empty(),
        "an open multiple Select must never report through the single-key \
         callback"
    );
}

/// Pinned React Aria 3.51.0's arrow delegates return null at an enabled
/// boundary, so a held Shift+Arrow there runs no `extendSelection` and must
/// report nothing -- unlike a registered Shift+Home/End, whose handlers
/// resolve their end key again and still report.
#[gpui::test]
fn select_held_shift_arrow_stays_silent_at_the_boundary(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new(
            "sel-shift-boundary",
            keyed(&["Alpha", "Beta", "Gamma", "Delta"]),
        )
        .selection_mode(SelectionMode::Multiple)
        .default_open(true)
        .on_selection_change_all(move |keys, _, _| {
            picks.borrow_mut().push(
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .into_any_element()
    });

    press(cx, "tab down enter");
    assert_eq!(picked.borrow().as_slice(), ["Alpha"]);

    press(cx, "shift-up");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha"],
        "the held Shift+Up at the top boundary must run no extension and \
         report nothing beyond the Enter pick"
    );

    press(cx, "shift-down shift-down shift-down");
    assert_eq!(
        picked.borrow().as_slice(),
        [
            "Alpha",
            "Alpha,Beta",
            "Alpha,Beta,Gamma",
            "Alpha,Beta,Gamma,Delta"
        ],
        "the walk to the last option must report every extension"
    );

    press(cx, "shift-down");
    assert_eq!(
        picked.borrow().as_slice(),
        [
            "Alpha",
            "Alpha,Beta",
            "Alpha,Beta,Gamma",
            "Alpha,Beta,Gamma,Delta"
        ],
        "the held Shift+Down at the bottom boundary must run no extension \
         and report nothing"
    );

    press(cx, "shift-up");
    assert_eq!(
        picked.borrow().as_slice(),
        [
            "Alpha",
            "Alpha,Beta",
            "Alpha,Beta,Gamma",
            "Alpha,Beta,Gamma,Delta",
            "Alpha,Beta,Gamma"
        ],
        "a Shift+Arrow off the boundary must still shrink the anchored \
         range, so the silence above is the boundary admission and not a \
         dead handler"
    );
}

/// Pinned `useSelectableItem` seats the cursor on pointer press, so a
/// Shift+Arrow that follows a click on a later row extends the anchored
/// adjacent range instead of resolving from a null or stale cursor.
#[gpui::test]
fn select_pointer_press_seats_the_cursor_for_shift_navigation(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new(
            "sel-pointer-cursor",
            keyed(&["Alpha", "Beta", "Gamma", "Delta", "Echo"]),
        )
        .selection_mode(SelectionMode::Multiple)
        .default_open(true)
        .on_selection_change_all(move |keys, _, _| {
            picks.borrow_mut().push(
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .into_any_element()
    });

    // Row *i* centres at y = 66 + 36i inside the popover: this is Delta.
    click(cx, 60., 174.);
    assert_eq!(
        picked.borrow().as_slice(),
        ["Delta"],
        "the click must add Delta and seat the anchor on it"
    );

    press(cx, "shift-down");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Delta", "Delta,Echo"],
        "the following Shift+Down must extend the adjacent anchored range \
         Delta..Echo from the seated cursor; a focus-theft or a null-cursor \
         resolve would have reported nothing or the whole list"
    );
}

/// From a null cursor a registered Shift+Home/End is wholly inert before
/// cursor seating -- no cursor move, no selection, no callback -- so the
/// later navigation probe starts at the top the null-cursor way.
#[gpui::test]
fn select_shift_home_end_from_a_null_cursor_stay_inert(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new(
            "sel-shift-ends-null-cursor",
            keyed(&["Alpha", "Beta", "Gamma", "Delta"]),
        )
        .selection_mode(SelectionMode::Multiple)
        .default_open(true)
        .on_selection_change_all(move |keys, _, _| {
            picks.borrow_mut().push(
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "shift-home");
    press(cx, "shift-end");
    assert!(
        picked.borrow().is_empty(),
        "the registered Shift+Home and Shift+End must be wholly inert while \
         the cursor is null"
    );

    press(cx, "down enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Alpha"],
        "the probe must land on Alpha: the inert chords never seated a \
         cursor, so Down started from the top; a seated Home/End would have \
         left the cursor on an end and reported Beta or Delta here"
    );
}

/// The Home/End registration veto is mode-independent, so a single-mode
/// Select leaves its host's unregistered chord entirely inert too: the
/// cursor does not move, and Enter selects the row the arrows reached. The
/// cfg-free unit truth tables in `select.rs` prove both platform maps.
#[gpui::test]
fn select_single_unregistered_home_end_chords_are_inert(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new(
            "sel-single-unregistered",
            keyed(&["Alpha", "Beta", "Gamma", "Delta"]),
        )
        .default_open(true)
        .on_selection_change(move |key, _, _| {
            picks.borrow_mut().push(format!("{key:?}"));
        })
        .into_any_element()
    });

    press(cx, "tab down");
    // Control-bearing is unregistered on macOS; Alt-bearing is unregistered
    // on Windows and Linux.
    if cfg!(target_os = "macos") {
        press(cx, "ctrl-shift-end");
    } else {
        press(cx, "alt-shift-end");
    }
    assert!(
        picked.borrow().is_empty(),
        "the host's unregistered chord must not select anything"
    );

    press(cx, "enter");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Some(\"Alpha\")"],
        "Enter must select Alpha, proving the unregistered chord never \
         moved the cursor off the arrows' row"
    );
}

/// After Mod+A's symbolic `all` -- which itself stays callback-silent -- a
/// Shift navigation collapses the selection to the target the way pinned
/// `extendSelection` replaces a raw `all`, and that collapse reports.
#[gpui::test]
fn select_mod_a_then_shift_navigation_collapses_to_the_target(cx: &mut TestAppContext) {
    let picks = events();
    let picked = picks.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        Select::new(
            "sel-select-all-collapse",
            keyed(&["Alpha", "Beta", "Gamma"]),
        )
        .selection_mode(SelectionMode::Multiple)
        .default_open(true)
        .on_selection_change_all(move |keys, _, _| {
            picks.borrow_mut().push(
                keys.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        })
        .into_any_element()
    });

    press(cx, "tab");
    press_mod_a(cx);
    assert!(
        picked.borrow().is_empty(),
        "the symbolic `all` must not report through onSelectionChange"
    );

    press(cx, "down shift-down");
    assert_eq!(
        picked.borrow().as_slice(),
        ["Beta"],
        "the Shift+Down after the select-all must collapse the selection to \
         its target (Beta) and report the collapse"
    );
}

// ---------------------------------------------------------------------------
// Disclosure / DisclosureGroup
// ---------------------------------------------------------------------------

/// A single `Disclosure` reports the expansion its trigger moves to, and a
/// `DisclosureGroup` reports which item did the moving — with the reflow that
/// follows (the expanded item's body pushes the next one down) proving the
/// expansion is real, not just a callback.
#[gpui::test]
fn disclosure_toggles_and_group_reports(cx: &mut TestAppContext) {
    let toggles = events();
    let toggled = toggles.clone();
    let group_keys = events();
    let reported = group_keys.clone();
    let single_open = Rc::new(RefCell::new(false));
    let expanded = Rc::new(RefCell::new(HashSet::<SharedString>::new()));

    let cx = open_host(cx, move || {
        let toggles = toggles.clone();
        let group_keys = group_keys.clone();
        let single_open = single_open.clone();
        let expanded = expanded.clone();
        let expanded_set = expanded.borrow().clone();
        // The single Disclosure and the group's first item are 100px apart so
        // neither expands into the other's trigger.
        let is_expanded = *single_open.borrow();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(100.))
            .child(
                Disclosure::new("calendars-single-disclosure", "General")
                    .is_expanded(is_expanded)
                    .on_expanded_change(move |next, window, _| {
                        *single_open.borrow_mut() = *next;
                        toggles.borrow_mut().push(next.to_string());
                        window.refresh();
                    })
                    .child(gpui::div().h(px(20.))),
            )
            .child(
                DisclosureGroup::new("calendars-disclosure-group")
                    .item("grp-a", "Alpha", gpui::div().h(px(20.)))
                    .item("grp-b", "Beta", gpui::div().h(px(20.)))
                    .expanded_keys(expanded_set)
                    .on_expanded_change(move |keys, window, _| {
                        *expanded.borrow_mut() = keys.clone();
                        let key = keys
                            .iter()
                            .next()
                            .map(ToString::to_string)
                            .unwrap_or_default();
                        group_keys.borrow_mut().push(key);
                        window.refresh();
                    }),
            )
            .into_any_element()
    });

    // The single disclosure's trigger is a 36px Button at the origin.
    click(cx, 60., 18.);
    assert_eq!(
        toggled.borrow().as_slice(),
        ["true"],
        "the disclosure must report expanding"
    );
    click(cx, 60., 18.);
    assert_eq!(
        toggled.borrow().as_slice(),
        ["true", "false"],
        "the disclosure must report collapsing on the next press"
    );

    // The group starts at y 136: item A's trigger centres at (60, 154) and
    // item B's at (60, 190) while both are closed.
    click(cx, 60., 190.);
    assert_eq!(reported.borrow().as_slice(), ["grp-b"]);
    click(cx, 60., 154.);
    assert_eq!(reported.borrow().as_slice(), ["grp-b", "grp-a"]);
    settle_collapsible(cx);

    // With A expanded its body (p-2 + a 20px child = 36px) pushes B down to
    // y 208..244; a press at the old spot records nothing and the new one
    // reaches B, which is the expansion made observable.
    click(cx, 60., 190.);
    assert_eq!(
        reported.borrow().as_slice(),
        ["grp-b", "grp-a"],
        "an expanding item must push its sibling below its old spot"
    );
    click(cx, 60., 226.);
    assert_eq!(
        reported.borrow().as_slice(),
        ["grp-b", "grp-a", "grp-b"],
        "the moved item must still answer where the layout puts it"
    );
}

// ---------------------------------------------------------------------------
// Toolbar
// ---------------------------------------------------------------------------

/// A Toolbar's children keep their own press handlers, and the arrows answer
/// the way v3's one-line description advertises ("A container for interactive
/// controls with arrow key navigation") — which, per the inheritance line on
/// the v3 page, is React Aria's pinned `useToolbar` (react-aria 3.51.0): the
/// arrows move *inside* the toolbar through a FocusManager built on the
/// toolbar's own element, whose `focusNext`/`focusPrevious` walk the subtree
/// with `wrap` unset — so an arrow at either end moves nothing *and is still
/// consumed* (`stopPropagation` + `preventDefault` run whether or not the
/// walker found a node). Tab is what leaves it, and one press leaves the
/// *entire* toolbar: the pinned handler runs `focusFirst`/`focusLast` and
/// lets the native Tab carry on from that end.
///
/// Everything is asserted through the keyboard: Tab enters on the first
/// control, the third Right holds on the last control instead of wrapping
/// (the old window-wide wrap landed Enter on the first control again), the
/// third Left holds on the first, and Tab from that *first* child still
/// reaches the sibling after the toolbar in one press.
#[gpui::test]
fn toolbar_arrows_stop_at_ends_and_tab_leaves(cx: &mut TestAppContext) {
    let pressed = events();
    let recorded = pressed.clone();

    let cx = open_host(cx, move || {
        let bold_pressed = pressed.clone();
        let italic_pressed = pressed.clone();
        let underline_pressed = pressed.clone();
        let outside_pressed = pressed.clone();
        // A plain button after the toolbar is the probe a window-wide arrow
        // would land on: nothing sits between the toolbar's last control and
        // it, so the old `focus_next` walked straight out to it.
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(100.))
            .child(
                Toolbar::new()
                    .gap(px(8.))
                    .child(
                        Button::new("tb-bold")
                            .label("Bold")
                            .on_press(move |_, _, _| bold_pressed.borrow_mut().push("bold".into())),
                    )
                    .child(
                        Button::new("tb-italic")
                            .label("Italic")
                            .on_press(move |_, _, _| {
                                italic_pressed.borrow_mut().push("italic".into());
                            }),
                    )
                    .child(Button::new("tb-underline").label("Underline").on_press(
                        move |_, _, _| {
                            underline_pressed.borrow_mut().push("underline".into());
                        },
                    )),
            )
            .child(
                Button::new("tb-outside")
                    .label("Outside")
                    .on_press(move |_, _, _| {
                        outside_pressed.borrow_mut().push("outside".into());
                    }),
            )
            .into_any_element()
    });

    // Tab enters the toolbar on the first control; two Rights walk to the
    // third, and the third Right stays there — pinned `useToolbar` consumes
    // the key at the end without moving. Enter reports which control holds
    // the focus.
    press(cx, "tab");
    press(cx, "right right right");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["underline"],
        "Right from the last control must stop there and be consumed, not \
         wrap to the first"
    );

    // Left from the first control stops there, again consumed.
    press(cx, "left left left");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["underline", "bold"],
        "Left from the first control must stop there and be consumed, not \
         wrap to the last control"
    );

    // Tab is the way out, and one press leaves the entire toolbar: this
    // exits from the *first* control, the far end from the sibling, and the
    // sibling's press must answer immediately.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["underline", "bold", "outside"],
        "Tab must leave the whole toolbar for the next control in the \
         window in one press, from any child"
    );
}

/// A disabled toolbar child is not a tab stop, so the arrows skip it in both
/// directions: Right from the first control lands on the third, and Left from
/// the third lands back on the first. The enabled ends are consumed stops —
/// Left from the first and Right from the last move nothing — and the
/// disabled child is never the destination.
#[gpui::test]
fn toolbar_arrows_skip_a_disabled_child(cx: &mut TestAppContext) {
    let pressed = events();
    let recorded = pressed.clone();

    let cx = open_host(cx, move || {
        let bold_pressed = pressed.clone();
        let underline_pressed = pressed.clone();
        Toolbar::new()
            .gap(px(8.))
            .child(
                Button::new("tb-bold")
                    .label("Bold")
                    .on_press(move |_, _, _| bold_pressed.borrow_mut().push("bold".into())),
            )
            .child(Button::new("tb-italic").label("Italic").is_disabled(true))
            .child(
                Button::new("tb-underline")
                    .label("Underline")
                    .on_press(move |_, _, _| {
                        underline_pressed.borrow_mut().push("underline".into());
                    }),
            )
            .into_any_element()
    });

    // Forward: Right skips the disabled control and lands on the third.
    press(cx, "tab");
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["underline"],
        "Right must skip a disabled child"
    );

    // Backward: Left skips it on the way to the first.
    press(cx, "left");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["underline", "bold"],
        "Left must skip a disabled child"
    );

    // The ends stop on the enabled controls: Left from the first is consumed
    // there, Right from the last is consumed there, and the disabled child is
    // never the destination.
    press(cx, "left");
    press(cx, "enter");
    press(cx, "right");
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["underline", "bold", "bold", "underline"],
        "the arrows must stop and be consumed at the enabled ends, over the \
         enabled controls"
    );
}

/// Pinned `useToolbar` records the last focused child when the focus leaves
/// the toolbar (the `lastFocused` ref, set by the Tab branch and by the
/// blur-capture) and restores it the next time the focus enters the toolbar
/// from outside (the focus-capture). So re-entry lands on the child the user
/// left, not on whichever end the entry walks onto first.
///
/// A button before and after the toolbar make both exits observable through
/// Enter: forward Tab from the middle child leaves in one press, Shift+Tab
/// leaves backward in one press, and Tab back in restores the middle child.
#[gpui::test]
fn toolbar_restores_last_child_on_re_entry(cx: &mut TestAppContext) {
    let pressed = events();
    let recorded = pressed.clone();

    let cx = open_host(cx, move || {
        let before_pressed = pressed.clone();
        let bold_pressed = pressed.clone();
        let italic_pressed = pressed.clone();
        let underline_pressed = pressed.clone();
        let after_pressed = pressed.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(100.))
            .child(
                Button::new("tb-before")
                    .label("Before")
                    .on_press(move |_, _, _| {
                        before_pressed.borrow_mut().push("before".into());
                    }),
            )
            .child(
                Toolbar::new()
                    .gap(px(8.))
                    .child(
                        Button::new("tb-re-bold")
                            .label("Bold")
                            .on_press(move |_, _, _| bold_pressed.borrow_mut().push("bold".into())),
                    )
                    .child(
                        Button::new("tb-re-italic")
                            .label("Italic")
                            .on_press(move |_, _, _| {
                                italic_pressed.borrow_mut().push("italic".into());
                            }),
                    )
                    .child(Button::new("tb-re-underline").label("Underline").on_press(
                        move |_, _, _| {
                            underline_pressed.borrow_mut().push("underline".into());
                        },
                    )),
            )
            .child(
                Button::new("tb-after")
                    .label("After")
                    .on_press(move |_, _, _| {
                        after_pressed.borrow_mut().push("after".into());
                    }),
            )
            .into_any_element()
    });

    // Tab onto the leading button, then into the toolbar, then Right to the
    // middle child.
    press(cx, "tab tab right");

    // Forward Tab from the middle child leaves the whole toolbar in one
    // press, for the sibling after it.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["after"],
        "forward Tab from a middle child must exit the toolbar in one press"
    );

    // Backward Shift+Tab — the focus sits on After, outside — walks to the
    // previous stop, which is inside the toolbar. That entry is exactly the
    // restore case: pinned `useToolbar`'s focus-capture sends the focus on to
    // the recorded child instead of leaving it on the end the walk hit.
    press(cx, "shift-tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["after", "italic"],
        "Shift+Tab into the toolbar must restore the last focused child"
    );
    // The restore left the focus on Italic; Shift+Tab leaves backwards in one
    // press, for the leading button.
    press(cx, "shift-tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["after", "italic", "before"],
        "Shift+Tab must leave the whole toolbar backwards in one press"
    );

    // And one more entry restores Italic again — the record follows the last
    // departure, which was from Italic.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["after", "italic", "before", "italic"],
        "Tab re-entry must restore the last focused child, not the first stop"
    );
}

/// Pinned `useToolbar`'s focus-capture says of a recorded child that is gone:
/// "If the element was removed, do nothing, either the first item in the
/// first group, or the last item in the last group will be focused, depending
/// on direction." So a rebuild that dropped the recorded child must not hand
/// the focus to its stale handle on re-entry — a handle with no rendered
/// element backs no focus — and the entry keeps its own landing.
#[gpui::test]
fn toolbar_removed_child_is_not_restored_on_re_entry(cx: &mut TestAppContext) {
    let pressed = events();
    let recorded = pressed.clone();
    let with_italic = Rc::new(Cell::new(true));
    let keep = with_italic.clone();

    let cx = open_host(cx, move || {
        let before_pressed = pressed.clone();
        let bold_pressed = pressed.clone();
        let italic_pressed = pressed.clone();
        let underline_pressed = pressed.clone();
        let after_pressed = pressed.clone();
        let mut bar = Toolbar::new().id("tb-removal").gap(px(8.)).child(
            Button::new("tb-rm-bold")
                .label("Bold")
                .on_press(move |_, _, _| bold_pressed.borrow_mut().push("bold".into())),
        );
        if keep.get() {
            bar = bar.child(Button::new("tb-rm-italic").label("Italic").on_press(
                move |_, _, _| {
                    italic_pressed.borrow_mut().push("italic".into());
                },
            ));
        }
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(100.))
            .child(
                Button::new("tb-rm-before")
                    .label("Before")
                    .on_press(move |_, _, _| before_pressed.borrow_mut().push("before".into())),
            )
            .child(
                bar.child(Button::new("tb-rm-underline").label("Underline").on_press(
                    move |_, _, _| {
                        underline_pressed.borrow_mut().push("underline".into());
                    },
                )),
            )
            .child(
                Button::new("tb-rm-after")
                    .label("After")
                    .on_press(move |_, _, _| {
                        after_pressed.borrow_mut().push("after".into());
                    }),
            )
            .into_any_element()
    });

    // Tab onto the leading button, into the toolbar, and Right on to Italic.
    press(cx, "tab tab right");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["italic"],
        "the middle child must hold the focus before the rebuild"
    );

    // Leave: forward Tab exits to After in one press and the exit frame
    // records Italic as the child the focus left from.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["italic", "after"],
        "forward Tab must exit the toolbar in one press"
    );

    // Rebuild without Italic, painting the removal before re-entering so the
    // rendered frame the restore consults no longer contains it.
    with_italic.set(false);
    flush_frame(cx);
    flush_frame(cx);

    // Re-entry from After: Shift+Tab walks back on to Underline, and the
    // restore must skip the removed child's dead handle — the landing stays.
    press(cx, "shift-tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["italic", "after", "underline"],
        "re-entry after a child was removed must keep the walk's landing, \
         not restore the removed child's handle"
    );

    // The rebuilt toolbar still navigates: Right from Underline is a consumed
    // end stop, and Enter still reports the same child afterwards.
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["italic", "after", "underline", "underline"],
        "the rebuilt toolbar's ends must still be consumed stops"
    );
}

/// Held-key auto-repeat (`KeyDownEvent::is_held`) walks and stops exactly like
/// distinct presses. This is also the behavioural half of the no-wrap probe's
/// side-effect review: gpui dispatches focus listeners only at frame end,
/// comparing the previous and current focus paths — which the probe's
/// temporary blur/refocus never changes when an end refuses the move — and
/// the refocus's `clear_pending_keystrokes` has nothing to clear in a window
/// with no multi-stroke bindings. Six repeats in a row past each end must
/// leave the focus exactly where a single press left it.
#[gpui::test]
fn toolbar_held_arrow_repeats_stop_at_ends(cx: &mut TestAppContext) {
    let pressed = events();
    let recorded = pressed.clone();
    let outside_pressed = events();
    let outside = outside_pressed.clone();

    let cx =
        open_host(cx, move || {
            let bold_pressed = pressed.clone();
            let italic_pressed = pressed.clone();
            let underline_pressed = pressed.clone();
            let outside = outside_pressed.clone();
            gpui::div()
                .flex()
                .flex_col()
                .gap(px(100.))
                .child(
                    Toolbar::new()
                        .id("tb-repeat")
                        .gap(px(8.))
                        .child(
                            Button::new("tb-hr-bold")
                                .label("Bold")
                                .on_press(move |_, _, _| {
                                    bold_pressed.borrow_mut().push("bold".into());
                                }),
                        )
                        .child(Button::new("tb-hr-italic").label("Italic").on_press(
                            move |_, _, _| italic_pressed.borrow_mut().push("italic".into()),
                        ))
                        .child(Button::new("tb-hr-underline").label("Underline").on_press(
                            move |_, _, _| {
                                underline_pressed.borrow_mut().push("underline".into());
                            },
                        )),
                )
                .child(
                    Button::new("tb-hr-outside")
                        .label("Outside")
                        .on_press(move |_, _, _| outside.borrow_mut().push("outside".into())),
                )
                .into_any_element()
        });

    let held = |cx: &mut VisualTestContext, key: &'static str| {
        for _ in 0..6 {
            cx.simulate_event(KeyDownEvent {
                keystroke: Keystroke::parse(key).unwrap(),
                is_held: true,
                prefer_character_input: false,
            });
        }
    };

    press(cx, "tab");
    // Six held Rights from the first control: three would reach the end, so
    // six prove the repeats are refused there instead of wrapping.
    held(cx, "right");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["underline"],
        "held Right repeats must stop on the last control, not wrap"
    );

    held(cx, "left");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["underline", "bold"],
        "held Left repeats must stop on the first control, not wrap"
    );

    // The focus survived every refused repeat: the toolbar is still what
    // holds it, and one Tab still leaves for the sibling.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        outside.borrow().as_slice(),
        ["outside"],
        "Tab must still leave the toolbar in one press after held repeats"
    );
}

// ---------------------------------------------------------------------------
// `value` — the pure builder seed on the calendar family
// ---------------------------------------------------------------------------

#[gpui::test]
fn calendar_value_builder_seeds_the_bound_state_at_first_render(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    // The builder chain needs no `&mut App`: the selection travels on the
    // struct and lands in the state when the grid first renders.
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone())
            .value(Some(Date::new(2026, 1, 15)))
            .into_any_element()
    });
    let selected = cx.update(|_, cx| state.read(cx).selected);
    assert_eq!(
        selected,
        Some(Date::new(2026, 1, 15)),
        "value must seed the bound CalendarState when the grid first renders"
    );
}

#[gpui::test]
fn calendar_value_builder_keeps_the_last_call_and_outranks_default_value(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 2, 2))
            .value(Some(Date::new(2026, 3, 3)))
            .value(Some(Date::new(2026, 1, 15)))
            .into_any_element()
    });
    let selected = cx.update(|_, cx| state.read(cx).selected);
    assert_eq!(
        selected,
        Some(Date::new(2026, 1, 15)),
        "a pure builder keeps the last call, and the controlled spelling outranks defaultValue"
    );
}

#[gpui::test]
fn calendar_value_builder_seeds_once_and_never_clobbers_edits(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Calendar::new(state_for_view.clone())
            .value(Some(Date::new(2026, 1, 15)))
            .on_change(move |date, _, _| {
                changes
                    .borrow_mut()
                    .push(date.map_or_else(|| "none".to_owned(), |d| d.format_iso()));
            })
            .into_any_element()
    });
    assert_eq!(
        cx.update(|_, cx| state.read(cx).selected),
        Some(Date::new(2026, 1, 15))
    );

    // The builder runs again on every refresh carrying the same `value`; the
    // seed must not rewrite the selection the user drove. The ring starts on
    // the seeded selection, so Right then Enter picks the next day.
    press(cx, "tab");
    press(cx, "right");
    press(cx, "enter");
    let picked = cx.update(|_, cx| state.read(cx).selected);
    assert_eq!(picked, Some(Date::new(2026, 1, 16)));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2026-01-16"],
        "the pick reports exactly once, from the keystroke"
    );
    cx.update(|window, _| window.refresh());
    assert_eq!(
        cx.update(|_, cx| state.read(cx).selected),
        Some(Date::new(2026, 1, 16)),
        "a re-render must not re-apply the value seed over the user's pick"
    );
}

#[gpui::test]
fn range_calendar_value_builder_seeds_the_bound_state_at_first_render(cx: &mut TestAppContext) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        RangeCalendar::new(state_for_view.clone())
            .value(Some(Date::new(2026, 1, 10)), Some(Date::new(2026, 1, 20)))
            .into_any_element()
    });
    let (start, end) = cx.update(|_, cx| {
        let st = state.read(cx);
        (st.start, st.end)
    });
    assert_eq!(
        (start, end),
        (Some(Date::new(2026, 1, 10)), Some(Date::new(2026, 1, 20))),
        "value must seed the bound DateRangeState when the grid first renders"
    );
}

#[gpui::test]
fn range_calendar_value_builder_keeps_the_last_call_and_outranks_default_value(
    cx: &mut TestAppContext,
) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        RangeCalendar::new(state_for_view.clone())
            .default_value((Date::new(2026, 2, 2), Date::new(2026, 2, 8)))
            .value(Some(Date::new(2026, 3, 1)), Some(Date::new(2026, 3, 5)))
            .value(Some(Date::new(2026, 1, 10)), Some(Date::new(2026, 1, 20)))
            .into_any_element()
    });
    let (start, end) = cx.update(|_, cx| {
        let st = state.read(cx);
        (st.start, st.end)
    });
    assert_eq!(
        (start, end),
        (Some(Date::new(2026, 1, 10)), Some(Date::new(2026, 1, 20))),
        "a pure builder keeps the last call, and the controlled spelling outranks defaultValue"
    );
}

#[gpui::test]
fn range_calendar_value_builder_seeds_once_and_never_clobbers_edits(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        RangeCalendar::new(state_for_view.clone())
            .value(Some(Date::new(2026, 1, 10)), Some(Date::new(2026, 1, 20)))
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });
    let (start, end) = cx.update(|_, cx| {
        let st = state.read(cx);
        (st.start, st.end)
    });
    assert_eq!(
        (start, end),
        (Some(Date::new(2026, 1, 10)), Some(Date::new(2026, 1, 20)))
    );

    // The builder runs again on every refresh carrying the same `value`; the
    // seed must not restore the range the user replaced. The keyboard cursor
    // starts on the seeded start, and Enter over a complete range re-anchors
    // it — an incomplete pick whose end is null, which v3 holds locally.
    press(cx, "tab");
    press(cx, "enter");
    let (start, end) = cx.update(|_, cx| {
        let st = state.read(cx);
        (st.start, st.end)
    });
    assert_eq!(
        (start, end),
        (Some(Date::new(2026, 1, 10)), None),
        "Enter must re-anchor the range instead of reporting the seed again"
    );
    assert!(
        recorded.borrow().is_empty(),
        "the incomplete re-anchor defers onChange, exactly as a first pick does"
    );
    cx.update(|window, _| window.refresh());
    assert_eq!(
        cx.update(|_, cx| state.read(cx).end),
        None,
        "a re-render must not re-apply the value seed over the user's pick"
    );
}
