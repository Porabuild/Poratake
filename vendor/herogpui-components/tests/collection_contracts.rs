//! Collection contracts inherited from HeroUI v3.2.4's pinned React Aria
//! 3.51.0 and React Stately 3.49.0 releases.
//!
//! These are deliberately interaction tests. The prop and anatomy audits
//! cannot observe focus entry, distinguish Space selection from Enter action,
//! or tell whether a still-visible load-more sentinel was re-armed after its
//! collection was replaced.

use crate::harness;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    point, prelude::*, px, ScrollDelta, ScrollEvent, SharedString, TestAppContext,
    VisualTestContext,
};
use herogpui_components::{
    Button, EscapeKeyBehavior, ListBox, ListBoxItem, SelectionMode, TabItem, Table, TableColumn,
    TableRow, Tabs, Tag, TagGroup, VirtualTreeMetadata,
};

use harness::{click, events, open_host, press};

fn sorted_join(keys: &HashSet<SharedString>) -> String {
    let mut keys: Vec<String> = keys.iter().map(ToString::to_string).collect();
    keys.sort();
    keys.join(",")
}

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

fn wheel(cx: &mut VisualTestContext, x: f32, y: f32, dy: f32) {
    cx.simulate_event(ScrollEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
        ..Default::default()
    });
    flush_frame(cx);
}

fn press_mod_a(cx: &mut VisualTestContext) {
    if cfg!(target_os = "macos") {
        press(cx, "cmd-a");
    } else {
        press(cx, "ctrl-a");
    }
}

fn tall_cell(text: impl Into<SharedString>) -> gpui::AnyElement {
    gpui::div()
        .h(px(80.))
        .flex()
        .items_center()
        .child(text.into())
        .into_any_element()
}

/// React Stately's `SelectionManager::toggleSelection` removes an already
/// selected key unless `disallowEmptySelection` is true. HeroUI does not
/// repeat that inherited prop in its v3 table, so this pins its default false
/// behavior alongside the explicit builder tests below.
#[gpui::test]
fn list_box_single_reselect_clears_by_default(cx: &mut TestAppContext) {
    let selected = Rc::new(RefCell::new(HashSet::<SharedString>::new()));
    let recorded = events();
    let selected_for_view = selected;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected = selected_for_view.clone();
        let held = selected.borrow().clone();
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-clear",
            vec![ListBoxItem::new("alpha", "Alpha")],
        )
        .selection_mode(SelectionMode::Single)
        .selected_keys(held)
        .on_selection_change(move |keys, window, _| {
            *selected.borrow_mut() = keys.clone();
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    // p-1 plus half a 36px row puts its centre at (60, 22).
    click(cx, 60., 22.);
    flush_frame(cx);
    click(cx, 60., 22.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", ""],
        "reselecting the only selected row must clear the selection"
    );
}

#[gpui::test]
fn list_box_disallow_empty_selection_blocks_final_item_reselect(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-keep-final",
            vec![ListBoxItem::new("alpha", "Alpha")],
        )
        .selection_mode(SelectionMode::Single)
        .default_selected_keys([SharedString::from("alpha")])
        .disallow_empty_selection(true)
        .on_selection_change(move |keys, _, _| {
            recorded.borrow_mut().push(sorted_join(keys));
        })
        .into_any_element()
    });

    click(cx, 60., 22.);
    assert!(
        recorded.borrow().is_empty(),
        "a blocked final-key removal must emit no selection change"
    );
}

#[gpui::test]
fn list_box_disallow_empty_selection_keeps_last_multiple_item(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-keep-final-multiple",
            vec![ListBoxItem::new("alpha", "Alpha")],
        )
        .selection_mode(SelectionMode::Multiple)
        .default_selected_keys([SharedString::from("alpha")])
        .disallow_empty_selection(true)
        .on_selection_change(move |keys, _, _| {
            recorded.borrow_mut().push(sorted_join(keys));
        })
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "space");
    assert!(recorded.borrow().is_empty());
}

/// Pinned React Aria 3.51 binds platform Mod+A to `selectAll` in a
/// multiple-selection collection, excluding disabled options.
#[gpui::test]
fn list_box_multiple_selects_every_enabled_option_on_mod_a(cx: &mut TestAppContext) {
    let selected = Rc::new(RefCell::new(HashSet::<SharedString>::new()));
    let recorded = events();
    let selected_for_view = selected;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected = selected_for_view.clone();
        let held = selected.borrow().clone();
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-mod-a",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
                ListBoxItem::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .disabled_keys([SharedString::from("beta")])
        .selected_keys(held)
        .on_selection_change(move |keys, window, _| {
            *selected.borrow_mut() = keys.clone();
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    click(cx, 60., 22.);
    flush_frame(cx);
    recorded.borrow_mut().clear();
    press_mod_a(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,gamma"],
        "Mod+A must select every enabled option in multiple mode"
    );
}

/// The Mod+A chord follows the runtime platform identity, not the
/// compile-time host: `os: Mac` makes Cmd+A the chord even for a Web
/// target, and `os: Windows` makes Ctrl+A the chord even on a Mac host --
/// while the other convention's chord stays inert in each.
#[gpui::test]
fn list_box_mod_a_follows_the_runtime_platform(cx: &mut TestAppContext) {
    for (os, target, select_chord, other_chord) in [
        (
            gpui::OperatingSystem::Mac,
            gpui::PlatformTarget::Web,
            "cmd-a",
            "ctrl-a",
        ),
        (
            gpui::OperatingSystem::Windows,
            gpui::PlatformTarget::Native,
            "ctrl-a",
            "cmd-a",
        ),
    ] {
        cx.set_platform_info(gpui::PlatformInfo { os, target });
        let selected = Rc::new(RefCell::new(HashSet::<SharedString>::new()));
        let recorded = events();
        let selected_for_view = selected;
        let for_view = recorded.clone();
        let vcx = open_host(cx, move || {
            let selected = selected_for_view.clone();
            let held = selected.borrow().clone();
            let recorded = for_view.clone();
            ListBox::new(
                "contract-list-mod-a-runtime",
                vec![
                    ListBoxItem::new("alpha", "Alpha"),
                    ListBoxItem::new("beta", "Beta"),
                    ListBoxItem::new("gamma", "Gamma"),
                ],
            )
            .selection_mode(SelectionMode::Multiple)
            .disabled_keys([SharedString::from("beta")])
            .selected_keys(held)
            .on_selection_change(move |keys, window, _| {
                *selected.borrow_mut() = keys.clone();
                recorded.borrow_mut().push(sorted_join(keys));
                window.refresh();
            })
            .into_any_element()
        });

        click(vcx, 60., 22.);
        flush_frame(vcx);
        recorded.borrow_mut().clear();
        press(vcx, other_chord);
        assert!(
            recorded.borrow().is_empty(),
            "{os:?}/{target:?}: {other_chord} must not select all"
        );
        press(vcx, select_chord);
        assert_eq!(
            recorded.borrow().as_slice(),
            ["alpha,gamma"],
            "{os:?}/{target:?}: {select_chord} must select every enabled option"
        );
    }
}

/// Pinned `useSelectableCollection` extends a multiple selection from the last
/// toggled key when Shift is held during keyboard navigation.
#[gpui::test]
fn list_box_shift_down_extends_multiple_selection_from_the_toggle_anchor(cx: &mut TestAppContext) {
    let selected = Rc::new(RefCell::new(HashSet::<SharedString>::new()));
    let recorded = events();
    let selected_for_view = selected;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected = selected_for_view.clone();
        let held = selected.borrow().clone();
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-shift-extend",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
                ListBoxItem::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .selected_keys(held)
        .on_selection_change(move |keys, window, _| {
            *selected.borrow_mut() = keys.clone();
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    press(cx, "tab");
    flush_frame(cx);
    press(cx, "space");
    flush_frame(cx);
    press(cx, "shift-down");
    flush_frame(cx);
    press(cx, "shift-up");
    flush_frame(cx);

    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,beta", "alpha"],
        "Shift+Arrow must extend and reverse from the last toggled row"
    );
}

#[gpui::test]
fn list_box_mod_a_is_idempotent_for_a_selection_superset(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-mod-a-idempotent",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .selected_keys([
            SharedString::from("alpha"),
            SharedString::from("beta"),
            SharedString::from("stale"),
        ])
        .on_selection_change(move |keys, _, _| {
            recorded.borrow_mut().push(sorted_join(keys));
        })
        .into_any_element()
    });

    press(cx, "tab");
    press_mod_a(cx);
    assert!(
        recorded.borrow().is_empty(),
        "Mod+A must not report or drop stale keys once every enabled option is selected"
    );
}

/// Pinned React Aria 3.51's default `escapeKeyBehavior="clearSelection"`
/// clears a nonempty selection while leaving the collection focus in place.
#[gpui::test]
fn list_box_escape_clears_uncontrolled_selection_and_keeps_focus(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let outer_events = for_view.clone();
        let selection_events = for_view.clone();
        gpui::div()
            .on_key_down(move |event, _, _| {
                if event.keystroke.key == "escape" {
                    outer_events.borrow_mut().push("outer-escape".into());
                }
            })
            .child(
                ListBox::new(
                    "contract-list-escape-clear",
                    vec![ListBoxItem::new("alpha", "Alpha")],
                )
                .selection_mode(SelectionMode::Single)
                .default_selected_keys([SharedString::from("alpha")])
                .on_selection_change(move |keys, window, _| {
                    selection_events.borrow_mut().push(sorted_join(keys));
                    window.refresh();
                }),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "escape");
    flush_frame(cx);
    press(cx, "escape");
    press(cx, "space");
    flush_frame(cx);
    press(cx, "shift-escape");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["", "outer-escape", "alpha", "outer-escape"],
        "handled Escape must stop, while empty and modified Escape pass through without losing focus"
    );
}

#[gpui::test]
fn list_box_disallow_empty_selection_leaves_escape_unhandled(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let outer_events = for_view.clone();
        let selection_events = for_view.clone();
        gpui::div()
            .on_key_down(move |event, _, _| {
                if event.keystroke.key == "escape" {
                    outer_events.borrow_mut().push("outer-escape".into());
                }
            })
            .child(
                ListBox::new(
                    "contract-list-keep-escape",
                    vec![ListBoxItem::new("alpha", "Alpha")],
                )
                .selection_mode(SelectionMode::Single)
                .default_selected_keys([SharedString::from("alpha")])
                .disallow_empty_selection(true)
                .on_selection_change(move |keys, _, _| {
                    selection_events.borrow_mut().push(sorted_join(keys));
                }),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "escape");
    assert_eq!(recorded.borrow().as_slice(), ["outer-escape"]);
}

#[gpui::test]
fn list_box_escape_none_preserves_selection_and_bubbles(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let outer_events = for_view.clone();
        let selection_events = for_view.clone();
        gpui::div()
            .on_key_down(move |event, _, _| {
                if event.keystroke.key == "escape" {
                    outer_events.borrow_mut().push("outer-escape".into());
                }
            })
            .child(
                ListBox::new(
                    "contract-list-escape-none",
                    vec![ListBoxItem::new("alpha", "Alpha")],
                )
                .selection_mode(SelectionMode::Single)
                .default_selected_keys([SharedString::from("alpha")])
                .escape_key_behavior(EscapeKeyBehavior::None)
                .on_selection_change(move |keys, _, _| {
                    selection_events.borrow_mut().push(sorted_join(keys));
                }),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "escape");
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["outer-escape", ""],
        "Escape must bubble without changing selection; Space proves the seed remained selected"
    );
}

#[gpui::test]
fn list_box_escape_clears_a_seed_when_every_option_is_disabled(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-disabled-escape-clear",
            vec![ListBoxItem::new("alpha", "Alpha")],
        )
        .selection_mode(SelectionMode::Single)
        .disabled_keys([SharedString::from("alpha")])
        .default_selected_keys([SharedString::from("alpha")])
        .on_selection_change(move |keys, _, _| {
            recorded.borrow_mut().push(sorted_join(keys));
        })
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "escape");
    assert_eq!(
        recorded.borrow().as_slice(),
        [""],
        "the focused collection must clear a seed even when it has no enabled row stop"
    );
}

#[gpui::test]
fn list_box_default_selected_keys_seed_uncontrolled_state(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-default-selection",
            vec![ListBoxItem::new("alpha", "Alpha")],
        )
        .selection_mode(SelectionMode::Single)
        .default_selected_keys([SharedString::from("alpha")])
        .on_selection_change(move |keys, window, _| {
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    click(cx, 60., 22.);
    flush_frame(cx);
    click(cx, 60., 22.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["", "alpha"],
        "the uncontrolled selection must advance from its seed after each press"
    );
}

#[gpui::test]
fn list_box_controlled_empty_overrides_the_uncontrolled_seed(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-controlled-empty",
            vec![ListBoxItem::new("alpha", "Alpha")],
        )
        .selection_mode(SelectionMode::Single)
        .default_selected_keys([SharedString::from("alpha")])
        .selected_keys(Vec::<SharedString>::new())
        .on_selection_change(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    click(cx, 60., 22.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha"],
        "an explicit controlled empty set must not read the uncontrolled seed"
    );
}

#[gpui::test]
fn list_box_instances_keep_separate_keyed_selection(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let first = for_view.clone();
        let second = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .child(
                ListBox::new(
                    "contract-list-instance-one",
                    vec![ListBoxItem::new("alpha", "Alpha")],
                )
                .selection_mode(SelectionMode::Single)
                .on_selection_change(move |keys, window, _| {
                    first
                        .borrow_mut()
                        .push(format!("one:{}", sorted_join(keys)));
                    window.refresh();
                }),
            )
            .child(
                ListBox::new(
                    "contract-list-instance-two",
                    vec![ListBoxItem::new("alpha", "Alpha")],
                )
                .selection_mode(SelectionMode::Single)
                .on_selection_change(move |keys, _, _| {
                    second
                        .borrow_mut()
                        .push(format!("two:{}", sorted_join(keys)));
                }),
            )
            .into_any_element()
    });

    click(cx, 60., 22.);
    flush_frame(cx);
    click(cx, 60., 66.);
    assert_eq!(recorded.borrow().as_slice(), ["one:alpha", "two:alpha"]);
}

/// `useSelectableCollection` moves focus on entry to the first selected item,
/// or the first enabled item when there is no selection. Enter must therefore
/// work immediately after Tab, without an arrow key first.
#[gpui::test]
fn list_box_tab_entry_targets_selected_then_first_item(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected_events = for_view.clone();
        let first_events = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .child(
                ListBox::new(
                    "contract-list-selected-entry",
                    vec![
                        ListBoxItem::new("alpha", "Alpha"),
                        ListBoxItem::new("beta", "Beta"),
                    ],
                )
                .selection_mode(SelectionMode::None)
                .selected_key("beta")
                .on_action(move |key, _, _| {
                    selected_events.borrow_mut().push(format!("selected:{key}"));
                }),
            )
            .child(
                ListBox::new(
                    "contract-list-first-entry",
                    vec![
                        ListBoxItem::new("alpha", "Alpha"),
                        ListBoxItem::new("beta", "Beta"),
                    ],
                )
                .on_action(move |key, _, _| {
                    first_events.borrow_mut().push(format!("first:{key}"));
                }),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "enter");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["selected:beta", "first:alpha"],
        "focus entry must target the selected item, then the first item when none is selected"
    );
}

/// Pinned `useSelectableItem` makes Space the selection key and Enter the
/// action key when an item has both behaviors. Neither key may cross-fire the
/// other callback.
#[gpui::test]
fn list_box_space_selects_and_enter_obeys_toggle_action_priority(cx: &mut TestAppContext) {
    let selected = Rc::new(RefCell::new(HashSet::<SharedString>::new()));
    let recorded = events();
    let selected_for_view = selected;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected = selected_for_view.clone();
        let held = selected.borrow().clone();
        let actions = for_view.clone();
        let selections = for_view.clone();
        ListBox::new(
            "contract-list-key-intent",
            vec![ListBoxItem::new("alpha", "Alpha")],
        )
        .selection_mode(SelectionMode::Single)
        .selected_keys(held)
        .on_action(move |key, _, _| actions.borrow_mut().push(format!("action:{key}")))
        .on_selection_change(move |keys, window, _| {
            *selected.borrow_mut() = keys.clone();
            selections
                .borrow_mut()
                .push(format!("selection:{}", sorted_join(keys)));
            window.refresh();
        })
        .into_any_element()
    });

    // Establish the cursor independently of the separate focus-entry contract.
    press(cx, "tab");
    press(cx, "down");
    recorded.borrow_mut().clear();
    press(cx, "enter");
    press(cx, "space");
    flush_frame(cx);
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["action:alpha", "selection:alpha"],
        "Enter performs the primary action while empty, Space selects, and Enter is inert once toggle selection is active"
    );
}

#[gpui::test]
fn list_box_pointer_action_and_selection_are_mutually_exclusive(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let empty_actions = for_view.clone();
        let empty_selections = for_view.clone();
        let selected_actions = for_view.clone();
        let selected_selections = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .child(
                ListBox::new(
                    "contract-list-pointer-action",
                    vec![ListBoxItem::new("alpha", "Alpha")],
                )
                .selection_mode(SelectionMode::Single)
                .on_action(move |key, _, _| {
                    empty_actions
                        .borrow_mut()
                        .push(format!("empty-action:{key}"));
                })
                .on_selection_change(move |keys, _, _| {
                    empty_selections
                        .borrow_mut()
                        .push(format!("empty-selection:{}", sorted_join(keys)));
                }),
            )
            .child(
                ListBox::new(
                    "contract-list-pointer-selection",
                    vec![ListBoxItem::new("alpha", "Alpha")],
                )
                .selection_mode(SelectionMode::Single)
                .default_selected_keys([SharedString::from("alpha")])
                .on_action(move |key, _, _| {
                    selected_actions
                        .borrow_mut()
                        .push(format!("selected-action:{key}"));
                })
                .on_selection_change(move |keys, _, _| {
                    selected_selections
                        .borrow_mut()
                        .push(format!("selected-selection:{}", sorted_join(keys)));
                }),
            )
            .into_any_element()
    });

    click(cx, 60., 22.);
    click(cx, 60., 66.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["empty-action:alpha", "selected-selection:"],
        "an empty toggle collection gives the pointer to its primary action, while a selected collection gives it to selection"
    );
}

#[gpui::test]
fn list_box_pointer_focuses_the_pressed_row_for_immediate_enter(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-pointer-focus",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
            ],
        )
        .selection_mode(SelectionMode::None)
        .on_action(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    click(cx, 60., 66.);
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["beta", "beta"],
        "the row pressed by the pointer must become the keyboard action target"
    );
}

/// A controlled owner feeds the pointer range back through its prop: a Shift
/// click extends from the anchor the last ordinary click seated, and a repeat
/// Shift click replaces the anchor..current range so walking back shrinks.
#[gpui::test]
fn list_box_controlled_shift_click_extends_and_repeat_shrinks(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(HashSet::<SharedString>::new()));
    let held_for_view = held;
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected = held_for_view.borrow().clone();
        let held = held_for_view.clone();
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-shift-click",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
                ListBoxItem::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .selected_keys(selected)
        .on_selection_change(move |keys, window, _| {
            *held.borrow_mut() = keys.clone();
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    // p-1 plus 36px rows put the row centres at y = 22, 58 and 94.
    click(cx, 60., 22.);
    flush_frame(cx);
    let mut modifiers = gpui::Modifiers::none();
    modifiers.shift = true;
    cx.simulate_click(point(px(60.), px(94.)), modifiers);
    flush_frame(cx);
    cx.simulate_click(point(px(60.), px(58.)), modifiers);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,beta,gamma", "alpha,beta"],
        "Shift+Click must extend from the seated anchor and a repeat must shrink the range rather than toggle"
    );
}

/// Pinned `useSelectableCollection` registers Home and End per platform:
/// Windows and Linux install none, Shift, Control, and Control+Shift, and
/// only Control+Shift extends; macOS installs none, Shift, Alt, and
/// Alt+Shift only, so Shift and Alt+Shift move the focus alone and every
/// Control- or Meta-bearing chord is entirely inert. Each branch drives its
/// own host's real chords; the cfg-free unit truth tables in the components
/// prove both maps everywhere.
#[gpui::test]
fn list_box_home_end_extend_only_from_the_registered_chord(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-shift-home-end",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
                ListBoxItem::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .on_selection_change(move |keys, window, _| {
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    press(cx, "tab shift-end");
    flush_frame(cx);
    assert!(
        recorded.borrow().is_empty(),
        "plain Shift+End must only move focus"
    );
    press(cx, "space");
    flush_frame(cx);
    if cfg!(target_os = "macos") {
        // Control-bearing chords sit outside the macOS registration: the
        // event is entirely inert, not even the focus moves.
        press(cx, "ctrl-shift-home");
        flush_frame(cx);
        assert_eq!(
            recorded.borrow().as_slice(),
            ["gamma"],
            "macOS must leave Control-bearing Home/End entirely inert"
        );
        // Alt+Shift *is* registered on macOS: the focus walks, extends nothing.
        press(cx, "alt-shift-home");
        flush_frame(cx);
        assert_eq!(
            recorded.borrow().as_slice(),
            ["gamma"],
            "macOS Alt+Shift+Home must move the focus without extending"
        );
    } else {
        press(cx, "ctrl-shift-home");
        assert_eq!(
            recorded.borrow().as_slice(),
            ["gamma", "alpha,beta,gamma"],
            "Control+Shift+Home must extend the anchor's range to the first option"
        );
        // The extension seats the cursor on Alpha; an Alt-bearing chord is
        // outside the Windows/Linux registration, so it cannot move the
        // focus or extend again.
        press(cx, "alt-shift-end");
        flush_frame(cx);
        assert_eq!(
            recorded.borrow().as_slice(),
            ["gamma", "alpha,beta,gamma"],
            "an unregistered Alt-bearing chord must leave Home and End inert"
        );
    }
}

/// The whole-event registration guard keeps an unregistered Home/End chord
/// from moving the collection's focus, not only from reporting a selection:
/// the row render state still names Beta after the chord, and the Space that
/// follows must toggle Beta through the focus that never moved.
#[gpui::test]
fn list_box_unregistered_home_end_leave_the_focus_in_place(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let focused = Rc::new(RefCell::new(HashMap::<String, bool>::new()));
    let focused_for_view = focused.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let focused = focused_for_view.clone();
        ListBox::new(
            "contract-list-inert-home-end",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
                ListBoxItem::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .item_content(move |key, state| {
            focused
                .borrow_mut()
                .insert(key.to_string(), state.is_focused);
            gpui::div().into_any_element()
        })
        .on_selection_change(move |keys, window, _| {
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });
    cx.update(|window, _| window.activate_window());
    flush_frame(cx);

    press(cx, "tab down");
    flush_frame(cx);
    assert!(
        focused.borrow()["beta"],
        "Down must seat the collection focus on Beta"
    );
    assert!(
        !focused.borrow()["alpha"] && !focused.borrow()["gamma"],
        "the focus entry rows must yield once Down moves the cursor"
    );

    // Each host's own unregistered chord: Control-bearing on macOS, Alt-
    // bearing on Windows and Linux. Removing the whole-event guard would
    // walk Home to Alpha, which both the render state and the Space target
    // below expose.
    if cfg!(target_os = "macos") {
        press(cx, "ctrl-shift-home");
    } else {
        press(cx, "alt-shift-home");
    }
    flush_frame(cx);
    assert!(
        focused.borrow()["beta"],
        "an unregistered Home/End chord must not move the collection focus"
    );
    assert!(
        !focused.borrow()["alpha"] && !focused.borrow()["gamma"],
        "no other row may pick up the focus the chord must not move"
    );
    press(cx, "space");
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["beta"],
        "the focus the unregistered chord left in place must answer Space with Beta"
    );
}

/// Pinned Stately's raw `all` ends when a pointer deselect toggles a key off,
/// so the next Shift click extends instead of collapsing to its target.
#[gpui::test]
fn list_box_pointer_deselect_after_select_all_clears_the_raw_all(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-raw-all-deselect",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta").is_disabled(true),
                ListBoxItem::new("gamma", "Gamma"),
                ListBoxItem::new("delta", "Delta"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .on_selection_change(move |keys, window, _| {
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    // Click Alpha to seat an anchor, then replace the selection with the raw
    // `all` Mod+A produces. Row centres sit at y = 22, 58, 94 and 130.
    click(cx, 60., 22.);
    flush_frame(cx);
    recorded.borrow_mut().clear();
    press_mod_a(cx);
    flush_frame(cx);
    click(cx, 60., 94.);
    flush_frame(cx);
    let mut modifiers = gpui::Modifiers::none();
    modifiers.shift = true;
    cx.simulate_click(point(px(60.), px(130.)), modifiers);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,delta,gamma", "alpha,delta"],
        "the deselect must end the raw `all`, and the Shift click on an already selected row must extend to it without reporting"
    );
}

/// A Shift range adds only enabled options: a disabled option between the
/// anchor and the target keeps its collection position but never joins.
#[gpui::test]
fn list_box_pointer_shift_click_skips_disabled_options(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        ListBox::new(
            "contract-list-shift-click-disabled",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta").is_disabled(true),
                ListBoxItem::new("gamma", "Gamma"),
                ListBoxItem::new("delta", "Delta"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .on_selection_change(move |keys, window, _| {
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    click(cx, 60., 22.);
    flush_frame(cx);
    let mut modifiers = gpui::Modifiers::none();
    modifiers.shift = true;
    cx.simulate_click(point(px(60.), px(130.)), modifiers);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,delta,gamma"],
        "the Shift+Click range must span the disabled option without selecting it"
    );
}

/// `useTagGroup` constructs a horizontal `ListKeyboardDelegate` with
/// `shouldFocusWrap: true`, so moving left from the first enabled tag lands on
/// the last enabled tag.
#[gpui::test]
fn tag_group_horizontal_arrows_wrap(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-wrap",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Single)
        .on_selection_change(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "left");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["gamma"],
        "Left from the first tag must wrap to the last tag"
    );
}

/// Pinned React Aria 3.51 routes TagGroup through the same selectable-list
/// Mod+A handler as ListBox, excluding disabled tags.
#[gpui::test]
fn tag_group_multiple_selects_every_enabled_tag_on_mod_a(cx: &mut TestAppContext) {
    let selected = Rc::new(RefCell::new(HashSet::<SharedString>::new()));
    let recorded = events();
    let selected_for_view = selected;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected = selected_for_view.clone();
        let held = selected.borrow().clone();
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-mod-a",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .disabled_keys([SharedString::from("beta")])
        .selected_keys(held)
        .on_selection_change(move |keys, window, _| {
            *selected.borrow_mut() = keys.clone();
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    press(cx, "tab");
    press_mod_a(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,gamma"],
        "Mod+A must select every enabled tag in multiple mode"
    );
}

#[gpui::test]
fn tag_group_uncontrolled_mod_a_persists_before_the_next_toggle(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-uncontrolled-mod-a",
            vec![Tag::new("alpha", "Alpha"), Tag::new("beta", "Beta")],
        )
        .selection_mode(SelectionMode::Multiple)
        .default_selected_keys([SharedString::from("alpha")])
        .on_selection_change(move |keys, window, _| {
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    press(cx, "tab");
    press_mod_a(cx);
    flush_frame(cx);
    press(cx, "right");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,beta", "alpha"],
        "the uncontrolled select-all result must become the next toggle's current selection"
    );
}

#[gpui::test]
fn tag_group_mod_a_is_idempotent_for_a_selection_superset(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-mod-a-idempotent",
            vec![Tag::new("alpha", "Alpha"), Tag::new("beta", "Beta")],
        )
        .selection_mode(SelectionMode::Multiple)
        .selected_keys([
            SharedString::from("alpha"),
            SharedString::from("beta"),
            SharedString::from("stale"),
        ])
        .on_selection_change(move |keys, _, _| {
            recorded.borrow_mut().push(sorted_join(keys));
        })
        .into_any_element()
    });

    press(cx, "tab");
    press_mod_a(cx);
    assert!(
        recorded.borrow().is_empty(),
        "Mod+A must not report or drop stale keys once every enabled tag is selected"
    );
}

/// Pinned React Aria 3.51's default `escapeKeyBehavior="clearSelection"`
/// also reaches TagGroup through its selectable-list behavior.
#[gpui::test]
fn tag_group_escape_clears_uncontrolled_selection_and_keeps_focus(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let outer_events = for_view.clone();
        let selection_events = for_view.clone();
        gpui::div()
            .on_key_down(move |event, _, _| {
                if event.keystroke.key == "escape" {
                    outer_events.borrow_mut().push("outer-escape".into());
                }
            })
            .child(
                TagGroup::new(
                    "contract-tags-escape-clear",
                    vec![Tag::new("alpha", "Alpha")],
                )
                .selection_mode(SelectionMode::Single)
                .default_selected_keys([SharedString::from("alpha")])
                .on_selection_change(move |keys, window, _| {
                    selection_events.borrow_mut().push(sorted_join(keys));
                    window.refresh();
                }),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "escape");
    flush_frame(cx);
    press(cx, "escape");
    press(cx, "enter");
    flush_frame(cx);
    press(cx, "shift-escape");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["", "outer-escape", "alpha", "outer-escape"],
        "handled Escape must stop, while empty and modified Escape pass through without losing focus"
    );
}

#[gpui::test]
fn tag_group_escape_clears_a_select_all_result(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-escape-select-all",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .disabled_keys([SharedString::from("beta")])
        .on_selection_change(move |keys, window, _| {
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    press(cx, "tab");
    press_mod_a(cx);
    flush_frame(cx);
    press(cx, "escape");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,gamma", ""],
        "Escape must clear the enabled selection produced by Mod+A"
    );
}

#[gpui::test]
fn tag_group_disallow_empty_selection_blocks_final_toggle_and_escape(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let outer_events = for_view.clone();
        let selection_events = for_view.clone();
        gpui::div()
            .on_key_down(move |event, _, _| {
                if event.keystroke.key == "escape" {
                    outer_events.borrow_mut().push("outer-escape".into());
                }
            })
            .child(
                TagGroup::new(
                    "contract-tags-disallow-empty",
                    vec![Tag::new("alpha", "Alpha")],
                )
                .selection_mode(SelectionMode::Single)
                .default_selected_keys([SharedString::from("alpha")])
                .disallow_empty_selection(true)
                .on_selection_change(move |keys, _, _| {
                    selection_events.borrow_mut().push(sorted_join(keys));
                }),
            )
            .into_any_element()
    });

    click(cx, 30., 14.);
    press(cx, "escape");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["outer-escape"],
        "the final selected tag must stay selected and unhandled Escape must bubble"
    );
}

#[gpui::test]
fn tag_group_escape_none_preserves_selection_and_bubbles(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let outer_events = for_view.clone();
        let selection_events = for_view.clone();
        gpui::div()
            .on_key_down(move |event, _, _| {
                if event.keystroke.key == "escape" {
                    outer_events.borrow_mut().push("outer-escape".into());
                }
            })
            .child(
                TagGroup::new(
                    "contract-tags-escape-none",
                    vec![Tag::new("alpha", "Alpha")],
                )
                .selection_mode(SelectionMode::Single)
                .default_selected_keys([SharedString::from("alpha")])
                .escape_key_behavior(EscapeKeyBehavior::None)
                .on_selection_change(move |keys, _, _| {
                    selection_events.borrow_mut().push(sorted_join(keys));
                }),
            )
            .into_any_element()
    });

    press(cx, "tab");
    press(cx, "escape");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["outer-escape", ""],
        "Escape must bubble without changing selection; Enter proves the seed remained selected"
    );
}

/// Pinned React Aria removes the entire selection when Delete or Backspace is
/// pressed on a selected tag, but reports only the focused tag otherwise.
#[gpui::test]
fn tag_group_remove_key_reports_the_selected_set_or_focused_tag(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-remove-selection",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .selected_keys([SharedString::from("alpha"), SharedString::from("gamma")])
        .on_remove(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "delete");
    press(cx, "right");
    press(cx, "backspace");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,gamma", "beta"],
        "a selected focused tag must remove the selection, while an unselected focused tag removes alone"
    );
}

#[gpui::test]
fn tag_group_remove_key_reads_the_uncontrolled_selection(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-remove-uncontrolled",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .default_selected_keys([SharedString::from("alpha"), SharedString::from("gamma")])
        .on_remove(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "delete");
    press_mod_a(cx);
    flush_frame(cx);
    press(cx, "delete");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,gamma", "alpha,beta,gamma"],
        "keyboard removal must read the live uncontrolled selection"
    );
}

#[gpui::test]
fn tag_group_default_selected_keys_seed_uncontrolled_state(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-default-selection",
            vec![Tag::new("alpha", "Alpha")],
        )
        .selection_mode(SelectionMode::Single)
        .default_selected_keys([SharedString::from("alpha")])
        .on_selection_change(move |keys, window, _| {
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    click(cx, 30., 14.);
    flush_frame(cx);
    click(cx, 30., 14.);
    assert_eq!(recorded.borrow().as_slice(), ["", "alpha"]);
}

#[gpui::test]
fn tag_group_controlled_empty_overrides_the_uncontrolled_seed(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-controlled-empty",
            vec![Tag::new("alpha", "Alpha")],
        )
        .selection_mode(SelectionMode::Single)
        .default_selected_keys([SharedString::from("alpha")])
        .selected_keys(Vec::<SharedString>::new())
        .on_selection_change(move |keys, _, _| {
            recorded.borrow_mut().push(sorted_join(keys));
        })
        .into_any_element()
    });

    click(cx, 30., 14.);
    assert_eq!(recorded.borrow().as_slice(), ["alpha"]);
}

/// A Tag remove button is an action nested inside the selectable tag. Its
/// press removes the tag and must not bubble into the tag's selection press.
#[gpui::test]
fn tag_group_remove_button_does_not_toggle_selection(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let removed = for_view.clone();
        let selected = for_view.clone();
        TagGroup::new(
            "contract-tags-remove",
            vec![Tag::new("alpha", "Alpha"), Tag::new("beta", "Beta")],
        )
        .selection_mode(SelectionMode::Single)
        .tag_content(|_, _| gpui::div().w(px(40.)).h(px(20.)).into_any_element())
        .on_remove(move |keys, _, _| {
            removed
                .borrow_mut()
                .push(format!("remove:{}", sorted_join(keys)));
        })
        .on_selection_change(move |keys, _, _| {
            selected
                .borrow_mut()
                .push(format!("selection:{}", sorted_join(keys)));
        })
        .into_any_element()
    });

    // A 72px tag plus 6px gap puts Beta's 12px remove button centre at x=136.
    click(cx, 136., 14.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["remove:beta"],
        "the nested remove action must not also select Beta"
    );
}

#[gpui::test]
fn tag_group_remove_button_activates_with_enter_and_space(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let removed = for_view.clone();
        TagGroup::new(
            "contract-tags-remove-keyboard",
            vec![Tag::new("alpha", "Alpha")],
        )
        .on_remove(move |keys, _, _| {
            removed
                .borrow_mut()
                .push(format!("remove:{}", sorted_join(keys)));
        })
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    press(cx, "tab");
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["remove:alpha", "remove:alpha"],
        "the focused remove button must activate on Enter and Space"
    );
}

#[gpui::test]
fn tag_group_custom_remove_content_keeps_keyboard_removal(cx: &mut TestAppContext) {
    let rendered = Rc::new(Cell::new(false));
    let recorded = events();
    let rendered_for_view = rendered.clone();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let rendered = rendered_for_view.clone();
        let removed = for_view.clone();
        TagGroup::new(
            "contract-tags-custom-remove",
            vec![Tag::new("alpha", "Alpha").remove_content(move || {
                rendered.set(true);
                gpui::div().child("−").into_any_element()
            })],
        )
        .on_remove(move |keys, _, _| {
            removed
                .borrow_mut()
                .push(format!("remove:{}", sorted_join(keys)));
        })
        .into_any_element()
    });

    assert!(rendered.get(), "the custom remove content must be rendered");
    press(cx, "tab");
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(recorded.borrow().as_slice(), ["remove:alpha"]);
}

/// A remove click reports the removal and then seats the group's focus and
/// roving cursor on the tag that owned the button. Pinned
/// `useSelectableItem` only isolates the child's press and hands DOM focus
/// to the button; this port seats the owning tag itself because the
/// report-only Rust model has no persisting native child and keyboard
/// continuity needs a stable roving target. The next Space and Shift
/// extension therefore originate from Beta — and the selection is not the
/// removal's to change.
#[gpui::test]
fn tag_group_remove_click_seats_the_owning_tag(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let removed = for_view.clone();
        let selected = for_view.clone();
        TagGroup::new(
            "contract-tags-remove-cursor",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .tag_content(|_, _| gpui::div().w(px(40.)).h(px(20.)).into_any_element())
        .on_remove(move |keys, _, _| {
            removed
                .borrow_mut()
                .push(format!("remove:{}", sorted_join(keys)));
        })
        .on_selection_change(move |keys, _, _| selected.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    press(cx, "tab");
    flush_frame(cx);
    // A 72px tag plus 6px gap puts Beta's 12px remove button centre at x=136.
    click(cx, 136., 14.);
    press(cx, "space");
    press(cx, "shift-right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["remove:beta", "beta", "beta,gamma"],
        "the remove click must seat focus and cursor on Beta, so Space and the Shift extension act from there"
    );
}

/// A body press is how a pointer user takes the group: React Aria seats the
/// roving cursor and the collection focus on pointer-down, so with no prior
/// Tab the arrows and Space still answer the tag that was pressed. Pointer
/// focus shows no focus-visible ring (the app root clears the flag on any
/// mouse-down, proved here by forcing the flag on first); the flag returns
/// when a keyboard press reaches the group again.
#[gpui::test]
fn tag_group_body_pointer_seats_cursor_and_focus(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let focused = Rc::new(RefCell::new(HashMap::<String, (bool, bool)>::new()));
    let focused_for_view = focused.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let focused = focused_for_view.clone();
        TagGroup::new(
            "contract-tags-pointer-seat",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .tag_content(move |tag, state| {
            focused.borrow_mut().insert(
                tag.key().to_string(),
                (state.is_focused, state.is_focus_visible),
            );
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .on_selection_change(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });
    cx.update(|window, _| window.activate_window());
    cx.update(|_, cx| herogpui_components::extend::set_focus_visible(true, cx));
    flush_frame(cx);

    // Alpha's body centre: the 12px remove button spans x 52..64, so x=28 is
    // the tag body and no prior Tab has happened.
    click(cx, 28., 14.);
    flush_frame(cx);
    let (alpha_focused, alpha_ring) = focused.borrow()["alpha"];
    assert!(
        alpha_focused && !alpha_ring,
        "the body press must seat the group's focus on Alpha without a focus-visible ring"
    );
    assert!(
        !focused.borrow()["beta"].0,
        "the cursor must stay on the pressed tag"
    );

    // The arrows answer because the press took the group: Right carries the
    // cursor to Beta and Space toggles it. The Space key-down is the first
    // key that bubbles past the chip's stop_propagation, so it is also where
    // the keyboard re-arms the focus-visible flag the click cleared.
    press(cx, "right");
    flush_frame(cx);
    assert!(
        focused.borrow()["beta"].0,
        "the keyboard move must carry the cursor to Beta"
    );
    press(cx, "space");
    flush_frame(cx);
    let (beta_focused, beta_ring) = focused.borrow()["beta"];
    assert!(
        beta_focused && beta_ring,
        "the keyboard press must ring the tag it activates"
    );
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,beta"],
        "the click must have selected Alpha and Space must toggle the tag the seated cursor moved to"
    );
}

/// Pinned React Stately's `extendSelection` replaces the anchor..current range
/// with anchor..target, so walking past a tag and back shrinks to one again.
#[gpui::test]
fn tag_group_shift_arrows_extend_and_reverse_shrink(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-shift-range",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .on_selection_change(move |keys, window, _| {
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    press(cx, "tab shift-right shift-right shift-left");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["beta", "beta,gamma", "beta"],
        "Shift+Right must extend from the anchor and Shift+Left must shrink back"
    );
}

/// The first Shift move with nothing selected has no anchor: the pinned
/// SelectionManager selects the moved-to key alone.
#[gpui::test]
fn tag_group_first_shift_arrow_selects_from_an_empty_selection(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-shift-empty",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .on_selection_change(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    press(cx, "tab shift-right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["beta"],
        "the first Shift+Right from an empty selection must select only the target tag"
    );
}

/// Pinned `useSelectableCollection` registers Home and End per platform:
/// Windows and Linux install none, Shift, Control, and Control+Shift, and
/// only Control+Shift extends; macOS installs none, Shift, Alt, and
/// Alt+Shift only, so Shift and Alt+Shift move the focus alone and every
/// Control- or Meta-bearing chord is entirely inert. Each branch drives its
/// own host's real chords; the cfg-free unit truth tables in the components
/// prove both maps everywhere.
#[gpui::test]
fn tag_group_home_end_extend_only_from_the_registered_chord(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-shift-home-end",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .on_selection_change(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    press(cx, "tab shift-end");
    assert!(
        recorded.borrow().is_empty(),
        "plain Shift+End must only move focus"
    );
    press(cx, "space");
    if cfg!(target_os = "macos") {
        // Control-bearing chords sit outside the macOS registration: the
        // event is entirely inert, not even the focus moves.
        press(cx, "ctrl-shift-home");
        assert_eq!(
            recorded.borrow().as_slice(),
            ["gamma"],
            "macOS must leave Control-bearing Home/End entirely inert"
        );
        // Alt+Shift *is* registered on macOS: the focus walks, extends nothing.
        press(cx, "alt-shift-home");
        assert_eq!(
            recorded.borrow().as_slice(),
            ["gamma"],
            "macOS Alt+Shift+Home must move the focus without extending"
        );
    } else {
        press(cx, "ctrl-shift-home");
        assert_eq!(
            recorded.borrow().as_slice(),
            ["gamma", "alpha,beta,gamma"],
            "Control+Shift+Home must extend the anchor's range to the first tag"
        );
        // The extension seats the cursor on Alpha; an Alt-bearing chord is
        // outside the Windows/Linux registration, so it cannot move the
        // focus or extend again.
        press(cx, "alt-shift-end");
        assert_eq!(
            recorded.borrow().as_slice(),
            ["gamma", "alpha,beta,gamma"],
            "an unregistered Alt-bearing chord must leave Home and End inert"
        );
    }
}

/// The whole-event registration guard keeps an unregistered Home/End chord
/// from moving the group's roving focus, not only from reporting a
/// selection: the tag render state still names Beta after the chord, and
/// the Space that follows must toggle Beta through the focus that never
/// moved.
#[gpui::test]
fn tag_group_unregistered_home_end_leave_the_focus_in_place(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let focused = Rc::new(RefCell::new(HashMap::<String, bool>::new()));
    let focused_for_view = focused.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let focused = focused_for_view.clone();
        TagGroup::new(
            "contract-tags-inert-home-end",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .tag_content(move |tag, state| {
            focused
                .borrow_mut()
                .insert(tag.key().to_string(), state.is_focused);
            gpui::div().w(px(40.)).h(px(20.)).into_any_element()
        })
        .on_selection_change(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });
    cx.update(|window, _| window.activate_window());
    flush_frame(cx);

    press(cx, "tab right");
    flush_frame(cx);
    assert!(
        focused.borrow()["beta"],
        "Right must move the group's roving focus to Beta"
    );
    assert!(
        !focused.borrow()["alpha"] && !focused.borrow()["gamma"],
        "the neighbouring tags must yield once Right moves the cursor"
    );

    // Each host's own unregistered chord: Control-bearing on macOS, Alt-
    // bearing on Windows and Linux. Removing the whole-event guard would
    // walk Home to Alpha, which both the render state and the Space target
    // below expose.
    if cfg!(target_os = "macos") {
        press(cx, "ctrl-shift-home");
    } else {
        press(cx, "alt-shift-home");
    }
    flush_frame(cx);
    assert!(
        focused.borrow()["beta"],
        "an unregistered Home/End chord must not move the group's roving focus"
    );
    assert!(
        !focused.borrow()["alpha"] && !focused.borrow()["gamma"],
        "no other tag may pick up the focus the chord must not move"
    );
    press(cx, "space");
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["beta"],
        "the focus the unregistered chord left in place must answer Space with Beta"
    );
}

/// Pinned row presses route Shift+Click through `extendSelection`, preserving
/// the anchor established by the prior toggle.
#[gpui::test]
fn tag_group_shift_click_extends_from_the_selection_anchor(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-shift-click",
            vec![Tag::new("alpha", "Alpha"), Tag::new("beta", "Beta")],
        )
        .selection_mode(SelectionMode::Multiple)
        .tag_content(|_, _| gpui::div().w(px(40.)).h(px(20.)).into_any_element())
        .on_selection_change(move |keys, window, _| {
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    press(cx, "tab space");
    flush_frame(cx);
    // A 56px tag plus 6px gap puts Beta's centre at x=90. The second press
    // tells extension from toggling: extending keeps Beta, toggling drops it.
    let mut modifiers = gpui::Modifiers::none();
    modifiers.shift = true;
    cx.simulate_click(point(px(90.), px(14.)), modifiers);
    cx.simulate_click(point(px(90.), px(14.)), modifiers);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,beta"],
        "Shift+Click must extend from the anchor rather than re-anchor and toggle"
    );
}

/// A Shift range adds only enabled tags: a disabled tag between the anchor and
/// the target is neither stopped on nor selected.
#[gpui::test]
fn tag_group_shift_extension_skips_disabled_tags(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-shift-disabled",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .disabled_keys([SharedString::from("beta")])
        .on_selection_change(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    press(cx, "tab");
    press(cx, "space");
    press(cx, "shift-right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,gamma"],
        "the Shift range must skip the disabled tag between anchor and target"
    );
}

/// Pinned Stately's raw `all` selection collapses to the moved-to key on the
/// next Shift move instead of extending across everything.
#[gpui::test]
fn tag_group_select_all_then_shift_collapses_to_the_new_tag(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-shift-collapse",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .on_selection_change(move |keys, window, _| {
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    press(cx, "tab");
    press_mod_a(cx);
    flush_frame(cx);
    press(cx, "shift-right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha,beta,gamma", "beta"],
        "the Shift move after select-all must collapse to the target tag"
    );
}

/// Pinned `SelectionManager::selectAll` is idempotent: when ordinary clicks
/// already selected every selectable tag, a redundant Mod+A preserves the
/// anchor instead of arming the raw-`all` collapse, so the next Shift move
/// extends from where the clicks left it.
#[gpui::test]
fn tag_group_redundant_select_all_preserves_the_click_anchor(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-shift-redundant-all",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .on_selection_change(move |keys, window, _| {
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    press(cx, "tab space");
    press(cx, "right space");
    press(cx, "right space");
    press_mod_a(cx);
    flush_frame(cx);
    press(cx, "shift-left");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,beta", "alpha,beta,gamma"],
        "a redundant Mod+A must keep the click anchor, so Shift extends within the full selection instead of collapsing to one tag"
    );
}

/// A registered extension whose Home target already holds the cursor still
/// extends: pinned `extendSelection` replaces the anchor..target range even
/// when the cursor does not move, so anchoring on Gamma and pressing the
/// registered chord from Alpha spans the whole group. The registration is
/// per platform: `secondary` is Ctrl on Windows and Linux, where
/// Ctrl+Shift+Home is exactly the extension chord, and Cmd on macOS, where
/// both chords sit outside the macOS registration and stay entirely inert.
#[gpui::test]
fn tag_group_home_target_equal_to_focus_still_extends(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-shift-home-seated",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .on_selection_change(move |keys, window, _| {
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    press(cx, "tab right right space");
    press(cx, "secondary-home");
    press(cx, "secondary-shift-home");
    if cfg!(target_os = "macos") {
        assert_eq!(
            recorded.borrow().as_slice(),
            ["gamma"],
            "macOS must leave the Cmd-bearing Home chords entirely inert"
        );
    } else {
        assert_eq!(
            recorded.borrow().as_slice(),
            ["gamma", "alpha,beta,gamma"],
            "the extension must run even though Home's target already holds the cursor, replacing the anchor's range"
        );
    }
}

/// The Home/End chord rule follows the runtime platform identity: with
/// `os: Windows` the secondary chord is Ctrl, so Ctrl+Shift+Home extends the
/// selection even on a Mac host; with `os: Mac` the Cmd-bearing chord is
/// outside the macOS registration and stays entirely inert.
#[gpui::test]
fn tag_group_home_end_follows_the_runtime_platform(cx: &mut TestAppContext) {
    for (os, target, expected) in [
        (
            gpui::OperatingSystem::Windows,
            gpui::PlatformTarget::Native,
            vec!["gamma", "alpha,beta,gamma"],
        ),
        (
            gpui::OperatingSystem::Mac,
            gpui::PlatformTarget::Web,
            vec!["gamma"],
        ),
    ] {
        cx.set_platform_info(gpui::PlatformInfo { os, target });
        let recorded = events();
        let for_view = recorded.clone();
        let vcx = open_host(cx, move || {
            let recorded = for_view.clone();
            TagGroup::new(
                "contract-tags-home-runtime",
                vec![
                    Tag::new("alpha", "Alpha"),
                    Tag::new("beta", "Beta"),
                    Tag::new("gamma", "Gamma"),
                ],
            )
            .selection_mode(SelectionMode::Multiple)
            .on_selection_change(move |keys, window, _| {
                recorded.borrow_mut().push(sorted_join(keys));
                window.refresh();
            })
            .into_any_element()
        });

        press(vcx, "tab right right space");
        press(vcx, "secondary-home");
        press(vcx, "secondary-shift-home");
        assert_eq!(
            recorded.borrow().as_slice(),
            expected.as_slice(),
            "{os:?}/{target:?}: the Home/End chord rule must follow the runtime platform"
        );
    }
}

/// A one-tag group's Shift+Arrow wraps to the tag itself: pinned
/// `extendSelection` still selects it, and a repeat whose selection is
/// already the target stays silent.
#[gpui::test]
fn tag_group_single_tag_shift_arrow_selects_the_wrapped_target(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new("contract-tags-shift-single", vec![Tag::new("only", "Only")])
            .selection_mode(SelectionMode::Multiple)
            .on_selection_change(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
            .into_any_element()
    });

    press(cx, "tab shift-right");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["only"],
        "the wrap-to-self Shift move must still extend to the single tag"
    );
    press(cx, "shift-left");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["only"],
        "a repeat extension whose selection is unchanged must not report again"
    );
}

/// Single mode has no ranges: a Shift click replaces the selection exactly
/// like an ordinary click.
#[gpui::test]
fn tag_group_single_mode_shift_click_toggles_instead_of_extending(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-single-shift-click",
            vec![Tag::new("alpha", "Alpha"), Tag::new("beta", "Beta")],
        )
        .selection_mode(SelectionMode::Single)
        .tag_content(|_, _| gpui::div().w(px(40.)).h(px(20.)).into_any_element())
        .on_selection_change(move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    press(cx, "tab space");
    flush_frame(cx);
    let mut modifiers = gpui::Modifiers::none();
    modifiers.shift = true;
    cx.simulate_click(point(px(90.), px(14.)), modifiers);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "beta"],
        "single mode must never range-extend"
    );
}

/// A controlled owner feeds the extension back through its prop; the component
/// only reports and keeps its own anchor across frames.
#[gpui::test]
fn tag_group_controlled_shift_range_reports_and_shrinks(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(HashSet::<SharedString>::new()));
    let held_for_view = held;
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected = held_for_view.borrow().clone();
        let held = held_for_view.clone();
        let recorded = for_view.clone();
        TagGroup::new(
            "contract-tags-controlled-shift",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .selected_keys(selected)
        .on_selection_change(move |keys, window, _| {
            *held.borrow_mut() = keys.clone();
            recorded.borrow_mut().push(sorted_join(keys));
            window.refresh();
        })
        .into_any_element()
    });

    press(cx, "tab shift-right shift-right shift-left");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["beta", "beta,gamma", "beta"],
        "the controlled selection must report each range move and re-anchor from the owner's value"
    );
}

/// React Stately repairs an uncontrolled tab selection when the selected item
/// disappears, choosing the first enabled item. The new panel must therefore
/// become interactive after the collection update.
#[gpui::test]
fn tabs_uncontrolled_selection_recovers_when_selected_item_disappears(cx: &mut TestAppContext) {
    let include_beta = Rc::new(Cell::new(true));
    let recorded = events();
    let include_for_view = include_beta.clone();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let alpha_events = for_view.clone();
        let mut items = vec![TabItem::new("alpha", "Alpha").content(
            gpui::div()
                .id("contract-alpha-panel")
                .w(px(80.))
                .h(px(40.))
                .on_click(move |_, _, _| alpha_events.borrow_mut().push("alpha-panel".into())),
        )];
        if include_for_view.get() {
            items.push(TabItem::new("beta", "Beta").content(gpui::div().h(px(40.))));
        }
        gpui::div()
            .w(px(240.))
            .child(
                Tabs::new("contract-tabs-recover", items, "beta").on_selection_change({
                    let recorded = for_view.clone();
                    move |key, _, _| recorded.borrow_mut().push(format!("selection:{key}"))
                }),
            )
            .into_any_element()
    });

    include_beta.set(false);
    flush_frame(cx);
    flush_frame(cx);
    // The 40px list, 8px root gap, pinned 16px panel margin and 8px panel
    // padding put the recovered panel content at y=72..112.
    click(cx, 20., 80.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["selection:alpha", "alpha-panel"],
        "removing the selected tab must select and render the first remaining tab"
    );
}

/// React Stately repairs an invalid uncontrolled default to the first enabled
/// tab and reports that normalized selection to the owner.
#[gpui::test]
fn tabs_invalid_uncontrolled_default_reports_its_repaired_selection(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Tabs::new(
            "contract-tabs-invalid-default",
            vec![
                TabItem::new("disabled", "Disabled").is_disabled(true),
                TabItem::new("alpha", "Alpha"),
            ],
            "missing",
        )
        .on_selection_change(move |key, _, _| {
            recorded.borrow_mut().push(key.to_string());
        })
        .into_any_element()
    });

    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha"],
        "an invalid defaultSelectedKey must report the first enabled replacement"
    );
}

/// React Aria applies selectable-row press props to the whole table row. The
/// checkbox is one affordance, not the only place a row can be selected.
#[gpui::test]
fn table_row_press_outside_checkbox_selects(cx: &mut TestAppContext) {
    let held = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let recorded = events();
    let held_for_view = held;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let held = held_for_view.clone();
        let selected = held.borrow().clone();
        let recorded = for_view.clone();
        gpui::div()
            .w(px(204.))
            .child(
                Table::new(vec![])
                    .id("contract-table-row-select")
                    .columns(vec![TableColumn::new("Name").default_width(px(160.))])
                    .selection_mode(SelectionMode::Multiple)
                    .selected_keys(selected)
                    .keyed_row("alpha", vec![tall_cell("Alpha")])
                    .on_selection_change(move |keys, window, _| {
                        *held.borrow_mut() = keys.to_vec();
                        recorded.borrow_mut().push(
                            keys.iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(","),
                        );
                        window.refresh();
                    }),
            )
            .into_any_element()
    });

    // x=100 is in the 160px data cell, outside the 44px checkbox column;
    // y=90 is the centre of the 105px row below the ~37px header.
    click(cx, 100., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha"],
        "pressing a row's data cell must select the row"
    );
}

#[gpui::test]
fn table_action_and_selection_arbitrate_pointer_enter_and_space(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let actions = for_view.clone();
        let selections = for_view.clone();
        Table::new(vec![])
            .id("contract-table-action-selection")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Single)
            .keyed_row("alpha", vec![tall_cell("Alpha")])
            .on_row_click(move |index, _, _, _| {
                actions.borrow_mut().push(format!("action:{index}"));
            })
            .on_selection_change(move |keys, window, _| {
                selections.borrow_mut().push(format!(
                    "selection:{}",
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                ));
                window.refresh();
            })
            .into_any_element()
    });

    click(cx, 100., 90.);
    press(cx, "space");
    flush_frame(cx);
    press(cx, "enter");
    click(cx, 100., 90.);
    flush_frame(cx);
    click(cx, 100., 90.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["action:0", "selection:alpha", "selection:", "action:0"],
        "pointer and Enter perform the primary action only while toggle selection is empty; Space selects, and no activation cross-fires"
    );
}

#[gpui::test]
fn table_pointer_row_press_sets_the_keyboard_cursor(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("contract-table-pointer-cursor")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::None)
            .keyed_row("alpha", vec![tall_cell("Alpha")])
            .keyed_row("beta", vec![tall_cell("Beta")])
            .keyed_row("gamma", vec![tall_cell("Gamma")])
            .on_row_click(move |index, _, _, _| {
                recorded.borrow_mut().push(format!("action:{index}"));
            })
            .into_any_element()
    });

    click(cx, 100., 195.);
    press(cx, "enter");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["action:1", "action:1", "action:2"],
        "Enter must reuse the pressed row and Down must advance from it"
    );
}

#[gpui::test]
fn table_checkbox_press_does_not_move_the_row_cursor(cx: &mut TestAppContext) {
    let selected = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let recorded = events();
    let selected_for_view = selected;
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let selected = selected_for_view.clone();
        let held = selected.borrow().clone();
        let recorded = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .child(Button::new("before-checkbox-table").label("Before"))
            .child(
                Table::new(vec![])
                    .id("contract-table-checkbox-cursor")
                    .columns(vec![TableColumn::new("Name").default_width(px(160.))])
                    .selection_mode(SelectionMode::Multiple)
                    .selected_keys(held)
                    .keyed_row("alpha", vec![tall_cell("Alpha")])
                    .keyed_row("beta", vec![tall_cell("Beta")])
                    .on_selection_change(move |keys, window, _| {
                        *selected.borrow_mut() = keys.to_vec();
                        recorded.borrow_mut().push(
                            keys.iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(","),
                        );
                        window.refresh();
                    }),
            )
            .into_any_element()
    });

    click(cx, 100., 231.);
    flush_frame(cx);
    click(cx, 22., 126.);
    flush_frame(cx);
    click(cx, 40., 18.);
    press(cx, "tab");
    press(cx, "space");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["beta", "beta,alpha", "alpha"],
        "the checkbox may select Alpha, but Space must still answer from the Beta row cursor"
    );
}

#[gpui::test]
fn table_uncontrolled_selection_persists_between_row_presses(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("contract-table-uncontrolled")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Single)
            .keyed_row("alpha", vec![tall_cell("Alpha")])
            .on_selection_change(move |keys, window, _| {
                recorded.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                window.refresh();
            })
            .into_any_element()
    });

    click(cx, 100., 90.);
    flush_frame(cx);
    click(cx, 100., 90.);
    assert_eq!(recorded.borrow().as_slice(), ["alpha", ""]);
}

#[gpui::test]
fn table_controlled_empty_remains_caller_owned_between_presses(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("contract-table-controlled-empty")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Single)
            .selected_keys(Vec::<SharedString>::new())
            .keyed_row("alpha", vec![tall_cell("Alpha")])
            .on_selection_change(move |keys, window, _| {
                recorded.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                window.refresh();
            })
            .into_any_element()
    });

    click(cx, 100., 90.);
    flush_frame(cx);
    click(cx, 100., 90.);
    assert_eq!(recorded.borrow().as_slice(), ["alpha", "alpha"]);
}

#[gpui::test]
fn table_nested_row_press_uses_the_flattened_tree_key(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("contract-table-tree-key")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Single)
            .expanded_keys([SharedString::from("parent")])
            .tree_row(
                TableRow::new(vec![tall_cell("Parent")])
                    .key("parent")
                    .children(vec![TableRow::new(vec![tall_cell("Child")]).key("child")]),
            )
            .keyed_row("sibling", vec![tall_cell("Sibling")])
            .on_selection_change(move |keys, _, _| {
                recorded.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    click(cx, 120., 190.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["child"],
        "the child row must not borrow the following top-level row's key"
    );
}

#[gpui::test]
fn table_select_all_includes_collapsed_descendants(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("contract-table-select-all-tree")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::Multiple)
            .selected_keys(["child"])
            .tree_row(
                TableRow::new(vec![tall_cell("Parent")])
                    .key("parent")
                    .children(vec![TableRow::new(vec![tall_cell("Child")]).key("child")]),
            )
            .keyed_row("sibling", vec![tall_cell("Sibling")])
            .on_selection_change(move |keys, _, _| {
                recorded.borrow_mut().push(
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
            })
            .into_any_element()
    });

    click(cx, 22., 18.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["parent,child,sibling"],
        "a partially selected collapsed tree must select the full recursive collection"
    );
}

#[gpui::test]
fn table_interactive_cell_child_owns_its_pointer_press(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let button_events = for_view.clone();
        let selection_events = for_view.clone();
        let action_events = for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .child(Button::new("before-interactive-table").label("Before"))
            .child(
                Table::new(vec![])
                    .id("contract-table-interactive-child")
                    .columns(vec![TableColumn::new("Action").default_width(px(160.))])
                    .selection_mode(SelectionMode::Single)
                    .keyed_row(
                        "alpha",
                        vec![gpui::div()
                            .child(Button::new("cell-action").label("Open").on_press(
                                move |_, _, _| {
                                    button_events.borrow_mut().push("button".into());
                                },
                            ))
                            .into_any_element()],
                    )
                    .keyed_row("beta", vec![tall_cell("Beta")])
                    .on_row_click(move |index, _, _, _| {
                        action_events.borrow_mut().push(format!("action:{index}"));
                    })
                    .on_selection_change(move |_, _, _| {
                        selection_events.borrow_mut().push("selection".into());
                    }),
            )
            .into_any_element()
    });

    click(cx, 180., 181.);
    click(cx, 90., 103.);
    press(cx, "enter");
    click(cx, 40., 18.);
    press(cx, "tab");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["action:1", "button", "button", "action:1"],
        "a nested focusable action must own both pointer and keyboard activation without moving the row cursor"
    );
}

#[gpui::test]
fn table_passive_non_div_cell_content_activates_its_row(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("contract-table-passive-svg")
            .columns(vec![TableColumn::new("Status").default_width(px(160.))])
            .keyed_row(
                "alpha",
                vec![gpui::svg()
                    .size(px(20.))
                    .path(herogpui_components::icons::CHECK)
                    .into_any_element()],
            )
            .on_row_click(move |index, _, _, _| {
                recorded.borrow_mut().push(format!("action:{index}"));
            })
            .into_any_element()
    });

    click(cx, 70., 70.);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["action:0"],
        "a passive non-Div child must not be mistaken for an interactive control"
    );
}

#[gpui::test]
fn table_tree_chevron_moves_the_row_cursor_to_its_row(cx: &mut TestAppContext) {
    let expanded = Rc::new(Cell::new(true));
    let recorded = events();
    let expanded_for_view = expanded.clone();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let action_events = for_view.clone();
        let expanded_events = for_view.clone();
        let expanded_state = expanded.clone();
        Table::new(vec![])
            .id("contract-table-chevron-cursor")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .selection_mode(SelectionMode::None)
            .expanded_keys(
                expanded_for_view
                    .get()
                    .then(|| SharedString::from("parent")),
            )
            .tree_row(
                TableRow::new(vec![tall_cell("Parent")])
                    .key("parent")
                    .children(vec![TableRow::new(vec![tall_cell("Child")]).key("child")]),
            )
            .keyed_row("sibling", vec![tall_cell("Sibling")])
            .on_row_click(move |index, _, _, _| {
                action_events.borrow_mut().push(format!("action:{index}"));
            })
            .on_expanded_change(move |keys, window, _| {
                expanded_state.set(!keys.is_empty());
                expanded_events.borrow_mut().push(format!(
                    "expanded:{}",
                    keys.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                ));
                window.refresh();
            })
            .into_any_element()
    });

    click(cx, 120., 300.);
    click(cx, 29., 90.);
    flush_frame(cx);
    press(cx, "enter");
    press(cx, "up");
    press(cx, "enter");
    press(cx, "down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["action:2", "expanded:", "action:0", "action:0", "action:1"],
        "the chevron must restore its row focus before the collapsed collection remaps navigation"
    );
}

/// `useLoadMoreSentinel` tears down and recreates its observer whenever the
/// collection object changes. Replacing one row with another must therefore
/// re-arm a visible sentinel even though the collection length is unchanged.
#[gpui::test]
fn table_load_more_rearms_after_same_length_collection_replacement(cx: &mut TestAppContext) {
    let second_page = Rc::new(Cell::new(false));
    let recorded = events();
    let page_for_view = second_page.clone();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("contract-table-load-replace")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .scroll_offset(0.)
            .keyed_row(
                if page_for_view.get() { "beta" } else { "alpha" },
                vec![tall_cell(if page_for_view.get() {
                    "Beta"
                } else {
                    "Alpha"
                })],
            )
            .on_load_more(move |_, _| recorded.borrow_mut().push("load-more".into()))
            .into_any_element()
    });

    flush_frame(cx);
    assert_eq!(recorded.borrow().as_slice(), ["load-more"]);
    second_page.set(true);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["load-more", "load-more"],
        "replacing a visible same-length collection must re-observe its sentinel"
    );
}

#[gpui::test]
fn table_load_more_does_not_rearm_for_expansion_only(cx: &mut TestAppContext) {
    let expanded = Rc::new(Cell::new(false));
    let recorded = events();
    let expanded_for_view = expanded.clone();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let expanded = expanded_for_view.get();
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("contract-table-load-expand")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .scroll_offset(0.)
            .expanded_keys(expanded.then(|| SharedString::from("parent")))
            .tree_row(
                TableRow::new(vec![tall_cell("Parent")])
                    .key("parent")
                    .children(vec![TableRow::new(vec![tall_cell("Child")]).key("child")]),
            )
            .on_expanded_change({
                let expanded = expanded_for_view.clone();
                move |keys, window, _| {
                    expanded.set(!keys.is_empty());
                    window.refresh();
                }
            })
            .on_load_more(move |_, _| recorded.borrow_mut().push("load-more".into()))
            .into_any_element()
    });

    flush_frame(cx);
    press(cx, "tab");
    press(cx, "down");
    press(cx, "right");
    flush_frame(cx);
    assert!(expanded.get(), "the tree key must expand the branch");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["load-more"],
        "expansion changes visibility, not the underlying collection identity"
    );
}

#[gpui::test]
fn virtual_table_load_more_does_not_rearm_for_expansion_only(cx: &mut TestAppContext) {
    let expanded = Rc::new(Cell::new(false));
    let recorded = events();
    let expanded_for_view = expanded.clone();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let expanded = expanded_for_view.get();
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("contract-virtual-table-load-expand")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .tree_column(0)
            .row_height(px(80.))
            .max_h(px(160.))
            .scroll_offset(0.)
            .expanded_keys(expanded.then(|| SharedString::from("parent")))
            .virtual_rows(
                2,
                "virtual-tree-data",
                |index| ["parent", "child"][index].into(),
                |index| TableRow::new(vec![tall_cell(["Parent", "Child"][index])]),
            )
            .virtual_tree_metadata(|index| {
                if index == 0 {
                    VirtualTreeMetadata {
                        depth: 0,
                        parent_key: None,
                        has_children: true,
                    }
                } else {
                    VirtualTreeMetadata {
                        depth: 1,
                        parent_key: Some("parent".into()),
                        has_children: false,
                    }
                }
            })
            .on_expanded_change({
                let expanded = expanded_for_view.clone();
                move |keys, window, _| {
                    expanded.set(!keys.is_empty());
                    window.refresh();
                }
            })
            .on_load_more(move |_, _| recorded.borrow_mut().push("load-more".into()))
            .into_any_element()
    });

    flush_frame(cx);
    press(cx, "tab");
    press(cx, "down");
    press(cx, "right");
    flush_frame(cx);
    assert!(
        expanded.get(),
        "the virtual tree key must expand the branch"
    );
    assert_eq!(
        recorded.borrow().as_slice(),
        ["load-more"],
        "virtual expansion changes the visible flatten, not the underlying collection identity"
    );
}

#[gpui::test]
fn virtual_table_load_more_rearms_after_same_count_key_replacement(cx: &mut TestAppContext) {
    let second_page = Rc::new(Cell::new(false));
    let factory_calls = Rc::new(Cell::new(0usize));
    let recorded = events();
    let page_for_view = second_page.clone();
    let calls_for_view = factory_calls.clone();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let page = page_for_view.clone();
        let key_page = page_for_view.clone();
        let calls = calls_for_view.clone();
        let recorded = for_view.clone();
        let identity = if page_for_view.get() {
            "beta-page"
        } else {
            "alpha-page"
        };
        Table::new(vec![])
            .id("contract-virtual-table-load-replace")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .row_height(px(80.))
            .max_h(px(160.))
            .scroll_offset(0.)
            .virtual_rows(
                1000,
                identity,
                move |index| {
                    if key_page.get() {
                        format!("beta-{index}").into()
                    } else {
                        format!("alpha-{index}").into()
                    }
                },
                move |index| {
                    calls.set(calls.get() + 1);
                    if page.get() {
                        TableRow::new(vec![tall_cell(format!("Beta {index}"))])
                    } else {
                        TableRow::new(vec![tall_cell(format!("Alpha {index}"))])
                    }
                },
            )
            .on_load_more(move |_, _| recorded.borrow_mut().push("load-more".into()))
            .into_any_element()
    });

    flush_frame(cx);
    assert!(
        recorded.borrow().is_empty(),
        "a virtual table must not request the next page while row 0 is visible"
    );
    let first_page_calls = factory_calls.get();
    assert!(
        first_page_calls <= 16,
        "two frames of a 160px viewport over 80px rows must stay within visible rows plus bounded overdraw; observed {first_page_calls}"
    );
    wheel(cx, 20., 100., -80000.);
    assert_eq!(recorded.borrow().as_slice(), ["load-more"]);
    flush_frame(cx);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["load-more"],
        "a virtual sentinel that remains at the end must report only once"
    );
    let replacement_start_calls = factory_calls.get();
    second_page.set(true);
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["load-more", "load-more"],
        "a virtual same-count collection replacement must carry its explicit identity into the sentinel"
    );
    assert!(
        factory_calls.get() - replacement_start_calls <= 16,
        "replacing the collection must keep factory work bounded to the viewport; observed {}",
        factory_calls.get() - replacement_start_calls
    );
}

#[gpui::test]
fn variable_height_virtual_table_load_more_waits_until_the_collection_end_is_near(
    cx: &mut TestAppContext,
) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("contract-variable-virtual-table-load-end")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .estimated_row_height(px(80.))
            .max_h(px(400.))
            .virtual_rows(
                32,
                "variable-virtual-tail-data",
                |index| format!("row-{index}").into(),
                |index| TableRow::new(vec![tall_cell(format!("row {index}"))]),
            )
            .on_load_more(move |_, _| recorded.borrow_mut().push("load-more".into()))
            .into_any_element()
    });

    flush_frame(cx);
    assert!(
        recorded.borrow().is_empty(),
        "a variable-height virtual table must not request the next page at row 0"
    );
    press(cx, "tab");
    for _ in 0..32 {
        press(cx, "down");
    }
    flush_frame(cx);
    assert_eq!(recorded.borrow().as_slice(), ["load-more"]);
}

#[gpui::test]
fn variable_height_virtual_table_exact_load_more_uses_the_real_last_row(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        Table::new(vec![])
            .id("contract-variable-virtual-table-exact-load-end")
            .columns(vec![TableColumn::new("Name").default_width(px(160.))])
            .estimated_row_height(px(80.))
            .max_h(px(400.))
            .scroll_offset(0.)
            .virtual_rows(
                32,
                "variable-virtual-short-tail-data",
                |index| format!("row-{index}").into(),
                |index| {
                    TableRow::new(vec![gpui::div()
                        .h(px(40.))
                        .child(format!("row {index}"))
                        .into_any_element()])
                },
            )
            .on_load_more(move |_, _| recorded.borrow_mut().push("load-more".into()))
            .into_any_element()
    });

    flush_frame(cx);
    assert!(recorded.borrow().is_empty());
    press(cx, "tab");
    for _ in 0..32 {
        press(cx, "down");
    }
    flush_frame(cx);
    assert_eq!(
        recorded.borrow().as_slice(),
        ["load-more"],
        "an overestimated row height must not hide the real last row from an exact sentinel"
    );
}
