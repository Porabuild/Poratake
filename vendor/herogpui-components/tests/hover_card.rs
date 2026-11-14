//! `HoverCard` (HeroGPUI extension): the open delay, the close delay that the
//! pointer reaching the card cancels, Escape and outside-press dismissal,
//! keyboard focus opening at once, and the controlled open state.

use crate::harness;

use std::time::Duration;

use gpui::{point, prelude::*, px, Modifiers, MouseMoveEvent, TestAppContext, VisualTestContext};
use harness::{events, open_host, press, still, Events};
use herogpui_components::{Button, HoverCard};

#[derive(Clone, Copy, Default)]
struct Config {
    controlled: Option<bool>,
}

fn host(cx: &mut TestAppContext, config: Config) -> (Events, &mut VisualTestContext) {
    still();
    let seen = events();
    let s = seen.clone();
    let cx = open_host(cx, move || {
        let log = s.clone();
        let mut card = HoverCard::new("card")
            .label("Profile")
            .open_delay(700)
            .close_delay(300)
            .content(|_, _| gpui::div().h(px(80.)).child("Jane Doe").into_any_element())
            .on_open_change(move |open, _, _| log.borrow_mut().push(format!("open:{open}")))
            .child(
                gpui::div()
                    .debug_selector(|| "trigger".into())
                    .child(Button::new("trigger").label("@jane")),
            );
        if let Some(open) = config.controlled {
            card = card.is_open(open);
        }
        gpui::div()
            .p(px(40.))
            .flex()
            .flex_col()
            .gap(px(200.))
            .child(card)
            .child(Button::new("elsewhere").label("Elsewhere"))
            .into_any_element()
    });
    (seen, cx)
}

fn frame(cx: &mut VisualTestContext) {
    for _ in 0..2 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

fn move_to(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>) {
    cx.simulate_event(MouseMoveEvent {
        position: at,
        pressed_button: None,
        modifiers: Modifiers::none(),
    });
    frame(cx);
}

fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    frame(cx);
}

fn trigger(cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
    frame(cx);
    cx.debug_bounds("trigger")
        .expect("trigger laid out")
        .center()
}

fn card(cx: &mut VisualTestContext) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.debug_bounds("card-card")
}

fn log(seen: &Events) -> Vec<String> {
    seen.borrow().clone()
}

#[gpui::test]
fn hovering_the_trigger_opens_only_after_the_open_delay(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    let at = trigger(cx);
    move_to(cx, at);
    wait(cx, 400);
    assert!(card(cx).is_none(), "the card waits out the open delay");
    wait(cx, 400);
    assert!(card(cx).is_some(), "the card opens after the delay");
    assert_eq!(log(&seen), ["open:true"]);
}

#[gpui::test]
fn a_pass_over_the_trigger_shorter_than_the_delay_opens_nothing(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    let at = trigger(cx);
    move_to(cx, at);
    wait(cx, 300);
    move_to(cx, point(px(900.), px(900.)));
    wait(cx, 1000);
    assert!(card(cx).is_none());
    assert!(
        log(&seen).is_empty(),
        "a stale timer must not open the card"
    );
}

#[gpui::test]
fn leaving_closes_after_the_close_delay_unless_the_pointer_reaches_the_card(
    cx: &mut TestAppContext,
) {
    let (seen, cx) = host(cx, Config::default());
    let at = trigger(cx);
    move_to(cx, at);
    wait(cx, 800);
    let panel = card(cx).expect("open");

    // Trigger → card within the close delay keeps it open.
    move_to(cx, point(px(900.), px(900.)));
    wait(cx, 100);
    move_to(cx, panel.center());
    wait(cx, 1000);
    assert!(card(cx).is_some(), "the card stays open under the pointer");
    assert_eq!(log(&seen), ["open:true"]);

    // Leaving the card closes it after the delay, not before.
    move_to(cx, point(px(900.), px(900.)));
    wait(cx, 100);
    assert_eq!(log(&seen), ["open:true"], "the close waits out the delay");
    wait(cx, 300);
    assert_eq!(log(&seen), ["open:true", "open:false"]);
    wait(cx, 300);
    assert!(card(cx).is_none(), "the exit finished");
}

#[gpui::test]
fn escape_and_an_outside_press_close_the_card(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    let at = trigger(cx);
    move_to(cx, at);
    wait(cx, 800);
    assert!(card(cx).is_some());
    press(cx, "escape");
    wait(cx, 300);
    assert_eq!(log(&seen), ["open:true", "open:false"]);
    assert!(card(cx).is_none());

    // Re-hover opens again; a press far outside closes it.
    move_to(cx, point(px(900.), px(900.)));
    move_to(cx, at);
    wait(cx, 800);
    assert!(card(cx).is_some());
    let far = point(px(900.), px(900.));
    cx.simulate_click(far, Modifiers::none());
    wait(cx, 300);
    assert_eq!(
        log(&seen),
        ["open:true", "open:false", "open:true", "open:false"]
    );
}

#[gpui::test]
fn keyboard_focus_opens_at_once_and_leaving_it_closes(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    press(cx, "tab");
    frame(cx);
    assert_eq!(
        log(&seen),
        ["open:true"],
        "keyboard focus opens without a delay"
    );
    assert!(card(cx).is_some());
    press(cx, "tab");
    wait(cx, 300);
    assert_eq!(log(&seen), ["open:true", "open:false"]);
}

#[gpui::test]
fn a_controlled_card_reports_and_follows_its_prop(cx: &mut TestAppContext) {
    let (seen, cx) = host(
        cx,
        Config {
            controlled: Some(false),
        },
    );
    let at = trigger(cx);
    move_to(cx, at);
    wait(cx, 800);
    assert_eq!(log(&seen), ["open:true"], "the request is reported");
    assert!(card(cx).is_none(), "but the closed prop wins");
}

#[gpui::test]
fn a_controlled_open_card_renders_without_hover(cx: &mut TestAppContext) {
    let (_seen, cx) = host(
        cx,
        Config {
            controlled: Some(true),
        },
    );
    frame(cx);
    let panel = card(cx).expect("the open prop renders the card");
    let trigger = trigger(cx);
    assert!(panel.top() > trigger.y, "the card sits below the trigger");
}
