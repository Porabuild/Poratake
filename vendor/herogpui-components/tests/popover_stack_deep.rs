//! Deep focus-scope and nested-dismissal contracts for Popover.

use crate::harness;

use std::{cell::Cell, cell::RefCell, rc::Rc, time::Duration};

use gpui::{canvas, point, prelude::*, px, AnyElement, Modifiers, MouseButton, TestAppContext};
use harness::{click, events, open_host, press};
use herogpui_components::{__private, Button, Popover, Tooltip, TooltipHover};

fn still() {
    harness::still();
}

#[gpui::test]
fn popover_body_text_does_not_inherit_host_line_height(cx: &mut TestAppContext) {
    for leading in [None, Some(48.)] {
        still();
        let cx = open_host(cx, move || {
            let mut root = gpui::div();
            if let Some(leading) = leading {
                root = root.text_size(px(32.)).line_height(px(leading));
            }
            root.child(
                Popover::new(Button::new("line-trigger").label("Open"))
                    .id("line-popover")
                    .default_open(true)
                    .title("Heading")
                    .child(
                        gpui::div()
                            .debug_selector(|| "popover-body-text".to_owned())
                            .child("First line\nSecond line"),
                    ),
            )
            .into_any_element()
        });
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("popover-body-text").unwrap().size.height,
            px(40.)
        );
    }
}

fn legacy_phase_probe(key: &'static str, open: bool, seen: harness::Events) -> AnyElement {
    canvas(
        move |_, window, cx| {
            let phase = __private::overlay_phase(window, cx, key, open);
            seen.borrow_mut().push(format!("{phase:?}"));
        },
        |_, _, _, _| {},
    )
    .size_0()
    .into_any_element()
}

fn explicit_phase_probe(key: &'static str, open: bool, seen: harness::Events) -> AnyElement {
    canvas(
        move |_, window, cx| {
            let (phase, _) = __private::overlay_scope(window, cx, key, open, true);
            seen.borrow_mut().push(format!("{phase:?}"));
        },
        |_, _, _, _| {},
    )
    .size_0()
    .into_any_element()
}

fn custom_exit_phase_probe(
    key: &'static str,
    open: bool,
    exit_ms: u64,
    seen: harness::Events,
) -> AnyElement {
    canvas(
        move |_, window, cx| {
            let (phase, _) =
                __private::overlay_scope_with_exit(window, cx, key, open, true, exit_ms);
            seen.borrow_mut().push(format!("{phase:?}"));
        },
        |_, _, _, _| {},
    )
    .size_0()
    .into_any_element()
}

fn nested_tooltip_open_probe(id: &'static str, seen: Rc<RefCell<Vec<bool>>>) -> AnyElement {
    canvas(
        move |_, window, cx| {
            let open = window.with_id(std::any::type_name::<Tooltip>(), |window| {
                window
                    .use_keyed_state(gpui::ElementId::Name(id.into()), cx, |_, _| {
                        TooltipHover::closed()
                    })
                    .read(cx)
                    .is_open()
            });
            seen.borrow_mut().push(open);
        },
        |_, _, _, _| {},
    )
    .size_0()
    .into_any_element()
}

#[gpui::test]
fn legacy_overlay_phase_ignores_a_stale_exit_timer(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let phases = events();
    let seen = phases.clone();
    let render_open = open.clone();
    let cx = open_host(cx, move || {
        legacy_phase_probe("phase-race", render_open.get(), seen.clone())
    });

    open.set(false);
    cx.refresh().unwrap();
    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    assert_eq!(phases.borrow().last().map(String::as_str), Some("Exiting"));

    cx.executor().advance_clock(Duration::from_millis(50));
    open.set(true);
    cx.refresh().unwrap();
    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    assert_eq!(phases.borrow().last().map(String::as_str), Some("Open"));

    open.set(false);
    cx.refresh().unwrap();
    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    assert_eq!(phases.borrow().last().map(String::as_str), Some("Exiting"));

    // The first close's timer has elapsed at t=110ms, but the second close's
    // timer must keep the overlay Exiting until t=150ms.
    cx.executor().advance_clock(Duration::from_millis(60));
    cx.refresh().unwrap();
    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    assert_eq!(phases.borrow().last().map(String::as_str), Some("Exiting"));
}

#[gpui::test]
fn explicit_overlay_scope_ignores_a_stale_exit_timer(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let phases = events();
    let seen = phases.clone();
    let render_open = open.clone();
    let cx = open_host(cx, move || {
        explicit_phase_probe("scope-race", render_open.get(), seen.clone())
    });

    open.set(false);
    cx.refresh().unwrap();
    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    assert_eq!(phases.borrow().last().map(String::as_str), Some("Exiting"));

    cx.executor().advance_clock(Duration::from_millis(50));
    open.set(true);
    cx.refresh().unwrap();
    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    assert_eq!(phases.borrow().last().map(String::as_str), Some("Open"));

    open.set(false);
    cx.refresh().unwrap();
    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    assert_eq!(phases.borrow().last().map(String::as_str), Some("Exiting"));

    cx.executor().advance_clock(Duration::from_millis(60));
    cx.refresh().unwrap();
    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    assert_eq!(phases.borrow().last().map(String::as_str), Some("Exiting"));
}

#[gpui::test]
fn explicit_overlay_scope_honours_a_custom_exit_duration(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let phases = events();
    let seen = phases.clone();
    let render_open = open.clone();
    let cx = open_host(cx, move || {
        custom_exit_phase_probe("scope-custom-exit", render_open.get(), 200, seen.clone())
    });

    open.set(false);
    cx.refresh().unwrap();
    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    assert_eq!(phases.borrow().last().map(String::as_str), Some("Exiting"));

    cx.executor().advance_clock(Duration::from_millis(110));
    cx.refresh().unwrap();
    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    assert_eq!(
        phases.borrow().last().map(String::as_str),
        Some("Exiting"),
        "a 200ms Drawer exit must not be truncated by the shared 100ms lifetime"
    );

    cx.executor().advance_clock(Duration::from_millis(100));
    cx.refresh().unwrap();
    cx.simulate_mouse_move(point(px(1.), px(1.)), None, Modifiers::none());
    assert_eq!(phases.borrow().last().map(String::as_str), Some("Closed"));
}

#[derive(IntoElement)]
struct DecliningOutsideSurface {
    events: harness::Events,
}

impl RenderOnce for DecliningOutsideSurface {
    fn render(self, window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let (_, token) = __private::overlay_scope(
            window,
            cx,
            gpui::ElementId::Name("declining-outside-surface".into()),
            true,
            false,
        );
        let panel = __private::dismiss_on_press_outside_with_token(
            gpui::div()
                .id("declining-outside-panel")
                .absolute()
                .left(px(100.))
                .size(px(100.)),
            token,
            |_, _| __private::DismissResult::Declined,
        );
        gpui::div()
            .relative()
            .size(px(200.))
            .child(
                Button::new("declining-outside-underlying")
                    .label("Underlying")
                    .on_press(move |_, _, _| self.events.borrow_mut().push("underlying".into())),
            )
            .child(panel)
    }
}

#[gpui::test]
fn declined_outside_dismissal_does_not_swallow_underlying_activation(cx: &mut TestAppContext) {
    still();
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        DecliningOutsideSurface {
            events: events.clone(),
        }
        .into_any_element()
    });

    click(cx, 40., 18.);

    assert_eq!(recorded.borrow().as_slice(), ["underlying"]);
}

#[gpui::test]
fn controlled_popover_contains_tab_and_close_activates_with_enter(cx: &mut TestAppContext) {
    still();
    let actions = events();
    let recorded = actions.clone();
    let open = Rc::new(RefCell::new(true));

    let cx = open_host(cx, move || {
        let outside_actions = actions.clone();
        let inside_actions = actions.clone();
        let open_actions = actions.clone();
        let is_open = *open.borrow();

        gpui::div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                Button::new("popover-scope-outside")
                    .label("Outside")
                    .on_press(move |_, _, _| outside_actions.borrow_mut().push("outside".into())),
            )
            .child(
                Popover::new(Button::new("popover-scope-trigger").label("Trigger"))
                    .id("popover-scope")
                    .is_open(is_open)
                    .show_close_button(true)
                    .on_open_change({
                        let open = open.clone();
                        move |value, window, _| {
                            *open.borrow_mut() = *value;
                            open_actions.borrow_mut().push(format!("open:{value}"));
                            window.refresh();
                        }
                    })
                    .child(
                        Button::new("popover-scope-inside")
                            .label("Inside")
                            .on_press(move |_, _, _| {
                                inside_actions.borrow_mut().push("inside".into());
                            }),
                    ),
            )
            .into_any_element()
    });

    // The panel itself owns focus. Its first Tab stop is the built-in close
    // button, followed by the child button, and the third Tab wraps to close.
    press(cx, "tab tab tab");
    press(cx, "enter");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["open:false"],
        "Tab must stay inside the dialog and Enter must activate close exactly once"
    );
}

#[gpui::test]
fn popover_does_not_inject_a_close_button_by_default(cx: &mut TestAppContext) {
    still();
    let actions = events();
    let recorded = actions.clone();
    let cx = open_host(cx, move || {
        let actions = actions.clone();
        Popover::new(Button::new("popover-no-close-trigger").label("Trigger"))
            .id("popover-no-close")
            .default_open(true)
            .child(
                Button::new("popover-no-close-action")
                    .label("Action")
                    .on_press(move |_, _, _| actions.borrow_mut().push("action".into())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["action"],
        "v3 composes close controls explicitly, so the first tab stop must be caller content"
    );
}

#[gpui::test]
fn popover_close_activates_with_space(cx: &mut TestAppContext) {
    still();
    let changes = events();
    let recorded = changes.clone();
    let open = Rc::new(RefCell::new(false));

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Popover::new(Button::new("popover-space-trigger").label("Trigger"))
            .id("popover-space")
            .is_open(*open.borrow())
            .show_close_button(true)
            .on_open_change({
                let open = open.clone();
                move |value, window, _| {
                    *open.borrow_mut() = *value;
                    changes.borrow_mut().push(format!("open:{value}"));
                    window.refresh();
                }
            })
            .into_any_element()
    });

    click(cx, 40., 18.);
    press(cx, "tab");
    press(cx, "space");
    // Closing restores the trigger's focus. Enter therefore opens the same
    // controlled popover again without another pointer press.
    press(cx, "enter");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["open:true", "open:false", "open:true"],
        "Space must close once and focus restoration must return Enter to the trigger"
    );
}

#[gpui::test]
fn clicking_an_open_popover_trigger_reports_one_close(cx: &mut TestAppContext) {
    still();
    let changes = events();
    let recorded = changes.clone();
    let open = Rc::new(RefCell::new(true));

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Popover::new(Button::new("popover-open-trigger-race").label("Trigger"))
            .id("popover-open-trigger-race")
            .is_open(*open.borrow())
            .on_open_change({
                let open = open.clone();
                move |value, window, _| {
                    *open.borrow_mut() = *value;
                    changes.borrow_mut().push(format!("open:{value}"));
                    window.refresh();
                }
            })
            .into_any_element()
    });

    click(cx, 40., 18.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["open:false"],
        "an open trigger owns its close; outside dismissal must not report a second change"
    );
}

#[gpui::test]
fn canceled_popover_trigger_press_does_not_block_a_later_outside_close(cx: &mut TestAppContext) {
    still();
    let changes = events();
    let recorded = changes.clone();
    let open = Rc::new(RefCell::new(true));

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        Popover::new(Button::new("popover-canceled-trigger").label("Trigger"))
            .id("popover-canceled-trigger")
            .is_open(*open.borrow())
            .on_open_change({
                let open = open.clone();
                move |value, window, _| {
                    *open.borrow_mut() = *value;
                    changes.borrow_mut().push(format!("open:{value}"));
                    window.refresh();
                }
            })
            .into_any_element()
    });

    let modifiers = Modifiers::none();
    cx.simulate_mouse_down(point(px(40.), px(18.)), MouseButton::Left, modifiers);
    cx.simulate_mouse_up(point(px(700.), px(500.)), MouseButton::Left, modifiers);
    click(cx, 700., 500.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["open:false"],
        "a canceled trigger press must not leave later outside dismissal declined"
    );
}

#[gpui::test]
fn outside_press_dismisses_without_activating_the_outside_control(cx: &mut TestAppContext) {
    still();
    let actions = events();
    let recorded = actions.clone();
    let open = Rc::new(RefCell::new(true));

    let cx = open_host(cx, move || {
        let outside_actions = actions.clone();
        let open_actions = actions.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(100.))
            .child(
                Button::new("popover-outside-action")
                    .label("Outside")
                    .on_press(move |_, _, _| outside_actions.borrow_mut().push("outside".into())),
            )
            .child(
                Popover::new(Button::new("popover-outside-trigger").label("Trigger"))
                    .id("popover-outside")
                    .is_open(*open.borrow())
                    .on_open_change({
                        let open = open.clone();
                        move |value, window, _| {
                            *open.borrow_mut() = *value;
                            open_actions.borrow_mut().push(format!("open:{value}"));
                            window.refresh();
                        }
                    }),
            )
            .into_any_element()
    });

    click(cx, 40., 18.);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["open:false"],
        "the dismissing outside press must not also activate the covered page control"
    );
}

#[gpui::test]
fn nested_popover_escape_closes_only_the_topmost_overlay(cx: &mut TestAppContext) {
    still();
    let popover_changes = events();
    let popover_recorded = popover_changes.clone();
    let outer_changes = events();
    let outer_recorded = outer_changes.clone();
    let popover_open = Rc::new(RefCell::new(true));
    let outer_open = Rc::new(RefCell::new(true));

    let cx = open_host(cx, move || {
        let popover_changes = popover_changes.clone();
        let outer_changes = outer_changes.clone();
        let is_popover_open = *popover_open.borrow();
        let is_outer_open = *outer_open.borrow();

        Popover::new(Button::new("popover-stack-outer-trigger").label("Outer trigger"))
            .id("popover-stack-outer")
            .is_open(is_outer_open)
            .on_open_change({
                let outer_open = outer_open.clone();
                move |value, window, _| {
                    *outer_open.borrow_mut() = *value;
                    outer_changes.borrow_mut().push(format!("outer:{value}"));
                    window.refresh();
                }
            })
            .child(
                Popover::new(Button::new("popover-stack-trigger").label("Trigger"))
                    .id("popover-stack-popover")
                    .is_open(is_popover_open)
                    .on_open_change({
                        let popover_open = popover_open.clone();
                        move |value, window, _| {
                            *popover_open.borrow_mut() = *value;
                            popover_changes
                                .borrow_mut()
                                .push(format!("popover:{value}"));
                            window.refresh();
                        }
                    })
                    .child(Button::new("popover-stack-action").label("Action")),
            )
            .into_any_element()
    });

    press(cx, "escape");
    assert_eq!(
        popover_recorded.borrow().as_slice(),
        ["popover:false"],
        "the first Escape must close the inner popover exactly once"
    );
    assert!(
        outer_recorded.borrow().is_empty(),
        "the inner Escape must not propagate into the outer popover"
    );

    press(cx, "escape");
    assert_eq!(
        outer_recorded.borrow().as_slice(),
        ["outer:false"],
        "a second Escape may close the still-open outer popover"
    );
    assert_eq!(
        popover_recorded.borrow().as_slice(),
        ["popover:false"],
        "closing the outer modal must not report a second popover close"
    );
}

#[gpui::test]
fn nested_popover_outside_press_closes_only_the_topmost_overlay(cx: &mut TestAppContext) {
    still();
    let popover_changes = events();
    let popover_recorded = popover_changes.clone();
    let outer_changes = events();
    let outer_recorded = outer_changes.clone();
    let popover_open = Rc::new(RefCell::new(true));
    let outer_open = Rc::new(RefCell::new(true));

    let cx = open_host(cx, move || {
        let popover_changes = popover_changes.clone();
        let outer_changes = outer_changes.clone();
        let is_popover_open = *popover_open.borrow();
        let is_outer_open = *outer_open.borrow();

        Popover::new(Button::new("popover-outside-stack-outer-trigger").label("Outer trigger"))
            .id("popover-outside-stack-outer")
            .is_open(is_outer_open)
            .on_open_change({
                let outer_open = outer_open.clone();
                move |value, window, _| {
                    *outer_open.borrow_mut() = *value;
                    outer_changes.borrow_mut().push(format!("outer:{value}"));
                    window.refresh();
                }
            })
            .child(
                Popover::new(Button::new("popover-outside-stack-trigger").label("Trigger"))
                    .id("popover-outside-stack-popover")
                    .is_open(is_popover_open)
                    .on_open_change({
                        let popover_open = popover_open.clone();
                        move |value, window, _| {
                            *popover_open.borrow_mut() = *value;
                            popover_changes
                                .borrow_mut()
                                .push(format!("popover:{value}"));
                            window.refresh();
                        }
                    })
                    .child(Button::new("popover-outside-stack-action").label("Action")),
            )
            .into_any_element()
    });

    // This is outside both panels. Capture listeners run in registration
    // order, so propagation alone would close the wrong layer.
    click(cx, 1000., 100.);
    assert_eq!(
        popover_recorded.borrow().as_slice(),
        ["popover:false"],
        "the first outside press must close only the inner popover"
    );
    assert!(
        outer_recorded.borrow().is_empty(),
        "the first outside press must not close the outer popover"
    );

    click(cx, 1000., 100.);
    assert_eq!(
        outer_recorded.borrow().as_slice(),
        ["outer:false"],
        "the second outside press may close the remaining outer popover"
    );
}

#[gpui::test]
fn closed_tooltip_does_not_swallow_parent_popover_escape(cx: &mut TestAppContext) {
    still();
    let popover_changes = events();
    let popover_recorded = popover_changes.clone();
    let popover_open = Rc::new(RefCell::new(true));

    let cx = open_host(cx, move || {
        let popover_changes = popover_changes.clone();
        let is_popover_open = *popover_open.borrow();
        Popover::new(Button::new("popover-closed-tooltip-trigger").label("Trigger"))
            .id("popover-closed-tooltip-popover")
            .is_open(is_popover_open)
            .on_open_change({
                let popover_open = popover_open.clone();
                move |value, window, _| {
                    *popover_open.borrow_mut() = *value;
                    popover_changes
                        .borrow_mut()
                        .push(format!("popover:{value}"));
                    window.refresh();
                }
            })
            .child(
                Tooltip::new("Delayed tip")
                    .delay(60_000)
                    .child(Button::new("popover-closed-tooltip-tooltip-trigger").label("Tip")),
            )
            .into_any_element()
    });

    // The trigger receives focus, but the long hover delay keeps this tooltip
    // closed. Escape must therefore reach the visible modal below it.
    click(cx, 960., 540.);
    press(cx, "escape");

    assert_eq!(
        popover_recorded.borrow().as_slice(),
        ["popover:false"],
        "a closed tooltip must not consume its parent popover's Escape"
    );
}

#[gpui::test]
fn open_tooltip_inside_popover_answers_escape_before_its_parent(cx: &mut TestAppContext) {
    still();
    let popover_changes = events();
    let popover_recorded = popover_changes.clone();
    let popover_open = Rc::new(RefCell::new(true));
    let tooltip_states = Rc::new(RefCell::new(Vec::<bool>::new()));
    let tooltip_seen = tooltip_states.clone();

    let cx = open_host(cx, move || {
        let popover_changes = popover_changes.clone();
        let is_popover_open = *popover_open.borrow();
        Popover::new(Button::new("popover-open-tooltip-trigger").label("Trigger"))
            .id("popover-open-tooltip-popover")
            .is_open(is_popover_open)
            .show_close_button(false)
            .on_open_change({
                let popover_open = popover_open.clone();
                move |value, window, _| {
                    *popover_open.borrow_mut() = *value;
                    popover_changes
                        .borrow_mut()
                        .push(format!("popover:{value}"));
                    window.refresh();
                }
            })
            .child(nested_tooltip_open_probe(
                "popover-open-tooltip-tip",
                tooltip_seen.clone(),
            ))
            .child(
                Tooltip::new("Open tip")
                    .id("popover-open-tooltip-tip")
                    .delay(0)
                    .child(gpui::div().w(px(120.)).h(px(36.)).child("Tip trigger")),
            )
            .into_any_element()
    });

    let mut opened = false;
    'rows: for y in (10..1080).step_by(20) {
        for x in (10..1920).step_by(20) {
            cx.simulate_mouse_move(point(px(x as f32), px(y as f32)), None, Modifiers::none());
            cx.refresh().unwrap();
            if tooltip_states.borrow().last().copied() == Some(true) {
                opened = true;
                break 'rows;
            }
        }
    }
    assert!(
        opened,
        "the pointer sweep must open the nested tooltip; last states: {:?}",
        tooltip_states.borrow().last()
    );

    press(cx, "escape");
    // The old defect also left the parent open after one Escape; the second
    // assertion is what proves the tooltip actually yielded the stack slot.
    assert!(
        popover_recorded.borrow().is_empty(),
        "the first Escape belongs to the open tooltip"
    );
    press(cx, "escape");
    assert_eq!(
        popover_recorded.borrow().as_slice(),
        ["popover:false"],
        "after the tooltip consumes one Escape, the next must close its parent"
    );
}

/// v3 portals the popover above the page: an open panel must paint after
/// later siblings, not in tree order underneath them. The sibling below is
/// opaque and covers the whole panel region, so tree-order painting would
/// hide the panel completely.
#[gpui::test]
fn open_popover_panel_paints_after_later_siblings(cx: &mut TestAppContext) {
    still();
    let cover = gpui::rgb(0xFF00FF);
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                Popover::new(Button::new("cover-trigger").label("Open"))
                    .id("cover-popover")
                    .default_open(true)
                    .title("Panel")
                    .child(gpui::div().child("panel body")),
            )
            .child(
                gpui::div()
                    .w(px(600.))
                    .h(px(300.))
                    .bg(cover)
                    .debug_selector(|| "cover-sibling".to_owned()),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    let scene = harness::painted(cx);
    let (panel_bg, cover_hsla) = cx.update(|_, cx| {
        use herogpui_theme::ActiveTheme;
        (cx.colors().overlay.background, gpui::Hsla::from(cover))
    });
    let cover_index = scene
        .quads
        .iter()
        .position(|q| q.background.as_solid() == Some(cover_hsla))
        .expect("the sibling must paint its cover");
    // The panel is ~260px wide; filter out any full-window background quad
    // that happens to share the overlay colour.
    let panel_indices: Vec<usize> = scene
        .quads
        .iter()
        .enumerate()
        .filter(|(_, q)| {
            q.background.as_solid() == Some(panel_bg) && {
                let w = f32::from(scene.bounds(q).size.width);
                (200.0..500.0).contains(&w)
            }
        })
        .map(|(i, _)| i)
        .collect();
    assert!(
        !panel_indices.is_empty(),
        "the open panel must paint its background\n{}",
        scene.describe()
    );
    assert!(
        panel_indices.iter().all(|&i| i > cover_index),
        "every panel-background quad must paint after the later sibling\n{}",
        scene.describe()
    );
}
