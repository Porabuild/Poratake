//! `ContextMenu` (HeroGPUI extension): a secondary press opens the shared
//! Dropdown `Menu` at the pointer; choosing an item, Escape or a press
//! outside closes it; a disabled context menu ignores the press.

use crate::harness;

use gpui::{
    point, prelude::*, px, Bounds, Modifiers, MouseButton, Pixels, TestAppContext,
    VisualTestContext,
};
use harness::{events, open_host, press, still, Events};
use herogpui_components::{
    Breadcrumbs, Button, ColorArea, ContextMenu, Crumb, ListBox, ListBoxItem, MenuBar, MenuBarMenu,
    MenuItem, PickerColor, Table, TreeItem, TreeView,
};

const PANEL: &str = "context-menu";

fn host(cx: &mut TestAppContext, disabled: bool) -> (Events, &mut VisualTestContext) {
    still();
    let seen = events();
    let s = seen.clone();
    let cx = open_host(cx, move || {
        let (a, o) = (s.clone(), s.clone());
        ContextMenu::new(
            "ctx",
            gpui::div().w(px(400.)).h(px(300.)),
            vec![
                MenuItem::new("copy", "Copy"),
                MenuItem::new("paste", "Paste"),
                MenuItem::new("delete", "Delete").danger(),
            ],
        )
        .is_disabled(disabled)
        .disabled_keys(["paste"])
        .on_action(move |key, _, _| a.borrow_mut().push(format!("action:{key}")))
        .on_open_change(move |open, _, _| o.borrow_mut().push(format!("open:{open}")))
        .into_any_element()
    });
    (seen, cx)
}

fn right_click(cx: &mut VisualTestContext, x: f32, y: f32) {
    let at = point(px(x), px(y));
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    frame(cx);
}

fn frame(cx: &mut VisualTestContext) {
    // Past any exit run (a closing menu stays mounted for its motion).
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(300));
    for _ in 0..3 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

fn panel(cx: &mut VisualTestContext) -> Option<Bounds<Pixels>> {
    cx.debug_bounds(PANEL)
}

#[gpui::test]
fn secondary_press_opens_at_the_pointer(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, false);
    frame(cx);
    assert!(panel(cx).is_none(), "closed until a secondary press");
    cx.simulate_click(point(px(100.), px(80.)), Modifiers::none());
    frame(cx);
    assert!(panel(cx).is_none(), "a primary press does not open it");

    right_click(cx, 100., 80.);
    let bounds = panel(cx).expect("the menu opened");
    assert_eq!(bounds.origin, point(px(100.), px(80.)));
    assert_eq!(seen.borrow().as_slice(), ["open:true"]);
}

#[gpui::test]
fn choosing_an_item_acts_and_closes(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, false);
    frame(cx);
    right_click(cx, 60., 40.);
    let bounds = panel(cx).unwrap();
    // The first row sits at the top of the panel, inside its padding.
    cx.simulate_click(bounds.origin + point(px(24.), px(18.)), Modifiers::none());
    frame(cx);
    assert!(
        seen.borrow().contains(&"action:copy".to_owned()),
        "{:?}",
        seen.borrow()
    );
    assert!(
        seen.borrow().contains(&"open:false".to_owned()),
        "{:?}",
        seen.borrow()
    );
    assert!(panel(cx).is_none());
}

#[gpui::test]
fn escape_and_outside_press_close(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, false);
    frame(cx);
    right_click(cx, 60., 40.);
    assert!(panel(cx).is_some());
    press(cx, "escape");
    frame(cx);
    assert!(panel(cx).is_none(), "Escape closes");

    right_click(cx, 60., 40.);
    assert!(panel(cx).is_some());
    cx.simulate_click(point(px(390.), px(290.)), Modifiers::none());
    frame(cx);
    assert!(panel(cx).is_none(), "an outside press closes");
    assert!(!seen.borrow().iter().any(|e| e.starts_with("action:")));
}

#[gpui::test]
fn disabled_ignores_the_secondary_press(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, true);
    frame(cx);
    right_click(cx, 60., 40.);
    assert!(panel(cx).is_none());
    assert!(seen.borrow().is_empty());
}

/// A context menu whose area holds one focusable child, for the keyboard
/// paths. Returns the child's focus handle.
fn keyboard_host(
    cx: &mut TestAppContext,
    disabled: bool,
) -> (Events, gpui::FocusHandle, &mut VisualTestContext) {
    still();
    let seen = events();
    let s = seen.clone();
    let child_focus = cx.update(|cx| cx.focus_handle());
    let focus = child_focus.clone();
    let cx = open_host(cx, move || {
        let (a, o) = (s.clone(), s.clone());
        // Inset from the window edge, past the positioner's edge margin.
        gpui::div()
            .p(px(40.))
            .child(
                ContextMenu::new(
                    "kctx",
                    gpui::div()
                        .w(px(400.))
                        .h(px(300.))
                        .p(px(40.))
                        .child(
                            gpui::div()
                                .id("kctx-child")
                                .track_focus(&focus)
                                .w(px(80.))
                                .h(px(24.))
                                .debug_selector(|| "kctx-child".into()),
                        )
                        .debug_selector(|| "kctx-area".into()),
                    vec![
                        MenuItem::new("copy", "Copy"),
                        MenuItem::new("paste", "Paste"),
                    ],
                )
                .is_disabled(disabled)
                .on_action(move |key, _, _| a.borrow_mut().push(format!("action:{key}")))
                .on_open_change(move |open, _, _| o.borrow_mut().push(format!("open:{open}"))),
            )
            .into_any_element()
    });
    (seen, child_focus, cx)
}

fn focus(cx: &mut VisualTestContext, handle: &gpui::FocusHandle) {
    let handle = handle.clone();
    cx.update(|window, cx| window.focus(&handle, cx));
    frame(cx);
}

#[gpui::test]
fn shift_f10_opens_at_the_area_with_the_first_item_focused(cx: &mut TestAppContext) {
    let (seen, child, cx) = keyboard_host(cx, false);
    frame(cx);
    focus(cx, &child);
    press(cx, "f10");
    frame(cx);
    assert!(panel(cx).is_none(), "F10 alone is not the context-menu key");

    press(cx, "shift-f10");
    frame(cx);
    let area = cx.debug_bounds("kctx-area").expect("area laid out");
    let bounds = panel(cx).expect("Shift+F10 opened the menu");
    assert_eq!(bounds.origin, area.origin, "anchored at the area's corner");
    assert_eq!(seen.borrow().as_slice(), ["open:true"]);

    // The first item holds the focus, so Enter chooses it without an arrow.
    press(cx, "enter");
    frame(cx);
    assert!(panel(cx).is_none());
    assert!(
        seen.borrow().contains(&"action:copy".to_owned()),
        "{:?}",
        seen.borrow()
    );
    let refocused = cx.update(|window, _| child.is_focused(window));
    assert!(refocused, "dismissal hands the focus back to the child");
}

#[gpui::test]
fn shift_f10_opens_below_a_focused_herogpui_button(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        gpui::div()
            .p(px(40.))
            .child(ContextMenu::new(
                "button-context-menu",
                gpui::div()
                    .w(px(400.))
                    .h(px(300.))
                    .p(px(40.))
                    .debug_selector(|| "button-context-area".into())
                    .child(Button::new("context-button").label("Actions").w(px(120.))),
                vec![MenuItem::new("copy", "Copy")],
            ))
            .into_any_element()
    });
    frame(cx);
    press(cx, "tab");
    frame(cx);
    press(cx, "shift-f10");
    frame(cx);
    let area = cx.debug_bounds("button-context-area").unwrap();
    let menu = panel(cx).expect("keyboard menu opened");
    assert_eq!(menu.left(), area.left() + px(40.));
    assert_eq!(menu.top(), area.top() + px(76.));
}

#[gpui::test]
fn shift_f10_opens_below_a_focused_breadcrumb(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        gpui::div()
            .p(px(40.))
            .child(ContextMenu::new(
                "crumb-context-menu",
                gpui::div()
                    .w(px(400.))
                    .h(px(300.))
                    .p(px(40.))
                    .debug_selector(|| "crumb-context-area".into())
                    .child(Breadcrumbs::new(vec![
                        Crumb::new("Build"),
                        Crumb::new("Live"),
                    ])),
                vec![MenuItem::new("copy", "Copy")],
            ))
            .into_any_element()
    });
    frame(cx);
    press(cx, "tab");
    frame(cx);
    press(cx, "shift-f10");
    frame(cx);
    let area = cx.debug_bounds("crumb-context-area").unwrap();
    let menu = panel(cx).expect("keyboard menu opened");
    assert_eq!(menu.left(), area.left() + px(40.));
    assert_eq!(menu.top(), area.top() + px(60.));
}

#[gpui::test]
fn shift_f10_opens_below_a_focused_color_area(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        gpui::div()
            .p(px(40.))
            .child(ContextMenu::new(
                "color-context-menu",
                gpui::div()
                    .w(px(400.))
                    .h(px(300.))
                    .p(px(40.))
                    .debug_selector(|| "color-context-area".into())
                    .child(
                        ColorArea::new("context-color", PickerColor::hsb(210., 0.5, 0.5))
                            .size(px(120.), px(80.)),
                    ),
                vec![MenuItem::new("copy", "Copy")],
            ))
            .into_any_element()
    });
    frame(cx);
    press(cx, "tab");
    frame(cx);
    press(cx, "shift-f10");
    frame(cx);
    let area = cx.debug_bounds("color-context-area").unwrap();
    let menu = panel(cx).expect("keyboard menu opened");
    assert_eq!(menu.left(), area.left() + px(40.));
    assert_eq!(menu.top(), area.top() + px(120.));
}

#[gpui::test]
fn shift_f10_opens_below_a_focused_menu_bar_trigger(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        gpui::div()
            .p(px(40.))
            .child(ContextMenu::new(
                "menu-bar-context-menu",
                gpui::div()
                    .w(px(400.))
                    .h(px(300.))
                    .p(px(40.))
                    .child(MenuBar::new(
                        "context-menu-bar",
                        vec![MenuBarMenu::new(
                            "file",
                            "File",
                            vec![MenuItem::new("new", "New")],
                        )],
                    )),
                vec![MenuItem::new("copy", "Copy")],
            ))
            .into_any_element()
    });
    frame(cx);
    press(cx, "tab");
    frame(cx);
    let trigger = cx.debug_bounds("menu-bar-trigger-file").unwrap();
    press(cx, "shift-f10");
    frame(cx);
    let menu = panel(cx).expect("keyboard menu opened");
    assert_eq!(menu.left(), trigger.left());
    assert_eq!(menu.top(), trigger.bottom());
}

#[gpui::test]
fn shift_f10_uses_the_active_list_row_instead_of_the_list_box(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        gpui::div()
            .p(px(40.))
            .child(ContextMenu::new(
                "list-context-menu",
                gpui::div()
                    .w(px(400.))
                    .h(px(300.))
                    .p(px(40.))
                    .debug_selector(|| "list-context-area".into())
                    .child(
                        ListBox::new(
                            "list-context-list",
                            vec![
                                ListBoxItem::new("first", "First"),
                                ListBoxItem::new("second", "Second"),
                            ],
                        )
                        .w(px(220.)),
                    ),
                vec![MenuItem::new("copy", "Copy")],
            ))
            .into_any_element()
    });
    frame(cx);
    let area = cx.debug_bounds("list-context-area").unwrap();
    cx.simulate_click(area.origin + point(px(70.), px(62.)), Modifiers::none());
    frame(cx);
    press(cx, "shift-f10");
    frame(cx);
    let area = cx.debug_bounds("list-context-area").unwrap();
    let menu = panel(cx).expect("keyboard menu opened");
    assert_eq!(menu.left(), area.left() + px(44.));
    assert_eq!(menu.top(), area.top() + px(80.));
}

#[gpui::test]
fn shift_f10_uses_the_active_tree_row(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        gpui::div()
            .p(px(40.))
            .child(ContextMenu::new(
                "tree-context-menu",
                gpui::div()
                    .w(px(400.))
                    .h(px(300.))
                    .p(px(40.))
                    .child(TreeView::new(
                        "context-tree",
                        vec![
                            TreeItem::new("first", "First"),
                            TreeItem::new("second", "Second"),
                        ],
                    )),
                vec![MenuItem::new("copy", "Copy")],
            ))
            .into_any_element()
    });
    frame(cx);
    press(cx, "tab");
    frame(cx);
    press(cx, "shift-f10");
    frame(cx);
    let row = cx.debug_bounds("context-tree-row-first").unwrap();
    let menu = panel(cx).expect("keyboard menu opened");
    assert_eq!(menu.left(), row.left());
    assert_eq!(menu.top(), row.bottom());
}

#[gpui::test]
fn shift_f10_uses_the_active_table_row(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        gpui::div()
            .p(px(40.))
            .child(ContextMenu::new(
                "table-context-menu",
                gpui::div().w(px(400.)).h(px(300.)).p(px(40.)).child(
                    Table::new(vec!["Name".into(), "Role".into()])
                        .id("context-table")
                        .row(vec![
                            gpui::div().child("Alice").into_any_element(),
                            gpui::div().child("Developer").into_any_element(),
                        ]),
                ),
                vec![MenuItem::new("copy", "Copy")],
            ))
            .into_any_element()
    });
    frame(cx);
    press(cx, "tab");
    press(cx, "down");
    frame(cx);
    press(cx, "shift-f10");
    frame(cx);
    let cell = cx.debug_bounds("table-row-track-0-0").unwrap();
    let menu = panel(cx).expect("keyboard menu opened");
    assert_eq!(menu.left(), cell.left());
    assert_eq!(menu.top(), cell.bottom());
}

#[gpui::test]
fn the_context_menu_key_opens_and_escape_restores_focus(cx: &mut TestAppContext) {
    let (seen, child, cx) = keyboard_host(cx, false);
    frame(cx);
    focus(cx, &child);
    press(cx, "menu");
    frame(cx);
    assert!(panel(cx).is_some(), "the ContextMenu key opens it");
    let in_child = cx.update(|window, _| child.is_focused(window));
    assert!(!in_child, "the menu took the focus");
    press(cx, "escape");
    frame(cx);
    assert!(panel(cx).is_none());
    let back = cx.update(|window, _| child.is_focused(window));
    assert!(back, "Escape hands the focus back");

    // The web spelling of the same key.
    press(cx, "contextmenu");
    frame(cx);
    assert!(panel(cx).is_some());
    assert_eq!(
        seen.borrow().as_slice(),
        ["open:true", "open:false", "open:true"]
    );
}

#[gpui::test]
fn modified_keys_do_not_open(cx: &mut TestAppContext) {
    let (_, child, cx) = keyboard_host(cx, false);
    frame(cx);
    focus(cx, &child);
    for keys in ["ctrl-shift-f10", "alt-shift-f10", "shift-menu"] {
        press(cx, keys);
        frame(cx);
        assert!(panel(cx).is_none(), "{keys} must not open the menu");
    }
}

#[gpui::test]
fn a_disabled_menu_ignores_the_keyboard(cx: &mut TestAppContext) {
    let (seen, child, cx) = keyboard_host(cx, true);
    frame(cx);
    focus(cx, &child);
    press(cx, "shift-f10");
    frame(cx);
    assert!(panel(cx).is_none(), "a disabled menu ignores the keyboard");
    assert!(seen.borrow().is_empty());
}

#[gpui::test]
fn a_primary_press_focuses_an_area_with_no_focusable_content(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, false);
    frame(cx);
    press(cx, "shift-f10");
    frame(cx);
    assert!(
        panel(cx).is_none(),
        "nothing inside the area holds the focus yet"
    );
    cx.simulate_click(point(px(100.), px(80.)), Modifiers::none());
    frame(cx);
    press(cx, "shift-f10");
    frame(cx);
    assert!(panel(cx).is_some(), "the press focused the area itself");
    assert_eq!(seen.borrow().as_slice(), ["open:true"]);
}

fn styled_host(cx: &mut TestAppContext, seen: Events) -> &mut VisualTestContext {
    still();
    open_host(cx, move || {
        let seen = seen.clone();
        ContextMenu::new(
            "styled-ctx",
            gpui::div().w(px(400.)).h(px(300.)),
            vec![MenuItem::new("copy", "C"), MenuItem::new("paste", "P")],
        )
        .recipe("accent")
        .item_content(|key, _| {
            let key = key.clone();
            gpui::div()
                .w(px(20.))
                .h(px(20.))
                .debug_selector(move || format!("label-{key}"))
                .into_any_element()
        })
        .item_start_content(move |key, state| {
            seen.borrow_mut()
                .push(format!("{key}:focused={}", state.is_focused));
            let key = key.clone();
            (key.as_ref() == "copy").then(|| {
                gpui::div()
                    .w(px(12.))
                    .h(px(12.))
                    .debug_selector(move || format!("start-{key}"))
                    .into_any_element()
            })
        })
        .into_any_element()
    })
}

#[gpui::test]
fn a_recipe_styles_the_context_menu_panel(cx: &mut TestAppContext) {
    use herogpui_theme::{set_theme, ComponentColor, ComponentTheme, ComponentThemes, MenuStyle};

    let cx = styled_host(cx, events());
    right_click(cx, 10., 10.);
    assert_eq!(
        panel(cx).expect("the menu opened").size.width,
        px(220.),
        "a recipe the theme does not define adds no override"
    );

    let hover = gpui::rgb(0x2255aa).into();
    cx.update(|window, cx| {
        set_theme(
            herogpui_theme::Theme::builder("app", herogpui_theme::Theme::light())
                .components(
                    ComponentThemes::default().menu(
                        ComponentTheme::default().recipe(
                            "accent",
                            MenuStyle::default()
                                .panel_min_width(px(128.))
                                .row_hover_bg(ComponentColor::Literal(hover)),
                        ),
                    ),
                )
                .build(),
            cx,
        );
        window.refresh();
    });
    frame(cx);
    assert_eq!(panel(cx).expect("still open").size.width, px(128.));

    let row = cx.debug_bounds("label-copy").expect("row laid out");
    cx.simulate_mouse_move(row.center(), None, Modifiers::none());
    let painted = harness::painted(cx);
    assert!(
        !painted.filled(hover).is_empty(),
        "the recipe's row hover fill reaches the hovered row"
    );
}

#[gpui::test]
fn item_start_content_leads_the_row_and_sees_its_state(cx: &mut TestAppContext) {
    let seen = events();
    let cx = styled_host(cx, seen.clone());
    right_click(cx, 10., 10.);

    let start = cx
        .debug_bounds("start-copy")
        .expect("leading element drawn");
    let label = cx.debug_bounds("label-copy").expect("label drawn");
    assert!(start.right() <= label.left(), "the element leads the label");
    assert!(start.top() >= label.top() && start.bottom() <= label.bottom() + px(4.));
    assert!(
        cx.debug_bounds("start-paste").is_none(),
        "a row whose closure returns None draws no leading element"
    );

    seen.borrow_mut().clear();
    press(cx, "down");
    frame(cx);
    assert!(
        seen.borrow()
            .iter()
            .any(|entry| entry == "copy:focused=true"),
        "the closure receives the row's focus: {:?}",
        seen.borrow()
    );
}
