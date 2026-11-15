//! Single-selection callback parity with React Stately 3.50.0.
//!
//! `useComboBoxState.setValue` reports a changed key through onChange, and
//! `useControlledState` leaves acceptance to the controlled owner. Re-picking
//! the current item can notify onSelectionChange but does not change onChange.

use crate::harness;

use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui::{prelude::*, px, Focusable, Keystroke, SharedString, TestAppContext};
use herogpui_components::{ComboBox, Input, InputState, MenuTrigger, PickerItem, SelectionMode};

use harness::{click, events, open_host, press, still};

fn settle_exit(cx: &mut gpui::VisualTestContext) {
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

#[gpui::test]
fn single_pick_reports_the_complete_selection_for_pointer_enter_and_tab(app: &mut TestAppContext) {
    for activation in ["pointer", "enter", "tab"] {
        for virtualized in [false, true] {
            let changes = events();
            let seen_changes = changes.clone();
            let picks = events();
            let seen_picks = picks.clone();
            let state = app.new(|cx| InputState::new(cx));
            let state_for_view = state.clone();
            let next = app.new(|cx| InputState::new(cx));
            let next_for_view = next.clone();
            let entity_id = state.entity_id().as_u64();
            let beta_row: &'static str =
                Box::leak(format!("combobox-{entity_id}-item-beta").into_boxed_str());
            still();
            let cx = open_host(app, move || {
                let changes = changes.clone();
                let picks = picks.clone();
                gpui::div()
                    .w(px(320.))
                    .child(
                        ComboBox::new(
                            state_for_view.clone(),
                            vec![
                                PickerItem::new("alpha", "Alpha"),
                                PickerItem::new("beta", "Beta"),
                            ],
                        )
                        .menu_trigger(MenuTrigger::Manual)
                        .default_open(true)
                        .when(virtualized, |picker| picker.row_height(px(36.)))
                        .on_selection_change_all(move |keys, _, _| {
                            changes.borrow_mut().push(keys.join(","));
                        })
                        .on_selection_change(move |key, _, _| {
                            picks.borrow_mut().push(key.to_string());
                        }),
                    )
                    .child(Input::new(next_for_view.clone()))
                    .into_any_element()
            });
            cx.update(|window, cx| window.focus(&state.read(cx).focus_handle(cx), cx));
            if activation == "pointer" {
                let row = cx.debug_bounds(beta_row).unwrap();
                click(cx, f32::from(row.center().x), f32::from(row.center().y));
            } else {
                press(cx, "down");
                press(cx, "down");
                press(cx, activation);
            }
            cx.update(|window, _| window.refresh());
            assert_eq!(
                seen_changes.borrow().as_slice(),
                ["beta"],
                "{activation}, virtual={virtualized}"
            );
            assert_eq!(seen_picks.borrow().as_slice(), ["beta"]);
            assert_eq!(
                state.read_with(cx, |state, _| state.value().to_owned()),
                "Beta"
            );
            settle_exit(cx);
            assert!(cx.debug_bounds(beta_row).is_none());
            if activation == "tab" {
                assert!(cx.update(|window, cx| next.read(cx).focus_handle(cx).is_focused(window)));
            }

            // A plain pointer re-pick still reports its key to the scalar
            // convenience callback; the complete value did not change.
            click(cx, 298., 18.);
            let row = cx.debug_bounds(beta_row).unwrap();
            click(cx, f32::from(row.center().x), f32::from(row.center().y));
            assert_eq!(seen_changes.borrow().as_slice(), ["beta"]);
            assert_eq!(seen_picks.borrow().as_slice(), ["beta", "beta"]);

            cx.update(|window, cx| window.focus(&state.read(cx).focus_handle(cx), cx));
            press(cx, "ctrl-a");
            press(cx, "backspace");
            assert_eq!(seen_changes.borrow().as_slice(), ["beta", ""]);
            assert_eq!(seen_picks.borrow().as_slice(), ["beta", "beta"]);
        }
    }
}

#[gpui::test]
fn single_slice_callback_can_accept_or_reject_controlled_picks(app: &mut TestAppContext) {
    for accept in [false, true] {
        for scalar_builder in [false, true] {
            let owner = Rc::new(RefCell::new(vec![SharedString::from("alpha")]));
            let owner_for_view = owner.clone();
            let displayed = Rc::new(RefCell::new(String::new()));
            let displayed_for_view = displayed.clone();
            let changes = events();
            let seen_changes = changes.clone();
            let state = app.new(|cx| InputState::new(cx));
            let entity_id = state.entity_id().as_u64();
            let beta_row: &'static str =
                Box::leak(format!("combobox-{entity_id}-item-beta").into_boxed_str());
            still();
            let cx = open_host(app, move || {
                let controlled = owner_for_view.borrow().clone();
                let owner = owner_for_view.clone();
                let displayed = displayed_for_view.clone();
                let changes = changes.clone();
                ComboBox::new(
                    state.clone(),
                    vec![
                        PickerItem::new("alpha", "Alpha"),
                        PickerItem::new("beta", "Beta"),
                    ],
                )
                .menu_trigger(MenuTrigger::Manual)
                .default_input_value("")
                .default_open(true)
                .when(scalar_builder, |picker| {
                    picker.selected_key(controlled[0].to_string())
                })
                .when(!scalar_builder, |picker| picker.selected_keys(controlled))
                .on_selection_change_all(move |keys, window, _| {
                    changes.borrow_mut().push(keys.join(","));
                    if accept {
                        *owner.borrow_mut() = keys.to_vec();
                        window.refresh();
                    }
                })
                .value_content(move |value| {
                    *displayed.borrow_mut() = value.selected_text.to_owned();
                    value.default_children
                })
                .into_any_element()
            });
            let row = cx.debug_bounds(beta_row).unwrap();
            click(cx, f32::from(row.center().x), f32::from(row.center().y));
            cx.update(|window, _| window.refresh());
            assert_eq!(seen_changes.borrow().as_slice(), ["beta"]);
            assert_eq!(
                owner.borrow()[0].as_ref(),
                if accept { "beta" } else { "alpha" }
            );
            assert_eq!(
                displayed.borrow().as_str(),
                if accept { "Beta" } else { "Alpha" }
            );
        }
    }
}

#[gpui::test]
fn picks_respect_read_only_and_disabled_keys(app: &mut TestAppContext) {
    for read_only in [false, true] {
        for virtualized in [false, true] {
            for multiple in [false, true] {
                let callbacks = events();
                let seen_callbacks = callbacks.clone();
                let state = app.new(|cx| InputState::new(cx));
                let state_for_view = state.clone();
                let entity_id = state.entity_id().as_u64();
                let beta_row: &'static str =
                    Box::leak(format!("combobox-{entity_id}-item-beta").into_boxed_str());
                still();
                let cx = open_host(app, move || {
                    let all = callbacks.clone();
                    let one = callbacks.clone();
                    ComboBox::new(
                        state_for_view.clone(),
                        vec![
                            PickerItem::new("alpha", "Alpha"),
                            PickerItem::new("beta", "Beta"),
                        ],
                    )
                    .selection_mode(if multiple {
                        SelectionMode::Multiple
                    } else {
                        SelectionMode::Single
                    })
                    .menu_trigger(MenuTrigger::Manual)
                    .default_open(true)
                    .is_read_only(read_only)
                    .when(!read_only, |picker| picker.disabled_keys(["beta".into()]))
                    .when(virtualized, |picker| picker.row_height(px(36.)))
                    .on_selection_change_all(move |keys, _, _| {
                        all.borrow_mut().push(keys.join(","));
                    })
                    .on_selection_change(move |key, _, _| one.borrow_mut().push(key.to_string()))
                    .into_any_element()
                });
                cx.update(|window, cx| window.focus(&state.read(cx).focus_handle(cx), cx));
                let row = cx.debug_bounds(beta_row).unwrap();
                click(cx, f32::from(row.center().x), f32::from(row.center().y));
                assert!(
                    seen_callbacks.borrow().is_empty(),
                    "read_only={read_only}, virtual={virtualized}"
                );
                assert!(state.read_with(cx, |state, _| state.value().is_empty()));
                press(cx, "down");
                press(cx, "enter");
                if read_only {
                    assert!(seen_callbacks.borrow().is_empty());
                } else {
                    assert_eq!(
                        seen_callbacks.borrow().as_slice(),
                        if multiple {
                            &["alpha"][..]
                        } else {
                            &["alpha", "alpha"][..]
                        }
                    );
                }
            }
        }
    }
}

/// No explicit refresh is inserted between events. Pinned GPUI redraws dirty
/// state before key dispatch; navigation must then use the edited collection,
/// as React Stately 3.50.0's displayedCollection does after inputValue changes.
#[gpui::test]
fn back_to_back_typing_and_acceptance_uses_the_current_query(app: &mut TestAppContext) {
    for initially_open in [true, false] {
        for virtualized in [false, true] {
            let changes = events();
            let seen_changes = changes.clone();
            let state = app.new(|cx| InputState::new(cx));
            let state_for_view = state.clone();
            still();
            let cx = open_host(app, move || {
                let changes = changes.clone();
                ComboBox::new(
                    state_for_view.clone(),
                    vec![
                        PickerItem::new("rust", "Rust"),
                        PickerItem::new("python", "Python"),
                        PickerItem::new("go", "Go"),
                    ],
                )
                .menu_trigger(MenuTrigger::Input)
                .default_input_value("Py")
                .default_open(initially_open)
                .when(virtualized, |picker| picker.row_height(px(36.)))
                .on_selection_change_all(move |keys, _, _| {
                    changes.borrow_mut().push(keys.join(","));
                })
                .into_any_element()
            });
            cx.update(|window, cx| {
                window.focus(&state.read(cx).focus_handle(cx), cx);
                window.draw(cx).clear(cx);
                for key in ["ctrl-a", "g", "o", "down", "enter"] {
                    window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
                }
            });
            assert_eq!(
                seen_changes.borrow().as_slice(),
                ["go"],
                "initially_open={initially_open}, virtual={virtualized}"
            );
            assert_eq!(
                state.read_with(cx, |state, _| state.value().to_owned()),
                "Go"
            );
        }
    }
}
