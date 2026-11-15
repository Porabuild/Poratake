//! The customisation tokens reach their consumers.
//!
//! Builder/JSON application is covered in `herogpui-theme` (a document key
//! lands in the same layout field the builder sets). These tests pin the
//! other half: `set_theme` publishes every token to the layout, each named
//! consumer reads the token that replaced its literal (wiring), and — at the
//! bottom — a themed value changes what the consumer does: the tooltip's open
//! and close delays on the test clock, the dropdown's long press, the hover
//! fade's duration and the Tabs hover wash on the painted scene.

use crate::harness;
use crate::source_scan;

use gpui::TestAppContext;
use herogpui_theme::{set_theme, ActiveTheme, Theme, ThemeProvider};
use source_scan::{component_src, scope_contains};

struct Consumer {
    file: &'static str,
    part: &'static str,
    /// Source marker whose enclosing function must read the token.
    owner: &'static str,
    token: &'static str,
    /// The hardcoded literal this consumer used before the token existed, if
    /// any; it must be gone.
    removed: Option<&'static str>,
}

const CONSUMERS: &[Consumer] = &[
    Consumer {
        file: "tabs.rs",
        part: "primary tab hover dim",
        owner: "crate::anim::hover_fade(",
        token: "tabs_hover_opacity",
        removed: Some("opacity(0.7)"),
    },
    Consumer {
        file: "tabs.rs",
        part: "secondary tab hover dim",
        owner: "crate::anim::hover_fade(",
        token: "tabs_hover_opacity",
        removed: Some("opacity(0.7)"),
    },
    Consumer {
        file: "tabs.rs",
        part: "scroll-arrow hover dim",
        owner: "tab_overlay",
        token: "tabs_hover_opacity",
        removed: Some("opacity(0.7)"),
    },
    Consumer {
        file: "tooltip.rs",
        part: "global cooldown floor",
        owner: "fn start_tooltip_cooldown",
        token: "tooltip_cooldown_ms",
        removed: Some("TOOLTIP_GLOBAL_COOLDOWN_MS"),
    },
    Consumer {
        file: "dropdown.rs",
        part: "long-press wait",
        owner: "DropdownTrigger::LongPress => {",
        token: "long_press_ms",
        removed: Some("LONG_PRESS_MS"),
    },
    Consumer {
        file: "anim.rs",
        part: "hover-fade duration",
        owner: "fn hover_fade_duration_with_override",
        token: "hover_fade_ms",
        removed: Some("Animation::new(Duration::from_millis(TRANSITION_MS))"),
    },
];

/// v3's dark `--overlay-shadow` is `0 0 1px rgba(255,255,255,.3) inset` —
/// the hairline that separates a floating panel from the page. Every surface
/// painting `overlay_shadow` must also paint `overlay_hairline`, or dark mode
/// loses the edge entirely.
#[test]
fn every_overlay_shadow_surface_paints_the_dark_hairline() {
    for file in [
        "toast.rs",
        "toolbar.rs",
        "sheet.rs",
        "modal.rs",
        "drawer.rs",
        "date_picker/picker.rs",
        // Already wired before this sweep; pinned so a regression cannot
        // drop one side of the pair.
        "popover.rs",
        "alert_dialog.rs",
        "select.rs",
        "tooltip.rs",
        "dropdown.rs",
        "combo_box.rs",
        "autocomplete.rs",
        "hover_card.rs",
        "color_picker/picker.rs",
        "command_palette.rs",
    ] {
        let source = component_src(file);
        assert!(
            source.contains("overlay_hairline"),
            "{file} paints overlay_shadow but never the dark-mode hairline"
        );
    }
}

#[test]
fn every_named_consumer_reads_its_token_and_drops_the_literal() {
    // The three Tabs closures must be fed by the layout-token binding: a
    // literal assigned to `tabs_hover_opacity` would satisfy every marker.
    let tabs = component_src("tabs.rs");
    assert!(
        tabs.contains("let tabs_hover_opacity = layout.tabs_hover_opacity;"),
        "the Tabs hover closures must take their opacity from the layout token"
    );

    let mut failures = Vec::new();
    for consumer in CONSUMERS {
        let source = component_src(consumer.file);
        if !source.contains(consumer.owner) {
            failures.push(format!(
                "{} [{}]: owner marker {:?} is gone",
                consumer.file, consumer.part, consumer.owner
            ));
            continue;
        }
        if let Err(err) = scope_contains(&source, consumer.owner, consumer.token) {
            failures.push(format!("{} [{}]: {err}", consumer.file, consumer.part));
        }
        if let Some(literal) = consumer.removed {
            if source.contains(literal) {
                failures.push(format!(
                    "{} [{}]: the replaced literal {:?} is still in the source",
                    consumer.file, consumer.part, literal
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "token consumer wiring failed:\n{}",
        failures.join("\n")
    );
}

#[gpui::test]
fn set_theme_publishes_every_customisation_token(cx: &mut TestAppContext) {
    cx.update(ThemeProvider::init);
    cx.update(|cx| {
        set_theme(
            Theme::builder("custom", Theme::light())
                .tabs_hover_opacity(0.25)
                .tooltip_cooldown_ms(111)
                .long_press_ms(222)
                .hover_fade_ms(333)
                .tooltip_delay_ms(444)
                .tooltip_close_delay_ms(555)
                .build(),
            cx,
        );
    });
    cx.update(|cx| {
        let layout = cx.layout();
        assert!((layout.tabs_hover_opacity - 0.25).abs() < 1e-6);
        assert_eq!(layout.tooltip_cooldown_ms, 111);
        assert_eq!(layout.long_press_ms, 222);
        assert_eq!(layout.hover_fade_ms, 333);
        assert_eq!(layout.tooltip_delay_ms, 444);
        assert_eq!(layout.tooltip_close_delay_ms, 555);
    });
}

// ---- behaviour: a themed token changes what its consumer does ----------

use gpui::{point, prelude::*, px, Modifiers, MouseButton, VisualTestContext};
use herogpui_theme::ThemeBuilder;
use std::time::Duration;

fn themed(cx: &mut VisualTestContext, build: impl FnOnce(ThemeBuilder) -> ThemeBuilder) {
    cx.update(|window, cx| {
        set_theme(build(Theme::builder("tokens", Theme::light())).build(), cx);
        window.refresh();
    });
}

fn flush(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn last(seen: &harness::Events) -> String {
    seen.borrow().last().cloned().unwrap_or_default()
}

/// A themed 300ms/80ms pair replaces v3's 1500ms/500ms: the tip opens between
/// 250ms and 350ms (the stock delay would keep it shut) and closes between
/// 40ms and 120ms after the pointer leaves.
#[gpui::test]
fn themed_tooltip_delays_drive_the_tooltip(cx: &mut TestAppContext) {
    use herogpui_components::Tooltip;
    harness::still();
    let seen = harness::events();
    let probe = seen.clone();
    let cx = harness::open_host(cx, move || {
        gpui::div()
            .child(harness::tooltip_open_probe("tok-tt", probe.clone(), false))
            .child(
                Tooltip::new("Saved")
                    .id("tok-tt")
                    .child(gpui::div().id("tok-tt-trigger").w(px(120.)).h(px(36.))),
            )
            .into_any_element()
    });
    themed(cx, |b| b.tooltip_delay_ms(300).tooltip_close_delay_ms(80));
    flush(cx);

    cx.simulate_mouse_move(point(px(60.), px(18.)), None, Modifiers::none());
    flush(cx);
    cx.executor().advance_clock(Duration::from_millis(250));
    flush(cx);
    assert_eq!(last(&seen), "open:false", "still inside the themed delay");
    cx.executor().advance_clock(Duration::from_millis(100));
    flush(cx);
    assert_eq!(
        last(&seen),
        "open:true",
        "open once the themed 300ms elapse"
    );

    cx.simulate_mouse_move(point(px(600.), px(600.)), None, Modifiers::none());
    flush(cx);
    cx.executor().advance_clock(Duration::from_millis(40));
    flush(cx);
    assert_eq!(
        last(&seen),
        "open:true",
        "still inside the themed close delay"
    );
    cx.executor().advance_clock(Duration::from_millis(80));
    flush(cx);
    assert_eq!(
        last(&seen),
        "open:false",
        "closed once the themed 80ms elapse"
    );
}

/// A themed 1000ms cooldown keeps the tooltips warm past the stock 500ms: a
/// re-hover 600ms after leaving opens at once, and one after the themed
/// cooldown waits for the open delay again.
#[gpui::test]
fn a_themed_tooltip_cooldown_keeps_the_warm_up(cx: &mut TestAppContext) {
    use herogpui_components::Tooltip;
    harness::still();
    let seen = harness::events();
    let probe = seen.clone();
    let cx = harness::open_host(cx, move || {
        gpui::div()
            .child(harness::tooltip_open_probe(
                "tok-cool",
                probe.clone(),
                false,
            ))
            .child(
                Tooltip::new("Saved")
                    .id("tok-cool")
                    .child(gpui::div().id("tok-cool-trigger").w(px(120.)).h(px(36.))),
            )
            .into_any_element()
    });
    themed(cx, |b| {
        b.tooltip_delay_ms(300)
            .tooltip_close_delay_ms(80)
            .tooltip_cooldown_ms(1000)
    });
    flush(cx);
    let on = point(px(60.), px(18.));
    let off = point(px(600.), px(600.));
    let wait = |cx: &mut VisualTestContext, ms: u64| {
        cx.executor().advance_clock(Duration::from_millis(ms));
        flush(cx);
    };

    cx.simulate_mouse_move(on, None, Modifiers::none());
    flush(cx);
    wait(cx, 350);
    assert_eq!(last(&seen), "open:true");
    cx.simulate_mouse_move(off, None, Modifiers::none());
    flush(cx);
    wait(cx, 600);
    assert_eq!(
        last(&seen),
        "open:false",
        "closed after the 80ms close delay"
    );

    // 600ms after leaving: cold under the stock 500ms, warm under 1000ms.
    cx.simulate_mouse_move(on, None, Modifiers::none());
    flush(cx);
    wait(cx, 10);
    assert_eq!(last(&seen), "open:true", "still warm, so it opens at once");

    cx.simulate_mouse_move(off, None, Modifiers::none());
    flush(cx);
    wait(cx, 1200);
    cx.simulate_mouse_move(on, None, Modifiers::none());
    flush(cx);
    wait(cx, 10);
    assert_eq!(
        last(&seen),
        "open:false",
        "past the themed cooldown it waits for the open delay again"
    );
}

/// A themed 120ms long press opens a long-press dropdown well before the
/// stock 500ms, and a press released before it leaves the menu shut.
#[gpui::test]
fn themed_long_press_opens_the_dropdown(cx: &mut TestAppContext) {
    use herogpui_components::{Button, Dropdown, DropdownTrigger, MenuItem};
    harness::still();
    let opens = harness::events();
    let record = opens.clone();
    let cx = harness::open_host(cx, move || {
        let record = record.clone();
        Dropdown::uncontrolled(
            "tok-dd",
            Button::new("tok-dd-trigger").label("Actions"),
            vec![MenuItem::new("one", "One")],
        )
        .trigger(DropdownTrigger::LongPress)
        .on_open_change(move |open, _, _| record.borrow_mut().push(format!("open:{open}")))
        .into_any_element()
    });
    themed(cx, |b| b.long_press_ms(120));
    flush(cx);
    let at = point(px(20.), px(18.));

    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    flush(cx);
    cx.executor().advance_clock(Duration::from_millis(60));
    flush(cx);
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    flush(cx);
    cx.executor().advance_clock(Duration::from_millis(200));
    flush(cx);
    assert!(
        !opens.borrow().iter().any(|e| e == "open:true"),
        "a press released inside the themed wait must not open: {:?}",
        opens.borrow()
    );

    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    flush(cx);
    cx.executor().advance_clock(Duration::from_millis(150));
    flush(cx);
    assert_eq!(
        last(&opens),
        "open:true",
        "held past the themed 120ms (short of the stock 500ms), it opens"
    );
}

const HOVER: gpui::Hsla = gpui::Hsla {
    h: 0.9,
    s: 0.7,
    l: 0.45,
    a: 1.0,
};

/// Whether hovering a Button with `hover_bg(HOVER)` shows `HOVER` after
/// `wait_ms` of real time under a themed `hover_fade_ms`.
fn button_reaches_hover(cx: &mut TestAppContext, fade_ms: u64, wait_ms: u64) -> bool {
    use herogpui_components::Button;
    let cx = harness::open_host(cx, || {
        Button::new("tok-fade")
            .label("Hover")
            .hover_bg(HOVER)
            .into_any_element()
    });
    themed(cx, |b| b.hover_fade_ms(fade_ms));
    harness::wait_ms(cx, 50);
    cx.simulate_mouse_move(point(px(20.), px(18.)), None, Modifiers::none());
    harness::wait_ms(cx, wait_ms);
    // Once the fade's fill is mounted it is the only painter of the colour —
    // the element underneath stays transparent — so the colour that shows is
    // the last solid painted over the element's box.
    let scene = harness::painted(cx);
    let Some(element) = scene
        .quads
        .iter()
        .find(|q| q.background.as_solid() == Some(HOVER))
    else {
        return false;
    };
    let shown = scene
        .quads
        .iter()
        .rev()
        .filter(|q| q.bounds == element.bounds)
        .find_map(|q| q.background.as_solid());
    shown.is_some_and(|c| harness::same_color(c, HOVER))
}

#[gpui::test]
fn a_zero_hover_fade_ms_paints_the_endpoint_at_once(cx: &mut TestAppContext) {
    assert!(button_reaches_hover(cx, 0, 0));
}

#[gpui::test]
fn a_long_hover_fade_ms_is_still_fading(cx: &mut TestAppContext) {
    assert!(
        !button_reaches_hover(cx, 60_000, 200),
        "a one-minute fade is nowhere near its endpoint after 200ms"
    );
}

/// The unselected-tab hover wash is the tray colour at `1 - tabs_hover_opacity`
/// alpha: a themed 0.25 paints a 0.75-alpha wash where the stock 0.7 paints
/// a 0.3 one.
#[gpui::test]
fn themed_tabs_hover_opacity_sets_the_wash_alpha(cx: &mut TestAppContext) {
    use herogpui_components::{TabItem, Tabs};
    let tray = gpui::hsla(0.55, 0.5, 0.5, 1.0);
    let cx = harness::open_host(cx, move || {
        Tabs::new(
            "tok-tabs",
            vec![
                TabItem::new("a", "Alpha").width(px(100.)),
                TabItem::new("b", "Beta").width(px(100.)),
            ],
            "a",
        )
        .list_bg(tray)
        .into_any_element()
    });
    themed(cx, |b| b.tabs_hover_opacity(0.25).hover_fade_ms(0));
    harness::wait_ms(cx, 50);
    // Both tabs are 100px wide inside the tray's 4px inset, so "Beta" spans
    // x 104..204 of the 32px row.
    cx.simulate_mouse_move(point(px(150.), px(18.)), None, Modifiers::none());
    harness::wait_ms(cx, 50);
    let fills = harness::painted(cx).solids();
    assert!(
        harness::has_color(&fills, tray.alpha(0.75)),
        "the themed wash is the tray at 0.75 alpha: {fills:?}"
    );
    assert!(!harness::has_color(&fills, tray.alpha(0.3)));
}
