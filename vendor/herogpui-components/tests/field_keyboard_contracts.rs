//! Keyboard and pointer contracts inherited by HeroUI v3's field family.
//!
//! The prop tables only say that these fields are read-only or clearable. The
//! behaviour comes from HeroUI v3.2.4's exact dependencies: `react-aria`
//! 3.51.0 and `react-stately` 3.49.0. In those versions, a read-only date
//! segment keeps `tabIndex = 0`, `useSpinButton` disables editing but not
//! segment-to-segment focus, SearchField's Escape shortcut clears a non-empty
//! value and calls `onClear`, and `onClear` otherwise belongs only to the clear
//! affordance. NumberField only wires Home/End when a corresponding bound is
//! present.
//!
//! Geometry is derived from the implementation. NumberField is 220px wide
//! with 40px stepper cells, so the decrement/increment centres are (20, 18)
//! and (200, 18).

use crate::harness;

use std::cell::{Cell, RefCell};
use std::process::Command;
use std::rc::Rc;

use gpui::{
    prelude::*, px, Context, ElementId, Entity, Focusable, Render, TestAppContext,
    VisualTestContext, Window,
};
use herogpui_components::{
    DateField, DateSegment, HourCycle, Input, InputState, NumberField, NumberState, SearchField,
    Time, TimeField, TimeGranularity, TimeSegment, TimeState,
};
use herogpui_theme::ThemeProvider;

use harness::{click, events, focus_date_segment, open_host, press, Events};

fn refresh(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

struct ControlledTimeHost {
    state: Entity<TimeState>,
    controlled: Rc<Cell<Option<Time>>>,
    changes: Events,
    rendered: Rc<RefCell<Vec<(TimeSegment, String)>>>,
}

impl Render for ControlledTimeHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let root = window
            .use_keyed_state(
                ElementId::Name("controlled-time-root".into()),
                cx,
                |_, cx| cx.focus_handle(),
            )
            .read(cx)
            .clone();
        if !root.contains_focused(window, cx) {
            window.focus(&root, cx);
        }

        let changes = self.changes.clone();
        let rendered = self.rendered.clone();
        gpui::div()
            .track_focus(&root)
            .on_key_down(|event, window, cx| {
                if event.keystroke.key == "tab" {
                    window.focus_next(cx);
                }
            })
            .size_full()
            .child(
                TimeField::new(self.state.clone())
                    .hour_cycle(HourCycle::H24)
                    .value(self.controlled.get())
                    .segment(move |segment, text| {
                        rendered.borrow_mut().push((segment, text.to_string()));
                        gpui::div().child(text).into_any_element()
                    })
                    .on_change(move |time, _, _| {
                        changes.borrow_mut().push(time.map_or_else(
                            || "none".to_owned(),
                            |time| format!("{:02}:{:02}", time.hour, time.minute),
                        ));
                    }),
            )
    }
}

fn open_controlled_time(
    cx: &mut TestAppContext,
    state: Entity<TimeState>,
    controlled: Rc<Cell<Option<Time>>>,
    changes: Events,
    rendered: Rc<RefCell<Vec<(TimeSegment, String)>>>,
) -> &mut VisualTestContext {
    cx.update(ThemeProvider::init);
    let (_view, cx) = cx.add_window_view(|_, _| ControlledTimeHost {
        state,
        controlled,
        changes,
        rendered,
    });
    cx
}

#[gpui::test]
fn search_field_escape_clears_nonempty_value_and_reports_clear(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let clears = events();
    let cleared = clears.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "rust"));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let clears = clears.clone();
        SearchField::new(state_for_view.clone())
            .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .on_clear(move |_, _| clears.borrow_mut().push("clear".to_owned()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "escape");

    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        (value, changed.borrow().clone(), cleared.borrow().clone()),
        (String::new(), vec![String::new()], vec!["clear".to_owned()]),
        "react-aria 3.51.0's SearchField Escape shortcut must clear a non-empty value, report the empty change, and invoke onClear once"
    );
}

#[gpui::test]
fn search_field_deleting_final_character_does_not_report_clear(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let clears = events();
    let cleared = clears.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "x"));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let clears = clears.clone();
        SearchField::new(state_for_view.clone())
            .on_change(move |text, _, _| changes.borrow_mut().push(text.to_owned()))
            .on_clear(move |_, _| clears.borrow_mut().push("clear".to_owned()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "backspace");

    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(value, "", "Backspace must still delete the last character");
    assert_eq!(changed.borrow().as_slice(), [""]);
    assert!(
        cleared.borrow().is_empty(),
        "react-aria 3.51.0 calls onClear from Escape or the clear button, not from an ordinary edit whose result happens to be empty"
    );
}

#[gpui::test]
fn date_field_read_only_stays_focusable_and_navigable_without_editing(cx: &mut TestAppContext) {
    let read_only = Rc::new(Cell::new(true));
    let read_only_for_view = read_only.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "2025-01-15"));
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
    let focused = cx.update(|window, cx| state.read(cx).focus_handle(cx).is_focused(window));
    assert!(
        focused,
        "react-aria 3.51.0 leaves read-only date segments in the tab order"
    );

    // Segment navigation remains active. The editing keys must do nothing
    // while the field is read-only.
    focus_date_segment(cx, DateSegment::Day);
    press(cx, "up");
    press(cx, "9");
    press(cx, "delete");
    assert!(changed.borrow().is_empty());
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(value, "2025-01-15");

    // Make the same surface editable without changing its focus or cursor.
    // Up now proves that read-only navigation moved the active segment to Day.
    read_only.set(false);
    refresh(cx);
    press(cx, "up");
    assert_eq!(
        changed.borrow().as_slice(),
        ["2025-01-16"],
        "read-only Left/Right must navigate between segments even though Up, digits and Delete cannot edit"
    );
}

#[gpui::test]
fn time_field_read_only_stays_focusable_and_navigable_without_editing(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 30)));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        TimeField::new(state_for_view.clone())
            .hour_cycle(HourCycle::H24)
            .is_read_only(true)
            .on_change(move |time, _, _| {
                changes.borrow_mut().push(time.map_or_else(
                    || "none".to_owned(),
                    |time| format!("{:02}:{:02}", time.hour, time.minute),
                ));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "up");
    press(cx, "9");
    press(cx, "delete");

    let snapshot = cx.update(|_, cx| {
        let state = state.read(cx);
        (state.focused, state.value)
    });
    assert_eq!(snapshot, (TimeSegment::Minute, Some(Time::new(9, 30))));
    assert!(
        changed.borrow().is_empty(),
        "read-only navigation may move the active segment but must not emit a value change"
    );
}

#[gpui::test]
fn time_field_default_cycle_follows_the_system_time_locale(cx: &mut TestAppContext) {
    const CHILD: &str = "HEROGPUI_TIME_LOCALE_TEST";
    if std::env::var_os(CHILD).is_none() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "time_field_default_cycle_follows_the_system_time_locale",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env_remove("LC_ALL")
            .env("LC_TIME", "en_US.UTF-8")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "12-hour locale child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return;
    }

    assert_eq!(HourCycle::default(), HourCycle::H12);
    let rendered: Rc<RefCell<Vec<(TimeSegment, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let forced: Rc<RefCell<Vec<(TimeSegment, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let rendered_for_view = rendered.clone();
    let forced_for_view = forced.clone();
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 5).with_second(7)));
    let forced_state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 5).with_second(7)));
    let cx = open_host(cx, move || {
        let rendered = rendered_for_view.clone();
        let forced = forced_for_view.clone();
        gpui::div()
            .children([
                TimeField::new(state.clone())
                    .show_seconds(true)
                    .segment(move |segment, text| {
                        rendered.borrow_mut().push((segment, text.to_string()));
                        gpui::div().child(text).into_any_element()
                    })
                    .into_any_element(),
                TimeField::new(forced_state.clone())
                    .show_seconds(true)
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

    let latest = |log: &Rc<RefCell<Vec<(TimeSegment, String)>>>, segment| {
        log.borrow()
            .iter()
            .rev()
            .find_map(|(part, text)| (*part == segment).then(|| text.clone()))
    };
    assert_eq!(latest(&rendered, TimeSegment::Hour).as_deref(), Some("9"));
    assert_eq!(
        latest(&rendered, TimeSegment::Minute).as_deref(),
        Some("05")
    );
    assert_eq!(
        latest(&rendered, TimeSegment::Second).as_deref(),
        Some("07")
    );
    assert_eq!(
        latest(&rendered, TimeSegment::Meridiem).as_deref(),
        Some("AM")
    );
    assert_eq!(latest(&forced, TimeSegment::Hour).as_deref(), Some("09"));
    assert_eq!(latest(&forced, TimeSegment::Minute).as_deref(), Some("05"));
}

#[gpui::test]
fn number_field_read_only_stepper_clicks_are_inert(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| NumberState::new(cx, 5.0));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        NumberField::new(state_for_view.clone())
            .is_read_only(true)
            .on_change(move |value, _, _| changes.borrow_mut().push(value.to_string()))
            .into_any_element()
    });

    click(cx, 20., 18.);
    click(cx, 200., 18.);

    let value = cx.update(|_, cx| state.read(cx).value().to_string());
    assert_eq!(value, "5");
    assert!(
        changed.borrow().is_empty(),
        "react-stately 3.49.0 makes canIncrement/canDecrement false when a NumberField is read-only"
    );
}

#[gpui::test]
fn number_field_unbounded_home_and_end_are_noops(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| NumberState::new(cx, 42.0));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        NumberField::new(state_for_view.clone())
            .on_change(move |value, _, _| changes.borrow_mut().push(value.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "home");
    press(cx, "end");

    let value = cx.update(|_, cx| state.read(cx).value().to_string());
    assert_eq!(value, "42");
    assert!(
        changed.borrow().is_empty(),
        "react-stately 3.49.0's decrementToMin/incrementToMax do nothing when the matching bound is absent"
    );
}

#[gpui::test]
fn number_field_explicit_extreme_bounds_still_answer_home_and_end(cx: &mut TestAppContext) {
    let changes: Rc<RefCell<Vec<u64>>> = Rc::new(RefCell::new(Vec::new()));
    let changed = changes.clone();
    let state = cx.new(|cx| NumberState::new(cx, 42.0));
    state.update(cx, |state, _| state.set_range(f64::MIN, f64::MAX));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        NumberField::new(state_for_view.clone())
            .on_change(move |value, _, _| changes.borrow_mut().push(value.to_bits()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "home");
    let minimum = cx.update(|_, cx| state.read(cx).value().to_bits());
    press(cx, "end");
    let maximum = cx.update(|_, cx| state.read(cx).value().to_bits());

    assert_eq!(
        (minimum, maximum),
        (f64::MIN.to_bits(), f64::MAX.to_bits()),
        "explicit extreme bounds are values, not the absence of bounds"
    );
    assert_eq!(
        changed.borrow().as_slice(),
        [f64::MIN.to_bits(), f64::MAX.to_bits()]
    );
}

#[gpui::test]
fn date_field_has_no_pointer_stepper_affordance(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| InputState::with_value(cx, "2025-01-15"));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        DateField::new(state_for_view.clone())
            .on_change(move |date, _, _| {
                changes
                    .borrow_mut()
                    .push(date.map_or_else(|| "none".to_owned(), |date| date.format_iso()));
            })
            .into_any_element()
    });

    click(cx, 132., 11.);
    click(cx, 132., 25.);

    assert_eq!(
        cx.update(|_, cx| state.read(cx).value().to_owned()),
        "2025-01-15"
    );
    assert!(
        changed.borrow().is_empty(),
        "v3 DateField has no pointer steppers after its segmented input"
    );
}

#[gpui::test]
fn time_field_delete_clears_only_the_active_segment_and_defers_change(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let validations = Rc::new(RefCell::new(Vec::new()));
    let validated = validations.clone();
    let rendered: Rc<RefCell<Vec<(TimeSegment, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let rendered_for_view = rendered.clone();
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 30)));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let validations = validations.clone();
        let rendered = rendered_for_view.clone();
        TimeField::new(state_for_view.clone())
            .hour_cycle(HourCycle::H24)
            .segment(move |segment, text| {
                rendered.borrow_mut().push((segment, text.to_string()));
                gpui::div().child(text).into_any_element()
            })
            .validate(move |value| {
                validations.borrow_mut().push(value.is_none());
                None
            })
            .on_change(move |time, _, _| {
                changes.borrow_mut().push(time.map_or_else(
                    || "none".to_owned(),
                    |time| format!("{:02}:{:02}", time.hour, time.minute),
                ));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "delete");
    refresh(cx);

    let latest = |segment| {
        rendered
            .borrow()
            .iter()
            .rev()
            .find_map(|(part, text)| (*part == segment).then(|| text.clone()))
            .unwrap()
    };
    assert_eq!(
        (latest(TimeSegment::Hour), latest(TimeSegment::Minute)),
        ("09".to_owned(), "--".to_owned())
    );
    let value = cx.update(|_, cx| state.read(cx).value);
    assert_eq!(
        value,
        Some(Time::new(9, 30)),
        "a partial display edit must retain the last committed form value"
    );
    assert_eq!(
        validated.borrow().last(),
        Some(&false),
        "validation must keep receiving the committed time while only one segment is empty"
    );
    assert!(
        changed.borrow().is_empty(),
        "react-stately 3.49.0 keeps an incomplete display override and defers onChange until the value is complete or blurred"
    );

    press(cx, "left");
    press(cx, "delete");
    refresh(cx);
    assert_eq!(cx.update(|_, cx| state.read(cx).value), None);
    assert_eq!(validated.borrow().last(), Some(&true));
    assert_eq!(
        changed.borrow().as_slice(),
        ["none"],
        "clearing every visible segment commits null and reports it once"
    );
}

#[gpui::test]
fn time_field_value_builder_seeds_the_bound_state_at_first_render(cx: &mut TestAppContext) {
    let state = cx.new(|cx| TimeState::new(cx));
    let state_for_view = state.clone();
    // The builder chain needs no `&mut App`: the controlled prop travels on
    // the struct and lands in the state when the field first renders.
    let _unrendered = TimeField::new(state.clone()).value(Some(Time::new(7, 45)));
    assert_eq!(
        cx.update(|cx| state.read(cx).value),
        None,
        "a pure builder must not touch the bound state before rendering"
    );
    let cx = open_host(cx, move || {
        TimeField::new(state_for_view.clone())
            .value(Some(Time::new(7, 45)))
            .into_any_element()
    });
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value),
        Some(Time::new(7, 45)),
        "value must sync the bound TimeState when the field first renders"
    );
}

#[gpui::test]
fn time_field_value_builder_keeps_the_last_call_and_outranks_default_value(
    cx: &mut TestAppContext,
) {
    let state = cx.new(|cx| TimeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        TimeField::new(state_for_view.clone())
            .default_value(Time::new(9, 30))
            .value(Some(Time::new(8, 15)))
            .value(Some(Time::new(7, 45)))
            .into_any_element()
    });
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value),
        Some(Time::new(7, 45)),
        "a pure builder keeps the last call, and the controlled spelling outranks defaultValue"
    );
}

#[gpui::test]
fn time_field_value_builder_leaves_the_edited_display_alone_across_renders(
    cx: &mut TestAppContext,
) {
    let rendered: Rc<RefCell<Vec<(TimeSegment, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let rendered_for_view = rendered.clone();
    let changes = events();
    let recorded = changes.clone();
    let state = cx.new(|cx| TimeState::new(cx));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let rendered = rendered_for_view.clone();
        let changes = changes.clone();
        TimeField::new(state_for_view.clone())
            .hour_cycle(HourCycle::H24)
            .value(Some(Time::new(7, 45)))
            .segment(move |segment, text| {
                rendered.borrow_mut().push((segment, text.to_string()));
                gpui::div().child(text).into_any_element()
            })
            .on_change(move |time, _, _| {
                changes.borrow_mut().push(time.map_or_else(
                    || "none".to_owned(),
                    |time| format!("{:02}:{:02}", time.hour, time.minute),
                ));
            })
            .into_any_element()
    });
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value),
        Some(Time::new(7, 45))
    );

    // The builder closure re-runs on every refresh carrying the same `value`;
    // the sync must not fire on it, so the segment the user cleared stays
    // cleared instead of snapping back to the seeded time.
    press(cx, "tab");
    press(cx, "delete");
    refresh(cx);
    let latest = |segment| {
        rendered
            .borrow()
            .iter()
            .rev()
            .find_map(|(part, text)| (*part == segment).then(|| text.clone()))
            .unwrap()
    };
    assert_eq!(
        (latest(TimeSegment::Hour), latest(TimeSegment::Minute)),
        ("--".to_owned(), "45".to_owned()),
        "a re-render must not re-apply the value seed over the user's segment edit"
    );
    assert!(
        recorded.borrow().is_empty(),
        "the cleared segment is an incomplete edit and defers onChange, which the re-render must not force"
    );
}

#[gpui::test]
fn time_field_direct_public_value_mutation_updates_complete_display(cx: &mut TestAppContext) {
    let rendered: Rc<RefCell<Vec<(TimeSegment, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let rendered_for_view = rendered.clone();
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 30)));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let rendered = rendered_for_view.clone();
        TimeField::new(state_for_view.clone())
            .hour_cycle(HourCycle::H24)
            .segment(move |segment, text| {
                rendered.borrow_mut().push((segment, text.to_string()));
                gpui::div().child(text).into_any_element()
            })
            .into_any_element()
    });

    cx.update(|_, cx| {
        state.update(cx, |state, cx| {
            state.value = Some(Time::new(7, 45));
            cx.notify();
        });
    });
    refresh(cx);

    let latest = |segment| {
        rendered
            .borrow()
            .iter()
            .rev()
            .find_map(|(part, text)| (*part == segment).then(|| text.clone()))
            .unwrap()
    };
    assert_eq!(
        (latest(TimeSegment::Hour), latest(TimeSegment::Minute)),
        ("07".to_owned(), "45".to_owned()),
        "direct updates to the public committed value must remain visible when no edit is incomplete"
    );
}

#[gpui::test]
fn time_field_deleting_every_visible_segment_reports_null(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let validations = Rc::new(RefCell::new(Vec::new()));
    let validated = validations.clone();
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 30)));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let validations = validations.clone();
        TimeField::new(state_for_view.clone())
            .hour_cycle(HourCycle::H24)
            .validate(move |value| {
                validations.borrow_mut().push(value.is_none());
                None
            })
            .on_change(move |time, _, _| {
                changes.borrow_mut().push(time.map_or_else(
                    || "none".to_owned(),
                    |time| format!("{:02}:{:02}", time.hour, time.minute),
                ));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "delete");
    assert!(changed.borrow().is_empty(), "one missing segment is local");
    press(cx, "right");
    press(cx, "delete");
    refresh(cx);

    let value = cx.update(|_, cx| state.read(cx).value);
    assert_eq!(value, None, "all visible segments empty the field value");
    assert_eq!(changed.borrow().as_slice(), ["none"]);
    assert_eq!(
        validated.borrow().last(),
        Some(&true),
        "validation must receive None after the whole visible value is cleared"
    );
}

#[gpui::test]
fn time_field_reentry_commits_only_after_every_visible_segment_is_complete(
    cx: &mut TestAppContext,
) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 30)));
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        TimeField::new(state_for_view.clone())
            .hour_cycle(HourCycle::H24)
            .on_change(move |time, _, _| {
                changes.borrow_mut().push(time.map_or_else(
                    || "none".to_owned(),
                    |time| format!("{:02}:{:02}", time.hour, time.minute),
                ));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "delete");
    press(cx, "right");
    press(cx, "delete");
    assert_eq!(changed.borrow().as_slice(), ["none"]);

    press(cx, "left");
    press(cx, "up");
    let partial = cx.update(|_, cx| state.read(cx).value);
    assert_eq!(partial, None, "one entered segment is still incomplete");
    assert_eq!(changed.borrow().as_slice(), ["none"]);

    press(cx, "right");
    press(cx, "up");
    assert_eq!(changed.borrow().as_slice(), ["none", "01:01"]);
    let complete = cx.update(|_, cx| state.read(cx).value);
    assert_eq!(complete, Some(Time::new(1, 1)));
}

#[gpui::test]
fn time_field_empty_keyboard_seed_respects_min_value(cx: &mut TestAppContext) {
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| TimeState::new(cx));
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        TimeField::new(state.clone())
            .hour_cycle(HourCycle::H24)
            .min_value(Time::new(9, 0))
            .on_change(move |time, _, _| {
                changes.borrow_mut().push(time.map_or_else(
                    || "none".to_owned(),
                    |time| format!("{:02}:{:02}", time.hour, time.minute),
                ));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "up");
    press(cx, "right");
    press(cx, "up");
    assert_eq!(changed.borrow().as_slice(), ["09:01"]);
}

#[gpui::test]
fn time_field_controlled_some_restores_after_clear_all(cx: &mut TestAppContext) {
    let controlled = Rc::new(Cell::new(Some(Time::new(9, 30))));
    let changes = events();
    let changed = changes.clone();
    let rendered = Rc::new(RefCell::new(Vec::new()));
    let state = cx.new(|cx| TimeState::new(cx));
    let cx = open_controlled_time(cx, state.clone(), controlled, changes, rendered.clone());

    press(cx, "tab");
    press(cx, "delete");
    refresh(cx);
    let cleared_hour = rendered
        .borrow()
        .iter()
        .rev()
        .find_map(|(part, text)| (*part == TimeSegment::Hour).then(|| text.clone()))
        .unwrap();
    assert_eq!(cleared_hour, "--");
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value),
        Some(Time::new(9, 30))
    );
    assert!(changed.borrow().is_empty());

    press(cx, "right");
    press(cx, "delete");
    refresh(cx);

    assert_eq!(changed.borrow().as_slice(), ["none"]);
    let value = cx.update(|_, cx| state.read(cx).value);
    assert_eq!(value, Some(Time::new(9, 30)));
    let latest = |segment| {
        rendered
            .borrow()
            .iter()
            .rev()
            .find_map(|(part, text)| (*part == segment).then(|| text.clone()))
            .unwrap()
    };
    assert_eq!(
        (latest(TimeSegment::Hour), latest(TimeSegment::Minute)),
        ("09".to_owned(), "30".to_owned()),
        "a controlled Some prop restores its display when the caller does not accept null"
    );
}

#[gpui::test]
fn time_field_controlled_none_preserves_partial_display_until_completion(cx: &mut TestAppContext) {
    let controlled = Rc::new(Cell::new(None));
    let changes = events();
    let changed = changes.clone();
    let rendered = Rc::new(RefCell::new(Vec::new()));
    let state = cx.new(|cx| TimeState::new(cx));
    let cx = open_controlled_time(
        cx,
        state.clone(),
        controlled.clone(),
        changes,
        rendered.clone(),
    );

    press(cx, "tab");
    press(cx, "up");
    refresh(cx);
    refresh(cx);
    let latest = |segment| {
        rendered
            .borrow()
            .iter()
            .rev()
            .find_map(|(part, text)| (*part == segment).then(|| text.clone()))
            .unwrap()
    };
    assert_eq!(
        (latest(TimeSegment::Hour), latest(TimeSegment::Minute)),
        ("01".to_owned(), "--".to_owned()),
        "repeated controlled None renders must preserve the local incomplete display"
    );
    assert_eq!(cx.update(|_, cx| state.read(cx).value), None);
    assert!(changed.borrow().is_empty());

    press(cx, "right");
    press(cx, "up");
    assert_eq!(changed.borrow().as_slice(), ["01:01"]);
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value),
        None,
        "without a controlled prop update, completion reports the candidate but keeps the controlled value null"
    );

    controlled.set(Some(Time::new(1, 1)));
    refresh(cx);
    assert_eq!(
        cx.update(|_, cx| state.read(cx).value),
        Some(Time::new(1, 1)),
        "accepting the callback through the controlled prop must restore the committed value"
    );
}

#[gpui::test]
fn time_field_hidden_cleared_segments_do_not_suppress_visible_commits(cx: &mut TestAppContext) {
    let granularity = Rc::new(Cell::new(TimeGranularity::Second));
    let granularity_for_view = granularity.clone();
    let cycle = Rc::new(Cell::new(HourCycle::H24));
    let cycle_for_view = cycle.clone();
    let changes = events();
    let changed = changes.clone();
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 30).with_second(45)));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        TimeField::new(state_for_view.clone())
            .granularity(granularity_for_view.get())
            .hour_cycle(cycle_for_view.get())
            .on_change(move |time, _, _| {
                changes.borrow_mut().push(time.map_or_else(
                    || "none".to_owned(),
                    |time| format!("{:02}:{:02}", time.hour, time.minute),
                ));
            })
            .into_any_element()
    });

    // Clear Second, then hide it. Moving back to Hour and stepping must emit
    // the complete Hour+Minute value even though the hidden Second is empty.
    press(cx, "tab");
    press(cx, "right");
    press(cx, "right");
    press(cx, "delete");
    granularity.set(TimeGranularity::Minute);
    cycle.set(HourCycle::H12);
    refresh(cx);
    press(cx, "left");
    press(cx, "up");
    assert_eq!(changed.borrow().as_slice(), ["10:30"]);

    // Clear the now-visible meridiem, switch back to H24, and prove that this
    // second hidden segment also cannot suppress the visible commit.
    press(cx, "right");
    press(cx, "right");
    press(cx, "delete");
    cycle.set(HourCycle::H24);
    refresh(cx);
    press(cx, "left");
    press(cx, "up");
    assert_eq!(changed.borrow().as_slice(), ["10:30", "11:30"]);
}

#[gpui::test]
fn time_field_native_validate_is_gated_until_blur_commits(cx: &mut TestAppContext) {
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 30)));
    let state_for_view = state;
    let other = cx.new(|cx| InputState::new(cx));
    let plain = other.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                gpui::div().debug_selector(|| "tf".to_owned()).child(
                    TimeField::new(state_for_view.clone())
                        .hour_cycle(HourCycle::H24)
                        .validate(|_| Some("Always invalid".into())),
                ),
            )
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
            cx.debug_bounds("tf")
                .expect("time field paints")
                .size
                .height,
        )
    };
    let rest = height(cx);
    // Focus the field, move between segments and edit one: neither segment
    // navigation nor an in-field edit is a commit trigger, so the gated error
    // must stay hidden while the field holds focus.
    press(cx, "tab");
    press(cx, "right");
    press(cx, "up");
    harness::wait_ms(cx, 800);
    assert_eq!(
        height(cx),
        rest,
        "segment navigation and editing must not commit the gated validation"
    );
    // Leaving the field after the change commits the snapshot and shows it.
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
fn time_field_allow_validate_shows_live_at_rest(cx: &mut TestAppContext) {
    let state = cx.new(|cx| TimeState::with_value(cx, Time::new(9, 30)));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        gpui::div()
            .debug_selector(|| "tf".to_owned())
            .child(
                TimeField::new(state_for_view.clone())
                    .hour_cycle(HourCycle::H24)
                    .validation_behavior(herogpui_components::ValidationBehavior::Allow)
                    .validate(|_| Some("Always invalid".into())),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    harness::wait_ms(cx, 800);
    let height = f32::from(
        cx.debug_bounds("tf")
            .expect("time field paints")
            .size
            .height,
    );
    assert!(
        height > px(36.).into(),
        "Allow must paint the live validation error at rest"
    );
}
