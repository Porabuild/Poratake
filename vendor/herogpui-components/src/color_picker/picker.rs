//! ColorPicker.

use super::*;

// ColorPicker
// ---------------------------------------------------------------------------

/// ColorPicker — a swatch trigger plus a full picking surface.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ColorPicker {
    /// `defaultValue` — set it to hand this component its own state.
    default_value: Option<PickerColor>,
    id: ElementId,
    value: PickerColor,
    label: Option<SharedString>,
    is_open: Option<bool>,
    placement: Placement,
    /// Adds an alpha slider under the hue slider.
    show_alpha: bool,
    is_disabled: bool,
    on_change: Option<OnColorChange>,
    on_open_change: Option<Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    /// The family the hex readout is drawn with; unset keeps the mono token.
    font_family: Option<SharedString>,
}

impl ColorPicker {
    /// Creates a color picker with the id `id`, showing `value`.
    pub fn new(id: impl Into<ElementId>, value: PickerColor) -> Self {
        Self {
            default_value: None,
            id: id.into(),
            value,
            label: None,
            is_open: None,
            placement: Placement::BottomStart,
            show_alpha: false,
            is_disabled: false,
            on_change: None,
            on_open_change: None,
            font_family: None,
        }
    }

    /// The family the hex readout is drawn with; unset keeps the mono token.
    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// `defaultValue` — the uncontrolled initial colour.
    ///
    /// Supplying it hands the component its own state: the constructor's
    /// `value` becomes the seed, and a change moves the component's copy.
    pub fn default_value(mut self, value: PickerColor) -> Self {
        self.default_value = Some(value);
        self
    }

    /// Sets the label shown above the picker.
    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// `placement` on `ColorPicker.Popover`.
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Sets the controlled open state of the popover (`isOpen`).
    pub fn is_open(mut self, v: bool) -> Self {
        self.is_open = Some(v);
        self
    }

    /// Sets whether an alpha slider is shown under the hue slider.
    pub fn show_alpha(mut self, v: bool) -> Self {
        self.show_alpha = v;
        self
    }

    /// Sets whether the picker is disabled (`isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Sets the handler called with the new color (`onChange`).
    pub fn on_change(
        mut self,
        handler: impl Fn(&PickerColor, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }

    /// Sets the handler called when the popover opens or closes (`onOpenChange`).
    pub fn on_open_change(
        mut self,
        handler: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for ColorPicker {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (is_open, open_own) = util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "picker-open"),
            self.is_open,
            false,
        );
        // `defaultValue` opts into the component holding its own colour;
        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        let (resolved, own) = util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "picker-value"),
            match self.default_value {
                Some(_) => None,
                None => Some(self.value),
            },
            self.default_value.unwrap_or(self.value),
        );
        self.value = resolved;
        // `.color-picker__trigger:focus-visible` is `status-focused`.
        // `use_keyed_state` takes `cx` mutably, so the handle precedes the theme.
        let trigger_focus =
            util::tab_stop_handle(element_id::scoped(&self.id, "picker-focus"), window, cx);
        // Only the `debug_selector` spelling the smoke tests query on; every
        // element id below scopes off `self.id` structurally instead.
        let base = format!("{:?}", self.id);
        let overlay_open = is_open && !self.is_disabled;
        let (phase, dismissal_token) = util::overlay_scope(
            window,
            cx,
            element_id::scoped(&self.id, "picker-overlay"),
            overlay_open,
            true,
        );
        let exiting = phase == util::OverlayPhase::Exiting;
        // Blur closes the logical open state without pulling focus back to the
        // trigger. Exit animation frames do not keep this watch armed.
        let group_scope = util::close_on_blur(window, cx, &self.id, overlay_open, {
            let cb = self.on_open_change.clone();
            let own = open_own.clone();
            move |window: &mut Window, cx: &mut App| {
                if let Some(held) = &own {
                    held.update(cx, |value, cx| {
                        *value = false;
                        cx.notify();
                    });
                }
                if let Some(cb) = &cb {
                    cb(&false, window, cx);
                }
            }
        });
        let panel_state =
            window.use_keyed_state(element_id::scoped(&self.id, "panel-scroll"), cx, |_, cx| {
                (
                    gpui::ScrollHandle::new(),
                    std::array::from_fn::<_, 3, _>(|_| cx.focus_handle()),
                    None::<usize>,
                )
            });
        let (panel_scroll, child_focus) =
            panel_state.update(cx, |(scroll, scopes, previous), cx| {
                let focused = scopes
                    .iter()
                    .position(|scope| scope.contains_focused(window, cx));
                if focused != *previous {
                    if let Some(index) = focused {
                        scroll.scroll_to_item(index);
                    }
                    *previous = focused;
                }
                (scroll.clone(), scopes.clone())
            });

        // The floating positioner resolves flips during prepaint, after this
        // render has already chosen the panel's entry direction. Keep the
        // requested placement and the resolved physical side in keyed state
        // so a resize, flip, or initially-open mount can rebuild the motion
        // with the side that is actually on screen.
        let requested_placement = window.use_keyed_state(
            element_id::scoped(&self.id, "requested-placement"),
            cx,
            |_, _| self.placement,
        );
        let resolved_placement = window.use_keyed_state(
            element_id::scoped(&self.id, "resolved-placement"),
            cx,
            |_, _| Rc::new(Cell::new(None::<Placement>)),
        );
        let placement_changed = *requested_placement.read(cx) != self.placement;
        if placement_changed {
            requested_placement.update(cx, |placement, _| *placement = self.placement);
            resolved_placement.read(cx).set(None);
        }
        let resolved_placement = resolved_placement.read(cx).clone();
        let entry_placement = resolved_placement.get().unwrap_or(self.placement);

        let anchor_bounds = window
            .use_keyed_state(element_id::scoped(&self.id, "anchor-bounds"), cx, |_, _| {
                Rc::new(Cell::new(None::<Bounds<Pixels>>))
            })
            .read(cx)
            .clone();
        // The trigger's focus-ring tween owns keyed state and therefore needs
        // a mutable app borrow; keep theme snapshots owned for the panel and
        // trigger chrome that follow.
        let colors = cx.colors().clone();
        let layout = cx.layout().clone();
        let popover_radius = layout.capped(layout.radius_lg() * 2.5);
        let trigger_pressed = Rc::new(Cell::new(false));

        // Trigger: swatch plus the hex value.
        let mut trigger = div()
            .id(element_id::scoped(&self.id, "trigger"))
            .when(!self.is_disabled, |el| el.track_focus(&trigger_focus))
            .flex()
            .flex_row()
            .items_center()
            // `.color-picker__trigger` is `inline-flex items-center gap-3
            // rounded-sm text-sm` -- a swatch beside its value, with no box of
            // its own.
            .gap(px(12.))
            .rounded(util::hairline_radius(cx))
            .text_size(px(14.))
            .line_height(px(20.))
            .text_color(colors.foreground)
            .child(ColorSwatch::new(self.value).size(SizeXl::Sm))
            .child(div().child(self.value.to_hex()));

        if self.is_disabled {
            trigger = trigger.opacity(layout.disabled_opacity);
        } else {
            trigger = util::cursor_interactive(trigger, cx);
            let pressed = trigger_pressed.clone();
            trigger = trigger.capture_any_mouse_down(move |_, _, cx| {
                pressed.set(true);
                let clear = pressed.clone();
                cx.defer(move |_| clear.set(false));
            });
            if self.on_open_change.is_some() || open_own.is_some() {
                let cb = self.on_open_change.clone();
                let own = open_own.clone();
                let next = !is_open;
                let pressed = trigger_pressed.clone();
                trigger = trigger.on_click(move |_, window, cx| {
                    if let Some(held) = &own {
                        held.update(cx, |value, cx| {
                            *value = next;
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &cb {
                        cb(&next, window, cx);
                    }
                    pressed.set(false);
                });
            }
        }

        let trigger_name =
            a11y::Name::maybe(self.label.clone()).described(Some(self.value.to_hex()));

        // Resolve the normal focused endpoint on the behavior owner first;
        // the motion wrapper below replaces that shadow list on every frame,
        // preserving the same endpoint while interpolating into and out of it.
        trigger = util::ring_if_focused(trigger, &trigger_focus, true, Vec::new(), window, cx);

        // HeroUI transitions the trigger's focus shadow over 150ms. Keep the
        // trigger as the stable behavior/a11y owner and animate only its
        // listener-free visual child so open/close and pointer dismissal do
        // not lose identity.
        let trigger_focused = util::shows_focus_ring(trigger_focus.is_focused(window), cx);
        let trigger_focus_motion = color_focus_ring_motion(
            &element_id::scoped(&self.id, "trigger"),
            trigger_focused,
            window,
            cx,
        );
        let trigger = trigger
            .a11y_named(a11y::Role::Button, &trigger_name)
            .a11y_expanded(is_open);
        let trigger = if self.is_disabled {
            trigger
        } else {
            util::record_focus_bounds(trigger, &trigger_focus, window, cx)
        };
        let trigger =
            trigger_focus_motion.render(trigger, Vec::new(), util::hairline_radius(cx), true, cx);

        // The popover overlays the page rather than pushing it down.
        let mut root = div().relative().flex().flex_col().gap(px(8.));
        if let Some(label) = self.label {
            root = root.child(crate::field::Label::new(label));
        }
        root = root.child(crate::popover::PopoverTriggerMeasure::new(
            trigger,
            anchor_bounds.clone(),
        ));
        root = root.track_focus(&group_scope);

        if phase == util::OverlayPhase::Closed {
            // RAC 1.20.0 unmounts the scroll DOM after close+exit, so a
            // reopen starts at the top. The keyed handle outlives the panel,
            // so reset it only once the exit is gone; touching it while
            // Exiting would shift the visible panel.
            panel_scroll.set_offset(gpui::point(px(0.), px(0.)));
            return root;
        }

        // React Aria dismisses the panel on Escape and on a press outside it.
        // Escape rides on the root: the panel holding the focus would take the
        // arrows away from the area and the sliders inside it.
        let close = util::shared({
            let cb = self.on_open_change.clone();
            let own = open_own;
            move |window: &mut Window, cx: &mut App| -> util::DismissResult {
                window.focus(&trigger_focus, cx);
                if let Some(held) = &own {
                    held.update(cx, |value, cx| {
                        *value = false;
                        cx.notify();
                    });
                }
                if let Some(cb) = &cb {
                    cb(&false, window, cx);
                }
                util::DismissResult::Handled
            }
        });
        root = util::dismiss_on_escape_with_token(root, dismissal_token.clone(), {
            let close = close.clone();
            move |window, cx| close(window, cx)
        });

        // `.color-picker__popover` is `gap-3 min-w-62 px-2`: a minimum width,
        // not the fixed 264 this used to force.
        let mut panel = div()
            .gap(px(12.))
            .px(px(8.))
            .pt(px(8.))
            .pb(px(12.))
            .min_w(px(248.))
            .id(element_id::scoped(&self.id, "panel"))
            .debug_selector({ let base = base.clone(); move || format!("{base}-panel") })
            .max_h_full()
            .overflow_x_hidden()
            .overflow_y_scroll()
            .restrict_scroll_to_axis()
            .occlude()
            .track_scroll(&panel_scroll)
            .flex()
            .flex_col()
            .rounded(popover_radius)
            .bg(colors.overlay.background)
            // v3 gives a floating panel no border: it is `bg-overlay
            // shadow-overlay` and a radius, and dark mode's inset hairline is
            // what separates the panel from the page.
            .when_some(layout.overlay_hairline, |el, hairline| {
                el.border(layout.border_width).border_color(hairline)
            })
            .when(!layout.overlay_shadow.is_empty(), |e| {
                e.shadow(layout.overlay_shadow.clone())
            })
            .a11y(a11y::Role::Dialog);

        let mut area = ColorArea::new(element_id::scoped(&self.id, "area"), self.value)
            .size(px(240.), px(160.));
        {
            let cb = self.on_change.clone();
            let own = own.clone();
            area = area.on_change(move |c, window, cx| {
                if let Some(held) = &own {
                    held.update(cx, |v, cx| {
                        *v = *c;
                        cx.notify();
                    });
                }
                if let Some(cb) = &cb {
                    cb(c, window, cx);
                }
            });
        }
        panel = panel.child(
            div()
                .flex_shrink_0()
                .track_focus(&child_focus[0])
                .child(area),
        );

        let mut hue = ColorSlider::new(
            element_id::scoped(&self.id, "hue"),
            self.value,
            ColorChannel::Hue,
        )
        .length(px(240.))
        .show_label(false);
        {
            let cb = self.on_change.clone();
            let own = own.clone();
            hue = hue.on_change(move |c, window, cx| {
                if let Some(held) = &own {
                    held.update(cx, |v, cx| {
                        *v = *c;
                        cx.notify();
                    });
                }
                if let Some(cb) = &cb {
                    cb(c, window, cx);
                }
            });
        }
        panel = panel.child(
            div()
                .flex_shrink_0()
                .track_focus(&child_focus[1])
                .debug_selector({
                    let base = base.clone();
                    move || format!("{base}-hue")
                })
                .child(hue),
        );

        if self.show_alpha {
            let mut alpha = ColorSlider::new(
                element_id::scoped(&self.id, "alpha"),
                self.value,
                ColorChannel::Alpha,
            )
            .length(px(240.))
            .show_label(false);
            {
                let cb = self.on_change.clone();
                let own = own;
                alpha = alpha.on_change(move |c, window, cx| {
                    if let Some(held) = &own {
                        held.update(cx, |v, cx| {
                            *v = *c;
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &cb {
                        cb(c, window, cx);
                    }
                });
            }
            panel = panel.child(
                div()
                    .flex_shrink_0()
                    .track_focus(&child_focus[2])
                    .debug_selector(move || format!("{base}-alpha"))
                    .child(alpha),
            );
        }

        panel = panel.child(
            div()
                .flex_shrink_0()
                .text_size(px(12.))
                .font_family(
                    self.font_family
                        .clone()
                        .unwrap_or_else(|| util::MONO_FONT.into()),
                )
                .text_color(colors.muted)
                .child(self.value.to_hex()),
        );

        let panel =
            util::dismiss_on_press_outside_with_token(panel, dismissal_token, move |window, cx| {
                if trigger_pressed.get() {
                    return util::DismissResult::Declined;
                }
                close(window, cx)
            });

        let (slide_x, slide_y) = color_picker_entry_offset(entry_placement);
        let zoom = crate::anim::ZoomBox {
            width: Some(px(256.)),
            padding_x: Some(px(8.)),
            padding_top: Some(px(8.)),
            padding_bottom: Some(px(12.)),
            radius: Some(popover_radius),
            slide_x: (slide_x != 0.0).then(|| px(slide_x)),
            slide_y: (slide_y != 0.0).then(|| px(slide_y)),
            ..Default::default()
        };

        let panel = if exiting {
            crate::anim::exiting(
                panel,
                element_id::scoped(&self.id, "panel-anim-out"),
                zoom,
                crate::anim::Motion::LIST_OUT,
                cx,
            )
        } else {
            crate::anim::entering_zoom(
                panel,
                element_id::scoped(&self.id, "panel-anim"),
                zoom,
                crate::anim::Motion::LIST_IN,
                cx,
            )
        };
        root.child(util::floating(
            crate::popover::scrollable_popover_with_resolved_placement(
                anchor_bounds,
                self.placement,
                Some(resolved_placement),
                panel,
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn trigger_focus_uses_the_shared_color_ring_transition() {
        let source = include_str!("picker.rs");
        let render_source = source
            .split("#[cfg(test)]")
            .next()
            .expect("the render implementation must precede this test module");
        assert!(source.contains("color_focus_ring_motion"));
        assert!(source.contains("trigger_focused"));
        assert!(source.contains("trigger_focus_motion.render"));
        assert!(render_source.contains("util::ring_if_focused(trigger"));
    }
}
