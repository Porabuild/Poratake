//! Deeper v3 coverage for edge dates and constraints on the calendar family.
//!
//! `calendars_and_more.rs` already drives the calendar's arrows and week
//! keys, PageUp/PageDown month changes, Home/End, min/max blocking and the
//! RangeCalendar's anchor/extend selection. These two tests push the edges the
//! grid geometry hides: the unavailable-date predicate (v3
//! `isDateUnavailable`: "Marks dates as unavailable") on both input paths,
//! and month paging across the December→January year boundary that lands on a
//! leap February (2028), where the date math must resolve Jan 31 forward to
//! Feb 29 rather than invent a Feb 31.
//!
//! Geometry is the same derivation the rest of the suite uses: a bare
//! Calendar at the window origin, `CALENDAR_WIDTH` (252) split into seven equal cells for the column centres, the first cell row at y = 86 with
//! 36px per row after it, and the month's leading blanks
//! (`DateConstraints::lead_cells`, Monday-start default) for a day's
//! row/column.

use crate::harness;
use crate::source_scan;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{point, prelude::*, px, Modifiers, MouseButton, TestAppContext};
use harness::{click, events, open_host, press};
use herogpui_components::{
    calendar::{Date, CALENDAR_WIDTH},
    Button, Calendar, CalendarState, DateConstraints, DateRangeState, RangeCalendar,
    VisibleDuration, Weekday,
};

/// v3 dims an unavailable date with `status-disabled`'s opacity but keeps it
/// focusable. The dim is an element opacity on the day cell, so it scales
/// every quad the cell paints: a caller-drawn marker inside an unavailable
/// cell paints at the disabled opacity, one in an available cell at full
/// strength. Both calendars, and the keyboard still lands on the date.
#[gpui::test]
fn calendar_unavailable_cells_use_status_disabled_visuals_without_losing_keyboard_focus(
    cx: &mut TestAppContext,
) {
    fn marker(day: u32) -> gpui::AnyElement {
        let hue = match day {
            15 => Some(0.0),
            16 => Some(0.33),
            _ => None,
        };
        match hue {
            Some(h) => gpui::div()
                .size(px(6.))
                .bg(gpui::hsla(h, 1., 0.5, 1.))
                .into_any_element(),
            None => gpui::div().into_any_element(),
        }
    }
    fn alphas(scene: &harness::Painted) -> (f32, f32) {
        let alpha = |h: f32| {
            scene
                .solids()
                .into_iter()
                .find(|c| (c.h - h).abs() < 1e-3 && (c.s - 1.).abs() < 1e-3)
                .map_or_else(|| panic!("the marker with hue {h} must paint"), |c| c.a)
        };
        (alpha(0.0), alpha(0.33))
    }

    let focused = events();
    let seen = focused.clone();
    let state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 12, 8)));
    harness::still();
    let vcx = open_host(cx, move || {
        let seen = seen.clone();
        Calendar::new(state.clone())
            .is_date_unavailable(|date| date.day == 15)
            .cell(|cell| marker(cell.date.day))
            .on_focus_change(move |date, _, _| seen.borrow_mut().push(date.format_iso()))
            .into_any_element()
    });
    let scene = harness::painted(vcx);
    let dim = vcx.update(|_, cx| {
        use herogpui_theme::ActiveTheme;
        cx.layout().disabled_opacity
    });
    let (unavailable, available) = alphas(&scene);
    assert!(
        (unavailable - dim).abs() < 1e-3 && (available - 1.).abs() < 1e-3,
        "Calendar: the unavailable cell must paint at {dim}, got {unavailable} \
         (available {available})"
    );
    press(vcx, "tab");
    press(vcx, "down");
    assert_eq!(
        focused.borrow().last().map(String::as_str),
        Some("2025-12-15"),
        "an unavailable date must stay reachable by the keyboard"
    );

    let range = cx.new(|cx| DateRangeState::new(cx));
    harness::still();
    let vcx = open_host(cx, move || {
        RangeCalendar::new(range.clone())
            .is_date_unavailable(|date, _| date.day == 15)
            .cell(|cell| marker(cell.date.day))
            .into_any_element()
    });
    let scene = harness::painted(vcx);
    let (unavailable, available) = alphas(&scene);
    assert!(
        (unavailable - dim).abs() < 1e-3 && (available - 1.).abs() < 1e-3,
        "RangeCalendar: the unavailable cell must paint at {dim}, got \
         {unavailable} (available {available})"
    );
}

/// Remaining source-text check: the not-allowed cursor. The test platform
/// records the cursor it is asked for privately and offers no readback.
#[test]
fn calendar_unavailable_cells_ask_for_the_not_allowed_cursor() {
    for file in ["calendar.rs", "range_calendar.rs"] {
        let source = source_scan::component_src(file);
        assert!(
            source.contains("CursorStyle::OperationNotAllowed"),
            "{file}: unavailable calendar cells need the v3 not-allowed cursor"
        );
    }
}

#[gpui::test]
fn calendar_picking_keeps_the_selection_aligned_view(cx: &mut TestAppContext) {
    for keyboard in [false, true] {
        let state = cx.new(|cx| {
            let mut state = CalendarState::with_selected(cx, Date::new(2025, 12, 8));
            state.view_year = 2035;
            state.view_month = 7;
            state.view_day = 1;
            state
        });
        let id = state.entity_id().as_u64();
        let view_state = state.clone();
        let cx = open_host(cx, move || {
            Calendar::new(view_state.clone())
                .visible_duration(VisibleDuration::Months(2))
                .selection_alignment(herogpui_components::SelectionAlignment::End)
                .first_day_of_week(Weekday::Mon)
                .into_any_element()
        });
        if keyboard {
            press(cx, "tab");
            press(cx, "right");
            press(cx, "right");
            press(cx, "enter");
        } else {
            let key = cal_cell_selector(id, 2025, 12, 10);
            let bounds = cx.debug_bounds(Box::leak(key.into_boxed_str())).unwrap();
            click(
                cx,
                f32::from(bounds.center().x),
                f32::from(bounds.center().y),
            );
        }
        assert_eq!(
            cx.update(|_, cx| (state.read(cx).anchor(), state.read(cx).selected)),
            (Date::new(2025, 11, 8), Some(Date::new(2025, 12, 10))),
            "keyboard={keyboard}: picking must retain both displayed months"
        );
    }
}

#[gpui::test]
fn range_picking_keeps_the_selection_aligned_view(cx: &mut TestAppContext) {
    use herogpui_components::SelectionAlignment;
    for (duration, alignment, expected_anchor) in [
        (
            VisibleDuration::Months(1),
            SelectionAlignment::Start,
            Date::new(2025, 12, 8),
        ),
        (
            VisibleDuration::Months(2),
            SelectionAlignment::End,
            Date::new(2025, 11, 8),
        ),
        (
            VisibleDuration::Weeks(2),
            SelectionAlignment::Start,
            Date::new(2025, 12, 8),
        ),
        (
            VisibleDuration::Days(5),
            SelectionAlignment::Start,
            Date::new(2025, 12, 8),
        ),
    ] {
        for keyboard in [false, true] {
            let state = cx.new(|cx| {
                let mut state = DateRangeState::with_range(
                    cx,
                    Some(Date::new(2025, 12, 8)),
                    Some(Date::new(2025, 12, 14)),
                );
                state.view_year = 2035;
                state.view_month = 7;
                state.view_day = 1;
                state
            });
            let id = state.entity_id().as_u64();
            let view_state = state.clone();
            let picked = events();
            let view_picked = picked.clone();
            let cx = open_host(cx, move || {
                let picked = view_picked.clone();
                RangeCalendar::new(view_state.clone())
                    .visible_duration(duration)
                    .selection_alignment(alignment)
                    .first_day_of_week(Weekday::Mon)
                    .on_change(move |start, end, _, _| {
                        picked.borrow_mut().push(format!(
                            "{}..{}",
                            start.format_iso(),
                            end.format_iso()
                        ));
                    })
                    .into_any_element()
            });
            let key = |day| {
                if duration.is_month_view() {
                    range_cell_selector(id, 2025, 12, day)
                } else {
                    format!(
                        r#"NamedInteger("range-cal", {id})-{}"#,
                        Date::new(2025, 12, day).format_iso()
                    )
                }
            };
            if keyboard {
                press(cx, "tab");
                press(cx, "right");
                press(cx, "right");
                press(cx, "enter");
            } else {
                let bounds = cx
                    .debug_bounds(Box::leak(key(10).into_boxed_str()))
                    .unwrap();
                click(
                    cx,
                    f32::from(bounds.center().x),
                    f32::from(bounds.center().y),
                );
            }
            assert_eq!(
                cx.update(|_, cx| state.read(cx).anchor()),
                expected_anchor,
                "{duration:?}, keyboard={keyboard}: preserve the displayed range"
            );
            let bounds = cx
                .debug_bounds(Box::leak(key(12).into_boxed_str()))
                .unwrap();
            click(
                cx,
                f32::from(bounds.center().x),
                f32::from(bounds.center().y),
            );
            assert_eq!(picked.borrow().as_slice(), ["2025-12-10..2025-12-12"]);
        }
    }
}

#[gpui::test]
fn calendar_spacing_matches_the_pinned_rendered_grids(cx: &mut TestAppContext) {
    for range in [false, true] {
        for duration in [
            VisibleDuration::Months(1),
            VisibleDuration::Weeks(2),
            VisibleDuration::Days(10),
        ] {
            let state = cx.new(|cx| CalendarState::new(cx));
            let range_state = cx.new(|cx| DateRangeState::new(cx));
            let id = if range {
                range_state.entity_id()
            } else {
                state.entity_id()
            }
            .as_u64();
            let base = if range {
                format!(r#"NamedInteger("range-cal", {id})"#)
            } else {
                format!(r#"NamedInteger("cal", {id})"#)
            };
            let cx = open_host(cx, move || {
                let content = if range {
                    RangeCalendar::new(range_state.clone())
                        .default_value((Date::new(2026, 8, 3), Date::new(2026, 8, 4)))
                        .selection_alignment(herogpui_components::SelectionAlignment::Start)
                        .visible_duration(duration)
                        .first_day_of_week(Weekday::Mon)
                        .into_any_element()
                } else {
                    Calendar::new(state.clone())
                        .default_value(Date::new(2026, 8, 3))
                        .selection_alignment(herogpui_components::SelectionAlignment::Start)
                        .visible_duration(duration)
                        .first_day_of_week(Weekday::Mon)
                        .into_any_element()
                };
                gpui::div()
                    .debug_selector(|| "calendar-spacing-host".to_owned())
                    .child(content)
                    .into_any_element()
            });
            let weekday = cx
                .debug_bounds(Box::leak(format!("{base}-weekday-0").into_boxed_str()))
                .unwrap();
            assert_eq!(weekday.top(), px(40.), "range={range}, {duration:?}");
            assert_eq!(weekday.size.height, px(24.));
            let first_day = if duration.is_month_view() { 1 } else { 3 };
            let mut bounds = |day| {
                let key = if !duration.is_month_view() {
                    format!("{base}-{}", Date::new(2026, 8, day).format_iso())
                } else if range {
                    range_cell_selector(id, 2026, 8, day)
                } else {
                    cal_cell_selector(id, 2026, 8, day)
                };
                cx.debug_bounds(Box::leak(key.into_boxed_str())).unwrap()
            };
            let first = bounds(first_day);
            let second = bounds(first_day + 7);
            assert_eq!(
                first.top() - weekday.bottom(),
                px(if range { 6. } else { 4. })
            );
            assert_eq!(
                second.top() - first.top(),
                px(if range { 40. } else { 36. })
            );
            let height = cx
                .debug_bounds("calendar-spacing-host")
                .unwrap()
                .size
                .height;
            let rows = if duration.is_month_view() { 6. } else { 2. };
            assert_eq!(height, px(68. + rows * if range { 40. } else { 36. }));
        }
    }
}

#[gpui::test]
fn calendar_day_text_keeps_pinned_metrics_in_every_state(cx: &mut TestAppContext) {
    for range in [false, true] {
        for (disabled, read_only) in [(false, false), (true, false), (false, true)] {
            for inherited in [20., 48.] {
                let seen = Rc::new(RefCell::new(Vec::new()));
                let view_seen = seen.clone();
                let calendar = cx.new(|cx| CalendarState::new(cx));
                let range_calendar = cx.new(|cx| DateRangeState::new(cx));
                let cx = open_host(cx, move || {
                    let seen = view_seen.clone();
                    let probe = move |label: gpui::SharedString| {
                        let seen = seen.clone();
                        gpui::div()
                            .child(label)
                            .child(gpui::canvas(
                                |_, _, _| {},
                                move |_, _, window, _| {
                                    let style = window.text_style();
                                    let rem = window.rem_size();
                                    seen.borrow_mut().push((
                                        style.font_size.to_pixels(rem),
                                        style
                                            .line_height
                                            .to_pixels(gpui::AbsoluteLength::Pixels(rem), rem),
                                        style.font_weight,
                                    ));
                                },
                            ))
                            .into_any_element()
                    };
                    let content = if range {
                        RangeCalendar::new(range_calendar.clone())
                            .default_value((Date::new(2026, 8, 10), Date::new(2026, 8, 15)))
                            .is_disabled(disabled)
                            .is_read_only(read_only)
                            .cell(move |cell| probe(cell.formatted_date.clone()))
                            .into_any_element()
                    } else {
                        Calendar::new(calendar.clone())
                            .default_value(Date::new(2026, 8, 10))
                            .is_disabled(disabled)
                            .is_read_only(read_only)
                            .cell(move |cell| probe(cell.formatted_date.clone()))
                            .into_any_element()
                    };
                    gpui::div()
                        .line_height(px(inherited))
                        .child(content)
                        .into_any_element()
                });
                cx.update(|window, _| window.refresh());
                let seen = seen.borrow();
                assert!(!seen.is_empty());
                for metrics in seen.iter() {
                    assert_eq!(*metrics, (px(14.), px(20.), gpui::FontWeight::MEDIUM),
                        "range={range}, disabled={disabled}, read_only={read_only}, inherited={inherited}");
                }
            }
        }
    }
}

#[gpui::test]
fn calendar_headers_do_not_inherit_host_line_height(cx: &mut TestAppContext) {
    for range in [false, true] {
        for duration in [
            VisibleDuration::Months(1),
            VisibleDuration::Weeks(1),
            VisibleDuration::Days(3),
        ] {
            let mut heights = Vec::new();
            for leading in [20., 48.] {
                let calendar = cx.new(|cx| CalendarState::new(cx));
                let range_calendar = cx.new(|cx| DateRangeState::new(cx));
                let cx = open_host(cx, move || {
                    let content = if range {
                        RangeCalendar::new(range_calendar.clone())
                            .default_value((Date::new(2026, 8, 10), Date::new(2026, 8, 15)))
                            .visible_duration(duration)
                            .into_any_element()
                    } else {
                        Calendar::new(calendar.clone())
                            .default_value(Date::new(2026, 8, 10))
                            .visible_duration(duration)
                            .into_any_element()
                    };
                    gpui::div()
                        .debug_selector(|| "header-leading-host".to_owned())
                        .line_height(px(leading))
                        .child(content)
                        .into_any_element()
                });
                heights.push(cx.debug_bounds("header-leading-host").unwrap().size.height);
            }
            assert_eq!(
                heights[0], heights[1],
                "range={range}, duration={duration:?}"
            );
        }
    }
}

#[gpui::test]
fn calendar_days_align_with_the_seven_weekday_columns(cx: &mut TestAppContext) {
    for duration in [VisibleDuration::Months(1), VisibleDuration::Weeks(1)] {
        let state = cx.new(|cx| CalendarState::new(cx));
        let base = format!(r#"NamedInteger("cal", {})"#, state.entity_id().as_u64());
        let cx = open_host(cx, move || {
            Calendar::new(state.clone())
                .default_value(Date::new(2026, 8, 3))
                .first_day_of_week(Weekday::Mon)
                .visible_duration(duration)
                .into_any_element()
        });
        for column in 0..7 {
            let date = Date::new(2026, 8, 3 + column);
            let cell_key = if duration.is_month_view() {
                format!("{base}-2026-8-d{}", date.day)
            } else {
                format!("{base}-{}", date.format_iso())
            };
            let header = cx
                .debug_bounds(Box::leak(
                    format!("{base}-weekday-{column}").into_boxed_str(),
                ))
                .unwrap();
            let cell = cx
                .debug_bounds(Box::leak(cell_key.into_boxed_str()))
                .unwrap();
            assert_eq!(header.size.width, px(36.));
            assert_eq!(cell.size.width, px(36.));
            assert!(
                (f32::from(header.center().x - cell.center().x)).abs() <= 0.1,
                "{duration:?}, column={column}: {header:?} vs {cell:?}"
            );
        }
    }
}

#[gpui::test]
fn multi_month_cells_fill_the_documented_panel_width(cx: &mut TestAppContext) {
    for range in [false, true] {
        let state = cx.new(|cx| CalendarState::new(cx));
        let range_state = cx.new(|cx| DateRangeState::new(cx));
        let id = if range {
            range_state.entity_id()
        } else {
            state.entity_id()
        }
        .as_u64();
        let cx = open_host(cx, move || {
            if range {
                RangeCalendar::new(range_state.clone())
                    .default_value((Date::new(2026, 8, 3), Date::new(2026, 8, 4)))
                    .selection_alignment(herogpui_components::SelectionAlignment::Start)
                    .visible_duration(VisibleDuration::Months(2))
                    .first_day_of_week(Weekday::Mon)
                    .into_any_element()
            } else {
                Calendar::new(state.clone())
                    .default_value(Date::new(2026, 8, 3))
                    .visible_duration(VisibleDuration::Months(2))
                    .first_day_of_week(Weekday::Mon)
                    .into_any_element()
            }
        });
        let mut bounds = |month, day| {
            let key = if range {
                range_cell_selector(id, 2026, month, day)
            } else {
                cal_cell_selector(id, 2026, month, day)
            };
            cx.debug_bounds(Box::leak(key.into_boxed_str())).unwrap()
        };
        let first = bounds(8, 3);
        let last = bounds(8, 9);
        assert!(
            (f32::from(first.size.width) - 256. / 7.).abs() < 0.1,
            "range={range}: {first:?}"
        );
        assert_eq!(first.size.width, first.size.height);
        assert!(
            (f32::from(last.right() - first.left()) - 256.).abs() < 0.1,
            "range={range}: first={first:?}, last={last:?}"
        );
        // September 7 is Monday, so the matching column is one panel plus gap away.
        let next = bounds(9, 7);
        assert!((f32::from(next.left() - first.left()) - 288.).abs() < 0.1);
    }
}

#[gpui::test]
fn multi_month_view_scrolls_to_the_second_month_in_a_narrow_host(cx: &mut TestAppContext) {
    for range in [false, true] {
        let state = cx.new(|cx| CalendarState::new(cx));
        let range_state = cx.new(|cx| DateRangeState::new(cx));
        let id = if range {
            range_state.entity_id()
        } else {
            state.entity_id()
        }
        .as_u64();
        let cx = open_host(cx, move || {
            let content = if range {
                RangeCalendar::new(range_state.clone())
                    .default_value((Date::new(2026, 8, 3), Date::new(2026, 8, 4)))
                    .selection_alignment(herogpui_components::SelectionAlignment::Start)
                    .visible_duration(VisibleDuration::Months(2))
                    .first_day_of_week(Weekday::Mon)
                    .into_any_element()
            } else {
                Calendar::new(state.clone())
                    .default_value(Date::new(2026, 8, 3))
                    .visible_duration(VisibleDuration::Months(2))
                    .first_day_of_week(Weekday::Mon)
                    .into_any_element()
            };
            gpui::div().w(px(320.)).child(content).into_any_element()
        });
        press(cx, "tab");
        press(cx, "end");
        press(cx, "down");
        press(cx, "right");
        cx.run_until_parked();
        let key = if range {
            range_cell_selector(id, 2026, 9, 8)
        } else {
            cal_cell_selector(id, 2026, 9, 8)
        };
        let bounds = cx.debug_bounds(Box::leak(key.into_boxed_str())).unwrap();
        assert!(
            bounds.left() >= px(0.) && bounds.right() <= px(320.5),
            "keyboard range={range}: {bounds:?}"
        );
        press(cx, "home");
        press(cx, "up");
        cx.run_until_parked();
        let key = if range {
            range_cell_selector(id, 2026, 8, 25)
        } else {
            cal_cell_selector(id, 2026, 8, 25)
        };
        let bounds = cx.debug_bounds(Box::leak(key.into_boxed_str())).unwrap();
        assert!(
            bounds.left() >= px(0.) && bounds.right() <= px(320.5),
            "keyboard return range={range}: {bounds:?}"
        );
        cx.simulate_event(gpui::ScrollEvent {
            position: point(px(160.), px(90.)),
            delta: gpui::ScrollDelta::Pixels(point(px(-500.), px(0.))),
            ..Default::default()
        });
        cx.update(|window, _| window.refresh());
        let key = if range {
            range_cell_selector(id, 2026, 9, 27)
        } else {
            cal_cell_selector(id, 2026, 9, 27)
        };
        let bounds = cx.debug_bounds(Box::leak(key.into_boxed_str())).unwrap();
        assert!(
            bounds.left() >= px(280.) && bounds.right() <= px(320.5),
            "range={range}: {bounds:?}"
        );
    }
}

#[gpui::test]
fn multi_month_year_picker_reveals_its_active_column(cx: &mut TestAppContext) {
    for range in [false, true] {
        for initial_scroll in [0., -500.] {
            let state = cx.new(|cx| CalendarState::new(cx));
            let range_state = cx.new(|cx| DateRangeState::new(cx));
            let base = if range {
                format!(
                    r#"NamedInteger("range-cal", {})"#,
                    range_state.entity_id().as_u64()
                )
            } else {
                format!(r#"NamedInteger("cal", {})"#, state.entity_id().as_u64())
            };
            let open = Rc::new(std::cell::Cell::new(false));
            let view_open = open.clone();
            let focused = Rc::new(std::cell::Cell::new(Date::new(2020, 8, 10)));
            let view_focused = focused.clone();
            let cx = open_host(cx, move || {
                let focused = view_focused.clone();
                let open = view_open.clone();
                let content = if range {
                    RangeCalendar::new(range_state.clone())
                        .default_value((Date::new(2020, 8, 10), Date::new(2020, 8, 15)))
                        .focused_value(view_focused.get())
                        .on_focus_change(move |date, window, _| {
                            focused.set(*date);
                            window.refresh();
                        })
                        .min_value(Date::new(2000, 1, 1))
                        .max_value(Date::new(2040, 12, 31))
                        .visible_duration(VisibleDuration::Months(2))
                        .is_year_picker_open(view_open.get())
                        .on_year_picker_open_change(move |value, window, _| {
                            open.set(*value);
                            window.refresh();
                        })
                        .into_any_element()
                } else {
                    Calendar::new(state.clone())
                        .default_value(Date::new(2020, 8, 10))
                        .focused_value(view_focused.get())
                        .on_focus_change(move |date, window, _| {
                            focused.set(*date);
                            window.refresh();
                        })
                        .min_value(Date::new(2000, 1, 1))
                        .max_value(Date::new(2040, 12, 31))
                        .visible_duration(VisibleDuration::Months(2))
                        .is_year_picker_open(view_open.get())
                        .on_year_picker_open_change(move |value, window, _| {
                            open.set(*value);
                            window.refresh();
                        })
                        .into_any_element()
                };
                gpui::div().w(px(320.)).child(content).into_any_element()
            });
            cx.simulate_event(gpui::ScrollEvent {
                position: point(px(160.), px(90.)),
                delta: gpui::ScrollDelta::Pixels(point(px(initial_scroll), px(0.))),
                ..Default::default()
            });
            open.set(true);
            cx.update(|window, _| window.refresh());
            for (key, year) in [
                (None, 2020),
                (Some("left"), 2019),
                (Some("left"), 2018),
                (Some("right"), 2019),
            ] {
                if let Some(key) = key {
                    press(cx, key);
                }
                cx.run_until_parked();
                let key = Box::leak(format!("{base}-y{year}").into_boxed_str());
                let bounds = cx.debug_bounds(key).unwrap();
                assert!(
                    bounds.left() >= px(0.) && bounds.right() <= px(320.5),
                    "range={range}, initial_scroll={initial_scroll}, year={year}: {bounds:?}"
                );
            }
            press(cx, "home");
            press(cx, "shift-tab");
            cx.run_until_parked();
            // The second heading sits at the right edge of the same month strip.
            let right = cx
                .debug_bounds(Box::leak(format!("{base}-y2002").into_boxed_str()))
                .unwrap();
            assert!(
                right.left() >= px(0.) && right.right() <= px(320.5),
                "open heading range={range}: {right:?}"
            );
        }
    }
}

#[gpui::test]
fn day_views_keep_week_columns_and_disable_leading_dates(cx: &mut TestAppContext) {
    for range in [false, true] {
        let calendar = cx.new(|cx| CalendarState::new(cx));
        let range_calendar = cx.new(|cx| DateRangeState::new(cx));
        let states = Rc::new(RefCell::new(HashMap::new()));
        let view_states = states.clone();
        let changes = events();
        let view_changes = changes.clone();
        let cx = open_host(cx, move || {
            let states = view_states.clone();
            let changes = view_changes.clone();
            let probe = move |date: Date, label: gpui::SharedString, disabled, outside| {
                states
                    .borrow_mut()
                    .insert(date.format_iso(), (disabled, outside));
                gpui::div().child(label).into_any_element()
            };
            if range {
                RangeCalendar::new(range_calendar.clone())
                    .default_value((Date::new(2026, 9, 2), Date::new(2026, 9, 4)))
                    .selection_alignment(herogpui_components::SelectionAlignment::Start)
                    .visible_duration(VisibleDuration::Days(3))
                    .first_day_of_week(Weekday::Mon)
                    .cell(move |cell| {
                        probe(
                            cell.date,
                            cell.formatted_date.clone(),
                            cell.is_disabled,
                            cell.is_outside_month,
                        )
                    })
                    .on_change(move |start, end, _, _| {
                        changes.borrow_mut().push(format!(
                            "{}..{}",
                            start.format_iso(),
                            end.format_iso()
                        ));
                    })
                    .into_any_element()
            } else {
                Calendar::new(calendar.clone())
                    .default_value(Date::new(2026, 9, 2))
                    .selection_alignment(herogpui_components::SelectionAlignment::Start)
                    .visible_duration(VisibleDuration::Days(3))
                    .first_day_of_week(Weekday::Mon)
                    .cell(move |cell| {
                        probe(
                            cell.date,
                            cell.formatted_date.clone(),
                            cell.is_disabled,
                            cell.is_outside_month,
                        )
                    })
                    .on_change(move |date, _, _| {
                        changes.borrow_mut().push(date.unwrap().format_iso());
                    })
                    .into_any_element()
            }
        });
        assert_eq!(
            states.borrow().len(),
            5,
            "range={range}: two leading dates and three visible dates"
        );
        for date in ["2026-08-31", "2026-09-01"] {
            assert_eq!(
                states.borrow().get(date),
                Some(&(true, false)),
                "range={range}: {date}"
            );
        }
        for date in ["2026-09-02", "2026-09-03", "2026-09-04"] {
            assert_eq!(
                states.borrow().get(date),
                Some(&(false, false)),
                "range={range}: {date}"
            );
        }
        click(cx, 18., 74.);
        click(cx, 54., 74.);
        assert!(changes.borrow().is_empty(), "leading dates cannot select");
        click(cx, 126., 74.);
        if range {
            click(cx, 162., 74.);
            assert_eq!(changes.borrow().as_slice(), ["2026-09-03..2026-09-04"]);
        } else {
            assert_eq!(changes.borrow().as_slice(), ["2026-09-03"]);
        }
    }
}

/// Column *c*'s centre in a bare Calendar: seven cells across
/// `CALENDAR_WIDTH` with no horizontal gaps.
fn cal_col_x(col: usize) -> f32 {
    let cell_w = f32::from(CALENDAR_WIDTH) / 7.;
    col as f32 * cell_w + cell_w / 2.
}

/// Row *r*'s centre in a bare Calendar: first row at y = 86, then 36px per row.
fn cal_row_y(row: usize) -> f32 {
    86. + row as f32 * 36.
}

/// The centre of the cell holding `day` of `(year, month)` in a bare
/// Calendar, derived from the month's leading blanks (the same
/// `DateConstraints` the test's calendars use).
fn cal_day(year: i32, month: u32, day: u32) -> (f32, f32) {
    let lead = DateConstraints::new().lead_cells(year, month);
    let idx = day as usize + lead - 1;
    (cal_col_x(idx % 7), cal_row_y(idx / 7))
}

/// The centre of the cell holding `day` of `(year, month)` in a bare
/// RangeCalendar, derived from the month's leading blanks. The pinned grid
/// runs seven 36px cells across the 252px width with no horizontal gaps, and
/// 40px per row: the 36px cell plus the pinned `my-[2px]` cell margins that
/// separate two rows.
fn range_day(year: i32, month: u32, day: u32) -> (f32, f32) {
    let lead = DateConstraints::new().lead_cells(year, month);
    let idx = day as usize + lead - 1;
    (18. + 36. * (idx % 7) as f32, 88. + 40. * (idx / 7) as f32)
}

/// React Aria disables a month button when the day immediately beyond that
/// side of the visible range is outside minValue/maxValue. August is the only
/// valid month here, so neither chevron may move the state entity away from it.
#[gpui::test]
fn calendar_nav_buttons_stop_at_min_and_max(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .min_value(Date::new(2026, 8, 10))
            .max_value(Date::new(2026, 8, 20))
            .into_any_element()
    });

    click(cx, 14., 12.);
    let previous = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month)
    });
    assert_eq!(previous, (2026, 8), "previous must stop at minValue");

    click(cx, 238., 12.);
    let next = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month)
    });
    assert_eq!(next, (2026, 8), "next must stop at maxValue");
}

/// RangeCalendar inherits the same useCalendarBase button contract. Its
/// 252px column puts the next button at x=238; both adjacent months are fully
/// outside the allowed August interval.
#[gpui::test]
fn range_calendar_nav_buttons_stop_at_min_and_max(cx: &mut TestAppContext) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        RangeCalendar::new(state_for_view.clone())
            .default_value((Date::new(2026, 8, 15), Date::new(2026, 8, 16)))
            .min_value(Date::new(2026, 8, 10))
            .max_value(Date::new(2026, 8, 20))
            .into_any_element()
    });

    click(cx, 14., 12.);
    let previous = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month)
    });
    assert_eq!(previous, (2026, 8), "previous must stop at minValue");

    click(cx, 238., 12.);
    let next = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month)
    });
    assert_eq!(next, (2026, 8), "next must stop at maxValue");
}

/// HeroUI's Calendar wrapper always supplies Gregorian 1900-01-01 and
/// 2099-12-31 when callers omit minValue/maxValue.
#[gpui::test]
fn calendar_implicit_bounds_stop_navigation(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 1900;
        state.view_month = 1;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone()).into_any_element()
    });

    click(cx, 14., 12.);
    assert_eq!(
        cx.update(|_, cx| (state.read(cx).view_year, state.read(cx).view_month)),
        (1900, 1)
    );

    cx.update(|window, cx| {
        state.update(cx, |state, _| {
            state.view_year = 2099;
            state.view_month = 12;
        });
        window.refresh();
    });
    click(cx, 238., 12.);
    assert_eq!(
        cx.update(|_, cx| (state.read(cx).view_year, state.read(cx).view_month)),
        (2099, 12)
    );
}

#[gpui::test]
fn range_calendar_implicit_bounds_stop_navigation(cx: &mut TestAppContext) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 1900;
        state.view_month = 1;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        RangeCalendar::new(state_for_view.clone()).into_any_element()
    });

    click(cx, 14., 12.);
    assert_eq!(
        cx.update(|_, cx| (state.read(cx).view_year, state.read(cx).view_month)),
        (1900, 1)
    );

    cx.update(|window, cx| {
        state.update(cx, |state, _| {
            state.view_year = 2099;
            state.view_month = 12;
        });
        window.refresh();
    });
    click(cx, 238., 12.);
    assert_eq!(
        cx.update(|_, cx| (state.read(cx).view_year, state.read(cx).view_month)),
        (2099, 12)
    );
}

/// A partly valid visible month keeps navigation alive in the valid direction.
/// `isDateUnavailable` is deliberately independent: React Stately consults
/// only minValue/maxValue for the adjacent-day paging predicate, and readOnly
/// prevents selection without freezing the visible month.
#[gpui::test]
fn calendar_partial_bounds_read_only_and_unavailable_dates_keep_valid_paging(
    cx: &mut TestAppContext,
) {
    let focuses = events();
    let focused = focuses.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let changes = changes.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .min_value(Date::new(2026, 8, 10))
            .max_value(Date::new(2026, 9, 20))
            .is_date_unavailable(|date| date.month == 9)
            .is_read_only(true)
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .on_change(move |date, _, _| {
                changes
                    .borrow_mut()
                    .push(date.map_or_else(|| "none".into(), |value| value.format_iso()));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(focused.borrow().as_slice(), ["2026-08-16"]);
    assert!(
        changed.borrow().is_empty(),
        "readOnly must keep grid navigation but block selection"
    );

    click(cx, 14., 12.);
    let previous = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month)
    });
    assert_eq!(
        previous,
        (2026, 8),
        "the previous adjacent day is before minValue"
    );

    click(cx, 238., 12.);
    let next = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month)
    });
    assert_eq!(
        next,
        (2026, 9),
        "readOnly and unavailable September dates must not disable paging"
    );
}

/// RangeCalendar has the same read-only split: its grid stays in the tab order
/// and arrows move the focus, but Enter cannot start or replace a range.
#[gpui::test]
fn range_calendar_read_only_keeps_grid_navigation_without_selection(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let changes = changes.clone();
        RangeCalendar::new(state_for_view.clone())
            .default_value((Date::new(2026, 8, 15), Date::new(2026, 8, 16)))
            .is_read_only(true)
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}..{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(focused.borrow().as_slice(), ["2026-08-16"]);
    assert!(changed.borrow().is_empty());
    let value = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.start, state.end)
    });
    assert_eq!(
        value,
        (Some(Date::new(2026, 8, 15)), Some(Date::new(2026, 8, 16)))
    );
}

/// Pinned React Stately keeps the first endpoint in `anchorDate`; `onChange`
/// is reserved for the completed range even though the preview begins at once.
#[gpui::test]
fn range_calendar_first_endpoint_does_not_publish_a_half_open_value(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
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

    let (day10_x, day10_y) = range_day(2026, 8, 10);
    click(cx, day10_x, day10_y);

    assert!(
        changed.borrow().is_empty(),
        "the pending anchor is internal selection state, not a changed value"
    );
    let value = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.start, state.end)
    });
    assert_eq!(value, (Some(Date::new(2026, 8, 10)), None));
}

/// A first keyboard endpoint advances the ring one day so a second Enter
/// completes a range instead of selecting the same day twice.
#[gpui::test]
fn range_calendar_first_keyboard_pick_advances_focus(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let changes = events();
    let changed = changes.clone();
    let selected = Rc::new(RefCell::new(HashMap::new()));
    let selected_cells = selected.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let changes = changes.clone();
        let selected_cells = selected_cells.clone();
        RangeCalendar::new(state_for_view.clone())
            .cell(move |cell| {
                if !cell.is_outside_month && matches!(cell.date.day, 10 | 11 | 12 | 15) {
                    selected_cells
                        .borrow_mut()
                        .insert(cell.date.day, cell.is_selected);
                }
                gpui::div().into_any_element()
            })
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    let (day10_x, day10_y) = range_day(2026, 8, 10);
    click(cx, day10_x, day10_y);
    press(cx, "escape");
    focused.borrow_mut().clear();
    changed.borrow_mut().clear();

    let (day15_x, day15_y) = range_day(2026, 8, 15);
    cx.simulate_mouse_move(
        point(px(day15_x), px(day15_y)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    press(cx, "enter");
    assert_eq!(selected.borrow().get(&10), Some(&true));
    assert_eq!(selected.borrow().get(&11), Some(&true));
    assert_eq!(selected.borrow().get(&12), Some(&false));
    assert_eq!(selected.borrow().get(&15), Some(&false));

    let (day14_x, day14_y) = range_day(2026, 8, 14);
    cx.simulate_mouse_move(
        point(px(day14_x), px(day14_y)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    press(cx, "right");
    assert_eq!(selected.borrow().get(&12), Some(&true));
    assert_eq!(selected.borrow().get(&15), Some(&true));
    press(cx, "space");

    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-08-11", "2026-08-14", "2026-08-15"]
    );
    assert_eq!(changed.borrow().as_slice(), ["2026-08-10->2026-08-15"]);
    let value = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.start, state.end)
    });
    assert_eq!(
        value,
        (Some(Date::new(2026, 8, 10)), Some(Date::new(2026, 8, 15)))
    );
}

/// When the following day is unavailable in a contiguous range, pinned
/// React Stately advances to the previous available day instead.
#[gpui::test]
fn range_calendar_keyboard_focus_falls_back_before_an_unavailable_day(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let changes = changes.clone();
        RangeCalendar::new(state_for_view.clone())
            .is_date_unavailable(|date, anchor| {
                anchor == Some(Date::new(2026, 8, 10)) && date == Date::new(2026, 8, 11)
            })
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    let (day10_x, day10_y) = range_day(2026, 8, 10);
    click(cx, day10_x, day10_y);
    press(cx, "escape");
    focused.borrow_mut().clear();
    changed.borrow_mut().clear();

    press(cx, "enter");
    press(cx, "enter");

    assert_eq!(focused.borrow().as_slice(), ["2026-08-09"]);
    assert_eq!(changed.borrow().as_slice(), ["2026-08-09->2026-08-10"]);
}

/// v3 passes the first endpoint back into `isDateUnavailable` while a range is
/// open. The predicate may therefore introduce an interior barrier only after
/// the anchor is chosen, and it returns to its anchor-free result once the
/// range is complete.
#[gpui::test]
fn range_calendar_unavailable_dates_follow_the_active_anchor(cx: &mut TestAppContext) {
    let anchors = Rc::new(RefCell::new(Vec::new()));
    let seen_anchors = anchors.clone();
    let unavailable = Rc::new(RefCell::new(None));
    let unavailable_probe = unavailable.clone();
    let selected = Rc::new(RefCell::new(HashMap::new()));
    let selected_probe = selected.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let anchors = anchors.clone();
        let unavailable = unavailable.clone();
        let selected = selected.clone();
        RangeCalendar::new(state_for_view.clone())
            .is_date_unavailable(move |date, anchor| {
                if date == Date::new(2026, 8, 13) {
                    anchors.borrow_mut().push(anchor);
                }
                anchor == Some(Date::new(2026, 8, 10)) && date == Date::new(2026, 8, 13)
            })
            .cell(move |cell| {
                if cell.date == Date::new(2026, 8, 13) {
                    *unavailable.borrow_mut() = Some(cell.is_unavailable);
                }
                if (12..=14).contains(&cell.date.day) && cell.date.month == 8 {
                    selected
                        .borrow_mut()
                        .insert(cell.date.day, cell.is_selected);
                }
                gpui::div().size(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        *unavailable_probe.borrow(),
        Some(false),
        "without an anchor the first endpoint remains available"
    );
    assert_eq!(seen_anchors.borrow().last(), Some(&None));

    let (start_x, start_y) = range_day(2026, 8, 10);
    click(cx, start_x, start_y);
    cx.update(|window, _| window.refresh());
    assert_eq!(
        *unavailable_probe.borrow(),
        Some(true),
        "the active anchor must be supplied while the second endpoint is pending"
    );
    assert_eq!(
        seen_anchors.borrow().last(),
        Some(&Some(Date::new(2026, 8, 10)))
    );

    let (end_x, end_y) = range_day(2026, 8, 15);
    cx.simulate_mouse_move(
        point(px(end_x), px(end_y)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    cx.update(|window, _| window.refresh());
    assert_eq!(
        selected_probe.borrow().get(&12),
        Some(&false),
        "hovering a disabled target must not create a partial preview"
    );
    assert_eq!(
        selected_probe.borrow().get(&13),
        Some(&false),
        "the anchor-derived unavailable date must stay outside the preview"
    );
    assert_eq!(
        selected_probe.borrow().get(&14),
        Some(&false),
        "dates beyond the anchor-derived bound must remain outside the preview"
    );
    let (selectable_end_x, selectable_end_y) = range_day(2026, 8, 12);
    click(cx, selectable_end_x, selectable_end_y);
    cx.update(|window, _| window.refresh());
    let value = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.start, state.end)
    });
    assert_eq!(
        value,
        (Some(Date::new(2026, 8, 10)), Some(Date::new(2026, 8, 12))),
        "the last selectable date before the anchor-derived barrier must complete the range"
    );
    assert_eq!(
        *unavailable_probe.borrow(),
        Some(false),
        "a completed range must call the predicate without an active anchor"
    );
    assert_eq!(seen_anchors.borrow().last(), Some(&None));
}

/// Non-contiguous mode lets keyboard focus move onto a date rejected relative
/// to the active anchor, but pinned React Aria still ignores Enter there.
#[gpui::test]
fn range_calendar_anchor_unavailable_keyboard_endpoint_is_inert(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        RangeCalendar::new(state_for_view.clone())
            .allows_non_contiguous_ranges(true)
            .is_date_unavailable(|date, anchor| {
                anchor == Some(Date::new(2026, 8, 10)) && date == Date::new(2026, 8, 13)
            })
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    let (anchor_x, anchor_y) = range_day(2026, 8, 10);
    click(cx, anchor_x, anchor_y);
    press(cx, "right");
    press(cx, "right");
    press(cx, "right");
    press(cx, "enter");

    assert!(changed.borrow().is_empty());
    let value = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.start, state.end)
    });
    assert_eq!(value, (Some(Date::new(2026, 8, 10)), None));
}

/// Escape cancels only an in-progress keyboard anchor. It reports no value or
/// focus change and leaves the grid ready to start a fresh range.
#[gpui::test]
fn range_calendar_escape_cancels_a_half_open_keyboard_range(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let changes = changes.clone();
        RangeCalendar::new(state_for_view.clone())
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    let (day10_x, day10_y) = range_day(2026, 8, 10);
    click(cx, day10_x, day10_y);
    press(cx, "escape");
    focused.borrow_mut().clear();
    changed.borrow_mut().clear();

    press(cx, "enter");
    press(cx, "escape");

    assert_eq!(focused.borrow().as_slice(), ["2026-08-11"]);
    assert!(changed.borrow().is_empty());
    let value = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.start, state.end)
    });
    assert_eq!(value, (None, None));
}

/// Cancelling a pointer anchor leaves focus on its date, so keyboard input can
/// immediately start a fresh range there.
#[gpui::test]
fn range_calendar_escape_keeps_pointer_focus_for_the_next_keyboard_pick(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let changes = changes.clone();
        RangeCalendar::new(state_for_view.clone())
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    let (day5_x, day5_y) = range_day(2026, 8, 5);
    click(cx, day5_x, day5_y);
    press(cx, "escape");
    press(cx, "enter");

    assert_eq!(focused.borrow().as_slice(), ["2026-08-05", "2026-08-06"]);
    assert!(changed.borrow().is_empty());
}

/// Cancelling a replacement anchor restores the last committed range rather
/// than clearing the value it temporarily covered.
#[gpui::test]
fn range_calendar_escape_restores_the_committed_range(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        RangeCalendar::new(state_for_view.clone())
            .default_value((Date::new(2026, 8, 5), Date::new(2026, 8, 8)))
            .on_change(move |_, _, _, _| changes.borrow_mut().push("changed".into()))
            .into_any_element()
    });

    let (day10_x, day10_y) = range_day(2026, 8, 10);
    click(cx, day10_x, day10_y);
    let replacement = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.start, state.end)
    });
    assert_eq!(replacement, (Some(Date::new(2026, 8, 10)), None));
    let reports_before_escape = changed.borrow().len();
    press(cx, "escape");

    assert_eq!(changed.borrow().len(), reports_before_escape);
    let value = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.start, state.end)
    });
    assert_eq!(
        value,
        (Some(Date::new(2026, 8, 5)), Some(Date::new(2026, 8, 8)))
    );
}

/// If both adjacent dates are outside the allowed range, the first keyboard
/// endpoint keeps focus and a second Enter completes a one-day range.
#[gpui::test]
fn range_calendar_keyboard_focus_stays_when_both_neighbours_are_invalid(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let changes = changes.clone();
        RangeCalendar::new(state_for_view.clone())
            .min_value(Date::new(2026, 8, 10))
            .max_value(Date::new(2026, 8, 10))
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    let (day10_x, day10_y) = range_day(2026, 8, 10);
    click(cx, day10_x, day10_y);
    press(cx, "escape");
    focused.borrow_mut().clear();
    changed.borrow_mut().clear();

    press(cx, "enter");
    press(cx, "enter");

    assert!(focused.borrow().is_empty());
    assert_eq!(changed.borrow().as_slice(), ["2026-08-10->2026-08-10"]);
}

/// Non-contiguous mode leaves an unavailable neighbour focusable; it blocks
/// selection only when the user tries to take it as an endpoint.
#[gpui::test]
fn range_calendar_non_contiguous_keyboard_focus_can_advance_to_an_unavailable_day(
    cx: &mut TestAppContext,
) {
    let focuses = events();
    let focused = focuses.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let changes = changes.clone();
        RangeCalendar::new(state_for_view.clone())
            .allows_non_contiguous_ranges(true)
            .is_date_unavailable(|date, _| date == Date::new(2026, 8, 11))
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    let (day10_x, day10_y) = range_day(2026, 8, 10);
    click(cx, day10_x, day10_y);
    press(cx, "escape");
    focused.borrow_mut().clear();
    changed.borrow_mut().clear();

    press(cx, "enter");
    press(cx, "enter");

    assert_eq!(focused.borrow().as_slice(), ["2026-08-11"]);
    assert!(changed.borrow().is_empty());
}

/// Auto-advance across a month edge realigns the visible window so the focused
/// end remains rendered.
#[gpui::test]
fn range_calendar_keyboard_advance_realigns_the_visible_month(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        RangeCalendar::new(state_for_view.clone())
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .into_any_element()
    });

    let (day31_x, day31_y) = range_day(2026, 8, 31);
    click(cx, day31_x, day31_y);
    press(cx, "escape");
    focused.borrow_mut().clear();

    press(cx, "enter");

    assert_eq!(focused.borrow().as_slice(), ["2026-09-01"]);
    let visible = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month, state.view_day)
    });
    assert_eq!(visible, (2026, 9, 1));
}

/// `focusedValue` is controlled state in pinned React Stately. It realigns the
/// visible range, while attempted keyboard moves only report a proposal until
/// the owner supplies a new value.
#[gpui::test]
fn range_calendar_controlled_focus_realigns_and_waits_for_its_owner(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let focused_day_outside = Rc::new(RefCell::new(None));
    let outside_for_view = focused_day_outside.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let focused_day_outside = outside_for_view.clone();
        RangeCalendar::new(state_for_view.clone())
            .focused_value(Date::new(2026, 9, 1))
            .cell(move |cell| {
                if cell.date == Date::new(2026, 9, 1) {
                    *focused_day_outside.borrow_mut() = Some(cell.is_outside_month);
                }
                gpui::div()
                    .child(cell.formatted_date.clone())
                    .into_any_element()
            })
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .into_any_element()
    });

    assert_eq!(
        *focused_day_outside.borrow(),
        Some(false),
        "the controlled focusedValue must be inside the rendered month"
    );

    press(cx, "tab");
    press(cx, "right right");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-09-02", "2026-09-02"],
        "owner-rejected moves must keep proposing from the controlled date"
    );
}

/// Pinned React Stately starts a long selected range at the first visible
/// month by default, but an explicit `center` keeps the selection start in the
/// centered unit even when the range end falls outside the window.
#[gpui::test]
fn range_calendar_long_range_auto_aligns_start_without_overriding_center(cx: &mut TestAppContext) {
    let automatic = Rc::new(RefCell::new(Vec::new()));
    let automatic_for_view = automatic.clone();
    let centered = Rc::new(RefCell::new(Vec::new()));
    let centered_for_view = centered.clone();
    let automatic_state = cx.new(|cx| DateRangeState::new(cx));
    let centered_state = cx.new(|cx| DateRangeState::new(cx));
    let cx = open_host(cx, move || {
        let automatic = automatic_for_view.clone();
        let centered = centered_for_view.clone();
        gpui::div()
            .child(
                RangeCalendar::new(automatic_state.clone())
                    .default_value((Date::new(2026, 8, 10), Date::new(2026, 10, 15)))
                    .visible_duration(VisibleDuration::Months(3))
                    .cell(move |cell| {
                        if !cell.is_outside_month {
                            let month = (cell.date.year, cell.date.month);
                            if !automatic.borrow().contains(&month) {
                                automatic.borrow_mut().push(month);
                            }
                        }
                        gpui::div().into_any_element()
                    }),
            )
            .child(
                RangeCalendar::new(centered_state.clone())
                    .default_value((Date::new(2026, 8, 10), Date::new(2026, 10, 15)))
                    .visible_duration(VisibleDuration::Months(3))
                    .selection_alignment(herogpui_components::SelectionAlignment::Center)
                    .cell(move |cell| {
                        if !cell.is_outside_month {
                            let month = (cell.date.year, cell.date.month);
                            if !centered.borrow().contains(&month) {
                                centered.borrow_mut().push(month);
                            }
                        }
                        gpui::div().into_any_element()
                    }),
            )
            .into_any_element()
    });
    cx.update(|window, _| window.refresh());

    assert_eq!(
        automatic.borrow().as_slice(),
        [(2026, 8), (2026, 9), (2026, 10)]
    );
    assert_eq!(
        centered.borrow().as_slice(),
        [(2026, 7), (2026, 8), (2026, 9)]
    );
}

#[gpui::test]
fn calendar_controlled_focus_realigns_and_waits_for_its_owner(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let focused_day_outside = Rc::new(RefCell::new(None));
    let outside_for_view = focused_day_outside.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let focused_day_outside = outside_for_view.clone();
        Calendar::new(state_for_view.clone())
            .focused_value(Date::new(2026, 9, 1))
            .cell(move |cell| {
                if cell.date == Date::new(2026, 9, 1) {
                    *focused_day_outside.borrow_mut() = Some(cell.is_outside_month);
                }
                gpui::div()
                    .child(cell.formatted_date.clone())
                    .into_any_element()
            })
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .into_any_element()
    });

    assert_eq!(*focused_day_outside.borrow(), Some(false));
    press(cx, "tab");
    press(cx, "right right");
    assert_eq!(focused.borrow().as_slice(), ["2026-09-02", "2026-09-02"]);
}

/// The calendar grid is the first tab stop. The enabled previous chevron is
/// next and activates on Enter, proving the nav controls are keyboard-reachable
/// rather than pointer-only.
#[gpui::test]
fn calendar_enabled_nav_button_is_a_tab_stop(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    let view = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month)
    });
    assert_eq!(view, (2026, 7));
}

/// Both chevrons are disabled by the August-only bounds, and the plain
/// heading is not a tab stop. They must not remain in gpui's tab registry:
/// after the grid, the next Tab reaches the following button rather than
/// landing on a dead chevron.
#[gpui::test]
fn calendar_disabled_nav_buttons_leave_the_tab_order(cx: &mut TestAppContext) {
    let presses = events();
    let presses_for_view = presses.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let pressed = presses_for_view.clone();
        gpui::div()
            .child(
                Calendar::new(state_for_view.clone())
                    .default_value(Date::new(2026, 8, 15))
                    .min_value(Date::new(2026, 8, 10))
                    .max_value(Date::new(2026, 8, 20)),
            )
            .child(
                Button::new("after-calendar")
                    .label("After")
                    .on_press(move |_, _, _| pressed.borrow_mut().push("after".into())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(presses.borrow().as_slice(), ["after"]);
}

#[gpui::test]
fn year_picker_keeps_the_day_view_dimensions(cx: &mut TestAppContext) {
    for range in [false, true] {
        for duration in [
            VisibleDuration::Months(1),
            VisibleDuration::Months(2),
            VisibleDuration::Weeks(2),
            VisibleDuration::Days(3),
        ] {
            let mut sizes = Vec::new();
            for open in [false, true] {
                let calendar = cx.new(|cx| CalendarState::new(cx));
                let range_calendar = cx.new(|cx| DateRangeState::new(cx));
                let cx = open_host(cx, move || {
                    let content = if range {
                        RangeCalendar::new(range_calendar.clone())
                            .default_value((Date::new(2026, 8, 10), Date::new(2026, 8, 15)))
                            .visible_duration(duration)
                            .default_year_picker_open(open)
                            .into_any_element()
                    } else {
                        Calendar::new(calendar.clone())
                            .default_value(Date::new(2026, 8, 10))
                            .visible_duration(duration)
                            .default_year_picker_open(open)
                            .into_any_element()
                    };
                    gpui::div()
                        .flex()
                        .child(
                            gpui::div()
                                .debug_selector(|| "year-picker-footprint".to_owned())
                                .child(content),
                        )
                        .into_any_element()
                });
                sizes.push(cx.debug_bounds("year-picker-footprint").unwrap().size);
            }
            assert_eq!(sizes[0], sizes[1], "range={range}, duration={duration:?}");
        }
    }
}

#[gpui::test]
fn year_picker_reveals_the_opening_and_keyboard_year(cx: &mut TestAppContext) {
    for range in [false, true] {
        let calendar = cx.new(|cx| CalendarState::new(cx));
        let range_calendar = cx.new(|cx| DateRangeState::new(cx));
        let base = if range {
            format!(
                r#"NamedInteger("range-cal", {})"#,
                range_calendar.entity_id().as_u64()
            )
        } else {
            format!(r#"NamedInteger("cal", {})"#, calendar.entity_id().as_u64())
        };
        let lower_bound = Rc::new(std::cell::Cell::new(2000));
        let view_lower_bound = lower_bound.clone();
        let cx = open_host(cx, move || {
            if range {
                RangeCalendar::new(range_calendar.clone())
                    .default_value((Date::new(2020, 8, 10), Date::new(2020, 8, 15)))
                    .min_value(Date::new(view_lower_bound.get(), 1, 1))
                    .max_value(Date::new(2040, 12, 31))
                    .default_year_picker_open(true)
                    .into_any_element()
            } else {
                Calendar::new(calendar.clone())
                    .default_value(Date::new(2020, 8, 10))
                    .min_value(Date::new(view_lower_bound.get(), 1, 1))
                    .max_value(Date::new(2040, 12, 31))
                    .default_year_picker_open(true)
                    .into_any_element()
            }
        });
        let mut column_width = None;
        for (key, year) in [(None, 2020), (Some("end"), 2040), (Some("home"), 2000)] {
            if let Some(key) = key {
                press(cx, key);
            }
            cx.run_until_parked();
            let viewport = cx
                .debug_bounds(Box::leak(format!("{base}-year-viewport").into_boxed_str()))
                .unwrap();
            let cell = cx
                .debug_bounds(Box::leak(format!("{base}-y{year}").into_boxed_str()))
                .unwrap();
            assert!(
                cell.top() >= viewport.top() && cell.bottom() <= viewport.bottom(),
                "range={range}, year={year}: {cell:?} outside {viewport:?}"
            );
            assert_eq!(cell.size.height, px(32.));
            let width = *column_width.get_or_insert(cell.size.width);
            assert!(
                (f32::from(cell.size.width - width)).abs() <= 1.,
                "partial year rows must retain three equal columns"
            );
        }
        lower_bound.set(1900);
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let viewport = cx
            .debug_bounds(Box::leak(format!("{base}-year-viewport").into_boxed_str()))
            .unwrap();
        let cell = cx
            .debug_bounds(Box::leak(format!("{base}-y2000").into_boxed_str()))
            .unwrap();
        assert!(
            cell.top() >= viewport.top() && cell.bottom() <= viewport.bottom(),
            "range={range}: changed bounds must reveal the same active year: {cell:?} in {viewport:?}"
        );
    }
}

/// v3 overlays the year grid on the calendar and makes the month chevrons
/// pointer-inert while it is open. The port previously left both chevrons live
/// and repurposed them as twelve-year paging controls.
#[gpui::test]
fn calendar_year_picker_hides_month_navigation(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .default_year_picker_open(true)
            .into_any_element()
    });

    click(cx, 14., 12.);
    let previous = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month)
    });
    assert_eq!(previous, (2026, 8));

    click(cx, 238., 12.);
    let next = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month)
    });
    assert_eq!(next, (2026, 8));
}

/// v3's basic composition is a plain `Calendar.Heading`: without
/// `year_picker(true)` the heading is a label, not a tab stop. The standalone
/// tab order is grid, previous, next — the third Tab reaches Next, so Enter
/// pages forward instead of opening the year grid.
#[gpui::test]
fn calendar_heading_is_a_plain_label_without_year_picker(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");

    let view = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month)
    });
    assert_eq!(view, (2026, 9));
}

/// The heading is a real button in v3. In the standalone month layout the tab
/// order is grid, previous, heading; Enter opens it and transfers focus to the
/// selected year without requiring another Tab.
#[gpui::test]
fn calendar_year_picker_trigger_opens_from_the_keyboard(cx: &mut TestAppContext) {
    let open_changes = events();
    let open_changes_for_view = open_changes.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let open_changes = open_changes_for_view.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .year_picker(true)
            .on_year_picker_open_change(move |open, _, _| {
                open_changes.borrow_mut().push(open.to_string());
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    press(cx, "down");
    press(cx, "enter");

    let selected = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month, state.view_day)
    });
    assert_eq!(selected, (2029, 8, 15));
    assert_eq!(open_changes.borrow().as_slice(), ["true", "false"]);
}

/// Opening the v3 year picker focuses the selected year. Its three-column
/// keyboard delegate moves Down by three, and Enter selects that year and
/// closes the uncontrolled picker.
#[gpui::test]
fn calendar_year_picker_keyboard_moves_selects_and_closes(cx: &mut TestAppContext) {
    let open_changes = events();
    let open_changes_for_view = open_changes.clone();
    let focus_changes = events();
    let focus_changes_for_view = focus_changes.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let open_changes = open_changes_for_view.clone();
        let focus_changes = focus_changes_for_view.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .default_year_picker_open(true)
            .on_year_picker_open_change(move |open, _, _| {
                open_changes.borrow_mut().push(open.to_string());
            })
            .on_focus_change(move |date, _, _| {
                focus_changes.borrow_mut().push(date.format_iso());
            })
            .into_any_element()
    });

    press(cx, "down");
    press(cx, "enter");

    let selected = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month, state.view_day)
    });
    assert_eq!(selected, (2029, 8, 15));
    assert_eq!(focus_changes.borrow().as_slice(), ["2029-08-15"]);
    assert_eq!(open_changes.borrow().as_slice(), ["false"]);
}

/// `visibleYears` is a YearPickerGrid prop in v3. Home and End operate on that
/// bounded window, not on an invented decade page.
#[gpui::test]
fn calendar_year_picker_visible_years_respects_date_bounds(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .min_value(Date::new(2024, 1, 1))
            .max_value(Date::new(2028, 12, 31))
            .visible_years(3)
            .default_year_picker_open(true)
            .into_any_element()
    });

    press(cx, "home");
    press(cx, "enter");
    let selected = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month, state.view_day)
    });
    assert_eq!(selected, (2025, 8, 15));
}

/// Changing only the year must resolve the day against the destination month.
/// February 29, 2028 becomes February 28 in 2027 rather than an invalid date.
#[gpui::test]
fn calendar_year_picker_clamps_leap_day(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2028, 2, 29))
            .visible_years(5)
            .default_year_picker_open(true)
            .into_any_element()
    });

    press(cx, "left");
    press(cx, "enter");
    let selected = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month, state.view_day)
    });
    assert_eq!(selected, (2027, 2, 28));
}

/// Escape closes the uncontrolled picker and restores focus to its trigger.
/// Enter reopens it, proving the open-session autofocus resets; a second Escape
/// must restore the day-grid cell beneath it on the next frame.
#[gpui::test]
fn calendar_year_picker_escape_restores_the_day_grid(cx: &mut TestAppContext) {
    let open_changes = events();
    let open_changes_for_view = open_changes.clone();
    let selections = events();
    let selections_for_view = selections.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let open_changes = open_changes_for_view.clone();
        let selections = selections_for_view.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .default_year_picker_open(true)
            .on_year_picker_open_change(move |open, _, _| {
                open_changes.borrow_mut().push(open.to_string());
            })
            .on_change(move |date, _, _| {
                selections
                    .borrow_mut()
                    .push(date.map_or_else(|| "none".into(), |value| value.format_iso()));
            })
            .into_any_element()
    });

    press(cx, "escape");
    press(cx, "enter");
    press(cx, "right");
    press(cx, "escape");
    let (x, y) = cal_day(2026, 8, 16);
    click(cx, x, y);
    assert_eq!(open_changes.borrow().as_slice(), ["false", "true", "false"]);
    assert_eq!(selections.borrow().as_slice(), ["2026-08-16"]);
}

/// RangeCalendar uses the same year-picker delegate and uncontrolled close
/// state. Right advances one year without changing either selected range end.
#[gpui::test]
fn range_calendar_year_picker_keyboard_moves_and_closes(cx: &mut TestAppContext) {
    let open_changes = events();
    let open_changes_for_view = open_changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let open_changes = open_changes_for_view.clone();
        RangeCalendar::new(state_for_view.clone())
            .default_value((Date::new(2026, 8, 15), Date::new(2026, 8, 16)))
            .visible_years(5)
            .default_year_picker_open(true)
            .on_year_picker_open_change(move |open, _, _| {
                open_changes.borrow_mut().push(open.to_string());
            })
            .into_any_element()
    });

    press(cx, "right");
    press(cx, "enter");
    let view = cx.update(|_, cx| {
        let state = state.read(cx);
        (
            state.view_year,
            state.view_month,
            state.view_day,
            state.start,
            state.end,
        )
    });
    assert_eq!(
        view,
        (
            2027,
            8,
            15,
            Some(Date::new(2026, 8, 15)),
            Some(Date::new(2026, 8, 16))
        )
    );
    assert_eq!(open_changes.borrow().as_slice(), ["false"]);
}

/// In a multi-month Calendar, Escape restores focus to the heading that opened
/// the year picker. Tab must therefore reach the following next-month button,
/// not the other heading.
#[gpui::test]
fn calendar_year_picker_escape_returns_to_its_opening_heading(cx: &mut TestAppContext) {
    let open_changes = events();
    let open_changes_for_view = open_changes.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let open_changes = open_changes_for_view.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .visible_duration(VisibleDuration::Months(2))
            .year_picker(true)
            .on_year_picker_open_change(move |open, _, _| {
                open_changes.borrow_mut().push(open.to_string());
            })
            .into_any_element()
    });

    for _ in 0..4 {
        press(cx, "tab");
    }
    press(cx, "enter");
    press(cx, "escape");
    press(cx, "tab");
    press(cx, "enter");

    let view = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month)
    });
    assert_eq!(view, (2026, 10));
    assert_eq!(open_changes.borrow().as_slice(), ["true", "false"]);
}

/// Selecting a year has the same focus-restoration contract as Escape. This
/// drives RangeCalendar's separate exit path from its second visible heading.
#[gpui::test]
fn range_calendar_year_selection_returns_to_its_opening_heading(cx: &mut TestAppContext) {
    let open_changes = events();
    let open_changes_for_view = open_changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let open_changes = open_changes_for_view.clone();
        RangeCalendar::new(state_for_view.clone())
            .default_value((Date::new(2026, 8, 15), Date::new(2026, 8, 16)))
            .visible_duration(VisibleDuration::Months(2))
            .year_picker(true)
            .on_year_picker_open_change(move |open, _, _| {
                open_changes.borrow_mut().push(open.to_string());
            })
            .into_any_element()
    });

    for _ in 0..4 {
        press(cx, "tab");
    }
    press(cx, "enter");
    press(cx, "enter");
    press(cx, "tab");
    press(cx, "enter");

    let view = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.view_year, state.view_month)
    });
    assert_eq!(view, (2026, 10));
    assert_eq!(open_changes.borrow().as_slice(), ["true", "false"]);
}

/// `isDateUnavailable` blocks the date on both input paths without blocking
/// the ring from resting on it. The arrows still move the focus onto an
/// unavailable day (`onFocusChange` fires, as React Aria keeps the focused
/// date navigable), but Enter is stopped by the same `constraints.allows`
/// gate (`calendar.rs`'s key handler) and the cell itself carries no click
/// handler at all. The neighbouring available day answers both.
#[gpui::test]
fn calendar_is_date_unavailable_blocks_selection_not_focus(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let changes = changes.clone();
        // `defaultValue` seeds the selection and the view month, so the grid
        // shows August 2026 whatever day the suite runs on. Day 16 is the
        // only unavailable day in the month, next door to the selected 15th.
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .is_date_unavailable(|date| date.day == 16)
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .on_change(move |date, _, _| {
                let iso = date.map(|d| d.format_iso()).unwrap_or_default();
                changes.borrow_mut().push(iso);
            })
            .into_any_element()
    });

    // Enter on the unavailable day: Right moves the focus there and reports
    // it, but the pick is stopped.
    press(cx, "tab");
    press(cx, "right");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2026-08-16"],
        "the arrows must still move the focus onto an unavailable day"
    );
    press(cx, "enter");
    assert!(
        changed.borrow().is_empty(),
        "Enter on an unavailable day must not select it"
    );

    // One more Right reaches the 17th, which is available: Enter selects it.
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-17"],
        "Enter on the neighbouring available day must select it"
    );

    // The pointer path: an unavailable cell gets no handler at all, so a click
    // records nothing -- not even the focus event a keyboard move fires.
    let (unavail_x, unavail_y) = cal_day(2026, 8, 16);
    click(cx, unavail_x, unavail_y);
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-17"],
        "clicking an unavailable day must not select it"
    );
    assert_eq!(
        focused.borrow().len(),
        2,
        "an unavailable cell must not answer the pointer either"
    );

    let (avail_x, avail_y) = cal_day(2026, 8, 17);
    click(cx, avail_x, avail_y);
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-17", "2026-08-17"],
        "clicking the neighbouring available day must still select it"
    );
}

/// Right from Dec 31 2027 crosses the year boundary into January 2028 and the
/// visible month follows the cursor. End then jumps to Jan 31, and PageDown
/// from there resolves to Feb 29 -- the day `days_in_month` grants a leap
/// February, the exact edge a non-leap year would collapse to the 28th. The
/// leap day is a real day: Enter selects it.
#[gpui::test]
fn calendar_pages_dec_to_jan_and_clamps_into_leap_february(cx: &mut TestAppContext) {
    let focuses = events();
    let focused = focuses.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();

    let cx = open_host(cx, move || {
        let focuses = focuses.clone();
        let changes = changes.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2027, 12, 31))
            .on_focus_change(move |date, _, _| {
                focuses.borrow_mut().push(date.format_iso());
            })
            .on_change(move |date, _, _| {
                let iso = date.map(|d| d.format_iso()).unwrap_or_default();
                changes.borrow_mut().push(iso);
            })
            .into_any_element()
    });

    // Right from Dec 31 2027 crosses into January 2028: the date upshifts the
    // year and the grid follows the cursor.
    press(cx, "tab");
    press(cx, "right");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2028-01-01"],
        "Right across the year boundary must land on Jan 1"
    );
    {
        let (year, month) =
            cx.update(|_, cx| (state.read(cx).view_year, state.read(cx).view_month));
        assert_eq!((year, month), (2028, 1), "the view must follow the cursor");
    }

    // End jumps to the last day of the visible month, Jan 31.
    press(cx, "end");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2028-01-01", "2028-01-31"],
        "End must jump to the last day of the month"
    );

    // PageDown from Jan 31 into the leap February: the 29th, not a clamped
    // 28th and not a Feb 31.
    press(cx, "pagedown");
    assert_eq!(
        focused.borrow().as_slice(),
        ["2028-01-01", "2028-01-31", "2028-02-29"],
        "PageDown must clamp Jan 31 to Feb 29 in a leap year"
    );
    {
        let (year, month) =
            cx.update(|_, cx| (state.read(cx).view_year, state.read(cx).view_month));
        assert_eq!((year, month), (2028, 2), "the view must follow the cursor");
    }

    // The leap day is a real day: Enter selects it.
    press(cx, "enter");
    assert_eq!(
        changed.borrow().as_slice(),
        ["2028-02-29"],
        "Enter must select the leap day"
    );
}

/// Without `allowsNonContiguousRanges`, React Stately turns the last available
/// day before the first unavailable date into a temporary navigation and cell
/// bound. Dates beyond August 7 are disabled rather than clickable shortcuts
/// that silently clamp the range.
#[gpui::test]
fn range_calendar_bounds_cells_and_navigation_at_the_first_unavailable_date(
    cx: &mut TestAppContext,
) {
    let changes = events();
    let changed = changes.clone();
    let selected_cells = Rc::new(RefCell::new(HashMap::new()));
    let selected_probe = selected_cells.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let selected_cells = selected_cells.clone();
        RangeCalendar::new(state_for_view.clone())
            .is_date_unavailable(|date, _| date == Date::new(2026, 8, 7))
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .cell(move |state| {
                if !state.is_outside_month && (6..=10).contains(&state.date.day) {
                    selected_cells.borrow_mut().insert(
                        state.date.day,
                        (state.is_selected, state.is_disabled, state.is_unavailable),
                    );
                }
                gpui::div().size(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    let (start_x, start_y) = range_day(2026, 8, 5);
    click(cx, start_x, start_y);
    cx.update(|window, _| window.refresh());
    assert_eq!(
        selected_probe.borrow().get(&6),
        Some(&(false, false, false)),
        "the last date before the barrier must remain selectable"
    );
    assert_eq!(
        selected_probe.borrow().get(&7),
        Some(&(false, true, true)),
        "the unavailable barrier is also outside the effective range bound"
    );
    assert_eq!(
        selected_probe.borrow().get(&8),
        Some(&(false, true, false)),
        "dates beyond the barrier must be disabled without being marked unavailable"
    );

    click(cx, 238., 12.);
    assert_eq!(
        cx.update(|_, cx| (state.read(cx).view_year, state.read(cx).view_month)),
        (2026, 8),
        "the anchor-derived maximum must disable forward paging"
    );

    let (end_x, end_y) = range_day(2026, 8, 10);
    cx.simulate_mouse_move(
        point(px(end_x), px(end_y)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    cx.update(|window, _| window.refresh());
    assert_eq!(
        selected_probe.borrow().get(&6),
        Some(&(false, false, false)),
        "hovering a disabled target must not create a clamped preview"
    );
    assert_eq!(
        selected_probe.borrow().get(&7),
        Some(&(false, true, true)),
        "the unavailable day must not be part of the preview"
    );
    assert_eq!(
        selected_probe.borrow().get(&8),
        Some(&(false, true, false)),
        "the default preview must not continue beyond the unavailable day"
    );
    click(cx, end_x, end_y);
    assert!(
        changed.borrow().is_empty(),
        "a disabled date beyond the barrier must not commit a clamped range"
    );
    let (last_x, last_y) = range_day(2026, 8, 6);
    click(cx, last_x, last_y);
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-05->2026-08-06"],
        "the last selectable date must complete the bounded range"
    );

    // A completed range starts over on the next click. The backward bound is
    // symmetric: dates before the barrier are inert until the user chooses its
    // last selectable neighbour.
    click(cx, end_x, end_y);
    cx.update(|window, _| window.refresh());
    click(cx, start_x, start_y);
    assert_eq!(changed.borrow().len(), 1);
    let (backward_end_x, backward_end_y) = range_day(2026, 8, 8);
    click(cx, backward_end_x, backward_end_y);

    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-05->2026-08-06", "2026-08-08->2026-08-10",],
        "a backward range must clamp after the first unavailable day"
    );
}

/// React Stately bounds the unavailable-date search to one visible duration.
/// A gap farther away is not promoted into a navigation bound, so paging and a
/// later available endpoint remain valid even in contiguous mode.
#[gpui::test]
fn range_calendar_unavailable_bound_search_stops_after_the_visible_duration(
    cx: &mut TestAppContext,
) {
    let changes = events();
    let changed = changes.clone();
    let endpoint_state = Rc::new(RefCell::new(None));
    let endpoint_probe = endpoint_state.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let endpoint_state = endpoint_state.clone();
        RangeCalendar::new(state_for_view.clone())
            .is_date_unavailable(|date, _| date == Date::new(2026, 9, 10))
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .cell(move |cell| {
                if cell.date == Date::new(2026, 9, 15) && !cell.is_outside_month {
                    *endpoint_state.borrow_mut() = Some(cell.is_disabled);
                }
                gpui::div().size(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    let (anchor_x, anchor_y) = range_day(2026, 8, 5);
    click(cx, anchor_x, anchor_y);
    click(cx, 238., 12.);
    assert_eq!(
        cx.update(|_, cx| (state.read(cx).view_year, state.read(cx).view_month)),
        (2026, 9),
        "an unavailable date beyond the bounded scan must not disable paging"
    );
    cx.update(|window, _| window.refresh());
    assert_eq!(
        *endpoint_probe.borrow(),
        Some(false),
        "a later available endpoint must remain enabled"
    );

    let (end_x, end_y) = range_day(2026, 9, 15);
    click(cx, end_x, end_y);
    assert_eq!(changed.borrow().as_slice(), ["2026-08-05->2026-09-15"]);
}

/// The bounded scan includes one sentinel probe immediately after the visible
/// duration. With an August 5 anchor, September 6 closes the effective range at
/// September 5 even though the barrier itself is one day past the month span.
#[gpui::test]
fn range_calendar_unavailable_sentinel_day_closes_the_effective_bound(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let cell_states = Rc::new(RefCell::new(HashMap::new()));
    let states_probe = cell_states.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let cell_states = cell_states.clone();
        RangeCalendar::new(state_for_view.clone())
            .is_date_unavailable(|date, _| date == Date::new(2026, 9, 6))
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .cell(move |cell| {
                if cell.date.month == 9 && (5..=7).contains(&cell.date.day) {
                    cell_states
                        .borrow_mut()
                        .insert(cell.date.day, (cell.is_disabled, cell.is_unavailable));
                }
                gpui::div().size(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    let (anchor_x, anchor_y) = range_day(2026, 8, 5);
    click(cx, anchor_x, anchor_y);
    click(cx, 238., 12.);
    assert_eq!(
        cx.update(|_, cx| (state.read(cx).view_year, state.read(cx).view_month)),
        (2026, 9)
    );
    cx.update(|window, _| window.refresh());
    assert_eq!(states_probe.borrow().get(&5), Some(&(false, false)));
    assert_eq!(states_probe.borrow().get(&6), Some(&(true, true)));
    assert_eq!(states_probe.borrow().get(&7), Some(&(true, false)));

    let (disabled_x, disabled_y) = range_day(2026, 9, 7);
    click(cx, disabled_x, disabled_y);
    assert!(changed.borrow().is_empty());
    let (last_x, last_y) = range_day(2026, 9, 5);
    click(cx, last_x, last_y);
    assert_eq!(changed.borrow().as_slice(), ["2026-08-05->2026-09-05"]);
}

/// Enabling `allowsNonContiguousRanges` removes only the interior gap
/// constraint. Available endpoints on either side of August 7 may complete the
/// range, while the unavailable day itself remains unavailable as an endpoint.
#[gpui::test]
fn range_calendar_allows_a_gap_but_not_an_unavailable_endpoint(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let unavailable_state = Rc::new(RefCell::new(None));
    let state_probe = unavailable_state.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let unavailable_state = unavailable_state.clone();
        RangeCalendar::new(state_for_view.clone())
            .is_date_unavailable(|date, _| date == Date::new(2026, 8, 7))
            .allows_non_contiguous_ranges(true)
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .cell(move |state| {
                if state.date == Date::new(2026, 8, 7) {
                    *unavailable_state.borrow_mut() =
                        Some((state.is_selected, state.is_unavailable));
                }
                gpui::div().size(px(20.)).into_any_element()
            })
            .into_any_element()
    });

    let (start_x, start_y) = range_day(2026, 8, 5);
    click(cx, start_x, start_y);
    let (end_x, end_y) = range_day(2026, 8, 10);
    click(cx, end_x, end_y);
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-05->2026-08-10"],
        "the enabled mode must allow available endpoints across a gap"
    );
    cx.update(|window, _| window.refresh());
    assert_eq!(
        *state_probe.borrow(),
        Some((false, true)),
        "the unavailable interior date must remain outside the selected cells"
    );

    let (unavailable_x, unavailable_y) = range_day(2026, 8, 7);
    click(cx, unavailable_x, unavailable_y);
    assert_eq!(
        changed.borrow().len(),
        1,
        "an unavailable day must remain unavailable as an endpoint"
    );
}

/// Home can jump straight over both a minimum and an unavailable date. Range
/// resolution must apply both constraints and choose the one nearest the
/// anchor: minimum August 9 beats unavailable August 7 going backward.
#[gpui::test]
fn range_calendar_uses_the_tighter_bound_when_keys_jump(cx: &mut TestAppContext) {
    let backward = events();
    let reported = backward.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let backward = backward.clone();
        RangeCalendar::new(state_for_view.clone())
            .min_value(Date::new(2026, 8, 9))
            .is_date_unavailable(|date, _| date == Date::new(2026, 8, 7))
            .on_change(move |start, end, _, _| {
                backward
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    press(cx, "tab");
    let (anchor_x, anchor_y) = range_day(2026, 8, 10);
    click(cx, anchor_x, anchor_y);
    cx.update(|window, _| window.refresh());
    press(cx, "home");
    press(cx, "enter");
    assert_eq!(
        reported.borrow().as_slice(),
        ["2026-08-09->2026-08-10"],
        "the minimum must clamp before the farther unavailable boundary"
    );
}

/// The forward mirror of the minimum test: End jumps past maximum August 7
/// and unavailable August 9, so the nearer maximum must finish the range.
#[gpui::test]
fn range_calendar_maximum_beats_a_farther_unavailable_date(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    state.update(cx, |state, _| {
        state.view_year = 2026;
        state.view_month = 8;
        state.view_day = 1;
        state.user_navigated = true;
    });
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        RangeCalendar::new(state_for_view.clone())
            .max_value(Date::new(2026, 8, 7))
            .is_date_unavailable(|date, _| date == Date::new(2026, 8, 9))
            .on_change(move |start, end, _, _| {
                changes
                    .borrow_mut()
                    .push(format!("{}->{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    press(cx, "tab");
    let (anchor_x, anchor_y) = range_day(2026, 8, 5);
    click(cx, anchor_x, anchor_y);
    cx.update(|window, _| window.refresh());
    press(cx, "end");
    press(cx, "enter");
    assert_eq!(
        changed.borrow().as_slice(),
        ["2026-08-05->2026-08-07"],
        "the maximum must clamp before the farther unavailable boundary"
    );
}

/// In multiple mode v3's `onChange` value is the whole date array. The legacy
/// single-value callback can only name the activated date; `on_change_all`
/// must report the complete toggled set, including a plural `defaultValue`
/// seed, and it must remove a date when that cell is picked again.
#[gpui::test]
fn calendar_multiple_selection_reports_the_full_date_set(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();

    let cx = open_host(cx, move || {
        let single = changes.clone();
        let all = changes.clone();
        Calendar::new(state_for_view.clone())
            .selection_mode(herogpui_components::SelectionMode::Multiple)
            .default_values([Date::new(2026, 8, 3), Date::new(2026, 8, 5)])
            .on_change(move |date, _, _| {
                single.borrow_mut().push(format!(
                    "one:{}",
                    date.map(|date| date.format_iso()).unwrap_or_default()
                ));
            })
            .on_change_all(move |dates, _, _| {
                all.borrow_mut().push(format!(
                    "all:{}",
                    dates
                        .iter()
                        .map(Date::format_iso)
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            })
            .into_any_element()
    });

    assert_eq!(
        cx.update(|_, cx| state.read(cx).selected_dates().to_vec()),
        [Date::new(2026, 8, 3), Date::new(2026, 8, 5)],
        "the plural defaultValue must seed every selected date"
    );

    let (day5_x, day5_y) = cal_day(2026, 8, 5);
    click(cx, day5_x, day5_y);
    let (day7_x, day7_y) = cal_day(2026, 8, 7);
    click(cx, day7_x, day7_y);
    cx.update(|window, _| window.refresh());
    // A pressed cell keeps the grid's focus scope, so keyboard navigation
    // continues from the pointer-selected day without another Tab.
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(
        changed.borrow().as_slice(),
        [
            "one:2026-08-05",
            "all:2026-08-03",
            "one:2026-08-07",
            "all:2026-08-03,2026-08-07",
            "one:2026-08-08",
            "all:2026-08-03,2026-08-07,2026-08-08",
        ],
        "the full callback must report the set after pointer and keyboard toggles"
    );
}

/// Turns a built selector string into the `&'static str` `debug_bounds`
/// wants; only tests read these, so leaking is fine.
fn leak(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

/// A cell registers its bounds under its element-id key, whose prefix is the
/// component id's Debug form (`NamedInteger("cal", N)`).
fn cal_cell_selector(entity_id: u64, year: i32, month: u32, day: u32) -> String {
    format!(r#"NamedInteger("cal", {entity_id})-{year}-{month}-d{day}"#)
}

/// The cell indicator registers its bounds under the day circle's key plus
/// `-indicator`.
fn cal_indicator_selector(entity_id: u64, year: i32, month: u32, day: u32) -> String {
    format!(r#"NamedInteger("cal", {entity_id})-{year}-{month}-d{day}-indicator"#)
}

/// A pressed cell registers its bounds under its element-id key, whose
/// prefix is the component id's Debug form (`NamedInteger("range-cal", N)`).
fn range_cell_selector(entity_id: u64, year: i32, month: u32, day: u32) -> String {
    format!(r#"NamedInteger("range-cal", {entity_id})-{year}-{month}-day-{day}"#)
}

/// The outer `.range-calendar__cell` around a day registers its bounds under
/// the inner button's key plus `-track`. It carries the range track fill,
/// which must not scale with the pressed inner button.
fn range_track_selector(entity_id: u64, year: i32, month: u32, day: u32) -> String {
    format!(r#"NamedInteger("range-cal", {entity_id})-{year}-{month}-day-{day}-track"#)
}

/// The centre of a probed cell, so the simulated press lands on the cell the
/// frame actually laid out rather than on a derived coordinate.
fn centre_of(bounds: gpui::Bounds<gpui::Pixels>) -> gpui::Point<gpui::Pixels> {
    point(
        px(f32::from(bounds.origin.x) + f32::from(bounds.size.width) / 2.),
        px(f32::from(bounds.origin.y) + f32::from(bounds.size.height) / 2.),
    )
}

/// The pinned pressed state scales the day cell to 0.95 and must still land
/// its click: the pressed background merges with the press geometry in one
/// refinement (`anim::pressed_with_background`), so a mid-press frame shows
/// the shrunken circle and the release selects the day. A chained `.active`
/// after `anim::pressed` would overwrite the whole refinement and leave the
/// cell at its full 36px while pressed -- and a selected cell with no press
/// at all would too, since the pinned CSS presses `bg-accent-hover` under
/// `[data-selected]` as well.
#[gpui::test]
fn calendar_day_press_scales_and_still_selects(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let recorded = events();
    let held = recorded.clone();
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let for_view = held.clone();
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .on_change(move |date, _, _| {
                for_view
                    .borrow_mut()
                    .push(date.expect("a click always carries a date").format_iso());
            })
            .into_any_element()
    });

    // The selected day presses and releases through the accent-hover branch.
    let selected = leak(cal_cell_selector(state.entity_id().as_u64(), 2026, 8, 15));
    let at_rest = cx
        .debug_bounds(selected)
        .expect("the day cell registered its bounds")
        .size;
    assert!(
        (f32::from(at_rest.width) - 36.).abs() < 0.01
            && (f32::from(at_rest.height) - 36.).abs() < 0.01,
        "a resting day cell is a 36px circle, got {at_rest:?}"
    );
    let centre = centre_of(
        cx.debug_bounds(selected)
            .expect("the day cell registered its bounds"),
    );
    cx.simulate_mouse_move(centre, None, Modifiers::none());
    cx.refresh().unwrap();
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    let pressed_bounds = cx
        .debug_bounds(selected)
        .expect("the pressed cell kept its bounds");
    let pressed = pressed_bounds.size;
    assert!(
        (f32::from(pressed.width) - 34.2).abs() < 0.5
            && (f32::from(pressed.height) - 34.2).abs() < 0.5,
        "a pressed day cell scales uniformly to 0.95 (34.2px), got {pressed:?}"
    );
    // The scale is about the centre: every edge moves inward by the same
    // 0.9px, so the pressed centre coincides with the resting centre. A
    // bottom edge that stays put (top-anchored shrink) fails here.
    let rest_centre = centre;
    let pressed_centre = centre_of(pressed_bounds);
    assert!(
        (f32::from(pressed_centre.x) - f32::from(rest_centre.x)).abs() < 0.25
            && (f32::from(pressed_centre.y) - f32::from(rest_centre.y)).abs() < 0.25,
        "a pressed day cell collapses toward its centre, rest {rest_centre:?} pressed {pressed_centre:?} bounds {pressed_bounds:?}"
    );
    let rest_bottom = f32::from(centre.y) + 18.;
    let pressed_bottom = f32::from(pressed_bounds.origin.y) + f32::from(pressed.height);
    assert!(
        (rest_bottom - pressed_bottom - 0.9).abs() < 0.25,
        "the bottom edge rises by 0.9px, rest {rest_bottom} pressed {pressed_bottom}"
    );
    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    let released = cx.debug_bounds(selected).expect("the cell survives").size;
    assert!(
        (f32::from(released.width) - 36.).abs() < 0.01,
        "the cell springs back after the release, got {released:?}"
    );
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2026-08-15"],
        "the release must still select the day"
    );

    // A plain unselected day presses through the bg-default branch the same
    // way.
    let plain = leak(cal_cell_selector(state.entity_id().as_u64(), 2026, 8, 10));
    let centre = centre_of(
        cx.debug_bounds(plain)
            .expect("the plain day cell registered its bounds"),
    );
    cx.simulate_mouse_move(centre, None, Modifiers::none());
    cx.refresh().unwrap();
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    let pressed = cx
        .debug_bounds(plain)
        .expect("the pressed plain cell kept its bounds")
        .size;
    assert!(
        (f32::from(pressed.width) - 34.2).abs() < 0.5
            && (f32::from(pressed.height) - 34.2).abs() < 0.5,
        "a pressed plain day cell scales uniformly to 0.95 (34.2px), got {pressed:?}"
    );
    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2026-08-15", "2026-08-10"],
        "the release must select the plain day through the unchanged handler"
    );
}

/// A nav button registers its bounds under its element-id key, whose prefix is
/// the component id's Debug form (`NamedInteger("cal", N)`). The month header and the
/// week/day header share one `nav_btn` builder, so the same key serves both.
fn cal_nav_selector(entity_id: u64, side: &str) -> String {
    format!(r#"NamedInteger("cal", {entity_id})-{side}"#)
}

/// The pinned `.calendar__nav-button:active` is a bare `transform: scale(0.95)`
/// on top of the hover fill, so a mid-press frame shows the shrunken 24px box
/// and the release still pages the month. The chevron itself keeps its size
/// (gpui 0.2.2 cannot transform an svg), like every `anim::pressed` icon-only
/// control.
#[gpui::test]
fn calendar_nav_press_scales_and_still_pages(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .into_any_element()
    });
    let entity_id = state.entity_id().as_u64();

    for (side, expected) in [
        ("prev", Date::new(2026, 7, 15)),
        ("next", Date::new(2026, 8, 15)),
    ] {
        let selector = leak(cal_nav_selector(entity_id, side));
        let at_rest = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("the {side} nav button registered its bounds"))
            .size;
        assert!(
            (f32::from(at_rest.width) - 24.).abs() < 0.01
                && (f32::from(at_rest.height) - 24.).abs() < 0.01,
            "a resting nav button is a 24px square, got {at_rest:?}"
        );
        let centre = centre_of(
            cx.debug_bounds(selector)
                .unwrap_or_else(|| panic!("the {side} nav button registered its bounds")),
        );
        cx.simulate_mouse_move(centre, None, Modifiers::none());
        cx.refresh().unwrap();
        cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
        cx.refresh().unwrap();
        let pressed = cx
            .debug_bounds(selector)
            .expect("the pressed nav button kept its bounds")
            .size;
        assert!(
            (f32::from(pressed.width) - 22.8).abs() < 0.5
                && (f32::from(pressed.height) - 22.8).abs() < 0.5,
            "a pressed nav button scales uniformly to 0.95 (22.8px), got {pressed:?}"
        );
        cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
        cx.refresh().unwrap();
        let released = cx
            .debug_bounds(selector)
            .expect("the nav button survives")
            .size;
        assert!(
            (f32::from(released.width) - 24.).abs() < 0.01,
            "the nav button springs back after the release, got {released:?}"
        );
        assert_eq!(
            cx.update(|_, cx| state.read(cx).anchor()),
            expected,
            "the {side} release must page the month"
        );
    }
}

/// The week/day header reuses the same `nav_btn` builder, and its next button
/// pages across the December→January boundary. The press must scale there too,
/// and the release must land the year-crossing anchor.
#[gpui::test]
fn calendar_day_view_nav_presses_across_the_year_boundary(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 12, 30))
            .visible_duration(VisibleDuration::Days(3))
            .into_any_element()
    });

    let selector = leak(cal_nav_selector(state.entity_id().as_u64(), "next"));
    let rest = cx
        .debug_bounds(selector)
        .expect("the day view's next button registered its bounds")
        .size;
    let centre = centre_of(
        cx.debug_bounds(selector)
            .expect("the day view's next button registered its bounds"),
    );
    cx.simulate_mouse_move(centre, None, Modifiers::none());
    cx.refresh().unwrap();
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    let pressed = cx
        .debug_bounds(selector)
        .expect("the pressed next button kept its bounds")
        .size;
    // The long range heading squeezes the fixed-width button horizontally at
    // rest; the press leaves that width alone and the height carries the
    // 0.95 scale.
    assert!(
        (f32::from(pressed.height) - 22.8).abs() < 0.5
            && (f32::from(pressed.width) - f32::from(rest.width) * 0.95).abs() < 0.5,
        "the day view's pressed nav button scales uniformly to 0.95, got {rest:?} -> {pressed:?}"
    );
    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    // The selection realigns the window before the first frame, so the paged
    // anchor is the realigned one advanced by the visible day count; what the
    // boundary proves is that the release moved the window into 2027.
    let after = cx.update(|_, cx| state.read(cx).anchor());
    assert_eq!(
        (after.year, after.month),
        (2027, 1),
        "the release must page the three-day window into the next year, got {after:?}"
    );
}

/// A min/max-blocked nav button is disabled: no press geometry and no paging.
/// The disabled button keeps its id (only its handlers are gated), so its
/// bounds stay observable.
#[gpui::test]
fn calendar_disabled_nav_button_neither_presses_nor_pages(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .min_value(Date::new(2026, 8, 10))
            .max_value(Date::new(2026, 8, 20))
            .into_any_element()
    });

    let selector = leak(cal_nav_selector(state.entity_id().as_u64(), "prev"));
    let bounds = cx
        .debug_bounds(selector)
        .expect("the disabled nav button registered its bounds");
    let centre = centre_of(bounds);
    cx.simulate_mouse_move(centre, None, Modifiers::none());
    cx.refresh().unwrap();
    cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    let pressed = cx
        .debug_bounds(selector)
        .expect("the disabled nav button kept its bounds")
        .size;
    assert!(
        (f32::from(pressed.width) - 24.).abs() < 0.01
            && (f32::from(pressed.height) - 24.).abs() < 0.01,
        "a disabled nav button must not scale, got {pressed:?}"
    );
    cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    assert_eq!(
        cx.update(|_, cx| state.read(cx).anchor()),
        Date::new(2026, 8, 15),
        "a disabled nav button must not page"
    );
}

/// The pinned range pressed state scales only the inner
/// `.range-calendar__cell-button` to 0.9 -- caps and the middle of the range
/// alike -- while the outer `.range-calendar__cell` keeps the range track at
/// its full 36px, so a pressed middle cell never breaks the run. The release
/// still drives the anchor/extend pick. A chained `.active` after
/// `anim::pressed` would overwrite the whole refinement and leave the button
/// at its full 36px.
#[gpui::test]
fn range_calendar_press_scales_caps_and_interior_and_still_picks(cx: &mut TestAppContext) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let recorded = events();
    let held = recorded.clone();
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let for_view = held.clone();
        RangeCalendar::new(state_for_view.clone())
            .default_value((Date::new(2026, 8, 10), Date::new(2026, 8, 14)))
            .on_change(move |start, end, _, _| {
                for_view
                    .borrow_mut()
                    .push(format!("{}..{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    let entity_id = state.entity_id().as_u64();
    let cap = leak(range_cell_selector(entity_id, 2026, 8, 10));
    let middle = leak(range_cell_selector(entity_id, 2026, 8, 12));
    for selector in [cap, middle] {
        let at_rest = cx
            .debug_bounds(selector)
            .expect("the range cell registered its bounds")
            .size;
        assert!(
            (f32::from(at_rest.width) - 36.).abs() < 0.01
                && (f32::from(at_rest.height) - 36.).abs() < 0.01,
            "a resting range cell is a 36px square, got {at_rest:?}"
        );
    }

    // The range track is continuous across the middle days: every outer cell
    // keeps the 36px footprint and the neighbours touch with no gap.
    let track11 = leak(range_track_selector(entity_id, 2026, 8, 11));
    let track12 = leak(range_track_selector(entity_id, 2026, 8, 12));
    let track13 = leak(range_track_selector(entity_id, 2026, 8, 13));
    for selector in [track11, track12, track13] {
        let at_rest = cx
            .debug_bounds(selector)
            .expect("the range track registered its bounds")
            .size;
        assert!(
            (f32::from(at_rest.width) - 36.).abs() < 0.01
                && (f32::from(at_rest.height) - 36.).abs() < 0.01,
            "a resting range track is a 36px square, got {at_rest:?}"
        );
    }
    let track11_at_rest = cx
        .debug_bounds(track11)
        .expect("the range track registered its bounds");
    let track12_at_rest = cx
        .debug_bounds(track12)
        .expect("the range track registered its bounds");
    let track13_at_rest = cx
        .debug_bounds(track13)
        .expect("the range track registered its bounds");
    assert!(
        (f32::from(track11_at_rest.origin.x) + 36. - f32::from(track12_at_rest.origin.x)).abs()
            < 0.01
            && (f32::from(track12_at_rest.origin.x) + 36. - f32::from(track13_at_rest.origin.x))
                .abs()
                < 0.01,
        "adjacent middle tracks must touch -- got {track11_at_rest:?}, \
         {track12_at_rest:?}, {track13_at_rest:?}"
    );

    // Press the start cap: the 0.9 scale must show mid-press.
    let (x, y) = range_day(2026, 8, 10);
    cx.simulate_mouse_move(point(px(x), px(y)), None, Modifiers::none());
    cx.refresh().unwrap();
    cx.simulate_mouse_down(point(px(x), px(y)), MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    let pressed = cx
        .debug_bounds(cap)
        .expect("the pressed cap kept its bounds")
        .size;
    assert!(
        (f32::from(pressed.width) - 32.4).abs() < 0.5
            && (f32::from(pressed.height) - 32.4).abs() < 0.5,
        "a pressed range cell scales uniformly to 0.9 (32.4px), got {pressed:?}"
    );
    cx.simulate_mouse_up(point(px(x), px(y)), MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    assert!(
        recorded.borrow().is_empty(),
        "re-picking a cap starts a new anchor; no complete range is published yet"
    );

    // Press a middle cell: the inner button takes the same 0.9 scale while
    // the outer track -- here and on the untouched neighbour -- keeps its
    // full 36px, and the release completes the range through the unchanged
    // click handler.
    let (x, y) = range_day(2026, 8, 12);
    cx.simulate_mouse_move(point(px(x), px(y)), None, Modifiers::none());
    cx.refresh().unwrap();
    cx.simulate_mouse_down(point(px(x), px(y)), MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    let pressed = cx
        .debug_bounds(middle)
        .expect("the pressed middle cell kept its bounds")
        .size;
    assert!(
        (f32::from(pressed.width) - 32.4).abs() < 0.5
            && (f32::from(pressed.height) - 32.4).abs() < 0.5,
        "a pressed middle cell-button scales uniformly to 0.9 (32.4px), got {pressed:?}"
    );
    for (selector, label) in [(track12, "the pressed"), (track11, "the neighbour")] {
        let held_bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{label} track kept its bounds"));
        assert!(
            (f32::from(held_bounds.size.width) - 36.).abs() < 0.01
                && (f32::from(held_bounds.size.height) - 36.).abs() < 0.01,
            "{label} middle track must stay a 36px square while the inner \
             button presses, got {held_bounds:?}"
        );
    }
    cx.simulate_mouse_up(point(px(x), px(y)), MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2026-08-10..2026-08-12"],
        "the release must complete the range"
    );
}

/// A pressed today cell takes the same branch as a pressed middle cell: the
/// inner `.range-calendar__cell-button` scales to 0.9 while the outer cell
/// keeps its 36px footprint, and the release picks today as the range anchor.
/// Today is derived at runtime -- the component and the test both read
/// `Date::today()`, so no wall-clock date is hardcoded.
#[gpui::test]
fn range_calendar_today_press_scales_the_inner_button_and_keeps_the_cell(cx: &mut TestAppContext) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let recorded = events();
    let held = recorded.clone();
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let for_view = held.clone();
        RangeCalendar::new(state_for_view.clone())
            .on_change(move |start, end, _, _| {
                for_view
                    .borrow_mut()
                    .push(format!("{}..{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    // `DateRangeState::new` anchors the view on today, so the today cell is
    // an in-month, unselected cell of the default grid.
    let today = Date::today();
    let entity_id = state.entity_id().as_u64();
    let button = leak(range_cell_selector(
        entity_id,
        today.year,
        today.month,
        today.day,
    ));
    let track = leak(range_track_selector(
        entity_id,
        today.year,
        today.month,
        today.day,
    ));

    let at_rest = cx
        .debug_bounds(button)
        .expect("the today cell registered its bounds")
        .size;
    assert!(
        (f32::from(at_rest.width) - 36.).abs() < 0.01
            && (f32::from(at_rest.height) - 36.).abs() < 0.01,
        "a resting today cell-button is a 36px square, got {at_rest:?}"
    );

    let (x, y) = range_day(today.year, today.month, today.day);
    cx.simulate_mouse_move(point(px(x), px(y)), None, Modifiers::none());
    cx.refresh().unwrap();
    cx.simulate_mouse_down(point(px(x), px(y)), MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    let pressed = cx
        .debug_bounds(button)
        .expect("the pressed today cell kept its bounds")
        .size;
    assert!(
        (f32::from(pressed.width) - 32.4).abs() < 0.5
            && (f32::from(pressed.height) - 32.4).abs() < 0.5,
        "a pressed today cell-button scales uniformly to 0.9 (32.4px), got {pressed:?}"
    );
    let held_cell = cx
        .debug_bounds(track)
        .expect("the outer cell kept its bounds");
    assert!(
        (f32::from(held_cell.size.width) - 36.).abs() < 0.01
            && (f32::from(held_cell.size.height) - 36.).abs() < 0.01,
        "the outer cell must stay a 36px square while the inner button \
         presses, got {held_cell:?}"
    );
    cx.simulate_mouse_up(point(px(x), px(y)), MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();

    let start = cx.update(|_, cx| state.read(cx).start);
    assert_eq!(start, Some(today), "the release must anchor on today");
    assert!(
        recorded.borrow().is_empty(),
        "a first pick anchors; no complete range is published yet"
    );
}

/// `isInvalid` swaps the accent for danger everywhere, the pressed state
/// included: a pressed cap recolours under the same 0.9 inner-button scale,
/// and a pressed middle cell leaves the danger track -- its own and its
/// neighbour's -- at the full 36px. The range derives from `Date::today()`
/// at runtime, so no wall-clock date is hardcoded; days 10-14 always exist
/// inside today's month.
#[gpui::test]
fn range_calendar_invalid_press_scales_the_inner_button_and_keeps_the_track(
    cx: &mut TestAppContext,
) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let recorded = events();
    let held = recorded.clone();
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let for_view = held.clone();
        RangeCalendar::new(state_for_view.clone())
            .is_invalid(true)
            .default_value((
                Date::new(Date::today().year, Date::today().month, 10),
                Date::new(Date::today().year, Date::today().month, 14),
            ))
            .on_change(move |start, end, _, _| {
                for_view
                    .borrow_mut()
                    .push(format!("{}..{}", start.format_iso(), end.format_iso()));
            })
            .into_any_element()
    });

    let month = Date::today();
    let (year, month) = (month.year, month.month);
    let entity_id = state.entity_id().as_u64();
    let cap = leak(range_cell_selector(entity_id, year, month, 10));
    let middle = leak(range_cell_selector(entity_id, year, month, 12));
    let track11 = leak(range_track_selector(entity_id, year, month, 11));
    let track12 = leak(range_track_selector(entity_id, year, month, 12));

    // Press the invalid start cap: the 0.9 scale must show mid-press, and
    // the release re-anchors without publishing a range.
    let (x, y) = range_day(year, month, 10);
    cx.simulate_mouse_move(point(px(x), px(y)), None, Modifiers::none());
    cx.refresh().unwrap();
    cx.simulate_mouse_down(point(px(x), px(y)), MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    let pressed = cx
        .debug_bounds(cap)
        .expect("the pressed invalid cap kept its bounds")
        .size;
    assert!(
        (f32::from(pressed.width) - 32.4).abs() < 0.5
            && (f32::from(pressed.height) - 32.4).abs() < 0.5,
        "a pressed invalid cap scales uniformly to 0.9 (32.4px), got {pressed:?}"
    );
    cx.simulate_mouse_up(point(px(x), px(y)), MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    assert!(
        recorded.borrow().is_empty(),
        "re-picking a cap starts a new anchor; no complete range is published yet"
    );

    // Press an invalid middle cell: the danger track keeps its full 36px
    // here and on the neighbour, the inner button still scales, and the
    // release completes the range through the unchanged handler.
    let (x, y) = range_day(year, month, 12);
    cx.simulate_mouse_move(point(px(x), px(y)), None, Modifiers::none());
    cx.refresh().unwrap();
    cx.simulate_mouse_down(point(px(x), px(y)), MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    let pressed = cx
        .debug_bounds(middle)
        .expect("the pressed invalid middle cell kept its bounds")
        .size;
    assert!(
        (f32::from(pressed.width) - 32.4).abs() < 0.5
            && (f32::from(pressed.height) - 32.4).abs() < 0.5,
        "a pressed invalid middle cell-button scales uniformly to 0.9 (32.4px), got {pressed:?}"
    );
    for (selector, label) in [(track12, "the pressed"), (track11, "the neighbour")] {
        let held_bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{label} invalid track kept its bounds"));
        assert!(
            (f32::from(held_bounds.size.width) - 36.).abs() < 0.01
                && (f32::from(held_bounds.size.height) - 36.).abs() < 0.01,
            "{label} invalid middle track must stay a 36px square while the \
             inner button presses, got {held_bounds:?}"
        );
    }
    cx.simulate_mouse_up(point(px(x), px(y)), MouseButton::Left, Modifiers::none());
    cx.refresh().unwrap();
    assert_eq!(
        recorded.borrow().as_slice(),
        [format!(
            "{}..{}",
            Date::new(year, month, 10).format_iso(),
            Date::new(year, month, 12).format_iso()
        )],
        "the release must complete the range through the danger tokens"
    );
}

/// A range nav button registers its bounds under its element-id key, whose
/// prefix is the component id's Debug form (`NamedInteger("range-cal", N)`).
fn range_nav_selector(entity_id: u64, side: &str) -> String {
    format!(r#"NamedInteger("range-cal", {entity_id})-{side}"#)
}

/// The cell indicator registers its bounds under the inner button's key plus
/// `-indicator`.
fn range_indicator_selector(entity_id: u64, year: i32, month: u32, day: u32) -> String {
    format!(r#"NamedInteger("range-cal", {entity_id})-{year}-{month}-day-{day}-indicator"#)
}

/// The pinned range track runs under the caps too, and it stays continuous
/// across a week boundary: the last column of a row closes the run at the
/// 252px right edge, the first column of the next row reopens it at x = 0
/// exactly one 40px row pitch lower. This geometry fixture explicitly uses a
/// Monday-first grid, so August 2026's five leading blanks put day 16 in the
/// last column of its row and day 17 in the first column of the next one.
#[gpui::test]
fn range_calendar_track_crosses_the_row_boundary_and_caps_carry_it(cx: &mut TestAppContext) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        RangeCalendar::new(state_for_view.clone())
            .default_value((Date::new(2026, 8, 13), Date::new(2026, 8, 20)))
            .first_day_of_week(Weekday::Mon)
            .into_any_element()
    });

    let entity_id = state.entity_id().as_u64();
    let start_cap = leak(range_track_selector(entity_id, 2026, 8, 13));
    let row_last = leak(range_track_selector(entity_id, 2026, 8, 16));
    let row_first = leak(range_track_selector(entity_id, 2026, 8, 17));
    let end_cap = leak(range_track_selector(entity_id, 2026, 8, 20));

    // The caps carry the outer soft track as well: every selected cell's
    // outer box registers its full 36px bounds.
    for (selector, label) in [
        (start_cap, "the start cap"),
        (row_last, "the row's last interior day"),
        (row_first, "the next row's first interior day"),
        (end_cap, "the end cap"),
    ] {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{label} track registered its bounds"));
        assert!(
            (f32::from(bounds.size.width) - 36.).abs() < 0.01
                && (f32::from(bounds.size.height) - 36.).abs() < 0.01,
            "{label} track must be a 36px square, got {bounds:?}"
        );
    }

    let start = cx
        .debug_bounds(start_cap)
        .expect("the start cap registered its bounds");
    let last = cx
        .debug_bounds(row_last)
        .expect("the row's last interior track registered its bounds");
    let first = cx
        .debug_bounds(row_first)
        .expect("the next row's first interior track registered its bounds");
    let end = cx
        .debug_bounds(end_cap)
        .expect("the end cap registered its bounds");

    // Day 16 closes the row at the calendar's right edge; day 17 reopens it
    // at x = 0, one 40px row pitch below.
    assert!(
        (f32::from(last.origin.x) + 36. - f32::from(CALENDAR_WIDTH)).abs() < 0.01,
        "the last column's track must close the row at the 252px edge, got {last:?}"
    );
    assert!(
        f32::from(first.origin.x).abs() < 0.01,
        "the next row's first track must start at x = 0, got {first:?}"
    );
    assert!(
        (f32::from(first.origin.y) - (f32::from(last.origin.y) + 40.)).abs() < 0.01,
        "the track must resume exactly one 40px row pitch below the boundary, \
         got {last:?} then {first:?}"
    );

    // The caps sit where the grid puts them: the start cap three columns
    // left of the row's last interior day, the end cap one row below and
    // three columns right of the boundary -- the run never breaks.
    assert!(
        (f32::from(last.origin.x) - (f32::from(start.origin.x) + 3. * 36.)).abs() < 0.01
            && (f32::from(last.origin.y) - f32::from(start.origin.y)).abs() < 0.01,
        "the start cap must share the row with the last interior day, \
         got {start:?} then {last:?}"
    );
    assert!(
        (f32::from(end.origin.y) - f32::from(first.origin.y)).abs() < 0.01
            && (f32::from(end.origin.x) - (f32::from(first.origin.x) + 3. * 36.)).abs() < 0.01,
        "the end cap must share the next row with the first interior day, \
         got {first:?} then {end:?}"
    );
}

/// The pinned `.range-calendar__cell-indicator` is a 3px dot at `bottom-1` --
/// 4px above the cell's bottom edge, centred in the 36px cell.
#[gpui::test]
fn range_calendar_indicator_sits_four_pixels_above_the_cell_bottom(cx: &mut TestAppContext) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        RangeCalendar::new(state_for_view.clone())
            .default_value((Date::new(2026, 8, 10), Date::new(2026, 8, 14)))
            .cell_indicator(|date| date.month == 8 && date.day == 12)
            .into_any_element()
    });

    let entity_id = state.entity_id().as_u64();
    let track = leak(range_track_selector(entity_id, 2026, 8, 12));
    let indicator = leak(range_indicator_selector(entity_id, 2026, 8, 12));

    let cell = cx
        .debug_bounds(track)
        .expect("the marked cell registered its bounds");
    let dot = cx
        .debug_bounds(indicator)
        .expect("the indicator registered its bounds");
    assert!(
        (f32::from(dot.size.width) - 3.).abs() < 0.01
            && (f32::from(dot.size.height) - 3.).abs() < 0.01,
        "the indicator is a 3px dot, got {dot:?}"
    );
    assert!(
        (f32::from(dot.origin.x) + 1.5 - (f32::from(cell.origin.x) + 18.)).abs() < 0.01,
        "the indicator must be centred in the cell, got {dot:?} in {cell:?}"
    );
    assert!(
        (f32::from(dot.origin.y) + 3. - (f32::from(cell.origin.y) + 36. - 4.)).abs() < 0.01,
        "the indicator must sit 4px (bottom-1) above the cell's bottom edge, \
         got {dot:?} in {cell:?}"
    );
}

/// The pinned `.calendar__cell-indicator` is a 3px dot at `bottom-1` --
/// 4px above the 36px cell's bottom edge, centred in the slot like the
/// RangeCalendar's dot, not hanging 2px low.
#[gpui::test]
fn calendar_indicator_sits_four_pixels_above_the_cell_bottom(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        Calendar::new(state_for_view.clone())
            .default_value(Date::new(2026, 8, 15))
            .cell_indicator(|date| date.month == 8 && date.day == 12)
            .into_any_element()
    });

    let entity_id = state.entity_id().as_u64();
    let circle = leak(cal_cell_selector(entity_id, 2026, 8, 12));
    let indicator = leak(cal_indicator_selector(entity_id, 2026, 8, 12));

    let cell = cx
        .debug_bounds(circle)
        .expect("the marked day circle registered its bounds");
    let dot = cx
        .debug_bounds(indicator)
        .expect("the indicator registered its bounds");
    assert!(
        (f32::from(dot.size.width) - 3.).abs() < 0.01
            && (f32::from(dot.size.height) - 3.).abs() < 0.01,
        "the indicator is a 3px dot, got {dot:?}"
    );
    // The slot's flex alignment centres the dot the same way it centres the
    // 36px day circle: both share the slot's horizontal centre.
    assert!(
        (f32::from(dot.origin.x) + 1.5 - (f32::from(cell.origin.x) + 18.)).abs() < 0.01,
        "the indicator must be centred in the cell, got {dot:?} in {cell:?}"
    );
    assert!(
        (f32::from(dot.origin.y) + 3. - (f32::from(cell.origin.y) + 36. - 4.)).abs() < 0.01,
        "the indicator must sit 4px (bottom-1) above the cell's bottom edge, \
         got {dot:?} in {cell:?}"
    );
}

/// The pinned `.range-calendar__nav-button:active` is a bare
/// `transform: scale(0.95)` on top of the hover fill, so a mid-press frame
/// shows the shrunken 24px box and the release still pages the month. The
/// chevron itself keeps its size (gpui 0.2.2 cannot transform an svg), like
/// every `anim::pressed` icon-only control.
#[gpui::test]
fn range_calendar_nav_press_scales_and_still_pages(cx: &mut TestAppContext) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        RangeCalendar::new(state_for_view.clone()).into_any_element()
    });
    let entity_id = state.entity_id().as_u64();

    // `DateRangeState::new` anchors the view on today, so each press pages
    // from whatever month the view holds at that point.
    let mut expected = (Date::today().year, Date::today().month);
    for (side, step) in [("prev", -1), ("next", 1)] {
        let selector = leak(range_nav_selector(entity_id, side));
        let at_rest = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("the {side} nav button registered its bounds"))
            .size;
        assert!(
            (f32::from(at_rest.width) - 24.).abs() < 0.01
                && (f32::from(at_rest.height) - 24.).abs() < 0.01,
            "a resting nav button is a 24px square, got {at_rest:?}"
        );
        let centre = centre_of(
            cx.debug_bounds(selector)
                .unwrap_or_else(|| panic!("the {side} nav button registered its bounds")),
        );
        cx.simulate_mouse_move(centre, None, Modifiers::none());
        cx.refresh().unwrap();
        cx.simulate_mouse_down(centre, MouseButton::Left, Modifiers::none());
        cx.refresh().unwrap();
        let pressed = cx
            .debug_bounds(selector)
            .expect("the pressed nav button kept its bounds")
            .size;
        assert!(
            (f32::from(pressed.width) - 22.8).abs() < 0.5
                && (f32::from(pressed.height) - 22.8).abs() < 0.5,
            "a pressed nav button scales uniformly to 0.95 (22.8px), got {pressed:?}"
        );
        cx.simulate_mouse_up(centre, MouseButton::Left, Modifiers::none());
        cx.refresh().unwrap();
        let released = cx
            .debug_bounds(selector)
            .expect("the nav button survives")
            .size;
        assert!(
            (f32::from(released.width) - 24.).abs() < 0.01,
            "the nav button springs back after the release, got {released:?}"
        );
        let (view_year, view_month) = cx.update(|_, cx| {
            let state = state.read(cx);
            (state.view_year, state.view_month)
        });
        let stepped = expected.1 as i32 - 1 + step;
        expected = if stepped < 0 {
            (expected.0 - 1, 12)
        } else if stepped > 11 {
            (expected.0 + 1, 1)
        } else {
            (expected.0, stepped as u32 + 1)
        };
        assert_eq!(
            (view_year, view_month),
            expected,
            "the {side} release must page the month"
        );
    }
}
