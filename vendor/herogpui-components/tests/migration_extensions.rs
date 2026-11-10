//! Native application composition: seek gestures, compact pickers and standalone menus.
use crate::harness;

use gpui::{
    point, prelude::*, px, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    TestAppContext,
};
use harness::{events, open_host, press};
use herogpui_components::{Menu, MenuItem, PickerItem, Select, Slider};
use std::{cell::RefCell, rc::Rc};

#[gpui::test]
fn continuous_seek_brackets_changes_and_release_outside_once(cx: &mut TestAppContext) {
    let seen = events();
    let view_seen = seen.clone();
    let value = Rc::new(RefCell::new(0.5));
    let cx = open_host(cx, move || {
        let (start, change, commit, end) = (
            view_seen.clone(),
            view_seen.clone(),
            view_seen.clone(),
            view_seen.clone(),
        );
        let current = *value.borrow();
        let value = value.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("seek", current)
                    .max_value(1.)
                    .continuous(true)
                    .on_drag_start(move |_, _| start.borrow_mut().push("start".into()))
                    .on_change(move |v, window, _| {
                        *value.borrow_mut() = *v;
                        change.borrow_mut().push(format!("value:{v:.3}"));
                        window.refresh();
                    })
                    .on_change_end(move |v, _, _| {
                        commit.borrow_mut().push(format!("commit:{v:.3}"));
                    })
                    .on_drag_end(move |_, _| end.borrow_mut().push("end".into())),
            )
            .into_any_element()
    });
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(123.), px(10.)),
        ..Default::default()
    });
    cx.update(|window, _| window.refresh());
    cx.simulate_event(MouseMoveEvent {
        pressed_button: Some(MouseButton::Left),
        position: point(px(333.), px(80.)),
        ..Default::default()
    });
    cx.update(|window, _| window.refresh());
    for _ in 0..2 {
        cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position: point(px(333.), px(80.)),
            ..Default::default()
        });
        cx.update(|window, _| window.refresh());
    }
    assert_eq!(
        &*seen.borrow(),
        &["start", "value:0.205", "value:0.555", "commit:0.555", "end"]
    );
    seen.borrow_mut().clear();
    press(cx, "right");
    assert_eq!(&*seen.borrow(), &["value:0.565", "commit:0.565"]);
}

#[gpui::test]
fn arbitrary_steps_sort_filter_and_keyboard_visits_adjacent_points(cx: &mut TestAppContext) {
    let seen = events();
    let view_seen = seen.clone();
    let cx = open_host(cx, move || {
        let seen = view_seen.clone();
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("presets", 0.0)
                    .default_value(0.)
                    .max_value(1.)
                    .steps([1., 0.1, f32::NAN, -2., 0.7, 0.1, 0.])
                    .on_change(move |v, _, _| seen.borrow_mut().push(format!("{v:.1}"))),
            )
            .into_any_element()
    });
    cx.simulate_click(point(px(330.), px(10.)), Modifiers::none());
    cx.update(|window, _| window.refresh());
    press(cx, "left");
    press(cx, "right");
    press(cx, "end");
    press(cx, "home");
    assert_eq!(&*seen.borrow(), &["0.7", "0.1", "0.7", "1.0", "0.0"]);
}

#[gpui::test]
fn standalone_menu_focus_compact_geometry_and_submenu_dismissal(cx: &mut TestAppContext) {
    let focus = cx.update(|cx| cx.focus_handle());
    let supplied = focus.clone();
    let seen = events();
    let view_seen = seen.clone();
    let cx = open_host(cx, move || {
        let seen = view_seen.clone();
        Menu::new(
            "tray",
            vec![MenuItem::new("parent", "Parent").submenu(vec![MenuItem::new("child", "Child")])],
        )
        .focus_handle(supplied.clone())
        .animate_entry(false)
        .panel_min_width(px(180.))
        .panel_max_width(px(180.))
        .panel_max_height(px(120.))
        .row_height(px(28.))
        .row_padding_x(px(8.))
        .row_padding_y(px(2.))
        .row_text_size(px(12.))
        .row_gap(px(8.))
        .panel_padding(px(4.))
        .item_content(|key, _| {
            let key = key.clone();
            gpui::div()
                .h(px(20.))
                .w_full()
                .debug_selector(move || format!("label-{key}"))
                .child("Label")
                .into_any_element()
        })
        .on_dismiss(move |refocus, _, _| seen.borrow_mut().push(format!("dismiss:{refocus}")))
        .into_any_element()
    });
    assert!(cx.update(|window, _| focus.is_focused(window)));
    let row = cx.debug_bounds("label-parent").unwrap();
    assert_eq!(row.origin.y, px(8.));
    assert_eq!(row.origin.x, px(12.));
    press(cx, "down");
    press(cx, "right");
    assert!(!cx.update(|window, _| focus.is_focused(window)));
    let child = cx.debug_bounds("label-child").unwrap();
    assert_eq!(child.size.height, px(20.));
    press(cx, "left");
    assert!(cx.update(|window, _| focus.is_focused(window)));
    press(cx, "escape");
    assert_eq!(&*seen.borrow(), &["dismiss:true"]);
}

#[gpui::test]
fn disabling_during_seek_keeps_release_cleanup_and_blocks_movement(cx: &mut TestAppContext) {
    let disabled = Rc::new(std::cell::Cell::new(false));
    let for_view = disabled.clone();
    let seen = events();
    let view_seen = seen.clone();
    let cx = open_host(cx, move || {
        let (start, change, end) = (view_seen.clone(), view_seen.clone(), view_seen.clone());
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("disable-seek", 0.5)
                    .max_value(1.)
                    .continuous(true)
                    .is_disabled(for_view.get())
                    .on_drag_start(move |_, _| start.borrow_mut().push("start".into()))
                    .on_change(move |_, _, _| change.borrow_mut().push("change".into()))
                    .on_drag_end(move |_, _| end.borrow_mut().push("end".into())),
            )
            .into_any_element()
    });
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: point(px(300.), px(10.)),
        ..Default::default()
    });
    disabled.set(true);
    cx.update(|window, _| window.refresh());
    cx.simulate_event(MouseMoveEvent {
        pressed_button: Some(MouseButton::Left),
        position: point(px(500.), px(80.)),
        ..Default::default()
    });
    cx.update(|window, _| window.refresh());
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: point(px(500.), px(80.)),
        ..Default::default()
    });
    cx.update(|window, _| window.refresh());
    assert_eq!(&*seen.borrow(), &["start", "end"]);
}

#[gpui::test]
fn selected_menu_highlight_keeps_paired_foreground(cx: &mut TestAppContext) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let view_seen = seen.clone();
    let highlight = gpui::rgb(0xffcc11).into();
    let cx = open_host(cx, move || {
        let seen = view_seen.clone();
        Menu::new("highlight", vec![MenuItem::new("selected", "Selected")])
            .selected_key("selected")
            .animate_entry(false)
            .row_hover_bg(gpui::rgb(0x000044))
            .row_hover_foreground(highlight)
            .row_text_size(px(12.))
            .item_content(move |_, _| {
                let seen = seen.clone();
                gpui::canvas(
                    move |_, window, _| {
                        seen.borrow_mut().push(window.text_style());
                    },
                    |_, _, _, _| {},
                )
                .w(px(30.))
                .h(px(20.))
                .into_any_element()
            })
            .into_any_element()
    });
    press(cx, "down");
    let last = seen.borrow().last().unwrap().clone();
    assert_eq!(last.color, highlight);
    assert_eq!(last.font_size.to_pixels(px(16.)), px(12.));
}

#[gpui::test]
fn compact_select_trigger_and_panel_use_configured_parts(cx: &mut TestAppContext) {
    harness::still();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let view_seen = seen.clone();
    let cx = open_host(cx, move || {
        let seen = view_seen.clone();
        Select::new("compact", vec![PickerItem::new("a", "Alpha")])
            .value(Some("a".into()))
            .is_open(true)
            .trigger_text_size(px(12.))
            .row_text_size(px(12.))
            .panel_padding(px(4.))
            .row_padding_x(px(8.))
            .row_padding_y(px(2.))
            .row_height(px(28.))
            .value_content(move |_| {
                let seen = seen.clone();
                gpui::canvas(
                    move |_, window, _| {
                        seen.borrow_mut().push(window.text_style());
                    },
                    |_, _, _, _| {},
                )
                .w(px(30.))
                .h(px(20.))
                .into_any_element()
            })
            .into_any_element()
    });
    let last = seen.borrow().last().unwrap().clone();
    assert_eq!(last.font_size.to_pixels(px(16.)), px(12.));
    let panel = cx
        .debug_bounds("select-list-Name(\"compact\")-panel")
        .unwrap();
    let row = cx
        .debug_bounds("select-list-Name(\"compact\")-opt-0")
        .unwrap();
    assert_eq!(row.origin.x - panel.origin.x, px(4.));
    assert_eq!(row.origin.y - panel.origin.y, px(4.));
    assert_eq!(row.size.height, px(28.));
}

#[gpui::test]
fn standalone_menu_panel_gap_controls_row_interval_and_total_height(cx: &mut TestAppContext) {
    for gap in [None, Some(px(0.))] {
        let cx = open_host(cx, move || {
            let menu = Menu::new(
                "panel-gap",
                vec![
                    MenuItem::new("first", "First"),
                    MenuItem::new("second", "Second"),
                ],
            )
            .animate_entry(false)
            .panel_padding(px(4.))
            .row_height(px(28.))
            .row_padding_y(px(2.))
            .item_content(|key, _| {
                let key = key.clone();
                gpui::div()
                    .h(px(20.))
                    .w(px(40.))
                    .debug_selector(move || format!("gap-{key}"))
                    .into_any_element()
            });
            gpui::div()
                .debug_selector(|| "gap-panel".into())
                .child(match gap {
                    Some(gap) => menu.panel_gap(gap),
                    None => menu,
                })
                .into_any_element()
        });
        let first = cx.debug_bounds("gap-first").expect("first row");
        let second = cx.debug_bounds("gap-second").expect("second row");
        let panel = cx.debug_bounds("gap-panel").expect("panel");
        let gap = gap.unwrap_or(px(2.));
        assert_eq!(second.origin.y - first.origin.y, px(28.) + gap);
        assert_eq!(panel.size.height, px(64.) + gap);
    }
}
