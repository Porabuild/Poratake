//! Behaviour tests for the navigation family's keyboard and edge cases:
//! Accordion, DisclosureGroup, Breadcrumbs, Pagination and Toolbar.
//!
//! `tests/collections.rs` drives Accordion's click-toggle and its single-expand
//! mode, Pagination's next/page clicks and Breadcrumbs' last-crumb rule;
//! `tests/table_tabs_accordion.rs` covers Accordion's multiple-expand and
//! disabled keys, DisclosureGroup's click path, and the horizontal Toolbar's
//! arrow scoping (which was fixed and pinned there). Nothing here duplicates
//! those — every test drives a keyboard contract or an edge the earlier
//! suites left undriven, and every assertion is behavioural (recorded
//! callbacks, or a probe click that must record nothing), never appearance.
//!
//! ---------------------------------------------------------------------------
//! The v3 contracts, quoted from https://heroui.com/react/llms-full.txt
//! (pages read by their line ranges, September 2026 snapshot):
//!
//! * **Accordion** — the page has **no `## Accessibility` section** (its
//!   sections are Usage, Anatomy, Examples, Customization, API Reference), so
//!   the inherited behaviour is the contract: the trigger is a native button
//!   (`AccordionTrigger` renders `<Button slot="trigger">`), which means Tab
//!   reaches each trigger in turn and Enter/Space toggle it — and nothing else
//!   is bound: no arrow keys between triggers, no Home/End. The page's own
//!   "Interactive States" list only `:focus-visible` on the trigger. The API
//!   table documents `allowsMultipleExpanded` with **default `false`**, which
//!   this port defaults to `true` instead (a parity note, not a keyboard gap).
//!   v3's migration page says "content always mounted in v3"; the RAC panel is
//!   `hidden` when collapsed, so a control in a closed body is *not* reachable
//!   by Tab either way — the port unmounts the body, with the same Tab effect.
//! * **Breadcrumbs** — HAS an `## Accessibility` section: "Breadcrumbs uses
//!   React Aria Components' Breadcrumbs primitive, which provides: Proper
//!   ARIA attributes... **Keyboard navigation support**... The last breadcrumb
//!   item (without `href`) automatically becomes the current page indicator."
//!   The API table has no `maxItems`-style collapse prop — only `separator`,
//!   `isDisabled`, `children` and `render` — so the port's lack of overflow
//!   truncation matches v3 rather than losing something.
//! * **DisclosureGroup** — props: `expandedKeys`, `defaultExpandedKeys`,
//!   `onExpandedChange: (keys: Set<Key>) => void`, `allowsMultipleExpanded`
//!   (default `false` — expanding one collapses the others), `isDisabled`.
//! * **Pagination** — v3 is a composition (`Pagination.Link/Previous/Next/
//!   Ellipsis`); the v2 `page`/`total`/`siblings`/`boundaries` props were
//!   **removed** ("Removed (compose items manually)"). The port keeps the v2
//!   API. Its `## Accessibility` section claims: "Keyboard navigation via Tab
//!   key through all interactive elements", "Ellipsis marked with
//!   `aria-hidden` to avoid screen reader confusion", "Disabled states
//!   properly communicated to assistive technology via `isDisabled`", and the
//!   note that press handlers "from React Aria" normalise pointer *and*
//!   keyboard presses — so a disabled Previous must fire no press at all.
//! * **Toolbar** — "Inherits from React Aria Toolbar". React Aria's
//!   `useToolbar` handles exactly the orientation's axis — horizontal:
//!   ArrowRight/ArrowLeft; vertical: ArrowDown/ArrowUp — nothing else, and
//!   Tab moves out of the toolbar. This port maps both axes in both
//!   orientations (a deviation pinned by one test below).
//!
//! ---------------------------------------------------------------------------
//! Geometry, derived from the components' own constants — every number
//! carries its arithmetic in the test that uses it:
//!
//! - Accordion: a trigger is `px-4 py-4` (16px all round) around one 20px
//!   line, so 52px; items are joined by a 1px separator. An open body is
//!   `pt-2 pb-4` (2/16) around its content, so with a 36px Button inside it
//!   is 54px tall. Header 0 spans y 0..52 (centre 26); header 1 sits at
//!   53..105 (centre 79); with item 0 open and a 36px body, header 1 sits at
//!   107..159 and the open body's button spans y 107..143.
//! - DisclosureGroup: each trigger is a md `Button` (`h-9`, 36px) stretched to
//!   the group width; an open body is `p-2` (8px all round) around a 36px
//!   probe, so 52px. With both items open the probes sit at y 44..80 (centre
//!   62) and y 132..168 (centre 150).
//! - Breadcrumbs: crumb labels are *measured* with the window's own text
//!   system (`Window::text_system().shape_line`) at the link's own weight —
//!   `.breadcrumbs__link` is `font-medium`, so MEDIUM — because a click
//!   target's x depends on the label's advance width in the renderer's font.
//!   The label line is `leading-5` = 20px, so its centre y is 10. Since
//!   v3.2.6 an item row is the unpadded link plus `gap-1` (4px) and a 12px
//!   separator slot, so a row is `w_label + 16` wide, a label starts flush in
//!   its row, and the root gaps rows by `gap-1.5` (6px) without wrapping.
//! - Pagination: `size-md` cells are 32px squares at y 0..32 (centre y 16); a
//!   nav button is `px-2.5` (10px each side) around a 14px glyph = 34px; the
//!   row gaps items by `gap-1` (4px). Prev spans x 0..34 (centre 17), page
//!   cell *k* spans 38+36k .. 70+36k (centre 54+36k), and the next button
//!   starts at 38+36·(cell count) — 146 for three cells (centre 163), 290 for
//!   seven (centre 307), 74 for one (centre 91).
//! - Toolbar: driven entirely through the keyboard (Tab, arrows, Enter), so no
//!   geometry enters; the window's tab order is what moves.
//!
//! Each instance gets its own element id; two components sharing an id share
//! their keyed state, which AGENTS.md documents as a silent failure. The
//! `press` helper releases the last key because gpui activates a focused
//! element's click listeners on key **up** (Enter/Space only — verified in
//! gpui's `div.rs`, which maps Enter/Space and nothing else).

use crate::harness;
use crate::source_scan;

use std::{cell::Cell, collections::HashSet, rc::Rc, time::Duration};

use gpui::{
    prelude::*, px, Font, FontFeatures, FontStyle, FontWeight, SharedString, TestAppContext,
    VisualTestContext,
};
use herogpui_components::{
    Accordion, AccordionItem, Breadcrumbs, Button, Crumb, Disclosure, DisclosureGroup, Orientation,
    Pagination, Toolbar,
};

use harness::{click, events, open_host, press, Events};

#[gpui::test]
fn accordion_subtitle_and_body_keep_their_line_boxes(cx: &mut TestAppContext) {
    for leading in [None, Some(48.)] {
        let cx = open_host(cx, move || {
            let mut root = gpui::div();
            if let Some(leading) = leading {
                root = root.text_size(px(32.)).line_height(px(leading));
            }
            root.child(
                gpui::div()
                    .debug_selector(|| "accordion-box".to_owned())
                    .child(
                        Accordion::new(vec![AccordionItem::new("one", "Title")
                            .subtitle("Subtitle")
                            .content(
                                gpui::div()
                                    .debug_selector(|| "accordion-body-text".to_owned())
                                    .child("Body"),
                            )])
                        .id("line-box-accordion")
                        .default_expanded("one")
                        .hide_separator(true),
                    ),
            )
            .into_any_element()
        });
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("accordion-body-text").unwrap().size.height,
            px(20.)
        );
        assert_eq!(
            cx.debug_bounds("accordion-box").unwrap().size.height,
            px(104.)
        );
    }
}

/// A div probe: a full-width, 36px-tall clickable strip recording `label` when
/// pressed. It is *not* a tab stop (no `track_focus`), so it can be used as
/// body content without inserting its own stop into a keyboard walk.
fn probe(id: &'static str, label: &'static str, recorded: Events) -> gpui::AnyElement {
    gpui::div()
        .id(id)
        .w_full()
        .h(px(36.))
        .flex()
        .items_center()
        .cursor_pointer()
        .on_click(move |_, _, _| recorded.borrow_mut().push(label.to_owned()))
        .into_any_element()
}

/// Pushes the pending frame through. Keyboard navigation reads the *last
/// rendered frame*'s tab stops (`rendered_frame.tab_stops`), so a stop that
/// appears or disappears with a state change — an accordion body, a
/// disclosure body — must be painted before the next Tab can see it.
fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// Lets the shared 200ms Accordion/Disclosure panel motion reach its endpoint
/// before a coordinate-sensitive assertion. React Aria keeps a panel mounted
/// while that transition runs, so tests that click the settled body or the
/// trigger below it must advance the deterministic clock first.
fn settle_collapsible(cx: &mut VisualTestContext) {
    // Since gpui-pre 0.3.11 GPUI's animation phase and the retained panel
    // timer both run on the test executor's clock.
    cx.executor().advance_clock(Duration::from_millis(220));
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    cx.executor().advance_clock(Duration::from_millis(210));
    cx.run_until_parked();
    flush_frame(cx);
}

/// The keys of an expanded set joined in a stable order.
///
/// A `HashSet` iterates in no particular order, so asserting on a raw join
/// would be flaky; sorting makes the recorded report deterministic.
fn sorted_join(keys: &HashSet<SharedString>) -> String {
    let mut keys: Vec<String> = keys.iter().map(ToString::to_string).collect();
    keys.sort();
    keys.join(",")
}

/// The advance width of `text` shaped the way the components shape it: gpui's
/// default `.SystemUIFont` stack at `size` px and `weight` (copied from
/// `tests/collections.rs` — breadcrumb labels are laid out by the window's own
/// `WindowTextSystem`, so this measurement is the render's measurement).
fn text_width(system: &gpui::WindowTextSystem, text: &str, size: f32, weight: FontWeight) -> f32 {
    let run = gpui::TextRun {
        len: text.len(),
        font: Font {
            family: ".SystemUIFont".into(),
            features: FontFeatures::default(),
            weight,
            style: FontStyle::default(),
            fallbacks: None,
        },
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = system.shape_line(text.to_owned().into(), px(size), &[run], None);
    f32::from(line.width)
}

// ---------------------------------------------------------------------------
// Accordion
// ---------------------------------------------------------------------------
//
// The v3 page has no Accessibility section; the trigger is a native button, so
// the contract is: Tab reaches each trigger in turn, Enter and Space toggle
// it, and nothing else is bound — no arrow keys between triggers, no
// Home/End. Each trigger is its own tab stop in this port, which is the
// native-button model.

/// Tab reaches each trigger, Enter and Space both toggle, and the keys v3
/// never binds — the arrows, Home and End — must do nothing while a trigger
/// holds the focus. Every trigger in this port is its own tab stop, so Tab
/// steps one trigger at a time; the assertion that the arrows and Home/End
/// record nothing is what pins the *absence* of a roving-stop keyboard,
/// which is v3's button-model contract.
#[gpui::test]
fn accordion_keyboard_toggles_and_ignores_unbound_keys(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Accordion::new(vec![
            AccordionItem::new("alpha", "Alpha"),
            AccordionItem::new("beta", "Beta"),
        ])
        .id("nvd-acc-keys")
        .on_expanded_change(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    // Tab lands on the first trigger (nothing else on the page is a stop),
    // and Enter fires its click listener on key-up. The report is the
    // expanded set: "alpha" opens, then "" closes.
    press(cx, "tab");
    press(cx, "enter");
    flush_frame(cx);
    settle_collapsible(cx);
    assert_eq!(recorded.borrow().as_slice(), ["alpha"]);
    press(cx, "space");
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", ""],
        "Space on the focused trigger must toggle it shut again"
    );

    // With item 0 closed, the next Tab reaches the second trigger (the
    // closed body contributes no stop) and Space opens it.
    press(cx, "tab");
    press(cx, "space");
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "", "beta"],
        "Tab must step from the first trigger to the second, which Space opens"
    );

    // None of the keys v3's button-model never binds may do anything on a
    // focused trigger.
    press(cx, "right");
    press(cx, "left");
    press(cx, "up");
    press(cx, "down");
    press(cx, "home");
    press(cx, "end");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "", "beta"],
        "an accordion trigger is a button: the arrows and Home/End must not \
         move the focus or toggle anything"
    );
}

/// `defaultExpandedKeys` seeds the *uncontrolled* set — the accordion owns it
/// and toggles itself on press. The proof runs backwards: the first press on
/// the seeded-open trigger must report the empty set (it was open to begin
/// with), and a second press on the other trigger must report only that key.
#[gpui::test]
fn accordion_default_expanded_seed_opens_and_toggles(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Accordion::new(vec![
            AccordionItem::new("alpha", "Alpha").content(gpui::div().h(px(40.))),
            AccordionItem::new("beta", "Beta").content(gpui::div().h(px(40.))),
        ])
        .id("nvd-acc-seed")
        // v3's `defaultExpandedKeys` — no `expanded_keys`, so the accordion
        // holds the set itself.
        .default_expanded_keys(HashSet::from(["alpha".into()]))
        .on_expanded_change(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    // Header 0 spans y 0..52 (py-4 twice around a 20px line), centre y 26.
    // The seed means it is open from the first frame, so the first press
    // reports the empty set — a closed item would have reported "alpha".
    click(cx, 60., 26.);
    flush_frame(cx);
    settle_collapsible(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        [""],
        "the seed must leave the item open, so the first press closes it"
    );

    // Item 0 now closed, item 1's header sits at 52 + 1 (separator) + half a
    // header = 79. Pressing it opens beta and only beta — the accordion's
    // own state, not a caller's set, answered.
    click(cx, 60., 79.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["", "beta"],
        "the open item must have been closed before the second press, whose \
         report is the set the accordion itself now holds"
    );
}

#[gpui::test]
fn accordion_item_owns_default_disabled_change_and_indicator_state(cx: &mut TestAppContext) {
    let changed = events();
    let reported = changed.clone();
    let indicator_states = events();
    let observed = indicator_states.clone();
    let cx = open_host(cx, move || {
        let changed = changed.clone();
        let first_states = indicator_states.clone();
        let second_states = indicator_states.clone();
        Accordion::new(vec![
            AccordionItem::new("alpha", "Alpha")
                .default_expanded(true)
                .content(gpui::div().h(px(40.)))
                .indicator(move |state, _, _| {
                    first_states
                        .borrow_mut()
                        .push(format!("alpha:{}:{}", state.is_expanded, state.is_disabled));
                    gpui::div()
                        .child(if state.is_expanded { "−" } else { "+" })
                        .into_any_element()
                })
                .on_expanded_change(move |expanded, _, _| {
                    changed.borrow_mut().push(expanded.to_string());
                }),
            AccordionItem::new("beta", "Beta")
                .is_disabled(true)
                .indicator(move |state, _, _| {
                    second_states
                        .borrow_mut()
                        .push(format!("beta:{}:{}", state.is_expanded, state.is_disabled));
                    gpui::div().child("+").into_any_element()
                }),
        ])
        .id("nvd-accordion-item-contract")
        .into_any_element()
    });

    assert!(observed
        .borrow()
        .iter()
        .any(|state| state == "alpha:true:false"));
    assert!(observed
        .borrow()
        .iter()
        .any(|state| state == "beta:false:true"));

    click(cx, 60., 26.);
    flush_frame(cx);
    assert_eq!(reported.borrow().as_slice(), ["false"]);
    assert!(observed
        .borrow()
        .iter()
        .any(|state| state == "alpha:false:false"));

    click(cx, 60., 79.);
    assert_eq!(
        reported.borrow().as_slice(),
        ["false"],
        "the item-level disabled prop must block its trigger and callback"
    );
}

/// A focusable control inside an open item's body is part of the Tab walk;
/// inside a closed item it does not exist — the port unmounts the body. v3's
/// migration page says content is "always mounted in v3", but the react-aria
/// panel is `hidden` when collapsed, so the Tab contract is the same: reach
/// it open, skip it closed. Both halves are asserted.
#[gpui::test]
fn accordion_body_control_tab_reachable_open_skipped_closed(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Accordion::new(vec![
            AccordionItem::new("alpha", "Alpha"),
            AccordionItem::new("beta", "Beta").content(
                Button::new("nvd-acc-body-btn")
                    .label("Go")
                    .on_press(move |_, _, _| recorded.borrow_mut().push("body".to_owned())),
            ),
        ])
        .id("nvd-acc-body")
        .on_expanded_change(move |_, _, _| {})
        .into_any_element()
    });

    // Both closed: header 1 spans y 53..105 (52px + 1px separator). If item
    // beta were open, its body's button would sit at y 107..143 (52 + 1 +
    // pt-2 + 36) — a click there must record nothing while closed.
    click(cx, 60., 125.);
    assert!(
        recorded.borrow().is_empty(),
        "a closed item's body must not exist: where its button would be, a \
         click records nothing"
    );

    // Tab reaches trigger 0, then trigger 1 directly — the closed body
    // contributes no stop. Enter opens beta, reporting through the accordion.
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    flush_frame(cx);
    settle_collapsible(cx);

    // The body is mounted now, so its Button is the next stop after the
    // trigger: Tab from the trigger reaches it and Enter fires *it*.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["body"],
        "an open item's body control must join the Tab walk after its trigger"
    );

    // And the same seat answers the pointer.
    click(cx, 60., 125.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["body", "body"],
        "the open body's button must answer a click at its laid-out place"
    );
}

/// v3's API table documents `allowsMultipleExpanded` with default `false` —
/// expanding one item collapses the others — but this port defaults it to
/// `true` (a parity note, not a keyboard gap). Both halves of the flipped
/// default are asserted in one host: a plain accordion (prop unset)
/// collapses the first panel when the second opens, while
/// `.allows_multiple_expanded(true)` reports both keys at once. The
/// accordions are stacked 100px apart so each header's seat is a constant,
/// and they use distinct item keys — two instances with the same keys share
/// gpui's component-namespaced keyed state (the silent collision AGENTS.md
/// warns about), which would make the second one inert without proving
/// anything about the default.
#[gpui::test]
fn accordion_default_single_expand_and_opt_in_multiple(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let items = || {
            vec![
                AccordionItem::new("one", "Item one").content(gpui::div().h(px(40.))),
                AccordionItem::new("two", "Item two").content(gpui::div().h(px(40.))),
            ]
        };
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(100.))
            .child(
                // Prop unset: the v3 default, single-expand.
                Accordion::new(items())
                    .id("nvd-acc-single-default")
                    .on_expanded_change(move |keys, _, _| {
                        recorded.borrow_mut().push(sorted_join(keys));
                    }),
            )
            .child({
                let recorded = for_view.clone();
                Accordion::new(vec![
                    AccordionItem::new("three", "Item three").content(gpui::div().h(px(40.))),
                    AccordionItem::new("four", "Item four").content(gpui::div().h(px(40.))),
                ])
                .id("nvd-acc-multi-optin")
                .allows_multiple_expanded(true)
                .on_expanded_change(move |keys, _, _| {
                    recorded.borrow_mut().push(sorted_join(keys));
                })
            })
            .into_any_element()
    });

    // Row 0 starts at the origin: header 0 centre y 26; with item one open
    // its 58px body (pt-2 + 40 + pb-4) pushes header 1 down to centre 137.
    // The second press must report exactly "two" — the first panel collapsed.
    click(cx, 60., 26.);
    flush_frame(cx);
    settle_collapsible(cx);
    click(cx, 60., 137.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["one", "two"],
        "with the prop unset (v3's default false) opening the second item \
         must collapse the first"
    );

    // Row 1 starts at y 263 (row 0 ends at 163 + the 100px gap): header 0
    // centre 289, and with item three open header 1 centre 400. The same two
    // presses report both keys this time — the opt-in multiple mode
    // (sorted_join puts "four" before "three").
    click(cx, 60., 289.);
    flush_frame(cx);
    settle_collapsible(cx);
    click(cx, 60., 400.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["one", "two", "three", "four,three"],
        ".allows_multiple_expanded(true) must keep both panels open"
    );
}

/// Two `Accordion`s with *identical* item keys on one page collide: gpui's
/// `RenderOnce` prepaint namespaces keyed state by the component's type
/// name, not by the instance's id, so keys derived from the item keys alone
/// (`acc-one`, `acc-one-focus`) resolve to the same slot in both instances
/// and the second one answers no clicks. The fix keys every slot by the
/// accordion's own id first; these two instances carry distinct ids but
/// identical item keys, so any residual sharing shows up here. Clicking the
/// second's identically-keyed header must toggle only the second — recorded
/// in its own recorder, from its own set — and leave the first, whose panel
/// stays open throughout, completely untouched.
#[gpui::test]
fn accordion_identical_item_keys_stay_independent_per_instance(cx: &mut TestAppContext) {
    let first = events();
    let second = events();
    let probed = events();
    let for_first = first.clone();
    let for_second = second.clone();
    let for_probed = probed.clone();
    let cx = open_host(cx, move || {
        let probed = for_probed.clone();
        let items = move || {
            // The probe is the open body's seat: a click there later only
            // records while item "one" is still open.
            let probed = probed.clone();
            vec![
                AccordionItem::new("one", "Item one").content(probe(
                    "nvd-acc-dup-1-body",
                    "one-open",
                    probed,
                )),
                AccordionItem::new("two", "Item two").content(gpui::div().h(px(40.))),
            ]
        };
        // Distinct ids, identical item keys — the collision AGENTS.md
        // documents for TagGroup, here triggered by caller-supplied keys.
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(100.))
            .child({
                let first = for_first.clone();
                Accordion::new(items())
                    .id("nvd-acc-dup-1")
                    .on_expanded_change(move |keys, _, _| {
                        first.borrow_mut().push(sorted_join(keys));
                    })
            })
            .child({
                let second = for_second.clone();
                Accordion::new(items())
                    .id("nvd-acc-dup-2")
                    .on_expanded_change(move |keys, _, _| {
                        second.borrow_mut().push(sorted_join(keys));
                    })
            })
            .into_any_element()
    });

    // Row 0: header 0 spans y 0..52 (centre 26). Opening item "one" must be
    // recorded by the first accordion only.
    click(cx, 60., 26.);
    flush_frame(cx);
    settle_collapsible(cx);
    assert_eq!(first.borrow().as_slice(), ["one"]);
    assert!(
        second.borrow().is_empty(),
        "the second accordion's recorder must stay silent when the first is \
         clicked — got {:?}",
        second.borrow()
    );

    // Row 1 starts at y 260 (row 0 ends at 160 with the probe body + the
    // 100px gap): header 0 centre 286. The identically-keyed header must
    // answer the click and open the *second* accordion's own item, echoing
    // nothing into the first's recorder.
    click(cx, 60., 286.);
    flush_frame(cx);
    settle_collapsible(cx);
    assert_eq!(
        second.borrow().as_slice(),
        ["one"],
        "the second accordion's identically-keyed header must answer the \
         click and open its own item"
    );
    assert_eq!(
        first.borrow().as_slice(),
        ["one"],
        "opening the second accordion must leave the first's set untouched"
    );

    // A second press on the same seat closes it again — the second accordion
    // answers the pointer on its own, start to finish.
    click(cx, 60., 286.);
    flush_frame(cx);
    settle_collapsible(cx);
    assert_eq!(
        second.borrow().as_slice(),
        ["one", ""],
        "the second accordion must close its own item on the next press"
    );
    assert_eq!(
        first.borrow().as_slice(),
        ["one"],
        "closing the second accordion's item must not close the first's"
    );

    // And the first is not just silent, it is *open*: with item "one" closed
    // in the second, the first still renders its body probe at y 55..91
    // (pt-2 + a 36px probe inside the 54px body), centre 73.
    click(cx, 60., 73.);
    assert_eq!(
        probed.borrow().as_slice(),
        ["one-open"],
        "the first accordion's item must still be open after the second \
         accordion's whole open/close cycle"
    );
    assert_eq!(
        first.borrow().as_slice(),
        ["one"],
        "the probe press must not have toggled the first accordion itself"
    );
}

// ---------------------------------------------------------------------------
// DisclosureGroup
// ---------------------------------------------------------------------------
//
// v3 documents `expandedKeys`/`defaultExpandedKeys`/`onExpandedChange`,
// `allowsMultipleExpanded` (default false: expanding one collapses the rest)
// and `isDisabled`. The group owns its set when `expandedKeys` is absent and
// reports the complete next set from both controlled and uncontrolled modes.

#[gpui::test]
fn disclosure_uncontrolled_press_opens_and_closes_its_body(cx: &mut TestAppContext) {
    let pressed = events();
    let probes = pressed.clone();
    let cx = open_host(cx, move || {
        let pressed = pressed.clone();
        Disclosure::new("nav-uncontrolled-disclosure", "Details")
            .child(probe("nvd-uncontrolled-body", "body", pressed))
            .into_any_element()
    });

    click(cx, 60., 18.);
    flush_frame(cx);
    settle_collapsible(cx);
    click(cx, 60., 62.);
    assert_eq!(probes.borrow().as_slice(), ["body"]);

    click(cx, 60., 18.);
    flush_frame(cx);
    settle_collapsible(cx);
    click(cx, 60., 62.);
    assert_eq!(
        probes.borrow().as_slice(),
        ["body"],
        "the body must leave the hit-test tree after the uncontrolled close"
    );
}

/// React Aria keeps a disclosure panel mounted until its 200ms height/opacity
/// transition finishes, then applies `hidden`. The GPUI port must preserve
/// that lifetime so an in-flight close does not tear down measured content or
/// leave the keyed animation without a child tree.
#[gpui::test]
fn disclosure_panel_retains_body_until_exit_motion_finishes(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        Disclosure::new("nav-disclosure-motion", "Details")
            .default_expanded(true)
            .child(
                gpui::div()
                    .h(px(36.))
                    .debug_selector(|| "disclosure-motion-body".to_owned()),
            )
            .into_any_element()
    });

    assert!(cx.debug_bounds("disclosure-motion-body").is_some());
    click(cx, 60., 18.);
    flush_frame(cx);
    assert!(
        cx.debug_bounds("disclosure-motion-body").is_some(),
        "the body must remain mounted during the 200ms close transition"
    );

    // Reopening before the stale close timer fires must cancel that timer and
    // retarget the same keyed motion slot from its live frame.
    click(cx, 60., 18.);
    flush_frame(cx);
    settle_collapsible(cx);
    assert!(
        cx.debug_bounds("disclosure-motion-body").is_some(),
        "a reopened panel must survive the stale close timer"
    );

    click(cx, 60., 18.);
    flush_frame(cx);
    settle_collapsible(cx);
    assert!(
        cx.debug_bounds("disclosure-motion-body").is_none(),
        "the body must leave the tree after the retained exit lifetime"
    );
}

#[gpui::test]
fn disclosure_default_expanded_seeds_uncontrolled_state(cx: &mut TestAppContext) {
    let pressed = events();
    let probes = pressed.clone();
    let cx = open_host(cx, move || {
        let pressed = pressed.clone();
        Disclosure::new("nav-default-disclosure", "Details")
            .default_expanded(true)
            .child(probe("nvd-default-body", "body", pressed))
            .into_any_element()
    });

    click(cx, 60., 62.);
    assert_eq!(probes.borrow().as_slice(), ["body"]);

    click(cx, 60., 18.);
    flush_frame(cx);
    settle_collapsible(cx);
    click(cx, 60., 62.);
    assert_eq!(
        probes.borrow().as_slice(),
        ["body"],
        "the default seed must not be reapplied after the disclosure closes"
    );
}

#[gpui::test]
fn disclosure_content_render_prop_receives_live_expanded_state(cx: &mut TestAppContext) {
    let rendered = events();
    let observed = rendered.clone();
    let cx = open_host(cx, move || {
        let rendered = rendered.clone();
        Disclosure::new("nav-render-disclosure", "Details")
            .content(move |state| {
                rendered.borrow_mut().push(format!(
                    "expanded:{} disabled:{}",
                    state.is_expanded, state.is_disabled
                ));
                gpui::div().h(px(36.)).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        observed.borrow().last().map(String::as_str),
        Some("expanded:false disabled:false")
    );
    click(cx, 60., 18.);
    flush_frame(cx);
    assert_eq!(
        observed.borrow().last().map(String::as_str),
        Some("expanded:true disabled:false"),
        "the render closure must receive the disclosure's updated owned state"
    );
}

#[gpui::test]
fn disclosure_content_render_prop_waits_for_controlled_owner_feedback(cx: &mut TestAppContext) {
    let owner = Rc::new(Cell::new(true));
    let owner_for_view = owner.clone();
    let rendered = events();
    let observed = rendered.clone();
    let changes = events();
    let proposed = changes.clone();
    let cx = open_host(cx, move || {
        let rendered = rendered.clone();
        let changes = changes.clone();
        Disclosure::new("nav-render-controlled-disclosure", "Details")
            .is_expanded(owner_for_view.get())
            .on_expanded_change(move |expanded, _, _| {
                changes.borrow_mut().push(expanded.to_string());
            })
            .content(move |state| {
                rendered.borrow_mut().push(state.is_expanded.to_string());
                gpui::div().h(px(36.)).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(observed.borrow().last().map(String::as_str), Some("true"));
    click(cx, 60., 18.);
    flush_frame(cx);
    assert_eq!(proposed.borrow().as_slice(), ["false"]);
    assert_eq!(
        observed.borrow().last().map(String::as_str),
        Some("true"),
        "the render closure must keep the controlled owner value after an unaccepted proposal"
    );

    owner.set(false);
    flush_frame(cx);
    assert_eq!(
        observed.borrow().last().map(String::as_str),
        Some("false"),
        "the render closure must update once the owner feeds the proposed value back"
    );
}

#[gpui::test]
fn disclosure_content_render_prop_receives_disabled_state(cx: &mut TestAppContext) {
    let rendered = events();
    let observed = rendered.clone();
    let cx = open_host(cx, move || {
        let rendered = rendered.clone();
        Disclosure::new("nav-render-disabled-disclosure", "Details")
            .is_disabled(true)
            .content(move |state| {
                rendered.borrow_mut().push(format!(
                    "expanded:{} disabled:{}",
                    state.is_expanded, state.is_disabled
                ));
                gpui::div().h(px(36.)).into_any_element()
            })
            .into_any_element()
    });

    assert_eq!(
        observed.borrow().last().map(String::as_str),
        Some("expanded:false disabled:true")
    );
    click(cx, 60., 18.);
    flush_frame(cx);
    assert!(
        observed
            .borrow()
            .iter()
            .all(|state| state == "expanded:false disabled:true"),
        "a disabled trigger must not propose an expanded render state"
    );
}

// `redundant_clone` falsely fires on the `toggled.clone()` below: the host
// re-renders the `Fn` content closure every frame, so the recorder must stay
// in its environment and each frame's inner closure needs a fresh copy — the
// clone is load-bearing, and "used once" only looks at one call.
#[allow(clippy::redundant_clone)]
#[gpui::test]
fn disclosure_group_default_seed_and_single_mode_replace_the_open_item(cx: &mut TestAppContext) {
    let toggled = events();
    let reported = toggled.clone();
    let pressed = events();
    let probes = pressed.clone();
    let cx = open_host(cx, move || {
        let toggled = toggled.clone();
        let pressed = pressed.clone();
        DisclosureGroup::new("nav-disclosure-group")
            .default_expanded_keys(["dga"])
            .item(
                "dga",
                "Alpha",
                probe("nvd-dg-probe-a", "A-body", pressed.clone()),
            )
            .item("dgb", "Beta", probe("nvd-dg-probe-b", "B-body", pressed))
            .on_expanded_change(move |keys, _window, _| {
                toggled.borrow_mut().push(sorted_join(keys));
            })
            .into_any_element()
    });

    // Alpha begins open from `defaultExpandedKeys`, so its body pushes Beta's
    // trigger to y 88..124. Tab reaches Alpha, then Beta; Enter replaces Alpha
    // with Beta because the default mode allows only one expanded item.
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    flush_frame(cx);
    settle_collapsible(cx);
    assert_eq!(
        reported.borrow().as_slice(),
        ["dgb"],
        "the default single-expand mode must report only the newly opened key"
    );

    // Alpha is now closed: Beta moves to y 36..72 and its open body sits at
    // y 72..124 with the probe at 80..116 (centre 98). Reaching that probe
    // proves the uncontrolled set, not merely the callback, changed.
    click(cx, 60., 98.);
    assert_eq!(
        probes.borrow().as_slice(),
        ["B-body"],
        "the newly expanded body must replace the seeded body in layout"
    );
}

#[gpui::test]
fn disclosure_group_controlled_value_waits_for_the_owner(cx: &mut TestAppContext) {
    let changed = events();
    let reported = changed.clone();
    let pressed = events();
    let probes = pressed.clone();
    let cx = open_host(cx, move || {
        let changed = changed.clone();
        let pressed = pressed.clone();
        DisclosureGroup::new("nav-controlled-disclosure-group")
            .expanded_keys(["dga"])
            .item(
                "dga",
                "Alpha",
                probe("nvd-controlled-a", "A-body", pressed.clone()),
            )
            .item("dgb", "Beta", probe("nvd-controlled-b", "B-body", pressed))
            .on_expanded_change(move |keys, _, _| {
                changed.borrow_mut().push(sorted_join(keys));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    flush_frame(cx);
    settle_collapsible(cx);
    assert_eq!(
        reported.borrow().as_slice(),
        ["dgb"],
        "a controlled group must report the proposed replacement set"
    );

    click(cx, 60., 62.);
    assert_eq!(
        probes.borrow().as_slice(),
        ["A-body"],
        "the controlled body must stay on the owner's value until it is accepted"
    );
}

#[gpui::test]
fn disclosure_group_multiple_mode_keeps_both_items_open(cx: &mut TestAppContext) {
    let changed = events();
    let reported = changed.clone();
    let pressed = events();
    let probes = pressed.clone();
    let cx = open_host(cx, move || {
        let changed = changed.clone();
        let pressed = pressed.clone();
        DisclosureGroup::new("nav-multiple-disclosure-group")
            .allows_multiple_expanded(true)
            .default_expanded_keys(["dga"])
            .item(
                "dga",
                "Alpha",
                probe("nvd-multiple-a", "A-body", pressed.clone()),
            )
            .item("dgb", "Beta", probe("nvd-multiple-b", "B-body", pressed))
            .on_expanded_change(move |keys, _, _| {
                changed.borrow_mut().push(sorted_join(keys));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    flush_frame(cx);
    settle_collapsible(cx);
    assert_eq!(reported.borrow().as_slice(), ["dga,dgb"]);

    click(cx, 60., 62.);
    click(cx, 60., 150.);
    assert_eq!(
        probes.borrow().as_slice(),
        ["A-body", "B-body"],
        "multiple mode must retain both expanded bodies in layout"
    );
}

#[gpui::test]
fn disclosure_group_disabled_blocks_every_trigger(cx: &mut TestAppContext) {
    let changed = events();
    let reported = changed.clone();
    let cx = open_host(cx, move || {
        let changed = changed.clone();
        DisclosureGroup::new("nav-disabled-disclosure-group")
            .is_disabled(true)
            .item("dga", "Alpha", gpui::div().h(px(36.)))
            .item("dgb", "Beta", gpui::div().h(px(36.)))
            .on_expanded_change(move |keys, _, _| {
                changed.borrow_mut().push(sorted_join(keys));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    click(cx, 60., 54.);
    press(cx, "tab");
    press(cx, "enter");
    assert!(
        reported.borrow().is_empty(),
        "a disabled group must expose no actionable trigger"
    );
}

#[gpui::test]
fn disclosure_group_single_mode_normalizes_a_multi_key_default(cx: &mut TestAppContext) {
    let changed = events();
    let reported = changed.clone();
    let pressed = events();
    let probes = pressed.clone();
    let cx = open_host(cx, move || {
        let changed = changed.clone();
        let pressed = pressed.clone();
        DisclosureGroup::new("nav-normalized-disclosure-group")
            .default_expanded_keys(["dga", "dgb"])
            .item(
                "dga",
                "Alpha",
                probe("nvd-normalized-a", "A-body", pressed.clone()),
            )
            .item("dgb", "Beta", probe("nvd-normalized-b", "B-body", pressed))
            .on_expanded_change(move |keys, _, _| {
                changed.borrow_mut().push(sorted_join(keys));
            })
            .into_any_element()
    });

    flush_frame(cx);
    assert_eq!(
        reported.borrow().as_slice(),
        ["dga"],
        "normalizing an invalid default must report the retained key once"
    );
    click(cx, 60., 62.);
    click(cx, 60., 150.);
    assert_eq!(
        probes.borrow().as_slice(),
        ["A-body"],
        "single mode must retain only the first matching default key"
    );
}

#[gpui::test]
fn disclosure_group_duplicate_titles_keep_distinct_key_identity(cx: &mut TestAppContext) {
    let changed = events();
    let reported = changed.clone();
    let cx = open_host(cx, move || {
        let changed = changed.clone();
        DisclosureGroup::new("nav-duplicate-title-disclosure-group")
            .item("dga", "Details", gpui::div().h(px(36.)))
            .item("dgb", "Details", gpui::div().h(px(36.)))
            .on_expanded_change(move |keys, _, _| {
                changed.borrow_mut().push(sorted_join(keys));
            })
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        reported.borrow().as_slice(),
        ["dgb"],
        "equal labels must not collapse distinct keyed trigger identities"
    );
}

// ---------------------------------------------------------------------------
// Breadcrumbs
// ---------------------------------------------------------------------------

/// Remaining source-text check: `aria-current` reaches only the AccessKit
/// tree, which the headless platform never builds (`a11y_deep.rs`).
#[test]
fn current_breadcrumb_publishes_accesskit_current_page_state() {
    let source = source_scan::component_src("breadcrumbs.rs");
    assert!(source.contains("a11y_current(a11y::AriaCurrent::Page)"));
    assert!(source.contains(".when(is_last"));
}

//
// v3's API table documents no overflow/collapse prop (no `maxItems`-style
// truncation anywhere on the page), so the port's always-visible single row
// matches v3: `.breadcrumbs` is `flex items-center` with no wrap, and each
// `.breadcrumbs__item` is `shrink-0`. The last-crumb rule is pinned in
// `collections.rs`; here: a disabled breadcrumb answers no press, and the
// Accessibility section's "keyboard navigation support" claim is checked
// against a port whose crumbs carry no focus handles at all.

#[gpui::test]
fn breadcrumbs_disabled_answers_no_click(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Breadcrumbs::new(vec![
            Crumb::new("Build"),
            Crumb::new("Deploy"),
            Crumb::new("Live"),
        ])
        .is_disabled(true)
        .on_navigate(move |index, crumb, _, _, _| {
            recorded
                .borrow_mut()
                .push(format!("{index}:{}", crumb.label));
        })
        .into_any_element()
    });

    // v3: `isDisabled` "disables all links". The labels are measured with the
    // window's own text system at the link's MEDIUM weight; a label centres
    // at (w/2, 10) inside its unpadded row on a 20px line, and the second row
    // starts at w_build + 22 (4px gap + 12px separator + 6px root gap).
    // Neither may record.
    let w_build =
        cx.update(|window, _| text_width(window.text_system(), "Build", 14.0, FontWeight::MEDIUM));
    let w_deploy =
        cx.update(|window, _| text_width(window.text_system(), "Deploy", 14.0, FontWeight::MEDIUM));
    click(cx, w_build / 2., 10.);
    click(cx, w_build + 22. + w_deploy / 2., 10.);
    assert!(
        recorded.borrow().is_empty(),
        "a disabled breadcrumb must not answer a press on any non-last crumb"
    );
}

/// A disabled control "does not activate or enter the tab order"
/// (`docs/agents/components.md`), and v3's `isDisabled` "disables all links" —
/// so a disabled bar must contribute no crumb tab stops at all. The walk
/// proves the absence directly: with a Button after the bar, the first Tab
/// must land on the Button (no crumb stop may sit between the root and it),
/// and Enter must activate only that Button.
#[gpui::test]
fn breadcrumbs_disabled_bar_contributes_no_tab_stops(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let for_button = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let for_button = for_button.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(
                Breadcrumbs::new(vec![
                    Crumb::new("Build").href("#/build"),
                    Crumb::new("Deploy").href("#/deploy"),
                    Crumb::new("Live"),
                ])
                .is_disabled(true)
                .on_navigate(move |index, crumb, _, _, _| {
                    recorded
                        .borrow_mut()
                        .push(format!("{index}:{}", crumb.label));
                }),
            )
            .child(
                Button::new("bc-after-disabled")
                    .label("After")
                    .on_press(move |_, _, _| for_button.borrow_mut().push("after".into())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["after"],
        "the first Tab after a disabled bar must reach the control following \
         it — no crumb, not even one carrying an `href`, may hold a tab stop"
    );
}

/// v3's Breadcrumbs Accessibility section: "Breadcrumbs uses React Aria
/// Components' Breadcrumbs primitive, which provides: ... **Keyboard
/// navigation support**", and "The last breadcrumb item (without `href`)
/// automatically becomes the current page indicator." Every crumb that
/// carries an `href` is a link, so Tab reaches each in turn and Enter
/// activates it; only the last — the current page — is inert and stays out
/// of the walk. The fixture has three crumbs, two of them with `href`s, so
/// two Tab+Enter pairs must navigate two crumbs.
#[gpui::test]
fn breadcrumbs_tab_reaches_each_link_and_enter_navigates(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Breadcrumbs::new(vec![
            Crumb::new("Build").href("#/build"),
            Crumb::new("Deploy").href("#/deploy"),
            Crumb::new("Live"),
        ])
        .on_navigate(move |index, crumb, _, _, _| {
            recorded
                .borrow_mut()
                .push(format!("{index}:{}", crumb.label));
        })
        .into_any_element()
    });

    // The documented contract: Tab reaches each href crumb in turn and Enter
    // activates it (React Aria's press on a link). The last crumb has no
    // `href` — it is the current page — so the walk stops after the second.
    press(cx, "tab");
    press(cx, "enter");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["0:Build", "1:Deploy"],
        "Tab must reach each href crumb so that Enter navigates it, and the \
         last crumb, as the current page, must stay out of the walk"
    );
}

/// Upstream renders a Link for *every* non-current crumb, including the span
/// link with neither `href` nor `onAction` — so such a crumb must remain a
/// keyboard-reachable link with the same focus-visible inputs as any other,
/// while firing nothing at all: no navigation, no URL, no callback.
#[gpui::test]
fn breadcrumbs_plain_crumb_is_a_focusable_link_that_fires_nothing(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        // No `href`s and no `on_navigate`: every non-current crumb is a
        // span link, so the walk must still reach it and Enter must still
        // do nothing with it.
        Breadcrumbs::new(vec![
            Crumb::new("Build"),
            Crumb::new("Deploy"),
            Crumb::new("Live"),
        ])
        .into_any_element()
    });

    press(cx, "tab");
    cx.update(|_, cx| {
        assert!(
            herogpui_components::extend::focus_visible(cx),
            "a plain span-link crumb must take keyboard focus like any other \
             link"
        );
    });
    press(cx, "enter");
    assert!(
        cx.opened_url().is_none(),
        "a crumb with neither href nor on_navigate must fire nothing on \
         activation"
    );
}

/// RAC's collection gives every item its own key and hands `onAction` that
/// `node.key`, so two Breadcrumbs instances in one window must never share
/// tab stops or element states. The port used to derive both the focus
/// handles and the link ids from bare `crumb-{i}` literals, which made the
/// second instance re-use the first one's identity.
#[gpui::test]
fn breadcrumbs_instances_keep_distinct_tab_stops(cx: &mut TestAppContext) {
    let recorded = events();
    let for_a = recorded.clone();
    let for_b = recorded.clone();
    let cx = open_host(cx, move || {
        let for_a = for_a.clone();
        let for_b = for_b.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(
                Breadcrumbs::new(vec![
                    Crumb::new("One"),
                    Crumb::new("Two"),
                    Crumb::new("Three"),
                ])
                .on_navigate(move |index, crumb, _, _, _| {
                    for_a.borrow_mut().push(format!("a{index}:{}", crumb.label));
                }),
            )
            .child(
                Breadcrumbs::new(vec![
                    Crumb::new("Four"),
                    Crumb::new("Five"),
                    Crumb::new("Six"),
                ])
                .on_navigate(move |index, crumb, _, _, _| {
                    for_b.borrow_mut().push(format!("b{index}:{}", crumb.label));
                }),
            )
            .into_any_element()
    });

    // The walk covers instance A's two links before instance B's; each Enter
    // navigates exactly one link of exactly one instance.
    for expected in ["a0:One", "a1:Two", "b0:Four", "b1:Five"] {
        press(cx, "tab");
        press(cx, "enter");
        assert_eq!(
            recorded.borrow().last().map(String::as_str),
            Some(expected),
            "each Breadcrumbs instance must own its own tab stops and links"
        );
    }
    assert_eq!(
        recorded.borrow().len(),
        4,
        "Enter must navigate one link per press, never both instances"
    );
}

/// Upstream `Breadcrumbs.Item` renders a `Link`, and a Link that carries an
/// `href` navigates itself — with no `onAction` configured at all. The port's
/// transport for that navigation is `open_url`, so an `href` crumb must be
/// pressable (and open its URL) even when the builder sets no callback.
#[gpui::test]
fn breadcrumbs_href_crumb_opens_its_url_without_a_callback(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        Breadcrumbs::new(vec![
            Crumb::new("Build").href("#/build"),
            Crumb::new("Deploy").href("#/deploy"),
            Crumb::new("Live"),
        ])
        .into_any_element()
    });

    let w_build =
        cx.update(|window, _| text_width(window.text_system(), "Build", 14.0, FontWeight::MEDIUM));
    let w_deploy =
        cx.update(|window, _| text_width(window.text_system(), "Deploy", 14.0, FontWeight::MEDIUM));
    click(cx, w_build / 2., 10.);
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("#/build"),
        "an href crumb must open its URL on press even without on_navigate"
    );
    click(cx, w_build + 22. + w_deploy / 2., 10.);
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("#/deploy"),
        "each href crumb must open its own URL"
    );
}

/// An href crumb with a configured callback does both: it reports the press
/// through `on_navigate` (RAC's `onAction(node.key)`) *and* opens its URL —
/// RAC's press on an anchor link fires `onPress` in addition to navigation.
#[gpui::test]
fn breadcrumbs_href_crumb_reports_and_opens(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Breadcrumbs::new(vec![
            Crumb::new("Build").href("#/build"),
            Crumb::new("Deploy").href("#/deploy"),
            Crumb::new("Live"),
        ])
        .on_navigate(move |index, crumb, _, _, _| {
            recorded
                .borrow_mut()
                .push(format!("{index}:{}", crumb.label));
        })
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["0:Build"],
        "Enter on the focused href crumb must report the navigation"
    );
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("#/build"),
        "the same press must also open the crumb's URL"
    );
}

/// The current page is the *last* item positionally (RAC:
/// `isCurrent = node.nextKey == null`), not "the first item without `href`".
/// A last crumb that still carries an `href` is therefore inert: it is
/// disabled upstream (`isDisabled: isDisabled || isCurrent`), stays out of
/// the tab walk, and answers no press.
#[gpui::test]
fn breadcrumbs_last_crumb_is_current_even_with_an_href(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Breadcrumbs::new(vec![
            Crumb::new("Build"),
            Crumb::new("Deploy"),
            Crumb::new("Live").href("#/live"),
        ])
        .on_navigate(move |index, crumb, _, _, _| {
            recorded
                .borrow_mut()
                .push(format!("{index}:{}", crumb.label));
        })
        .into_any_element()
    });

    // Clicking the third (current, href-carrying) crumb must record nothing.
    // The labels are measured at the link's MEDIUM weight: the third label
    // starts flush in the third row, and rows two and three each begin
    // `w + 22` later (4px gap + 12px separator + 6px root gap).
    let w_build =
        cx.update(|window, _| text_width(window.text_system(), "Build", 14.0, FontWeight::MEDIUM));
    let w_deploy =
        cx.update(|window, _| text_width(window.text_system(), "Deploy", 14.0, FontWeight::MEDIUM));
    let w_live =
        cx.update(|window, _| text_width(window.text_system(), "Live", 14.0, FontWeight::MEDIUM));
    click(cx, w_build / 2., 10.);
    click(cx, w_build + 22. + w_deploy + 22. + w_live / 2., 10.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["0:Build"],
        "the last crumb is the current page and must not navigate even with \
         an href"
    );
    assert_ne!(
        cx.opened_url().as_deref(),
        Some("#/live"),
        "the current page's href must not be opened"
    );
}

/// v3.2.6's stylesheet, verbatim: `.breadcrumbs` is `flex items-center
/// gap-1.5` (no wrap), `.breadcrumbs__item` is `flex shrink-0 items-center
/// justify-center gap-1` with no padding, and `.breadcrumbs__link` is
/// `text-sm leading-5 font-medium` with no padding either. With a 12px
/// separator slot a row is therefore `w_label + 16` wide, labels start flush
/// in their rows, rows sit 6px apart, and every row shares one line.
#[gpui::test]
fn breadcrumbs_row_geometry_matches_v3_metrics(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        Breadcrumbs::new(vec![
            Crumb::new("Build").href("#/build"),
            Crumb::new("Deploy").href("#/deploy"),
            Crumb::new("Live"),
        ])
        .id("bc-geom")
        .into_any_element()
    });
    flush_frame(cx);

    let w_build =
        cx.update(|window, _| text_width(window.text_system(), "Build", 14.0, FontWeight::MEDIUM));
    let w_deploy =
        cx.update(|window, _| text_width(window.text_system(), "Deploy", 14.0, FontWeight::MEDIUM));
    let first = cx
        .debug_bounds("Name(\"bc-geom\")-item-0")
        .expect("the first item row must be laid out");
    let second = cx
        .debug_bounds("Name(\"bc-geom\")-item-1")
        .expect("the second item row must be laid out");
    let current = cx
        .debug_bounds("Name(\"bc-geom\")-item-2")
        .expect("the current item row must be laid out");

    assert!(
        (f32::from(first.size.width) - (w_build + 16.)).abs() < 1.,
        "a row must be the unpadded label plus the 4px gap and 12px \
         separator: {:?} vs {}",
        first.size.width,
        w_build + 16.
    );
    assert!(
        (f32::from(second.size.width) - (w_deploy + 16.)).abs() < 1.,
        "each row must scale with its own label"
    );
    assert!(
        (f32::from(second.origin.x)
            - (f32::from(first.origin.x) + f32::from(first.size.width) + 6.))
            .abs()
            < 1.,
        "the root must gap rows by `gap-1.5`, 6px"
    );
    assert_eq!(
        second.origin.y, current.origin.y,
        "the root must keep every row on one line: v3 does not wrap"
    );
}

/// `flex_shrink_0` on `.breadcrumbs__item` and no `flex-wrap` on the root mean
/// a narrow parent must never compress or fold the trail: rows keep their full
/// `w_label + 16` width and overflow the parent instead.
#[gpui::test]
fn breadcrumbs_rows_do_not_shrink_or_wrap_in_a_narrow_parent(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(80.))
            .child(
                Breadcrumbs::new(vec![
                    Crumb::new("Build").href("#/build"),
                    Crumb::new("Deploy").href("#/deploy"),
                    Crumb::new("Live"),
                ])
                .id("bc-narrow"),
            )
            .into_any_element()
    });
    flush_frame(cx);

    let w_build =
        cx.update(|window, _| text_width(window.text_system(), "Build", 14.0, FontWeight::MEDIUM));
    let w_deploy =
        cx.update(|window, _| text_width(window.text_system(), "Deploy", 14.0, FontWeight::MEDIUM));
    let first = cx
        .debug_bounds("Name(\"bc-narrow\")-item-0")
        .expect("the first row must be laid out");
    let second = cx
        .debug_bounds("Name(\"bc-narrow\")-item-1")
        .expect("the second row must be laid out");

    assert!(
        (f32::from(first.size.width) - (w_build + 16.)).abs() < 1.,
        "a row wider than its parent must keep its full width, not shrink: \
         {:?} vs {}",
        first.size.width,
        w_build + 16.
    );
    assert!(
        (f32::from(second.size.width) - (w_deploy + 16.)).abs() < 1.,
        "no row may compress to fit the parent"
    );
    assert_eq!(
        second.origin.y, first.origin.y,
        "a narrow parent must not fold the trail onto a second line"
    );
}

/// v3's `separator?: ReactNode` accepts any node; the port narrows it to a
/// per-index render closure whose output paints inside the 12px, muted
/// `breadcrumbs__separator` slot under a stable per-instance id. A narrower
/// custom node must still leave the row `w_label + 16` wide (the slot fixes
/// the geometry, not the content), the last crumb must get no separator, and
/// the custom separator must not disturb the link presses.
#[gpui::test]
fn breadcrumbs_custom_separator_renders_per_item_in_the_slot(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let for_view = for_view.clone();
        Breadcrumbs::new(vec![
            Crumb::new("Build").href("#/build"),
            Crumb::new("Deploy").href("#/deploy"),
            Crumb::new("Live"),
        ])
        .id("bc-sep")
        .separator_render(move |i| {
            let for_view = for_view.clone();
            gpui::div()
                .id(gpui::ElementId::Name(format!("bc-sep-custom-{i}").into()))
                .size(px(8.))
                .bg(gpui::black())
                .on_click(move |_, _, _| {
                    for_view.borrow_mut().push(format!("separator-{i}"));
                })
                .into_any_element()
        })
        .into_any_element()
    });
    flush_frame(cx);

    let w_build =
        cx.update(|window, _| text_width(window.text_system(), "Build", 14.0, FontWeight::MEDIUM));
    let first = cx
        .debug_bounds("Name(\"bc-sep\")-item-0")
        .expect("the first item row must be laid out");
    let slot = cx
        .debug_bounds("Name(\"bc-sep\")-separator-0")
        .expect("the custom separator must paint in its slot");
    let second_slot = cx.debug_bounds("Name(\"bc-sep\")-separator-1");

    assert!(
        (f32::from(slot.size.width) - 12.).abs() < 1.
            && (f32::from(slot.size.height) - 12.).abs() < 1.,
        "a custom separator must paint inside v3's 12px `size-3` slot: {:?}",
        slot.size
    );
    assert!(
        (f32::from(first.size.width) - (w_build + 16.)).abs() < 1.,
        "the slot fixes a row's geometry regardless of the custom content"
    );
    assert!(
        second_slot.is_some(),
        "the second non-current crumb must get its own separator instance"
    );
    assert!(
        cx.debug_bounds("Name(\"bc-sep\")-separator-2").is_none(),
        "the current (last) crumb must get no separator"
    );

    click(cx, w_build / 2., 10.);
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("#/build"),
        "a custom separator must not intercept the crumb's press"
    );
    // The first slot centres at w_build + 10: the unpadded label, the 4px
    // gap, then half the 12px slot. The click must reach the custom node the
    // closure built for index 0.
    click(cx, w_build + 10., 10.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["separator-0"],
        "the custom node the closure built for one index must be live and \
         record its own press"
    );
}

/// The port draws the `:focus-visible` status ring on a keyboard-focused crumb
/// (the Link precedent — v3.2.4's breadcrumbs CSS defines no focus rule of its
/// own). Its two paint inputs are `is_focused` and the focus-visible flag, so
/// this proves both: a mouse press must leave the flag off, a keyboard focus
/// must arm it, and the Tab that armed it must have landed on the crumb the
/// following Enter navigates.
#[gpui::test]
fn breadcrumbs_focus_ring_inputs_track_the_input_modality(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Breadcrumbs::new(vec![
            Crumb::new("Build").href("#/build"),
            Crumb::new("Live"),
        ])
        .on_navigate(move |index, crumb, _, _, _| {
            recorded
                .borrow_mut()
                .push(format!("{index}:{}", crumb.label));
        })
        .into_any_element()
    });

    let w_build =
        cx.update(|window, _| text_width(window.text_system(), "Build", 14.0, FontWeight::MEDIUM));
    click(cx, w_build / 2., 10.);
    assert_eq!(recorded.borrow().as_slice(), ["0:Build"]);
    cx.update(|_, cx| {
        assert!(
            !herogpui_components::extend::focus_visible(cx),
            "a mouse press must not arm the focus-visible ring"
        );
    });

    press(cx, "tab");
    cx.update(|_, cx| {
        assert!(
            herogpui_components::extend::focus_visible(cx),
            "keyboard focus must arm the ring the focused link draws"
        );
    });
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["0:Build", "0:Build"],
        "the Tab that armed the ring must have focused the crumb Enter then \
         navigates"
    );
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------
//
// v3's Accessibility section claims Tab-key navigation through the interactive
// elements, an `aria-hidden` ellipsis, and disabled states that genuinely
// block presses ("Press events handled across mouse, touch, and keyboard
// interactions via React Aria"). The port spells the v2 API (`page`, `total`,
// `on_change`) that v3 removed; `siblings` exists as a *field* with no
// builder, so it is frozen at 1 — another write-only prop on the parity list.
// The tests below pin the edge cells: disabled arrows and ellipses must answer
// nothing, the active page remains pressable, the derived page set must be
// exactly the v3 "Controlled"-example arithmetic (siblings = boundaries = 1),
// and Tab must walk prev, cells and next.

#[gpui::test]
fn pagination_text_owns_size_specific_line_boxes(cx: &mut TestAppContext) {
    for (size, leading) in [
        (herogpui_components::Size::Sm, 16.),
        (herogpui_components::Size::Md, 20.),
        (herogpui_components::Size::Lg, 24.),
    ] {
        let cx = open_host(cx, move || {
            gpui::div()
                .text_size(px(32.))
                .line_height(px(48.))
                .child(
                    gpui::div()
                        .debug_selector(|| "pagination-box".to_owned())
                        .child(
                            Pagination::new("leading-pagination", 2, 3)
                                .size(size)
                                .summary("Page\n2 of 3")
                                .disabled_keys([1])
                                .link(|page, _| {
                                    gpui::div()
                                        .debug_selector(move || format!("page-label-{page}"))
                                        .child(page.to_string())
                                        .into_any_element()
                                })
                                .previous_icon(
                                    gpui::div()
                                        .debug_selector(|| "previous-label".to_owned())
                                        .child("Previous"),
                                ),
                        ),
                )
                .into_any_element()
        });
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("pagination-box").unwrap().size.height,
            px(leading * 2.)
        );
        for selector in ["page-label-1", "page-label-2", "previous-label"] {
            assert_eq!(
                cx.debug_bounds(selector).unwrap().size.height,
                px(leading),
                "{selector}"
            );
        }
    }
}

/// At page 1 the Previous arrow is disabled (v3: `isDisabled` communicates
/// "disabled states properly" and React Aria's press never fires for them),
/// and at the last page the Next arrow is. Neither disabled arrow is a tab stop
/// or answers a pointer press; the first Tab instead reaches active page 1,
/// which remains a live link and reports itself.
#[gpui::test]
fn pagination_disabled_arrows_are_inert(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        // Two independents rows 100px apart so each can be hit without the
        // other: on page 1 the prev is disabled; on page 3 of 3 the next is.
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(100.))
            .child(
                Pagination::new("nvd-pg-d1", 1, 3)
                    .on_change(move |page, _, _| recorded.borrow_mut().push(page.to_string())),
            )
            .child({
                let recorded = for_view.clone();
                Pagination::new("nvd-pg-d2", 3, 3)
                    .on_change(move |page, _, _| recorded.borrow_mut().push(page.to_string()))
            })
            .into_any_element()
    });

    // Row 0 at y 0..32 (centre 16): the first Tab must NOT land on the
    // disabled prev (a disabled control is not a stop), and neither a click
    // nor Enter on it may report. Enter therefore activates page 1.
    press(cx, "tab");
    press(cx, "enter");
    click(cx, 17., 16.);
    // Row 1 at y 132..164 (centre 148): the disabled next must not report
    // the page after the last.
    click(cx, 163., 148.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1"],
        "the disabled arrows must answer nothing while the first enabled tab \
         stop, active page 1, reports itself — got {:?}",
        recorded.borrow()
    );
}

/// `Pagination::new` clamps `total` to at least 1, so a single-page
/// pagination still renders prev, the one active page link and next. The two
/// arrows are disabled at the bounds, while pressing the active link reports
/// page 1 exactly like any other enabled Pagination.Link.
#[gpui::test]
fn pagination_single_page_has_working_tab_order(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Pagination::new("nvd-pg-solo", 1, 1)
            .on_change(move |page, _, _| recorded.borrow_mut().push(page.to_string()))
            .into_any_element()
    });

    // One cell: prev spans x 0..34 (centre 17), the cell 38..70 (centre 54),
    // next starts at 38+36 = 74 (centre 91); the row centres on y 16.
    click(cx, 17., 16.);
    click(cx, 91., 16.);
    click(cx, 54., 16.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1"],
        "on a single page only the active page link may answer — got {:?}",
        recorded.borrow()
    );
}

/// Total 1 represents the no-movement state with a cell that IS the current
/// page. This one pins the shape that does pass: navigation reports are
/// recorded, but the hopeless directions stay inert (nothing here is
/// disabled-by-construction, so both arrows are enabled and report what they
/// navigate to).
#[gpui::test]
fn pagination_two_pages_edge_reports(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Pagination::new("nvd-pg-two", 1, 2)
            .on_change(move |page, _, _| recorded.borrow_mut().push(page.to_string()))
            .into_any_element()
    });

    // Two cells: prev 0..34 (centre 17), cell 1 at 38..70 (centre 54), cell 2
    // at 74..106 (centre 90), next starts at 38+72 = 110 (centre 127); y 16.
    click(cx, 127., 16.);
    click(cx, 90., 16.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["2", "2"],
        "on a two-page feed the next arrow reports page 2, and the page-2 \
         cell (never the current page here) reports page 2 as well"
    );
}

/// v3 marks the ellipsis `aria-hidden`, but its active page remains a live
/// React Aria Button: `aria-current` styles and identifies it without disabling
/// its forwarded `onPress`. Page 5 of 10 renders 1 … 4 5 6 … 10.
#[gpui::test]
fn pagination_ellipses_are_inert_and_active_cell_reports(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Pagination::new("nvd-pg-el", 5, 10)
            .on_change(move |page, _, _| recorded.borrow_mut().push(page.to_string()))
            .into_any_element()
    });

    // 10 > 2*1+5, so `visible_pages` yields 1, …, 4, 5, 6, …, 10 — seven
    // cells at centres 54+36k: 54, 90, 126, 162, 198, 234, 270 (y 16).
    // Page 5 — the current page — is the cell at k = 3 (centre 162).
    click(cx, 90., 16.); // first ellipsis
    click(cx, 162., 16.); // the active page 5
    click(cx, 234., 16.); // second ellipsis
    assert_eq!(
        recorded.borrow().as_slice(),
        ["5"],
        "the ellipses must stay inert while the active page reports its page"
    );
    click(cx, 126., 16.);
    click(cx, 270., 16.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["5", "4", "10"],
        "the page cells around the ellipses must report their own pages"
    );
}

/// Which numbers appear is the port's `visible_pages` arithmetic — the v3
/// "Controlled"-example scheme (first, last, current, one sibling each side,
/// no `dotsJump`) — and it is fixed: `siblings`/`boundaries` are v2 props
/// this port's `siblings` field cannot even set (it has no builder). The
/// seats below are derived from that fixed scheme, so the test pins both the
/// arithmetic and its immobility: page 6 of 12 renders 1 … 5 6 7 … 12.
#[gpui::test]
fn pagination_middle_page_shows_the_derived_set(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Pagination::new("nvd-pg-set", 6, 12)
            .on_change(move |page, _, _| recorded.borrow_mut().push(page.to_string()))
            .into_any_element()
    });

    // 12 > 7, page 6: left = 5, right = 7, so the cells are 1, …, 5, 6, 7,
    // …, 12 — seven cells at centres 54+36k (54, 90, 126, 162, 198, 234,
    // 270), y 16. The next button starts at 38+36*7 = 290 (centre 307).
    click(cx, 126., 16.); // cell 2 -> "5"
    click(cx, 162., 16.); // cell 3 -> "6" (the current page remains live)
    click(cx, 198., 16.); // cell 4 -> "7"
    click(cx, 234., 16.); // second ellipsis -> nothing
    click(cx, 270., 16.); // cell 6 -> "12"
    click(cx, 307., 16.); // next -> "7"
    assert_eq!(
        recorded.borrow().as_slice(),
        ["5", "6", "7", "12", "7"],
        "the middle-page window must render exactly 1 … 5 6 7 … 12 with the \
         ellipsis inert and the next arrow reporting the following page"
    );
}

/// v3's Accessibility section: "Keyboard navigation via Tab key through all
/// interactive elements." The port gives every cell and both arrows their own
/// tab stop, so the walk is prev -> every cell -> next; Enter activates
/// whatever holds the focus. The active cell remains a live Button;
/// `aria-current` identifies it without disabling it.
#[gpui::test]
fn pagination_keyboard_reaches_arrows_and_cells(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Pagination::new("nvd-pg-keys", 2, 3)
            .on_change(move |page, _, _| recorded.borrow_mut().push(page.to_string()))
            .into_any_element()
    });

    // Tab order: prev, cell 1, cell 2 (active), cell 3, next. Each Enter
    // activates the focused element on key-up.
    press(cx, "tab");
    press(cx, "enter"); // prev -> reports page 1
    press(cx, "tab");
    press(cx, "enter"); // cell 1 -> reports page 1
    press(cx, "tab");
    press(cx, "enter"); // cell 2 (active) -> reports page 2
    press(cx, "tab");
    press(cx, "enter"); // cell 3 -> reports page 3
    press(cx, "tab");
    press(cx, "enter"); // next -> reports page 3 (2 + 1)
    assert_eq!(
        recorded.borrow().as_slice(),
        ["1", "1", "2", "3", "3"],
        "Tab must walk prev, every page cell and next, activating each on Enter"
    );
}

// ---------------------------------------------------------------------------
// Toolbar
// ---------------------------------------------------------------------------
//
// "Inherits from React Aria Toolbar". Pinned `useToolbar` (react-aria 3.51.0)
// moves the focus with the orientation's axis only — vertical:
// ArrowDown/ArrowUp, with Left/Right doing nothing — through a FocusManager
// whose walk has `wrap` unset, so an arrow at either end is consumed without
// moving. Tab leaves the *entire* toolbar in one press (Shift+Tab backwards).
// The horizontal scoping (end stops, one-press Tab exit, disabled children
// skipped, re-entry restore) is pinned in `tests/calendars_and_more.rs`; here
// is the vertical axis, the perpendicular keys the port answers anyway, a
// toolbar whose only child is disabled, the vertical sheet alignment, and the
// nested-toolbar group contract.

#[gpui::test]
fn toolbar_vertical_arrows_stop_at_ends_and_tab_leaves(cx: &mut TestAppContext) {
    let pressed = events();
    let recorded = pressed.clone();
    let outside_pressed = events();
    let outside = outside_pressed.clone();
    let cx = open_host(cx, move || {
        let bold = pressed.clone();
        let italic = pressed.clone();
        let underline = pressed.clone();
        let outside = outside_pressed.clone();
        // A vertical toolbar (the v3 "Vertical" example is exactly Buttons
        // stacked) with a plain button after it as the Tab-out probe.
        gpui::div()
            .flex()
            .flex_col()
            .gap(px(100.))
            .child(
                Toolbar::new()
                    .orientation(Orientation::Vertical)
                    .child(
                        Button::new("nvd-vtb-bold")
                            .label("Bold")
                            .on_press(move |_, _, _| bold.borrow_mut().push("bold".into())),
                    )
                    .child(
                        Button::new("nvd-vtb-italic")
                            .label("Italic")
                            .on_press(move |_, _, _| italic.borrow_mut().push("italic".into())),
                    )
                    .child(
                        Button::new("nvd-vtb-underline")
                            .label("Underline")
                            .on_press(move |_, _, _| {
                                underline.borrow_mut().push("underline".into());
                            }),
                    ),
            )
            .child(
                Button::new("nvd-vtb-outside")
                    .label("Outside")
                    .on_press(move |_, _, _| outside.borrow_mut().push("outside".into())),
            )
            .into_any_element()
    });

    // Tab enters on the first control; Down walks bold -> italic -> underline,
    // the third Down stays on underline (the FocusManager walk has no wrap, so
    // the end is a consumed stop — the old window-wide wrap landed Enter on
    // bold again), and Up from the first stays on bold. Enter reports which
    // control holds the focus.
    press(cx, "tab");
    press(cx, "down down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["underline"],
        "Down from the last control must stop there and be consumed, not \
         wrap to the first, staying inside the vertical toolbar"
    );
    press(cx, "up up up");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["underline", "bold"],
        "Up from the first control must stop there and be consumed"
    );
    // Tab is the way out, in one press, exactly as in the horizontal case —
    // this exits from the *first* child and the sibling after the toolbar
    // must answer.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        outside.borrow().as_slice(),
        ["outside"],
        "Tab must leave the whole vertical toolbar for the next control in \
         the window in one press"
    );
}

/// React Aria's `useToolbar` handles only the orientation's axis: in vertical
/// mode ArrowRight/ArrowLeft "return early" (the source is explicit that
/// nothing else is handled). The port's handler maps the same axis set, so
/// Right in a vertical toolbar moves nothing *and* stays unconsumed — it
/// bubbles on to whatever else may want it.
#[gpui::test]
fn toolbar_vertical_ignores_perpendicular_arrows(cx: &mut TestAppContext) {
    let pressed = events();
    let recorded = pressed.clone();
    let cx = open_host(cx, move || {
        let bold = pressed.clone();
        let italic = pressed.clone();
        Toolbar::new()
            .orientation(Orientation::Vertical)
            .child(
                Button::new("nvd-vpa-b")
                    .label("Bold")
                    .on_press(move |_, _, _| bold.borrow_mut().push("bold".into())),
            )
            .child(
                Button::new("nvd-vpa-i")
                    .label("Italic")
                    .on_press(move |_, _, _| italic.borrow_mut().push("italic".into())),
            )
            .into_any_element()
    });

    // Right is not a vertical-toolbar key: the focus must stay on Bold and
    // Enter must fire Bold.
    press(cx, "tab");
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["bold"],
        "Right must not move the focus in a vertical toolbar"
    );
}

/// A toolbar whose only child is disabled contributes nothing to the Tab
/// order: the disabled child is not a stop and has no press, and the
/// toolbar's own scope handle is a non-stop, so Tab walks past the whole
/// toolbar and every key the group would have answered does nothing.
#[gpui::test]
fn toolbar_only_child_disabled_answers_nothing(cx: &mut TestAppContext) {
    let pressed = events();
    let recorded = pressed.clone();
    let cx = open_host(cx, move || {
        let pressed = pressed.clone();
        Toolbar::new()
            .gap(px(8.))
            .child(
                Button::new("nvd-tbl-lone")
                    .label("Lone")
                    .is_disabled(true)
                    .on_press(move |_, _, _| pressed.borrow_mut().push("lone".into())),
            )
            .into_any_element()
    });

    // The disabled button (h-9, ~63px wide for this label) sits at the
    // origin: a click at (40, 18) must record nothing — `on_click` is only
    // attached when the button is interactive.
    click(cx, 40., 18.);
    // And no key reaches it: Tab walks past (the scope handle is not a
    // stop), the arrows find no stop to move to, and Enter/Space activate
    // nothing. All four directions are included in case the step-and-check
    // wrap ever lands outside the empty toolbar.
    press(cx, "tab");
    press(cx, "right");
    press(cx, "down");
    press(cx, "left");
    press(cx, "up");
    press(cx, "enter");
    press(cx, "space");
    assert!(
        recorded.borrow().is_empty(),
        "a toolbar whose only child is disabled must answer no key and no \
         click, and must not strand the focus"
    );
}

/// Pinned `useToolbar` detects a nested toolbar —
/// `ref.current.parentElement.closest('[role="toolbar"]')` — and a nested one
/// renders `role="group"` with `onKeyDownCapture`, `onFocusCapture` and
/// `onBlurCapture` all undefined: **no keyboard or focus management of its
/// own**, so the enclosing toolbar's manager walks straight across its
/// children. The port asks the same question of the last rendered frame's
/// dispatch tree (`FocusHandle::contains` over a weak per-window registry of
/// toolbar scopes), which gpui offers as the runtime ancestor query the DOM
/// spelling assumes.
///
/// The regression drives all three consequences: the outer horizontal arrows
/// cross the vertical inner toolbar's boundary in both directions; Down
/// *inside* the inner toolbar binds to nothing (an inner manager would move
/// i1 -> i2 and consume the key); one Tab leaves the whole outer toolbar from
/// inside the inner one; and the sibling toolbar after it — a root of its
/// own, with its own keyed state — still navigates independently.
#[gpui::test]
fn toolbar_nested_defers_to_the_outer_manager(cx: &mut TestAppContext) {
    let pressed = events();
    let recorded = pressed.clone();
    let cx =
        open_host(cx, move || {
            let o1_pressed = pressed.clone();
            let i1_pressed = pressed.clone();
            let i2_pressed = pressed.clone();
            let o2_pressed = pressed.clone();
            let after_pressed = pressed.clone();
            let s1_pressed = pressed.clone();
            let s2_pressed = pressed.clone();
            gpui::div()
                .flex()
                .flex_col()
                .gap(px(100.))
                .child(
                    Toolbar::new()
                        .id("tb-outer")
                        .gap(px(8.))
                        .child(
                            Button::new("tb-nt-o1")
                                .label("Cut")
                                .on_press(move |_, _, _| o1_pressed.borrow_mut().push("o1".into())),
                        )
                        .child(
                            Toolbar::new()
                                .id("tb-inner")
                                .orientation(Orientation::Vertical)
                                .child(Button::new("tb-nt-i1").label("Bold").on_press(
                                    move |_, _, _| i1_pressed.borrow_mut().push("i1".into()),
                                ))
                                .child(Button::new("tb-nt-i2").label("Italic").on_press(
                                    move |_, _, _| i2_pressed.borrow_mut().push("i2".into()),
                                )),
                        )
                        .child(
                            Button::new("tb-nt-o2")
                                .label("Paste")
                                .on_press(move |_, _, _| o2_pressed.borrow_mut().push("o2".into())),
                        ),
                )
                .child(
                    Button::new("tb-nt-after")
                        .label("After")
                        .on_press(move |_, _, _| after_pressed.borrow_mut().push("after".into())),
                )
                .child(
                    Toolbar::new()
                        .id("tb-sibling")
                        .gap(px(8.))
                        .child(
                            Button::new("tb-nt-s1")
                                .label("One")
                                .on_press(move |_, _, _| s1_pressed.borrow_mut().push("s1".into())),
                        )
                        .child(
                            Button::new("tb-nt-s2")
                                .label("Two")
                                .on_press(move |_, _, _| s2_pressed.borrow_mut().push("s2".into())),
                        ),
                )
                .into_any_element()
        });

    // Tab enters the outer toolbar on its first child.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["o1"],
        "Tab must land on the outer toolbar's first child"
    );

    // One Right crosses *into* the vertical inner toolbar: the outer manager
    // walks across the group boundary exactly as pinned's FocusManager does.
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["o1", "i1"],
        "the outer toolbar's arrows must walk across the nested toolbar's \
         boundary"
    );

    // Down is the inner toolbar's axis: an inner manager would move to i2
    // and consume the key. The inner is a group, so nothing happens at all —
    // the key is not even consumed, and i1 keeps the focus.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["o1", "i1", "i1"],
        "a nested toolbar must bind no keyboard management of its own"
    );

    // Right carries on across the inner toolbar's children and out the far
    // side, to the outer toolbar's own last child.
    press(cx, "right");
    press(cx, "enter");
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["o1", "i1", "i1", "i2", "o2"],
        "the outer manager must cross the group in both directions"
    );

    // Tab from inside the inner toolbar leaves the *entire* outer toolbar in
    // one press — the inner group's boundary is no extra stop.
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["o1", "i1", "i1", "i2", "o2", "after"],
        "one Tab must leave the whole outer toolbar from inside the nested \
         one"
    );

    // The sibling toolbar is a root of its own: it answers its own axis, and
    // its keyed focus state is independent of the nested composition above.
    press(cx, "tab");
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["o1", "i1", "i1", "i2", "o2", "after", "s2"],
        "a sibling toolbar must keep its own independent navigation"
    );
}

/// `.toolbar--vertical` is `grid-flow-row items-start justify-start`: a column
/// whose controls hug the start edge — not the base `.toolbar` rule's
/// centered cross axis, which stays with the horizontal toolbar. The
/// toolbar's children are opaque elements, so each button sits in a probe div
/// whose laid-out bounds answer the alignment: a centered column would indent
/// the narrow control by half the difference.
#[gpui::test]
fn toolbar_vertical_children_hug_the_start_edge(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        Toolbar::new()
            .id("tb-v-align")
            .orientation(Orientation::Vertical)
            .child(
                gpui::div()
                    .debug_selector(|| "vtb-wide".into())
                    .child(Button::new("tb-va-wide").label("A very long label")),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "vtb-narrow".into())
                    .child(Button::new("tb-va-narrow").label("B")),
            )
            .into_any_element()
    });
    flush_frame(cx);
    flush_frame(cx);

    let wide = cx
        .debug_bounds("vtb-wide")
        .expect("the wide probe must be laid out");
    let narrow = cx
        .debug_bounds("vtb-narrow")
        .expect("the narrow probe must be laid out");

    assert_eq!(
        wide.origin.x, narrow.origin.x,
        "vertical toolbar children must share the start edge (items-start)"
    );
    assert!(
        narrow.origin.y > wide.origin.y,
        "vertical toolbar children must stack down the column (grid-flow-row)"
    );
}
