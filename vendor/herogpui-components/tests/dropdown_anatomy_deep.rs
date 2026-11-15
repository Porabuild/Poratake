//! Dropdown composition metrics, explicit identity and nested overlay behavior.

use crate::harness;

use std::{cell::Cell, cell::RefCell, collections::HashMap, rc::Rc};

use gpui::{prelude::*, px, TestAppContext};
use harness::{click, events, open_host};
use herogpui_components::{Button, Dropdown, Menu, MenuItem, Popover, SelectionMode};

fn same_call_site_dropdown(
    trigger_id: &'static str,
    label: &'static str,
    prefix: &'static str,
    actions: harness::Events,
) -> Dropdown {
    Dropdown::uncontrolled(
        gpui::ElementId::Name(format!("{trigger_id}-dropdown").into()),
        Button::new(trigger_id).label(label),
        vec![MenuItem::new("same", "Same")],
    )
    .on_action(move |key, _, _| actions.borrow_mut().push(format!("{prefix}:{key}")))
}

#[gpui::test]
fn explicitly_identified_dropdowns_keep_callbacks_independent(cx: &mut TestAppContext) {
    let actions = events();
    let first_actions = actions.clone();
    let second_actions = actions.clone();

    let cx = open_host(cx, move || {
        let first_actions = first_actions.clone();
        let second_actions = second_actions.clone();
        gpui::div()
            .flex()
            .gap(px(24.))
            .child(
                Dropdown::uncontrolled(
                    "implicit-dropdown-first",
                    gpui::div()
                        .w(px(100.))
                        .child(Button::new("implicit-dropdown-first-trigger").label("First")),
                    vec![MenuItem::new("same", "Same")],
                )
                .on_action(move |key, _, _| {
                    first_actions.borrow_mut().push(format!("first:{key}"));
                }),
            )
            .child(
                Dropdown::uncontrolled(
                    "implicit-dropdown-second",
                    gpui::div()
                        .w(px(100.))
                        .child(Button::new("implicit-dropdown-second-trigger").label("Second")),
                    vec![MenuItem::new("same", "Same")],
                )
                .on_action(move |key, _, _| {
                    second_actions.borrow_mut().push(format!("second:{key}"));
                }),
            )
            .into_any_element()
    });

    click(cx, 40., 18.);
    click(cx, 40., 64.);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(150));
    click(cx, 150., 18.);
    click(cx, 150., 64.);

    assert_eq!(actions.borrow().as_slice(), ["first:same", "second:same"]);
}

#[gpui::test]
fn same_constructor_call_site_dropdowns_keep_state_independent(cx: &mut TestAppContext) {
    let actions = events();
    let recorded = actions.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .gap(px(24.))
            .child(same_call_site_dropdown(
                "same-call-first",
                "First",
                "first",
                recorded.clone(),
            ))
            .child(same_call_site_dropdown(
                "same-call-second",
                "Second",
                "second",
                recorded.clone(),
            ))
            .into_any_element()
    });

    click(cx, 40., 18.);
    click(cx, 40., 64.);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(150));
    click(cx, 150., 18.);
    click(cx, 150., 64.);

    assert_eq!(actions.borrow().as_slice(), ["first:same", "second:same"]);
}

#[gpui::test]
fn explicitly_identified_menus_seed_and_update_independently(cx: &mut TestAppContext) {
    let first_selected = Rc::new(RefCell::new(HashMap::<String, bool>::new()));
    let second_selected = Rc::new(RefCell::new(HashMap::<String, bool>::new()));
    let selections = events();
    let first_selected_for_render = first_selected.clone();
    let second_selected_for_render = second_selected.clone();
    let first_selections = selections.clone();
    let second_selections = selections.clone();

    let cx = open_host(cx, move || {
        let first_selected = first_selected_for_render.clone();
        let second_selected = second_selected_for_render.clone();
        let first_selections = first_selections.clone();
        let second_selections = second_selections.clone();
        gpui::div()
            .flex()
            .gap(px(24.))
            .child(
                Menu::new(
                    "implicit-menu-first",
                    vec![
                        MenuItem::new("same", "Same"),
                        MenuItem::new("other", "Other"),
                    ],
                )
                .selection_mode(SelectionMode::Multiple)
                .default_selected_keys(["same"])
                .item_content(move |key, state| {
                    first_selected
                        .borrow_mut()
                        .insert(key.to_string(), state.is_selected);
                    gpui::div().child(key.to_string()).into_any_element()
                })
                .on_selection_change(move |keys, _, _| {
                    first_selections
                        .borrow_mut()
                        .push(format!("first:{}", keys.len()));
                }),
            )
            .child(
                Menu::new(
                    "implicit-menu-second",
                    vec![
                        MenuItem::new("same", "Same"),
                        MenuItem::new("other", "Other"),
                    ],
                )
                .selection_mode(SelectionMode::Multiple)
                .default_selected_keys(["other"])
                .item_content(move |key, state| {
                    second_selected
                        .borrow_mut()
                        .insert(key.to_string(), state.is_selected);
                    gpui::div().child(key.to_string()).into_any_element()
                })
                .on_selection_change(move |keys, _, _| {
                    second_selections
                        .borrow_mut()
                        .push(format!("second:{}", keys.len()));
                }),
            )
            .into_any_element()
    });

    assert_eq!(first_selected.borrow().get("same"), Some(&true));
    assert_eq!(first_selected.borrow().get("other"), Some(&false));
    assert_eq!(second_selected.borrow().get("same"), Some(&false));
    assert_eq!(second_selected.borrow().get("other"), Some(&true));

    click(cx, 40., 22.);
    click(cx, 280., 64.);
    assert_eq!(selections.borrow().as_slice(), ["first:0", "second:0"]);
}

#[gpui::test]
fn higher_overlay_does_not_dismiss_open_submenu(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let higher_bounds = Rc::new(RefCell::new(None));
    let higher_bounds_for_render = higher_bounds.clone();
    let higher_open = Rc::new(Cell::new(false));
    let higher_open_for_render = higher_open.clone();

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let higher_bounds = higher_bounds_for_render.clone();
        let higher_open = higher_open_for_render.get();
        gpui::div()
            .flex()
            .gap(px(24.))
            .child(
                Dropdown::uncontrolled(
                    "submenu-lower",
                    Button::new("submenu-lower-trigger").label("Lower"),
                    vec![MenuItem::new("more", "More")
                        .submenu(vec![MenuItem::new("child", "Child")])],
                )
                .on_open_change({
                    let changes = changes.clone();
                    move |open, _, _| changes.borrow_mut().push(format!("dropdown:{open}"))
                }),
            )
            .child(
                Popover::new(Button::new("submenu-higher-trigger").label("Higher"))
                    .is_open(higher_open)
                    .on_open_change({
                        move |open, _, _| changes.borrow_mut().push(format!("popover:{open}"))
                    })
                    .child(
                        gpui::div()
                            .w(px(180.))
                            .h(px(80.))
                            .child(gpui::canvas(
                                move |bounds, _, _| {
                                    *higher_bounds.borrow_mut() = Some(bounds);
                                    bounds
                                },
                                |_, _, _, _| {},
                            ))
                            .child("Higher overlay"),
                    ),
            )
            .into_any_element()
    });

    click(cx, 40., 18.);
    click(cx, 40., 64.);
    higher_open.set(true);
    cx.update(|window, _| window.refresh());
    let bounds = higher_bounds
        .borrow()
        .expect("the higher overlay content must be painted");
    // This point is inside the higher Popover's content but outside the lower
    // Dropdown's parent panel. A raw outside listener would dismiss the lower
    // menu; the shared token must let the higher surface own the press.
    click(
        cx,
        f32::from(bounds.origin.x) + 90.,
        f32::from(bounds.origin.y) + 40.,
    );

    assert_eq!(recorded.borrow().as_slice(), ["dropdown:true"]);
}

#[gpui::test]
fn item_start_content_reaches_dropdown_and_submenu_rows(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, move || {
        Dropdown::uncontrolled(
            "start-content-dropdown",
            Button::new("start-content-trigger").label("Open"),
            vec![
                MenuItem::new("new", "New"),
                MenuItem::new("more", "More").submenu(vec![MenuItem::new("nested", "Nested")]),
            ],
        )
        .item_start_content(|key, _| {
            let key = key.clone();
            Some(
                gpui::div()
                    .w(px(12.))
                    .h(px(12.))
                    .debug_selector(move || format!("dd-start-{key}"))
                    .into_any_element(),
            )
        })
        .into_any_element()
    });
    click(cx, 30., 18.);
    harness::settle(cx);
    assert!(cx.debug_bounds("dd-start-new").is_some());
    let more = cx
        .debug_bounds("dd-start-more")
        .expect("submenu trigger row");

    cx.simulate_mouse_move(more.center(), None, gpui::Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    harness::settle(cx);
    assert!(
        cx.debug_bounds("dd-start-nested").is_some(),
        "the submenu inherits the leading-element closure"
    );
}
