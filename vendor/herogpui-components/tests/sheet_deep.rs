//! HeroUI v3.2.7 Sheet state, focus and snap-position contracts.
use std::{cell::Cell, rc::Rc};

use gpui::{point, prelude::*, px, Modifiers, MouseButton, TestAppContext, VisualTestContext};
use herogpui_components::{Button, CloseButton, Sheet};

use crate::harness::{self, click, open_host, press};

fn flush(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

#[gpui::test]
fn uncontrolled_trigger_and_explicit_close_work(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, || {
        Sheet::new("sheet")
            .trigger(Button::new("open").label("Open"))
            .close_trigger(CloseButton::new("close"))
            .title("Details")
            .child("Body")
            .into_any_element()
    });
    assert!(cx.debug_bounds("sheet-dialog").is_none());
    click(cx, 20., 16.);
    flush(cx);
    let panel = cx
        .debug_bounds("sheet-dialog")
        .expect("trigger must open the sheet");
    click(
        cx,
        f32::from(panel.right()) - 24.,
        f32::from(panel.top()) + 24.,
    );
    flush(cx);
    assert!(cx.debug_bounds("sheet-dialog").is_none());
}

#[gpui::test]
fn controlled_sheet_reports_requests_without_mutating_its_prop(cx: &mut TestAppContext) {
    harness::still();
    let requests = harness::events();
    let seen = requests.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        Sheet::new("controlled")
            .is_open(true)
            .title("Controlled")
            .on_open_change(move |open, _, _| seen.borrow_mut().push(open.to_string()))
            .into_any_element()
    });
    press(cx, "escape");
    flush(cx);
    assert_eq!(&*requests.borrow(), &["false"]);
    assert!(
        cx.debug_bounds("sheet-dialog").is_some(),
        "the caller still owns true"
    );
}

#[gpui::test]
fn non_dismissible_sheet_blocks_escape_and_outside_but_allows_close(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, || {
        Sheet::new("fixed")
            .default_open(true)
            .is_dismissible(false)
            .title("Required")
            .close_trigger(CloseButton::new("close"))
            .child("Body")
            .into_any_element()
    });
    press(cx, "escape");
    click(cx, 10., 10.);
    flush(cx);
    let panel = cx
        .debug_bounds("sheet-dialog")
        .expect("dismissal must be blocked");
    click(
        cx,
        f32::from(panel.right()) - 24.,
        f32::from(panel.top()) + 24.,
    );
    flush(cx);
    assert!(cx.debug_bounds("sheet-dialog").is_none());
}

#[gpui::test]
fn outside_press_closes_uncontrolled_sheet(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, || {
        Sheet::new("outside")
            .default_open(true)
            .title("Details")
            .child("Body")
            .into_any_element()
    });
    click(cx, 10., 10.);
    flush(cx);
    assert!(cx.debug_bounds("sheet-dialog").is_none());
}

#[gpui::test]
fn handle_drag_expands_a_partial_sheet_and_reports_live_state(cx: &mut TestAppContext) {
    harness::still();
    let expanded = Rc::new(Cell::new(false));
    let seen = expanded.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        Sheet::new("snap")
            .default_open(true)
            .snap_points([200.])
            .content(move |state| {
                seen.set(state.is_expanded);
                gpui::div().child("Body").into_any_element()
            })
            .into_any_element()
    });
    assert!(!expanded.get());
    let handle = cx.debug_bounds("sheet-handle").unwrap();
    let from = handle.center();
    let to = point(from.x, px(0.));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    flush(cx);
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    flush(cx);
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    flush(cx);
    assert!(expanded.get());
}

#[gpui::test]
fn nested_escape_closes_only_the_top_sheet(cx: &mut TestAppContext) {
    harness::still();
    let outer_closed = Rc::new(Cell::new(0));
    let inner_closed = Rc::new(Cell::new(0));
    let outer = outer_closed.clone();
    let inner = inner_closed.clone();
    let cx = open_host(cx, move || {
        let outer = outer.clone();
        let inner = inner.clone();
        Sheet::new("parent")
            .default_open(true)
            .title("Parent")
            .on_close(move |_, _, _| outer.set(outer.get() + 1))
            .nested_child(
                Sheet::new("child")
                    .default_open(true)
                    .title("Child")
                    .on_close(move |_, _, _| inner.set(inner.get() + 1)),
            )
            .into_any_element()
    });
    press(cx, "escape");
    flush(cx);
    assert_eq!(inner_closed.get(), 1);
    assert_eq!(outer_closed.get(), 0);
    press(cx, "escape");
    flush(cx);
    assert_eq!(outer_closed.get(), 1);
}

#[gpui::test]
fn close_button_preserves_its_own_handler_and_disabled_state(cx: &mut TestAppContext) {
    harness::still();
    let hits = Rc::new(Cell::new(0));
    let seen = hits.clone();
    let disabled = Rc::new(Cell::new(true));
    let locked = disabled.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        Sheet::new("close")
            .default_open(true)
            .title("Details")
            .close_trigger(
                CloseButton::new("explicit-close")
                    .is_disabled(locked.get())
                    .aria_label("Dismiss details")
                    .on_press(move |_, _, _| seen.set(seen.get() + 1)),
            )
            .into_any_element()
    });
    let panel = cx.debug_bounds("sheet-dialog").unwrap();
    let x = f32::from(panel.right()) - 24.;
    let y = f32::from(panel.top()) + 24.;
    click(cx, x, y);
    flush(cx);
    assert_eq!(hits.get(), 0);
    disabled.set(false);
    flush(cx);
    click(cx, x, y);
    flush(cx);
    assert_eq!(hits.get(), 1);
    assert!(cx.debug_bounds("sheet-dialog").is_none());
}

#[gpui::test]
fn disabled_trigger_and_close_action_preserve_disabled_semantics(cx: &mut TestAppContext) {
    harness::still();
    let enabled = Rc::new(Cell::new(false));
    let unlocked = enabled.clone();
    let cx = open_host(cx, move || {
        Sheet::new("disabled-parts")
            .trigger(
                Button::new("disabled-open")
                    .label("Open")
                    .is_disabled(!unlocked.get()),
            )
            .close_child(
                Button::new("disabled-close")
                    .label("Done")
                    .is_disabled(true),
            )
            .title("Details")
            .into_any_element()
    });
    click(cx, 20., 16.);
    flush(cx);
    assert!(cx.debug_bounds("sheet-dialog").is_none());
    enabled.set(true);
    flush(cx);
    click(cx, 20., 16.);
    flush(cx);
    let panel = cx.debug_bounds("sheet-dialog").unwrap();
    click(
        cx,
        f32::from(panel.right()) - 40.,
        f32::from(panel.bottom()) - 32.,
    );
    flush(cx);
    assert!(cx.debug_bounds("sheet-dialog").is_some());
}

#[gpui::test]
fn nested_trigger_lives_inside_its_parent_dialog(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, || {
        Sheet::new("parent-geometry")
            .default_open(true)
            .title("Parent")
            .nested_child(Sheet::new("nested-geometry").trigger(
                Button::new("nested-trigger").child(harness::probe("nested-trigger-content")),
            ))
            .into_any_element()
    });
    let parent = cx.debug_bounds("sheet-dialog").unwrap();
    let child = cx.debug_bounds("nested-trigger-content").unwrap();
    assert!(parent.contains(&child.center()));
    click(cx, f32::from(child.center().x), f32::from(child.center().y));
    flush(cx);
    press(cx, "escape");
    flush(cx);
    assert!(
        cx.debug_bounds("sheet-dialog").is_some(),
        "only the child closes"
    );
}

#[gpui::test]
fn top_partial_snap_keeps_the_handle_visible(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, || {
        Sheet::new("top-snap")
            .default_open(true)
            .placement(herogpui_components::DrawerPlacement::Top)
            .snap_points([200.])
            .title("Top")
            .into_any_element()
    });
    let handle = cx.debug_bounds("sheet-handle").unwrap();
    assert!(handle.top() >= px(0.) && handle.bottom() <= px(200.));
}

#[gpui::test]
fn ignored_full_size_sentinel_does_not_force_snap_layout(cx: &mut TestAppContext) {
    harness::still();
    let sentinel = Rc::new(Cell::new(false));
    let has_sentinel = sentinel.clone();
    let cx = open_host(cx, move || {
        Sheet::new("sentinel")
            .default_open(true)
            .title("Natural")
            .snap_points(if has_sentinel.get() { vec![1.] } else { vec![] })
            .child("Body")
            .into_any_element()
    });
    let natural = cx.debug_bounds("sheet-dialog").unwrap().size.height;
    sentinel.set(true);
    flush(cx);
    assert_eq!(
        cx.debug_bounds("sheet-dialog").unwrap().size.height,
        natural
    );
}

#[gpui::test]
fn dragging_preserves_child_keyed_identity(cx: &mut TestAppContext) {
    let allocations = Rc::new(Cell::new(0));
    let held = allocations.clone();
    let cx = open_host(cx, move || {
        let held = held.clone();
        Sheet::new("stateful")
            .default_open(true)
            .snap_points([200.])
            .child(
                gpui::canvas(
                    move |_, window, cx| {
                        window.use_keyed_state("child-state", cx, move |_, _| {
                            held.set(held.get() + 1);
                            false
                        });
                    },
                    |_, _, _, _| {},
                )
                .h(px(20.)),
            )
            .into_any_element()
    });
    harness::wait_ms(cx, 550);
    let before = allocations.get();
    let handle = cx.debug_bounds("sheet-handle").unwrap().center();
    cx.simulate_mouse_down(handle, MouseButton::Left, Modifiers::none());
    flush(cx);
    cx.simulate_mouse_move(
        point(handle.x, handle.y - px(100.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    flush(cx);
    cx.simulate_mouse_up(
        point(handle.x, handle.y - px(100.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    flush(cx);
    assert_eq!(
        allocations.get(),
        before,
        "snap motion must preserve the child's keyed state"
    );
}

#[gpui::test]
fn drag_start_cancels_the_pending_enter_completion(cx: &mut TestAppContext) {
    let ends = Rc::new(Cell::new(0));
    let seen = ends.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        Sheet::new("interrupted")
            .default_open(true)
            .snap_points([200.])
            .on_animation_end(move |_, _, _| seen.set(seen.get() + 1))
            .into_any_element()
    });
    harness::wait_ms(cx, 100);
    let handle = cx.debug_bounds("sheet-handle").unwrap().center();
    cx.simulate_mouse_down(handle, MouseButton::Left, Modifiers::none());
    flush(cx);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(600));
    flush(cx);
    assert_eq!(
        ends.get(),
        0,
        "an interrupted enter must not report completion"
    );
}

#[gpui::test]
fn lost_pointer_release_restores_the_starting_detent(cx: &mut TestAppContext) {
    harness::still();
    let position = Rc::new(Cell::new(0.));
    let seen = position.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        Sheet::new("cancel-drag")
            .default_open(true)
            .snap_points([200., 400.])
            .content(move |state| {
                seen.set(state.position);
                gpui::div().child("Body").into_any_element()
            })
            .into_any_element()
    });
    let handle = cx.debug_bounds("sheet-handle").unwrap().center();
    let to = point(handle.x, handle.y - px(100.));
    cx.simulate_mouse_down(handle, MouseButton::Left, Modifiers::none());
    flush(cx);
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    flush(cx);
    assert!((position.get() - 300_f32).abs() < 0.5);
    cx.simulate_mouse_move(to, None, Modifiers::none());
    flush(cx);
    assert!(
        (position.get() - 200_f32).abs() < 0.5,
        "cancel must restore the starting snap"
    );
}

#[gpui::test]
fn fractional_snap_tracks_the_resized_viewport(cx: &mut TestAppContext) {
    harness::still();
    let position = Rc::new(Cell::new(0.));
    let seen = position.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        Sheet::new("resize-fraction")
            .default_open(true)
            .snap_points([herogpui_components::SheetSnapPoint::Fraction(0.5)])
            .content(move |state| {
                seen.set(state.position);
                gpui::div().child("Body").into_any_element()
            })
            .into_any_element()
    });
    cx.simulate_resize(gpui::size(px(600.), px(600.)));
    flush(cx);
    assert!((position.get() - 300_f32).abs() < 0.5);
    cx.simulate_resize(gpui::size(px(600.), px(800.)));
    flush(cx);
    assert!((position.get() - 400_f32).abs() < 0.5);
}

#[gpui::test]
fn expanded_snap_stays_expanded_after_resize(cx: &mut TestAppContext) {
    harness::still();
    let expanded = Rc::new(Cell::new(false));
    let seen = expanded.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        Sheet::new("resize-full")
            .default_open(true)
            .snap_points([200.])
            .content(move |state| {
                seen.set(state.is_expanded);
                gpui::div().child("Body").into_any_element()
            })
            .into_any_element()
    });
    cx.simulate_resize(gpui::size(px(600.), px(600.)));
    flush(cx);
    let from = cx.debug_bounds("sheet-handle").unwrap().center();
    let to = point(from.x, px(0.));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    flush(cx);
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    flush(cx);
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    flush(cx);
    assert!(expanded.get());
    cx.simulate_resize(gpui::size(px(600.), px(800.)));
    flush(cx);
    assert!(
        expanded.get(),
        "an expanded body must keep scrolling after resize"
    );
}

#[gpui::test]
fn only_the_original_last_snap_is_a_legacy_sentinel(cx: &mut TestAppContext) {
    harness::still();
    let position = Rc::new(Cell::new(0.));
    let seen = position.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        Sheet::new("double-sentinel")
            .default_open(true)
            .snap_points([1., 1.])
            .content(move |state| {
                seen.set(state.position);
                gpui::div().child("Body").into_any_element()
            })
            .into_any_element()
    });
    flush(cx);
    assert!((position.get() - 1_f32).abs() < 0.01);
}

#[gpui::test]
fn exiting_content_cannot_receive_pointer_or_keyboard_actions(cx: &mut TestAppContext) {
    let body_hits = Rc::new(Cell::new(0));
    let seen = body_hits.clone();
    let after_hits = Rc::new(Cell::new(0));
    let after = after_hits.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        let after = after.clone();
        gpui::div()
            .flex()
            .flex_col()
            .child(
                Sheet::new("exit-inert")
                    .trigger(Button::new("exit-open").label("Open"))
                    .child(
                        Button::new("exit-body")
                            .child(harness::probe("exit-body-probe"))
                            .on_press(move |_, _, _| seen.set(seen.get() + 1)),
                    ),
            )
            .child(
                Button::new("after-sheet")
                    .label("After")
                    .on_press(move |_, _, _| after.set(after.get() + 1)),
            )
            .into_any_element()
    });
    click(cx, 20., 16.);
    flush(cx);
    harness::wait_ms(cx, 550);
    let body = cx.debug_bounds("exit-body-probe").unwrap().center();
    press(cx, "escape");
    flush(cx);
    assert!(
        cx.debug_bounds("sheet-dialog").is_some(),
        "exit keeps the surface visible"
    );
    assert!(
        cx.debug_bounds("exit-body-probe").is_none(),
        "closed content must leave the interaction tree"
    );
    click(cx, body.x.into(), body.y.into());
    flush(cx);
    press(cx, "tab");
    press(cx, "enter");
    flush(cx);
    assert_eq!(body_hits.get(), 0);
    assert_eq!(after_hits.get(), 1);
}

#[gpui::test]
fn a_nonterminal_full_fraction_opens_fully_expanded(cx: &mut TestAppContext) {
    harness::still();
    let expanded = Rc::new(Cell::new(false));
    let seen = expanded.clone();
    let cx = open_host(cx, move || {
        let seen = seen.clone();
        Sheet::new("first-full")
            .default_open(true)
            .snap_points([
                herogpui_components::SheetSnapPoint::Fraction(1.),
                herogpui_components::SheetSnapPoint::Fraction(0.5),
            ])
            .content(move |state| {
                seen.set(state.is_expanded);
                gpui::div().child("Body").into_any_element()
            })
            .into_any_element()
    });
    flush(cx);
    assert!(expanded.get());
}
