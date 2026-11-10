//! Explicit overlay-stack behavior for Modal, Drawer, and AlertDialog.

use crate::harness;

use std::{cell::RefCell, rc::Rc};

use gpui::{prelude::*, px, TestAppContext};
use harness::{click, events, open_host, press};
use herogpui_components::{AlertDialog, Drawer, DrawerPlacement, Modal};

fn still() {
    harness::still();
}

#[gpui::test]
fn window_dialogs_escape_clipped_ancestors_and_later_siblings(cx: &mut TestAppContext) {
    for kind in ["modal", "drawer", "alert"] {
        still();
        let hits = events();
        let recorded = hits.clone();
        let cx = open_host(cx, move || {
            let content_hits = hits.clone();
            let close_hits = hits.clone();
            let background_hits = hits.clone();
            let content = gpui::div()
                .id("window-dialog-content")
                .debug_selector(|| "window-dialog-content".to_owned())
                .w_full()
                .h(px(36.))
                .flex_shrink_0()
                .on_click(move |_, _, _| content_hits.borrow_mut().push("content".into()))
                .child("Dialog content");
            let on_open_change = move |open: &bool, _: &mut gpui::Window, _: &mut gpui::App| {
                close_hits.borrow_mut().push(format!("open:{open}"));
            };
            let dialog = match kind {
                "modal" => Modal::new()
                    .is_open(true)
                    .is_dismissible(true)
                    .child(content)
                    .on_open_change(on_open_change)
                    .into_any_element(),
                "drawer" => Drawer::new()
                    .is_open(true)
                    .placement(DrawerPlacement::Right)
                    .is_dismissible(true)
                    .child(content)
                    .on_open_change(on_open_change)
                    .into_any_element(),
                _ => AlertDialog::new("Confirm")
                    .is_open(true)
                    .is_dismissible(true)
                    .child(content)
                    .on_open_change(on_open_change)
                    .into_any_element(),
            };
            gpui::div()
                .relative()
                .size_full()
                .child(
                    gpui::div()
                        .relative()
                        .ml(px(100.))
                        .mt(px(100.))
                        .w(px(240.))
                        .h(px(180.))
                        .overflow_hidden()
                        .child(dialog),
                )
                .child(
                    gpui::div()
                        .id("later-background")
                        .absolute()
                        .inset_0()
                        .occlude()
                        .on_click(move |_, _, _| {
                            background_hits.borrow_mut().push("background".into());
                        }),
                )
                .into_any_element()
        });
        let bounds = cx.debug_bounds("window-dialog-content").unwrap();
        let viewport = cx.update(|window, _| window.viewport_size());
        if kind == "drawer" {
            assert!(bounds.left() > viewport.width / 2., "{kind}: {bounds:?}");
        } else {
            assert!(
                bounds.left() < viewport.width / 2. && bounds.right() > viewport.width / 2.,
                "{kind}: content must span the window center, got {bounds:?}"
            );
        }
        click(
            cx,
            f32::from(bounds.center().x),
            f32::from(bounds.center().y),
        );
        assert_eq!(recorded.borrow().as_slice(), ["content"], "{kind}");
        click(cx, 10., 10.);
        assert_eq!(
            recorded.borrow().as_slice(),
            ["content", "open:false"],
            "{kind}"
        );
    }
}

#[gpui::test]
fn nested_modals_escape_only_the_topmost_then_reaches_parent(cx: &mut TestAppContext) {
    still();
    let outer_open = Rc::new(RefCell::new(true));
    let inner_open = Rc::new(RefCell::new(true));
    let changes = events();

    let cx = open_host(cx, {
        let outer_open = outer_open.clone();
        let changes = changes.clone();
        move || {
            let outer_open_change = outer_open.clone();
            let inner_open_change = inner_open.clone();
            let outer_changes = changes.clone();
            let inner_changes = changes.clone();
            Modal::new()
                .id("dialog-stack-outer")
                .is_open(*outer_open.borrow())
                .is_keyboard_dismiss_disabled(false)
                .on_open_change(move |open, window, _| {
                    *outer_open_change.borrow_mut() = *open;
                    outer_changes.borrow_mut().push(format!("outer:{open}"));
                    window.refresh();
                })
                .child(
                    Modal::new()
                        .id("dialog-stack-inner")
                        .is_open(*inner_open.borrow())
                        .is_keyboard_dismiss_disabled(false)
                        .on_open_change(move |open, window, _| {
                            *inner_open_change.borrow_mut() = *open;
                            inner_changes.borrow_mut().push(format!("inner:{open}"));
                            window.refresh();
                        }),
                )
                .into_any_element()
        }
    });

    press(cx, "escape");
    assert_eq!(changes.borrow().as_slice(), ["inner:false"]);
    assert!(*outer_open.borrow());

    press(cx, "escape");
    assert_eq!(changes.borrow().as_slice(), ["inner:false", "outer:false"]);
}

#[gpui::test]
fn nested_modal_and_drawer_outside_press_closes_only_topmost_once(cx: &mut TestAppContext) {
    still();
    let outer_open = Rc::new(RefCell::new(true));
    let drawer_open = Rc::new(RefCell::new(true));
    let changes = events();

    let cx = open_host(cx, {
        let outer_open = outer_open.clone();
        let changes = changes.clone();
        move || {
            let outer_open_change = outer_open.clone();
            let drawer_open_change = drawer_open.clone();
            let outer_changes = changes.clone();
            let drawer_changes = changes.clone();
            Modal::new()
                .id("dialog-outside-outer")
                .is_open(*outer_open.borrow())
                .on_open_change(move |open, window, _| {
                    *outer_open_change.borrow_mut() = *open;
                    outer_changes.borrow_mut().push(format!("outer:{open}"));
                    window.refresh();
                })
                .child(
                    Drawer::new()
                        .id("dialog-outside-drawer")
                        .is_open(*drawer_open.borrow())
                        .on_open_change(move |open, window, _| {
                            *drawer_open_change.borrow_mut() = *open;
                            drawer_changes.borrow_mut().push(format!("drawer:{open}"));
                            window.refresh();
                        }),
                )
                .into_any_element()
        }
    });

    click(cx, 10., 10.);
    assert_eq!(changes.borrow().as_slice(), ["drawer:false"]);
    assert!(*outer_open.borrow());

    click(cx, 10., 10.);
    assert_eq!(changes.borrow().as_slice(), ["drawer:false", "outer:false"]);
}

#[gpui::test]
fn alert_dialog_escape_is_topmost_and_does_not_double_report(cx: &mut TestAppContext) {
    still();
    let modal_open = Rc::new(RefCell::new(true));
    let alert_open = Rc::new(RefCell::new(true));
    let changes = events();

    let cx = open_host(cx, {
        let changes = changes.clone();
        move || {
            let modal_open_change = modal_open.clone();
            let alert_open_change = alert_open.clone();
            let modal_changes = changes.clone();
            let alert_changes = changes.clone();
            Modal::new()
                .id("alert-stack-modal")
                .is_open(*modal_open.borrow())
                .is_keyboard_dismiss_disabled(false)
                .on_open_change(move |open, window, _| {
                    *modal_open_change.borrow_mut() = *open;
                    modal_changes.borrow_mut().push(format!("modal:{open}"));
                    window.refresh();
                })
                .child(
                    AlertDialog::new("Confirm")
                        .id("alert-stack-alert")
                        .is_open(*alert_open.borrow())
                        .is_dismissible(true)
                        .is_keyboard_dismiss_disabled(false)
                        .on_open_change(move |open, window, _| {
                            *alert_open_change.borrow_mut() = *open;
                            alert_changes.borrow_mut().push(format!("alert:{open}"));
                            window.refresh();
                        }),
                )
                .into_any_element()
        }
    });

    press(cx, "escape");
    assert_eq!(changes.borrow().as_slice(), ["alert:false"]);

    press(cx, "escape");
    assert_eq!(changes.borrow().as_slice(), ["alert:false", "modal:false"]);
}
