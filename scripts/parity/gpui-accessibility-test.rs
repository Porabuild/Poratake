#[cfg(test)]
mod tests {
    use gpui::{
        Context, IntoElement, Render, TestAppContext, VisualTestContext, Window, accesskit,
        prelude::*,
    };
    use herogpui_components::{PickerItem, Select, Slider};
    use herogpui_theme::ThemeProvider;
    use std::{cell::RefCell, rc::Rc};

    struct Host(Rc<RefCell<Vec<&'static str>>>);

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let events = self.0.clone();
            gpui::div()
                .id("probe")
                .role(accesskit::Role::Button)
                .aria_label("Probe")
                .on_a11y_action(accesskit::Action::Expand, move |_, _, _| {
                    events.borrow_mut().push("expand");
                })
        }
    }

    #[gpui::test]
    fn dispatches_action_through_the_test_adapter(cx: &mut TestAppContext) {
        cx.update(ThemeProvider::init);
        let events = Rc::new(RefCell::new(Vec::new()));
        let observed = events.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Host(events));
        cx.activate_test_a11y();
        let tree = cx.test_a11y_tree().expect("accessibility tree");
        let (id, node) = tree
            .nodes
            .iter()
            .find(|(_, n)| n.label() == Some("Probe"))
            .expect("probe node");
        assert!(node.supports_action(accesskit::Action::Expand));
        cx.send_test_a11y_action(accesskit::ActionRequest {
            action: accesskit::Action::Expand,
            target_tree: accesskit::TreeId::ROOT,
            target_node: *id,
            data: None,
        });
        assert_eq!(observed.borrow().as_slice(), ["expand"]);
    }

    struct SelectHost(Rc<RefCell<Vec<bool>>>);
    impl Render for SelectHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let events = self.0.clone();
            Select::new("select", vec![PickerItem::new("a", "Alpha")])
                .aria_label("Probe select")
                .on_open_change(move |open, _, _| events.borrow_mut().push(*open))
        }
    }

    fn find_node(
        cx: &VisualTestContext,
        label: &str,
        role: accesskit::Role,
    ) -> (accesskit::NodeId, accesskit::Node) {
        let tree = cx.test_a11y_tree().expect("accessibility tree");
        tree.nodes
            .into_iter()
            .find(|(_, node)| node.label() == Some(label) && node.role() == role)
            .expect("named node")
    }

    #[gpui::test]
    fn select_accepts_expand_and_collapse_actions(cx: &mut TestAppContext) {
        cx.update(ThemeProvider::init);
        let events = Rc::new(RefCell::new(Vec::new()));
        let observed = events.clone();
        let (_, cx) = cx.add_window_view(move |_, _| SelectHost(events));
        cx.activate_test_a11y();
        let (id, node) = find_node(cx, "Probe select", accesskit::Role::Button);
        assert_eq!(node.is_expanded(), Some(false));
        assert!(
            node.supports_action(accesskit::Action::Expand),
            "select must advertise Expand"
        );
        cx.send_test_a11y_action(accesskit::ActionRequest {
            action: accesskit::Action::Expand,
            target_tree: accesskit::TreeId::ROOT,
            target_node: id,
            data: None,
        });
        assert_eq!(observed.borrow().as_slice(), [true]);
        let (id, node) = find_node(cx, "Probe select", accesskit::Role::Button);
        assert_eq!(node.is_expanded(), Some(true));
        assert!(node.supports_action(accesskit::Action::Collapse));
        cx.send_test_a11y_action(accesskit::ActionRequest {
            action: accesskit::Action::Collapse,
            target_tree: accesskit::TreeId::ROOT,
            target_node: id,
            data: None,
        });
        assert_eq!(observed.borrow().as_slice(), [true, false]);
    }

    struct SliderHost(Rc<RefCell<Vec<f32>>>);
    impl Render for SliderHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let events = self.0.clone();
            Slider::new("slider", 20.0)
                .aria_label("Probe slider")
                .min_value(0.0)
                .max_value(100.0)
                .step(5.0)
                .default_value(20.0)
                .on_change(move |value, _, _| events.borrow_mut().push(*value))
        }
    }

    #[gpui::test]
    fn slider_accepts_numeric_set_value(cx: &mut TestAppContext) {
        cx.update(ThemeProvider::init);
        let events = Rc::new(RefCell::new(Vec::new()));
        let observed = events.clone();
        let (_, cx) = cx.add_window_view(move |_, _| SliderHost(events));
        cx.activate_test_a11y();
        let (id, node) = find_node(cx, "Probe slider", accesskit::Role::Slider);
        assert!(
            node.supports_action(accesskit::Action::SetValue),
            "slider must advertise SetValue"
        );
        cx.send_test_a11y_action(accesskit::ActionRequest {
            action: accesskit::Action::SetValue,
            target_tree: accesskit::TreeId::ROOT,
            target_node: id,
            data: Some(accesskit::ActionData::NumericValue(37.0)),
        });
        assert_eq!(observed.borrow().as_slice(), [35.0]);
        let (_, node) = find_node(cx, "Probe slider", accesskit::Role::Slider);
        assert_eq!(node.numeric_value(), Some(35.0));
    }

    struct RangeHost(Rc<RefCell<Vec<Vec<f32>>>>);
    impl Render for RangeHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let events = self.0.clone();
            Slider::new("range", 20.0)
                .aria_label("Range slider")
                .min_value(0.0)
                .max_value(100.0)
                .step(5.0)
                .default_values([20.0, 80.0])
                .disabled_keys([1])
                .on_change_all(move |values, _, _| events.borrow_mut().push(values.to_vec()))
        }
    }

    fn send(
        cx: &mut VisualTestContext,
        id: accesskit::NodeId,
        action: accesskit::Action,
        data: Option<accesskit::ActionData>,
    ) {
        cx.send_test_a11y_action(accesskit::ActionRequest {
            action,
            target_tree: accesskit::TreeId::ROOT,
            target_node: id,
            data,
        });
    }

    #[gpui::test]
    fn range_slider_rejects_invalid_values_and_disabled_thumb(cx: &mut TestAppContext) {
        cx.update(ThemeProvider::init);
        let events = Rc::new(RefCell::new(Vec::new()));
        let observed = events.clone();
        let (_, cx) = cx.add_window_view(move |_, _| RangeHost(events));
        cx.activate_test_a11y();
        let tree = cx.test_a11y_tree().expect("accessibility tree");
        let mut nodes: Vec<_> = tree
            .nodes
            .into_iter()
            .filter(|(_, node)| {
                node.label() == Some("Range slider") && node.role() == accesskit::Role::Slider
            })
            .collect();
        nodes.sort_by(|a, b| {
            a.1.numeric_value()
                .partial_cmp(&b.1.numeric_value())
                .unwrap()
        });
        assert_eq!(nodes.len(), 2);
        let (first, first_node) = &nodes[0];
        let (second, second_node) = &nodes[1];
        assert!(first_node.supports_action(accesskit::Action::SetValue));
        assert!(!second_node.supports_action(accesskit::Action::SetValue));
        send(
            cx,
            *second,
            accesskit::Action::SetValue,
            Some(accesskit::ActionData::NumericValue(40.0)),
        );
        send(
            cx,
            *first,
            accesskit::Action::SetValue,
            Some(accesskit::ActionData::NumericValue(f64::NAN)),
        );
        send(
            cx,
            *first,
            accesskit::Action::SetValue,
            Some(accesskit::ActionData::NumericValue(f64::INFINITY)),
        );
        send(
            cx,
            *first,
            accesskit::Action::SetValue,
            Some(accesskit::ActionData::Value("bad".into())),
        );
        assert!(observed.borrow().is_empty());
        send(
            cx,
            *first,
            accesskit::Action::SetValue,
            Some(accesskit::ActionData::NumericValue(f64::MAX)),
        );
        assert_eq!(observed.borrow().as_slice(), [vec![80.0, 80.0]]);
    }

    struct StepsHost(Rc<RefCell<Vec<f32>>>);
    impl Render for StepsHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let events = self.0.clone();
            Slider::new("steps", 10.0)
                .aria_label("Step slider")
                .min_value(0.0)
                .max_value(100.0)
                .steps([10.0, 35.0, 80.0])
                .default_value(10.0)
                .on_change(move |value, _, _| events.borrow_mut().push(*value))
        }
    }

    #[gpui::test]
    fn slider_increment_and_decrement_visit_discrete_steps(cx: &mut TestAppContext) {
        cx.update(ThemeProvider::init);
        let events = Rc::new(RefCell::new(Vec::new()));
        let observed = events.clone();
        let (_, cx) = cx.add_window_view(move |_, _| StepsHost(events));
        cx.activate_test_a11y();
        let (id, node) = find_node(cx, "Step slider", accesskit::Role::Slider);
        assert!(node.supports_action(accesskit::Action::Increment));
        assert!(node.supports_action(accesskit::Action::Decrement));
        send(cx, id, accesskit::Action::Increment, None);
        send(cx, id, accesskit::Action::Increment, None);
        send(cx, id, accesskit::Action::Decrement, None);
        assert_eq!(observed.borrow().as_slice(), [35.0, 80.0, 35.0]);
    }

    struct DisabledSelectHost(Rc<RefCell<Vec<bool>>>);
    impl Render for DisabledSelectHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let events = self.0.clone();
            Select::new("disabled", vec![PickerItem::new("a", "Alpha")])
                .aria_label("Disabled select")
                .is_disabled(true)
                .on_open_change(move |open, _, _| events.borrow_mut().push(*open))
        }
    }

    #[gpui::test]
    fn disabled_select_does_not_accept_expand(cx: &mut TestAppContext) {
        cx.update(ThemeProvider::init);
        let events = Rc::new(RefCell::new(Vec::new()));
        let observed = events.clone();
        let (_, cx) = cx.add_window_view(move |_, _| DisabledSelectHost(events));
        cx.activate_test_a11y();
        let (id, node) = find_node(cx, "Disabled select", accesskit::Role::Button);
        assert!(!node.supports_action(accesskit::Action::Expand));
        send(cx, id, accesskit::Action::Expand, None);
        assert!(observed.borrow().is_empty());
    }

    struct ControlledHost {
        open: bool,
        value: f32,
        changes: Rc<RefCell<Vec<f32>>>,
        ends: Rc<RefCell<Vec<f32>>>,
    }

    impl Render for ControlledHost {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let select_host = cx.entity().downgrade();
            let slider_host = cx.entity().downgrade();
            let changes = self.changes.clone();
            let ends = self.ends.clone();
            gpui::div()
                .child(
                    Select::new("controlled-select", vec![PickerItem::new("a", "Alpha")])
                        .aria_label("Controlled select")
                        .is_open(self.open)
                        .on_open_change(move |open, _, cx| {
                            let _ = select_host.update(cx, |host, cx| {
                                host.open = *open;
                                cx.notify();
                            });
                        }),
                )
                .child(
                    Slider::new("controlled-slider", self.value)
                        .aria_label("Controlled slider")
                        .min_value(0.0)
                        .max_value(100.0)
                        .step(5.0)
                        .on_change(move |value, _, cx| {
                            changes.borrow_mut().push(*value);
                            let _ = slider_host.update(cx, |host, cx| {
                                host.value = *value;
                                cx.notify();
                            });
                        })
                        .on_change_end(move |value, _, _| ends.borrow_mut().push(*value)),
                )
        }
    }

    #[gpui::test]
    fn controlled_actions_update_the_parent_and_report_committed_values(cx: &mut TestAppContext) {
        cx.update(ThemeProvider::init);
        let changes = Rc::new(RefCell::new(Vec::new()));
        let ends = Rc::new(RefCell::new(Vec::new()));
        let observed_changes = changes.clone();
        let observed_ends = ends.clone();
        let (_, cx) = cx.add_window_view(move |_, _| ControlledHost {
            open: false,
            value: 20.0,
            changes,
            ends,
        });
        cx.activate_test_a11y();
        let (select, _) = find_node(cx, "Controlled select", accesskit::Role::Button);
        send(cx, select, accesskit::Action::Expand, None);
        let (_, node) = find_node(cx, "Controlled select", accesskit::Role::Button);
        assert_eq!(node.is_expanded(), Some(true));
        send(cx, select, accesskit::Action::Collapse, None);
        let (_, node) = find_node(cx, "Controlled select", accesskit::Role::Button);
        assert_eq!(node.is_expanded(), Some(false));
        let (slider, _) = find_node(cx, "Controlled slider", accesskit::Role::Slider);
        send(
            cx,
            slider,
            accesskit::Action::SetValue,
            Some(accesskit::ActionData::NumericValue(37.0)),
        );
        let (_, node) = find_node(cx, "Controlled slider", accesskit::Role::Slider);
        assert_eq!(node.numeric_value(), Some(35.0));
        send(
            cx,
            slider,
            accesskit::Action::SetValue,
            Some(accesskit::ActionData::NumericValue(-50.0)),
        );
        assert_eq!(observed_changes.borrow().as_slice(), [35.0, 0.0]);
        assert_eq!(observed_ends.borrow().as_slice(), [35.0, 0.0]);
    }

    struct InvalidBoundsHost {
        min: f32,
        max: f32,
        changes: Rc<RefCell<Vec<f32>>>,
        ends: Rc<RefCell<Vec<f32>>>,
    }

    impl Render for InvalidBoundsHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            let ends = self.ends.clone();
            Slider::new("invalid", 20.0)
                .aria_label("Invalid bounds")
                .min_value(self.min)
                .max_value(self.max)
                .default_value(20.0)
                .on_change(move |value, _, _| changes.borrow_mut().push(*value))
                .on_change_end(move |value, _, _| ends.borrow_mut().push(*value))
        }
    }

    #[gpui::test]
    fn slider_ignores_actions_for_invalid_bounds(cx: &mut TestAppContext) {
        cx.update(ThemeProvider::init);
        for (min, max) in [
            (100.0, 0.0),
            (f32::NAN, 100.0),
            (0.0, f32::NAN),
            (f32::NEG_INFINITY, 100.0),
            (0.0, f32::INFINITY),
        ] {
            let changes = Rc::new(RefCell::new(Vec::new()));
            let ends = Rc::new(RefCell::new(Vec::new()));
            let observed_changes = changes.clone();
            let observed_ends = ends.clone();
            let (_, cx) = cx.add_window_view(move |_, _| InvalidBoundsHost {
                min,
                max,
                changes,
                ends,
            });
            cx.activate_test_a11y();
            let (id, _) = find_node(cx, "Invalid bounds", accesskit::Role::Slider);
            send(
                cx,
                id,
                accesskit::Action::SetValue,
                Some(accesskit::ActionData::NumericValue(50.0)),
            );
            send(cx, id, accesskit::Action::Increment, None);
            send(cx, id, accesskit::Action::Decrement, None);
            assert!(observed_changes.borrow().is_empty());
            assert!(observed_ends.borrow().is_empty());
        }
    }
}
