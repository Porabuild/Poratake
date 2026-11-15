//! ColorSwatchPicker.

use super::*;

const SWATCH_SCALE_TRANSITION_MS: u64 = 100;
const SWATCH_INDICATOR_TRANSITION_MS: u64 = 150;

fn swatch_visual_scale(selected: bool, hovered: bool) -> f32 {
    if selected {
        0.77
    } else if hovered {
        1.1
    } else {
        1.0
    }
}

// ColorSwatchPicker
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::swatch_visual_scale;

    #[test]
    #[allow(clippy::float_cmp)] // the scale ladder is declared exactly
    fn selected_scale_wins_over_hover_and_rest() {
        assert_eq!(swatch_visual_scale(false, false), 1.0);
        assert_eq!(swatch_visual_scale(false, true), 1.1);
        assert_eq!(swatch_visual_scale(true, false), 0.77);
        assert_eq!(swatch_visual_scale(true, true), 0.77);
    }

    #[test]
    fn the_selection_border_overlay_follows_the_item_radius() {
        let source = include_str!("swatch_picker.rs");
        // v3's selected ring is round on a round swatch: the overlay is the
        // only thing that draws it, so a square overlay squares off the
        // corners the item radius just rounded. `rsplit` anchors on the real
        // builder -- this test's own pattern literals precede it in the file.
        let overlay = source
            .rsplit("let border_visual = div()")
            .next()
            .and_then(|rest| rest.split("let border_visual = if").next())
            .expect("the selection border overlay");
        assert!(
            overlay.contains(".rounded(item_radius)"),
            "the selection border overlay must carry the item's radius"
        );
    }
}

/// Layout of a [`ColorSwatchPicker`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwatchLayout {
    /// Swatches wrapped in a grid.
    #[default]
    Grid,
    /// Swatches stacked in a single column.
    Stack,
}

/// State handed to `ColorSwatchPicker.Item`'s render function.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ColorSwatchPickerItemState {
    /// The swatch color.
    pub color: PickerColor,
    /// Whether the swatch is hovered.
    pub is_hovered: bool,
    /// Whether the swatch is pressed.
    pub is_pressed: bool,
    /// Whether the swatch is selected.
    pub is_selected: bool,
    /// Whether the swatch has focus.
    pub is_focused: bool,
    /// Whether focus is visible (keyboard focus).
    pub is_focus_visible: bool,
    /// Whether the swatch is disabled.
    pub is_disabled: bool,
}

/// ColorSwatchPicker — chooses from a predefined palette.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ColorSwatchPicker {
    /// `defaultValue` — set it to hand this component its own state.
    default_value: Option<PickerColor>,
    id: ElementId,
    swatches: Vec<PickerColor>,
    value: Option<PickerColor>,
    size: SizeXl,
    shape: SwatchShape,
    layout: SwatchLayout,
    is_disabled: bool,
    /// `ColorSwatchPicker.Item.isDisabled` — swatches that cannot be chosen,
    /// by index.
    disabled_keys: std::collections::HashSet<usize>,
    item_content:
        Option<Arc<dyn Fn(usize, ColorSwatchPickerItemState) -> gpui::AnyElement + 'static>>,
    indicator: Option<Arc<dyn Fn(usize, ColorSwatchPickerItemState) -> gpui::AnyElement + 'static>>,
    on_change: Option<OnColorChange>,
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl ColorSwatchPicker {
    /// Creates a picker over `swatches`, with the first-party default size, shape and layout.
    pub fn new(id: impl Into<ElementId>, swatches: Vec<PickerColor>) -> Self {
        Self {
            default_value: None,
            id: id.into(),
            swatches,
            value: None,
            // `.color-swatch` is `size-8` (32px), which is `SizeXl::Md` on v3's
            // own swatch scale (16/24/32/36/40).
            size: SizeXl::Md,
            shape: SwatchShape::Circle,
            layout: SwatchLayout::Grid,
            is_disabled: false,
            disabled_keys: std::collections::HashSet::new(),
            item_content: None,
            indicator: None,
            on_change: None,
            sx: None,
        }
    }

    #[doc = "Refines the swatch group layout, including spacing, wrapping and width."]
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
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

    /// Sets the controlled selected color (`value`).
    pub fn value(mut self, value: PickerColor) -> Self {
        self.value = Some(value);
        self
    }

    /// Sets the swatch size (`size`).
    pub fn size(mut self, size: SizeXl) -> Self {
        self.size = size;
        self
    }

    /// Sets the swatch shape.
    pub fn shape(mut self, shape: SwatchShape) -> Self {
        self.shape = shape;
        self
    }

    /// Sets the swatch layout.
    pub fn layout(mut self, layout: SwatchLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Sets whether the picker is disabled (`isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// `ColorSwatchPicker.Item.isDisabled` — swatches that cannot be chosen,
    /// by index.
    ///
    /// A disabled swatch draws dimmed (`status-disabled`'s reduced opacity),
    /// answers no click, and leaves the tab order, exactly as a disabled
    /// control must in this port. The dictionary is by palette position — the
    /// same projection `RadioGroup::disabled_keys` gives `Radio.isDisabled` —
    /// since the swatches are a plain list with no keys of their own.
    pub fn disabled_keys(mut self, keys: impl IntoIterator<Item = usize>) -> Self {
        self.disabled_keys = keys.into_iter().collect();
        self
    }

    /// `children` on `ColorSwatchPicker.Item` — replaces the built-in swatch
    /// and indicator while the stable item keeps selection and navigation.
    /// The closure receives the item's palette index and the complete pinned
    /// React Aria item render state.
    pub fn item_content(
        mut self,
        render: impl Fn(usize, ColorSwatchPickerItemState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.item_content = Some(Arc::new(render));
        self
    }

    /// `ColorSwatchPicker.Indicator` — replaces the selected checkmark. The
    /// render function receives the same item state as its pinned compound
    /// part, including while the indicator is visually hidden.
    pub fn indicator(
        mut self,
        render: impl Fn(usize, ColorSwatchPickerItemState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.indicator = Some(Arc::new(render));
        self
    }

    /// Sets the handler called with the chosen color (`onChange`).
    pub fn on_change(
        mut self,
        handler: impl Fn(&PickerColor, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for ColorSwatchPicker {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `defaultValue` opts into the component holding its own selection;
        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        let (resolved, own) = util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "swatches-value"),
            match self.default_value {
                Some(_) => None,
                None => Some(self.value),
            },
            self.default_value.or(self.value),
        );
        self.value = resolved;

        // One tab stop for the whole list: v3's picker sits on React Aria's
        // ListBox, which roves a listbox's tabindex, so Tab enters the picker
        // once and the arrows move inside it (AGENTS.md's roving tab stop).
        // Which swatch claims the group's handle is held in keyed state,
        // because a handle's `tab_stop` is fixed where the handle is made;
        // the cursor never rests on a disabled swatch, so a stop stranded on
        // one cannot take the picker out of the tab order. The state precedes
        // the theme borrow.
        let swatch_focus = util::tab_stop_handle(element_id::scoped(&self.id, "focus"), window, cx);
        let cursor =
            window.use_keyed_state(element_id::scoped(&self.id, "cursor"), cx, |_, _| 0usize);
        let enabled: Vec<usize> = (0..self.swatches.len())
            .filter(|index| !(self.is_disabled || self.disabled_keys.contains(index)))
            .collect();
        let at = *cursor.read(cx);
        let cursor_index = enabled
            .iter()
            .copied()
            .find(|i| *i >= at)
            .or_else(|| enabled.first().copied());
        // The built-in swatch uses the same interaction slot as render props:
        // HeroUI animates hover geometry even when callers do not provide a
        // custom item renderer, so every enabled item needs a live hover bit.
        let interactions: Vec<util::Interaction> = (0..self.swatches.len())
            .map(|index| {
                util::interaction(
                    element_id::scoped(
                        &element_id::indexed(&self.id, "swatch", index),
                        "interaction",
                    ),
                    window,
                    cx,
                )
            })
            .collect();
        let pointer_focus =
            window.use_keyed_state(element_id::scoped(&self.id, "pointer-focus"), cx, |_, _| {
                None::<usize>
            });
        let item_edge = self.size.swatch_px();
        let border_width = match self.size {
            SizeXl::Xs => px(1.),
            SizeXl::Sm | SizeXl::Md => px(2.),
            SizeXl::Lg | SizeXl::Xl => px(3.),
        };
        let item_radius = match self.shape {
            SwatchShape::Circle => match self.size {
                SizeXl::Xs => px(8.),
                SizeXl::Sm => px(12.),
                SizeXl::Md => px(16.),
                SizeXl::Lg | SizeXl::Xl => px(24.),
            },
            SwatchShape::Square => match self.size {
                SizeXl::Xs => px(6.),
                SizeXl::Sm => px(8.),
                SizeXl::Md | SizeXl::Lg | SizeXl::Xl => px(12.),
            },
        };
        // RAC's grid delegate moves Up and Down between the same column of the
        // adjacent row. A grid row holds the most cells of the active size
        // that fit under the picker's 280px maximum with its 8px gap.
        let columns = window
            .use_keyed_state(element_id::scoped(&self.id, "columns"), cx, |_, _| {
                Rc::new(Cell::new(0usize))
            })
            .read(cx)
            .clone();
        let grid = self.layout == SwatchLayout::Grid;
        let wraps =
            self.sx.as_ref().and_then(|style| style.flex_wrap) != Some(gpui::FlexWrap::NoWrap);
        let gap = self
            .sx
            .as_ref()
            .and_then(|style| style.gap.width)
            .and_then(|gap| match gap {
                gpui::DefiniteLength::Absolute(gpui::AbsoluteLength::Pixels(value)) => {
                    Some(f32::from(value))
                }
                _ => None,
            })
            .unwrap_or(8.);
        let padding = util::sx_padding(&self.sx);
        let horizontal_padding =
            f32::from(padding.left.unwrap_or(px(0.))) + f32::from(padding.right.unwrap_or(px(0.)));
        let count = self.swatches.len();
        let swatch_ring = util::focus_visible(cx);

        let clear_pointer_focus = pointer_focus.clone();
        let mut row = div()
            .id(element_id::scoped(&self.id, "swatches"))
            .flex()
            .items_center()
            .gap(px(8.))
            .on_mouse_down_out(move |_, _, cx| {
                clear_pointer_focus.update(cx, |focused, cx| {
                    if focused.take().is_some() {
                        cx.notify();
                    }
                });
            });
        row = match self.layout {
            SwatchLayout::Grid => row.flex_row().flex_wrap().max_w(px(280.)),
            SwatchLayout::Stack => row.flex_col(),
        };

        let measured_columns = columns.clone();
        row = row.relative().child(
            gpui::canvas(
                move |bounds, _, _| {
                    let width = (f32::from(bounds.size.width) - horizontal_padding).max(0.);
                    let capacity = if !grid {
                        0
                    } else if !wraps {
                        count
                    } else {
                        (((width + gap) / (f32::from(item_edge) + gap)) as usize).max(1)
                    };
                    measured_columns.set(capacity);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        );

        for (index, swatch) in self.swatches.iter().enumerate() {
            let item_id = element_id::indexed(&self.id, "swatch", index);
            let selected = self.value.is_some_and(|v| v.to_hex() == swatch.to_hex());
            // `ColorSwatchPicker.Item.isDisabled` — the item's own flag
            // beside the group-wide one: dimmed, no press, and out of the tab
            // order (`track_focus` below is gated on it, and a stop resting
            // on nothing is what takes a control out of the order).
            let item_disabled = self.is_disabled || self.disabled_keys.contains(&index);
            let (recorded_hover, recorded_press) = interactions
                .get(index)
                .map(|slot| *slot.read(cx))
                .unwrap_or_default();
            let item_focused = !item_disabled
                && cursor_index == Some(index)
                && (swatch_focus.is_focused(window)
                    || *pointer_focus.read(cx) == Some(index)
                    || recorded_press);
            let state = ColorSwatchPickerItemState {
                color: *swatch,
                is_hovered: !item_disabled && recorded_hover,
                is_pressed: !item_disabled && recorded_press,
                is_selected: selected,
                is_focused: item_focused,
                is_focus_visible: item_focused && swatch_ring,
                is_disabled: item_disabled,
            };

            let reduced_motion = ActiveTheme::reduce_motion(cx);
            let target_scale = swatch_visual_scale(selected, recorded_hover);
            let mut scale =
                crate::anim::Tween::keyed(&item_id, "swatch-scale", target_scale, window, cx);
            scale.snap_if_reduced(reduced_motion);
            let target_border = if selected {
                swatch.to_hsla()
            } else {
                gpui::transparent_black()
            };
            let mut border =
                crate::anim::Tween::keyed(&item_id, "selection-border", target_border, window, cx);
            border.snap_if_reduced(reduced_motion);
            let mut indicator_scale = crate::anim::Tween::keyed(
                &item_id,
                "indicator-scale",
                if selected { 1.0 } else { 0.0 },
                window,
                cx,
            );
            indicator_scale.snap_if_reduced(reduced_motion);

            let mut cell = div()
                .id(item_id.clone())
                .when(cursor_index == Some(index), |c| {
                    c.track_focus(&swatch_focus)
                })
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .size(item_edge)
                .rounded(item_radius)
                .border(border_width)
                .border_color(gpui::transparent_black());

            if let Some(render) = &self.item_content {
                cell = cell.child(render(index, state));
            } else {
                // `.color-swatch-picker__swatch` owns the animated built-in
                // color surface; the stable cell remains the interaction hit
                // target and the selection border is layered above it.
                let base_radius = match (self.shape, self.size) {
                    (SwatchShape::Circle, _) => f32::from(item_radius),
                    (SwatchShape::Square, SizeXl::Xs) => 6.,
                    (SwatchShape::Square, SizeXl::Sm) => 6.,
                    (SwatchShape::Square, _) => 8.,
                };
                let color = swatch.to_hsla();
                let visual = div()
                    .size(gpui::relative(scale.target()))
                    .rounded(px(base_radius * scale.target()))
                    .flex_shrink_0()
                    .overflow_hidden()
                    // The checkerboard that shows through a translucent
                    // colour, as on a plain `ColorSwatch`.
                    .bg(cx.colors().surface_secondary)
                    // Vanilla GPUI clips `overflow_hidden()` to the
                    // rectangle, so the color fill carries the visual
                    // radius itself or it squares off the corners.
                    .child(div().size_full().rounded(px(base_radius * scale.target())).bg(color));
                let visual = if scale.animates(reduced_motion) {
                    let from = scale.from();
                    let value = scale.value();
                    visual
                        .with_animation(
                            element_id::indexed(&item_id, "swatch-scale", scale.generation()),
                            Animation::new(Duration::from_millis(SWATCH_SCALE_TRANSITION_MS))
                                .with_easing(crate::anim::ease_out()),
                            move |el, delta| {
                                let next = from + (target_scale - from) * delta;
                                value.set(next);
                                el.size(gpui::relative(next))
                                    .rounded(px(base_radius * next))
                            },
                        )
                        .into_any_element()
                } else {
                    scale.settle();
                    visual.into_any_element()
                };
                cell = cell.child(visual);
            }

            let field_shadow = cx.layout().field_shadow.clone();
            let border_visual = div()
                .absolute()
                .inset_0()
                .rounded(item_radius)
                .border(border_width)
                .when(selected && !field_shadow.is_empty(), |b| {
                    b.shadow(field_shadow)
                });
            let border_visual = if border.animates(reduced_motion) {
                let from = border.from();
                let value = border.value();
                border_visual
                    .with_animation(
                        element_id::indexed(&item_id, "selection-border", border.generation()),
                        Animation::new(Duration::from_millis(SWATCH_SCALE_TRANSITION_MS))
                            .with_easing(crate::anim::ease_out()),
                        move |el, delta| {
                            let next = herogpui_core::mix_oklab(from, target_border, delta);
                            value.set(next);
                            el.border_color(next)
                        },
                    )
                    .into_any_element()
            } else {
                border.settle();
                border_visual
                    .border_color(border.target())
                    .into_any_element()
            };
            cell = cell.child(border_visual);

            if self.item_content.is_none() {
                // `.color-swatch-picker__indicator` is an absolute centered
                // slot; its child scales in and out without changing the hit
                // box or pushing the swatch layout.
                let indicator_hidden =
                    indicator_scale.target() == 0.0 && !indicator_scale.animates(reduced_motion);
                let indicator_content = if let Some(render) = &self.indicator {
                    let content = render(index, state);
                    (!indicator_hidden).then_some(content)
                } else {
                    (!indicator_hidden).then(|| {
                        gpui::svg()
                            .size(px(f32::from(item_edge) / 3.))
                            .path(crate::icons::CHECK)
                            .text_color(color_swatch_indicator_color(*swatch))
                            .into_any_element()
                    })
                };
                let indicator_edge = f32::from(item_edge) / 3.;
                let indicator_visual = div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .size(px(indicator_edge * indicator_scale.target()))
                    .children(indicator_content);
                let indicator_visual = if indicator_scale.animates(reduced_motion) {
                    let from = indicator_scale.from();
                    let target = indicator_scale.target();
                    let value = indicator_scale.value();
                    indicator_visual
                        .with_animation(
                            element_id::indexed(
                                &item_id,
                                "indicator-scale",
                                indicator_scale.generation(),
                            ),
                            Animation::new(Duration::from_millis(SWATCH_INDICATOR_TRANSITION_MS))
                                .with_easing(crate::anim::ease_out()),
                            move |el, delta| {
                                let next = from + (target - from) * delta;
                                value.set(next);
                                el.size(px(indicator_edge * next))
                            },
                        )
                        .into_any_element()
                } else {
                    indicator_scale.settle();
                    indicator_visual.into_any_element()
                };
                cell = cell.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(indicator_visual),
                );
            }

            // One commit for the pointer click and the Enter/Space key-up:
            // uncontrolled takes the selection, or the press would do
            // nothing.
            let commit = {
                let own = own.clone();
                let on_change = self.on_change.clone();
                let value = *swatch;
                move |window: &mut Window, cx: &mut App| {
                    if let Some(held) = &own {
                        held.update(cx, |v, cx| {
                            *v = Some(value);
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &on_change {
                        cb(&value, window, cx);
                    }
                }
            };

            if item_disabled {
                // v3's sheet: `[data-disabled="true"]` is `status-disabled`,
                // reduced opacity, and no press handler is attached here at
                // all -- a disabled swatch cannot report a choice.
                cell = cell.opacity(cx.layout().disabled_opacity);
            } else if self.on_change.is_some() || own.is_some() {
                let commit_click = commit.clone();
                cell = cell
                    .cursor(util::interactive_cursor(cx))
                    .on_click(move |_, window, cx| commit_click(window, cx));
            }

            if !item_disabled {
                let moved = cursor.clone();
                let focus = swatch_focus.clone();
                let pointer = pointer_focus.clone();
                if let Some(interaction) = interactions.get(index) {
                    cell = util::track_interaction_on_mouse_down(
                        cell,
                        interaction,
                        move |window, cx| {
                            moved.update(cx, |cursor, cx| {
                                *cursor = index;
                                cx.notify();
                            });
                            pointer.update(cx, |focused, cx| {
                                *focused = Some(index);
                                cx.notify();
                            });
                            window.focus(&focus, cx);
                            // gpui's mouse-down default focuses the nearest
                            // focusable ancestor of the press -- the app
                            // root here, since only the *previous* cursor cell
                            // tracks the group's handle -- and would steal the
                            // handle back in the same dispatch.
                            window.prevent_default();
                        },
                    );
                } else {
                    cell = cell.on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                        moved.update(cx, |cursor, cx| {
                            *cursor = index;
                            cx.notify();
                        });
                        pointer.update(cx, |focused, cx| {
                            *focused = Some(index);
                            cx.notify();
                        });
                        window.focus(&focus, cx);
                        window.prevent_default();
                    });
                }
            }

            // The collection keyboard, the ListBox's: the arrows rove the
            // focus over the enabled swatches and Home and End jump, while a
            // disabled swatch is never a stop. Enter and Space are the press,
            // fired on key up -- the same edge gpui maps a focused press to a
            // click on -- but the commit runs here rather than through gpui's
            // pending-keyboard record, which a pointer-pressed swatch leaves
            // unarmed; the capture keeps gpui's half from also firing, so the
            // selection still lands exactly once. Disabled swatches never see
            // this handler (they cannot hold the focus), so the cursor only
            // moves between `enabled`.
            if !item_disabled {
                let stops = enabled.clone();
                let moved = cursor.clone();
                let pointer = pointer_focus.clone();
                let columns = columns.clone();
                let commit_key = commit.clone();
                cell = cell
                    .capture_key_down(move |event, window, _cx| {
                        let key = event.keystroke.key.as_str();
                        if matches!(key, "enter" | "space") && !event.keystroke.modifiers.modified()
                        {
                            window.prevent_default();
                        }
                    })
                    .on_key_up(move |event, window, cx| {
                        let key = event.keystroke.key.as_str();
                        if matches!(key, "enter" | "space") && !event.keystroke.modifiers.modified()
                        {
                            commit_key(window, cx);
                        }
                    });
                cell = cell.on_key_down(move |event, _window, cx| {
                    let key = event.keystroke.key.as_str();
                    if key == "tab" {
                        pointer.update(cx, |focused, cx| {
                            if focused.take().is_some() {
                                cx.notify();
                            }
                        });
                        return;
                    }
                    let next = match key {
                        // Grid rows: same column, adjacent row. The strided
                        // target must be a real enabled swatch; otherwise the
                        // row does not exist, which is what the geometric
                        // delegate reports for a ragged grid too.
                        "up" | "down" if columns.get() > 0 => {
                            let j = index as isize
                                + if key == "down" {
                                    columns.get() as isize
                                } else {
                                    -(columns.get() as isize)
                                };
                            if j < 0 || !stops.contains(&(j as usize)) {
                                return;
                            }
                            j as usize
                        }
                        key @ ("up" | "down") if columns.get() == 0 => {
                            let crate::list_nav::Move::To(next) =
                                crate::list_nav::resolve(&stops, Some(index), key, false)
                            else {
                                return;
                            };
                            next
                        }
                        key @ ("left" | "right" | "home" | "end") => {
                            let key = match key {
                                "right" => "down",
                                "left" => "up",
                                other => other,
                            };
                            let crate::list_nav::Move::To(next) =
                                crate::list_nav::resolve(&stops, Some(index), key, false)
                            else {
                                return;
                            };
                            next
                        }
                        _ => return,
                    };
                    cx.stop_propagation();
                    // No refocusing: the next render has the swatch at `next`
                    // claim the group's handle, so the focus goes with it.
                    moved.update(cx, |v, cx| {
                        *v = next;
                        cx.notify();
                    });
                });
            }

            let mut cell = cell
                .a11y_named(
                    a11y::Role::RadioButton,
                    &a11y::Name::labelled(swatch.to_hex()),
                )
                .a11y_selected(selected);
            if cursor_index == Some(index) {
                cell = util::record_focus_bounds(cell, &swatch_focus, window, cx);
            }
            let focus_motion =
                color_focus_ring_motion(&item_id, swatch_ring && item_focused, window, cx);
            row = row.child(focus_motion.render(cell, Vec::new(), item_radius, true, cx));
        }

        util::apply_sx(row, &self.sx).a11y(a11y::Role::RadioGroup)
    }
}

// ---------------------------------------------------------------------------
