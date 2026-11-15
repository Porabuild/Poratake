//! Behaviour tests for what opens a ComboBox's suggestion list.
//!
//! The pickers suite drives ComboBox from the chevron; these tests cover the
//! `menuTrigger` paths it never exercised. The regressions preserved here are
//! the v3 default Focus trigger, explicit Input and Manual behavior, dismissal
//! and reopening, read-only inertness, and keyboard-open reports. Everything
//! is asserted on recorded callbacks and behavioral probes -- never appearance.
//!
//! Geometry is borrowed from tests/pickers.rs: the trigger field is a 36px
//! row at the window origin (centre (60, 18)), the chevron sits at the right
//! end of the 320px-wide field (x = 298), and the panel's `p(4)` puts row *i*
//! at y 64+36i with ≤4px of entry-zoom padding, which that y covers in every
//! phase of the animation.
//!
//! Reduce motion is not set. The ComboBox retains its panel for the pinned
//! 100ms exit so close motion can play; the exit frame is visual-only and does
//! not occlude a later pointer target.

use crate::harness;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{prelude::*, Focusable, TestAppContext};
use herogpui_components::{Button, ComboBox, Input, InputState, MenuTrigger, PickerItem};

use harness::{click, events, open_host, press};

/// Items whose labels are unique, so the key can be the label itself.
fn keyed(labels: &[&str]) -> Vec<PickerItem> {
    labels
        .iter()
        .map(|l| PickerItem::new(l.to_string(), l.to_string()))
        .collect()
}

fn selector(name: String) -> &'static str {
    let leaked: &'static mut str = Box::leak(name.into_boxed_str());
    &*leaked
}

/// An `InputState` entity, created before the host opens so the test can keep
/// its own handle to it.
fn combo_state(cx: &mut TestAppContext) -> gpui::Entity<InputState> {
    cx.new(|cx| InputState::new(cx))
}

#[gpui::test]
fn combo_box_typing_opens_the_list(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, keyed(&["Typst", "Rust", "Go"]))
            .menu_trigger(MenuTrigger::Input)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    // Clicking into the field focuses it. With the input trigger the focus is
    // not the gesture -- the first non-empty edit is.
    click(cx, 60., 18.);
    assert!(
        opened.borrow().is_empty(),
        "focus alone must not open the list under `MenuTrigger::Input`"
    );

    // The failing reproduction: typing "ty" must open the list, because the
    // suggestion that matches the text lives inside it.
    cx.simulate_input("ty");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the first non-empty edit must open the list"
    );

    // Row 0 is now drawn at y = 64 and records the pick.
    click(cx, 60., 64.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Typst"],
        "clicking the matching suggestion must select it"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "picking must close the list"
    );
}

#[gpui::test]
fn combo_box_default_trigger_opens_on_focus(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, keyed(&["Typst", "Rust", "Go"]))
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    // v3 defaults menuTrigger to Focus: no keystroke, chevron or builder.
    // Clicking into the field opens the list on that very frame.
    click(cx, 60., 18.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the default trigger must open when the field takes focus"
    );

    // Row 0 is already drawn without any typing, and records the pick.
    click(cx, 60., 64.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Typst"],
        "clicking the first suggestion must select it"
    );
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "picking must close the list"
    );
}

#[gpui::test]
fn combo_box_retains_panel_for_exit_motion_then_releases_it(cx: &mut TestAppContext) {
    let state = combo_state(cx);
    let entity_id = state.entity_id().as_u64();
    let panel_selector = selector(format!("combobox-panel-{entity_id}"));
    let state_for_view = state;
    let cx = open_host(cx, move || {
        ComboBox::new(state_for_view.clone(), keyed(&["Typst", "Rust"]))
            .default_open(true)
            .into_any_element()
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(panel_selector).is_some(),
        "an opened ComboBox must render its panel"
    );

    click(cx, 298., 18.);
    cx.update(|window, _| window.refresh());
    assert!(
        cx.debug_bounds(panel_selector).is_some(),
        "the panel must remain mounted during the 100ms exit transition"
    );

    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(panel_selector).is_none(),
        "the retained panel must leave the tree after its exit lifetime"
    );
}

#[gpui::test]
fn combo_box_focus_open_shows_all_items_before_the_next_edit(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = combo_state(cx);
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Typst", "Rust", "Go"]))
            .default_input_value("ru")
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    click(cx, 60., 18.);
    click(cx, 60., 64.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Typst"],
        "a Focus open must show the original collection, not the filtered query"
    );
}

#[gpui::test]
fn combo_box_chevron_open_shows_all_items_for_an_input_trigger(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let state = combo_state(cx);
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let changes = changes.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Typst", "Rust", "Go"]))
            .default_input_value("ru")
            .menu_trigger(MenuTrigger::Input)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .into_any_element()
    });

    click(cx, 298., 18.);
    click(cx, 60., 64.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Typst"],
        "a chevron press is a manual open and must show the original collection"
    );
}

#[gpui::test]
fn combo_box_manual_trigger_ignores_typing(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, keyed(&["Typst", "Rust", "Go"]))
            .menu_trigger(MenuTrigger::Manual)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    cx.simulate_input("ty");
    assert!(
        opened.borrow().is_empty(),
        "typing must not open a `MenuTrigger::Manual` list"
    );

    // The chevron is still a trigger: x = 320 - 12px field padding - half the
    // 20px button box, at the field's vertical centre.
    click(cx, 298., 18.);
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the chevron must still open a manual list"
    );

    // The manual press opens the full collection. A subsequent edit switches
    // back to filtered results while keeping the already-open list visible.
    press(cx, "ctrl-a");
    cx.simulate_input("go");
    click(cx, 60., 64.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Go"],
        "typing in an open manual list must replace show-all with filtered rows"
    );
    assert_eq!(opened.borrow().as_slice(), ["open:true", "open:false"]);
}

#[gpui::test]
fn combo_box_focus_trigger_reopens_after_a_later_edit(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, keyed(&["Typst", "Rust", "Go"]))
            .menu_trigger(MenuTrigger::Focus)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    assert_eq!(opened.borrow().as_slice(), ["open:true"]);

    // Escape closes the list and leaves the focus on the field. The one-shot
    // under test is what must keep the panel closed afterwards: were the
    // focus-open check answering the still-held focus every frame, the next
    // render would reopen it.
    press(cx, "escape");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "escape must close the list"
    );

    // Force another frame while focus remains in the field. The focus-open
    // one-shot must not answer the still-held focus by reopening the panel.
    cx.update(|window, _| window.refresh());
    assert!(recorded.borrow().is_empty());
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "no reopen after escape"
    );

    cx.simulate_input("t");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false", "open:true"],
        "a later edit must reopen a dismissed Focus-triggered list"
    );
}

#[gpui::test]
fn combo_box_escape_does_not_steal_a_later_pointer_focus(cx: &mut TestAppContext) {
    let combo = combo_state(cx);
    let other = combo_state(cx);
    let combo_for_view = combo;
    let other_for_view = other.clone();
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                ComboBox::new(combo_for_view.clone(), keyed(&["Typst", "Rust", "Go"]))
                    .into_any_element(),
            )
            .child(Input::new(other_for_view.clone()).into_any_element())
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "escape");
    click(cx, 60., 54.);
    assert!(
        cx.update(|window, cx| other.read(cx).focus_handle(cx).is_focused(window)),
        "Escape dismissal must not reclaim focus from a later pointer target"
    );
}

#[gpui::test]
fn combo_box_manual_no_match_edit_closes_logical_open_state(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Typst", "Rust", "Go"]))
            .menu_trigger(MenuTrigger::Manual)
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 298., 18.);
    cx.simulate_input("zz");
    press(cx, "escape");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "an unmatched edit must close Manual state once, not leave a hidden overlay"
    );
}

#[gpui::test]
fn combo_box_arrow_open_shows_all_items_for_an_input_trigger(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Typst", "Rust", "Go"]))
            .default_input_value("zz")
            .menu_trigger(MenuTrigger::Input)
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "down");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "ArrowDown is a manual open and must use the original collection"
    );
}

#[gpui::test]
fn combo_box_read_only_refuses_focus_and_chevron_open(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Typst", "Rust", "Go"]))
            .is_read_only(true)
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    click(cx, 298., 18.);
    assert!(
        opened.borrow().is_empty(),
        "a read-only ComboBox must not open from focus or its chevron"
    );
}

#[gpui::test]
fn combo_box_arrow_open_reports_to_a_controlled_owner(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let opens = opens.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Typst", "Rust", "Go"]))
            .is_open(false)
            .menu_trigger(MenuTrigger::Manual)
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "down");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "ArrowDown must report a manual open even when the owner has not accepted it"
    );
}

/// PageUp/PageDown belong to the open suggestion list: pinned React Aria
/// 3.51.0 binds them through `useSelectableCollection`, which a closed
/// ComboBox never runs. The proof is not vacuous: the field is focused but
/// closed under the `MenuTrigger::Input` trigger, so the page keys land on
/// the very handler that answers them on an open list — and a following Down
/// still opens. A page key on the closed field must not open the list and
/// must not commit anything.
#[gpui::test]
fn combo_box_page_keys_ignore_a_closed_field(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, keyed(&["Typst", "Rust", "Go"]))
            .menu_trigger(MenuTrigger::Input)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    // Clicking into the field focuses it without opening: the Input trigger
    // opens on the first edit, so the presses land on a live handler.
    click(cx, 60., 18.);
    assert!(
        opened.borrow().is_empty(),
        "the probe must begin from a focused, closed field"
    );

    press(cx, "pagedown");
    press(cx, "pageup");
    assert!(
        opened.borrow().is_empty(),
        "a page key on the closed field must not open the list"
    );

    // The presses were delivered: the same focused handler still opens on
    // Down, which also seats the cursor on the first row.
    press(cx, "down");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the page keys must have reached a live handler, not a dead one"
    );

    // Escape closes the list but leaves the cursor seated on "Typst" -- so
    // the page keys now reach the handler *with* a cursor, exactly the state
    // an unconditional end mapping would move (and the Move::To path would
    // even reopen the list with).
    press(cx, "escape");
    press(cx, "pagedown");
    press(cx, "pageup");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "a page key on the closed field must not reopen the list: the only \
         reports are the open and the Escape close"
    );
    assert!(
        recorded.borrow().is_empty(),
        "a page key on the closed field must not commit anything"
    );

    // Down then reopens, walks the retained cursor to the second row, and
    // Enter commits it -- the page keys left the cursor where they found it.
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Rust"],
        "the page keys must have left the closed field answering as before"
    );
}

/// Pinned HeroUI v3.2.4 scrolls the *popover*, with the ListBox element
/// itself `overflow-clip`, so pinned React Aria 3.51.0 never sees a
/// scrollable list behind a ComboBox: with a cursor, page keys take the
/// enabled ends whatever the list's length. Those handlers require a focused
/// key, though — a chevron-opened, selection-less ComboBox has a null cursor,
/// so its page keys are inert; a Down establishes the cursor, and paging from
/// there reaches the first and last enabled rows, skipping the disabled rows
/// at both ends. Paging only moves the highlight until Enter commits it.
#[gpui::test]
fn combo_box_page_keys_reach_enabled_ends_after_a_cursor_exists(cx: &mut TestAppContext) {
    let changes = events();
    let recorded = changes.clone();
    let options: Vec<PickerItem> = (0..20)
        .map(|i| {
            let label = format!("Option {i:02}");
            PickerItem::new(label.clone(), label)
        })
        .collect();
    let state = combo_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let changes = changes.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, options.clone())
            .max_items(20)
            .menu_trigger(MenuTrigger::Input)
            .on_change(move |item, _, _| changes.borrow_mut().push(item.to_string()))
            .disabled_keys(["Option 00".into(), "Option 19".into()])
            .into_any_element()
    });

    // Chevron-open with no selection: the cursor is null, so both page keys
    // must be inert. Down from a null cursor enters the first enabled row
    // (Option 01); had either page key created a cursor, Down would hold on
    // Option 18 or step to Option 02, and the commit would betray the
    // unconditional cursor creation.
    click(cx, 298., 18.);
    press(cx, "pagedown");
    press(cx, "pageup");
    assert!(
        recorded.borrow().is_empty(),
        "page keys on a chevron-opened, cursor-less list must commit nothing"
    );
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Option 01"],
        "page keys on a cursor-less list must be inert: Down must still \
         enter the first enabled row"
    );

    // Enter closed the list; the chevron reopens it, two Downs establish the
    // cursor on Option 02, and PageDown must take the last enabled row
    // (Option 18), never the disabled Option 19.
    click(cx, 298., 18.);
    press(cx, "down down");
    press(cx, "pagedown");
    assert!(
        recorded.borrow().as_slice() == ["Option 01"],
        "PageDown must only move the highlight, never commit by itself"
    );
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Option 01", "Option 18"],
        "PageDown with a cursor must reach the last enabled row"
    );

    // Reopen once more; the cursor starts over on the first enabled row and
    // PageUp must walk back to it — which is also the first enabled row, so
    // take two Downs first — never the disabled Option 00.
    click(cx, 298., 18.);
    press(cx, "down down");
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["Option 01", "Option 18", "Option 01"],
        "PageUp with a cursor must reach the first enabled row"
    );
}

/// The outside-press contract Select already has: an open ComboBox's
/// dismissal must not swallow the press, so clicking another ComboBox's
/// chevron opens it while the first list closes. Before the fix the
/// topmost panel's `on_mouse_down_out` consumed every outside press, so any
/// open list left the rest of the page unclickable.
#[gpui::test]
fn combo_box_outside_press_reaches_the_next_trigger(cx: &mut TestAppContext) {
    let eater_state = combo_state(cx);
    let target_state = combo_state(cx);
    let eater_id = eater_state.entity_id().as_u64();
    let target_id = target_state.entity_id().as_u64();
    let eater_for_view = eater_state;
    let target_for_view = target_state;

    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .gap(gpui::px(8.))
            .child(
                ComboBox::new(target_for_view.clone(), keyed(&["Gamma", "Delta"]))
                    .into_any_element(),
            )
            .child(
                ComboBox::new(eater_for_view.clone(), keyed(&["Alpha", "Beta"]))
                    .default_open(true)
                    .into_any_element(),
            )
            .into_any_element()
    });
    harness::settle(cx);

    assert!(
        cx.debug_bounds(selector(format!("combobox-panel-{eater_id}")))
            .is_some(),
        "the default_open combo's panel must be laid out"
    );

    let field = cx
        .debug_bounds(selector(format!("combobox-field-{target_id}")))
        .expect("the second combo's field must be laid out");
    // The chevron sits at the field's right end; it owns the open gesture
    // under every `MenuTrigger`.
    let chevron = gpui::point(
        field.origin.x + field.size.width - gpui::px(22.),
        field.center().y,
    );
    cx.simulate_click(chevron, gpui::Modifiers::none());
    harness::settle(cx);
    // The dismissed panel keeps its exit frame for ~100ms; let it finish.
    harness::wait_ms(cx, 200);

    assert!(
        cx.debug_bounds(selector(format!("combobox-panel-{target_id}")))
            .is_some(),
        "the press must reach the field and open its list"
    );
    assert!(
        cx.debug_bounds(selector(format!("combobox-panel-{eater_id}")))
            .is_none(),
        "the same press dismisses the previously open list"
    );
}

/// The gallery's controlled ComboBox: the parent owns `open` and feeds it
/// back through `is_open`/`on_open_change`. A pointer click on the chevron --
/// the `menuTrigger` under every `MenuTrigger` -- must run the whole path:
/// click, open callback, re-render, panel layout.
#[gpui::test]
fn combo_box_controlled_chevron_click_opens_the_panel(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let open_flag = Rc::new(RefCell::new(false));
    let state = combo_state(cx);
    let entity_id = state.entity_id().as_u64();
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let opens = opens.clone();
        let is_open = *open_flag.borrow();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta", "Gamma"]))
            .is_open(is_open)
            .on_open_change({
                let open_flag = open_flag.clone();
                move |v, window, _| {
                    *open_flag.borrow_mut() = *v;
                    opens.borrow_mut().push(format!("open:{v}"));
                    window.refresh();
                }
            })
            .into_any_element()
    });
    harness::settle(cx);

    let field = cx
        .debug_bounds(selector(format!("combobox-field-{entity_id}")))
        .expect("the field must be laid out");
    // The chevron sits at the field's right end; it owns the open gesture.
    let chevron = gpui::point(
        field.origin.x + field.size.width - gpui::px(22.),
        field.center().y,
    );
    cx.simulate_click(chevron, gpui::Modifiers::none());
    harness::settle(cx);

    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the chevron click must report the open through on_open_change"
    );
    assert!(
        cx.debug_bounds(selector(format!("combobox-panel-{entity_id}")))
            .is_some(),
        "the controlled panel must lay out after the click"
    );
}

/// ComboBox's popover is `isNonModal` in React Aria — no underlay — so an
/// outside press closes the list *and* reaches the control underneath: the
/// sibling button fires while the combo's panel dismisses. Select and the
/// dialog popovers keep the consuming underlay contract instead.
#[gpui::test]
fn combo_box_outside_press_reaches_the_sibling(cx: &mut TestAppContext) {
    let clicks = events();
    let clicked = clicks.clone();
    let state = combo_state(cx);
    let entity_id = state.entity_id().as_u64();
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let clicks = clicks.clone();
        gpui::div()
            .flex()
            .flex_col()
            .gap(gpui::px(8.))
            .child(
                gpui::div()
                    .id("cb-outside-btn")
                    .w(gpui::px(140.))
                    .h(gpui::px(36.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .debug_selector(|| "cb-outside-btn".to_owned())
                    .child(
                        Button::new("cb-outside-inner")
                            .label("Outside")
                            .on_press(move |_, _, _| clicks.borrow_mut().push("click".into())),
                    )
                    .into_any_element(),
            )
            .child(
                ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta"]))
                    .default_open(true)
                    .into_any_element(),
            )
            .into_any_element()
    });
    harness::settle(cx);

    assert!(
        cx.debug_bounds(selector(format!("combobox-panel-{entity_id}")))
            .is_some(),
        "the combo's panel must start open"
    );

    let button = cx
        .debug_bounds("cb-outside-btn")
        .expect("the sibling button must be laid out");
    cx.simulate_click(button.center(), gpui::Modifiers::none());
    harness::settle(cx);
    // The dismissed panel keeps its exit frame for ~100ms; let it finish.
    harness::wait_ms(cx, 200);

    assert_eq!(
        clicked.borrow().as_slice(),
        ["click"],
        "the non-modal press must reach the sibling button"
    );
    assert!(
        cx.debug_bounds(selector(format!("combobox-panel-{entity_id}")))
            .is_none(),
        "the same press dismisses the open combo"
    );
}

/// The chevron owns the close gesture: with the list open, a press on it
/// must end closed — the panel's outside-press dismissal declines the press
/// (`inside_pressed`) and the chevron's own click performs the close, once.
#[gpui::test]
fn combo_box_chevron_press_while_open_closes_once(cx: &mut TestAppContext) {
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let entity_id = state.entity_id().as_u64();
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let opens = opens.clone();
        ComboBox::new(state_for_view.clone(), keyed(&["Alpha", "Beta", "Gamma"]))
            .on_open_change(move |v, _, _| opens.borrow_mut().push(format!("open:{v}")))
            .into_any_element()
    });
    harness::settle(cx);

    let field = cx
        .debug_bounds(selector(format!("combobox-field-{entity_id}")))
        .expect("the field must be laid out");
    // The chevron sits at the field's right end; it owns the open gesture.
    let chevron = gpui::point(
        field.origin.x + field.size.width - gpui::px(22.),
        field.center().y,
    );
    cx.simulate_click(chevron, gpui::Modifiers::none());
    harness::settle(cx);
    assert!(
        cx.debug_bounds(selector(format!("combobox-panel-{entity_id}")))
            .is_some(),
        "the first chevron click must open the list"
    );

    cx.simulate_click(chevron, gpui::Modifiers::none());
    harness::settle(cx);
    // The dismissed panel keeps its exit frame for ~100ms; let it finish.
    harness::wait_ms(cx, 200);

    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true", "open:false"],
        "the chevron press must close once -- no dismiss-then-reopen"
    );
    assert!(
        cx.debug_bounds(selector(format!("combobox-panel-{entity_id}")))
            .is_none(),
        "the panel must be gone after the closing press"
    );
}

/// ArrowDown opens the suggestion list and the shared `seat_on_open` lets the
/// selected key win over the arrow's own first-row move: reopening onto a
/// selected "Rust" seats the cursor there, so a following Down walks to "Go"
/// and Enter commits it. Without selected-wins the cursor would sit on the
/// first row and the same keys would commit "Rust".
#[gpui::test]
fn arrow_down_open_seats_the_selected_row(cx: &mut TestAppContext) {
    let picks = events();
    let seen = picks.clone();
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, keyed(&["Typst", "Rust", "Go"]))
            .menu_trigger(MenuTrigger::Input)
            .selected_key("Rust")
            .on_change(move |item, _, _| picks.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "down");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "ArrowDown must open the closed field"
    );
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        seen.borrow().as_slice(),
        ["Go"],
        "the selected row must win the open seat: Down from Rust lands on Go"
    );
}

/// ArrowUp opens onto the last enabled stop when nothing is selected: the
/// arrow's own move already seats the last row and `seat_on_open`'s `Keep`
/// leaves it there, so Enter commits "Go" — not the first row.
#[gpui::test]
fn arrow_up_open_with_no_selection_seats_the_last_row(cx: &mut TestAppContext) {
    let picks = events();
    let seen = picks.clone();
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, keyed(&["Typst", "Rust", "Go"]))
            .menu_trigger(MenuTrigger::Input)
            .on_change(move |item, _, _| picks.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    press(cx, "up");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "ArrowUp must open the closed field"
    );
    press(cx, "enter");
    assert_eq!(
        seen.borrow().as_slice(),
        ["Go"],
        "ArrowUp with no selection must seat the last enabled row"
    );
}

/// Removing the highlighted suggestion repairs the cursor to the nearest
/// enabled neighbour while the input text is unchanged: the highlighted row
/// is dropped from `items`, and the next render walks forward onto the row
/// that slid into its place. Enter then commits the repaired row.
#[gpui::test]
fn removing_the_highlighted_row_repairs_to_the_next(cx: &mut TestAppContext) {
    let items = Rc::new(RefCell::new(vec![
        ("Typst", "Typst"),
        ("Rust", "Rust"),
        ("Go", "Go"),
        ("Python", "Python"),
    ]));
    let picks = events();
    let seen = picks.clone();
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let state_for_view = state;
    let items_for_view = items.clone();

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        let items = items_for_view.borrow();
        let its: Vec<PickerItem> = items
            .iter()
            .map(|(key, label)| PickerItem::new(*key, *label))
            .collect();
        ComboBox::new(state, its)
            .menu_trigger(MenuTrigger::Input)
            .on_change(move |item, _, _| picks.borrow_mut().push(item.to_string()))
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    // Focus the field, open with Down onto the first row, then walk to "Go"
    // (index 2 of 4).
    click(cx, 60., 18.);
    press(cx, "down");
    press(cx, "down");
    press(cx, "down");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "ArrowDown must open the closed field"
    );

    // The caller drops "Go"; the input text is untouched, so only the row
    // removal can move the highlight.
    items.borrow_mut().remove(2);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();

    press(cx, "enter");
    assert_eq!(
        seen.borrow().last().map(String::as_str),
        Some("Python"),
        "removing the highlighted row must repair the cursor to the next row"
    );
}

/// React Aria autofocuses the listbox only when it mounts: the keystroke that
/// opens the suggestion list may seat the selected row, but a later keystroke
/// while the list is already open only clears the highlight — it must not
/// re-run the open seat and re-highlight the still-matching selected item.
#[gpui::test]
fn typing_while_open_does_not_rehighlight_the_selected_row(cx: &mut TestAppContext) {
    let picks = events();
    let seen = picks.clone();
    let opens = events();
    let opened = opens.clone();
    let state = combo_state(cx);
    let state_for_view = state;

    let cx = open_host(cx, move || {
        let picks = picks.clone();
        let opens = opens.clone();
        let state = state_for_view.clone();
        ComboBox::new(state, keyed(&["Typst", "Rust", "Go"]))
            .menu_trigger(MenuTrigger::Input)
            .selected_key("Rust")
            .on_selection_change(move |key, _, _| {
                picks.borrow_mut().push(key.to_string());
            })
            .on_open_change(move |open, _, _| {
                opens.borrow_mut().push(format!("open:{open}"));
            })
            .into_any_element()
    });

    click(cx, 60., 18.);
    // The controlled selection renders "Rust" into the field; clear it so the
    // keystrokes below type a fresh query rather than editing that text.
    press(cx, "ctrl-a");
    press(cx, "backspace");
    // The first edit opens the list; "r" still matches the selected "Rust".
    cx.simulate_input("r");
    assert_eq!(
        opened.borrow().as_slice(),
        ["open:true"],
        "the first edit must open the list"
    );
    // A second keystroke while open narrows the query but stays on "Rust";
    // it must clear the highlight, not seat the selected row again.
    cx.simulate_input("ru");
    press(cx, "enter");
    assert!(
        !seen.borrow().iter().any(|key| key == "Rust"),
        "a keystroke while open must not re-highlight the selected row: {:?}",
        seen.borrow()
    );
}
