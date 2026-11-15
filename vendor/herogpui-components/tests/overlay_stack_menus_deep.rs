//! Nested Dropdown and Tooltip overlay-stack contracts.

use crate::harness;

use std::{cell::RefCell, rc::Rc};

use gpui::{prelude::*, px, TestAppContext};
use harness::{events, open_host, press};
use herogpui_components::{Button, Dropdown, MenuItem, Popover, Tooltip};

fn still() {
    harness::still();
}

#[gpui::test]
fn dropdown_escape_closes_before_parent_popover(cx: &mut TestAppContext) {
    still();
    let changes = events();
    let recorded = changes.clone();
    let dropdown_open = Rc::new(RefCell::new(true));
    let popover_open = Rc::new(RefCell::new(true));

    let cx = open_host(cx, move || {
        let dropdown_open = dropdown_open.clone();
        let popover_open = popover_open.clone();
        let changes = changes.clone();
        let popover_is_open = *popover_open.borrow();
        let dropdown_is_open = *dropdown_open.borrow();
        Popover::new(Button::new("menu-stack-trigger").label("Outer"))
            .id("menu-stack-popover")
            .is_open(popover_is_open)
            .on_open_change({
                let changes = changes.clone();
                move |open, window, _| {
                    *popover_open.borrow_mut() = *open;
                    changes.borrow_mut().push(format!("popover:{open}"));
                    window.refresh();
                }
            })
            .child(
                Dropdown::new(
                    "menu-stack-dropdown",
                    Button::new("menu-stack-dropdown-trigger").label("Open"),
                    vec![MenuItem::new("one", "One")],
                    dropdown_is_open,
                )
                .id("menu-stack-dropdown")
                .on_open_change({
                    move |open, window, _| {
                        *dropdown_open.borrow_mut() = *open;
                        changes.borrow_mut().push(format!("dropdown:{open}"));
                        window.refresh();
                    }
                }),
            )
            .into_any_element()
    });

    press(cx, "escape");
    assert_eq!(recorded.borrow().as_slice(), ["dropdown:false"]);

    press(cx, "escape");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["dropdown:false", "popover:false"],
        "the second Escape must reach the parent after the menu closes"
    );
}

#[gpui::test]
fn later_sibling_dropdown_handles_escape_first(cx: &mut TestAppContext) {
    still();
    let changes = events();
    let recorded = changes.clone();
    let first_open = Rc::new(RefCell::new(true));
    let second_open = Rc::new(RefCell::new(true));

    let cx = open_host(cx, move || {
        let first_open = first_open.clone();
        let second_open = second_open.clone();
        let changes = changes.clone();
        let first_is_open = *first_open.borrow();
        let second_is_open = *second_open.borrow();
        gpui::div()
            .flex()
            .gap(px(12.))
            .child(
                Dropdown::new(
                    "sibling-menu-first",
                    Button::new("sibling-menu-first-trigger").label("First"),
                    vec![MenuItem::new("first", "First")],
                    first_is_open,
                )
                .id("sibling-menu-first")
                .on_open_change({
                    let changes = changes.clone();
                    move |open, window, _| {
                        *first_open.borrow_mut() = *open;
                        changes.borrow_mut().push(format!("first:{open}"));
                        window.refresh();
                    }
                }),
            )
            .child(
                Dropdown::new(
                    "sibling-menu-second",
                    Button::new("sibling-menu-second-trigger").label("Second"),
                    vec![MenuItem::new("second", "Second")],
                    second_is_open,
                )
                .id("sibling-menu-second")
                .on_open_change({
                    move |open, window, _| {
                        *second_open.borrow_mut() = *open;
                        changes.borrow_mut().push(format!("second:{open}"));
                        window.refresh();
                    }
                }),
            )
            .into_any_element()
    });

    press(cx, "escape");
    assert_eq!(recorded.borrow().as_slice(), ["second:false"]);
    // Siblings do not form a parent/child focus chain. The first menu remains
    // open until its own trigger or panel receives focus; it must not be closed
    // by the same Escape event that dismissed the later sibling.
}

#[gpui::test]
fn tooltip_does_not_close_parent_when_it_is_closed(cx: &mut TestAppContext) {
    still();
    let changes = events();
    let recorded = changes.clone();
    let popover_open = Rc::new(RefCell::new(true));

    let cx = open_host(cx, move || {
        let popover_open = popover_open.clone();
        let changes = changes.clone();
        let popover_is_open = *popover_open.borrow();
        Popover::new(Button::new("tooltip-stack-trigger").label("Outer"))
            .id("tooltip-stack-popover")
            .is_open(popover_is_open)
            .on_open_change({
                move |open, window, _| {
                    *popover_open.borrow_mut() = *open;
                    changes.borrow_mut().push(format!("popover:{open}"));
                    window.refresh();
                }
            })
            .child(
                Tooltip::new("Slow")
                    .delay(60_000)
                    .child(Button::new("tooltip-stack-child").label("Tip")),
            )
            .into_any_element()
    });

    press(cx, "escape");
    assert_eq!(recorded.borrow().as_slice(), ["popover:false"]);
}
