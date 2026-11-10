//! `MenuBar` (HeroGPUI extension): a horizontal bar of Dropdown menus with
//! roving arrow-key focus between the top-level items, keyboard and pointer
//! opening, Left/Right and hover switching while a menu is open, and Escape.

use crate::harness;

use gpui::{point, prelude::*, px, Modifiers, MouseButton, TestAppContext, VisualTestContext};
use harness::{events, open_host, press, still, Events};
use herogpui_components::{MenuBar, MenuBarMenu, MenuItem};

const PANEL: &str = "menu-bar-menu";

fn host(cx: &mut TestAppContext) -> (Events, &mut VisualTestContext) {
    still();
    let seen = events();
    let s = seen.clone();
    let cx = open_host(cx, move || {
        let (a, o) = (s.clone(), s.clone());
        gpui::div()
            .p(px(40.))
            .child(
                MenuBar::new(
                    "bar",
                    vec![
                        MenuBarMenu::new(
                            "file",
                            "File",
                            vec![MenuItem::new("new", "New"), MenuItem::new("open", "Open")],
                        ),
                        MenuBarMenu::new(
                            "edit",
                            "Edit",
                            vec![MenuItem::new("undo", "Undo"), MenuItem::new("redo", "Redo")],
                        )
                        .disabled_keys(["redo"]),
                        MenuBarMenu::new("help", "Help", vec![MenuItem::new("about", "About")])
                            .is_disabled(true),
                        MenuBarMenu::new("view", "View", vec![MenuItem::new("zoom", "Zoom")]),
                    ],
                )
                .on_action(move |menu, item, _, _| {
                    a.borrow_mut().push(format!("action:{menu}/{item}"));
                })
                .on_open_change(move |open, _, _| {
                    o.borrow_mut().push(format!(
                        "open:{}",
                        open.as_ref().map_or("none", |k| k.as_ref())
                    ));
                }),
            )
            .into_any_element()
    });
    (seen, cx)
}

fn frame(cx: &mut VisualTestContext) {
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(300));
    for _ in 0..3 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

fn trigger(cx: &mut VisualTestContext, key: &str) -> gpui::Bounds<gpui::Pixels> {
    cx.debug_bounds(Box::leak(
        format!("menu-bar-trigger-{key}").into_boxed_str(),
    ))
    .expect("trigger laid out")
}

fn panel(cx: &mut VisualTestContext) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.debug_bounds(PANEL)
}

fn click_trigger(cx: &mut VisualTestContext, key: &str) {
    let at = trigger(cx, key).center();
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    frame(cx);
}

fn log(seen: &Events) -> Vec<String> {
    seen.borrow().clone()
}

#[gpui::test]
fn a_press_opens_below_the_trigger_and_again_closes(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx);
    frame(cx);
    assert!(panel(cx).is_none());
    click_trigger(cx, "edit");
    let edit = trigger(cx, "edit");
    let bounds = panel(cx).expect("the Edit menu opened");
    assert_eq!(
        bounds.origin.x, edit.origin.x,
        "start-aligned to its trigger"
    );
    assert!(bounds.origin.y >= edit.bottom(), "below its trigger");
    // The open trigger is outside the menu panel, so pressing it is the
    // panel's outside press: that dismissal closes the menu and stops the
    // press, which never reaches the trigger to reopen it.
    click_trigger(cx, "edit");
    assert!(panel(cx).is_none(), "pressing the open trigger closes it");
    assert_eq!(log(&seen), ["open:edit", "open:none"]);
}

#[gpui::test]
fn arrows_rove_between_triggers_skipping_disabled(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx);
    frame(cx);
    press(cx, "tab");
    frame(cx);
    // Right from File reaches Edit, then skips the disabled Help to View,
    // then wraps to File; Left wraps back to View. Nothing opens.
    press(cx, "right");
    press(cx, "right");
    frame(cx);
    press(cx, "enter");
    frame(cx);
    assert_eq!(log(&seen), ["open:view"], "Enter opened the focused View");
    press(cx, "escape");
    frame(cx);
    assert!(panel(cx).is_none());
    press(cx, "right");
    press(cx, "down");
    frame(cx);
    press(cx, "escape");
    frame(cx);
    press(cx, "home");
    press(cx, "left");
    press(cx, "space");
    frame(cx);
    assert_eq!(
        log(&seen),
        [
            "open:view",
            "open:none",
            "open:file",
            "open:none",
            "open:view"
        ]
    );
}

#[gpui::test]
fn left_and_right_switch_the_open_menu(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx);
    frame(cx);
    press(cx, "tab");
    press(cx, "down");
    frame(cx);
    assert!(panel(cx).is_some());
    press(cx, "right");
    frame(cx);
    let at_edit = panel(cx).expect("switched, still open");
    assert_eq!(at_edit.origin.x, trigger(cx, "edit").origin.x);
    // The switched-to menu has its first item focused: Enter chooses Undo.
    press(cx, "enter");
    frame(cx);
    assert!(panel(cx).is_none(), "choosing closes the bar");
    assert_eq!(
        log(&seen),
        ["open:file", "open:edit", "action:edit/undo", "open:none"]
    );
    // The focus went back to Edit's trigger: Left moves to File.
    press(cx, "left");
    press(cx, "enter");
    frame(cx);
    assert_eq!(log(&seen).last().unwrap(), "open:file");
}

#[gpui::test]
fn hovering_another_trigger_switches_only_while_open(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx);
    frame(cx);
    let view = trigger(cx, "view").center();
    cx.simulate_mouse_move(view, None, Modifiers::none());
    frame(cx);
    assert!(panel(cx).is_none(), "hover alone opens nothing");

    click_trigger(cx, "file");
    let help = trigger(cx, "help").center();
    cx.simulate_mouse_move(help, None, Modifiers::none());
    frame(cx);
    let edit = trigger(cx, "edit").center();
    cx.simulate_mouse_move(edit, None, Modifiers::none());
    frame(cx);
    assert_eq!(
        panel(cx).expect("still open").origin.x,
        trigger(cx, "edit").origin.x
    );
    assert_eq!(log(&seen), ["open:file", "open:edit"], "Help is disabled");
}

#[gpui::test]
fn escape_and_an_outside_press_close(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx);
    frame(cx);
    click_trigger(cx, "file");
    assert!(panel(cx).is_some());
    press(cx, "escape");
    frame(cx);
    assert!(panel(cx).is_none());
    click_trigger(cx, "file");
    cx.simulate_click(point(px(700.), px(500.)), Modifiers::none());
    frame(cx);
    assert!(panel(cx).is_none());
    click_trigger(cx, "help");
    assert!(panel(cx).is_none(), "a disabled item never opens");
    assert_eq!(
        log(&seen),
        ["open:file", "open:none", "open:file", "open:none"]
    );
}

#[gpui::test]
fn an_open_menu_that_becomes_disabled_closes_the_bar(cx: &mut TestAppContext) {
    still();
    let seen = events();
    let s = seen.clone();
    let edit_disabled = std::rc::Rc::new(std::cell::Cell::new(false));
    let d = edit_disabled.clone();
    let cx = open_host(cx, move || {
        let o = s.clone();
        gpui::div()
            .p(px(40.))
            .child(
                MenuBar::new(
                    "bar",
                    vec![
                        MenuBarMenu::new("file", "File", vec![MenuItem::new("new", "New")]),
                        MenuBarMenu::new("edit", "Edit", vec![MenuItem::new("undo", "Undo")])
                            .is_disabled(d.get()),
                        MenuBarMenu::new("view", "View", vec![MenuItem::new("zoom", "Zoom")]),
                    ],
                )
                .on_open_change(move |open, _, _| {
                    o.borrow_mut().push(format!(
                        "open:{}",
                        open.as_ref().map_or("none", |k| k.as_ref())
                    ));
                }),
            )
            .into_any_element()
    });
    frame(cx);
    click_trigger(cx, "edit");
    assert!(panel(cx).is_some());
    edit_disabled.set(true);
    // One frame sees the change and starts the exit motion; the next one
    // lets its timer run out.
    frame(cx);
    frame(cx);
    assert!(panel(cx).is_none());
    assert_eq!(log(&seen), ["open:edit", "open:none"]);
    // The bar is closed for real: hovering another trigger opens nothing.
    let view = trigger(cx, "view").center();
    cx.simulate_mouse_move(view, None, Modifiers::none());
    frame(cx);
    assert!(panel(cx).is_none(), "hover does not reopen a closed bar");
    assert_eq!(log(&seen), ["open:edit", "open:none"]);
}
