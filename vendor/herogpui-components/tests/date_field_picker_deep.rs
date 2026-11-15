//! Deep inherited contracts for the v3 date field and picker family.
//!
//! HeroUI 3.2.4 composes React Aria's date controls. Its pinned
//! `react-stately` 3.49.0 keeps an incomplete segmented display separate from
//! the committed value, and its pinned `react-aria` 3.51.0 opens picker groups
//! with Alt+ArrowDown or Alt+ArrowUp.

use crate::harness;

use std::{
    cell::{Cell, RefCell},
    process::Command,
    rc::Rc,
};

use gpui::{prelude::*, px, Focusable, TestAppContext, VisualTestContext};
use herogpui_components::{
    CalendarState, Date, DateConstraints, DateField, DatePicker, DateRangePicker, DateRangeState,
    DateSegment, FieldSegment, Form, Granularity, HourCycle, Input, InputState, TimeSegment,
    ValidationBehavior,
};
use herogpui_theme::ActiveTheme;

use harness::{click, events, focus_date_segment, open_host, press, system_date_order, type_date};

fn refresh(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

#[gpui::test]
fn date_field_delete_keeps_an_incomplete_display_without_committing(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let rendered: Rc<RefCell<Vec<(FieldSegment, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let rendered_for_view = rendered.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "2025-10-15"));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let rendered = rendered_for_view.clone();
        DateField::new(state_for_view.clone())
            .segment(move |segment, text| {
                rendered.borrow_mut().push((segment, text.to_string()));
                gpui::div().child(text).into_any_element()
            })
            .on_change(move |date, _, _| {
                changes
                    .borrow_mut()
                    .push(date.map_or_else(|| "none".to_owned(), |date| date.format_iso()));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "delete");
    refresh(cx);

    assert_eq!(
        cx.update(|_, cx| state.read(cx).value().to_owned()),
        "2025-10-15"
    );
    assert!(
        changed.borrow().is_empty(),
        "clearing one date segment is an incomplete local edit, not a committed value"
    );
    let latest = |segment| {
        rendered
            .borrow()
            .iter()
            .rev()
            .find_map(|(part, text)| (*part == segment).then(|| text.clone()))
            .unwrap()
    };
    let cleared = system_date_order()[0];
    for (segment, value, hint) in [
        (DateSegment::Month, "10", "mm"),
        (DateSegment::Day, "15", "dd"),
        (DateSegment::Year, "2025", "yyyy"),
    ] {
        assert_eq!(
            latest(FieldSegment::Date(segment)),
            if segment == cleared { hint } else { value }
        );
    }

    press(cx, "right");
    press(cx, "delete");
    assert!(changed.borrow().is_empty());
    press(cx, "right");
    press(cx, "delete");

    assert_eq!(cx.update(|_, cx| state.read(cx).value().to_owned()), "");
    assert_eq!(
        changed.borrow().as_slice(),
        ["none"],
        "only clearing every visible segment commits null"
    );
}

#[gpui::test]
fn date_field_leading_zeros_follow_locale_and_force_all_numeric_segments(cx: &mut TestAppContext) {
    const CHILD: &str = "HEROGPUI_DATE_FIELD_LEADING_ZERO_TEST";
    if std::env::var_os(CHILD).is_none() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "date_field_leading_zeros_follow_locale_and_force_all_numeric_segments",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env_remove("LC_ALL")
            .env("LC_TIME", "en_US.UTF-8")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "DateField leading-zero locale child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return;
    }

    let regional: Rc<RefCell<Vec<(FieldSegment, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let forced: Rc<RefCell<Vec<(FieldSegment, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let regional_for_view = regional.clone();
    let forced_for_view = forced.clone();
    let regional_state = cx.new(|cx| InputState::with_value(cx, "2025-02-03T08:05:07"));
    let forced_state = cx.new(|cx| InputState::with_value(cx, "2025-02-03T08:05:07"));
    let cx = open_host(cx, move || {
        let regional = regional_for_view.clone();
        let forced = forced_for_view.clone();
        gpui::div()
            .children([
                DateField::new(regional_state.clone())
                    .granularity(Granularity::Second)
                    .hour_cycle(HourCycle::H12)
                    .segment(move |segment, text| {
                        regional.borrow_mut().push((segment, text.to_string()));
                        gpui::div().child(text).into_any_element()
                    })
                    .into_any_element(),
                DateField::new(forced_state.clone())
                    .granularity(Granularity::Second)
                    .hour_cycle(HourCycle::H12)
                    .should_force_leading_zeros(true)
                    .segment(move |segment, text| {
                        forced.borrow_mut().push((segment, text.to_string()));
                        gpui::div().child(text).into_any_element()
                    })
                    .into_any_element(),
            ])
            .into_any_element()
    });
    refresh(cx);

    let latest = |log: &Rc<RefCell<Vec<(FieldSegment, String)>>>, segment| {
        log.borrow()
            .iter()
            .rev()
            .find_map(|(part, text)| (*part == segment).then(|| text.clone()))
    };
    assert_eq!(
        latest(&regional, FieldSegment::Date(DateSegment::Month)).as_deref(),
        Some("2")
    );
    assert_eq!(
        latest(&regional, FieldSegment::Date(DateSegment::Day)).as_deref(),
        Some("3")
    );
    assert_eq!(
        latest(&regional, FieldSegment::Time(TimeSegment::Hour)).as_deref(),
        Some("8")
    );
    assert_eq!(
        latest(&regional, FieldSegment::Time(TimeSegment::Minute)).as_deref(),
        Some("05")
    );
    assert_eq!(
        latest(&regional, FieldSegment::Time(TimeSegment::Second)).as_deref(),
        Some("07")
    );
    assert_eq!(
        latest(&forced, FieldSegment::Date(DateSegment::Month)).as_deref(),
        Some("02")
    );
    assert_eq!(
        latest(&forced, FieldSegment::Date(DateSegment::Day)).as_deref(),
        Some("03")
    );
    assert_eq!(
        latest(&forced, FieldSegment::Time(TimeSegment::Hour)).as_deref(),
        Some("08")
    );
}

#[gpui::test]
fn date_field_segment_order_follows_system_locale(cx: &mut TestAppContext) {
    let rendered: Rc<RefCell<Vec<FieldSegment>>> = Rc::new(RefCell::new(Vec::new()));
    let rendered_for_view = rendered.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "2025-02-03"));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let rendered = rendered_for_view.clone();
        let changes = changes.clone();
        DateField::new(state_for_view.clone())
            .segment(move |segment, text| {
                rendered.borrow_mut().push(segment);
                gpui::div().child(text).into_any_element()
            })
            .on_change(move |date, _, _| {
                changes
                    .borrow_mut()
                    .push(date.map_or_else(|| "none".to_owned(), |date| date.format_iso()));
            })
            .into_any_element()
    });
    refresh(cx);

    let date_segments: Vec<_> = rendered
        .borrow()
        .iter()
        .copied()
        .filter(|segment| matches!(segment, FieldSegment::Date(_)))
        .collect();
    let observed = &date_segments[date_segments.len() - DateSegment::ALL.len()..];
    let expected = system_date_order().map(FieldSegment::Date);
    assert_eq!(observed, expected);
    let first = observed[0];

    press(cx, "tab");
    press(cx, "up");
    let expected = match first {
        FieldSegment::Date(DateSegment::Day) => "2025-02-04",
        FieldSegment::Date(DateSegment::Month) => "2025-03-03",
        FieldSegment::Date(DateSegment::Year) => "2026-02-03",
        FieldSegment::Time(_) => unreachable!("the regional date order starts with a date segment"),
    };
    assert_eq!(changed.borrow().as_slice(), [expected]);
}

#[gpui::test]
fn date_field_reentry_waits_until_every_segment_is_complete(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "2025-01-15"));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        DateField::new(state_for_view.clone())
            .placeholder_value(Date::new(2025, 1, 15))
            .on_change(move |date, _, _| {
                changes
                    .borrow_mut()
                    .push(date.map_or_else(|| "none".to_owned(), |date| date.format_iso()));
            })
            .into_any_element()
    });

    press(cx, "tab");
    for index in 0..3 {
        press(cx, "delete");
        if index < 2 {
            press(cx, "right");
        }
    }
    assert_eq!(changed.borrow().as_slice(), ["none"]);

    focus_date_segment(cx, DateSegment::Month);
    press(cx, "0");
    press(cx, "1");
    focus_date_segment(cx, DateSegment::Day);
    press(cx, "1");
    press(cx, "5");
    assert_eq!(
        changed.borrow().as_slice(),
        ["none"],
        "two entered segments still leave the date incomplete"
    );
    assert_eq!(cx.update(|_, cx| state.read(cx).value().to_owned()), "");

    focus_date_segment(cx, DateSegment::Year);
    press(cx, "up");
    assert_eq!(changed.borrow().as_slice(), ["none", "2026-01-15"]);
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value().to_owned()),
        "2026-01-15"
    );
}

#[gpui::test]
fn date_field_page_and_bound_keys_follow_react_stately_steps(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "2025-03-31"));
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        DateField::new(state.clone())
            .on_change(move |date, _, _| {
                changes
                    .borrow_mut()
                    .push(date.map_or_else(|| "none".to_owned(), |date| date.format_iso()));
            })
            .into_any_element()
    });

    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Month);
    press(cx, "pageup");
    press(cx, "pagedown");
    focus_date_segment(cx, DateSegment::Day);
    press(cx, "pagedown");
    press(cx, "home");
    press(cx, "end");
    focus_date_segment(cx, DateSegment::Year);
    press(cx, "pageup");
    press(cx, "pagedown");
    press(cx, "home");
    press(cx, "end");

    assert_eq!(
        changed.borrow().as_slice(),
        [
            "2025-05-31",
            "2025-03-31",
            "2025-03-24",
            "2025-03-01",
            "2025-03-31",
            "2030-03-31",
            "2025-03-31",
            "0001-03-31",
            "9999-03-31",
        ]
    );
}

#[gpui::test]
fn date_field_read_only_page_and_bound_keys_are_inert(cx: &mut TestAppContext) {
    let read_only = Rc::new(Cell::new(true));
    let read_only_for_view = read_only.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "2025-03-31"));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        DateField::new(state_for_view.clone())
            .is_read_only(read_only_for_view.get())
            .on_change(move |date, _, _| {
                changes
                    .borrow_mut()
                    .push(date.map_or_else(|| "none".to_owned(), |date| date.format_iso()));
            })
            .into_any_element()
    });

    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Day);
    press(cx, "pageup");
    press(cx, "home");
    press(cx, "end");
    assert!(changed.borrow().is_empty());
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value().to_owned()),
        "2025-03-31"
    );

    read_only.set(false);
    refresh(cx);
    press(cx, "pageup");
    assert_eq!(
        changed.borrow().as_slice(),
        ["2025-03-07"],
        "read-only navigation keeps the day segment focused, whose PageUp step is seven"
    );
}

#[gpui::test]
fn date_picker_alt_open_escape_restore_and_space_reopen(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        DatePicker::new(state_for_view.clone())
            .on_open_change(move |open, _, _| opens.borrow_mut().push(open.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "alt-down");
    assert_eq!(opened.borrow().as_slice(), ["true"]);
    press(cx, "escape");
    assert_eq!(opened.borrow().as_slice(), ["true", "false"]);
    press(cx, "space");
    assert_eq!(
        opened.borrow().as_slice(),
        ["true", "false", "true"],
        "Escape must restore the initiating field so Space reopens directly"
    );
}

#[gpui::test]
fn date_picker_selection_restore_allows_space_reopen(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        DatePicker::new(state_for_view.clone())
            .on_open_change(move |open, _, _| opens.borrow_mut().push(open.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "alt-up");
    press(cx, "enter");
    assert_eq!(
        opened.borrow().as_slice(),
        ["true", "false"],
        "the Enter that completes selection must close once, not refire the trigger"
    );
    press(cx, "space");
    assert_eq!(opened.borrow().as_slice(), ["true", "false", "true"]);
}

#[gpui::test]
fn date_picker_segments_do_not_open_but_the_trigger_does(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 6, 15)));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        DatePicker::new(state_for_view.clone())
            .on_open_change(move |open, _, _| opens.borrow_mut().push(open.to_string()))
            .into_any_element()
    });

    click(cx, 20., 18.);
    assert!(
        opened.borrow().is_empty(),
        "a pointer press on an editable segment focuses it without opening the calendar"
    );
    // Three date segments occupy roughly 82px after the 12px inset; the
    // separate 24px trigger is pinned to the field's right edge.
    click(cx, 295., 18.);
    assert_eq!(opened.borrow().as_slice(), ["true"]);
    press(cx, "escape");
    press(cx, "space");
    assert_eq!(
        opened.borrow().as_slice(),
        ["true", "false", "true"],
        "trigger-initiated dismissal restores the separate trigger"
    );
}

#[gpui::test]
fn date_picker_trigger_is_a_separate_keyboard_stop(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        DatePicker::new(state_for_view.clone())
            .on_open_change(move |open, _, _| opens.borrow_mut().push(open.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(opened.borrow().as_slice(), ["true"]);
    press(cx, "escape");
    press(cx, "space");
    assert_eq!(opened.borrow().as_slice(), ["true", "false", "true"]);
}

#[gpui::test]
fn date_picker_incomplete_segment_survives_a_repaint(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 1, 15)));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        DatePicker::new(state_for_view.clone())
            .on_change(move |date, _, _| {
                changes
                    .borrow_mut()
                    .push(date.map_or_else(|| "none".to_owned(), |date| date.format_iso()));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "delete");
    refresh(cx);
    press(cx, "right");
    press(cx, "delete");
    press(cx, "right");
    press(cx, "delete");

    assert_eq!(changed.borrow().as_slice(), ["none"]);
}

#[gpui::test]
fn date_range_picker_is_a_keyboard_stop_and_opens_with_alt_arrow(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        DateRangePicker::new(state_for_view.clone())
            .on_open_change(move |open, _, _| opens.borrow_mut().push(open.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "alt-up");
    assert_eq!(
        opened.borrow().as_slice(),
        ["true"],
        "Alt+ArrowUp can only reach the picker after its field enters the tab order"
    );
    press(cx, "escape");
    press(cx, "space");
    assert_eq!(opened.borrow().as_slice(), ["true", "false", "true"]);
}

#[gpui::test]
fn date_range_picker_completed_selection_restores_the_initiating_field(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        DateRangePicker::new(state_for_view.clone())
            .on_open_change(move |open, _, _| opens.borrow_mut().push(open.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "alt-down");
    press(cx, "enter");
    assert_eq!(
        opened.borrow().as_slice(),
        ["true"],
        "the first range endpoint keeps the calendar open"
    );
    press(cx, "enter");
    assert_eq!(opened.borrow().as_slice(), ["true", "false"]);
    press(cx, "space");
    assert_eq!(
        opened.borrow().as_slice(),
        ["true", "false", "true"],
        "a completed range restores its initiating field for direct reopening"
    );
}

#[gpui::test]
fn date_range_picker_segments_do_not_open_but_the_trigger_does(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| {
        DateRangeState::with_range(
            cx,
            Some(Date::new(2025, 6, 15)),
            Some(Date::new(2025, 6, 20)),
        )
    });
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        DateRangePicker::new(state_for_view.clone())
            .on_open_change(move |open, _, _| opens.borrow_mut().push(open.to_string()))
            .into_any_element()
    });

    click(cx, 20., 18.);
    assert!(opened.borrow().is_empty());
    click(cx, 300., 18.);
    assert_eq!(opened.borrow().as_slice(), ["true"]);
    press(cx, "escape");
    press(cx, "space");
    assert_eq!(opened.borrow().as_slice(), ["true", "false", "true"]);
}

#[gpui::test]
fn date_picker_field_is_editable_like_the_composed_v3_date_field(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 6, 15)));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DatePicker::new(state_for_view.clone()).into_any_element()
    });

    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Month);
    press(cx, "home");
    assert_eq!(
        cx.update(|_, cx| state.read(cx).selected),
        Some(Date::new(2025, 1, 15))
    );
}

#[gpui::test]
fn date_range_picker_start_field_is_editable(cx: &mut TestAppContext) {
    let state = cx.new(|cx| {
        DateRangeState::with_range(
            cx,
            Some(Date::new(2025, 6, 15)),
            Some(Date::new(2025, 6, 20)),
        )
    });
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DateRangePicker::new(state_for_view.clone()).into_any_element()
    });

    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Month);
    press(cx, "home");
    assert_eq!(
        cx.update(|_, cx| state.read(cx).start),
        Some(Date::new(2025, 1, 15))
    );
}

#[gpui::test]
fn date_range_picker_keeps_start_and_end_fields_distinct(cx: &mut TestAppContext) {
    let state = cx.new(|cx| {
        DateRangeState::with_range(
            cx,
            Some(Date::new(2025, 6, 15)),
            Some(Date::new(2025, 6, 20)),
        )
    });
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DateRangePicker::new(state_for_view.clone()).into_any_element()
    });

    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Month);
    press(cx, "home");
    assert_eq!(
        cx.update(|_, cx| state.read(cx).start),
        Some(Date::new(2025, 1, 15))
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).end),
        Some(Date::new(2025, 6, 20))
    );

    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Month);
    press(cx, "home");
    assert_eq!(
        cx.update(|_, cx| state.read(cx).start),
        Some(Date::new(2025, 1, 15))
    );
    assert_eq!(
        cx.update(|_, cx| state.read(cx).end),
        Some(Date::new(2025, 1, 20))
    );
}

#[gpui::test]
fn date_picker_trigger_toggles_uncontrolled_without_duplicate_outside_close(
    cx: &mut TestAppContext,
) {
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        DatePicker::new(state_for_view.clone())
            .on_open_change(move |open, _, _| opens.borrow_mut().push(open.to_string()))
            .into_any_element()
    });

    click(cx, 295., 18.);
    click(cx, 295., 18.);
    assert_eq!(opened.borrow().as_slice(), ["true", "false"]);

    click(cx, 295., 18.);
    click(cx, 400., 18.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["true", "false", "true", "false"]
    );
}

#[gpui::test]
fn date_picker_trigger_toggles_controlled_open_state(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let is_open = Rc::new(Cell::new(false));
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state;
    let open_for_view = is_open;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        DatePicker::new(state_for_view.clone())
            .is_open(open_for_view.get())
            .on_open_change({
                let open_for_view = open_for_view.clone();
                move |open, _, _| {
                    open_for_view.set(*open);
                    opens.borrow_mut().push(open.to_string());
                }
            })
            .into_any_element()
    });

    click(cx, 295., 18.);
    refresh(cx);
    click(cx, 295., 18.);
    assert_eq!(opened.borrow().as_slice(), ["true", "false"]);
}

#[gpui::test]
fn read_only_pickers_stay_navigable_but_do_not_edit_select_or_open(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let date_state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 6, 15)));
    let range_state = cx.new(|cx| {
        DateRangeState::with_range(
            cx,
            Some(Date::new(2025, 6, 15)),
            Some(Date::new(2025, 6, 20)),
        )
    });
    let date_for_view = date_state.clone();
    let range_for_view = range_state.clone();
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        gpui::div()
            .flex()
            .flex_col()
            .child(
                DatePicker::new(date_for_view.clone())
                    .is_read_only(true)
                    .on_open_change(move |open, _, _| opens.borrow_mut().push(open.to_string())),
            )
            .child(DateRangePicker::new(range_for_view.clone()).is_read_only(true))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "home");
    press(cx, "alt-down");
    click(cx, 295., 18.);
    assert_eq!(
        cx.update(|_, cx| date_state.read(cx).selected),
        Some(Date::new(2025, 6, 15))
    );
    assert!(opened.borrow().is_empty());

    press(cx, "tab");
    press(cx, "home");
    assert_eq!(
        cx.update(|_, cx| {
            let state = range_state.read(cx);
            (state.start, state.end)
        }),
        (Some(Date::new(2025, 6, 15)), Some(Date::new(2025, 6, 20)))
    );
    click(cx, 300., 18.);
    assert!(opened.borrow().is_empty());
}

#[gpui::test]
fn date_range_text_edits_preserve_endpoint_identity_when_crossing(cx: &mut TestAppContext) {
    let state = cx.new(|cx| {
        DateRangeState::with_range(
            cx,
            Some(Date::new(2025, 6, 15)),
            Some(Date::new(2025, 6, 20)),
        )
    });
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DateRangePicker::new(state_for_view.clone()).into_any_element()
    });

    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Day);
    press(cx, "end");
    assert_eq!(
        cx.update(|_, cx| {
            let state = state.read(cx);
            (state.start, state.end)
        }),
        (Some(Date::new(2025, 6, 30)), Some(Date::new(2025, 6, 20)))
    );

    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Month);
    press(cx, "home");
    assert_eq!(
        cx.update(|_, cx| {
            let state = state.read(cx);
            (state.start, state.end)
        }),
        (Some(Date::new(2025, 6, 30)), Some(Date::new(2025, 1, 20)))
    );
}

#[gpui::test]
fn date_picker_invalid_text_keeps_last_valid_calendar_value(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 6, 15)));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DatePicker::new(state_for_view.clone())
            .min_value(Date::new(2025, 6, 10))
            .max_value(Date::new(2025, 6, 20))
            .into_any_element()
    });

    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Year);
    press(cx, "end");
    refresh(cx);
    assert_eq!(
        cx.update(|_, cx| state.read(cx).selected),
        Some(Date::new(2025, 6, 15)),
        "an invalid complete field edit must not replace the last valid picker value"
    );

    press(cx, "2");
    press(cx, "0");
    press(cx, "2");
    press(cx, "5");
    assert_eq!(
        cx.update(|_, cx| state.read(cx).selected),
        Some(Date::new(2025, 6, 15)),
        "the valid correction keeps the unchanged month/day selection"
    );
}

#[gpui::test]
fn date_range_picker_trigger_toggles_without_duplicate_outside_close(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        DateRangePicker::new(state_for_view.clone())
            .on_open_change(move |open, _, _| opens.borrow_mut().push(open.to_string()))
            .into_any_element()
    });

    click(cx, 300., 18.);
    click(cx, 300., 18.);
    click(cx, 300., 18.);
    assert_eq!(opened.borrow().as_slice(), ["true", "false", "true"]);
}

#[gpui::test]
fn date_field_form_reads_live_invalidity_after_render(cx: &mut TestAppContext) {
    let invalids = events();
    let invalids_for_form = invalids.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "2025-06-15"));
    let form_field = DateField::new(state.clone())
        .name("date")
        .form_field()
        .expect("named date field");
    let form = Form::new()
        .field(form_field)
        .on_invalid(move |_, _, _| invalids_for_form.borrow_mut().push("invalid".to_owned()));
    let submit = form.submit_handler();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        DateField::new(state_for_view.clone())
            .min_value(Date::new(2025, 6, 20))
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(invalids.borrow().as_slice(), ["invalid"]);
}

#[gpui::test]
fn date_picker_form_invalid_submit_focuses_its_field(cx: &mut TestAppContext) {
    let invalids = events();
    let invalids_for_form = invalids.clone();
    let state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 6, 15)));
    let picker = DatePicker::new(state.clone()).name("date").is_invalid(true);
    let form_field = cx.update(|cx| picker.form_field(cx).expect("named picker field"));
    let form = Form::new()
        .field(form_field)
        .on_invalid(move |_, _, _| invalids_for_form.borrow_mut().push("invalid".to_owned()));
    let submit = form.submit_handler();
    let picker = Rc::new(RefCell::new(Some(picker)));
    let picker_for_view = picker;
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        picker_for_view
            .borrow_mut()
            .take()
            .unwrap_or_else(|| {
                DatePicker::new(state_for_view.clone())
                    .name("date")
                    .is_invalid(true)
            })
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(invalids.borrow().as_slice(), ["invalid"]);
    focus_date_segment(cx, DateSegment::Month);
    press(cx, "home");
    assert_eq!(
        cx.update(|_, cx| state.read(cx).selected),
        Some(Date::new(2025, 1, 15)),
        "an invalid picker submit focuses the actual DateField"
    );
}

#[gpui::test]
fn required_empty_date_picker_blocks_form_and_focuses_its_field(cx: &mut TestAppContext) {
    let invalids = events();
    let invalids_for_form = invalids.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let picker = DatePicker::new(state.clone())
        .name("date")
        .is_required(true);
    let form_field = cx.update(|cx| picker.form_field(cx).expect("named picker field"));
    let form = Form::new()
        .field(form_field)
        .on_invalid(move |_, _, _| invalids_for_form.borrow_mut().push("invalid".to_owned()));
    let submit = form.submit_handler();
    let picker = Rc::new(RefCell::new(Some(picker)));
    let picker_for_view = picker;
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        picker_for_view
            .borrow_mut()
            .take()
            .unwrap_or_else(|| {
                DatePicker::new(state_for_view.clone())
                    .name("date")
                    .is_required(true)
            })
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(invalids.borrow().as_slice(), ["invalid"]);
    type_date(cx, Date::new(2025, 1, 1));
    assert_eq!(
        cx.update(|_, cx| state.read(cx).selected),
        Some(Date::new(2025, 1, 1)),
        "a required picker submit focuses the actual DateField"
    );
}

#[gpui::test]
fn required_empty_date_range_picker_blocks_form_and_focuses_start(cx: &mut TestAppContext) {
    let invalids = events();
    let invalids_for_form = invalids.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let picker = DateRangePicker::new(state.clone())
        .start_name("start")
        .end_name("end")
        .is_required(true);
    let fields = cx.update(|cx| picker.form_fields(cx));
    let form = fields
        .into_iter()
        .fold(Form::new(), Form::field)
        .on_invalid(move |_, _, _| invalids_for_form.borrow_mut().push("invalid".to_owned()));
    let submit = form.submit_handler();
    let picker = Rc::new(RefCell::new(Some(picker)));
    let picker_for_view = picker;
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        picker_for_view
            .borrow_mut()
            .take()
            .unwrap_or_else(|| {
                DateRangePicker::new(state_for_view.clone())
                    .start_name("start")
                    .end_name("end")
                    .is_required(true)
            })
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(invalids.borrow().as_slice(), ["invalid"]);
    type_date(cx, Date::new(2025, 1, 1));
    assert_eq!(
        cx.update(|_, cx| state.read(cx).start),
        Some(Date::new(2025, 1, 1)),
        "a required range submit focuses the start DateField first"
    );
}

#[gpui::test]
fn date_picker_custom_validation_blocks_native_form_submission(cx: &mut TestAppContext) {
    let invalids = events();
    let invalids_for_form = invalids.clone();
    let selected = Date::new(2025, 6, 15);
    let state = cx.new(|cx| CalendarState::with_selected(cx, selected));
    let picker = DatePicker::new(state.clone())
        .name("date")
        .validate(move |value| {
            (*value == Some(selected)).then(|| "That date is already booked".into())
        });
    let form_field = cx.update(|cx| picker.form_field(cx).expect("named picker field"));
    let form = Form::new()
        .field(form_field)
        .on_invalid(move |_, _, _| invalids_for_form.borrow_mut().push("invalid".to_owned()));
    let submit = form.submit_handler();
    let picker = Rc::new(RefCell::new(Some(picker)));
    let picker_for_view = picker;
    let state_for_view = state;
    let cx = open_host(cx, move || {
        picker_for_view
            .borrow_mut()
            .take()
            .unwrap_or_else(|| {
                DatePicker::new(state_for_view.clone())
                    .name("date")
                    .validate(move |value| {
                        (*value == Some(selected)).then(|| "That date is already booked".into())
                    })
            })
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(invalids.borrow().as_slice(), ["invalid"]);
}

#[gpui::test]
fn date_picker_aria_custom_validation_does_not_block_submission(cx: &mut TestAppContext) {
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let selected = Date::new(2025, 6, 15);
    let state = cx.new(|cx| CalendarState::with_selected(cx, selected));
    let picker = DatePicker::new(state.clone())
        .name("date")
        .validation_behavior(ValidationBehavior::Allow)
        .validate(move |value| {
            (*value == Some(selected)).then(|| "That date is already booked".into())
        });
    let form_field = cx.update(|cx| picker.form_field(cx).expect("named picker field"));
    let form = Form::new().field(form_field).on_submit(move |data, _, _| {
        submitted_for_form
            .borrow_mut()
            .push(data.text("date").unwrap_or_default().to_string());
    });
    let submit = form.submit_handler();
    let picker = Rc::new(RefCell::new(Some(picker)));
    let picker_for_view = picker;
    let state_for_view = state;
    let cx = open_host(cx, move || {
        picker_for_view
            .borrow_mut()
            .take()
            .unwrap_or_else(|| {
                DatePicker::new(state_for_view.clone())
                    .name("date")
                    .validation_behavior(ValidationBehavior::Allow)
                    .validate(move |value| {
                        (*value == Some(selected)).then(|| "That date is already booked".into())
                    })
            })
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), [selected.format_iso()]);
}

#[gpui::test]
fn date_picker_auto_focuses_its_editable_field(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DatePicker::new(state_for_view.clone())
            .auto_focus(true)
            .into_any_element()
    });

    type_date(cx, Date::new(2025, 1, 1));
    assert_eq!(
        cx.update(|_, cx| state.read(cx).selected),
        Some(Date::new(2025, 1, 1))
    );
}

#[gpui::test]
fn date_range_picker_custom_validation_blocks_native_form_submission(cx: &mut TestAppContext) {
    let invalids = events();
    let invalids_for_form = invalids.clone();
    let selected = (Date::new(2025, 6, 15), Date::new(2025, 6, 20));
    let state = cx.new(|cx| DateRangeState::with_range(cx, Some(selected.0), Some(selected.1)));
    let picker = DateRangePicker::new(state.clone())
        .start_name("start")
        .end_name("end")
        .validate(move |value| (*value == Some(selected)).then(|| "That range is booked".into()));
    let fields = cx.update(|cx| picker.form_fields(cx));
    let form = fields
        .into_iter()
        .fold(Form::new(), Form::field)
        .on_invalid(move |_, _, _| invalids_for_form.borrow_mut().push("invalid".to_owned()));
    let submit = form.submit_handler();
    let picker = Rc::new(RefCell::new(Some(picker)));
    let picker_for_view = picker;
    let state_for_view = state;
    let cx = open_host(cx, move || {
        picker_for_view
            .borrow_mut()
            .take()
            .unwrap_or_else(|| {
                DateRangePicker::new(state_for_view.clone())
                    .start_name("start")
                    .end_name("end")
                    .validate(move |value| {
                        (*value == Some(selected)).then(|| "That range is booked".into())
                    })
            })
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(invalids.borrow().as_slice(), ["invalid"]);
}

#[gpui::test]
fn date_range_picker_aria_custom_validation_does_not_block_submission(cx: &mut TestAppContext) {
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let selected = (Date::new(2025, 6, 15), Date::new(2025, 6, 20));
    let state = cx.new(|cx| DateRangeState::with_range(cx, Some(selected.0), Some(selected.1)));
    let picker = DateRangePicker::new(state.clone())
        .start_name("start")
        .end_name("end")
        .validation_behavior(ValidationBehavior::Allow)
        .validate(move |value| (*value == Some(selected)).then(|| "That range is booked".into()));
    let fields = cx.update(|cx| picker.form_fields(cx));
    let form = fields
        .into_iter()
        .fold(Form::new(), Form::field)
        .on_submit(move |data, _, _| {
            submitted_for_form.borrow_mut().push(format!(
                "{}:{}",
                data.text("start").unwrap_or_default(),
                data.text("end").unwrap_or_default()
            ));
        });
    let submit = form.submit_handler();
    let picker = Rc::new(RefCell::new(Some(picker)));
    let picker_for_view = picker;
    let state_for_view = state;
    let cx = open_host(cx, move || {
        picker_for_view
            .borrow_mut()
            .take()
            .unwrap_or_else(|| {
                DateRangePicker::new(state_for_view.clone())
                    .start_name("start")
                    .end_name("end")
                    .validation_behavior(ValidationBehavior::Allow)
                    .validate(move |value| {
                        (*value == Some(selected)).then(|| "That range is booked".into())
                    })
            })
            .into_any_element()
    });

    cx.update(|window, cx| submit(window, cx));
    assert_eq!(
        submitted.borrow().as_slice(),
        [format!(
            "{}:{}",
            selected.0.format_iso(),
            selected.1.format_iso()
        )]
    );
}

#[gpui::test]
fn date_range_picker_auto_focuses_its_start_field(cx: &mut TestAppContext) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DateRangePicker::new(state_for_view.clone())
            .auto_focus(true)
            .into_any_element()
    });

    type_date(cx, Date::new(2025, 1, 1));
    assert_eq!(
        cx.update(|_, cx| state.read(cx).start),
        Some(Date::new(2025, 1, 1))
    );
}

#[gpui::test]
fn date_picker_reset_restores_display_and_validity_after_repaint(cx: &mut TestAppContext) {
    let invalids = events();
    let invalids_for_form = invalids.clone();
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let default = Date::new(2025, 6, 15);
    let state = cx.new(|cx| CalendarState::with_selected(cx, default));
    let picker = DatePicker::new(state.clone())
        .name("date")
        .default_value(default)
        .min_value(Date::new(2025, 6, 10))
        .max_value(Date::new(2025, 6, 20));
    let form_field = cx.update(|cx| picker.form_field(cx).expect("named picker field"));
    let form = Form::new()
        .field(form_field)
        .on_submit(move |data, _, _| {
            submitted_for_form
                .borrow_mut()
                .push(data.text("date").unwrap_or_default().to_string());
        })
        .on_invalid(move |_, _, _| invalids_for_form.borrow_mut().push("invalid".to_owned()));
    let submit = form.submit_handler();
    let reset = form.reset_handler();
    let picker = Rc::new(RefCell::new(Some(picker)));
    let picker_for_view = picker;
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        picker_for_view
            .borrow_mut()
            .take()
            .unwrap_or_else(|| {
                DatePicker::new(state_for_view.clone())
                    .name("date")
                    .default_value(default)
                    .min_value(Date::new(2025, 6, 10))
                    .max_value(Date::new(2025, 6, 20))
            })
            .into_any_element()
    });

    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Year);
    press(cx, "end");
    refresh(cx);
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(invalids.borrow().as_slice(), ["invalid"]);

    invalids.borrow_mut().clear();
    cx.update(|window, cx| reset(window, cx));
    refresh(cx);
    cx.update(|window, cx| submit(window, cx));
    assert!(invalids.borrow().is_empty());
    assert_eq!(submitted.borrow().as_slice(), [default.format_iso()]);
    assert_eq!(cx.update(|_, cx| state.read(cx).selected), Some(default));
}

#[gpui::test]
fn date_picker_invalid_form_data_uses_the_displayed_text(cx: &mut TestAppContext) {
    let invalids = events();
    let recorded = invalids.clone();
    let selected = Date::new(2025, 6, 15);
    let state = cx.new(|cx| CalendarState::with_selected(cx, selected));
    let picker = DatePicker::new(state.clone())
        .name("date")
        .min_value(Date::new(2025, 6, 10))
        .max_value(Date::new(2025, 6, 20));
    let field = cx.update(|cx| picker.form_field(cx).expect("named picker field"));
    let form = Form::new().field(field).on_invalid(move |data, _, _| {
        recorded
            .borrow_mut()
            .push(data.text("date").unwrap_or_default().to_string());
    });
    let submit = form.submit_handler();
    let picker = Rc::new(RefCell::new(Some(picker)));
    let picker_for_view = picker;
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        picker_for_view
            .borrow_mut()
            .take()
            .unwrap_or_else(|| {
                DatePicker::new(state_for_view.clone())
                    .name("date")
                    .min_value(Date::new(2025, 6, 10))
                    .max_value(Date::new(2025, 6, 20))
            })
            .into_any_element()
    });

    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Year);
    press(cx, "end");
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(invalids.borrow().as_slice(), ["9999-06-15"]);
    assert_eq!(cx.update(|_, cx| state.read(cx).selected), Some(selected));
}

#[gpui::test]
fn date_range_picker_end_only_field_resets_after_invalid_text_and_repaint(cx: &mut TestAppContext) {
    let invalids = events();
    let invalids_for_form = invalids.clone();
    let submitted = events();
    let submitted_for_form = submitted.clone();
    let default = (Date::new(2025, 6, 10), Date::new(2025, 6, 20));
    let state = cx.new(|cx| DateRangeState::with_range(cx, Some(default.0), Some(default.1)));
    let picker = DateRangePicker::new(state.clone())
        .end_name("end")
        .default_value(default)
        .min_value(Date::new(2025, 6, 1))
        .max_value(Date::new(2025, 6, 30));
    let fields = cx.update(|cx| picker.form_fields(cx));
    assert_eq!(fields.len(), 1);
    let form = Form::new()
        .field(fields.into_iter().next().unwrap())
        .on_submit(move |data, _, _| {
            submitted_for_form
                .borrow_mut()
                .push(data.text("end").unwrap_or_default().to_string());
        })
        .on_invalid(move |data, _, _| {
            invalids_for_form
                .borrow_mut()
                .push(data.text("end").unwrap_or_default().to_string());
        });
    let submit = form.submit_handler();
    let reset = form.reset_handler();
    let picker = Rc::new(RefCell::new(Some(picker)));
    let picker_for_view = picker;
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        picker_for_view
            .borrow_mut()
            .take()
            .unwrap_or_else(|| {
                DateRangePicker::new(state_for_view.clone())
                    .end_name("end")
                    .default_value(default)
                    .min_value(Date::new(2025, 6, 1))
                    .max_value(Date::new(2025, 6, 30))
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Year);
    press(cx, "end");
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(invalids.borrow().as_slice(), ["9999-06-20"]);

    cx.update(|window, cx| reset(window, cx));
    refresh(cx);
    cx.update(|window, cx| submit(window, cx));
    assert_eq!(submitted.borrow().as_slice(), [default.1.format_iso()]);
    assert_eq!(
        cx.update(|_, cx| {
            let state = state.read(cx);
            (state.start, state.end)
        }),
        (Some(default.0), Some(default.1))
    );
}

// ---------------------------------------------------------------------------
// `value` — the pure builder seed on the date picker family
// ---------------------------------------------------------------------------

#[gpui::test]
fn date_picker_value_builder_seeds_the_bound_state_at_first_render(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    // The builder chain needs no `&mut App`: the date travels on the struct
    // and lands in the state when the picker first renders.
    let cx = open_host(cx, move || {
        DatePicker::new(state_for_view.clone())
            .value(Some(Date::new(2026, 1, 15)))
            .into_any_element()
    });
    let selected = cx.update(|_, cx| state.read(cx).selected);
    assert_eq!(
        selected,
        Some(Date::new(2026, 1, 15)),
        "value must seed the bound CalendarState when the picker first renders"
    );
}

#[gpui::test]
fn date_picker_value_builder_keeps_the_last_call_and_outranks_default_value(
    cx: &mut TestAppContext,
) {
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DatePicker::new(state_for_view.clone())
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
fn date_picker_value_builder_seeds_once_and_never_clobbers_edits(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| CalendarState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        DatePicker::new(state_for_view.clone())
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
    // seed must not rewrite a date the owner wrote into their own entity.
    cx.update(|_, cx| {
        state.update(cx, |s, cx| {
            s.selected = Some(Date::new(2026, 2, 3));
            s.selected_dates = vec![Date::new(2026, 2, 3)];
            cx.notify();
        });
    });
    refresh(cx);
    assert_eq!(
        cx.update(|_, cx| state.read(cx).selected),
        Some(Date::new(2026, 2, 3)),
        "a re-render must not re-apply the value seed over the entity's date"
    );
    assert!(
        recorded.borrow().is_empty(),
        "the seed and its non-re-application report nothing through onChange"
    );
}

#[gpui::test]
fn date_range_picker_value_builder_seeds_the_bound_state_at_first_render(cx: &mut TestAppContext) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DateRangePicker::new(state_for_view.clone())
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
        "value must seed the bound DateRangeState when the picker first renders"
    );
}

#[gpui::test]
fn date_range_picker_value_builder_keeps_the_last_call_and_outranks_default_value(
    cx: &mut TestAppContext,
) {
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DateRangePicker::new(state_for_view.clone())
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
fn date_range_picker_value_builder_seeds_once_and_never_clobbers_edits(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| DateRangeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let range = state_for_view.clone();
        DateRangePicker::new(state_for_view.clone())
            .value(Some(Date::new(2026, 1, 10)), Some(Date::new(2026, 1, 20)))
            .on_change(move |_, cx| {
                let (start, end) = {
                    let st = range.read(cx);
                    (st.start, st.end)
                };
                changes.borrow_mut().push(format!(
                    "{}->{}",
                    start.map_or_else(|| "-".into(), |d| d.format_iso()),
                    end.map_or_else(|| "-".into(), |d| d.format_iso())
                ));
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
    // seed must not restore a range the owner replaced in their own entity.
    cx.update(|_, cx| {
        state.update(cx, |st, cx| {
            st.start = Some(Date::new(2026, 2, 3));
            st.end = Some(Date::new(2026, 2, 9));
            cx.notify();
        });
    });
    refresh(cx);
    let (start, end) = cx.update(|_, cx| {
        let st = state.read(cx);
        (st.start, st.end)
    });
    assert_eq!(
        (start, end),
        (Some(Date::new(2026, 2, 3)), Some(Date::new(2026, 2, 9))),
        "a re-render must not re-apply the value seed over the entity's range"
    );
    assert!(
        recorded.borrow().is_empty(),
        "the seed and its non-re-application report nothing through onChange"
    );
}

#[gpui::test]
fn date_field_value_builder_seeds_the_bound_state_at_first_render(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    // The builder chain needs no `&mut App`: the date travels on the struct
    // and lands in the text state as ISO when the field first renders.
    let cx = open_host(cx, move || {
        DateField::new(state_for_view.clone())
            .value(Some(Date::new(2026, 1, 15)))
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        seeded, "2026-01-15",
        "value must seed the bound InputState's ISO text at first render"
    );
}

#[gpui::test]
fn date_field_value_builder_keeps_the_last_call_and_outranks_default_value(
    cx: &mut TestAppContext,
) {
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DateField::new(state_for_view.clone())
            .default_value(Date::new(2026, 2, 2))
            .value(Some(Date::new(2026, 3, 3)))
            .value(Some(Date::new(2026, 1, 15)))
            .into_any_element()
    });
    let seeded = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        seeded, "2026-01-15",
        "a pure builder keeps the last call, and the controlled spelling outranks defaultValue"
    );
}

#[gpui::test]
fn date_field_value_builder_seeds_once_and_never_clobbers_edits(cx: &mut TestAppContext) {
    let rendered: Rc<RefCell<Vec<(FieldSegment, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let rendered_for_view = rendered.clone();
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| InputState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let rendered = rendered_for_view.clone();
        let changes = changes.clone();
        DateField::new(state_for_view.clone())
            .value(Some(Date::new(2026, 1, 15)))
            .segment(move |segment, text| {
                rendered.borrow_mut().push((segment, text.to_string()));
                gpui::div().child(text).into_any_element()
            })
            .on_change(move |date, _, _| {
                changes
                    .borrow_mut()
                    .push(date.map_or_else(|| "none".to_owned(), |d| d.format_iso()));
            })
            .into_any_element()
    });
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value().to_owned()),
        "2026-01-15"
    );

    // The builder runs again on every refresh carrying the same `value`; the
    // seed must not restore the segment the user cleared. Deleting the first
    // regional segment is an incomplete display edit, so nothing commits.
    press(cx, "tab");
    press(cx, "delete");
    refresh(cx);
    let cleared = system_date_order()[0];
    let latest = |segment| {
        rendered
            .borrow()
            .iter()
            .rev()
            .find_map(|(part, text)| (*part == segment).then(|| text.clone()))
            .unwrap()
    };
    let (month, day, year) = (
        latest(FieldSegment::Date(DateSegment::Month)),
        latest(FieldSegment::Date(DateSegment::Day)),
        latest(FieldSegment::Date(DateSegment::Year)),
    );
    let still_cleared = match cleared {
        DateSegment::Month => month == "mm",
        DateSegment::Day => day == "dd",
        DateSegment::Year => year == "yyyy",
    };
    assert!(
        still_cleared,
        "a re-render must not re-apply the value seed over the user's segment edit \
         (cleared {cleared:?}: {month}/{day}/{year})"
    );
    assert!(
        recorded.borrow().is_empty(),
        "the incomplete segment edit defers onChange, which the re-render must not force"
    );
}

/// `.date-input-group__suffix` is `shrink-0 me-3`: the suffix pins to the
/// field's end, not trailing the last segment. The group's `px` supplies the
/// inset, so on a wide field the suffix's right edge sits the padding (plus
/// the border) inside the group's right edge.
#[gpui::test]
fn date_field_suffix_pins_to_the_fields_end(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::with_value(cx, "2025-10-15"));
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(400.))
            .child(
                DateField::new(state.clone())
                    .full_width(true)
                    .suffix(gpui::div().w(px(24.)).h(px(24.)).bg(gpui::red())),
            )
            .into_any_element()
    });
    harness::settle(cx);

    let scene = harness::painted(cx);
    let (field_bg, border) = cx.update(|_, cx| {
        use herogpui_theme::ActiveTheme;
        (
            cx.colors().field.background,
            f32::from(cx.layout().field_border_width),
        )
    });
    // The group's fill quad is the only fill wider than the 320px cap; the
    // suffix marker is the small red quad.
    let group = scene
        .filled(field_bg)
        .into_iter()
        .find(|q| f32::from(scene.bounds(q).size.width) > 300.)
        .expect("the field group must paint its fill");
    let suffix = scene
        .filled(gpui::red())
        .into_iter()
        .next()
        .expect("the suffix marker must paint");
    let group_right =
        f32::from(scene.bounds(group).origin.x) + f32::from(scene.bounds(group).size.width);
    let suffix_right =
        f32::from(scene.bounds(suffix).origin.x) + f32::from(scene.bounds(suffix).size.width);
    let inset = group_right - suffix_right;
    let expected = 12. + border;
    assert!(
        (inset - expected).abs() < 1.5,
        "the suffix must sit at the field's end: inset {inset} vs expected {expected}\n{}",
        scene.describe()
    );
}

#[gpui::test]
fn date_field_native_validate_is_gated_until_blur_commits(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::with_value(cx, "2025-10-15"));
    let state_for_view = state;
    let other = cx.new(|cx| InputState::new(cx));
    let plain = other.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(gpui::div().debug_selector(|| "df".to_owned()).child(
                DateField::new(state_for_view.clone()).validate(|_| Some("Always invalid".into())),
            ))
            .child(
                gpui::div()
                    .debug_selector(|| "other".to_owned())
                    .child(Input::new(plain.clone()).label("Other")),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let height = |cx: &mut VisualTestContext| {
        f32::from(
            cx.debug_bounds("df")
                .expect("date field paints")
                .size
                .height,
        )
    };
    let rest = height(cx);
    // Focus the field and change the parsed date via a segment key: still
    // focused, so nothing commits and the gated error stays hidden.
    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Year);
    press(cx, "up");
    harness::wait_ms(cx, 800);
    assert_eq!(
        height(cx),
        rest,
        "editing a segment while focused must not commit the gated validation"
    );
    // Leaving the field after the change commits the snapshot.
    cx.update(|window, cx| {
        let handle = other.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    harness::wait_ms(cx, 800);
    assert!(
        height(cx) > rest,
        "blurring after a change must commit the gated validation and show the error"
    );
}

#[gpui::test]
fn date_field_incomplete_segment_blur_commits_nothing(cx: &mut TestAppContext) {
    // Fix B: blur compares the parsed date value, not the segment display. A
    // single cleared segment is an incomplete local edit — the underlying ISO
    // text and parsed date are unchanged, so blurring commits nothing.
    let state = cx.new(|cx| InputState::with_value(cx, "2025-10-15"));
    let state_for_view = state;
    let other = cx.new(|cx| InputState::new(cx));
    let plain = other.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(gpui::div().debug_selector(|| "df".to_owned()).child(
                DateField::new(state_for_view.clone()).validate(|_| Some("Always invalid".into())),
            ))
            .child(
                gpui::div()
                    .debug_selector(|| "other".to_owned())
                    .child(Input::new(plain.clone()).label("Other")),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let height = |cx: &mut VisualTestContext| {
        f32::from(
            cx.debug_bounds("df")
                .expect("date field paints")
                .size
                .height,
        )
    };
    let rest = height(cx);
    press(cx, "tab");
    press(cx, "delete");
    cx.update(|window, cx| {
        let handle = other.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    harness::wait_ms(cx, 800);
    assert_eq!(
        height(cx),
        rest,
        "an incomplete segment edit leaves the parsed date unchanged, so blur must not commit"
    );
}

#[gpui::test]
fn date_field_allow_validate_shows_live_at_rest(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::with_value(cx, "2025-10-15"));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        gpui::div()
            .debug_selector(|| "df".to_owned())
            .child(
                DateField::new(state_for_view.clone())
                    .validation_behavior(ValidationBehavior::Allow)
                    .validate(|_| Some("Always invalid".into())),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    let height = f32::from(
        cx.debug_bounds("df")
            .expect("date field paints")
            .size
            .height,
    );
    assert!(
        height > px(36.).into(),
        "Allow must paint the live validation error at rest"
    );
}

/// The pickers forward only the invalid *flag* to their chrome (custom
/// `validate` text is never painted — a pre-existing gap), so the gated state
/// is observed through the danger border/outline. The outline animates in on
/// the border-color track; GPUI animations only advance on
/// `simulate_next_frame`, which `harness::settle`/`wait_ms` never call, so
/// drain the animation frames before reading the painted scene.
fn pending_frames(cx: &mut VisualTestContext) -> usize {
    cx.update(|window, cx| window.simulate_next_frame(cx))
}

fn settle_frames(cx: &mut VisualTestContext) {
    for _ in 0..200 {
        if pending_frames(cx) == 0 {
            return;
        }
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(10));
    }
}

/// `painted_quads` reflects the last frame `Window::draw` produced; neither
/// `refresh` nor `simulate_next_frame` draws, so force one explicitly after
/// the animation clock has settled.
fn draw_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
}

/// The pickers forward only the invalid *flag* to their chrome (custom
/// `validate` text is never painted — a pre-existing gap), so the gated state
/// is observed through the danger border. A *blurred* invalid field paints the
/// border as a quad; a focused one paints a rasterised ring the pinned GPUI
/// never exposes, so callers must move focus off the field first.
fn danger_quads(cx: &mut VisualTestContext) -> usize {
    settle_frames(cx);
    draw_frame(cx);
    let danger = cx.update(|_window, cx| cx.colors().danger.color);
    let (quads, scale) = cx.update(|window, _| (window.painted_quads(), window.scale_factor()));
    let scene = harness::Painted { quads, scale };
    scene
        .quads
        .iter()
        .filter(|q| {
            let reddish = |c: gpui::Hsla| {
                c.a > 0.3 && c.s > 0.3 && {
                    let dh = (c.h - danger.h).abs();
                    dh.min(1.0 - dh) < 0.05 && (c.s - danger.s).abs() < 0.4
                }
            };
            reddish(q.border_color) || q.background.as_solid().is_some_and(reddish)
        })
        .count()
}

fn cal_cell_selector(entity_id: u64, year: i32, month: u32, day: u32) -> String {
    format!(r#"NamedInteger("cal", {entity_id})-{year}-{month}-d{day}"#)
}

fn range_cell_selector(entity_id: u64, year: i32, month: u32, day: u32) -> String {
    format!(r#"NamedInteger("range-cal", {entity_id})-{year}-{month}-day-{day}"#)
}

fn click_cell(cx: &mut VisualTestContext, key: String) {
    let bounds = cx
        .debug_bounds(Box::leak(key.into_boxed_str()))
        .expect("calendar day cell paints");
    click(
        cx,
        f32::from(bounds.center().x),
        f32::from(bounds.center().y),
    );
    cx.run_until_parked();
}

#[gpui::test]
fn date_picker_native_validate_is_gated_until_a_calendar_pick_commits(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 6, 15)));
    let id = state.entity_id().as_u64();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        DatePicker::new(state_for_view.clone())
            .validate(|_| Some("Always invalid".into()))
            .into_any_element()
    });
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    assert_eq!(
        danger_quads(cx),
        0,
        "a failing `validate` must not paint danger chrome at rest"
    );
    // Open the calendar and pick a different day: the pick commits both the
    // picker and inner-field snapshots, surfacing the gated invalid chrome.
    press(cx, "tab");
    press(cx, "alt-down");
    cx.run_until_parked();
    click_cell(cx, cal_cell_selector(id, 2025, 6, 20));
    // Close the calendar and move focus off the field: a *focused* invalid
    // field paints a rasterised ring (invisible to `painted_quads`), while a
    // blurred one paints the danger border quad this test reads.
    press(cx, "escape");
    click(cx, 8., 320.);
    harness::wait_ms(cx, 800);
    assert!(
        danger_quads(cx) > 0,
        "a calendar pick must commit the gated validation and paint the danger chrome"
    );
}

#[gpui::test]
fn date_picker_calendar_pick_replaces_rejected_typed_text(cx: &mut TestAppContext) {
    // Type a date the picker rejects (a day past `max_value`): it parses, so
    // the field shows it, but `selected` does not move — leaving the field
    // "dirty" (`field_value != last synced`). Upstream the field and calendar
    // share one value, so a later calendar pick must replace that stale text.
    // Observed through the danger chrome: stale rejected text stays invalid
    // (danger paints), while the picked in-range date clears it.
    let state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 6, 15)));
    let id = state.entity_id().as_u64();
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DatePicker::new(state_for_view.clone())
            .min_value(Date::new(2025, 6, 10))
            .max_value(Date::new(2025, 6, 20))
            .into_any_element()
    });
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Day);
    press(cx, "2");
    press(cx, "5");
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    assert_eq!(
        cx.update(|_, cx| state.read(cx).selected),
        Some(Date::new(2025, 6, 15)),
        "a rejected typed date must not move the selection"
    );
    // Open the calendar on the still-selected June and pick an in-range day.
    press(cx, "alt-down");
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    settle_frames(cx);
    draw_frame(cx);
    cx.run_until_parked();
    click_cell(cx, cal_cell_selector(id, 2025, 6, 18));
    assert_eq!(
        cx.update(|_, cx| state.read(cx).selected),
        Some(Date::new(2025, 6, 18)),
        "the calendar pick must update the selection"
    );
    // The pick commits both snapshots; blur so a still-invalid field paints
    // its danger border as a quad rather than a focused ring.
    press(cx, "escape");
    click(cx, 8., 320.);
    harness::wait_ms(cx, 800);
    assert_eq!(
        danger_quads(cx),
        0,
        "a calendar pick must replace the stale rejected field text"
    );
}

#[gpui::test]
fn date_picker_controlled_invalid_shows_danger_at_rest(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 6, 15)));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        DatePicker::new(state_for_view.clone())
            .is_invalid(true)
            .into_any_element()
    });
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    assert!(
        danger_quads(cx) > 0,
        "a controlled `is_invalid` must paint the danger chrome at rest"
    );
}

#[gpui::test]
fn date_picker_allow_validate_shows_danger_at_rest(cx: &mut TestAppContext) {
    let state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(2025, 6, 15)));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        DatePicker::new(state_for_view.clone())
            .validation_behavior(ValidationBehavior::Allow)
            .validate(|_| Some("Always invalid".into()))
            .into_any_element()
    });
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    assert!(
        danger_quads(cx) > 0,
        "Allow must paint the live invalid chrome at rest"
    );
}

#[gpui::test]
fn date_range_picker_commits_only_on_a_completed_range(cx: &mut TestAppContext) {
    let state = cx.new(|cx| {
        DateRangeState::with_range(
            cx,
            Some(Date::new(2025, 6, 15)),
            Some(Date::new(2025, 6, 20)),
        )
    });
    let id = state.entity_id().as_u64();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        DateRangePicker::new(state_for_view.clone())
            .validate(|_| Some("Always invalid".into()))
            .into_any_element()
    });
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    assert_eq!(
        danger_quads(cx),
        0,
        "a failing `validate` must not paint danger chrome at rest"
    );
    // Open the range calendar and pick only the start: an incomplete range is
    // not a commit trigger, so no danger chrome appears.
    press(cx, "tab");
    press(cx, "alt-down");
    cx.run_until_parked();
    click_cell(cx, range_cell_selector(id, 2025, 6, 10));
    harness::wait_ms(cx, 800);
    assert_eq!(
        danger_quads(cx),
        0,
        "the first click of a range (start only) must not commit the gated validation"
    );
    // Completing the range commits all four snapshots and paints the chrome.
    // Move focus off the fields so the invalid chrome is the border quad.
    click_cell(cx, range_cell_selector(id, 2025, 6, 25));
    press(cx, "escape");
    click(cx, 8., 320.);
    harness::wait_ms(cx, 800);
    assert!(
        danger_quads(cx) > 0,
        "completing the range must commit the gated validation and paint the danger chrome"
    );
}

#[gpui::test]
fn date_range_picker_calendar_pick_replaces_rejected_start_text(cx: &mut TestAppContext) {
    // Same defect as the single picker, per field: type a start date the picker
    // rejects (a day before `min_value`) so the start field is "dirty" while
    // `range.start` does not move; completing a range in the calendar must
    // replace that stale start text. Observed through the danger chrome —
    // stale rejected text stays invalid (danger), the picked start clears it.
    let state = cx.new(|cx| {
        DateRangeState::with_range(
            cx,
            Some(Date::new(2025, 6, 15)),
            Some(Date::new(2025, 6, 20)),
        )
    });
    let id = state.entity_id().as_u64();
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        DateRangePicker::new(state_for_view.clone())
            .min_value(Date::new(2025, 6, 10))
            .max_value(Date::new(2025, 6, 28))
            .into_any_element()
    });
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    // Focus the start field and type a day before `min_value`: it parses but is
    // rejected, leaving the field dirty while `range.start` stays at the 15th.
    press(cx, "tab");
    focus_date_segment(cx, DateSegment::Day);
    press(cx, "0");
    press(cx, "5");
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    assert_eq!(
        cx.update(|_, cx| state.read(cx).start),
        Some(Date::new(2025, 6, 15)),
        "a rejected typed start must not move the range start"
    );
    // Open the range calendar and complete a new in-range range.
    press(cx, "alt-down");
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    settle_frames(cx);
    draw_frame(cx);
    cx.run_until_parked();
    click_cell(cx, range_cell_selector(id, 2025, 6, 12));
    click_cell(cx, range_cell_selector(id, 2025, 6, 18));
    assert_eq!(
        cx.update(|_, cx| {
            let st = state.read(cx);
            (st.start, st.end)
        }),
        (Some(Date::new(2025, 6, 12)), Some(Date::new(2025, 6, 18))),
        "the calendar picks must set the completed range"
    );
    // Blur so a still-invalid start field paints its danger border as a quad.
    press(cx, "escape");
    click(cx, 8., 320.);
    harness::wait_ms(cx, 800);
    assert_eq!(
        danger_quads(cx),
        0,
        "a completed range pick must replace the stale rejected start text"
    );
}

#[gpui::test]
fn date_picker_valid_calendar_pick_clears_a_committed_field_error(cx: &mut TestAppContext) {
    // The inner DateField is embedded inside the picker, so it never paints
    // constraint text — but its danger chrome still follows the snapshot the
    // picker's `queue_commit_display` commits on a calendar pick. Seed an
    // out-of-range value and submit the form to commit the error into both
    // snapshots without dirtying the field (a typed edit would mark it dirty
    // and stop it following the selection), then pick an in-range date: the
    // pick must clear the committed danger chrome too.
    let state = cx.new(|cx| CalendarState::with_selected(cx, Date::new(202, 6, 20)));
    let id = state.entity_id().as_u64();
    let constraints = DateConstraints {
        min_value: Some(Date::new(202, 6, 26)),
        max_value: Some(Date::new(202, 7, 5)),
        ..Default::default()
    };
    let picker = DatePicker::new(state.clone())
        .constraints(constraints.clone())
        .name("date");
    let form_field = cx.update(|cx| picker.form_field(cx).expect("named picker field"));
    let form = Form::new().field(form_field);
    let submit = form.submit_handler();
    let picker = Rc::new(RefCell::new(Some(picker)));
    let picker_for_view = picker;
    let state_for_view = state;
    let cx = open_host(cx, move || {
        picker_for_view
            .borrow_mut()
            .take()
            .unwrap_or_else(|| {
                DatePicker::new(state_for_view.clone())
                    .constraints(constraints.clone())
                    .name("date")
            })
            .into_any_element()
    });
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    // Submit commits the gated constraint error into the picker and inner-field
    // snapshots; the seeded value stays clean so it still follows the calendar.
    // An invalid submit focuses the field, so blur it: a focused invalid field
    // paints a rasterised ring `painted_quads` cannot see, a blurred one a
    // danger border quad.
    cx.update(|window, cx| submit(window, cx));
    press(cx, "escape");
    click(cx, 8., 320.);
    harness::wait_ms(cx, 800);
    assert!(
        danger_quads(cx) > 0,
        "a committed out-of-range value must paint the danger chrome"
    );
    // Focus the field and open the calendar, then pick an in-range date.
    press(cx, "tab");
    press(cx, "alt-down");
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    settle_frames(cx);
    draw_frame(cx);
    cx.run_until_parked();
    click_cell(cx, cal_cell_selector(id, 202, 6, 28));
    press(cx, "escape");
    click(cx, 8., 320.);
    harness::wait_ms(cx, 800);
    assert_eq!(
        danger_quads(cx),
        0,
        "a valid calendar pick must clear the committed field error's danger chrome"
    );
}
