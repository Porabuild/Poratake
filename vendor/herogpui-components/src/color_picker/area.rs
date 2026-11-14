//! ColorArea.

use super::*;

// ColorArea
// ---------------------------------------------------------------------------

/// State handed to `ColorArea.Thumb`'s render function.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ColorAreaThumbState {
    /// The color at the thumb.
    pub color: PickerColor,
    /// Whether the thumb is being dragged.
    pub is_dragging: bool,
    /// Whether the area is hovered.
    pub is_hovered: bool,
    /// Whether the area has focus.
    pub is_focused: bool,
    /// Whether focus is visible (keyboard focus).
    pub is_focus_visible: bool,
    /// Whether the area is disabled.
    pub is_disabled: bool,
}

const COLOR_AREA_THUMB_IDLE_PX: f32 = 16.0;
const COLOR_AREA_THUMB_DRAGGING_PX: f32 = 20.0;
const COLOR_AREA_THUMB_TRANSITION_MS: u64 = 150;

#[derive(Clone)]
pub(super) struct ColorAreaThumbMotion {
    dragging: bool,
    generation: usize,
    from: f32,
    size: Rc<Cell<f32>>,
}

pub(super) struct ColorAreaThumbMotionFrame {
    base: ElementId,
    generation: usize,
    from: f32,
    to: f32,
    size: Rc<Cell<f32>>,
    animate: bool,
}

impl ColorAreaThumbMotionFrame {
    fn render(self, thumb: gpui::Div) -> gpui::AnyElement {
        if !self.animate {
            self.size.set(self.to);
            return place_color_area_thumb(thumb, self.to).into_any_element();
        }

        let size = self.size;
        let from = self.from;
        let to = self.to;
        thumb
            .with_animation(
                element_id::indexed(&self.base, "thumb-size", self.generation),
                Animation::new(Duration::from_millis(COLOR_AREA_THUMB_TRANSITION_MS))
                    .with_easing(|t| crate::anim::Curve::Out.at(t)),
                move |thumb, delta| {
                    let next = from + (to - from) * delta;
                    size.set(next);
                    place_color_area_thumb(thumb, next)
                },
            )
            .into_any_element()
    }
}

pub(super) fn place_color_area_thumb(thumb: gpui::Div, size: f32) -> gpui::Div {
    let inset = (COLOR_AREA_THUMB_DRAGGING_PX - size) / 2.0;
    thumb.size(px(size)).ml(px(inset)).mt(px(inset))
}

pub(super) fn color_area_thumb_motion(
    id: &ElementId,
    dragging: bool,
    window: &mut Window,
    cx: &mut App,
) -> ColorAreaThumbMotionFrame {
    let state = window.use_keyed_state(element_id::scoped(id, "thumb-motion"), cx, |_, _| {
        ColorAreaThumbMotion {
            dragging,
            generation: 0,
            from: if dragging {
                COLOR_AREA_THUMB_DRAGGING_PX
            } else {
                COLOR_AREA_THUMB_IDLE_PX
            },
            size: Rc::new(Cell::new(if dragging {
                COLOR_AREA_THUMB_DRAGGING_PX
            } else {
                COLOR_AREA_THUMB_IDLE_PX
            })),
        }
    });
    let mut current = state.read(cx).clone();
    let to = if dragging {
        COLOR_AREA_THUMB_DRAGGING_PX
    } else {
        COLOR_AREA_THUMB_IDLE_PX
    };
    if current.dragging != dragging {
        current.dragging = dragging;
        current.generation = current.generation.wrapping_add(1);
        current.from = current.size.get();
        state.update(cx, |stored, _| *stored = current.clone());
    }
    if ActiveTheme::reduce_motion(cx) && (current.size.get() - to).abs() > f32::EPSILON {
        current.from = to;
        current.size.set(to);
        state.update(cx, |stored, _| *stored = current.clone());
    }
    ColorAreaThumbMotionFrame {
        base: id.clone(),
        generation: current.generation,
        from: current.from,
        to,
        size: current.size,
        animate: current.generation != 0
            && !ActiveTheme::reduce_motion(cx)
            && (current.from - to).abs() > f32::EPSILON,
    }
}

/// ColorArea — a two-dimensional gradient for picking two channels at once.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ColorArea {
    /// `defaultValue` — set it to hand this component its own state.
    default_value: Option<PickerColor>,
    id: ElementId,
    value: PickerColor,
    /// `colorSpace` — set explicitly, it selects the channel pair; an explicit
    /// `x_channel`/`y_channel` still wins.
    color_space: Option<ColorSpace>,
    x_channel: ColorChannel,
    y_channel: ColorChannel,
    width: Pixels,
    height: Pixels,
    is_disabled: bool,
    show_dots: bool,
    thumb: Option<Arc<dyn Fn(ColorAreaThumbState) -> gpui::AnyElement + 'static>>,
    on_change: Option<OnColorChange>,
    on_change_end: Option<OnColorChange>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl ColorArea {
    /// Creates a color area showing `value`.
    pub fn new(id: impl Into<ElementId>, value: PickerColor) -> Self {
        Self {
            default_value: None,
            id: id.into(),
            value,
            color_space: None,
            x_channel: ColorChannel::Saturation,
            y_channel: ColorChannel::Brightness,
            width: px(224.),
            height: px(224.),
            is_disabled: false,
            show_dots: false,
            thumb: None,
            on_change: None,
            on_change_end: None,
            sx: None,
        }
    }

    /// `defaultValue` — the uncontrolled initial colour.
    ///
    /// Supplying it hands the component its own state: the constructor's
    /// `value` becomes the seed, and a change moves the component's copy.
    pub fn default_value(mut self, value: PickerColor) -> Self {
        self.default_value = Some(value);
        self
    }

    /// `colorSpace` — the space whose channels the area edits.
    ///
    /// Sets both axes to that space's pair; call `x_channel`/`y_channel`
    /// afterwards to override either one.
    pub fn color_space(mut self, space: ColorSpace) -> Self {
        let (x, y) = space.area_channels();
        self.color_space = Some(space);
        self.x_channel = x;
        self.y_channel = y;
        self
    }

    /// Sets the channel edited along the horizontal axis.
    pub fn x_channel(mut self, channel: ColorChannel) -> Self {
        self.x_channel = channel;
        self
    }

    /// Sets the channel edited along the vertical axis.
    pub fn y_channel(mut self, channel: ColorChannel) -> Self {
        self.y_channel = channel;
        self
    }

    /// Sets the width and height of the area.
    pub fn size(mut self, width: impl Into<Pixels>, height: impl Into<Pixels>) -> Self {
        self.width = width.into();
        self.height = height.into();
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the area's root element after every value the channels and
    /// the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }

    /// Sets whether the area is disabled (`isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// `showDots` — overlays a dot grid for finer visual positioning.
    pub fn show_dots(mut self, v: bool) -> Self {
        self.show_dots = v;
        self
    }

    /// `ColorArea.Thumb`'s render function — the closure receives the live
    /// color and interaction state while the built-in thumb keeps ownership
    /// of positioning, focus and pointer behavior.
    pub fn thumb(
        mut self,
        render: impl Fn(ColorAreaThumbState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.thumb = Some(Arc::new(render));
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

    /// `onChangeEnd` — fires once when the drag finishes.
    pub fn on_change_end(
        mut self,
        handler: impl Fn(&PickerColor, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change_end = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for ColorArea {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `defaultValue` opts into the component holding its own colour;
        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        let (resolved, own) = util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "area-value"),
            match self.default_value {
                Some(_) => None,
                None => Some(self.value),
            },
            self.default_value.unwrap_or(self.value),
        );
        self.value = resolved;
        // `.color-area__thumb[data-focus-visible]` is `status-focused`.
        // `use_keyed_state` takes `cx` mutably, so the handle precedes the
        // theme.
        let area_focus =
            util::tab_stop_handle(element_id::scoped(&self.id, "area-focus"), window, cx);
        let bounds_slot =
            window.use_keyed_state(element_id::scoped(&self.id, "area-bounds"), cx, |_, _| {
                Bounds::<f32> {
                    origin: gpui::point(0., 0.),
                    size: gpui::size(0., 0.),
                }
            });
        let dragging =
            window.use_keyed_state(element_id::scoped(&self.id, "area-dragging"), cx, |_, _| {
                false
            });
        let thumb_hovered = window.use_keyed_state(
            element_id::scoped(&self.id, "area-thumb-hovered"),
            cx,
            |_, _| false,
        );
        let colors = cx.colors();
        let border_color = util::sx_border_color(&self.sx).unwrap_or(colors.border);
        // `.color-area` is `rounded-2xl`, which is `soft_radius`.
        let radius = util::soft_radius(cx);

        let (x_min, x_max) = self.x_channel.range();
        let (y_min, y_max) = self.y_channel.range();
        let color_space = self.color_space.unwrap_or_default();
        let x_norm = ((self.value.channel_in(self.x_channel, color_space) - x_min)
            / (x_max - x_min))
            .clamp(0.0, 1.0);
        let y_norm = ((self.value.channel_in(self.y_channel, color_space) - y_min)
            / (y_max - y_min))
            .clamp(0.0, 1.0);

        // Saturation left-to-right over the hue, brightness/lightness
        // bottom-to-top. RGB uses sampled vertical ramps because gpui has no
        // screen blend mode, which React Aria uses to combine its axes.
        let mut area = div()
            .id(self.id.clone())
            .when(!self.is_disabled, |el| el.track_focus(&area_focus))
            .relative()
            .w(self.width)
            .h(self.height)
            .rounded(radius);

        // `.color-area` is `overflow: visible` -- the thumb is meant to hang
        // over the edge, and upstream can allow that because its gradient stack
        // is the element's own `background` plus an `::after`, both of which
        // take the radius. Here the stack is real children, so the clip that
        // holds them inside the corner goes on this one inner layer rather than
        // on the area: clipping the area cut the thumb in half at every edge.
        let mut layers = div().absolute().inset_0().rounded(radius).overflow_hidden();

        // The gradient stack stays div fills, each carrying the area radius
        // itself: GPUI's `Svg` element renders through an alpha mask, so a
        // multicolor gradient SVG can only ever paint a monochrome
        // silhouette (`window.rs`: `render_alpha_mask`). The single-curve
        // alternative does not exist on vanilla; the per-layer radii below
        // coincide because every layer shares one box and one radius value.
        layers =
            if self.x_channel == ColorChannel::Hue || self.y_channel == ColorChannel::Hue {
                layers.child(color_area_hue_layers(
                    self.value,
                    color_space,
                    self.x_channel,
                    self.y_channel,
                    radius,
                ))
            } else {
                match (color_space, self.x_channel, self.y_channel) {
                    (ColorSpace::Hsb, ColorChannel::Saturation, ColorChannel::Brightness) => layers
                        .bg(gpui::linear_gradient(
                            90.0,
                            gpui::linear_color_stop(gpui::white(), 0.0),
                            gpui::linear_color_stop(
                                PickerColor::hsb(self.value.hue, 1.0, 1.0).to_hsla(),
                                1.0,
                            ),
                        ))
                        .child(div().absolute().inset_0().rounded(radius).bg(
                            gpui::linear_gradient(
                                180.0,
                                gpui::linear_color_stop(gpui::transparent_black(), 0.0),
                                gpui::linear_color_stop(gpui::black(), 1.0),
                            ),
                        )),
                    (ColorSpace::Hsl, ColorChannel::Saturation, ColorChannel::Lightness) => {
                        let gray = self.value.with_hsl_channels(0.0, 0.5).to_hsla();
                        let hue = self.value.with_hsl_channels(1.0, 0.5).to_hsla();
                        layers
                            .bg(gpui::linear_gradient(
                                90.0,
                                gpui::linear_color_stop(gray, 0.0),
                                gpui::linear_color_stop(hue, 1.0),
                            ))
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .right_0()
                                    .rounded(radius)
                                    .h(px(f32::from(self.height) / 2.0))
                                    .bg(gpui::linear_gradient(
                                        180.0,
                                        gpui::linear_color_stop(gpui::white(), 0.0),
                                        gpui::linear_color_stop(gpui::transparent_white(), 1.0),
                                    )),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .bottom_0()
                                    .left_0()
                                    .right_0()
                                    .rounded(radius)
                                    .h(px(f32::from(self.height) / 2.0))
                                    .bg(gpui::linear_gradient(
                                        180.0,
                                        gpui::linear_color_stop(gpui::transparent_black(), 0.0),
                                        gpui::linear_color_stop(gpui::black(), 1.0),
                                    )),
                            )
                    }
                    _ => layers.child(color_area_channel_grid(
                        self.value,
                        color_space,
                        self.x_channel,
                        self.y_channel,
                        radius,
                        self.width,
                        self.height,
                    )),
                }
            };

        // `showDots` — the dot-grid overlay. gpui has no repeating background,
        // so the grid is drawn as rows of small translucent dots. Dots sit on
        // 8px cell centers, clear of the corner curve.
        if self.show_dots {
            const STEP: f32 = 8.0;
            let cols = (f32::from(self.width) / STEP).floor().max(1.0) as usize;
            let rows = (f32::from(self.height) / STEP).floor().max(1.0) as usize;
            let mut grid = div().absolute().inset_0().flex().flex_col();
            for r in 0..rows {
                let mut line = div().flex().h(px(STEP));
                for c in 0..cols {
                    let _ = c;
                    line = line.child(
                        div()
                            .w(px(STEP))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .size(px(2.))
                                    .rounded_full()
                                    .bg(gpui::white().alpha(0.25)),
                            ),
                    );
                }
                let _ = r;
                grid = grid.child(line);
            }
            layers = layers.child(grid);
        }

        let recorder_bounds = bounds_slot.clone();
        area = area.child(
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

        area = area.child(layers);

        // GPUI paints an element's border after its children. Keeping the
        // visible border as a child lets the thumb, which is appended after
        // this overlay, remain above the border at the edge of the area.
        // The gradient stack remains clipped by `layers`, while the root
        // itself stays overflow-visible so a boundary thumb can hang over it.
        area = area.child(
            div()
                .absolute()
                .inset_0()
                .rounded(radius)
                .border(cx.layout().border_width)
                .border_color(border_color)
                .shadow(vec![color_inner_shadow()]),
        );

        let is_dragging = !self.is_disabled && *dragging.read(cx);
        let is_focused = !self.is_disabled && area_focus.is_focused(window);
        let is_focus_visible = is_focused && util::focus_visible(cx);
        let thumb_state = ColorAreaThumbState {
            color: self.value,
            is_dragging,
            is_hovered: !self.is_disabled && *thumb_hovered.read(cx),
            is_focused,
            is_focus_visible,
            is_disabled: self.is_disabled,
        };
        let thumb_content = self.thumb.as_ref().map(|render| render(thumb_state));
        let thumb_motion = color_area_thumb_motion(&self.id, is_dragging, window, cx);
        // The ring goes on the visual child, not on the 20px hover wrapper:
        // this is the element that carries the thumb's `rounded()` and whose
        // circle the ring has to stay concentric with. The wrapper is square
        // and larger, so a ring on it would read as a box around the knob.
        let thumb_visual = util::with_focus_ring_overlay(
            div()
                // `.color-area__thumb` is `rounded-xl`, which is circular at
                // both the idle and dragging sizes.
                .rounded(px(12.))
                // `.color-area__thumb` is `border: 3px solid white`.
                .border(px(3.))
                .border_color(gpui::white())
                .shadow(color_thumb_shadows())
                .bg(self.value.to_hsla()),
            is_focus_visible,
            true,
            px(12.),
            Vec::new(),
            cx,
        );
        // The stable 20px wrapper owns hover. The changing animation id lives
        // on its listener-free visual child, so a drag transition cannot drop
        // the interaction path or a caller-provided child.
        let mut thumb = div()
            .id(element_id::scoped(&self.id, "area-thumb"))
            .absolute()
            .left(px(
                f32::from(self.width) * x_norm - COLOR_AREA_THUMB_DRAGGING_PX / 2.0
            ))
            .top(px(
                f32::from(self.height) * (1.0 - y_norm) - COLOR_AREA_THUMB_DRAGGING_PX / 2.0
            ))
            .size(px(COLOR_AREA_THUMB_DRAGGING_PX))
            .child(thumb_motion.render(thumb_visual))
            .when_some(thumb_content, |thumb, content| thumb.child(content));
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
        area = area.child(thumb);

        // `useColorArea` hands screen readers two slider inputs — one per
        // axis, x horizontal and y vertical, each named by its channel with a
        // numeric value/min/max. GPUI's area is a single operable surface whose
        // keys already move both axes, so these children carry each channel's
        // slider semantics for assistive tech rather than adding a second tab
        // stop per axis; they paint nothing and take no input.
        let x_value = self.value.channel_in(self.x_channel, color_space) as f64;
        let y_value = self.value.channel_in(self.y_channel, color_space) as f64;
        let x_range = a11y::Range::new(x_min as f64, x_max as f64, x_value);
        let y_range = a11y::Range::new(y_min as f64, y_max as f64, y_value);
        area = area
            .child(
                div()
                    .id(element_id::scoped(&self.id, "a11y-x"))
                    .absolute()
                    .inset_0()
                    .a11y_named(
                        a11y::Role::Slider,
                        &a11y::Name::maybe(Some(self.x_channel.localized_label(cx))),
                    )
                    .a11y_range(&x_range)
                    .a11y_orientation(herogpui_core::Orientation::Horizontal),
            )
            .child(
                div()
                    .id(element_id::scoped(&self.id, "a11y-y"))
                    .absolute()
                    .inset_0()
                    .a11y_named(
                        a11y::Role::Slider,
                        &a11y::Name::maybe(Some(self.y_channel.localized_label(cx))),
                    )
                    .a11y_range(&y_range)
                    .a11y_orientation(herogpui_core::Orientation::Vertical),
            );

        if self.is_disabled {
            return area.opacity(cx.layout().disabled_opacity);
        }

        // v3's ColorArea inherits React Aria's keyboard: the arrows move the
        // thumb, left/right on the x channel and up/down on the y, and Page
        // Up/Down move the y while Home/End move the x by the page step
        // (React Aria's `useColorArea` shortcuts step by the page, not to the
        // edge). A two-axis control that answered no key could only be moved
        // with the pointer; the step sizes are ColorSlider's own, so the two
        // colour controls agree.
        let keys_value = self.value;
        let on_change_keys = self.on_change.clone();
        let end_keys = self.on_change_end.clone();
        let own_keys = own.clone();
        let (x_channel, y_channel) = (self.x_channel, self.y_channel);
        let key_space = color_space;
        let (x_min, x_max) = x_channel.range();
        let (y_min, y_max) = y_channel.range();
        // One step per unit for a wide channel (hue, an 8-bit byte), a
        // percentage point for the normalised ones -- the same rule
        // ColorSlider's keys use.
        let x_step = if x_max - x_min > 2.0 { 1.0 } else { 0.01 };
        let y_step = if y_max - y_min > 2.0 { 1.0 } else { 0.01 };
        // A tenth of the range, never less than a step -- React Aria's page.
        let x_page = ((x_max - x_min) / 10.0).max(x_step);
        let y_page = ((y_max - y_min) / 10.0).max(y_step);
        area = area
            .key_context("ColorArea")
            .on_key_down(move |event, window, cx| {
                let x_now = keys_value.channel_in(x_channel, key_space);
                let y_now = keys_value.channel_in(y_channel, key_space);
                let (nx, ny) = match event.keystroke.key.as_str() {
                    "left" => (x_now - x_step, y_now),
                    "right" => (x_now + x_step, y_now),
                    "down" => (x_now, y_now - y_step),
                    "up" => (x_now, y_now + y_step),
                    "pageup" => (x_now, y_now + y_page),
                    "pagedown" => (x_now, y_now - y_page),
                    "home" => (x_now - x_page, y_now),
                    "end" => (x_now + x_page, y_now),
                    _ => return,
                };
                let next = keys_value
                    .with_channel_in(x_channel, key_space, nx.clamp(x_min, x_max))
                    .with_channel_in(y_channel, key_space, ny.clamp(y_min, y_max));
                if next == keys_value {
                    return;
                }
                // Uncontrolled: move our own copy, as the pointer path does.
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
                // with it rather than waiting for a release that never comes.
                if let Some(cb) = &end_keys {
                    cb(&next, window, cx);
                }
            });

        if self.on_change.is_some() || self.on_change_end.is_some() || own.is_some() {
            let down_bounds = bounds_slot.clone();
            let down_dragging = dragging.clone();
            let down_focus = area_focus.clone();
            let down_change = self.on_change.clone();
            let down_own = own.clone();
            let down_value = self.value;
            let (x_channel, y_channel) = (self.x_channel, self.y_channel);
            area = area.cursor(util::interactive_cursor(cx)).on_mouse_down(
                gpui::MouseButton::Left,
                move |event: &MouseDownEvent, window, cx| {
                    if event.modifiers.alt || event.modifiers.control || event.modifiers.platform {
                        return;
                    }
                    down_dragging.update(cx, |value, cx| {
                        if !*value {
                            *value = true;
                            cx.notify();
                        }
                    });
                    window.focus(&down_focus, cx);
                    if let Some(next) = area_color_from_pointer(
                        &down_bounds,
                        event.position,
                        down_value,
                        color_space,
                        x_channel,
                        y_channel,
                        cx,
                    ) {
                        if next.changed {
                            report_color_change(next.value, &down_own, &down_change, window, cx);
                        }
                    }
                },
            );

            let global_bounds = bounds_slot;
            let global_dragging = dragging;
            let global_change = self.on_change;
            let global_end = self.on_change_end;
            let global_own = own;
            let global_value = self.value;
            area = area.child(
                gpui::canvas(
                    |bounds, _, _| bounds,
                    move |_, _, window, _| {
                        let move_bounds = global_bounds.clone();
                        let move_dragging = global_dragging.clone();
                        let move_change = global_change.clone();
                        let move_own = global_own.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase == gpui::DispatchPhase::Capture
                                && event.pressed_button == Some(gpui::MouseButton::Left)
                                && *move_dragging.read(cx)
                            {
                                if let Some(next) = area_color_from_pointer(
                                    &move_bounds,
                                    event.position,
                                    global_value,
                                    color_space,
                                    x_channel,
                                    y_channel,
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
                        });

                        let up_bounds = global_bounds.clone();
                        let up_dragging = global_dragging.clone();
                        let up_end = global_end.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                            if phase == gpui::DispatchPhase::Capture
                                && event.button == gpui::MouseButton::Left
                            {
                                finish_area_drag(
                                    &up_dragging,
                                    &up_bounds,
                                    event.position,
                                    global_value,
                                    color_space,
                                    x_channel,
                                    y_channel,
                                    &up_end,
                                    window,
                                    cx,
                                );
                            }
                        });
                    },
                )
                .absolute()
                .inset_0(),
            );
        }

        area = util::apply_sx(area, &self.sx);
        if !self.is_disabled {
            area = util::record_focus_bounds(area, &area_focus, window, cx);
        }
        // `useColorArea` is `role: 'group'` on the area; the thumb is
        // `role: 'presentation'` and has no node of its own.
        area.a11y(a11y::Role::Group)
    }
}

#[allow(clippy::too_many_arguments)] // the two axes and callback are the color-area drag state
pub(super) fn finish_area_drag(
    dragging: &Entity<bool>,
    bounds: &Entity<Bounds<f32>>,
    position: gpui::Point<Pixels>,
    value: PickerColor,
    color_space: ColorSpace,
    x_channel: ColorChannel,
    y_channel: ColorChannel,
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
        area_color_from_pointer(
            bounds,
            position,
            value,
            color_space,
            x_channel,
            y_channel,
            cx,
        ),
    ) {
        callback(&next.value, window, cx);
    }
}

/// The hue-axis gradient stack: hue bands plus the other channel's overlay.
///
/// Each painted layer carries the area radius itself: vanilla GPUI clips
/// `overflow_hidden()` to the rectangle (see `util::inner_fill_radius`).
pub(super) fn color_area_hue_layers(
    value: PickerColor,
    color_space: ColorSpace,
    x_channel: ColorChannel,
    y_channel: ColorChannel,
    radius: Pixels,
) -> gpui::Div {
    let hue_is_vertical = y_channel == ColorChannel::Hue;
    let other = if x_channel == ColorChannel::Hue {
        y_channel
    } else {
        x_channel
    };
    let other_is_vertical = y_channel == other;
    let base = match (color_space, other) {
        (ColorSpace::Hsl, ColorChannel::Saturation) => value
            .with_hsl_channels(1.0, value.hsl_lightness())
            .with_alpha(1.0),
        (ColorSpace::Hsl, ColorChannel::Lightness) => value
            .with_hsl_channels(value.hsl_saturation(), 0.5)
            .with_alpha(1.0),
        (ColorSpace::Hsb, ColorChannel::Saturation) => PickerColor::hsb(0.0, 1.0, value.brightness),
        (ColorSpace::Hsb, ColorChannel::Brightness) => PickerColor::hsb(0.0, value.saturation, 1.0),
        _ => PickerColor::hsb(0.0, 1.0, 1.0),
    };

    let mut layers = div().absolute().inset_0().child(hue_gradient(
        base,
        color_space,
        hue_is_vertical,
        radius,
        0.0,
    ));
    layers = match other {
        ColorChannel::Saturation => {
            let start = base
                .with_channel_in(other, color_space, other.range().0)
                .to_hsla();
            layers.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(radius)
                    .bg(gpui::linear_gradient(
                        if other_is_vertical { 0.0 } else { 90.0 },
                        gpui::linear_color_stop(start, 0.0),
                        gpui::linear_color_stop(start.alpha(0.0), 1.0),
                    )),
            )
        }
        ColorChannel::Brightness => layers.child(div().absolute().inset_0().rounded(radius).bg(
            gpui::linear_gradient(
                if other_is_vertical { 0.0 } else { 90.0 },
                gpui::linear_color_stop(gpui::black(), 0.0),
                gpui::linear_color_stop(gpui::transparent_black(), 1.0),
            ),
        )),
        ColorChannel::Lightness => layers.child(three_stop_gradient(
            other_is_vertical,
            gpui::black(),
            gpui::transparent_black(),
            gpui::white(),
            radius,
            0.0,
        )),
        _ => layers,
    };
    layers
}

/// Six hue bands laid across the box.
///
/// `inset` is the fraction of the box at each end that must hold a constant
/// colour (0.0 for `ColorArea`, `cap / length` for `ColorSlider`). The bands
/// travel over `[inset, 1 - inset]`, but the first and last bands are
/// *stretched* out to the box edges and compensate with stop percentages, so
/// the end zones are flat colour without any extra element: GPUI's shader
/// remaps `t` by `(t - stop0) / (stop1 - stop0)` and clamps to `[0, 1]`, which
/// holds the end colour over the stretched part. Painting a separate cap
/// instead would break under `opacity()` (GPUI applies opacity per element,
/// not per group) and could not carry the r10 curve anyway, because GPUI
/// clamps a corner radius to half the element's shortest side.
///
/// The first and last bands own the exterior corners: vanilla GPUI clips
/// `overflow_hidden()` to the rectangle (see `util::inner_fill_radius`), so
/// each paints its outer pair. Interior bands stay square under their
/// neighbours. Band 0 is the bottom sixth (vertical) or the left sixth
/// (horizontal).
pub(super) fn hue_gradient(
    value: PickerColor,
    color_space: ColorSpace,
    vertical: bool,
    radius: Pixels,
    inset: f32,
) -> gpui::Div {
    let stops = hue_stop_colors(value, color_space);
    let mut gradient = div().absolute().inset_0();
    for index in 0..6 {
        let (first, last) = (index == 0, index == 5);
        let offset = hue_band_offset(index, vertical, inset);
        let extent = hue_band_extent(index, inset);
        let (from, to) = hue_band_stop_percentages(index, inset);
        gradient = if vertical {
            gradient.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(gpui::relative(offset))
                    .h(gpui::relative(extent))
                    .when(first, |band| band.rounded_bl(radius).rounded_br(radius))
                    .when(last, |band| band.rounded_tl(radius).rounded_tr(radius))
                    .bg(gpui::linear_gradient(
                        0.0,
                        gpui::linear_color_stop(stops[index], from),
                        gpui::linear_color_stop(stops[index + 1], to),
                    )),
            )
        } else {
            gradient.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(gpui::relative(offset))
                    .w(gpui::relative(extent))
                    .when(first, |band| band.rounded_tl(radius).rounded_bl(radius))
                    .when(last, |band| band.rounded_tr(radius).rounded_br(radius))
                    .bg(gpui::linear_gradient(
                        90.0,
                        gpui::linear_color_stop(stops[index], from),
                        gpui::linear_color_stop(stops[index + 1], to),
                    )),
            )
        };
    }
    gradient
}

/// Fraction of the box spanned by hue band `index` under `inset`.
pub(super) fn hue_band_extent(index: usize, inset: f32) -> f32 {
    let (start, end) = hue_band_span(index, inset);
    end - start
}

/// `[start, end]` of hue band `index` measured from the band-0 end of the box.
fn hue_band_span(index: usize, inset: f32) -> (f32, f32) {
    let inset = inset.clamp(0.0, 0.45);
    let width = (1.0 - inset * 2.0) / 6.0;
    let start = if index == 0 {
        0.0
    } else {
        inset + index as f32 * width
    };
    let end = if index == 5 {
        1.0
    } else {
        inset + (index + 1) as f32 * width
    };
    (start, end)
}

/// Gradient stop percentages for hue band `index`. The stretched end bands
/// keep their travel unchanged and hold their outer colour over the inset.
fn hue_band_stop_percentages(index: usize, inset: f32) -> (f32, f32) {
    let inset = inset.clamp(0.0, 0.45);
    let width = (1.0 - inset * 2.0) / 6.0;
    let total = width + inset;
    if total <= 0.0 {
        return (0.0, 1.0);
    }
    match index {
        0 => (inset / total, 1.0),
        5 => (0.0, width / total),
        _ => (0.0, 1.0),
    }
}

/// CSS-side offset of hue band `index`: `top` when vertical (band 0 is the
/// bottom band, so the axis is mirrored), `left` when horizontal.
pub(super) fn hue_band_offset(index: usize, vertical: bool, inset: f32) -> f32 {
    let (start, end) = hue_band_span(index, inset);
    if vertical {
        1.0 - end
    } else {
        start
    }
}

pub(super) fn hue_stop_colors(value: PickerColor, color_space: ColorSpace) -> [Hsla; 7] {
    [0.0, 60.0, 120.0, 180.0, 240.0, 300.0, 360.0].map(|hue| {
        value
            .with_channel_in(ColorChannel::Hue, color_space, hue)
            .to_hsla()
    })
}

/// Vertical distance a strip must give up at horizontal distance `d` from a
/// rounded edge so it stays strictly inside a corner of radius `r`.
///
/// `r - sqrt(r^2 - (r - d)^2)`: the full radius at the very edge, zero once
/// the strip clears the corner zone, monotone decreasing in between.
pub(super) fn corner_arc_inset(d: f32, r: f32) -> f32 {
    if r <= 0.0 || d >= r {
        return 0.0;
    }
    let d = d.max(0.0);
    let leg = r - d;
    r - (r * r - leg * leg).max(0.0).sqrt()
}

/// Sampled channel grid: 64 vertical strips, each its own bottom-to-top
/// gradient.
///
/// The exterior corners cannot ride on the edge strips. A strip is
/// `width / 64` wide -- 2.5px on the default 160px area -- and GPUI clamps a
/// corner radius to half the element's shortest side, so a strip's own radius
/// collapses to about a pixel and all four corners render square. Instead a
/// base strip `2 * radius` wide sits at each edge, sampled at that edge's `x`
/// and wide enough to escape the clamp, and the 64 sampled strips paint over
/// it. Strips inside a corner zone are pulled in vertically by
/// `corner_arc_inset` at their outermost `x`, so they never poke past the arc;
/// the base shows through those slivers, at most a couple of samples' worth of
/// colour away from the strip it backs.
pub(super) fn color_area_channel_grid(
    value: PickerColor,
    color_space: ColorSpace,
    x_channel: ColorChannel,
    y_channel: ColorChannel,
    radius: Pixels,
    width: Pixels,
    height: Pixels,
) -> gpui::Div {
    const STRIPS: usize = 64;
    let (x_min, x_max) = x_channel.range();
    let (y_min, y_max) = y_channel.range();
    let width_px = f32::from(width).max(1.0);
    let height_px = f32::from(height).max(1.0);
    let radius_px = f32::from(radius)
        .max(0.0)
        .min(width_px / 2.0)
        .min(height_px / 2.0);
    // `inset_px` is how far the strip is shortened at each end. Its gradient
    // must still map bottom-to-top over the FULL area height, so the stops
    // are pushed past the shortened box: the shader remaps
    // `t = (t - p0) / (p1 - p0)` and clamps, which lands `t = 0` at
    // `inset / height` and `t = 1` at `1 - inset / height`.
    let ramp_at = |x: f32, inset_px: f32| {
        let at_x = value.with_channel_in(x_channel, color_space, x_min + x * (x_max - x_min));
        let bottom = at_x
            .with_channel_in(y_channel, color_space, y_min)
            .to_hsla();
        let top = at_x
            .with_channel_in(y_channel, color_space, y_max)
            .to_hsla();
        let visible = (height_px - 2.0 * inset_px).max(1.0);
        let overshoot = inset_px / visible;
        gpui::linear_gradient(
            0.0,
            gpui::linear_color_stop(bottom, -overshoot),
            gpui::linear_color_stop(top, 1.0 + overshoot),
        )
    };

    let base_w = px((radius_px * 2.0).min(width_px));
    // The base only ever shows in the corner slivers, which lie within
    // `radius` of the edge, so sample it at the middle of that zone rather
    // than at the edge itself: the colour error across the sliver halves.
    let base_sample = (radius_px / 2.0 / width_px).min(0.5);
    let mut grid = div()
        .absolute()
        .inset_0()
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .w(base_w)
                .rounded_tl(radius)
                .rounded_bl(radius)
                .bg(ramp_at(base_sample, 0.0)),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .right_0()
                .w(base_w)
                .rounded_tr(radius)
                .rounded_br(radius)
                .bg(ramp_at(1.0 - base_sample, 0.0)),
        );

    let strip_w = width_px / STRIPS as f32;
    for index in 0..STRIPS {
        let x = index as f32 / (STRIPS - 1) as f32;
        let left = index as f32 * strip_w;
        // The outermost x of the strip is its own outer edge.
        let distance = left.min(width_px - (left + strip_w));
        let inset_px = corner_arc_inset(distance, radius_px);
        let inset = px(inset_px);
        grid = grid.child(
            div()
                .absolute()
                .left(px(left))
                .w(px(strip_w))
                .top(inset)
                .bottom(inset)
                .bg(ramp_at(x, inset_px)),
        );
    }
    grid
}

pub(super) struct PointerColor {
    pub(super) value: PickerColor,
    pub(super) changed: bool,
}

#[allow(clippy::float_cmp)] // snapped channel values are exact state coordinates
pub(super) fn area_color_from_pointer(
    bounds: &Entity<Bounds<f32>>,
    position: gpui::Point<Pixels>,
    value: PickerColor,
    color_space: ColorSpace,
    x_channel: ColorChannel,
    y_channel: ColorChannel,
    cx: &App,
) -> Option<PointerColor> {
    let bounds = *bounds.read(cx);
    if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
        return None;
    }
    let x = ((f32::from(position.x) - bounds.origin.x) / bounds.size.width).clamp(0.0, 1.0);
    let y = ((f32::from(position.y) - bounds.origin.y) / bounds.size.height).clamp(0.0, 1.0);
    let (x_min, x_max) = x_channel.range();
    let (y_min, y_max) = y_channel.range();
    let x_value = snap_color_channel(x_channel, x_min + x * (x_max - x_min));
    let y_value = snap_color_channel(y_channel, y_min + (1.0 - y) * (y_max - y_min));
    Some(PointerColor {
        changed: x_value != value.channel_in(x_channel, color_space)
            || y_value != value.channel_in(y_channel, color_space),
        value: value
            .with_channel_in(x_channel, color_space, x_value)
            .with_channel_in(y_channel, color_space, y_value),
    })
}

pub(super) fn report_color_change(
    next: PickerColor,
    own: &Option<Entity<PickerColor>>,
    on_change: &Option<OnColorChange>,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(held) = own {
        held.update(cx, |value, cx| {
            *value = next;
            cx.notify();
        });
    }
    if let Some(callback) = on_change {
        callback(&next, window, cx);
    }
}

pub(super) fn live_color_form_state(
    value: crate::form::FormValue,
) -> Rc<RefCell<crate::form::LiveFormFieldState>> {
    Rc::new(RefCell::new(crate::form::LiveFormFieldState {
        commit_validation: None,
        reset_validation: None,
        value,
        is_invalid: false,
        is_successful: true,
        focus: None,
        restore: None,
    }))
}

pub(super) fn sync_color_form_state(
    state: &Rc<RefCell<crate::form::LiveFormFieldState>>,
    value: crate::form::FormValue,
    is_successful: bool,
    is_invalid: bool,
) {
    let mut state = state.borrow_mut();
    state.value = value;
    state.is_successful = is_successful;
    state.is_invalid = is_invalid;
}

/// React Aria ColorSlider submits the channel number of a hidden range input.
pub(super) fn color_slider_form_value(
    value: PickerColor,
    channel: ColorChannel,
    space: ColorSpace,
) -> crate::form::FormValue {
    let value = value.channel_in(channel, space);
    let value = if matches!(
        channel,
        ColorChannel::Saturation | ColorChannel::Brightness | ColorChannel::Lightness
    ) {
        value * 100.0
    } else {
        value
    };
    crate::form::FormValue::Number(f64::from(value))
}

/// React Aria ColorField submits the hex text, or the channel number when
/// `channel` is set (ColorChannelField is a NumberField).
pub(super) fn color_field_form_value(
    value: Option<PickerColor>,
    channel: Option<ColorChannel>,
    space: ColorSpace,
) -> crate::form::FormValue {
    let Some(value) = value else {
        return crate::form::FormValue::Text(SharedString::default());
    };
    match channel {
        None => crate::form::FormValue::Text(value.to_hex().into()),
        Some(channel) => {
            crate::form::FormValue::Number(f64::from(value.channel_in(channel, space)))
        }
    }
}

pub(super) fn color_field_display_text(
    value: Option<PickerColor>,
    channel: Option<ColorChannel>,
    space: ColorSpace,
) -> String {
    let Some(value) = value else {
        return String::new();
    };
    match channel {
        None => value.to_hex(),
        Some(channel) => format_color_channel_value(value, channel, space),
    }
}

// ---------------------------------------------------------------------------

crate::util::impl_component_styled!(ColorArea);
