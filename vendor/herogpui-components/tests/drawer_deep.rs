//! Behaviour contracts specific to HeroUI v3.2.4's Drawer anatomy.

use crate::harness;

use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    point, prelude::*, px, Modifiers, MouseButton, ScrollDelta, ScrollEvent, TestAppContext,
    VisualTestContext,
};
use harness::{click, events, open_host, painted, press, Events};
use herogpui_components::{Button, Drawer, DrawerHandle, DrawerPlacement};
use herogpui_theme::ActiveTheme;

fn still() {
    harness::still();
}

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn drag(cx: &mut VisualTestContext, from: (f32, f32), to: (f32, f32)) {
    let modifiers = Modifiers::none();
    cx.simulate_mouse_down(point(px(from.0), px(from.1)), MouseButton::Left, modifiers);
    flush_frame(cx);
    cx.simulate_mouse_move(point(px(to.0), px(to.1)), MouseButton::Left, modifiers);
    flush_frame(cx);
    cx.simulate_mouse_up(point(px(to.0), px(to.1)), MouseButton::Left, modifiers);
    flush_frame(cx);
}

fn slow_drag(cx: &mut VisualTestContext, from: (f32, f32), to: (f32, f32)) {
    let modifiers = Modifiers::none();
    cx.simulate_mouse_down(point(px(from.0), px(from.1)), MouseButton::Left, modifiers);
    flush_frame(cx);
    std::thread::sleep(Duration::from_millis(100));
    cx.simulate_mouse_move(point(px(to.0), px(to.1)), MouseButton::Left, modifiers);
    flush_frame(cx);
    std::thread::sleep(Duration::from_millis(100));
    cx.simulate_mouse_up(point(px(to.0), px(to.1)), MouseButton::Left, modifiers);
    flush_frame(cx);
}

fn press_backward(cx: &mut VisualTestContext) {
    cx.simulate_keystrokes("shift+tab");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("shift+tab").unwrap(),
    });
}

fn wheel(cx: &mut VisualTestContext, x: f32, y: f32, dy: f32) {
    cx.simulate_event(ScrollEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
        ..Default::default()
    });
    flush_frame(cx);
}

fn close_callbacks(drawer: Drawer, open: Rc<Cell<bool>>, recorded: Events) -> Drawer {
    drawer
        .on_close({
            let recorded = recorded.clone();
            move |_, _, _| recorded.borrow_mut().push("close".into())
        })
        .on_open_change(move |value, window, _| {
            open.set(*value);
            recorded.borrow_mut().push(format!("open:{value}"));
            window.refresh();
        })
}

#[gpui::test]
fn content_defaults_to_bottom_placement(cx: &mut TestAppContext) {
    still();
    let hit = events();
    let for_view = hit.clone();
    let cx = open_host(cx, move || {
        let hit = for_view.clone();
        Drawer::new()
            .id("drawer-default-bottom")
            .is_open(true)
            .is_dismissible(false)
            .child(DrawerHandle::new())
            .child(
                gpui::div()
                    .id("drawer-default-bottom-probe")
                    .w_full()
                    .h(px(40.))
                    .on_click(move |_, _, _| hit.borrow_mut().push("body".into())),
            )
            .into_any_element()
    });

    // The short bottom sheet sizes to its content near the bottom edge. Its
    // body starts after the dialog's 24px inset and 12px handle; a right
    // drawer would not cover x=100 at all.
    click(cx, 100., 1020.);
    assert_eq!(hit.borrow().as_slice(), ["body"]);
}

#[gpui::test]
fn closing_drawer_stays_mounted_for_its_full_exit_duration(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let open_for_view = open.clone();
    let hits = events();
    let hits_for_view = hits.clone();
    let cx = open_host(cx, move || {
        let hits = hits_for_view.clone();
        Drawer::new()
            .id("drawer-exit-lifetime")
            .is_open(open_for_view.get())
            .is_dismissible(false)
            .child(DrawerHandle::new())
            .child(
                gpui::div()
                    .id("drawer-exit-lifetime-probe")
                    .w_full()
                    .h(px(40.))
                    .on_click(move |_, _, _| hits.borrow_mut().push("body".into())),
            )
            .into_any_element()
    });

    open.set(false);
    flush_frame(cx);
    cx.executor().advance_clock(Duration::from_millis(110));
    flush_frame(cx);
    click(cx, 100., 1020.);
    assert_eq!(
        hits.borrow().as_slice(),
        ["body"],
        "the Drawer body must remain mounted after the shared 100ms exit lifetime"
    );

    cx.executor().advance_clock(Duration::from_millis(100));
    flush_frame(cx);
    click(cx, 100., 1020.);
    assert_eq!(
        hits.borrow().as_slice(),
        ["body"],
        "the Drawer body must unmount after its 200ms exit completes"
    );
}

#[gpui::test]
fn body_scroll_reaches_last_row_without_dismissal_and_body_drag_is_excluded(
    cx: &mut TestAppContext,
) {
    still();
    let open = Rc::new(Cell::new(true));
    let callbacks = events();
    let rows = events();
    let open_for_view = open;
    let callbacks_for_view = callbacks.clone();
    let rows_for_view = rows.clone();
    let cx = open_host(cx, move || {
        let mut drawer = Drawer::new()
            .id("drawer-scroll-body")
            .is_open(open_for_view.get())
            .placement(DrawerPlacement::Right)
            .child(DrawerHandle::new());
        for index in 0..40 {
            let label = format!("row-{index}");
            let recorded = rows_for_view.clone();
            drawer = drawer.child(
                gpui::div()
                    .id(gpui::SharedString::from(format!(
                        "drawer-scroll-row-{index}"
                    )))
                    .w_full()
                    .h(px(48.))
                    .flex_shrink_0()
                    .on_click(move |_, _, _| recorded.borrow_mut().push(label.clone())),
            );
        }
        close_callbacks(drawer, open_for_view.clone(), callbacks_for_view.clone())
            .into_any_element()
    });

    // The body begins below the handle. A full-threshold pull here must stay
    // a body gesture rather than starting drawer dismissal.
    drag(cx, (1760., 300.), (1860., 300.));
    assert!(callbacks.borrow().is_empty(), "body drag must not dismiss");

    // Negative pixel dy scrolls down. The 40 fixed-height rows plus gaps are
    // taller than the 1080px side panel, so the last row is unreachable until
    // the body's own native scroller moves.
    wheel(cx, 1760., 500., -4000.);
    let mut y = 100.;
    while y < 1050. {
        click(cx, 1760., y);
        y += 24.;
    }
    assert!(
        rows.borrow().iter().any(|row| row == "row-39"),
        "the final body row must become hit-testable after wheel scrolling: {:?}",
        rows.borrow().as_slice()
    );
    assert!(
        callbacks.borrow().is_empty(),
        "scrolling and probing inside the body must not dismiss the drawer"
    );
}

#[gpui::test]
fn titleless_right_drawer_dismisses_once_from_visible_handle(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let recorded = events();
    let open_for_view = open;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        close_callbacks(
            Drawer::new()
                .id("drawer-handle-drag")
                .is_open(open_for_view.get())
                .placement(DrawerPlacement::Right)
                .child(DrawerHandle::new()),
            open_for_view.clone(),
            for_view.clone(),
        )
        .into_any_element()
    });

    // The side panel starts at x=1600. Its visible handle occupies y=24..36;
    // 100px exceeds the 80px dismissal threshold.
    drag(cx, (1760., 28.), (1860., 28.));
    assert_eq!(recorded.borrow().as_slice(), ["close", "open:false"]);
}

#[gpui::test]
fn right_drawer_dismisses_once_from_header(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let recorded = events();
    let open_for_view = open;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        close_callbacks(
            Drawer::new()
                .id("drawer-header-drag")
                .is_open(open_for_view.get())
                .placement(DrawerPlacement::Right)
                .title("Header")
                .child(DrawerHandle::new()),
            open_for_view.clone(),
            for_view.clone(),
        )
        .into_any_element()
    });

    drag(cx, (1760., 48.), (1860., 48.));
    assert_eq!(recorded.borrow().as_slice(), ["close", "open:false"]);
}

#[gpui::test]
fn right_drawer_dismisses_once_from_footer(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let recorded = events();
    let open_for_view = open;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        close_callbacks(
            Drawer::new()
                .id("drawer-footer-drag")
                .is_open(open_for_view.get())
                .placement(DrawerPlacement::Right)
                .child(DrawerHandle::new())
                .footer_child(gpui::div().w(px(80.)).h(px(40.))),
            open_for_view.clone(),
            for_view.clone(),
        )
        .into_any_element()
    });

    // With no header or body, the footer follows the 12px handle at y=36.
    drag(cx, (1700., 56.), (1800., 56.));
    assert_eq!(recorded.borrow().as_slice(), ["close", "open:false"]);
}

#[gpui::test]
fn footer_button_does_not_start_drag_and_still_activates(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let callbacks = events();
    let actions = events();
    let open_for_view = open;
    let callbacks_for_view = callbacks.clone();
    let actions_for_view = actions.clone();
    let cx = open_host(cx, move || {
        let actions = actions_for_view.clone();
        close_callbacks(
            Drawer::new()
                .id("drawer-footer-button")
                .is_open(open_for_view.get())
                .placement(DrawerPlacement::Right)
                .child(DrawerHandle::new())
                .footer_child(
                    gpui::div().w(px(160.)).child(
                        Button::new("drawer-footer-action")
                            .label("Action")
                            .full_width(true)
                            .on_press(move |_, _, _| {
                                actions.borrow_mut().push("button".into());
                            }),
                    ),
                ),
            open_for_view.clone(),
            callbacks_for_view.clone(),
        )
        .into_any_element()
    });

    // The 160px footer action occupies x=1736..1896. A 100px pull wholly
    // inside it must stay a button gesture rather than starting dismissal.
    drag(cx, (1760., 56.), (1860., 56.));
    assert!(
        callbacks.borrow().is_empty(),
        "interactive footer descendants must not initiate drawer drag"
    );

    actions.borrow_mut().clear();
    click(cx, 1800., 56.);
    assert_eq!(actions.borrow().as_slice(), ["button"]);
    assert!(
        callbacks.borrow().is_empty(),
        "ordinary footer button activation must not dismiss the drawer"
    );
}

#[gpui::test]
fn unfocusable_footer_click_does_not_start_drag_and_still_activates(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let callbacks = events();
    let actions = events();
    let open_for_view = open;
    let callbacks_for_view = callbacks.clone();
    let actions_for_view = actions.clone();
    let cx = open_host(cx, move || {
        let actions = actions_for_view.clone();
        close_callbacks(
            Drawer::new()
                .id("drawer-footer-unfocusable-click")
                .is_open(open_for_view.get())
                .placement(DrawerPlacement::Right)
                .child(DrawerHandle::new())
                .footer_child(
                    gpui::div()
                        .id("drawer-footer-clickable-div")
                        .w(px(160.))
                        .h(px(40.))
                        .on_click(move |_, _, _| actions.borrow_mut().push("div".into())),
                ),
            open_for_view.clone(),
            callbacks_for_view.clone(),
        )
        .into_any_element()
    });

    drag(cx, (1760., 56.), (1860., 56.));
    assert!(
        callbacks.borrow().is_empty(),
        "clickable footer descendants must not drag"
    );
    actions.borrow_mut().clear();
    click(cx, 1800., 56.);
    assert_eq!(actions.borrow().as_slice(), ["div"]);
    assert!(callbacks.borrow().is_empty());
}

#[gpui::test]
fn noninteractive_full_width_footer_child_remains_draggable(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let recorded = events();
    let open_for_view = open;
    let recorded_for_view = recorded.clone();
    let cx = open_host(cx, move || {
        close_callbacks(
            Drawer::new()
                .id("drawer-footer-full-width")
                .is_open(open_for_view.get())
                .placement(DrawerPlacement::Right)
                .child(DrawerHandle::new())
                .footer_child(gpui::div().w_full().h(px(40.))),
            open_for_view.clone(),
            recorded_for_view.clone(),
        )
        .into_any_element()
    });

    drag(cx, (1700., 56.), (1820., 56.));
    assert_eq!(recorded.borrow().as_slice(), ["close", "open:false"]);
}

#[gpui::test]
fn top_drawer_drag_reaches_past_window_edge(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let recorded = events();
    let open_for_view = open;
    let recorded_for_view = recorded.clone();
    let cx = open_host(cx, move || {
        close_callbacks(
            Drawer::new()
                .id("drawer-top-global-drag")
                .is_open(open_for_view.get())
                .placement(DrawerPlacement::Top)
                .title("Top"),
            open_for_view.clone(),
            recorded_for_view.clone(),
        )
        .into_any_element()
    });

    // With no composed handle the title row starts at the 24px inset and
    // ends at y=48, so the pull begins at its centre.
    drag(cx, (960., 36.), (960., -120.));
    assert_eq!(recorded.borrow().as_slice(), ["close", "open:false"]);
}

#[gpui::test]
fn top_drawer_handle_and_footer_can_start_global_drag(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let recorded = events();
    let open_for_view = open.clone();
    let recorded_for_view = recorded.clone();
    let cx = open_host(cx, move || {
        close_callbacks(
            Drawer::new()
                .id("drawer-top-surfaces")
                .is_open(open_for_view.get())
                .placement(DrawerPlacement::Top)
                .title("Top")
                .child(DrawerHandle::new())
                .footer_child(gpui::div().w(px(80.)).h(px(40.))),
            open_for_view.clone(),
            recorded_for_view.clone(),
        )
        .into_any_element()
    });

    // v3 mounts the composed Top handle after the footer: the sheet is
    // 120px tall, so the 4px bar sits at y[108..112] and the footer row at
    // y[68..108].
    drag(cx, (960., 110.), (960., -120.));
    assert_eq!(recorded.borrow().as_slice(), ["close", "open:false"]);

    recorded.borrow_mut().clear();
    open.set(true);
    flush_frame(cx);
    drag(cx, (960., 100.), (960., -120.));
    assert_eq!(recorded.borrow().as_slice(), ["close", "open:false"]);
}

#[gpui::test]
fn drawer_drag_needs_eight_pixels_but_fast_flick_can_dismiss(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let recorded = events();
    let open_for_view = open;
    let recorded_for_view = recorded.clone();
    let cx = open_host(cx, move || {
        close_callbacks(
            Drawer::new()
                .id("drawer-drag-activation")
                .is_open(open_for_view.get())
                .placement(DrawerPlacement::Right)
                .child(DrawerHandle::new()),
            open_for_view.clone(),
            recorded_for_view.clone(),
        )
        .into_any_element()
    });

    slow_drag(cx, (1760., 28.), (1766., 28.));
    assert!(
        recorded.borrow().is_empty(),
        "a six-pixel pull must not activate"
    );

    drag(cx, (1760., 28.), (1770., 28.));
    assert_eq!(recorded.borrow().as_slice(), ["close", "open:false"]);
}

#[gpui::test]
fn non_dismissible_blocks_surface_drags_and_backdrop_but_not_escape(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let recorded = events();
    let open_for_view = open;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        close_callbacks(
            Drawer::new()
                .id("drawer-not-dismissible")
                .is_open(open_for_view.get())
                .placement(DrawerPlacement::Right)
                .is_dismissible(false)
                .title("Header")
                .child(DrawerHandle::new())
                .footer_child(gpui::div().w_full().h(px(40.))),
            open_for_view.clone(),
            for_view.clone(),
        )
        .into_any_element()
    });

    drag(cx, (1760., 28.), (1860., 28.));
    drag(cx, (1760., 48.), (1860., 48.));
    drag(cx, (1760., 100.), (1860., 100.));
    click(cx, 100., 100.);
    assert!(
        recorded.borrow().is_empty(),
        "isDismissable=false must block handle, header, footer, and backdrop dismissal"
    );

    // React Aria gates Escape separately from outside/drag dismissal.
    press(cx, "escape");
    assert_eq!(recorded.borrow().as_slice(), ["close", "open:false"]);
}

#[gpui::test]
fn keyboard_dismiss_disabled_blocks_escape_but_not_handle_drag(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let recorded = events();
    let open_for_view = open;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        close_callbacks(
            Drawer::new()
                .id("drawer-keyboard-disabled")
                .is_open(open_for_view.get())
                .placement(DrawerPlacement::Right)
                .is_keyboard_dismiss_disabled(true)
                .child(DrawerHandle::new()),
            open_for_view.clone(),
            for_view.clone(),
        )
        .into_any_element()
    });

    press(cx, "escape");
    assert!(recorded.borrow().is_empty(), "Escape must be disabled");

    drag(cx, (1760., 28.), (1860., 28.));
    assert_eq!(recorded.borrow().as_slice(), ["close", "open:false"]);
}

#[gpui::test]
fn tab_remains_trapped_inside_drawer(cx: &mut TestAppContext) {
    still();
    let outside = events();
    let inside = events();
    let outside_for_view = outside.clone();
    let inside_for_view = inside.clone();
    let cx = open_host(cx, move || {
        let outside = outside_for_view.clone();
        let inside = inside_for_view.clone();
        let second_inside = inside_for_view.clone();
        gpui::div()
            .child(
                Button::new("drawer-tab-outside")
                    .label("Outside")
                    .on_press(move |_, _, _| outside.borrow_mut().push("outside".into())),
            )
            .child(
                Drawer::new()
                    .id("drawer-tab-trap")
                    .is_open(true)
                    .placement(DrawerPlacement::Right)
                    .child(
                        Button::new("drawer-tab-inside-first")
                            .label("First")
                            .on_press(move |_, _, _| inside.borrow_mut().push("inside".into())),
                    )
                    .child(
                        Button::new("drawer-tab-inside-second")
                            .label("Second")
                            .on_press(move |_, _, _| {
                                second_inside.borrow_mut().push("second".into());
                            }),
                    ),
            )
            .into_any_element()
    });

    press(cx, "tab tab");
    press_backward(cx);
    press_backward(cx);
    press(cx, "enter");
    assert_eq!(inside.borrow().as_slice(), ["second"]);
    assert!(outside.borrow().is_empty(), "Tab must not leave the drawer");
}

/// The handle's whole painted surface is the 36x4 `bg-separator` bar, so
/// finding separator-coloured quads of that size finds every composed
/// handle and nothing else in these minimal sheets.
fn handle_bars(cx: &mut VisualTestContext) -> Vec<gpui::Bounds<gpui::Pixels>> {
    let scene = painted(cx);
    let separator = cx.update(|_, cx| cx.colors().separator);
    scene
        .quads
        .iter()
        .filter(|quad| quad.background.as_solid() == Some(separator))
        .map(|quad| scene.bounds(quad))
        .filter(|bounds| {
            (f32::from(bounds.size.width) - 36.).abs() < 0.6
                && (f32::from(bounds.size.height) - 4.).abs() < 0.6
        })
        .collect()
}

/// v3's `Drawer.Handle` is an optional composed part: a drawer that composes
/// none paints no bar, so right and left sheets — which v3 never gives one —
/// stay clean by default.
#[gpui::test]
fn drawer_without_composed_handle_paints_no_bar(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        Drawer::new()
            .id("drawer-no-handle")
            .is_open(true)
            .placement(DrawerPlacement::Right)
            .title("No handle")
            .into_any_element()
    });

    assert!(
        handle_bars(cx).is_empty(),
        "an uncomposed Drawer.Handle must paint no separator bar"
    );
}

/// A composed handle paints the 36x4 bar before the header on Bottom: the
/// bar's top sits above the body probe by the title row plus `mt-2`, not
/// merely above it — a bar mounted between header and body would land only
/// ~19px above the probe instead of ~47px.
#[gpui::test]
fn composed_handle_paints_before_the_header_on_bottom(cx: &mut TestAppContext) {
    still();
    let body_color: gpui::Hsla = gpui::rgb(0x334455).into();
    let footer_color: gpui::Hsla = gpui::rgb(0x556677).into();
    let body_color_for_view = body_color;
    let footer_color_for_view = footer_color;
    let cx = open_host(cx, move || {
        Drawer::new()
            .id("drawer-bottom-handle")
            .is_open(true)
            .placement(DrawerPlacement::Bottom)
            .title("Bottom")
            .child(DrawerHandle::new())
            .child(gpui::div().w(px(40.)).h(px(40.)).bg(body_color_for_view))
            .footer_child(gpui::div().w(px(40.)).h(px(40.)).bg(footer_color_for_view))
            .into_any_element()
    });

    let scene = painted(cx);
    let body_probe = scene
        .filled(body_color)
        .into_iter()
        .next()
        .map(|quad| scene.bounds(quad))
        .expect("the body probe must paint");
    let bars = handle_bars(cx);
    assert_eq!(bars.len(), 1, "exactly one composed handle must paint");
    let gap = f32::from(body_probe.origin.y) - f32::from(bars[0].origin.y);
    assert!(
        gap > 35.,
        "the handle must mount before the header: bar top is {gap}px above the body probe"
    );
}

/// On Top the composed handle mounts after the footer — v3's `--top`
/// variant parks the bar at the sheet's far edge — so the bar's top is at
/// or below the footer probe's bottom.
#[gpui::test]
fn composed_handle_mounts_after_the_footer_on_top(cx: &mut TestAppContext) {
    still();
    let footer_color: gpui::Hsla = gpui::rgb(0x556677).into();
    let footer_color_for_view = footer_color;
    let cx = open_host(cx, move || {
        Drawer::new()
            .id("drawer-top-handle")
            .is_open(true)
            .placement(DrawerPlacement::Top)
            .title("Top")
            .child(DrawerHandle::new())
            .footer_child(gpui::div().w(px(40.)).h(px(40.)).bg(footer_color_for_view))
            .into_any_element()
    });

    let scene = painted(cx);
    let footer_probe = scene
        .filled(footer_color)
        .into_iter()
        .next()
        .map(|quad| scene.bounds(quad))
        .expect("the footer probe must paint");
    let bars = handle_bars(cx);
    assert_eq!(bars.len(), 1, "exactly one composed handle must paint");
    let footer_bottom = f32::from(footer_probe.origin.y + footer_probe.size.height);
    let bar_top = f32::from(bars[0].origin.y);
    assert!(
        bar_top >= footer_bottom - 0.5,
        "the Top handle must mount after the footer: bar top {bar_top} vs footer bottom {footer_bottom}"
    );
}

/// The composed handle keeps the drag-to-dismiss wiring: a pull on the
/// Bottom sheet's bar travels past the threshold and reports the close.
#[gpui::test]
fn bottom_drawer_composed_handle_drag_still_dismisses(cx: &mut TestAppContext) {
    still();
    let open = Rc::new(Cell::new(true));
    let recorded = events();
    let open_for_view = open;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        close_callbacks(
            Drawer::new()
                .id("drawer-bottom-handle-drag")
                .is_open(open_for_view.get())
                .placement(DrawerPlacement::Bottom)
                .title("Bottom")
                .child(DrawerHandle::new()),
            open_for_view.clone(),
            for_view.clone(),
        )
        .into_any_element()
    });

    // Title + handle only: the 84px sheet sits at y[996..1080] and its
    // handle strip spans y[1020..1032]. A 94px pull clears 30% of it.
    drag(cx, (960., 1026.), (960., 1120.));
    assert_eq!(recorded.borrow().as_slice(), ["close", "open:false"]);
}
