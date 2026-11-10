//! Probe the keyed selection axis directly, including a positive allocation control.
use crate::harness;

use gpui::{prelude::*, AnyElement, SharedString, TestAppContext};
use harness::open_host;
use herogpui_components::{PickerItem, Select, Slider};
use herogpui_core::{element_id, SelectionMode};
use std::{cell::Cell, rc::Rc};

fn axis_probe<C: 'static, T: Default + 'static>(
    id: &'static str,
    axis: &'static str,
    created: Rc<Cell<Option<bool>>>,
) -> AnyElement {
    gpui::canvas(
        move |_, window, cx| {
            if created.get().is_some() {
                return;
            }
            created.set(Some(false));
            window.with_id(std::any::type_name::<C>(), |window| {
                window.use_keyed_state(element_id::scoped(&id.into(), axis), cx, |_, _| {
                    created.set(Some(true));
                    T::default()
                });
            });
        },
        |_, _, _, _| {},
    )
    .size_0()
    .into_any_element()
}

#[gpui::test]
fn slider_allocates_only_active_uncontrolled_axis(cx: &mut TestAppContext) {
    let unused = Rc::new(Cell::new(None));
    let active = Rc::new(Cell::new(None));
    let (u, a) = (unused.clone(), active.clone());
    let cx = open_host(cx, move || {
        gpui::div()
            .child(
                Slider::new("controlled-single", 0.5)
                    .max_value(1.)
                    .step(0.01),
            )
            .child(axis_probe::<Slider, Vec<f32>>(
                "controlled-single",
                "values",
                u.clone(),
            ))
            .child(
                Slider::new("uncontrolled-range", 0.)
                    .default_values([0.2, 0.8])
                    .max_value(1.)
                    .step(0.01),
            )
            .child(axis_probe::<Slider, Vec<f32>>(
                "uncontrolled-range",
                "values",
                a.clone(),
            ))
            .into_any_element()
    });
    cx.update(|window, _| window.refresh());
    assert_eq!(
        active.get(),
        Some(false),
        "positive control: range owns its values state"
    );
    assert_eq!(
        unused.get(),
        Some(true),
        "single thumb must leave range state unallocated"
    );
}

#[gpui::test]
fn select_allocates_only_active_selection_axis(cx: &mut TestAppContext) {
    let unused_multi = Rc::new(Cell::new(None));
    let unused_single = Rc::new(Cell::new(None));
    let active_multi = Rc::new(Cell::new(None));
    let active_single = Rc::new(Cell::new(None));
    let (um, us, am, asingle) = (
        unused_multi.clone(),
        unused_single.clone(),
        active_multi.clone(),
        active_single.clone(),
    );
    let cx = open_host(cx, move || {
        let items = || vec![PickerItem::new("a", "A")];
        gpui::div()
            .child(Select::new("controlled-single", items()).value(Some("a".into())))
            .child(axis_probe::<Select, Vec<SharedString>>(
                "controlled-single",
                "values",
                um.clone(),
            ))
            .child(
                Select::new("controlled-multi", items())
                    .selection_mode(SelectionMode::Multiple)
                    .selected_keys(["a".into()]),
            )
            .child(axis_probe::<Select, Option<SharedString>>(
                "controlled-multi",
                "value",
                us.clone(),
            ))
            .child(Select::new("uncontrolled-single", items()).default_value(Some("a".into())))
            .child(axis_probe::<Select, Option<SharedString>>(
                "uncontrolled-single",
                "value",
                asingle.clone(),
            ))
            .child(
                Select::new("uncontrolled-multi", items())
                    .selection_mode(SelectionMode::Multiple)
                    .default_selected_keys(["a".into()]),
            )
            .child(axis_probe::<Select, Vec<SharedString>>(
                "uncontrolled-multi",
                "values",
                am.clone(),
            ))
            .into_any_element()
    });
    cx.update(|window, _| window.refresh());
    assert_eq!(
        active_single.get(),
        Some(false),
        "positive control: single owns its value state"
    );
    assert_eq!(
        active_multi.get(),
        Some(false),
        "positive control: multi owns its values state"
    );
    assert_eq!(
        (unused_multi.get(), unused_single.get()),
        (Some(true), Some(true)),
        "single and multi must leave their inactive axis unallocated"
    );
}
