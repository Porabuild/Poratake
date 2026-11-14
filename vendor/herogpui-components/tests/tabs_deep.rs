//! Deeper Tabs keyboard behaviour: the vertical axis and the ends.
//!
//! v3's Tabs API table documents `orientation: "horizontal" | "vertical"`
//! ("Tab layout orientation") and nothing else about the keyboard; the
//! component is built on React Aria Components (v3's own statement of record),
//! whose `TabsKeyboardDelegate` contract is: a vertical tab list answers both
//! Up/Down and Left/Right, a horizontal one answers Left/Right and ignores
//! Up/Down, and Home/End jump to the first/last tab.
//!
//! `collections.rs` already drives basic horizontal arrows; this file drives
//! both vertical axes, end wrapping, pointer-to-keyboard focus handoff, and
//! orientation layout.

use crate::harness;
use crate::source_scan;

use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui::{
    point, prelude::*, px, Font, FontFeatures, FontStyle, FontWeight, KeyDownEvent, Keystroke,
    Modifiers, MouseButton, Pixels, TestAppContext, VisualTestContext, WindowTextSystem,
};
use herogpui_components::{
    Button, KeyboardActivation, Orientation, TabItem, Tabs, TabsAlign, TabsVariant,
};
use herogpui_theme::ActiveTheme;

use harness::{click, events, open_host, press};

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn text_width(system: &WindowTextSystem, text: &str) -> f32 {
    let run = gpui::TextRun {
        len: text.len(),
        font: Font {
            family: ".SystemUIFont".into(),
            features: FontFeatures::default(),
            weight: FontWeight::MEDIUM,
            style: FontStyle::default(),
            fallbacks: None,
        },
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = system.shape_line(text.to_owned().into(), px(14.), &[run], None);
    f32::from(line.width)
}

/// `Tabs.Tab.isDisabled` is per item, not the root's disabled state. A dead
/// tab takes neither a click nor a roving stop, while its siblings remain live.
#[gpui::test]
fn tabs_disabled_item_is_skipped_by_keys_and_clicks(cx: &mut TestAppContext) {
    let recorded = events();
    let selected = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Tabs::new(
            "tb-disabled-item",
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second").is_disabled(true),
                TabItem::new("third", "Third"),
            ],
            "first",
        )
        .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    let first = cx.update(|window, _| text_width(window.text_system(), "First")) + 32.;
    let second = cx.update(|window, _| text_width(window.text_system(), "Second")) + 32.;
    click(cx, 4. + first + second / 2., 20.);
    assert!(
        selected.borrow().is_empty(),
        "a disabled tab must not answer the pointer"
    );

    press(cx, "tab");
    press(cx, "right");
    press(cx, "left");
    press(cx, "end");
    press(cx, "home");
    assert_eq!(
        selected.borrow().as_slice(),
        ["third", "first", "third", "first"],
        "arrows and Home/End must skip the disabled middle tab"
    );
}

/// A pointer press selects and focuses the tab in pinned React Aria. The next
/// arrow therefore starts at the pressed tab rather than at the old keyboard
/// stop or outside the list.
#[gpui::test]
fn tabs_pointer_selection_hands_off_to_roving_keyboard_focus(cx: &mut TestAppContext) {
    let recorded = events();
    let selected = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Tabs::new(
            "tb-pointer-focus",
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
                TabItem::new("third", "Third"),
            ],
            "first",
        )
        .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    let first = cx.update(|window, _| text_width(window.text_system(), "First")) + 32.;
    let second = cx.update(|window, _| text_width(window.text_system(), "Second")) + 32.;
    click(cx, 4. + first + second / 2., 20.);
    cx.update(|window, _| window.refresh());
    press(cx, "right");

    assert_eq!(
        selected.borrow().as_slice(),
        ["second", "third"],
        "Right after a pointer selection must continue from the pressed tab"
    );
}

fn assert_pointer_selection_timing(
    cx: &mut TestAppContext,
    variant: TabsVariant,
    id: &'static str,
    indicator_id: &'static str,
) {
    let recorded = events();
    let selected = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Tabs::new(
            id,
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
                TabItem::new("third", "Third"),
            ],
            "first",
        )
        .variant(variant)
        .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
        .into_any_element()
    });
    flush_frame(cx);
    flush_frame(cx);
    flush_frame(cx);
    flush_frame(cx);
    let first_indicator = cx
        .debug_bounds(indicator_id)
        .expect("the selected-tab indicator must be painted before pointer input");

    let first = cx.update(|window, _| text_width(window.text_system(), "First")) + 32.;
    let second = cx.update(|window, _| text_width(window.text_system(), "Second")) + 32.;
    let second_x = 4. + first + second / 2.;

    cx.simulate_mouse_down(
        point(px(second_x), px(20.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert_eq!(
        selected.borrow().as_slice(),
        ["second"],
        "pinned useSelectableItem selects on pointer down before any release"
    );

    cx.simulate_mouse_up(
        point(px(second_x), px(20.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert_eq!(
        selected.borrow().as_slice(),
        ["second"],
        "a completed pointer click must report exactly once"
    );

    let third = cx.update(|window, _| text_width(window.text_system(), "Third")) + 32.;
    let third_x = 4. + first + second + third / 2.;
    cx.simulate_mouse_down(
        point(px(third_x), px(20.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    flush_frame(cx);
    cx.simulate_mouse_move(
        point(px(500.), px(120.)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    flush_frame(cx);
    cx.simulate_mouse_up(
        point(px(500.), px(120.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert_eq!(
        selected.borrow().as_slice(),
        ["second", "third"],
        "dragging out must neither revert nor repeat the press-start selection"
    );
    cx.executor().advance_clock(Duration::from_millis(300));
    flush_frame(cx);
    let indicator = cx
        .debug_bounds(indicator_id)
        .expect("the selected-tab indicator must remain painted after dragging out");
    assert!(
        f32::from(indicator.origin.x) >= f32::from(first_indicator.origin.x) + first + second - 1.,
        "dragging out must leave the committed selection on the third tab"
    );
}

/// Pinned react-aria 3.51.0's `useSelectableItem` selects a plain Tab on
/// pointer down. Its pointer-up path does nothing, so dragging out after that
/// press leaves the new selection committed.
#[gpui::test]
fn tabs_select_on_mouse_down_and_keep_a_drag_out(cx: &mut TestAppContext) {
    assert_pointer_selection_timing(
        cx,
        TabsVariant::Primary,
        "tb-primary-pointer-timing",
        "Name(\"tb-primary-pointer-timing\")-indicator",
    );
    assert_pointer_selection_timing(
        cx,
        TabsVariant::Secondary,
        "tb-secondary-pointer-timing",
        "Name(\"tb-secondary-pointer-timing\")-indicator",
    );
}

fn assert_pointer_release_preserves_roving_focus(
    cx: &mut TestAppContext,
    variant: TabsVariant,
    id: &'static str,
) {
    let recorded = events();
    let selected = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Tabs::new(
            id,
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
                TabItem::new("third", "Third"),
            ],
            "first",
        )
        .variant(variant)
        .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    let first = cx.update(|window, _| text_width(window.text_system(), "First")) + 32.;
    let second = cx.update(|window, _| text_width(window.text_system(), "Second")) + 32.;
    let second_point = point(px(4. + first + second / 2.), px(20.));
    cx.simulate_mouse_down(second_point, MouseButton::Left, Modifiers::none());
    press(cx, "right");
    assert_eq!(selected.borrow().as_slice(), ["second", "third"]);

    cx.simulate_mouse_up(second_point, MouseButton::Left, Modifiers::none());
    press(cx, "right");
    assert_eq!(
        selected.borrow().as_slice(),
        ["second", "third", "first"],
        "pointer release must not restore the tab that received pointer down"
    );
}

/// A plain Tab's pointer-up path is a no-op in pinned `useSelectableItem`.
/// Moving with an arrow while the pointer remains held must therefore survive
/// the eventual release rather than snapping the roving key back.
#[gpui::test]
fn tabs_pointer_release_does_not_revert_roving_focus(cx: &mut TestAppContext) {
    assert_pointer_release_preserves_roving_focus(
        cx,
        TabsVariant::Primary,
        "tb-primary-pointer-held",
    );
    assert_pointer_release_preserves_roving_focus(
        cx,
        TabsVariant::Secondary,
        "tb-secondary-pointer-held",
    );
}

/// Pinned `useSingleSelectListState` enables duplicate selection events for
/// Tabs. Clicking or activating the already-selected tab must still notify a
/// controlled owner, even though the selected key itself does not change.
#[gpui::test]
fn tabs_already_selected_tab_reports_every_activation(cx: &mut TestAppContext) {
    let recorded = events();
    let selected = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Tabs::new(
            "tb-repeat-selection",
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
            ],
            "first",
        )
        .selected_key("first")
        .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    let first = cx.update(|window, _| text_width(window.text_system(), "First")) + 32.;
    click(cx, 4. + first / 2., 20.);
    press(cx, "enter");
    press(cx, "space");

    assert_eq!(
        selected.borrow().as_slice(),
        ["first", "first", "first"],
        "pointer, Enter, and Space must each report the already-selected key"
    );
}

/// Pinned `useTabPanel` puts a selected panel with no tabbable child into the
/// tab order. The second Tab therefore leaves the list, and arrow keys from
/// the plain panel must not continue roving or selecting tabs.
#[gpui::test]
fn tabs_second_tab_lands_on_the_panel_and_mutes_arrows(cx: &mut TestAppContext) {
    let recorded = events();
    let selected = recorded.clone();
    let after_presses = events();
    let after = after_presses.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        let after_presses = after_presses.clone();
        gpui::div()
            .child(
                Tabs::new(
                    "tb-panel-stop",
                    vec![
                        TabItem::new("first", "First").content(gpui::div().child("First panel")),
                        TabItem::new("second", "Second").content(gpui::div().child("Second panel")),
                        TabItem::new("third", "Third").content(gpui::div().child("Third panel")),
                    ],
                    "first",
                )
                .on_selection_change(move |key, _, _| {
                    recorded.borrow_mut().push(key.to_string());
                }),
            )
            .child(
                Button::new("tb-panel-after")
                    .label("After tabs")
                    .on_press(move |_, _, _| after_presses.borrow_mut().push("after".into())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    assert!(
        after.borrow().is_empty(),
        "the second Tab must stop on the plain panel before the following button"
    );
    press(cx, "right");
    assert!(
        selected.borrow().is_empty(),
        "the selected panel must own focus after the second Tab"
    );

    press(cx, "shift-tab");
    press(cx, "right");
    assert_eq!(
        selected.borrow().as_slice(),
        ["second"],
        "reverse Tab from the panel must return to the roving list stop"
    );

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        after.borrow().as_slice(),
        ["after"],
        "Tab from the plain panel must reach the following button"
    );
    press(cx, "shift-tab");
    press(cx, "enter");
    assert_eq!(
        after.borrow().as_slice(),
        ["after"],
        "reverse Tab must leave the following button before the panel probe"
    );
    press(cx, "left");
    assert_eq!(
        selected.borrow().as_slice(),
        ["second"],
        "reverse Tab from the following control must stop on the plain panel"
    );
}

/// A selected tab without content has no TabPanel stop. Moving from a tab with
/// content to one without it must remove that stop, and moving back must add it
/// again without leaving stale focus state behind.
#[gpui::test]
fn tabs_uncontrolled_content_switch_updates_panel_traversal(cx: &mut TestAppContext) {
    let after_presses = events();
    let after = after_presses.clone();
    let cx = open_host(cx, move || {
        let after_presses = after_presses.clone();
        gpui::div()
            .child(Tabs::new(
                "tb-content-switch",
                vec![
                    TabItem::new("first", "First").content(gpui::div().child("First panel")),
                    TabItem::new("second", "Second"),
                ],
                "first",
            ))
            .child(
                Button::new("tb-content-switch-after")
                    .label("After tabs")
                    .on_press(move |_, _, _| after_presses.borrow_mut().push("after".into())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        after.borrow().as_slice(),
        ["after"],
        "a no-content selection must leave the list directly for the next stop"
    );

    press(cx, "shift-tab");
    press(cx, "left");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        after.borrow().as_slice(),
        ["after"],
        "restoring content must put the plain panel back before the next stop"
    );
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(after.borrow().as_slice(), ["after", "after"]);
}

/// A panel with a tabbable child is not itself a stop in pinned `useTabPanel`.
/// The second Tab must reach that child directly rather than adding an extra
/// wrapper stop before it.
#[gpui::test]
fn tabs_second_tab_enters_the_panels_first_tabbable_child(cx: &mut TestAppContext) {
    let recorded = events();
    let pressed = recorded.clone();
    let cx = open_host(cx, move || {
        let button_events = recorded.clone();
        let selection_events = recorded.clone();
        gpui::div()
            .child(
                Tabs::new(
                    "tb-panel-child",
                    vec![
                        TabItem::new("first", "First").content(
                            gpui::div().child(
                                Button::new("tb-panel-button")
                                    .label("Panel action")
                                    .on_press(move |_, _, _| {
                                        button_events.borrow_mut().push("pressed".into());
                                    }),
                            ),
                        ),
                        TabItem::new("second", "Second").content(gpui::div().child("Second panel")),
                    ],
                    "first",
                )
                .on_selection_change(move |key, _, _| {
                    selection_events.borrow_mut().push(key.to_string());
                }),
            )
            .child(Button::new("tb-panel-child-after").label("After tabs"))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        pressed.borrow().as_slice(),
        ["pressed"],
        "the first tabbable panel child must be the second stop"
    );

    press(cx, "tab");
    press(cx, "shift-tab");
    press(cx, "enter");
    assert_eq!(
        pressed.borrow().as_slice(),
        ["pressed", "pressed"],
        "reverse Tab from the following control must reach the panel child"
    );

    press(cx, "shift-tab");
    press(cx, "right");
    assert_eq!(
        pressed.borrow().as_slice(),
        ["pressed", "pressed", "second"],
        "reverse Tab from the first panel child must return to the roving list stop"
    );
}

/// A disabled tab list contributes no stop, but it does not disable the
/// selected panel's own controls. Pinned `useTabPanel` therefore yields
/// directly to the first tabbable panel child rather than its wrapper.
#[gpui::test]
fn tabs_all_disabled_enters_the_panels_first_tabbable_child(cx: &mut TestAppContext) {
    let recorded = events();
    let pressed = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Tabs::new(
            "tb-disabled-panel-child",
            vec![TabItem::new("first", "First").content(
                gpui::div().child(
                    Button::new("tb-disabled-panel-button")
                        .label("Panel action")
                        .on_press(move |_, _, _| recorded.borrow_mut().push("pressed".into())),
                ),
            )],
            "first",
        )
        .is_disabled(true)
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        pressed.borrow().as_slice(),
        ["pressed"],
        "the panel child must be the first stop when every tab is disabled"
    );
}

/// Windows sends repeated Tab key-downs while the key is held and only one
/// key-up at release. The synthetic wrapper must not add a repeat stop before
/// the first real panel child.
#[gpui::test]
fn tabs_all_disabled_held_tab_advances_past_the_panel_child(cx: &mut TestAppContext) {
    let recorded = events();
    let pressed = recorded.clone();
    let cx = open_host(cx, move || {
        let child_events = recorded.clone();
        let after_events = recorded.clone();
        gpui::div()
            .child(
                Tabs::new(
                    "tb-disabled-held-tab",
                    vec![TabItem::new("first", "First").content(
                        gpui::div().child(
                            Button::new("tb-disabled-held-child")
                                .label("Panel action")
                                .on_press(move |_, _, _| {
                                    child_events.borrow_mut().push("child".into());
                                }),
                        ),
                    )],
                    "first",
                )
                .is_disabled(true),
            )
            .child(
                Button::new("tb-disabled-held-after")
                    .label("After tabs")
                    .on_press(move |_, _, _| after_events.borrow_mut().push("after".into())),
            )
            .into_any_element()
    });

    cx.simulate_keystrokes("tab");
    cx.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse("tab").expect("tab is a valid keystroke"),
        is_held: true,
        prefer_character_input: false,
    });
    press(cx, "enter");
    assert_eq!(
        pressed.borrow().as_slice(),
        ["after"],
        "the first repeat must advance from the panel child to the following stop"
    );
}

/// The pinned SelectionIndicator is one measured child that owns the selected
/// pill. It starts on the selected tab and settles onto the next tab after its
/// 250ms geometry transition.
#[gpui::test]
fn tabs_indicator_moves_between_measured_tab_boxes(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        Tabs::new(
            "tb-indicator-geometry",
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
            ],
            "first",
        )
        .into_any_element()
    });

    flush_frame(cx);
    flush_frame(cx);
    flush_frame(cx);
    flush_frame(cx);
    let first = cx
        .debug_bounds("Name(\"tb-indicator-geometry\")-indicator")
        .expect("the measured selected-tab indicator must be painted");
    let expected_width = cx.update(|window, _| text_width(window.text_system(), "First")) + 32.;
    assert!(
        (f32::from(first.size.width) - expected_width).abs() < 1.,
        "the primary indicator must match the selected tab width"
    );
    assert!(
        (f32::from(first.size.height) - 32.).abs() < f32::EPSILON,
        "the primary indicator must match the tab height"
    );

    press(cx, "tab");
    press(cx, "right");
    flush_frame(cx);
    let moving = cx
        .debug_bounds("Name(\"tb-indicator-geometry\")-indicator")
        .expect("the moving indicator must remain painted");
    assert!(
        f32::from(moving.origin.x) < f32::from(first.origin.x + first.size.width) - 1.,
        "the indicator must animate rather than snap to the second tab"
    );
    cx.executor().advance_clock(Duration::from_millis(300));
    flush_frame(cx);
    let second = cx
        .debug_bounds("Name(\"tb-indicator-geometry\")-indicator")
        .expect("the indicator must remain painted after moving");
    assert!(
        f32::from(second.origin.x) >= f32::from(first.origin.x + first.size.width) - 1.,
        "after 250ms the indicator must settle onto the second tab"
    );
}

/// Theme changes rebuild every rendered element. The uncontrolled selection is
/// keyed component state, so switching from light to dark must not reseed it
/// from `defaultSelectedKey` between two arrow presses.
#[gpui::test]
fn tabs_uncontrolled_selection_survives_dark_mode_switch(cx: &mut TestAppContext) {
    let recorded = events();
    let selected = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Tabs::new(
            "tb-dark-state",
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
                TabItem::new("third", "Third"),
            ],
            "first",
        )
        .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    cx.update(|window, cx| {
        herogpui_theme::toggle_light_dark(cx);
        window.refresh();
    });
    press(cx, "right");
    assert_eq!(
        selected.borrow().as_slice(),
        ["second", "third"],
        "a theme rebuild must not reseed uncontrolled component state"
    );
}

/// Vertical orientation consumes Down/Up and Right/Left: React Aria's Tabs
/// delegate keeps the horizontal pair active in both orientations.
#[gpui::test]
fn tabs_vertical_axes_down_and_right_move(cx: &mut TestAppContext) {
    let recorded = events();
    let selected = recorded.clone();
    let bubbled = events();
    let outside = bubbled.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        let bubbled = bubbled.clone();
        gpui::div()
            .id("tb-vert-outer")
            .on_key_down(move |event, _, _| {
                if matches!(event.keystroke.key.as_str(), "right" | "down") {
                    bubbled.borrow_mut().push(event.keystroke.key.clone());
                }
            })
            .child(
                Tabs::new(
                    "tb-vert",
                    vec![
                        TabItem::new("first", "First"),
                        TabItem::new("second", "Second"),
                        TabItem::new("third", "Third"),
                    ],
                    "first",
                )
                .orientation(Orientation::Vertical)
                .on_selection_change(move |key, _, _| {
                    recorded.borrow_mut().push(key.to_string());
                }),
            )
            .into_any_element()
    });

    // Tab lands on the selected tab, whose roving handle the keyboard travels
    // with as the selection moves, exactly as the horizontal test does.
    press(cx, "tab");
    press(cx, "down");
    assert_eq!(
        selected.borrow().as_slice(),
        ["second"],
        "a vertical tab list must move the selection down on Down"
    );
    assert!(
        outside.borrow().is_empty(),
        "a consumed axis key must not also move an enclosing navigation control"
    );
    // TabsKeyboardDelegate exposes Left/Right regardless of orientation, so
    // Right advances just like Down and remains inside the tab list.
    press(cx, "right");
    assert_eq!(
        selected.borrow().as_slice(),
        ["second", "third"],
        "a vertical tab list must advance on Right as well as Down"
    );
    assert!(
        outside.borrow().is_empty(),
        "a consumed Right key must not reach an enclosing control"
    );
    press(cx, "down");
    assert_eq!(
        selected.borrow().as_slice(),
        ["second", "third", "first"],
        "Down from the last vertical tab must wrap to the first"
    );
}

/// v3's vertical orientation lays the tab list and panel side by side. The
/// list has an 80px minimum tab width, so the panel content must begin to its
/// right rather than below it at the root's leading edge.
#[gpui::test]
fn tabs_vertical_panel_is_to_the_right_of_the_list(cx: &mut TestAppContext) {
    let panel_bounds = Rc::new(RefCell::new(None));
    let for_view = panel_bounds.clone();
    let cx = open_host(cx, move || {
        let panel_bounds = for_view.clone();
        Tabs::new(
            "tb-vertical-layout",
            vec![
                TabItem::new("first", "A").content(gpui::div().size(px(40.)).child(gpui::canvas(
                    move |bounds, _, _| {
                        *panel_bounds.borrow_mut() = Some(bounds);
                        bounds
                    },
                    |_, _, _, _| {},
                ))),
                TabItem::new("second", "B"),
            ],
            "first",
        )
        .orientation(Orientation::Vertical)
        .into_any_element()
    });

    cx.update(|window, _| window.refresh());
    let bounds = panel_bounds
        .borrow()
        .expect("the selected vertical panel must be painted");
    assert!(
        f32::from(bounds.origin.x) > 100.,
        "the vertical panel must sit to the right of the minimum-width tab list"
    );
}

/// A constrained vertical Tabs list must release its intrinsic min-content
/// width so long labels wrap inside the available tab column, as the pinned
/// `.tabs__tab { width: 100% }` layout does. The panel stays beside the list;
/// it must not be pushed off-screen by one unbroken label. The tab itself
/// keeps the pinned fixed `h-8` box: upstream ships no truncate, nowrap or
/// overflow utility on `.tabs__tab`, so the wrapped lines paint past the pill
/// instead of growing it.
#[gpui::test]
fn tabs_vertical_labels_wrap_inside_the_pinned_fixed_height(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(320.))
            .h(px(180.))
            .child(
                Tabs::new(
                    "tb-vertical-wrap",
                    vec![
                        TabItem::new(
                            "first",
                            "Profile and account security and recovery settings",
                        )
                        .content(gpui::div().child("Security settings.")),
                        TabItem::new("second", "Billing and invoices")
                            .content(gpui::div().child("Billing settings.")),
                    ],
                    "first",
                )
                .orientation(Orientation::Vertical)
                .into_any_element(),
            )
            .into_any_element()
    });

    for _ in 0..4 {
        flush_frame(cx);
    }
    let indicator = cx
        .debug_bounds("Name(\"tb-vertical-wrap\")-indicator")
        .expect("the selected vertical tab indicator must be painted");
    assert_eq!(
        indicator.size.height,
        px(32.),
        "a wrapping label must keep the pinned fixed-height tab, got {indicator:?}"
    );
    assert!(
        indicator.size.width < px(260.),
        "the tab column must release its intrinsic label width, got {indicator:?}"
    );
}

/// Home and End jump the roving selection to the first and last tab
/// respectively — React Aria's `useTabList` is what makes these keys, and the
/// port's `list_nav::resolve` implements them beside the arrows.
#[gpui::test]
fn tabs_home_end_jump_to_the_ends(cx: &mut TestAppContext) {
    let events = events();
    let selected = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        Tabs::new(
            "tb-ends",
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
                TabItem::new("third", "Third"),
            ],
            "first",
        )
        .on_selection_change(move |key, _, _| events.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "end");
    assert_eq!(
        selected.borrow().as_slice(),
        ["third"],
        "End must jump the selection to the last tab"
    );
    press(cx, "home");
    assert_eq!(
        selected.borrow().as_slice(),
        ["third", "first"],
        "Home must jump the selection back to the first tab"
    );
}

/// Pinned `TabsKeyboardDelegate` wraps internally: its next/previous methods
/// join the collection ends independently of `shouldFocusWrap`.
#[gpui::test]
fn tabs_arrow_keys_wrap_at_the_ends(cx: &mut TestAppContext) {
    let events = events();
    let selected = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        Tabs::new(
            "tb-wrap",
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
                TabItem::new("third", "Third"),
            ],
            "first",
        )
        .on_selection_change(move |key, _, _| events.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "left");
    press(cx, "right");

    assert_eq!(
        selected.borrow().as_slice(),
        ["third", "first"],
        "Left at the first tab must wrap to the last, and Right must wrap back"
    );
}

/// `selectedKey` is controlled: proposing a different tab reports it, but the
/// rendered selection remains on the supplied key until the caller changes
/// that prop. Roving focus is independent, so consecutive Right presses report
/// `second` and then `third`.
#[gpui::test]
fn tabs_controlled_selection_keeps_independent_roving_focus(cx: &mut TestAppContext) {
    let proposals = events();
    let selected = proposals.clone();
    let panels = events();
    let clicked_panel = panels.clone();
    let cx = open_host(cx, move || {
        let events = proposals.clone();
        let first_panel = panels.clone();
        let second_panel = panels.clone();
        let third_panel = panels.clone();
        Tabs::new(
            "tb-controlled",
            vec![
                TabItem::new("first", "First").content(
                    gpui::canvas(
                        move |_, _, _| first_panel.borrow_mut().push("first".into()),
                        |_, _, _, _| {},
                    )
                    .size(px(32.)),
                ),
                TabItem::new("second", "Second").content(
                    gpui::canvas(
                        move |_, _, _| second_panel.borrow_mut().push("second".into()),
                        |_, _, _, _| {},
                    )
                    .size(px(32.)),
                ),
                TabItem::new("third", "Third").content(
                    gpui::canvas(
                        move |_, _, _| third_panel.borrow_mut().push("third".into()),
                        |_, _, _, _| {},
                    )
                    .size(px(32.)),
                ),
            ],
            "third",
        )
        .selected_key("first")
        .on_selection_change(move |key, _, _| events.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    press(cx, "tab");
    clicked_panel.borrow_mut().clear();
    press(cx, "right");
    press(cx, "right");
    assert_eq!(
        selected.borrow().as_slice(),
        ["second", "third"],
        "controlled selection must not prevent the roving focus key from advancing"
    );
    assert!(
        !clicked_panel.borrow().is_empty()
            && clicked_panel.borrow().iter().all(|panel| panel == "first"),
        "controlled arrow proposals must keep rendering only the caller-selected panel"
    );
}

/// Pinned `useTabListState` synchronizes focusedKey to selectedKey only while
/// the tab list is not focused. An owner update during keyboard navigation
/// changes the panel but must not change the next arrow's starting tab.
#[gpui::test]
fn tabs_controlled_owner_update_does_not_steal_focused_key(cx: &mut TestAppContext) {
    let selected_key = Rc::new(RefCell::new(String::from("first")));
    let selected_for_view = selected_key.clone();
    let proposals = events();
    let recorded = proposals.clone();
    let cx = open_host(cx, move || {
        let proposals = proposals.clone();
        Tabs::new(
            "tb-controlled-owner-update",
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
                TabItem::new("third", "Third"),
            ],
            "first",
        )
        .selected_key(selected_for_view.borrow().clone())
        .on_selection_change(move |key, _, _| proposals.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    *selected_key.borrow_mut() = String::from("third");
    cx.update(|window, _| window.refresh());
    press(cx, "right");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["second", "third"],
        "an owner selection update while focused must not reset the arrow-key origin"
    );
}

/// When a controlled owner selects a tab with no panel, the focused control in
/// the old panel disappears. The next Tab must re-enter on the selected tab,
/// and the following Tab must leave directly for the next external stop.
#[gpui::test]
fn tabs_controlled_no_content_update_releases_removed_panel_focus(cx: &mut TestAppContext) {
    let selected_key = Rc::new(RefCell::new(String::from("first")));
    let selected_for_view = selected_key.clone();
    let proposals = events();
    let proposed = proposals.clone();
    let panel_presses = events();
    let panel = panel_presses.clone();
    let after_presses = events();
    let after = after_presses.clone();
    let cx = open_host(cx, move || {
        let proposals = proposals.clone();
        let panel_presses = panel_presses.clone();
        let after_presses = after_presses.clone();
        gpui::div()
            .child(Button::new("tb-controlled-no-content-before").label("Before tabs"))
            .child(
                Tabs::new(
                    "tb-controlled-no-content",
                    vec![
                        TabItem::new("first", "First").content(
                            gpui::div().child(
                                Button::new("tb-controlled-panel-button")
                                    .label("Panel action")
                                    .on_press(move |_, _, _| {
                                        panel_presses.borrow_mut().push("panel".into());
                                    }),
                            ),
                        ),
                        TabItem::new("second", "Second"),
                    ],
                    "first",
                )
                .selected_key(selected_for_view.borrow().clone())
                .on_selection_change(move |key, _, _| {
                    proposals.borrow_mut().push(key.to_string());
                }),
            )
            .child(
                Button::new("tb-controlled-no-content-after")
                    .label("After tabs")
                    .on_press(move |_, _, _| after_presses.borrow_mut().push("after".into())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(panel.borrow().as_slice(), ["panel"]);

    *selected_key.borrow_mut() = String::from("second");
    cx.update(|window, _| window.refresh());
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        after.borrow().as_slice(),
        ["after"],
        "two Tabs after the focused panel unmounts must reach the following stop"
    );
    assert!(
        proposed.borrow().is_empty(),
        "focus recovery must not reactivate the already-selected no-content tab"
    );
}

/// A controlled selection change also removes the focused child when both the
/// old and new tabs have panels. Recovery must re-enter on the selected tab,
/// pass through the new plain panel, and then leave for the following stop.
#[gpui::test]
fn tabs_controlled_content_update_releases_removed_panel_focus(cx: &mut TestAppContext) {
    let selected_key = Rc::new(RefCell::new(String::from("first")));
    let selected_for_view = selected_key.clone();
    let proposals = events();
    let proposed = proposals.clone();
    let panel_presses = events();
    let panel = panel_presses.clone();
    let after_presses = events();
    let after = after_presses.clone();
    let cx = open_host(cx, move || {
        let proposals = proposals.clone();
        let panel_presses = panel_presses.clone();
        let after_presses = after_presses.clone();
        gpui::div()
            .child(Button::new("tb-controlled-content-before").label("Before tabs"))
            .child(
                Tabs::new(
                    "tb-controlled-content",
                    vec![
                        TabItem::new("first", "First").content(
                            gpui::div().child(
                                Button::new("tb-controlled-content-panel-button")
                                    .label("Panel action")
                                    .on_press(move |_, _, _| {
                                        panel_presses.borrow_mut().push("panel".into());
                                    }),
                            ),
                        ),
                        TabItem::new("second", "Second").content(gpui::div().child("Second panel")),
                    ],
                    "first",
                )
                .selected_key(selected_for_view.borrow().clone())
                .on_selection_change(move |key, _, _| {
                    proposals.borrow_mut().push(key.to_string());
                }),
            )
            .child(
                Button::new("tb-controlled-content-after")
                    .label("After tabs")
                    .on_press(move |_, _, _| after_presses.borrow_mut().push("after".into())),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(panel.borrow().as_slice(), ["panel"]);

    *selected_key.borrow_mut() = String::from("second");
    cx.update(|window, _| window.refresh());
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        after.borrow().as_slice(),
        ["after"],
        "three Tabs after the focused panel changes must reach the following stop"
    );
    assert!(
        proposed.borrow().is_empty(),
        "focus recovery must not activate the externally selected content tab"
    );
}

/// Pinned list state repairs a removed focused key to the next surviving
/// enabled key, falling back to the previous one only when there is no next
/// item. The controlled selection remains independent throughout.
#[gpui::test]
fn tabs_removed_focused_key_repairs_to_the_next_survivor(cx: &mut TestAppContext) {
    let keys = Rc::new(RefCell::new(vec!["first", "second", "third"]));
    let keys_for_view = keys.clone();
    let proposals = events();
    let recorded = proposals.clone();
    let cx = open_host(cx, move || {
        let proposals = proposals.clone();
        let items = keys_for_view
            .borrow()
            .iter()
            .map(|key| TabItem::new(*key, *key))
            .collect();
        Tabs::new("tb-controlled-removal", items, "first")
            .selected_key("first")
            .on_selection_change(move |key, _, _| proposals.borrow_mut().push(key.to_string()))
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    keys.borrow_mut().remove(1);
    cx.update(|window, _| window.refresh());
    press(cx, "right");

    assert_eq!(
        recorded.borrow().as_slice(),
        ["second", "first"],
        "removing the focused second tab must repair focus to third before Right wraps to first"
    );
}

/// HeroUI v3.2.4 pins `react-aria` 3.51.0 and `react-stately` 3.49.0: the
/// vertical tab delegate maps Down to the next tab, while controlled state
/// reports the proposal without replacing the caller's selected key.
#[gpui::test]
fn tabs_controlled_secondary_vertical_advances_roving_focus(cx: &mut TestAppContext) {
    let proposals = events();
    let selected = proposals.clone();
    let panels = events();
    let clicked_panel = panels.clone();
    let cx = open_host(cx, move || {
        let events = proposals.clone();
        let first_panel = panels.clone();
        let second_panel = panels.clone();
        let third_panel = panels.clone();
        Tabs::new(
            "tb-controlled-secondary-vertical",
            vec![
                TabItem::new("first", "First").content(
                    gpui::canvas(
                        move |_, _, _| first_panel.borrow_mut().push("first".into()),
                        |_, _, _, _| {},
                    )
                    .size(px(32.)),
                ),
                TabItem::new("second", "Second").content(
                    gpui::canvas(
                        move |_, _, _| second_panel.borrow_mut().push("second".into()),
                        |_, _, _, _| {},
                    )
                    .size(px(32.)),
                ),
                TabItem::new("third", "Third").content(
                    gpui::canvas(
                        move |_, _, _| third_panel.borrow_mut().push("third".into()),
                        |_, _, _, _| {},
                    )
                    .size(px(32.)),
                ),
            ],
            "first",
        )
        .variant(TabsVariant::Secondary)
        .orientation(Orientation::Vertical)
        .selected_key("first")
        .on_selection_change(move |key, _, _| events.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    press(cx, "tab");
    clicked_panel.borrow_mut().clear();
    press(cx, "down");
    press(cx, "down");
    assert_eq!(
        selected.borrow().as_slice(),
        ["second", "third"],
        "controlled vertical tabs must move roving focus without mutating the selected key"
    );
    assert!(
        !clicked_panel.borrow().is_empty()
            && clicked_panel.borrow().iter().all(|panel| panel == "first"),
        "controlled vertical arrow proposals must keep rendering only the first panel"
    );
}

/// Pinned React Aria's `keyboardActivation="manual"` moves the roving focus
/// with arrows without selecting. Enter and Space activate the focused tab
/// afterward, so a controlled owner receives one proposal per completed press.
#[gpui::test]
fn tabs_manual_activation_waits_for_enter(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Tabs::new(
            "tb-manual",
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
                TabItem::new("third", "Third"),
            ],
            "first",
        )
        .selected_key("first")
        .keyboard_activation(KeyboardActivation::Manual)
        .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "right");
    assert!(
        recorded.borrow().is_empty(),
        "manual activation must not select merely because focus moved"
    );

    cx.update(|window, _| window.refresh());
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["second"],
        "Enter must activate the manually focused tab"
    );
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["second", "second"],
        "Space must activate the manually focused tab"
    );
}

/// Drives the tab list through every tab and returns the box the indicator
/// occupies for each one. The indicator is laid over the selected tab's
/// measured bounds, so with motion suppressed it reports the tab geometry the
/// layout produced — the only handle a headless test has on it.
fn measure_tabs(
    cx: &mut VisualTestContext,
    selector: &'static str,
    count: usize,
) -> Vec<gpui::Bounds<Pixels>> {
    for _ in 0..4 {
        flush_frame(cx);
    }
    press(cx, "tab");
    flush_frame(cx);
    let mut boxes = Vec::with_capacity(count);
    for index in 0..count {
        if index > 0 {
            press(cx, "right");
            flush_frame(cx);
            flush_frame(cx);
        }
        boxes.push(
            cx.debug_bounds(selector)
                .expect("the selected-tab indicator must be painted"),
        );
    }
    boxes
}

#[gpui::test]
fn tabs_alignment_preserves_indicator_geometry_and_keyboard_selection(cx: &mut TestAppContext) {
    for variant in TabsVariant::ALL {
        for orientation in [Orientation::Horizontal, Orientation::Vertical] {
            let mut default_boxes = None;
            for alignment in [
                None,
                Some(TabsAlign::Start),
                Some(TabsAlign::Center),
                Some(TabsAlign::End),
            ] {
                harness::still();
                let recorded = events();
                let for_view = recorded.clone();
                let cx = open_host(cx, move || {
                    let recorded = for_view.clone();
                    let tabs = Tabs::new(
                        "aligned",
                        vec![
                            TabItem::new("a", "A"),
                            TabItem::new("disabled", "Disabled").is_disabled(true),
                            TabItem::new("b", "Longer billing label"),
                        ],
                        "a",
                    )
                    .variant(variant)
                    .orientation(orientation)
                    .full_width(true)
                    .on_selection_change(move |key, _, _| {
                        recorded.borrow_mut().push(key.to_string());
                    });
                    gpui::div()
                        .w(px(600.))
                        .h(px(160.))
                        .child(match alignment {
                            Some(align) => tabs.align(align),
                            None => tabs,
                        })
                        .into_any_element()
                });
                let boxes = measure_tabs(cx, "Name(\"aligned\")-indicator", 2);
                assert_eq!(
                    recorded.borrow().as_slice(),
                    ["b"],
                    "alignment must retain disabled skipping and activation"
                );
                if let Some(default) = &default_boxes {
                    assert_eq!(&boxes, default, "{variant:?}/{orientation:?}/{alignment:?}: alignment must move content only");
                } else {
                    default_boxes = Some(boxes);
                }
            }
        }
    }
}

/// `full_width` is v3's `<Tabs.List className="w-full">` with
/// `<Tabs.Trigger className="flex-1">`: the list spans its parent and the tabs
/// divide that width into equal shares.
#[gpui::test]
fn tabs_full_width_divides_the_container_equally(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(300.))
            .child(
                Tabs::new(
                    "tb-full-width",
                    vec![
                        TabItem::new("first", "First"),
                        TabItem::new("second", "Second"),
                        TabItem::new("third", "Third"),
                    ],
                    "first",
                )
                // Secondary's underline spans the tab without the primary
                // list's `p-1`, so the shares tile the container exactly.
                .variant(TabsVariant::Secondary)
                .full_width(true),
            )
            .into_any_element()
    });

    let boxes = measure_tabs(cx, "Name(\"tb-full-width\")-indicator", 3);
    let widths: Vec<f32> = boxes.iter().map(|b| f32::from(b.size.width)).collect();
    for width in &widths {
        assert!(
            (width - widths[0]).abs() < 1.,
            "full-width tabs must take equal shares, got {widths:?}"
        );
    }
    let span = f32::from(boxes[2].origin.x + boxes[2].size.width) - f32::from(boxes[0].origin.x);
    assert!(
        (span - 300.).abs() < 1.,
        "the equal shares must tile the container width, spanning {span}"
    );
    let total: f32 = widths.iter().sum();
    assert!(
        (total - span).abs() < 1.,
        "the shares must be contiguous: {total} of {span}"
    );
}

/// The default is unchanged: `.tabs__list` is `w-max`, so each tab hugs its
/// own label and the row is narrower than a wide container.
#[gpui::test]
fn tabs_default_width_stays_content_sized(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(300.))
            .child(
                Tabs::new(
                    "tb-content-width",
                    vec![
                        TabItem::new("first", "First"),
                        TabItem::new("second", "Second"),
                        TabItem::new("third", "Third"),
                    ],
                    "first",
                )
                .variant(TabsVariant::Secondary),
            )
            .into_any_element()
    });

    let boxes = measure_tabs(cx, "Name(\"tb-content-width\")-indicator", 3);
    let expected: Vec<f32> = ["First", "Second", "Third"]
        .iter()
        .map(|label| cx.update(|window, _| text_width(window.text_system(), label)) + 32.)
        .collect();
    for (index, measured) in boxes.iter().enumerate() {
        assert!(
            (f32::from(measured.size.width) - expected[index]).abs() < 1.,
            "tab {index} must stay content-sized: {} vs {}",
            f32::from(measured.size.width),
            expected[index]
        );
    }
    let span = f32::from(boxes[2].origin.x + boxes[2].size.width) - f32::from(boxes[0].origin.x);
    assert!(
        span < 299.,
        "content-sized tabs must not span the container, spanning {span}"
    );
}

/// Equal shares must not reintroduce the overflow the scroller exists for:
/// labels that overflow content-sized are clipped inside their share instead of
/// widening the row past the viewport.
#[gpui::test]
fn tabs_full_width_does_not_overflow_the_scroller(cx: &mut TestAppContext) {
    harness::still();
    let long = || {
        vec![
            TabItem::new("first", "Overview and history"),
            TabItem::new("second", "Billing and invoices"),
            TabItem::new("third", "Notifications and alerts"),
        ]
    };
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(240.))
            .child(
                Tabs::new("tb-full-width-overflow", long(), "first")
                    .variant(TabsVariant::Secondary)
                    .full_width(true),
            )
            .into_any_element()
    });

    let boxes = measure_tabs(cx, "Name(\"tb-full-width-overflow\")-indicator", 3);
    let right = f32::from(boxes[2].origin.x + boxes[2].size.width);
    assert!(
        (right - 240.).abs() < 1.,
        "the stretched row must end at the viewport edge, not past it: {right}"
    );
    let natural: f32 = [
        "Overview and history",
        "Billing and invoices",
        "Notifications and alerts",
    ]
    .iter()
    .map(|label| cx.update(|window, _| text_width(window.text_system(), label)) + 32.)
    .sum();
    assert!(
        natural > 240.,
        "the fixture must be one that overflows when content-sized: {natural}"
    );
}

/// The icon-only segmented control the reference capture toolbar builds: every
/// trigger renders an element instead of the label text, and the list overrides
/// its container and indicator paint. The observable half — geometry, hit
/// testing and keyboard activation — must survive the paint overrides and the
/// element triggers. Fills and corner radii are read off the painted scene in
/// `tabs_paint_overrides_reach_their_parts`; shadows have no headless
/// readback, and the gallery example is their visual proof.
#[gpui::test]
fn tabs_icon_triggers_render_activate_and_measure(cx: &mut TestAppContext) {
    harness::still();
    let recorded = events();
    let selected = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        let icon = |name: &'static str| {
            gpui::div()
                .size(px(16.))
                .debug_selector(move || name.to_owned())
        };
        Tabs::new(
            "tb-icon-trigger",
            vec![
                TabItem::new("grid", "Grid").trigger(icon("icon-grid")),
                TabItem::new("eye", "Preview").trigger(icon("icon-eye")),
                TabItem::new("gear", "Settings").trigger(icon("icon-gear")),
            ],
            "grid",
        )
        .radius(px(24.))
        .list_bg(gpui::rgb(0x101010))
        .indicator_bg(gpui::rgb(0x202020))
        .indicator_shadow(false)
        .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    // A 16px icon inside the pinned 32px-high box with 16px side padding: one
    // 48px segment. The label text must not render beside the icon, or the tab
    // box — and with it the indicator — would grow past it.
    for _ in 0..4 {
        flush_frame(cx);
    }
    let grid = cx
        .debug_bounds("icon-grid")
        .expect("the icon trigger element must render inside its tab");
    assert!(
        (f32::from(grid.size.width) - 16.).abs() < f32::EPSILON
            && (f32::from(grid.size.height) - 16.).abs() < f32::EPSILON,
        "the trigger element must render at its own size, got {grid:?}"
    );
    assert!(
        (f32::from(grid.origin.x) - 20.).abs() < 1.,
        "the icon must sit inside the tab's 16px side padding, got {grid:?}"
    );

    let indicator = cx
        .debug_bounds("Name(\"tb-icon-trigger\")-indicator")
        .expect("the selected icon segment's indicator must be painted");
    assert!(
        (f32::from(indicator.size.width) - 48.).abs() < 1.,
        "the indicator must measure the icon segment's tab box (16px icon + 2 x 16px padding), \
         not the absent label, got {indicator:?}"
    );
    assert!(
        (f32::from(indicator.size.height) - 32.).abs() < f32::EPSILON,
        "the indicator must keep the pinned tab height, got {indicator:?}"
    );
    assert!(
        (f32::from(indicator.origin.x) - 4.).abs() < 1.,
        "the first segment starts at the list's 4px inset, got {indicator:?}"
    );

    // The trigger is presentational: the press reaches the tab through it,
    // along the same pointer path a text label takes.
    click(cx, 4. + 48. + 24., 20.);
    assert_eq!(
        selected.borrow().as_slice(),
        ["eye"],
        "clicking an icon-only segment must select its tab"
    );
    flush_frame(cx);
    let eye = cx
        .debug_bounds("Name(\"tb-icon-trigger\")-indicator")
        .expect("the indicator must remain painted after the selection");
    assert!(
        (f32::from(eye.origin.x) - 52.).abs() < 1.,
        "reduced motion must snap the indicator onto the pressed icon segment, got {eye:?}"
    );

    // And the roving keyboard still activates icon-only tabs with no label
    // text anywhere in the list.
    press(cx, "tab");
    press(cx, "right");
    assert_eq!(
        selected.borrow().as_slice(),
        ["eye", "gear"],
        "arrow keys must keep activating icon-only tabs"
    );
    flush_frame(cx);
    let gear = cx
        .debug_bounds("Name(\"tb-icon-trigger\")-indicator")
        .expect("the indicator must follow the keyboard selection");
    let gear_icon = cx
        .debug_bounds("icon-gear")
        .expect("the third icon renders");
    let icon_center = f32::from(gear_icon.origin.x) + f32::from(gear_icon.size.width) / 2.;
    assert!(
        f32::from(gear.origin.x) < icon_center
            && icon_center < f32::from(gear.origin.x + gear.size.width),
        "the indicator must cover the focused icon segment: {gear:?} vs {gear_icon:?}"
    );
}

/// The 0.10.1 per-segment box overrides: `width`, `height` and `padding_x`
/// each beat what the size step derives, stay scoped to the tab that carries
/// them, and leave their siblings on the pinned metrics — in both variants.
/// Primary's full-box indicator reports each segment's exact box; Secondary's
/// underline only tracks widths, so its fixed segment carries an `h_full`
/// probe to read the overridden height.
#[gpui::test]
fn tabs_segment_box_overrides_beat_the_size_step_in_both_variants(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, move || {
        Tabs::new(
            "tb-segment-box",
            vec![
                TabItem::new("fixed", "A")
                    .width(px(64.))
                    .height(px(36.))
                    .padding_x(px(0.)),
                TabItem::new("stock", "Second"),
                // No width: with the padding gone the box hugs the label
                // exactly, which is what `padding_x(0)` means on its own.
                TabItem::new("flush", "Third").padding_x(px(0.)),
            ],
            "fixed",
        )
        .into_any_element()
    });
    let boxes = measure_tabs(cx, "Name(\"tb-segment-box\")-indicator", 3);
    assert_eq!(
        boxes[0].size.width,
        px(64.),
        "the fixed width must beat the content-hugging box, got {boxes:?}"
    );
    assert_eq!(
        boxes[0].size.height,
        px(36.),
        "the fixed height must beat the size step's tab height, got {boxes:?}"
    );
    let stock = cx.update(|window, _| text_width(window.text_system(), "Second")) + 32.;
    assert!(
        (f32::from(boxes[1].size.width) - stock).abs() < 1.,
        "an unoverridden sibling must keep the pinned box, expected {stock}, got {:?}",
        boxes[1]
    );
    assert_eq!(boxes[1].size.height, px(32.), "sibling height");
    let flush = cx.update(|window, _| text_width(window.text_system(), "Third"));
    assert!(
        (f32::from(boxes[2].size.width) - flush).abs() < 1.,
        "padding_x(0) must leave the label hugging the box with no horizontal \
         padding, expected {flush}, got {:?}",
        boxes[2]
    );
    assert_eq!(boxes[2].size.height, px(32.), "flush height");

    harness::still();
    let cx = open_host(cx, move || {
        Tabs::new(
            "tb-segment-box-underline",
            vec![
                // The probe stretches to its segment's full height, the only
                // per-tab readback the underline variant offers.
                TabItem::new("fixed", "A")
                    .width(px(64.))
                    .height(px(36.))
                    .padding_x(px(0.))
                    .trigger(
                        gpui::div()
                            .h_full()
                            .w(px(2.))
                            .debug_selector(|| "segment-probe".to_owned()),
                    ),
                TabItem::new("stock", "Second"),
                TabItem::new("flush", "Third").padding_x(px(0.)),
            ],
            "fixed",
        )
        .variant(TabsVariant::Secondary)
        .into_any_element()
    });
    let boxes = measure_tabs(cx, "Name(\"tb-segment-box-underline\")-indicator", 3);
    assert_eq!(
        boxes[0].size.width,
        px(64.),
        "the fixed width must beat the content-hugging box, got {boxes:?}"
    );
    let stock = cx.update(|window, _| text_width(window.text_system(), "Second")) + 32.;
    assert!(
        (f32::from(boxes[1].size.width) - stock).abs() < 1.,
        "an unoverridden sibling must keep the pinned box, expected {stock}, got {:?}",
        boxes[1]
    );
    let flush = cx.update(|window, _| text_width(window.text_system(), "Third"));
    assert!(
        (f32::from(boxes[2].size.width) - flush).abs() < 1.,
        "padding_x(0) must leave the label hugging the box, expected {flush}, got {:?}",
        boxes[2]
    );
    let probe = cx
        .debug_bounds("segment-probe")
        .expect("the fixed segment must render its height probe");
    assert_eq!(
        probe.size.height,
        px(36.),
        "the fixed height must beat the size step's tab height, got {probe:?}"
    );
}

/// An explicit per-tab width wins over `full_width`'s equal shares for that
/// tab alone: the fixed segment keeps its 64px, its siblings divide the rest
/// equally, and the shares still tile the container edge to edge.
#[gpui::test]
fn tabs_explicit_width_wins_over_full_width_shares(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(300.))
            .child(
                Tabs::new(
                    "tb-width-vs-stretch",
                    vec![
                        TabItem::new("fixed", "F").width(px(64.)),
                        TabItem::new("second", "Second"),
                        TabItem::new("third", "Third"),
                    ],
                    "fixed",
                )
                // Secondary again: no tray inset, so the shares tile the
                // container exactly.
                .variant(TabsVariant::Secondary)
                .full_width(true),
            )
            .into_any_element()
    });

    let boxes = measure_tabs(cx, "Name(\"tb-width-vs-stretch\")-indicator", 3);
    assert_eq!(
        boxes[0].size.width,
        px(64.),
        "the fixed segment must keep its width under full_width, got {boxes:?}"
    );
    let share1 = f32::from(boxes[1].size.width);
    let share2 = f32::from(boxes[2].size.width);
    assert!(
        (share1 - share2).abs() < 1.,
        "the unstretched siblings must divide the rest equally: {share1} vs {share2}"
    );
    assert!(
        (share1 - (300. - 64.) / 2.).abs() < 1.,
        "the siblings must split what the fixed segment leaves: {share1}"
    );
    let span = f32::from(boxes[2].origin.x + boxes[2].size.width) - f32::from(boxes[0].origin.x);
    assert!(
        (span - 300.).abs() < 1.,
        "the fixed segment and the shares must still tile the container, spanning {span}"
    );
}

/// In a vertical list an explicit width replaces both the cross-axis stretch
/// and the 80px minimum that would clamp a narrow segment back up.
#[gpui::test]
fn tabs_explicit_width_replaces_the_vertical_minimum(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, move || {
        Tabs::new(
            "tb-vertical-width",
            vec![TabItem::new("only", "Only").width(px(48.))],
            "only",
        )
        .orientation(Orientation::Vertical)
        .into_any_element()
    });

    let boxes = measure_tabs(cx, "Name(\"tb-vertical-width\")-indicator", 1);
    assert_eq!(
        boxes[0].size.width,
        px(48.),
        "the explicit width must beat the 80px vertical floor, got {boxes:?}"
    );
    assert_eq!(
        boxes[0].size.height,
        px(32.),
        "the height keeps the size step"
    );
}

/// The consumer's reference toolbar box: icon triggers in fixed 32px segments
/// with no side padding, a flush tray with no inset, and no hover wash. The
/// indicator must measure the fixed box exactly, animate onto the next
/// segment when the selection moves, and the fixed zero-padding boxes must
/// still answer the pointer.
#[gpui::test]
fn tabs_icon_toolbar_fixed_segments_measure_slide_and_activate(cx: &mut TestAppContext) {
    let recorded = events();
    let selected = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        let icon = |name: &'static str| {
            gpui::div()
                .size(px(16.))
                .debug_selector(move || name.to_owned())
        };
        Tabs::new(
            "tb-icon-toolbar",
            vec![
                TabItem::new("eye", "Preview")
                    .trigger(icon("toolbar-eye"))
                    .width(px(32.))
                    .height(px(32.))
                    .padding_x(px(0.)),
                TabItem::new("copy", "Copy")
                    .trigger(icon("toolbar-copy"))
                    .width(px(32.))
                    .height(px(32.))
                    .padding_x(px(0.)),
            ],
            "eye",
        )
        .list_padding(px(0.))
        .hover_fill(false)
        .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    for _ in 0..4 {
        flush_frame(cx);
    }
    let first = cx
        .debug_bounds("Name(\"tb-icon-toolbar\")-indicator")
        .expect("the selected segment's indicator must be painted");
    assert_eq!(
        first.origin.x,
        px(0.),
        "list_padding(0) must leave no tray inset ahead of the first segment"
    );
    assert_eq!(
        first.size.width,
        px(32.),
        "the indicator must measure the fixed 32px segment, not the 16px icon"
    );
    assert_eq!(first.size.height, px(32.), "the fixed height must hold");
    let eye = cx
        .debug_bounds("toolbar-eye")
        .expect("the icon trigger must render inside its fixed segment");
    assert!(
        (f32::from(eye.origin.x) - 8.).abs() < 1.,
        "the 16px icon must center inside the padding-less 32px segment, got {eye:?}"
    );

    // The two fixed segments tile 64x32 with nothing between or around them,
    // so the second segment's center is exactly one box past the first.
    click(cx, 32. + 16., 16.);
    assert_eq!(
        selected.borrow().as_slice(),
        ["copy"],
        "the fixed zero-padding segment must take the press at its own center"
    );
    flush_frame(cx);
    let moving = cx
        .debug_bounds("Name(\"tb-icon-toolbar\")-indicator")
        .expect("the indicator must remain painted while moving");
    assert!(
        f32::from(moving.origin.x) < f32::from(first.origin.x + first.size.width) - 1.,
        "the indicator must animate rather than snap between fixed boxes"
    );
    cx.executor().advance_clock(Duration::from_millis(300));
    flush_frame(cx);
    let second = cx
        .debug_bounds("Name(\"tb-icon-toolbar\")-indicator")
        .expect("the indicator must remain painted after settling");
    assert_eq!(
        second.origin.x,
        px(32.),
        "after the transition the indicator must sit exactly on the second segment"
    );
    assert_eq!(
        second.size.width,
        px(32.),
        "the fixed width must hold there"
    );
}

/// Walks the roving selection across both tabs, hands it back to the first,
/// then parks the pointer over the now-unselected second tab and returns the
/// solid fills painted that frame — the seam the list_bg wash test uses.
fn hovered_unselected_tab_solids(
    cx: &mut TestAppContext,
    id: &'static str,
    selector: &'static str,
    list_bg: bool,
    hover_fill: Option<bool>,
) -> Vec<gpui::Hsla> {
    harness::still();
    let cx = open_host(cx, move || {
        let mut tabs = Tabs::new(
            id,
            vec![TabItem::new("a", "A"), TabItem::new("b", "B")],
            "a",
        );
        if list_bg {
            tabs = tabs.list_bg(gpui::rgb(0x1a1a2e));
        }
        if let Some(fill) = hover_fill {
            tabs = tabs.hover_fill(fill);
        }
        tabs.into_any_element()
    });
    let boxes = measure_tabs(cx, selector, 2);
    press(cx, "left");
    flush_frame(cx);
    let unselected = &boxes[1];
    let center = point(
        px(f32::from(unselected.origin.x) + f32::from(unselected.size.width) / 2.),
        px(f32::from(unselected.origin.y) + f32::from(unselected.size.height) / 2.),
    );
    cx.simulate_mouse_move(center, None, Modifiers::none());
    flush_frame(cx);
    cx.update(|window, _| {
        window
            .painted_quads()
            .into_iter()
            .filter_map(|quad| quad.background.as_solid())
            .collect::<Vec<_>>()
    })
}

/// `hover_fill(false)` stops painting the unselected-tab hover wash and
/// replaces it with a text-colour hover: a hovered unselected tab keeps its
/// resting fill (including over an overridden tray) while its label colour
/// moves to the theme foreground. The default and an explicit `true` keep
/// painting the wash. Text colour itself has no painted-quad readback, so
/// the colour path is read by a style probe in
/// `tabs_toolbar_overrides_reach_the_tray_and_the_label`.
#[gpui::test]
fn tabs_hover_fill_false_removes_the_wash(cx: &mut TestAppContext) {
    let stock_wash = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            cx.colors()
                .default
                .color
                .alpha(1.0 - cx.layout().tabs_hover_opacity)
        })
    };

    let default_solids = hovered_unselected_tab_solids(
        cx,
        "tb-wash-default",
        "Name(\"tb-wash-default\")-indicator",
        false,
        None,
    );
    assert!(
        default_solids.contains(&stock_wash(cx)),
        "the default must keep painting the hover wash, painted: {default_solids:?}"
    );

    let on_solids = hovered_unselected_tab_solids(
        cx,
        "tb-wash-on",
        "Name(\"tb-wash-on\")-indicator",
        false,
        Some(true),
    );
    assert!(
        on_solids.contains(&stock_wash(cx)),
        "hover_fill(true) is today's rendering, painted: {on_solids:?}"
    );

    let off_solids = hovered_unselected_tab_solids(
        cx,
        "tb-wash-off",
        "Name(\"tb-wash-off\")-indicator",
        false,
        Some(false),
    );
    assert!(
        !off_solids.contains(&stock_wash(cx)),
        "hover_fill(false) must not paint the wash, painted: {off_solids:?}"
    );

    // And the composition: with the wash off, no tray fill is dimmed over the
    // tab regardless of what the tray resolves to.
    let override_bg = gpui::rgb(0x1a1a2e);
    let composed_solids = hovered_unselected_tab_solids(
        cx,
        "tb-wash-composed",
        "Name(\"tb-wash-composed\")-indicator",
        true,
        Some(false),
    );
    let override_wash =
        cx.update(|cx| gpui::Hsla::from(override_bg).alpha(1.0 - cx.layout().tabs_hover_opacity));
    assert!(
        !composed_solids.contains(&override_wash) && !composed_solids.contains(&stock_wash(cx)),
        "hover_fill(false) must wash nothing over an overridden tray either, \
         painted: {composed_solids:?}"
    );
}

/// `indicator_bg` recolors the secondary accent line without disturbing its
/// measured 2px geometry or the variant's activation path. The colour itself
/// is read off the painted underline in `tabs_paint_overrides_reach_their_parts`.
#[gpui::test]
fn tabs_secondary_indicator_bg_keeps_underline_geometry_and_activation(cx: &mut TestAppContext) {
    harness::still();
    let recorded = events();
    let selected = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        Tabs::new(
            "tb-secondary-indicator-bg",
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
            ],
            "first",
        )
        .variant(TabsVariant::Secondary)
        .indicator_bg(gpui::rgb(0x00aa44))
        .on_selection_change(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    let boxes = measure_tabs(cx, "Name(\"tb-secondary-indicator-bg\")-indicator", 2);
    assert_eq!(
        selected.borrow().as_slice(),
        ["second"],
        "the recolored secondary indicator must not change activation"
    );
    for (index, bounds) in boxes.iter().enumerate() {
        assert!(
            (f32::from(bounds.size.height) - 2.).abs() < f32::EPSILON,
            "secondary tab {index} keeps its 2px underline with the override, got {bounds:?}"
        );
    }
    assert!(
        boxes[1].origin.x > boxes[0].origin.x,
        "the recolored underline must still slide to the selected tab"
    );
}

/// With `list_bg` set, the unselected-tab hover wash fades to the resolved
/// tray fill — the override dimmed to `tabs_hover_opacity` — and not to the
/// stock tray token: a state fill fades from the resting background. The wash
/// is a painted quad, so the assertion goes through the same
/// `Window::painted_quads` seam the scrollbar tests use.
#[gpui::test]
fn tabs_hover_wash_fades_from_the_list_bg_override(cx: &mut TestAppContext) {
    harness::still();
    let override_bg = gpui::rgb(0x1a1a2e);
    let cx = open_host(cx, move || {
        Tabs::new(
            "tb-wash",
            vec![TabItem::new("a", "A"), TabItem::new("b", "B")],
            "a",
        )
        .list_bg(override_bg)
        .into_any_element()
    });
    // Drive the roving selection through both tabs: the indicator laid over
    // each selected tab reports that tab's box, the only handle a headless
    // test has on an unselected tab's position.
    let boxes = measure_tabs(cx, "Name(\"tb-wash\")-indicator", 2);
    press(cx, "left");
    flush_frame(cx);

    let (override_wash, stock_wash) = cx.update(|_, cx| {
        let opacity = cx.layout().tabs_hover_opacity;
        (
            gpui::Hsla::from(override_bg).alpha(1.0 - opacity),
            cx.colors().default.color.alpha(1.0 - opacity),
        )
    });
    // The pointer is parked off-window until now; move onto the second tab,
    // unselected since the `left` above handed selection back to the first.
    let unselected = &boxes[1];
    let center = point(
        px(f32::from(unselected.origin.x) + f32::from(unselected.size.width) / 2.),
        px(f32::from(unselected.origin.y) + f32::from(unselected.size.height) / 2.),
    );
    cx.simulate_mouse_move(center, None, Modifiers::none());
    flush_frame(cx);
    let solids = cx.update(|window, _| {
        window
            .painted_quads()
            .into_iter()
            .filter_map(|quad| quad.background.as_solid())
            .collect::<Vec<_>>()
    });
    assert!(
        solids.contains(&override_wash),
        "the hover wash must fade to the list_bg override dimmed by the token \
         opacity, painted solid fills: {solids:?}"
    );
    assert!(
        !solids.contains(&stock_wash),
        "the stock tray token must not wash over an overridden tray, painted \
         solid fills: {solids:?}"
    );
}

/// The paint knobs on the painted scene: each override refines the part it
/// names after the resolved theme default. The indicator is the quad at the
/// indicator's debug bounds, the tray the filled quad around it.
fn indicator_and_tray(
    cx: &mut VisualTestContext,
    id: &'static str,
) -> (harness::Painted, gpui::Quad, Option<gpui::Quad>) {
    let selector = Box::leak(format!("Name(\"{id}\")-indicator").into_boxed_str());
    let boxes = measure_tabs(cx, selector, 1);
    let scene = harness::painted(cx);
    let indicator = scene
        .quads
        .iter()
        .find(|q| {
            let b = scene.bounds(q);
            harness::contains(b, boxes[0]) && harness::contains(boxes[0], b)
        })
        .copied()
        .unwrap_or_else(|| panic!("{id}: the indicator must paint\n{}", scene.describe()));
    let tray = scene
        .around(boxes[0])
        .into_iter()
        .filter(|q| q.bounds != indicator.bounds)
        .filter(|q| q.background.as_solid().is_some_and(|c| c.a > 0.))
        .min_by(|a, b| {
            let area = |q: &&gpui::Quad| q.bounds.size.width.0 * q.bounds.size.height.0;
            area(a).total_cmp(&area(b))
        })
        .copied();
    (scene, indicator, tray)
}

fn two_tabs(id: &'static str) -> Tabs {
    Tabs::new(
        id,
        vec![TabItem::new("a", "Alpha"), TabItem::new("b", "Beta")],
        "a",
    )
}

#[gpui::test]
fn tabs_paint_overrides_reach_their_parts(cx: &mut TestAppContext) {
    const R: f32 = 5.;
    let (list, fill) = (gpui::rgb(0x1a1a2e).into(), gpui::rgb(0x00aa44).into());

    // Primary, stock: the indicator is the segment fill at the control
    // radius, the tray is `bg-default` at `radius * 2.5`.
    harness::still();
    let vcx = open_host(cx, || two_tabs("tp-stock").into_any_element());
    let (scene, indicator, tray) = indicator_and_tray(vcx, "tp-stock");
    let (segment, default, control, tray_r) = vcx.update(|_, cx| {
        (
            cx.colors().segment.background,
            cx.colors().default.color,
            f32::from(herogpui_components::extend::control_radius(cx)),
            f32::from(cx.layout().radius_lg() * 2.5),
        )
    });
    let tray = tray.expect("the stock tray must paint");
    assert_eq!(indicator.background.as_solid(), Some(segment));
    assert!(
        scene.is_uniform(&indicator, control),
        "stock indicator radius"
    );
    assert_eq!(tray.background.as_solid(), Some(default));
    assert!(scene.is_uniform(&tray, tray_r), "stock tray radius");

    // Primary, overridden: every knob lands on its own part.
    harness::still();
    let vcx = open_host(cx, move || {
        two_tabs("tp-over")
            .radius(px(R))
            .list_bg(list)
            .indicator_bg(fill)
            .into_any_element()
    });
    let (scene, indicator, tray) = indicator_and_tray(vcx, "tp-over");
    let tray = tray.expect("the overridden tray must paint");
    assert_eq!(indicator.background.as_solid(), Some(fill), "indicator_bg");
    assert!(
        scene.is_uniform(&indicator, R),
        "radius reaches the indicator"
    );
    assert_eq!(tray.background.as_solid(), Some(list), "list_bg");
    assert!(scene.is_uniform(&tray, R), "radius reaches the tray");

    // Secondary: the underline is the accent line, recoloured by the same
    // `indicator_bg`.
    for (id, over) in [("ts-stock", None), ("ts-over", Some(fill))] {
        harness::still();
        let vcx = open_host(cx, move || {
            let tabs = two_tabs(id).variant(TabsVariant::Secondary);
            match over {
                Some(c) => tabs.indicator_bg(c),
                None => tabs,
            }
            .into_any_element()
        });
        let (_, indicator, _) = indicator_and_tray(vcx, id);
        let accent = vcx.update(|_, cx| cx.colors().accent.color);
        assert_eq!(
            indicator.background.as_solid(),
            Some(over.unwrap_or(accent)),
            "{id}: the underline must paint the override or the accent"
        );
    }
}

/// Remaining source-text checks, each for a reason the painted scene cannot
/// cover: the indicator's `surface_shadow` (shadows are not exposed); the
/// pre-measurement fallback the active tab paints until the indicator has
/// geometry (the test window's first presented frame already has it, so the
/// fallback frame is never the one `painted_quads` returns); and the tab's
/// accessible name (the headless platform builds no AccessKit tree;
/// `a11y_deep.rs` pins why). That an element `trigger` replaces the label
/// is measured by `tabs_icon_triggers_render_activate_and_measure`.
#[test]
fn tabs_shadow_and_accessible_name_are_wired_at_their_parts() {
    let source = source_scan::component_src("tabs.rs");
    assert!(source.contains("indicator_shadow && !layout.surface_shadow.is_empty()"));
    assert!(source.contains("tab.bg(indicator_bg.unwrap_or(colors.segment.background))"));
    assert!(source.contains("tab.border_color(indicator_bg.unwrap_or(colors.accent.color))"));
    assert!(
        source.contains("a11y_named(a11y::Role::Tab, &a11y::Name::labelled(item.label.clone()))"),
        "the tab element must restate `label` as its accessible name"
    );
    assert!(
        source.contains(".rounded(crate::util::control_radius(cx))"),
        "the indicator's default radius call is the design-audit fixture anchor"
    );
}

/// `list_padding` replaces the tray's pinned `p-1` inset, and
/// `hover_fill(false)` recolours a hovered unselected label to the
/// foreground in both variants in place of the wash (the wash itself is
/// `tabs_hover_fill_false_removes_the_wash`). The label colour is read by a
/// style probe drawn as the unselected tab's trigger.
#[gpui::test]
fn tabs_toolbar_overrides_reach_the_tray_and_the_label(cx: &mut TestAppContext) {
    for (id, padding, want) in [("tl-stock", None, 4.), ("tl-wide", Some(10.), 10.)] {
        harness::still();
        let vcx = open_host(cx, move || {
            let tabs = two_tabs(id);
            match padding {
                Some(p) => tabs.list_padding(px(p)),
                None => tabs,
            }
            .into_any_element()
        });
        let (scene, indicator, tray) = indicator_and_tray(vcx, id);
        let tray = scene.bounds(&tray.expect("the tray must paint"));
        let inset = f32::from(scene.bounds(&indicator).origin.x - tray.origin.x);
        assert!(
            (inset - want).abs() < 0.5,
            "{id}: the first tab must sit {want}px inside the tray, got {inset}"
        );
    }

    for variant in [TabsVariant::Primary, TabsVariant::Secondary] {
        for hover_fill in [true, false] {
            let sink = harness::style_sink();
            let seen = sink.clone();
            harness::still();
            let vcx = open_host(cx, move || {
                Tabs::new(
                    "th",
                    vec![
                        TabItem::new("a", "Alpha"),
                        TabItem::new("b", "Beta").trigger(
                            gpui::div()
                                .debug_selector(|| "th-b".into())
                                .w(px(40.))
                                .h(px(16.))
                                .child(harness::style_probe(&sink)),
                        ),
                    ],
                    "a",
                )
                .variant(variant)
                .hover_fill(hover_fill)
                .into_any_element()
            });
            harness::settle(vcx);
            let resting = harness::seen_style(&seen, "unselected tab").color;
            let target = vcx
                .debug_bounds("th-b")
                .expect("the trigger must be laid out");
            vcx.simulate_mouse_move(target.center(), None, Modifiers::none());
            harness::settle(vcx);
            let hovered = harness::seen_style(&seen, "hovered tab").color;
            let (muted, foreground) =
                vcx.update(|_, cx| (cx.colors().muted, cx.colors().foreground));
            assert_eq!(
                resting, muted,
                "{variant:?}: an unselected label rests muted"
            );
            assert_eq!(
                hovered,
                if hover_fill { muted } else { foreground },
                "{variant:?} hover_fill={hover_fill}: only the wash-less hover \
                 recolours the label"
            );
        }
    }
}
