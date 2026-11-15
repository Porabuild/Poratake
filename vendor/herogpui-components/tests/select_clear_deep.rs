//! HeroUI v3.2.5 Select.ClearButton: real pointer and keyboard dispatch.
use crate::harness;
use gpui::{prelude::*, px, SharedString, TestAppContext};
use harness::{click, events, open_host, press, still};
use herogpui_components::{FieldVariant, PickerItem, Select, SelectClearButton, SelectionMode};
use std::{cell::RefCell, rc::Rc};

#[gpui::test]
fn clear_click_preserves_focus_and_layout_and_empty_target_opens(cx: &mut TestAppContext) {
    for variant in [FieldVariant::Primary, FieldVariant::Secondary] {
        for multiple in [false, true] {
            for controlled in [false, true] {
                still();
                let recorded = events();
                let log = recorded.clone();
                let values = Rc::new(RefCell::new(Vec::<Vec<SharedString>>::new()));
                let observed = values.clone();
                let cx = open_host(cx, move || {
                    let single = log.clone();
                    let all = log.clone();
                    let clear = log.clone();
                    let part = log.clone();
                    let opened = log.clone();
                    let values = values.clone();
                    let mut select = Select::new(
                        "clear",
                        vec![PickerItem::new("a", "Alpha"), PickerItem::new("b", "Beta")],
                    )
                    .variant(variant)
                    .is_required(true)
                    .full_width(true)
                    .selection_mode(if multiple {
                        SelectionMode::Multiple
                    } else {
                        SelectionMode::Single
                    })
                    .on_change(move |v, _, _| single.borrow_mut().push(format!("single:{v:?}")))
                    .on_selection_change_all(move |v, _, _| {
                        all.borrow_mut().push(format!("all:{v:?}"));
                    })
                    .on_clear(move |_, _| clear.borrow_mut().push("clear".into()))
                    .on_open_change(move |v, _, _| opened.borrow_mut().push(format!("open:{v}")))
                    .clear_button(
                        SelectClearButton::new()
                            .on_click(move |_, _, _| part.borrow_mut().push("part".into())),
                    )
                    .value_content(move |v| {
                        values.borrow_mut().push(v.selected_keys.unwrap().to_vec());
                        v.default_children
                    });
                    if multiple {
                        select = if controlled {
                            select.selected_keys(["a".into(), "b".into()])
                        } else {
                            select.default_selected_keys(["a".into(), "b".into()])
                        };
                    } else {
                        select = if controlled {
                            select.value(Some("a".into()))
                        } else {
                            select.default_value(Some("a".into()))
                        };
                    }
                    gpui::div().w(px(256.)).child(select).into_any_element()
                });
                let before = cx.debug_bounds("select-clear-0").unwrap();
                assert_eq!(before.size.width, px(24.));
                assert_eq!(before.size.height, px(24.));
                // One pixel inside the expanded left edge, outside the 20px visual.
                click(
                    cx,
                    f32::from(before.left()) + 1.,
                    f32::from(before.center().y),
                );
                let expected = if multiple { "all:[]" } else { "single:None" };
                assert_eq!(recorded.borrow().as_slice(), [expected, "clear", "part"]);
                assert_eq!(cx.debug_bounds("select-clear-0").unwrap(), before);
                assert_eq!(observed.borrow().last().unwrap().is_empty(), !controlled);
                // Keyboard must still address the trigger. A controlled owner
                // that rejects the clear request reports again without changing.
                press(cx, "backspace");
                assert!(
                    cx.update(|_, cx| herogpui_components::extend::focus_visible(cx)),
                    "clearing by keyboard must enable the focus ring after a pointer press"
                );
                if controlled {
                    assert_eq!(
                        recorded.borrow().as_slice(),
                        [expected, "clear", "part", expected, "clear"]
                    );
                } else {
                    assert_eq!(recorded.borrow().len(), 3);
                    click(
                        cx,
                        f32::from(before.center().x),
                        f32::from(before.center().y),
                    );
                    assert_eq!(recorded.borrow().last().unwrap(), "open:true");
                }
            }
        }
    }
}

/// `&:active, &[data-pressed="true"]` applies `transform: scale(0.93)` to the
/// whole clear control about its center, and CSS transforms rescale hit
/// testing without touching layout: the expanded pointer target shrinks from
/// 24px to 24 * 0.93 = 22.32px while the press lasts, stays centered on the
/// 20px slot, and restores on release. A release inside the shrunken box
/// still clears.
#[gpui::test]
fn clear_pressed_target_scales_about_the_slot_center(cx: &mut TestAppContext) {
    use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent};
    still();
    let recorded = events();
    let log = recorded.clone();
    let cx = open_host(cx, move || {
        let log = log.clone();
        Select::new("press-scale", vec![PickerItem::new("a", "Alpha")])
            .default_value(Some("a".into()))
            .clear_button(SelectClearButton::new())
            .on_clear(move |_, _| log.borrow_mut().push("clear".into()))
            .into_any_element()
    });
    let rest = cx.debug_bounds("select-clear-0").unwrap();
    assert_eq!(rest.size.width, px(24.));
    assert_eq!(rest.size.height, px(24.));
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: rest.center(),
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    cx.update(|w, _| w.refresh());
    let pressed = cx.debug_bounds("select-clear-0").unwrap();
    // Debug bounds are snapped to the device-pixel grid (2x here), so the
    // unsnapped 22.32px box reports as 22.5px.
    assert!(
        (f32::from(pressed.size.width) - f32::from(px(24. * 0.93))).abs() < 0.25,
        "the pressed pointer target must track the pinned scale, got {pressed:?}"
    );
    assert!((f32::from(pressed.size.height) - f32::from(px(24. * 0.93))).abs() < 0.25);
    assert!(
        (pressed.center() - rest.center()).magnitude() < px(0.4).into(),
        "the pressed target must stay centered on the 20px slot: {pressed:?} vs {rest:?}"
    );
    cx.simulate_event(MouseMoveEvent {
        position: rest.center(),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    });
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: rest.center(),
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    cx.update(|w, _| w.refresh());
    assert_eq!(recorded.borrow().as_slice(), ["clear"]);
    let released = cx.debug_bounds("select-clear-0").unwrap();
    assert_eq!(released.size.width, px(24.));
    assert_eq!(released.size.height, px(24.));
}

#[gpui::test]
fn clear_shortcut_requires_composition_nonempty_enabled_and_closed(cx: &mut TestAppContext) {
    for composed in [false, true] {
        for disabled in [false, true] {
            for open in [false, true] {
                for key in ["backspace", "delete"] {
                    still();
                    let recorded = events();
                    let log = recorded.clone();
                    let cx = open_host(cx, move || {
                        let log = log.clone();
                        let mut select =
                            Select::new("shortcut", vec![PickerItem::new("a", "Alpha")])
                                .default_value(Some("a".into()))
                                .is_disabled(disabled)
                                .is_open(open)
                                .on_clear(move |_, _| log.borrow_mut().push("clear".into()));
                        if composed {
                            select = select.clear_button(SelectClearButton::new());
                        }
                        select.into_any_element()
                    });
                    press(cx, "tab");
                    press(cx, key);
                    assert_eq!(
                        recorded.borrow().len(),
                        usize::from(composed && !disabled && !open),
                        "{composed} {disabled} {open} {key}"
                    );
                }
            }
        }
    }
}

#[gpui::test]
fn clear_pointer_cancellation_secondary_buttons_and_disabled_are_inert(cx: &mut TestAppContext) {
    use gpui::{point, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent};
    for disabled in [false, true] {
        still();
        let recorded = events();
        let log = recorded.clone();
        let cx = open_host(cx, move || {
            let clear = log.clone();
            let opened = log.clone();
            Select::new("pointer", vec![PickerItem::new("a", "Alpha")])
                .default_value(Some("a".into()))
                .is_disabled(disabled)
                .clear_button(
                    SelectClearButton::new()
                        .children([gpui::div().size(px(12.)).into_any_element()]),
                )
                .on_clear(move |_, _| clear.borrow_mut().push("clear".into()))
                .on_open_change(move |v, _, _| opened.borrow_mut().push(format!("open:{v}")))
                .into_any_element()
        });
        let center = cx.debug_bounds("select-clear-0").unwrap().center();
        for button in [MouseButton::Right, MouseButton::Middle, MouseButton::Left] {
            cx.simulate_event(MouseDownEvent {
                button,
                position: center,
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            });
            cx.update(|w, _| w.refresh());
            assert!(
                recorded.borrow().is_empty(),
                "pointer down must not clear or open"
            );
            let release = if button == MouseButton::Left {
                point(px(500.), px(300.))
            } else {
                center
            };
            cx.simulate_event(MouseMoveEvent {
                position: release,
                pressed_button: Some(button),
                modifiers: Modifiers::none(),
            });
            cx.update(|w, _| w.refresh());
            cx.simulate_event(MouseUpEvent {
                button,
                position: release,
                modifiers: Modifiers::none(),
                click_count: 1,
            });
            cx.update(|w, _| w.refresh());
            assert!(
                recorded.borrow().is_empty(),
                "secondary buttons and canceled presses must be inert"
            );
        }
        click(cx, f32::from(center.x), f32::from(center.y));
        assert_eq!(recorded.borrow().len(), usize::from(!disabled));
    }
}

#[gpui::test]
fn controlled_clear_updates_when_owner_accepts_and_parts_stay_independent(cx: &mut TestAppContext) {
    still();
    let selected = Rc::new(RefCell::new(Some(SharedString::from("a"))));
    let owner = selected.clone();
    let recorded = events();
    let log = recorded.clone();
    let cx = open_host(cx, move || {
        let state = selected.clone();
        let first = log.clone();
        let second = log.clone();
        Select::new("two-clear-parts", vec![PickerItem::new("a", "Alpha")])
            .value(selected.borrow().clone())
            .on_change(move |v, _, cx| {
                *state.borrow_mut() = v.clone();
                cx.refresh_windows();
            })
            .clear_button(
                SelectClearButton::new()
                    .on_click(move |_, _, _| first.borrow_mut().push("first".into())),
            )
            .clear_button(
                SelectClearButton::new()
                    .on_click(move |_, _, _| second.borrow_mut().push("second".into())),
            )
            .into_any_element()
    });
    let first = cx.debug_bounds("select-clear-0").unwrap();
    let second = cx.debug_bounds("select-clear-1").unwrap();
    assert!(first.right() <= second.left());
    click(
        cx,
        f32::from(second.center().x),
        f32::from(second.center().y),
    );
    assert!(owner.borrow().is_none());
    assert_eq!(recorded.borrow().as_slice(), ["second"]);
    press(cx, "delete");
    assert_eq!(recorded.borrow().as_slice(), ["second"]);
}

/// ArrowUp opens a Select onto React Aria's `last` strategy: with no
/// selection the cursor lands on the last enabled stop, so Enter commits it.
#[gpui::test]
fn arrow_up_open_with_no_selection_seats_the_last_row(cx: &mut TestAppContext) {
    let recorded = events();
    let seen = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Select::new(
            "sel-open-up",
            vec![
                PickerItem::new("alpha", "Alpha"),
                PickerItem::new("beta", "Beta"),
                PickerItem::new("gamma", "Gamma"),
            ],
        )
        .on_selection_change(move |v, _, _| {
            recorded
                .borrow_mut()
                .push(v.as_ref().map(ToString::to_string).unwrap_or_default());
        })
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "up");
    press(cx, "enter");
    assert_eq!(
        seen.borrow().last().map(String::as_str),
        Some("gamma"),
        "ArrowUp with no selection must seat the last enabled row"
    );
}

/// ArrowDown opens onto `first`, but a selected key that is disabled is not a
/// stop, so the seat falls back to the first enabled row rather than the
/// disabled selection.
#[gpui::test]
fn arrow_down_open_with_disabled_selection_seats_the_first_row(cx: &mut TestAppContext) {
    let recorded = events();
    let seen = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Select::new(
            "sel-open-disabled",
            vec![
                PickerItem::new("alpha", "Alpha"),
                PickerItem::new("beta", "Beta"),
                PickerItem::new("gamma", "Gamma"),
            ],
        )
        .default_value(Some("beta".into()))
        .disabled_keys([SharedString::from("beta")])
        .on_selection_change(move |v, _, _| {
            recorded
                .borrow_mut()
                .push(v.as_ref().map(ToString::to_string).unwrap_or_default());
        })
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        seen.borrow().last().map(String::as_str),
        Some("alpha"),
        "a disabled selected row must not take the open seat"
    );
}

/// A pointer open carries the `None` strategy, but the selected key still
/// wins when it is an enabled stop: opening by click seats the cursor on the
/// selected row, so a following Down walks to the next one.
#[gpui::test]
fn pointer_open_seats_the_selected_row(cx: &mut TestAppContext) {
    let recorded = events();
    let seen = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Select::new(
            "sel-open-pointer",
            vec![
                PickerItem::new("alpha", "Alpha"),
                PickerItem::new("beta", "Beta"),
                PickerItem::new("gamma", "Gamma"),
            ],
        )
        .default_value(Some("beta".into()))
        .on_selection_change(move |v, _, _| {
            recorded
                .borrow_mut()
                .push(v.as_ref().map(ToString::to_string).unwrap_or_default());
        })
        .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        seen.borrow().last().map(String::as_str),
        Some("gamma"),
        "pointer-open must seat the selected row: Down from beta lands on gamma"
    );
}

/// Removing the highlighted row repairs the cursor to the nearest enabled
/// neighbour in the previous order: the highlighted "gamma" is dropped from
/// `items`, and the next render walks forward onto "delta".
#[gpui::test]
fn removing_the_highlighted_row_repairs_to_the_next(cx: &mut TestAppContext) {
    let items = Rc::new(RefCell::new(vec![
        ("alpha", "Alpha"),
        ("beta", "Beta"),
        ("gamma", "Gamma"),
        ("delta", "Delta"),
    ]));
    let recorded = events();
    let seen = recorded.clone();
    let items_for_view = items.clone();

    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        let items = items_for_view.borrow();
        let its: Vec<PickerItem> = items
            .iter()
            .map(|(key, label)| PickerItem::new(*key, *label))
            .collect();
        Select::new("sel-repair", its)
            .on_selection_change(move |v, _, _| {
                recorded
                    .borrow_mut()
                    .push(v.as_ref().map(ToString::to_string).unwrap_or_default());
            })
            .into_any_element()
    });

    // Open onto the first row, then walk to "gamma" (index 2 of 4).
    press(cx, "tab");
    press(cx, "down");
    press(cx, "down");
    press(cx, "down");

    // The caller drops "gamma"; the next render must move the highlight on.
    items.borrow_mut().remove(2);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();

    press(cx, "enter");
    assert_eq!(
        seen.borrow().last().map(String::as_str),
        Some("delta"),
        "removing the highlighted row must repair the cursor to the next row"
    );
}

/// A multiple Select seats the first selected key *in selection order* that
/// is an enabled stop — not the first row and not alphabetical. With
/// `selected_keys` ordered `["gamma", "alpha"]`, "gamma" (a later row) wins
/// the seat over "alpha", so Enter toggles "gamma" off and leaves "alpha".
#[gpui::test]
fn multiple_open_seats_the_first_selected_key_in_selection_order(cx: &mut TestAppContext) {
    let recorded = events();
    let seen = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Select::new(
            "sel-open-multi",
            vec![
                PickerItem::new("alpha", "Alpha"),
                PickerItem::new("beta", "Beta"),
                PickerItem::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .selected_keys([SharedString::from("gamma"), SharedString::from("alpha")])
        .on_selection_change_all(move |keys, _, _| {
            recorded.borrow_mut().push(keys.join(","));
        })
        .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "enter");
    assert_eq!(
        seen.borrow().last().map(String::as_str),
        Some("alpha"),
        "the seat must land on the first selected key in selection order"
    );
}
