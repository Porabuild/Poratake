//! ColorSlider.

use super::*;

// ColorSlider
// ---------------------------------------------------------------------------

/// State handed to `ColorSlider.Thumb`'s render function.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ColorSliderThumbState {
    /// React Aria's ColorThumb render color excludes the alpha channel.
    pub color: PickerColor,
    /// Whether the thumb is being dragged.
    pub is_dragging: bool,
    /// Whether the slider is hovered.
    pub is_hovered: bool,
    /// Whether the slider has focus.
    pub is_focused: bool,
    /// Whether focus is visible (keyboard focus).
    pub is_focus_visible: bool,
    /// Whether the slider is disabled.
    pub is_disabled: bool,
}

const COLOR_SLIDER_TRACK_INSET_PX: f32 = 10.0;
const COLOR_SLIDER_THUMB_TRANSITION_MS: u64 = 250;

#[derive(Clone)]
struct ColorSliderThumbMotion {
    target: f32,
    generation: usize,
    from: f32,
    position: Rc<Cell<f32>>,
}

struct ColorSliderThumbMotionFrame {
    base: ElementId,
    generation: usize,
    from: f32,
    to: f32,
    position: Rc<Cell<f32>>,
    animate: bool,
}

impl ColorSliderThumbMotionFrame {
    fn render(self, thumb: gpui::Div, travel: Pixels, vertical: bool) -> gpui::AnyElement {
        if !self.animate {
            self.position.set(self.to);
            return thumb.into_any_element();
        }

        let base = self.base;
        let generation = self.generation;
        let from = self.from;
        let to = self.to;
        let position = self.position;
        thumb
            .relative()
            .with_animation(
                element_id::indexed(&base, "thumb-position", generation),
                Animation::new(Duration::from_millis(COLOR_SLIDER_THUMB_TRANSITION_MS))
                    .with_easing(|t| crate::anim::Curve::Out.at(t)),
                move |thumb, delta| {
                    let next = from + (to - from) * delta;
                    position.set(next);
                    // The wrapper has already moved to the target. Offset the
                    // visual child back toward the live frame until the tween
                    // reaches the endpoint. Vertical tracks measure from the
                    // opposite edge, so their sign is reversed.
                    let offset = color_slider_thumb_transition_offset(next, to, travel, vertical);
                    if vertical {
                        thumb.top(offset)
                    } else {
                        thumb.left(offset)
                    }
                },
            )
            .into_any_element()
    }
}

fn color_slider_thumb_motion(
    id: &ElementId,
    target: f32,
    window: &mut Window,
    cx: &mut App,
) -> ColorSliderThumbMotionFrame {
    let state = window.use_keyed_state(element_id::scoped(id, "thumb-position"), cx, |_, _| {
        ColorSliderThumbMotion {
            target,
            generation: 0,
            from: target,
            position: Rc::new(Cell::new(target)),
        }
    });
    let mut current = state.read(cx).clone();
    if (current.target - target).abs() > f32::EPSILON {
        current.target = target;
        current.generation = current.generation.wrapping_add(1);
        current.from = current.position.get();
        state.update(cx, |stored, _| *stored = current.clone());
    }
    if ActiveTheme::reduce_motion(cx) && (current.position.get() - target).abs() > f32::EPSILON {
        current.from = target;
        current.position.set(target);
        state.update(cx, |stored, _| *stored = current.clone());
    }
    ColorSliderThumbMotionFrame {
        base: id.clone(),
        generation: current.generation,
        from: current.from,
        to: target,
        position: current.position,
        animate: current.generation != 0
            && !ActiveTheme::reduce_motion(cx)
            && (current.from - target).abs() > f32::EPSILON,
    }
}

pub(super) fn color_slider_thumb_transition_offset(
    from: f32,
    to: f32,
    travel: Pixels,
    vertical: bool,
) -> Pixels {
    let offset = if vertical {
        (to - from) * f32::from(travel)
    } else {
        (from - to) * f32::from(travel)
    };
    px(offset)
}

/// ColorSlider — adjusts a single channel along a gradient track.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ColorSlider {
    /// `name` — the name this control submits under; read back by
    /// [`Self::form_field`].
    name: Option<SharedString>,
    /// `aria-label` — the explicit accessible name, which wins over the
    /// name- and channel-derived name exactly as the HTML name computation
    /// ranks it.
    aria_label: Option<SharedString>,
    /// `defaultValue` — set it to hand this component its own state.
    default_value: Option<PickerColor>,
    id: ElementId,
    value: PickerColor,
    channel: ColorChannel,
    /// `colorSpace` — only saturation differs between HSB and HSL, so this
    /// picks which one a saturation slider edits.
    color_space: ColorSpace,
    orientation: herogpui_core::Orientation,
    length: Pixels,
    show_label: bool,
    /// `ColorSlider.Output`'s render props: the closure is handed the current
    /// `color` and the formatted channel value.
    output: Option<Arc<dyn Fn(PickerColor, &str) -> gpui::AnyElement + 'static>>,
    thumb: Option<Arc<dyn Fn(ColorSliderThumbState) -> gpui::AnyElement + 'static>>,
    is_disabled: bool,
    on_change: Option<OnColorChange>,
    on_change_end: Option<OnColorChange>,
    form_state: Rc<RefCell<crate::form::LiveFormFieldState>>,
}

impl ColorSlider {
    /// Creates a slider editing `channel` of `value`.
    pub fn new(id: impl Into<ElementId>, value: PickerColor, channel: ColorChannel) -> Self {
        Self {
            name: None,
            aria_label: None,
            default_value: None,
            id: id.into(),
            value,
            channel,
            color_space: ColorSpace::default(),
            orientation: herogpui_core::Orientation::Horizontal,
            length: px(240.),
            show_label: true,
            output: None,
            thumb: None,
            is_disabled: false,
            on_change: None,
            on_change_end: None,
            form_state: live_color_form_state(crate::form::FormValue::Number(0.0)),
        }
    }

    /// `name` — the name this control submits under.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// `aria-label` — the explicit accessible name, which wins over the
    /// name- and channel-derived name exactly as the HTML name computation
    /// ranks it.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// The `Form` field this control submits, when it has a `name`.
    ///
    /// v3 discovers a field through the DOM; gpui gives a child no way to reach
    /// its ancestor, so the control hands the pair over instead. Borrows, so the
    /// control is still yours to place:
    ///
    /// ```
    /// # use gpui::{prelude::*, Window};
    /// # use herogpui_components::{ColorChannel, ColorSlider, Form, PickerColor};
    /// # struct Demo;
    /// # impl Render for Demo {
    /// #     fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    /// #         let form = Form::new();
    /// #         let color = PickerColor::hsb(210., 0.8, 0.9);
    /// #         let control = ColorSlider::new("hue", color, ColorChannel::Hue).name("hue");
    /// let field = control.form_field();
    /// form.field(field.unwrap()).child(control)
    /// #     }
    /// # }
    /// # let mut tcx = gpui::TestAppContext::single();
    /// # tcx.update(herogpui_theme::ThemeProvider::init);
    /// # let _ = tcx.add_window_view(|_, _| Demo);
    /// ```
    pub fn form_field(&self) -> Option<crate::form::FormField> {
        let name = self.name.clone()?;
        let value = self.default_value.unwrap_or(self.value);
        // A disabled range input is not a successful control in HTML, so the
        // field stays registered and is omitted from FormData.
        sync_color_form_state(
            &self.form_state,
            color_slider_form_value(value, self.channel, self.color_space),
            !self.is_disabled,
            false,
        );
        Some(crate::form::FormField::live(name, self.form_state.clone()).is_required(false))
    }

    /// `defaultValue` — the uncontrolled initial colour.
    ///
    /// Supplying it hands the component its own state: the constructor's
    /// `value` becomes the seed, and a change moves the component's copy.
    pub fn default_value(mut self, value: PickerColor) -> Self {
        self.default_value = Some(value);
        self
    }

    /// `orientation` — a vertical slider runs bottom to top.
    /// `colorSpace` — the space the channel is read in. Defaults to HSB, the
    /// space [`PickerColor`] stores.
    pub fn color_space(mut self, space: ColorSpace) -> Self {
        self.color_space = space;
        self
    }

    /// Sets the slider orientation (`orientation`).
    pub fn orientation(mut self, orientation: herogpui_core::Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// `onChangeEnd` — fires once when the drag finishes.
    pub fn on_change_end(
        mut self,
        handler: impl Fn(&PickerColor, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change_end = Some(Arc::new(handler));
        self
    }

    /// Sets the length of the slider along its axis.
    pub fn length(mut self, length: impl Into<Pixels>) -> Self {
        self.length = length.into();
        self
    }

    /// `ColorSlider.Output`'s render function — v3 hands it the `color`, which
    /// is what this closure takes along with the value as v3 formats it.
    pub fn output(
        mut self,
        render: impl Fn(PickerColor, &str) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.output = Some(Arc::new(render));
        self
    }

    /// `ColorSlider.Thumb`'s render function — the closure receives the shared
    /// ColorThumb interaction state while the built-in thumb retains its
    /// positioning, focus and drag behavior.
    pub fn thumb(
        mut self,
        render: impl Fn(ColorSliderThumbState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.thumb = Some(Arc::new(render));
        self
    }

    /// Sets whether the label is shown.
    pub fn show_label(mut self, v: bool) -> Self {
        self.show_label = v;
        self
    }

    /// Sets whether the slider is disabled (`isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Sets the handler called with the new color as the thumb moves (`onChange`).
    pub fn on_change(
        mut self,
        handler: impl Fn(&PickerColor, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }

    /// React Stately's `getDisplayColor`: hue uses the saturated spectrum,
    /// and only the alpha channel displays transparency.
    fn display_color(&self) -> PickerColor {
        match self.channel {
            ColorChannel::Hue => PickerColor::hsb(self.value.hue, 1.0, 1.0),
            ColorChannel::Alpha => self.value,
            _ => self.value.with_alpha(1.0),
        }
    }

    /// The two ends of the displayed gradient, shared by the ramp and caps.
    fn gradient_ends(&self) -> (Hsla, Hsla) {
        let (min, max) = self.channel.range();
        let color = self.display_color();
        (
            color
                .with_channel_in(self.channel, self.color_space, min)
                .to_hsla(),
            color
                .with_channel_in(self.channel, self.color_space, max)
                .to_hsla(),
        )
    }
}

/// Two gradient halves, `start -> middle` then `middle -> end`.
///
/// `inset` is the fraction of the box at each end that must hold a constant
/// colour (0.0 for `ColorArea`, `cap / length` for `ColorSlider`). The halves
/// travel over `[inset, 1 - inset]`, but each is *stretched* out to its box
/// edge and compensates with stop percentages, so the end zones are flat
/// colour without any extra element: GPUI's shader remaps `t` by
/// `(t - stop0) / (stop1 - stop0)` and clamps to `[0, 1]`, which holds the end
/// colour over the stretched part. A separate cap element would break under
/// `opacity()` (GPUI applies opacity per element, not per group) and could not
/// carry the r10 curve anyway, because GPUI clamps a corner radius to half the
/// element's shortest side.
///
/// Each half carries the clip radius on its exterior corners: vanilla GPUI
/// clips `overflow_hidden()` to the rectangle (see `util::inner_fill_radius`).
/// Interior corners meet mid-gradient where rounding would show, so only the
/// outer pair rounds.
pub(super) fn three_stop_gradient(
    vertical: bool,
    start: Hsla,
    middle: Hsla,
    end: Hsla,
    radius: Pixels,
    inset: f32,
) -> gpui::Div {
    let inset = inset.clamp(0.0, 0.45);
    let half = (1.0 - inset * 2.0) / 2.0;
    let extent = gpui::relative(half + inset);
    let total = half + inset;
    let (near_from, far_to) = if total > 0.0 {
        (inset / total, half / total)
    } else {
        (0.0, 1.0)
    };
    if vertical {
        // Band 0 (`start`) is the bottom half.
        div()
            .absolute()
            .inset_0()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .rounded_tl(radius)
                    .rounded_tr(radius)
                    .h(extent)
                    .bg(gpui::linear_gradient(
                        0.0,
                        gpui::linear_color_stop(middle, 0.0),
                        gpui::linear_color_stop(end, far_to),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .rounded_bl(radius)
                    .rounded_br(radius)
                    .h(extent)
                    .bg(gpui::linear_gradient(
                        0.0,
                        gpui::linear_color_stop(start, near_from),
                        gpui::linear_color_stop(middle, 1.0),
                    )),
            )
    } else {
        div()
            .absolute()
            .inset_0()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .rounded_tl(radius)
                    .rounded_bl(radius)
                    .w(extent)
                    .bg(gpui::linear_gradient(
                        90.0,
                        gpui::linear_color_stop(start, near_from),
                        gpui::linear_color_stop(middle, 1.0),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .rounded_tr(radius)
                    .rounded_br(radius)
                    .w(extent)
                    .bg(gpui::linear_gradient(
                        90.0,
                        gpui::linear_color_stop(middle, 0.0),
                        gpui::linear_color_stop(end, far_to),
                    )),
            )
    }
}

impl RenderOnce for ColorSlider {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `defaultValue` opts into the component holding its own colour;
        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        let (resolved, own) = util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "slider-value"),
            match self.default_value {
                Some(_) => None,
                None => Some(self.value),
            },
            self.default_value.unwrap_or(self.value),
        );
        self.value = resolved;
        let form_default = window.use_keyed_state(
            element_id::scoped(&self.id, "slider-form-default"),
            cx,
            |_, _| None::<PickerColor>,
        );
        if form_default.read(cx).is_none() {
            let initial = self.value;
            form_default.update(cx, |slot, cx| {
                *slot = Some(initial);
                cx.notify();
            });
        }
        let restore_default = form_default.read(cx).unwrap_or(self.value);
        // Submit the resolved colour's channel. Uncontrolled keyed state is
        // current after interaction; a controlled owner must accept the change
        // before the next render writes it here.
        sync_color_form_state(
            &self.form_state,
            color_slider_form_value(self.value, self.channel, self.color_space),
            !self.is_disabled,
            false,
        );
        let restore_own = own.clone();
        let restore_on_change = self.on_change.clone();
        let restore_form_state = Rc::downgrade(&self.form_state);
        let restore_channel = self.channel;
        let restore_space = self.color_space;
        let restore_is_disabled = self.is_disabled;
        let restore: Arc<dyn Fn(&mut Window, &mut App)> = util::shared(move |window, cx| {
            if let Some(own) = &restore_own {
                own.update(cx, |current, cx| {
                    *current = restore_default;
                    cx.notify();
                });
            }
            if let Some(callback) = &restore_on_change {
                callback(&restore_default, window, cx);
            }
            if let Some(state) = restore_form_state.upgrade() {
                sync_color_form_state(
                    &state,
                    color_slider_form_value(restore_default, restore_channel, restore_space),
                    !restore_is_disabled,
                    false,
                );
            }
        });
        self.form_state.borrow_mut().restore = Some(restore);
        // The handle the keys arrive on. `use_keyed_state` takes `cx` mutably, so
        // it precedes the theme tokens.
        let focus_handle =
            window.use_keyed_state(element_id::scoped(&self.id, "slider-focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            });
        let focus_handle = focus_handle.read(cx).clone();
        self.form_state.borrow_mut().focus = Some(focus_handle.clone());
        let bounds_slot =
            window.use_keyed_state(element_id::scoped(&self.id, "slider-bounds"), cx, |_, _| {
                Bounds::<f32> {
                    origin: gpui::point(0., 0.),
                    size: gpui::size(0., 0.),
                }
            });
        let dragging = window.use_keyed_state(
            element_id::scoped(&self.id, "slider-dragging"),
            cx,
            |_, _| false,
        );
        let thumb_hovered = window.use_keyed_state(
            element_id::scoped(&self.id, "slider-thumb-hovered"),
            cx,
            |_, _| false,
        );
        let (min, max) = self.channel.range();
        // Read in the requested space: HSL and HSB saturation are different
        // numbers for the same colour.
        let raw = self.value.channel_in(self.channel, self.color_space);
        let norm = ((raw - min) / (max - min)).clamp(0.0, 1.0);
        let thumb_motion = color_slider_thumb_motion(&self.id, norm, window, cx);
        let default_thumb_color = cx.colors().default.color;
        // `.color-slider__track` is `relative rounded-2xl` with the gradient
        // inside it; `.color-slider__output` is the value read-out above.
        let track_h = px(20.);

        let vertical = !self.orientation.is_horizontal();
        let mut track = div()
            .id(self.id.clone())
            .relative()
            .rounded(px(COLOR_SLIDER_TRACK_INSET_PX));
        track = if vertical {
            track.w(track_h).h(self.length)
        } else {
            track.w(self.length).h(track_h)
        };

        let recorder_bounds = bounds_slot.clone();
        track = track.child(
            gpui::canvas(
                move |bounds: Bounds<Pixels>, _, cx| {
                    recorder_bounds.update(cx, |slot, _| {
                        *slot = Bounds {
                            origin: gpui::point(
                                f32::from(bounds.origin.x),
                                f32::from(bounds.origin.y),
                            ),
                            size: gpui::size(
                                f32::from(bounds.size.width),
                                f32::from(bounds.size.height),
                            ),
                        };
                    });
                    bounds
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        );

        // Clip the complete painted track once, as ColorArea does. Edge
        // shadows and the thumb remain siblings outside this clip.
        let track_r = px(COLOR_SLIDER_TRACK_INSET_PX);
        let mut layers = div()
            .absolute()
            .inset_0()
            .rounded(track_r)
            .overflow_hidden();

        if self.channel == ColorChannel::Alpha {
            // Light cells ride the rounded base; dark cells are one clipped
            // silhouette (see `transparency_checker_cells`), so the
            // translucent end keeps exact corners on vanilla GPUI.
            let (checker_w, checker_h) = if vertical {
                (track_h, self.length)
            } else {
                (self.length, track_h)
            };
            layers = layers
                .bg(gpui::rgb(CHECKER_LIGHT))
                .child(transparency_checker_cells(
                    checker_w,
                    checker_h,
                    track_r,
                    if self.is_disabled {
                        cx.layout().disabled_opacity
                    } else {
                        1.0
                    },
                ));
        }

        // The ramp is one full-length element, and the 10px constant-colour
        // end zones come from gradient stop *percentages* rather than from cap
        // elements painted over or under it. Two vanilla-GPUI facts force that:
        //
        // * `opacity()` is applied per element, not per group, so a disabled
        //   track's translucent ramp would let anything underneath show
        //   through as solid blocks at the ends; and
        // * a corner radius is clamped to half the element's shortest side, so
        //   a 10px-wide cap tops out at r5 and cannot paint the track's r10
        //   curve. A full-length ramp's box is `length x 20`, where r10 is
        //   never clamped.
        //
        // GPUI's shader remaps `t` by `(t - stop0) / (stop1 - stop0)` and
        // clamps to `[0, 1]` (`shaders.metal`, "Adjust t based on the stop
        // percentages"), so a start stop at `f = cap / length` holds the start
        // colour over the first `cap` pixels and an end stop at `1 - f` holds
        // the end colour over the last `cap` pixels, while the travel in
        // between is unchanged. Nothing but the alpha checkerboard is painted
        // under the ramp, and nothing at all over it.
        let (start_color, end_color) = self.gradient_ends();
        let inset =
            (COLOR_SLIDER_TRACK_INSET_PX / f32::from(self.length).max(1.0)).clamp(0.0, 0.45);
        let ramp = div().absolute().inset_0().rounded(track_r);
        let ramp = if self.channel == ColorChannel::Lightness {
            let (start, middle, end) =
                lightness_gradient_colors(self.display_color(), self.color_space, min, max);
            ramp.child(three_stop_gradient(
                vertical, start, middle, end, track_r, inset,
            ))
        } else if self.channel == ColorChannel::Hue {
            ramp.child(hue_gradient(
                self.display_color(),
                self.color_space,
                vertical,
                track_r,
                inset,
            ))
        } else {
            ramp.bg(gpui::linear_gradient(
                if vertical { 0.0 } else { 90.0 },
                gpui::linear_color_stop(start_color, inset),
                gpui::linear_color_stop(end_color, 1.0 - inset),
            ))
        };
        layers = layers.child(ramp);

        track = track.child(layers);

        // HeroUI uses inset edge shadows, without a solid track border. A
        // separate outline over the antialiased fill leaves a colored fringe.
        // Keep the edge shading below the thumb, including at its endpoints.
        track = track.child(
            div()
                .absolute()
                .inset_0()
                .rounded(px(COLOR_SLIDER_TRACK_INSET_PX))
                .shadow(color_track_shadows()),
        );

        // A vertical slider's zero end is at the bottom, so the offset is
        // measured from the far edge.
        let travel = (f32::from(self.length) - COLOR_SLIDER_TRACK_INSET_PX * 2.0).max(0.0);
        let thumb_offset = px(COLOR_SLIDER_TRACK_INSET_PX
            + travel * if vertical { 1.0 - norm } else { norm }
            - 8.0);
        let is_dragging = !self.is_disabled && *dragging.read(cx);
        let is_focused = !self.is_disabled && focus_handle.is_focused(window);
        let is_focus_visible = is_focused && util::focus_visible(cx);
        let thumb_state = ColorSliderThumbState {
            color: self.value.with_alpha(1.0),
            is_dragging,
            is_hovered: !self.is_disabled && *thumb_hovered.read(cx),
            is_focused,
            is_focus_visible,
            is_disabled: self.is_disabled,
        };
        let thumb_content = self.thumb.as_ref().map(|render| render(thumb_state));
        let focus_motion = color_focus_ring_motion(
            &element_id::scoped(&self.id, "slider-thumb"),
            is_focus_visible,
            window,
            cx,
        );
        let thumb_surface = focus_motion.render(
            div()
                // `.color-slider__thumb` is `size-4`.
                .size(px(16.))
                .rounded(px(16.))
                .border(px(3.))
                .border_color(gpui::white())
                .bg(if self.is_disabled {
                    default_thumb_color
                } else {
                    self.value.with_alpha(1.0).to_hsla()
                })
                .when_some(thumb_content, |thumb, content| thumb.child(content)),
            color_thumb_shadows(),
            px(16.),
            true,
            cx,
        );
        let thumb_visual = div()
            .size(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .child(thumb_surface);
        let mut thumb = div()
            .id(element_id::scoped(&self.id, "slider-thumb"))
            .absolute()
            .when(vertical, |t| t.left(px(2.)).top(thumb_offset))
            .when(!vertical, |t| t.top(px(2.)).left(thumb_offset))
            .size(px(16.))
            .when(!self.is_disabled, |t| {
                t.cursor(if is_dragging {
                    gpui::CursorStyle::ClosedHand
                } else {
                    gpui::CursorStyle::OpenHand
                })
            })
            .child(thumb_motion.render(thumb_visual, px(travel), vertical));
        if !self.is_disabled {
            let hovered = thumb_hovered;
            thumb = thumb.on_hover(move |is_hovered, _, cx| {
                hovered.update(cx, |value, cx| {
                    if *value != *is_hovered {
                        *value = *is_hovered;
                        cx.notify();
                    }
                });
            });
        }
        track = track.child(thumb);

        if self.is_disabled {
            track = track.opacity(cx.layout().disabled_opacity);
        } else {
            let value = self.value;
            let channel = self.channel;
            let space = self.color_space;
            track = util::cursor_interactive(track, cx);
            // v3: the arrows step the channel, Home and End take it to its ends,
            // and Page Up/Down move by a tenth of the range -- React Aria's page
            // step. A colour slider with no keyboard is not the same control.
            let keys_value = self.value;
            let on_change_keys = self.on_change.clone();
            let end_keys = self.on_change_end.clone();
            let own_keys = own.clone();
            // One step per unit for a 0-360 hue or an 0-255 byte, and a
            // percentage point for the normalised channels.
            let step = if max - min > 2.0 { 1.0 } else { 0.01 };
            let page = ((max - min) / 10.0).max(step);
            track = track
                .track_focus(&focus_handle)
                .key_context("ColorSlider")
                .on_key_down(move |event, window, cx| {
                    let current = keys_value.channel_in(channel, space);
                    let next = match event.keystroke.key.as_str() {
                        "right" | "up" => current + step,
                        "left" | "down" => current - step,
                        "pageup" => current + page,
                        "pagedown" => current - page,
                        "home" => min,
                        "end" => max,
                        _ => return,
                    };
                    let next = keys_value.with_channel_in(channel, space, next.clamp(min, max));
                    if next == keys_value {
                        return;
                    }
                    if let Some(held) = &own_keys {
                        held.update(cx, |v, cx| {
                            *v = next;
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &on_change_keys {
                        cb(&next, window, cx);
                    }
                    // A keystroke is a finished change, so `onChangeEnd` fires
                    // with it rather than waiting for a release.
                    if let Some(cb) = &end_keys {
                        cb(&next, window, cx);
                    }
                });
            if self.on_change.is_some() || self.on_change_end.is_some() || own.is_some() {
                let down_bounds = bounds_slot.clone();
                let down_dragging = dragging.clone();
                let down_change = self.on_change.clone();
                let down_own = own.clone();
                let focus_for_press = focus_handle.clone();
                track = track.on_mouse_down(
                    gpui::MouseButton::Left,
                    move |event: &MouseDownEvent, window, cx| {
                        if event.modifiers.alt
                            || event.modifiers.control
                            || event.modifiers.platform
                        {
                            return;
                        }
                        down_dragging.update(cx, |value, cx| {
                            if !*value {
                                *value = true;
                                cx.notify();
                            }
                        });
                        window.focus(&focus_for_press, cx);
                        if let Some(next) = slider_color_from_pointer(
                            &down_bounds,
                            event.position,
                            vertical,
                            value,
                            channel,
                            space,
                            cx,
                        ) {
                            if next.changed {
                                report_color_change(
                                    next.value,
                                    &down_own,
                                    &down_change,
                                    window,
                                    cx,
                                );
                            }
                        }
                    },
                );

                let global_bounds = bounds_slot;
                let global_dragging = dragging;
                let global_change = self.on_change;
                let global_end = self.on_change_end;
                let global_own = own;
                track = track.child(
                    gpui::canvas(
                        |bounds, _, _| bounds,
                        move |_, _, window, _| {
                            let move_bounds = global_bounds.clone();
                            let move_dragging = global_dragging.clone();
                            let move_change = global_change.clone();
                            let move_own = global_own.clone();
                            window.on_mouse_event(
                                move |event: &MouseMoveEvent, phase, window, cx| {
                                    if phase == gpui::DispatchPhase::Capture
                                        && event.pressed_button == Some(gpui::MouseButton::Left)
                                        && *move_dragging.read(cx)
                                    {
                                        if let Some(next) = slider_color_from_pointer(
                                            &move_bounds,
                                            event.position,
                                            vertical,
                                            value,
                                            channel,
                                            space,
                                            cx,
                                        ) {
                                            if next.changed {
                                                report_color_change(
                                                    next.value,
                                                    &move_own,
                                                    &move_change,
                                                    window,
                                                    cx,
                                                );
                                            }
                                        }
                                    }
                                },
                            );

                            let up_bounds = global_bounds.clone();
                            let up_dragging = global_dragging.clone();
                            let up_end = global_end.clone();
                            window.on_mouse_event(
                                move |event: &MouseUpEvent, phase, window, cx| {
                                    if phase == gpui::DispatchPhase::Capture
                                        && event.button == gpui::MouseButton::Left
                                    {
                                        finish_slider_drag(
                                            &up_dragging,
                                            &up_bounds,
                                            event.position,
                                            vertical,
                                            value,
                                            channel,
                                            space,
                                            &up_end,
                                            window,
                                            cx,
                                        );
                                    }
                                },
                            );
                        },
                    )
                    .absolute()
                    .inset_0(),
                );
            }
        }

        // `useColorSlider` is `useSlider` with one thumb: `role: 'slider'`
        // on the interactive track. Its name is the explicit `aria_label`,
        // else the field `name`, else the channel it edits; its value is the
        // channel's position within that channel's own range.
        let slider_name = self
            .aria_label
            .clone()
            .or_else(|| self.name.clone())
            .unwrap_or_else(|| self.channel.localized_label(cx));
        let slider_step = if max - min > 2.0 { 1.0 } else { 0.01 };
        let value_text = match self.channel {
            ColorChannel::Hue => format!("{}°", raw.round()),
            ColorChannel::Red | ColorChannel::Green | ColorChannel::Blue => {
                format!("{}", raw.round())
            }
            _ => format!("{}%", (raw * 100.0).round()),
        };
        let thumb_range = a11y::Range::new(min as f64, max as f64, raw as f64)
            .step(slider_step)
            .text(Some(value_text));
        track = track
            .a11y_named(a11y::Role::Slider, &a11y::Name::maybe(Some(slider_name)))
            .a11y_range(&thumb_range)
            .a11y_orientation(self.orientation);
        if !self.is_disabled {
            track = util::record_focus_bounds(track, &focus_handle, window, cx);
        }

        if !self.show_label {
            return div().child(track);
        }

        let display = match self.channel {
            ColorChannel::Hue => format!("{}\u{00B0}", raw.round()),
            ColorChannel::Red | ColorChannel::Green | ColorChannel::Blue => {
                format!("{}", raw.round())
            }
            _ => format!("{}%", (raw * 100.0).round()),
        };
        let output = match &self.output {
            // `.color-slider__output`: v3's render prop is handed the colour,
            // which is what a caller needs to draw a swatch or a different
            // unit.
            Some(render) => render(self.value, &display),
            None => div()
                .text_color(cx.colors().muted)
                .child(display)
                .into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            // `.color-slider` is `grid w-full gap-1`.
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .w(self.length)
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(
                        div()
                            .text_color(cx.colors().foreground)
                            .child(self.channel.localized_label(cx)),
                    )
                    // The disabled root dims Output through status-disabled,
                    // while the stylesheet restores Label to full opacity.
                    .child(
                        div()
                            .when(self.is_disabled, |output| {
                                output.opacity(cx.layout().disabled_opacity)
                            })
                            .font_features(util::tabular_font_features())
                            .child(output),
                    ),
            )
            .child(track)
    }
}

pub(super) fn lightness_gradient_colors(
    value: PickerColor,
    color_space: ColorSpace,
    min: f32,
    max: f32,
) -> (Hsla, Hsla, Hsla) {
    (
        value
            .with_channel_in(ColorChannel::Lightness, color_space, min)
            .to_hsla(),
        value
            .with_channel_in(ColorChannel::Lightness, color_space, (max - min) / 2.0)
            .to_hsla(),
        value
            .with_channel_in(ColorChannel::Lightness, color_space, max)
            .to_hsla(),
    )
}

#[allow(clippy::float_cmp)] // snapped channel values are exact state coordinates
pub(super) fn slider_color_from_pointer(
    bounds: &Entity<Bounds<f32>>,
    position: gpui::Point<Pixels>,
    vertical: bool,
    value: PickerColor,
    channel: ColorChannel,
    color_space: ColorSpace,
    cx: &App,
) -> Option<PointerColor> {
    let bounds = *bounds.read(cx);
    let (reach, extent) = if vertical {
        (
            bounds.origin.y + bounds.size.height
                - COLOR_SLIDER_TRACK_INSET_PX
                - f32::from(position.y),
            bounds.size.height - COLOR_SLIDER_TRACK_INSET_PX * 2.0,
        )
    } else {
        (
            f32::from(position.x) - bounds.origin.x - COLOR_SLIDER_TRACK_INSET_PX,
            bounds.size.width - COLOR_SLIDER_TRACK_INSET_PX * 2.0,
        )
    };
    if extent <= 0.0 {
        return None;
    }
    let fraction = (reach / extent).clamp(0.0, 1.0);
    let (min, max) = channel.range();
    let next = snap_color_channel(channel, min + fraction * (max - min));
    Some(PointerColor {
        changed: next != value.channel_in(channel, color_space),
        value: value.with_channel_in(channel, color_space, next),
    })
}

#[allow(clippy::too_many_arguments)] // the channel and callback are the color-slider drag state
pub(super) fn finish_slider_drag(
    dragging: &Entity<bool>,
    bounds: &Entity<Bounds<f32>>,
    position: gpui::Point<Pixels>,
    vertical: bool,
    value: PickerColor,
    channel: ColorChannel,
    color_space: ColorSpace,
    on_change_end: &Option<OnColorChange>,
    window: &mut Window,
    cx: &mut App,
) {
    if !*dragging.read(cx) {
        return;
    }
    dragging.update(cx, |value, cx| {
        *value = false;
        cx.notify();
    });
    if let (Some(callback), Some(next)) = (
        on_change_end,
        slider_color_from_pointer(bounds, position, vertical, value, channel, color_space, cx),
    ) {
        callback(&next.value, window, cx);
    }
}

// ---------------------------------------------------------------------------
