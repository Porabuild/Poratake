//! Behaviour coverage for Dropdown.Menu's controlled and uncontrolled
//! `selectedKeys` pair.
//!
//! HeroUI v3 documents `defaultSelectedKeys` as "The initial selected keys
//! (uncontrolled)" and `selectedKeys` as the controlled value. The exact
//! react-stately 3.49.0 implementation passes those separately to
//! `useControlledState`, so an empty controlled set is not the same thing as
//! an absent controlled prop.

use crate::harness;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{prelude::*, px, SharedString, TestAppContext};
use herogpui_components::{Button, Dropdown, Menu, MenuItem, SelectionMode};

use harness::{click, events, open_host, press};

fn joined(keys: &[SharedString]) -> String {
    keys.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn let_exit_finish(cx: &mut TestAppContext) {
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(150));
}

#[gpui::test]
fn menu_default_selected_keys_reaches_the_initial_render(cx: &mut TestAppContext) {
    let states = Rc::new(RefCell::new(HashMap::<String, bool>::new()));
    let recorded = states.clone();

    let _cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Menu::new(
            "dds-menu-seed",
            vec![
                MenuItem::new("apple", "Apple"),
                MenuItem::new("banana", "Banana"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .default_selected_keys(["apple"])
        .item_content(move |key, state| {
            recorded
                .borrow_mut()
                .insert(key.to_string(), state.is_selected);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .into_any_element()
    });

    assert_eq!(
        states.borrow().get("apple"),
        Some(&true),
        "the uncontrolled seed must mark its item selected on the first frame"
    );
    assert_eq!(states.borrow().get("banana"), Some(&false));
}

#[gpui::test]
fn dropdown_uncontrolled_selection_accumulates_without_callback_feedback(cx: &mut TestAppContext) {
    let selections = events();
    let recorded = selections.clone();

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        Dropdown::uncontrolled(
            "dds-accumulate",
            Button::new("dds-accumulate-trigger").label("Fruits"),
            vec![
                MenuItem::new("apple", "Apple"),
                MenuItem::new("banana", "Banana"),
                MenuItem::new("cherry", "Cherry"),
            ],
        )
        .id("dds-accumulate")
        .selection_mode(SelectionMode::Multiple)
        .default_selected_keys(["apple"])
        .on_selection_change(move |keys, _, _| {
            selections.borrow_mut().push(joined(keys));
        })
        .into_any_element()
    });

    click(cx, 40., 18.);
    click(cx, 40., 102.);
    click(cx, 40., 140.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["apple,banana", "apple,banana,cherry"],
        "the component must retain both the seed and the first activation without caller feedback"
    );
}

#[gpui::test]
fn dropdown_controlled_empty_stays_controlled(cx: &mut TestAppContext) {
    let selections = events();
    let recorded = selections.clone();

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        Dropdown::uncontrolled(
            "dds-controlled",
            Button::new("dds-controlled-trigger").label("Fruits"),
            vec![
                MenuItem::new("apple", "Apple"),
                MenuItem::new("banana", "Banana"),
            ],
        )
        .id("dds-controlled")
        .selection_mode(SelectionMode::Multiple)
        .default_selected_keys(["banana"])
        .selected_keys(Vec::<SharedString>::new())
        .on_selection_change(move |keys, _, _| {
            selections.borrow_mut().push(joined(keys));
        })
        .into_any_element()
    });

    click(cx, 40., 18.);
    click(cx, 40., 64.);
    cx.update(|window, _| window.refresh());
    click(cx, 40., 64.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["apple", "apple"],
        "repeated reports from a controlled-empty value remain legitimate; only a blocked \
         final-key removal is suppressed"
    );
}

#[gpui::test]
fn dropdown_uncontrolled_selection_survives_close_and_reopen(cx: &mut TestAppContext) {
    let selections = events();
    let recorded = selections.clone();

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        Dropdown::uncontrolled(
            "dds-reopen",
            Button::new("dds-reopen-trigger").label("Fruits"),
            vec![
                MenuItem::new("apple", "Apple"),
                MenuItem::new("banana", "Banana"),
            ],
        )
        .id("dds-reopen")
        .selection_mode(SelectionMode::Single)
        .default_selected_keys(["apple"])
        .on_selection_change(move |keys, _, _| {
            selections.borrow_mut().push(joined(keys));
        })
        .into_any_element()
    });

    click(cx, 40., 18.);
    click(cx, 40., 102.);
    let_exit_finish(cx);
    click(cx, 40., 18.);
    click(cx, 40., 64.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["banana", "apple"],
        "closing the panel must not reset the wrapper's uncontrolled selection to its seed"
    );
}

#[gpui::test]
fn disallow_empty_selection_keeps_uncontrolled_single_seed(cx: &mut TestAppContext) {
    let selections = events();
    let recorded = selections.clone();
    let states = Rc::new(RefCell::new(HashMap::<String, bool>::new()));
    let rendered = states.clone();

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        let rendered = rendered.clone();
        Menu::new(
            "dds-menu-controlled",
            vec![
                MenuItem::new("apple", "Apple"),
                MenuItem::new("banana", "Banana"),
            ],
        )
        .id("dds-single-own")
        .selection_mode(SelectionMode::Single)
        .default_selected_keys(["apple"])
        .disallow_empty_selection(true)
        .item_content(move |key, state| {
            rendered
                .borrow_mut()
                .insert(key.to_string(), state.is_selected);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .on_selection_change(move |keys, _, _| {
            selections.borrow_mut().push(joined(keys));
        })
        .into_any_element()
    });

    click(cx, 40., 22.);
    cx.update(|window, _| window.refresh());

    assert!(
        recorded.borrow().is_empty(),
        "a blocked final-key removal must emit no selection change"
    );
    assert_eq!(
        states.borrow().get("apple"),
        Some(&true),
        "the blocked removal must also keep the uncontrolled seed selected"
    );
}

#[gpui::test]
fn disallow_empty_selection_keeps_controlled_single_value(cx: &mut TestAppContext) {
    let selections = events();
    let recorded = selections.clone();

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        Dropdown::uncontrolled(
            "dds-single-controlled",
            Button::new("dds-single-controlled-trigger").label("Fruits"),
            vec![
                MenuItem::new("apple", "Apple"),
                MenuItem::new("banana", "Banana"),
            ],
        )
        .id("dds-single-controlled")
        .selection_mode(SelectionMode::Single)
        .selected_keys(["apple"])
        .disallow_empty_selection(true)
        .on_selection_change(move |keys, _, _| {
            selections.borrow_mut().push(joined(keys));
        })
        .into_any_element()
    });

    click(cx, 40., 18.);
    click(cx, 40., 64.);

    assert!(recorded.borrow().is_empty());
}

#[gpui::test]
fn disallow_empty_selection_keeps_last_uncontrolled_multiple_key(cx: &mut TestAppContext) {
    let selections = events();
    let recorded = selections.clone();

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        Dropdown::uncontrolled(
            "dds-multi-own",
            Button::new("dds-multi-own-trigger").label("Fruits"),
            vec![
                MenuItem::new("apple", "Apple"),
                MenuItem::new("banana", "Banana"),
            ],
        )
        .id("dds-multi-own")
        .selection_mode(SelectionMode::Multiple)
        .default_selected_keys(["apple"])
        .disallow_empty_selection(true)
        .on_selection_change(move |keys, _, _| {
            selections.borrow_mut().push(joined(keys));
        })
        .into_any_element()
    });

    click(cx, 40., 18.);
    click(cx, 40., 64.);
    click(cx, 40., 102.);
    click(cx, 40., 64.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["apple,banana", "banana"],
        "the blocked final removal is silent, but changes resume once two keys are selected"
    );
}

#[gpui::test]
fn disallow_empty_selection_keeps_controlled_multiple_value(cx: &mut TestAppContext) {
    let selections = events();
    let recorded = selections.clone();

    let cx = open_host(cx, move || {
        let selections = selections.clone();
        Dropdown::uncontrolled(
            "dds-multi-controlled",
            Button::new("dds-multi-controlled-trigger").label("Fruits"),
            vec![
                MenuItem::new("apple", "Apple"),
                MenuItem::new("banana", "Banana"),
            ],
        )
        .id("dds-multi-controlled")
        .selection_mode(SelectionMode::Multiple)
        .selected_keys(["apple"])
        .disallow_empty_selection(true)
        .on_selection_change(move |keys, _, _| {
            selections.borrow_mut().push(joined(keys));
        })
        .into_any_element()
    });

    click(cx, 40., 18.);
    click(cx, 40., 64.);
    cx.update(|window, _| window.refresh());
    click(cx, 40., 64.);

    assert!(recorded.borrow().is_empty());
}

#[gpui::test]
fn pointer_reports_selection_then_action_then_close(cx: &mut TestAppContext) {
    let order = events();
    let recorded = order.clone();

    let cx = open_host(cx, move || {
        let selection_order = order.clone();
        let action_order = order.clone();
        let open_order = order.clone();
        Dropdown::uncontrolled(
            "dds-pointer-order",
            Button::new("dds-pointer-order-trigger").label("Actions"),
            vec![MenuItem::new("one", "One")],
        )
        .id("dds-pointer-order")
        .selection_mode(SelectionMode::Single)
        .on_selection_change(move |keys, _, _| {
            selection_order
                .borrow_mut()
                .push(format!("selection:{}", joined(keys)));
        })
        .on_action(move |key, _, _| {
            action_order.borrow_mut().push(format!("action:{key}"));
        })
        .on_open_change(move |open, _, _| {
            open_order.borrow_mut().push(format!("open:{open}"));
        })
        .into_any_element()
    });

    click(cx, 40., 18.);
    recorded.borrow_mut().clear();
    click(cx, 40., 64.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["selection:one", "action:one", "open:false"]
    );
}

#[gpui::test]
fn enter_reports_selection_then_action_and_closes_multiple_menu(cx: &mut TestAppContext) {
    let order = events();
    let recorded = order.clone();

    let cx = open_host(cx, move || {
        let selection_order = order.clone();
        let action_order = order.clone();
        let open_order = order.clone();
        Dropdown::uncontrolled(
            "dds-enter-order",
            Button::new("dds-enter-order-trigger").label("Actions"),
            vec![MenuItem::new("one", "One"), MenuItem::new("two", "Two")],
        )
        .id("dds-enter-order")
        .selection_mode(SelectionMode::Multiple)
        .on_selection_change(move |keys, _, _| {
            selection_order
                .borrow_mut()
                .push(format!("selection:{}", joined(keys)));
        })
        .on_action(move |key, _, _| {
            action_order.borrow_mut().push(format!("action:{key}"));
        })
        .on_open_change(move |open, _, _| {
            open_order.borrow_mut().push(format!("open:{open}"));
        })
        .into_any_element()
    });

    click(cx, 40., 18.);
    recorded.borrow_mut().clear();
    press(cx, "down");
    press(cx, "enter");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["selection:one", "action:one", "open:false"]
    );
}

#[gpui::test]
fn space_reports_selection_then_action_and_keeps_multiple_menu_open(cx: &mut TestAppContext) {
    let order = events();
    let recorded = order.clone();

    let cx = open_host(cx, move || {
        let selection_order = order.clone();
        let action_order = order.clone();
        let open_order = order.clone();
        Dropdown::uncontrolled(
            "dds-space-order",
            Button::new("dds-space-order-trigger").label("Actions"),
            vec![MenuItem::new("one", "One"), MenuItem::new("two", "Two")],
        )
        .id("dds-space-order")
        .selection_mode(SelectionMode::Multiple)
        .on_selection_change(move |keys, _, _| {
            selection_order
                .borrow_mut()
                .push(format!("selection:{}", joined(keys)));
        })
        .on_action(move |key, _, _| {
            action_order.borrow_mut().push(format!("action:{key}"));
        })
        .on_open_change(move |open, _, _| {
            open_order.borrow_mut().push(format!("open:{open}"));
        })
        .into_any_element()
    });

    click(cx, 40., 18.);
    recorded.borrow_mut().clear();
    press(cx, "down");
    press(cx, "space");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["selection:one", "action:one"]
    );

    click(cx, 40., 102.);
    assert_eq!(
        recorded.borrow().as_slice(),
        [
            "selection:one",
            "action:one",
            "selection:one,two",
            "action:two"
        ],
        "the second row remains clickable because Space did not close the menu"
    );
}

#[gpui::test]
fn action_only_menu_skips_disabled_items_without_selection_reports(cx: &mut TestAppContext) {
    let actions = events();
    let fired = actions.clone();
    let selections = events();
    let selected = selections.clone();

    let cx = open_host(cx, move || {
        let actions = actions.clone();
        let selections = selections.clone();
        Dropdown::uncontrolled(
            "dds-action",
            Button::new("dds-action-trigger").label("Actions"),
            vec![
                MenuItem::new("one", "One"),
                MenuItem::new("two", "Two"),
                MenuItem::new("three", "Three"),
            ],
        )
        .id("dds-action")
        .disabled_keys(["two"])
        .on_action(move |key, _, _| actions.borrow_mut().push(key.to_string()))
        .on_selection_change(move |keys, _, _| {
            selections.borrow_mut().push(joined(keys));
        })
        .into_any_element()
    });

    click(cx, 40., 18.);
    click(cx, 40., 102.);
    assert!(fired.borrow().is_empty(), "a disabled row must not act");
    assert!(
        selected.borrow().is_empty(),
        "an action-only menu must never report selection"
    );

    press(cx, "down");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        fired.borrow().as_slice(),
        ["three"],
        "keyboard navigation must skip the disabled row and activate once"
    );
    assert!(selected.borrow().is_empty());
}
