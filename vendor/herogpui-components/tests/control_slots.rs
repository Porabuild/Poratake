use crate::harness;
use gpui::{prelude::*, px, TestAppContext};
use herogpui_components::{ColorSwatchPicker, PickerColor, PickerItem, Select, SizeXl};
use std::{cell::RefCell, rc::Rc};

#[gpui::test]
fn custom_select_rows_keep_label_typeahead_and_selection(cx: &mut TestAppContext) {
    harness::still();
    let selected = Rc::new(RefCell::new(Vec::new()));
    let sink = selected.clone();
    let cx = harness::open_host(cx, move || {
        let sink = sink.clone();
        Select::new(
            "custom",
            vec![
                PickerItem::new("fine", "Fine"),
                PickerItem::new("heavy", "Heavy"),
                PickerItem::new("hidden", "Hidden"),
            ],
        )
        .disabled_keys(["hidden".into()])
        .item_leading(|_, _| panic!("full content replaces leading content"))
        .item_content(|key, _, _, _| {
            let key = key.clone();
            gpui::div()
                .size(px(16.))
                .debug_selector(move || format!("custom-{key}"))
                .into_any_element()
        })
        .on_selection_change(move |value, _, _| sink.borrow_mut().push(value.clone()))
        .into_any_element()
    });
    harness::click(cx, 20., 18.);
    assert!(cx.debug_bounds("custom-fine").is_some());
    assert!(cx.debug_bounds("custom-heavy").is_some());
    harness::press(cx, "h");
    harness::press(cx, "enter");
    assert_eq!(&*selected.borrow(), &[Some("heavy".into())]);
}

#[gpui::test]
fn select_gap_spaces_natural_and_virtual_option_content(cx: &mut TestAppContext) {
    harness::still();
    let cx = harness::open_host(cx, || {
        gpui::div()
            .flex()
            .gap(px(30.))
            .children([false, true].into_iter().map(|virtualized| {
                let id = if virtualized { "virtual" } else { "natural" };
                Select::new(
                    id,
                    vec![PickerItem::new("one", "One"), PickerItem::new("two", "Two")],
                )
                .is_open(true)
                .panel_gap(px(4.))
                .when(virtualized, |select| select.row_height(px(36.)))
                .item_content(move |key, _, _, _| {
                    let name = format!("{id}-{key}");
                    gpui::div()
                        .size(px(16.))
                        .debug_selector(move || name)
                        .into_any_element()
                })
            }))
            .into_any_element()
    });
    for (first, second) in [
        ("natural-one", "natural-two"),
        ("virtual-one", "virtual-two"),
    ] {
        let a = cx.debug_bounds(first).unwrap();
        let b = cx.debug_bounds(second).unwrap();
        assert!(
            (f32::from(b.origin.y - a.origin.y) - 40.).abs() < 0.01,
            "{first}: {a:?} -> {b:?}"
        );
    }
}

#[gpui::test]
fn swatch_sx_fits_one_row_and_keeps_horizontal_keyboard_navigation(cx: &mut TestAppContext) {
    harness::still();
    let selected = Rc::new(RefCell::new(Vec::new()));
    let sink = selected.clone();
    let cx = harness::open_host(cx, move || {
        let sink = sink.clone();
        ColorSwatchPicker::new(
            "compact",
            (0..12)
                .map(|i| PickerColor::hsb(i as f32 * 30., 1., 1.))
                .collect(),
        )
        .size(SizeXl::Xs)
        .sx(|el| el.w(px(230.)).gap(px(4.)).flex_nowrap().justify_between())
        .item_content(|i, _| {
            gpui::div()
                .size_full()
                .debug_selector(move || format!("color-{i}"))
                .into_any_element()
        })
        .on_change(move |value, _, _| sink.borrow_mut().push(value.to_hex()))
        .into_any_element()
    });
    let first = cx.debug_bounds("color-0").unwrap();
    let last = cx.debug_bounds("color-11").unwrap();
    assert_eq!(first.origin.y, last.origin.y);
    assert!(last.right() <= px(230.));
    harness::click(cx, f32::from(first.center().x), f32::from(first.center().y));
    harness::press(cx, "down");
    assert_eq!(selected.borrow().len(), 1, "one row has no lower neighbour");
    harness::press(cx, "right");
    harness::press(cx, "space");
    assert_eq!(
        selected.borrow().last(),
        Some(&PickerColor::hsb(30., 1., 1.).to_hex())
    );
}

#[gpui::test]
fn wrapped_swatch_navigation_tracks_padding_and_resize(cx: &mut TestAppContext) {
    harness::still();
    let width = Rc::new(std::cell::Cell::new(70.));
    let size = width.clone();
    let selected = Rc::new(RefCell::new(Vec::new()));
    let sink = selected.clone();
    let cx = harness::open_host(cx, move || {
        let sink = sink.clone();
        ColorSwatchPicker::new(
            "wrapped",
            (0..12)
                .map(|i| PickerColor::hsb(i as f32 * 30., 1., 1.))
                .collect(),
        )
        .size(SizeXl::Xs)
        .sx(|el| el.w(px(size.get())).p(px(6.)).gap(px(4.)))
        .item_content(|i, _| {
            gpui::div()
                .size_full()
                .debug_selector(move || format!("wrapped-{i}"))
                .into_any_element()
        })
        .on_change(move |value, _, _| sink.borrow_mut().push(value.to_hex()))
        .into_any_element()
    });
    for (width_value, expected) in [(70., 3), (50., 2)] {
        width.set(width_value);
        cx.update(|window, _| window.refresh());
        let at = cx.debug_bounds("wrapped-0").unwrap().center();
        harness::click(cx, f32::from(at.x), f32::from(at.y));
        harness::press(cx, "down");
        harness::press(cx, "space");
        assert_eq!(
            selected.borrow().last(),
            Some(&PickerColor::hsb(expected as f32 * 30., 1., 1.).to_hex())
        );
        harness::press(cx, "up");
        harness::press(cx, "space");
        assert_eq!(
            selected.borrow().last(),
            Some(&PickerColor::hsb(0., 1., 1.).to_hex())
        );
    }
}

#[gpui::test]
fn virtual_select_gap_reveals_and_selects_the_final_row(cx: &mut TestAppContext) {
    harness::still();
    let selected = Rc::new(RefCell::new(Vec::new()));
    let sink = selected.clone();
    let cx = harness::open_host(cx, move || {
        let sink = sink.clone();
        Select::new(
            "long-gap",
            (0..80)
                .map(|i| PickerItem::new(i.to_string(), format!("Option {i}")))
                .collect(),
        )
        .row_height(px(36.))
        .panel_gap(px(4.))
        .item_content(|key, _, _, _| {
            let name = format!("long-gap-{key}");
            gpui::div()
                .size(px(16.))
                .debug_selector(move || name)
                .into_any_element()
        })
        .on_selection_change(move |value, _, _| sink.borrow_mut().push(value.clone()))
        .into_any_element()
    });
    harness::click(cx, 20., 18.);
    harness::press(cx, "end");
    harness::settle(cx);
    let last = cx
        .debug_bounds("long-gap-79")
        .expect("the final row must be revealed");
    let panel = cx
        .debug_bounds("select-list-Name(\"long-gap\")-panel")
        .unwrap();
    assert!(last.top() >= panel.top() && last.bottom() <= panel.bottom());
    harness::click(cx, f32::from(last.center().x), f32::from(last.center().y));
    assert_eq!(&*selected.borrow(), &[Some("79".into())]);
}

#[gpui::test]
fn icon_select_popover_respects_minimum_width_and_centred_placement(cx: &mut TestAppContext) {
    harness::still();
    let cx = harness::open_host(cx, || {
        gpui::div()
            .flex()
            .pl(px(100.))
            .child(
                Select::new("narrow", vec![PickerItem::new("one", "One")])
                    .height(px(28.))
                    .padding_x(px(8.))
                    .padding_y(px(0.))
                    .is_open(true)
                    .panel_min_width(px(64.))
                    .placement(herogpui_core::Placement::Bottom)
                    .value_content(|_| gpui::div().size(px(16.)).into_any_element())
                    .item_content(|_, _, _, _| gpui::div().size(px(16.)).into_any_element()),
            )
            .into_any_element()
    });
    let panel = cx
        .debug_bounds("select-list-Name(\"narrow\")-panel")
        .unwrap();
    let trigger = cx.debug_bounds("select-trigger-Name(\"narrow\")").unwrap();
    assert_eq!(panel.size.width, px(64.));
    assert!((f32::from(panel.center().x - trigger.center().x)).abs() < 0.01);
}

#[gpui::test]
fn popover_panel_width_and_padding_constrain_its_content(cx: &mut TestAppContext) {
    use herogpui_components::{Button, Popover};
    harness::still();
    let cx = harness::open_host(cx, || {
        Popover::new(Button::new("width-trigger").label("Options"))
            .id("width-popover")
            .is_open(true)
            .aria_label("Options")
            .panel_width(px(192.))
            .padding(px(6.))
            .child(
                gpui::div()
                    .w_full()
                    .h(px(20.))
                    .debug_selector(|| "popover-width-body".into()),
            )
            .into_any_element()
    });
    let body = cx.debug_bounds("popover-width-body").unwrap();
    assert!((f32::from(body.size.width) - 180.).abs() < 0.1, "{body:?}");
}

#[gpui::test]
fn popover_panel_width_stays_inside_a_narrow_viewport(cx: &mut TestAppContext) {
    use herogpui_components::{Button, Popover};
    harness::still();
    let cx = harness::open_host(cx, || {
        Popover::new(Button::new("narrow-trigger").label("Options"))
            .id("narrow-popover")
            .is_open(true)
            .panel_width(px(500.))
            .padding(px(6.))
            .child(
                gpui::div()
                    .w_full()
                    .h(px(20.))
                    .debug_selector(|| "narrow-popover-body".into()),
            )
            .into_any_element()
    });
    cx.simulate_resize(gpui::size(px(240.), px(300.)));
    harness::settle(cx);
    let body = cx.debug_bounds("narrow-popover-body").unwrap();
    assert_eq!(body.size.width, px(228.), "{body:?}");
    assert!(
        body.left() >= px(6.) && body.right() <= px(234.),
        "{body:?}"
    );
    cx.simulate_resize(gpui::size(px(640.), px(300.)));
    harness::settle(cx);
    assert_eq!(
        cx.debug_bounds("narrow-popover-body").unwrap().size.width,
        px(488.)
    );
}

#[gpui::test]
fn modal_panel_width_and_padding_constrain_its_content(cx: &mut TestAppContext) {
    use herogpui_components::Modal;
    harness::still();
    let cx = harness::open_host(cx, || {
        Modal::new()
            .id("width-modal")
            .is_open(true)
            .title("Edit data")
            .panel_width(px(672.))
            .padding(px(16.))
            .child(
                gpui::div()
                    .w_full()
                    .h(px(20.))
                    .debug_selector(|| "modal-width-body".into()),
            )
            .into_any_element()
    });
    let body = cx.debug_bounds("modal-width-body").unwrap();
    assert!((f32::from(body.size.width) - 640.).abs() < 0.1, "{body:?}");
}

#[gpui::test]
fn scaled_switch_keeps_its_layout_and_activation_target(cx: &mut TestAppContext) {
    use herogpui_components::{Size, Switch};
    use herogpui_theme::ActiveTheme;
    harness::still();
    let changes = Rc::new(RefCell::new(Vec::new()));
    let recorded = changes.clone();
    let cx = harness::open_host(cx, move || {
        let changes = changes.clone();
        gpui::div()
            .flex()
            .items_start()
            .child(
                Switch::new("scaled-switch")
                    .size(Size::Sm)
                    .aria_label("Background")
                    .control_scale(0.75)
                    .is_selected(true)
                    .on_change(move |value, _, _| changes.borrow_mut().push(*value)),
            )
            .into_any_element()
    });
    let accent = cx.update(|_, cx| cx.colors().accent.color);
    let scene = harness::painted(cx);
    assert!(
        scene
            .filled(accent)
            .iter()
            .any(|quad| scene.bounds(quad).size == gpui::size(px(24.), px(12.))),
        "{}",
        scene.describe()
    );
    harness::click(cx, 1., 8.);
    assert_eq!(&*recorded.borrow(), &[false]);
}
