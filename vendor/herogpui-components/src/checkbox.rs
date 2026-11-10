//! Checkbox — port of `@heroui/checkbox`.

use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui::{
    prelude::*, px, AnimationExt, AnyElement, App, IntoElement, ParentElement, Pixels, RenderOnce,
    StatefulInteractiveElement, Styled, Window,
};
use herogpui_core::{element_id, Color};
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};
use crate::anim::Tween;

/// Field state handed to Checkbox's children and indicator render functions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CheckboxState {
    /// Whether the checkbox is selected.
    pub is_selected: bool,
    /// Whether the checkbox shows the indeterminate dash.
    pub is_indeterminate: bool,
    /// Whether the checkbox is disabled.
    pub is_disabled: bool,
    /// Whether the checkbox is read-only.
    pub is_read_only: bool,
    /// Whether the checkbox is invalid.
    pub is_invalid: bool,
    /// Whether the checkbox is required.
    pub is_required: bool,
}

// ---------------------------------------------------------------------------
// Selection motion
// ---------------------------------------------------------------------------
//
// The pinned v3.2.4 stylesheet animates a tick in three layers, each with its
// own duration and curve. `.checkbox__control::before` — the accent fill —
// runs `scale 100ms var(--ease-linear)` from `scale-70` and
// `opacity 200ms var(--ease-linear)` from `opacity-0`, over a
// `background-color 200ms var(--ease-out)` that the control's own background
// shares while indeterminate. The checkmark SVG carries `strokeDasharray: 22`
// and jumps `strokeDashoffset` from 66 (hidden) to 44 (drawn) in its JSX;
// selected, that offset runs `150ms linear` after `15ms`, and unselecting
// falls back to the base `transition-all duration-200`. gpui has no
// stroke-dashoffset, but its `PathBuilder` strokes a polyline, so the mark is
// drawn on a canvas up to the same revealed fraction.

/// `opacity 200ms var(--ease-linear)` on `.checkbox__control::before`.
const FILL_FADE_MS: u64 = 200;
/// `scale 100ms var(--ease-linear)` on the same pseudo-element, from
/// `scale-70`.
const FILL_SCALE_MS: u64 = 100;
/// `scale-70` — the fill's resting scale.
const FILL_REST_SCALE: f32 = 0.7;
/// `background-color 200ms var(--ease-out)` — the colour ease both animated
/// background layers ride: the fill's hover swap and the control's own
/// indeterminate/pressed change.
const FILL_BG_MS: u64 = 200;
/// Selected, the checkmark draws over `stroke-dashoffset 150ms linear` after a
/// `15ms` delay.
const CHECK_DRAW_MS: u64 = 150;
const CHECK_DRAW_DELAY_MS: u64 = 15;
/// Unselected it undraws over the base `transition-all duration-200`.
const CHECK_UNDRAW_MS: u64 = 200;

/// The pinned checkmark geometry: viewBox `0 0 17 18` and polyline
/// `1 9 7 14 15 4` from `@heroui/react` 3.2.4's `checkbox.js`, stroked at the
/// `stroke-[2.5px]` its stylesheet lays on the checkmark slot — the JSX
/// itself says 2. The dash reveal needs exactly this polyline — its length
/// stays under the 22-unit `strokeDasharray`, so the drawn state shows the
/// whole stroke — and the checkbox therefore strokes it on a canvas rather
/// than rendering the shared 24-unit `icons::CHECK` asset, whose tip the
/// 22-unit dash would clip.
const CHECK_VIEWBOX: (f32, f32) = (17., 18.);
const CHECK_POLYLINE: [(f32, f32); 3] = [(1., 9.), (7., 14.), (15., 4.)];
const CHECK_STROKE: f32 = 2.5;
/// How much stroke the CSS slide uncovers: `strokeDashoffset` runs 66 → 44,
/// so the visible dash grows by 22 units — overshooting this ~20.6-unit
/// polyline, whose tip the drawn state clamps at.
const CHECK_DASH_UNITS: f32 = 22.;

/// Animate the painted fill itself: GPUI's overflow clip is rectangular,
/// so rounding a transparent parent does not round its background child.
fn fill_layer(
    id: &gpui::ElementId,
    opacity: Tween<f32>,
    scale: Tween<f32>,
    reduce_motion: bool,
    radius: Pixels,
    box_px: Pixels,
    background: Tween<gpui::Hsla>,
) -> AnyElement {
    let (opacity_to, scale_to) = (opacity.target(), scale.target());
    let color_to = background.target();
    let base = gpui::div().absolute();
    if !opacity.animates(reduce_motion)
        && !scale.animates(reduce_motion)
        && !background.animates(reduce_motion)
    {
        opacity.settle();
        scale.settle();
        background.settle();
        let inset = box_px * (1.0 - scale_to) / 2.0;
        return base
            .left(inset)
            .top(inset)
            .right(inset)
            .bottom(inset)
            .rounded(radius * scale_to)
            .opacity(opacity_to)
            .bg(color_to)
            .into_any_element();
    }

    let color_from = background.from();
    let color = background.value();
    let colored = base.with_animation(
        element_id::indexed(id, "fill-color", background.generation()),
        gpui::Animation::new(Duration::from_millis(FILL_BG_MS))
            .with_easing(|t| crate::anim::Curve::Out.at(t)),
        move |el, delta| {
            let value = if delta >= 1.0 {
                color_to
            } else {
                herogpui_core::mix_oklab(color_from, color_to, delta)
            };
            color.set(value);
            el.bg(value)
        },
    );
    let scale_from = scale.from();
    let scale_value = scale.value();
    let scaled = colored.with_animation(
        element_id::indexed(id, "fill-scale", scale.generation()),
        gpui::Animation::new(Duration::from_millis(FILL_SCALE_MS))
            .with_easing(|t| crate::anim::Curve::Linear.at(t)),
        move |el, delta| {
            let value = scale_from + (scale_to - scale_from) * delta;
            scale_value.set(value);
            let inset = box_px * (1.0 - value) / 2.0;
            el.map_element(|fill| {
                fill.left(inset)
                    .top(inset)
                    .right(inset)
                    .bottom(inset)
                    .rounded(radius * value)
            })
        },
    );
    let opacity_from = opacity.from();
    let opacity_value = opacity.value();
    scaled
        .with_animation(
            element_id::indexed(id, "fill-fade", opacity.generation()),
            gpui::Animation::new(Duration::from_millis(FILL_FADE_MS))
                .with_easing(|t| crate::anim::Curve::Linear.at(t)),
            move |el, delta| {
                let value = opacity_from + (opacity_to - opacity_from) * delta;
                opacity_value.set(value);
                el.map_element(|colored| colored.map_element(|fill| fill.opacity(value)))
            },
        )
        .into_any_element()
}

/// A background layer riding `background-color 200ms var(--ease-out)` — an
/// OKLab ease between generations, a plain fill otherwise. Its animation id
/// belongs to a listener-free child, keeping the control's input path stable.
fn easing_bg_layer(
    id: &gpui::ElementId,
    tag: &'static str,
    target: gpui::Hsla,
    radius: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let reduce_motion = ActiveTheme::reduce_motion(cx);
    let mut tween = Tween::keyed(id, tag, target, window, cx);
    tween.snap_if_reduced(reduce_motion);
    let base = gpui::div().absolute().inset_0().rounded(radius);
    if !tween.animates(reduce_motion) {
        tween.settle();
        return base.bg(target).into_any_element();
    }
    let (from, to) = (tween.from(), tween.target());
    let color = tween.value();
    base.with_animation(
        element_id::indexed(id, tag, tween.generation()),
        gpui::Animation::new(Duration::from_millis(FILL_BG_MS))
            .with_easing(|t| crate::anim::Curve::Out.at(t)),
        move |el, delta| {
            let next = if delta >= 1.0 {
                to
            } else {
                herogpui_core::mix_oklab(from, to, delta)
            };
            color.set(next);
            el.bg(next)
        },
    )
    .into_any_element()
}

/// The checkmark canvas, stroked up to the current drawn fraction. The
/// animation id changes with the generation, which restarts the reveal from
/// the rendered fraction after an interrupted turn. Selected, the draw rides
/// the CSS `150ms linear` after its `15ms` delay; unselecting undraws over
/// the base `duration-200` on Tailwind's default transition curve.
fn check_layer(
    id: &gpui::ElementId,
    tween: Tween<f32>,
    reduce_motion: bool,
    size: Pixels,
    color: gpui::Hsla,
) -> AnyElement {
    let progress = tween.value();
    let canvas = gpui::canvas(
        |bounds, _, _| bounds,
        move |bounds, _, window, _| paint_check_stroke(bounds, progress.get(), color, window),
    )
    .size(size);
    if !tween.animates(reduce_motion) {
        tween.settle();
        return canvas.into_any_element();
    }

    let (from, to) = (tween.from(), tween.target());
    let (duration, easing): (u64, Box<dyn Fn(f32) -> f32>) = if to > from {
        let total = (CHECK_DRAW_DELAY_MS + CHECK_DRAW_MS) as f32;
        let delay = CHECK_DRAW_DELAY_MS as f32;
        let span = CHECK_DRAW_MS as f32;
        (
            CHECK_DRAW_DELAY_MS + CHECK_DRAW_MS,
            Box::new(move |t: f32| ((t * total - delay) / span).clamp(0., 1.)),
        )
    } else {
        (
            CHECK_UNDRAW_MS,
            Box::new(crate::anim::tailwind_default_ease()),
        )
    };
    let progress = tween.value();
    canvas
        .with_animation(
            element_id::indexed(id, "check-draw", tween.generation()),
            gpui::Animation::new(Duration::from_millis(duration)).with_easing(easing),
            move |el, delta| {
                progress.set(from + (to - from) * delta);
                el
            },
        )
        .into_any_element()
}

/// One butt-capped stroke between two points; the discs
/// [`paint_check_stroke`] paints over the shared ends are what make the caps
/// and the join read round.
fn stroke_segment(
    a: gpui::Point<Pixels>,
    b: gpui::Point<Pixels>,
    width: Pixels,
    color: gpui::Hsla,
    window: &mut Window,
) {
    let mut builder = gpui::PathBuilder::stroke(width);
    builder.move_to(a);
    builder.line_to(b);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// Strokes the leading fraction of the pinned upstream polyline — the drawing
/// end of the CSS `stroke-dashoffset` slide, which reveals the check from its
/// start point through the elbow to the tip.
fn paint_check_stroke(
    bounds: gpui::Bounds<Pixels>,
    progress: f32,
    color: gpui::Hsla,
    window: &mut Window,
) {
    if progress <= 0.0 {
        return;
    }
    let progress = progress.min(1.0);
    let (view_w, view_h) = CHECK_VIEWBOX;
    let scale = (f32::from(bounds.size.width) / view_w).min(f32::from(bounds.size.height) / view_h);
    let origin = gpui::point(
        bounds.origin.x + (bounds.size.width - px(view_w * scale)) / 2.0,
        bounds.origin.y + (bounds.size.height - px(view_h * scale)) / 2.0,
    );
    let map = |point: (f32, f32)| {
        gpui::point(
            origin.x + px(point.0 * scale),
            origin.y + px(point.1 * scale),
        )
    };

    let [start, elbow, end] = CHECK_POLYLINE;
    let first = segment_length(start, elbow);
    let total = first + segment_length(elbow, end);
    // The CSS slide uncovers `CHECK_DASH_UNITS` of arc length, overshooting
    // the polyline; the drawn state simply sits at the tip.
    let reveal = (progress * CHECK_DASH_UNITS).min(total);
    let past_elbow = reveal > first;
    let tip = if past_elbow {
        lerp_point(elbow, end, (reveal - first) / segment_length(elbow, end))
    } else {
        lerp_point(start, elbow, reveal / first)
    };

    // `stroke-linejoin="round"` and `stroke-linecap="round"`: gpui's stroke
    // builder miters and butt-caps with no way to change either, and a disc
    // painted over a miter leaves the spike showing past it. The segments are
    // therefore stroked disconnected and the elbow gets its own disc once the
    // reveal passes it — the same completion `ProgressCircle` paints for its
    // arc ends.
    let stroke_w = px(CHECK_STROKE * scale);
    stroke_segment(
        map(start),
        if past_elbow { map(elbow) } else { map(tip) },
        stroke_w,
        color,
        window,
    );
    if past_elbow {
        stroke_segment(map(elbow), map(tip), stroke_w, color, window);
    }
    let cap_radius = stroke_w / 2.0;
    let caps = [
        Some(map(start)),
        past_elbow.then(|| map(elbow)),
        Some(map(tip)),
    ];
    for center in caps.into_iter().flatten() {
        crate::util::paint_disc(center, cap_radius, color, window);
    }
}

fn segment_length(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((b.0 - a.0) * (b.0 - a.0) + (b.1 - a.1) * (b.1 - a.1)).sqrt()
}

fn lerp_point(a: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// HeroGPUI-only compact size for a [`Checkbox`].
///
/// | Metric (pixels) | Sm | Md (pinned default) |
/// | --- | --- | --- |
/// | Control / indicator / check canvas | 14 / 10 / 8 | 16 / 12 / 10 |
/// | Label text / line height | 12 / 16 | 14 / 20 |
/// | Control-to-label gap / supporting-text indent | 12 / 26 | 12 / 28 |
/// | Indeterminate dash width / height | 10 / 2 | 12 / 2 |
///
/// Both steps have zero row padding and no extra minimum hitbox: the clickable
/// row contains the control and caller content, growing for taller content.
/// Supporting text remains 12/16 with a 4px vertical gap. The control keeps
/// `mark_radius` unless `radius` overrides it; `is_round` uses half the control
/// width. Indicator fill motion and press behavior are unchanged. A Checkbox
/// has no orientation setting; CheckboxGroup owns its option layout.
///
/// v3.2.4 removed the field `size` prop (the control is `size-4` through
/// Tailwind), so this is additive: `Md` is byte-identical to the pinned
/// default and `Sm` is HeroGPUI's own 14px step. Not a v3 prop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckboxSize {
    /// The small step (14px control).
    Sm,
    /// The medium step; the pinned default.
    #[default]
    Md,
}

impl CheckboxSize {
    /// Every size, in declaration order.
    pub const ALL: [CheckboxSize; 2] = [Self::Sm, Self::Md];

    /// `(control, indicator, label text)` for this step.
    fn metrics(self) -> (Pixels, Pixels, Pixels) {
        match self {
            Self::Sm => (px(14.), px(10.), px(12.)),
            Self::Md => (px(16.), px(12.), px(14.)),
        }
    }

    /// The size's display name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Sm => "Small",
            Self::Md => "Medium",
        }
    }
}

/// HeroUI Checkbox.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Checkbox {
    /// `value` — what this control submits when checked. HTML's default is
    /// `"on"`.
    value: Option<gpui::SharedString>,
    /// `validationBehavior` — carried on this control's form field.
    validation_behavior: crate::form::ValidationBehavior,
    /// `name` — the name this control submits under; read back by
    /// [`Self::form_field`].
    name: Option<gpui::SharedString>,
    id: gpui::ElementId,
    /// `isSelected` — `None` leaves the component holding the state, seeded
    /// from `defaultSelected`.
    checked: Option<bool>,
    default_checked: bool,
    is_indeterminate: bool,
    is_disabled: bool,
    is_read_only: bool,
    is_required: bool,
    /// `validate` — run by the component, not the caller.
    validate: Option<crate::validation::Validator<bool>>,
    /// `validationErrors` — messages from a server round-trip.
    validation_errors: Vec<gpui::SharedString>,
    is_invalid: bool,
    variant: herogpui_core::FieldVariant,
    /// `Checkbox.Indicator` children — v3 swaps the glyph per field state,
    /// which is its "Custom Indicator" example.
    indicator: Option<Box<dyn Fn(CheckboxState) -> AnyElement + 'static>>,
    /// Checkbox root children render function, handed the live field state.
    content: Option<Box<dyn Fn(CheckboxState) -> AnyElement + 'static>>,
    /// A round control instead of `rounded-md`. v3's "Full Rounded" example
    /// does it with `className="rounded-full"` on `Checkbox.Control`.
    is_round: bool,
    /// The control fill while hovered, in place of `--accent-hover`.
    hover_bg: Option<gpui::Hsla>,
    /// The control's corner radius, in place of the owning `mark_radius`
    /// helper. `is_round` still forces the circle.
    radius: Option<Pixels>,
    /// The compact step; `Md` is the pinned default.
    size: CheckboxSize,
    /// The label's font size, in place of the size step's.
    text_size: Option<Pixels>,
    description: Option<gpui::SharedString>,
    /// The plain text of the label, when the caller had one.
    ///
    /// `label` takes an arbitrary element, and a gpui text child carries no
    /// element id, so it contributes no accessibility node and no accessible
    /// name. A [`CheckboxGroup`] composes its option labels into elements but
    /// still knows the string, and passes it here so the box is named.
    label_text: Option<gpui::SharedString>,
    /// `aria-label` — the explicit accessible name, which wins over the
    /// derived `label_text` exactly as the HTML name computation ranks it.
    aria_label: Option<gpui::SharedString>,
    error_message: Option<gpui::SharedString>,
    children: Vec<AnyElement>,
    on_change: Option<std::sync::Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    form_state: Rc<RefCell<crate::form::LiveFormFieldState>>,
    form_focus_target: Option<Rc<RefCell<crate::form::LiveFormFieldState>>>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Checkbox {
    /// `isReadOnly` — shows the value but refuses changes.
    /// `validate` — returns the message to show, or `None` when the state is fine.
    ///
    /// The component runs it and surfaces the result.
    pub fn validate(mut self, f: impl Fn(&bool) -> Option<gpui::SharedString> + 'static) -> Self {
        self.validate = Some(std::sync::Arc::new(f));
        self
    }

    /// `validationErrors` — messages produced elsewhere, shown ahead of
    /// whatever `validate` returns.
    pub fn validation_errors(
        mut self,
        errors: impl IntoIterator<Item = impl Into<gpui::SharedString>>,
    ) -> Self {
        self.validation_errors = errors.into_iter().map(Into::into).collect();
        self
    }

    /// `isRequired` — marks the label as required.
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// `isInvalid` — draws the control in the danger role.
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// `Checkbox.Indicator` — draws the mark yourself from the field state.
    pub fn indicator(mut self, render: impl Fn(CheckboxState) -> AnyElement + 'static) -> Self {
        self.indicator = Some(Box::new(render));
        self
    }

    /// Checkbox root children render function, handed the live field state.
    /// Replaces labels and extended static children when set.
    pub fn content(mut self, render: impl Fn(CheckboxState) -> AnyElement + 'static) -> Self {
        self.content = Some(Box::new(render));
        self
    }

    /// A fully round control, which v3's "Full Rounded" example asks for with
    /// `rounded-full` on `Checkbox.Control`.
    pub fn is_round(mut self, v: bool) -> Self {
        self.is_round = v;
        self
    }

    /// The control fill while hovered, in place of `--accent-hover`. The
    /// scale/fade Tween and the pressed target are unchanged.
    pub fn hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.hover_bg = Some(color.into());
        self
    }

    /// The control's corner radius, in place of the owning `mark_radius`
    /// helper. It replaces the helper's value only: `is_round` keeps its
    /// documented circle either way, and the control's animated fill layers
    /// follow the resolved value so they stay inside the corners. Not a v3
    /// prop; the removed v2 `radius` prop is prohibited and this is a
    /// per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// Sets the compact step. `Md` is the default and byte-identical to the
    /// pinned control; `Sm` is a 14px control with a 10px indicator and 12px
    /// label text (16px leading). Not a v3 prop.
    pub fn size(mut self, size: CheckboxSize) -> Self {
        self.size = size;
        self
    }

    /// The label font size, in place of the size step's. A 12/14/16px size
    /// takes v3's leading pair (16/20/24); any other keeps the 20px leading.
    /// The control box, its mark and the description keep the size step, so the override changes the text and its line box only. Not a v3 prop: v3 sets it with a class on `Checkbox.Content`.
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// `variant` — `Secondary` drops the shadow for use on a surface.
    pub fn variant(mut self, variant: herogpui_core::FieldVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the checkbox's root element after every value the variant and
    /// the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// Sets whether the checkbox is read-only.
    pub fn is_read_only(mut self, v: bool) -> Self {
        self.is_read_only = v;
        self
    }

    /// Creates an unchecked checkbox with the given element id.
    pub fn new(id: impl Into<gpui::ElementId>) -> Self {
        Self {
            value: None,
            validation_behavior: crate::form::ValidationBehavior::Native,
            name: None,
            id: id.into(),
            checked: None,
            default_checked: false,
            is_indeterminate: false,
            is_disabled: false,
            is_read_only: false,
            is_required: false,
            validate: None,
            validation_errors: Vec::new(),
            is_invalid: false,
            variant: herogpui_core::FieldVariant::Primary,
            indicator: None,
            content: None,
            is_round: false,
            hover_bg: None,
            radius: None,
            size: CheckboxSize::default(),
            text_size: None,
            description: None,
            label_text: None,
            aria_label: None,
            error_message: None,
            children: Vec::new(),
            on_change: None,
            form_state: Rc::new(RefCell::new(crate::form::LiveFormFieldState {
                commit_validation: None,
                reset_validation: None,
                value: crate::form::FormValue::Flag(false),
                is_invalid: false,
                is_successful: true,
                focus: None,
                restore: None,
            })),
            form_focus_target: None,
            sx: None,
        }
    }

    /// `value` — what this control submits when checked.
    ///
    /// An HTML checkbox submits `"on"` unless told otherwise; this is that
    /// override, and it is read by [`Self::form_field`].
    pub fn value(mut self, value: impl Into<gpui::SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// `validationBehavior` — `Allow` shows the message without blocking form
    /// submission. Carried on the [`Self::form_field`] this control produces.
    pub fn validation_behavior(mut self, behavior: crate::form::ValidationBehavior) -> Self {
        self.validation_behavior = behavior;
        self
    }

    /// `name` — the name this control submits under.
    pub fn name(mut self, name: impl Into<gpui::SharedString>) -> Self {
        self.name = Some(name.into());
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
    /// # use herogpui_components::{Checkbox, Form};
    /// # struct Demo;
    /// # impl Render for Demo {
    /// #     fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    /// #         let form = Form::new();
    /// #         let control = Checkbox::new("terms").name("terms");
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
        let checked = self.checked.unwrap_or(self.default_checked);
        let validity = crate::validation::resolve(
            self.is_invalid,
            &self.validation_errors,
            self.validate.as_ref().and_then(|f| f(&checked)),
            self.error_message.clone(),
        );
        {
            let mut state = self.form_state.borrow_mut();
            state.value = match (&self.value, checked) {
                (Some(value), true) => crate::form::FormValue::Text(value.clone()),
                _ => crate::form::FormValue::Flag(checked),
            };
            state.is_invalid = validity.is_invalid;
            state.is_successful = !self.is_disabled;
        }
        Some(
            crate::form::FormField::live(name, self.form_state.clone())
                .is_required(self.is_required)
                .validation_behavior(self.validation_behavior),
        )
    }

    /// `isSelected` — the controlled state; `None` leaves the component
    /// holding it, seeded from `defaultSelected`.
    pub fn is_selected(mut self, v: bool) -> Self {
        self.checked = Some(v);
        self
    }

    /// `defaultSelected` — the uncontrolled initial state.
    ///
    /// Only consulted when `checked` is not supplied; the component then owns
    /// the state and toggles itself on click.
    pub fn default_selected(mut self, v: bool) -> Self {
        self.default_checked = v;
        self
    }

    /// Shows a dash instead of the check (`isIndeterminate`).
    pub fn is_indeterminate(mut self, v: bool) -> Self {
        self.is_indeterminate = v;
        self
    }

    /// Sets whether the checkbox is disabled.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Label content. A plain string label also names the control for
    /// assistive technology; a richer element leaves the name to `aria_label`.
    pub fn label(mut self, el: impl IntoElement + 'static) -> Self {
        if let Some(text) = a11y::label_text(&el) {
            self.label_text = Some(text);
        }
        self.children.push(el.into_any_element());
        self
    }

    /// `Description` — help text below and aligned with the label.
    pub fn description(mut self, text: impl Into<gpui::SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// `aria-label` — the explicit accessible name, which wins over the
    /// label-derived name exactly as the HTML name computation ranks it.
    pub fn aria_label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// The label's plain text, for the accessibility node. See the
    /// `label_text` field for why an element-typed label is not enough.
    fn a11y_label(mut self, text: impl Into<gpui::SharedString>) -> Self {
        self.label_text = Some(text.into());
        self
    }

    /// `FieldError` — fallback validation text below and aligned with the label.
    pub fn error_message(mut self, text: impl Into<gpui::SharedString>) -> Self {
        self.error_message = Some(text.into());
        self
    }

    /// Sets the handler called with the new selected state when the checkbox is toggled.
    pub fn on_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(std::sync::Arc::new(f));
        self
    }

    fn form_focus_target(mut self, state: Rc<RefCell<crate::form::LiveFormFieldState>>) -> Self {
        self.form_focus_target = Some(state);
        self
    }
}

impl ParentElement for Checkbox {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        let (checked, own) = crate::util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "checked"),
            self.checked,
            self.default_checked,
        );
        let reset_own = own.clone();
        let reset_state = Rc::downgrade(&self.form_state);
        let reset_value = self.value.clone();
        let reset_change = self
            .checked
            .is_some()
            .then(|| self.on_change.clone())
            .flatten();
        self.form_state.borrow_mut().restore = (reset_own.is_some() || reset_change.is_some())
            .then(|| {
                let default_checked = self.default_checked;
                let reset_state = reset_state.clone();
                crate::util::shared(move |window: &mut Window, cx: &mut App| {
                    if let Some(state) = reset_state.upgrade() {
                        state.borrow_mut().value = match (&reset_value, default_checked) {
                            (Some(value), true) => crate::form::FormValue::Text(value.clone()),
                            _ => crate::form::FormValue::Flag(default_checked),
                        };
                    }
                    if let Some(held) = &reset_own {
                        held.update(cx, |checked, cx| {
                            *checked = default_checked;
                            cx.notify();
                        });
                    }
                    if let Some(on_change) = &reset_change {
                        on_change(&default_checked, window, cx);
                    }
                }) as std::sync::Arc<dyn Fn(&mut Window, &mut App)>
            });

        // v3 order: the controlled flag, then server errors, then `validate`.
        let validate_result = self.validate.as_ref().and_then(|f| f(&checked));
        let validity = crate::validation::resolve(
            self.is_invalid,
            &self.validation_errors,
            validate_result.clone(),
            self.error_message.clone(),
        );
        // React Stately's commit-gated display: `validate` alone is the
        // pending half, synced every render; a toggle queues the commit that
        // the next render's sync lands. `display_validity` composes what the
        // field paints — the controlled flag, server messages and
        // `errorMessage` stay live, the gated half is the snapshot.
        let pending = crate::validation::resolve(false, &[], validate_result, None);
        let validation_display =
            window.use_keyed_state(element_id::scoped(&self.id, "validation"), cx, |_, _| {
                crate::validation::ValidationDisplay::default()
            });
        validation_display.update(cx, |d, _| d.sync(pending));
        let display = crate::validation::display_validity(
            self.validation_behavior,
            &validity,
            self.is_invalid,
            &self.validation_errors,
            validation_display.read(cx).displayed(),
            self.error_message.clone(),
        );
        {
            let mut state = self.form_state.borrow_mut();
            state.value = match (&self.value, checked) {
                (Some(value), true) => crate::form::FormValue::Text(value.clone()),
                _ => crate::form::FormValue::Flag(checked),
            };
            state.is_invalid = validity.is_invalid;
            state.is_successful = !self.is_disabled;
            // The form's submit/reset reach the snapshot through these
            // closures; `commit`/`reset` notify only when they changed it.
            let (commit, reset) = crate::validation::form_hooks(&validation_display);
            state.commit_validation = Some(commit);
            state.reset_validation = Some(reset);
        }

        // v3 focuses the checkbox and rings `.checkbox__control`, so the two sit
        // on different elements: the row takes the focus, the box shows it.
        // `use_keyed_state` takes `cx` mutably, so it precedes the theme.
        let focus_handle =
            crate::util::tab_stop_handle(element_id::scoped(&self.id, "focus"), window, cx);
        self.form_state.borrow_mut().focus = Some(focus_handle.clone());
        if let Some(target) = &self.form_focus_target {
            target.borrow_mut().focus = Some(focus_handle.clone());
        }
        // The role's colours, copied out so the keyed-state calls below can
        // take `cx` mutably. `isInvalid` outranks the colour role, as it does
        // on every field: the danger role is chosen here and nowhere else.
        let (accent_color, accent_hover, accent_foreground) = {
            let sem = if display.is_invalid {
                cx.role(Color::Danger)
            } else {
                cx.role(Color::Accent)
            };
            (sem.color, sem.hover(), sem.foreground)
        };

        // Two marks, two targets: the CSS fill lights for any checked box —
        // `data-selected` has no indeterminate escape — while the dash
        // replaces the checkmark outright, so only a plain tick draws itself.
        let fill_visible = checked;
        let check_visible = checked && !self.is_indeterminate;

        // The fill's hover colour and the indeterminate press read the same
        // one-frame-late hover/press slot the switch track reads. A disabled
        // box does not track the pointer, and a slot gone stale — disabled
        // under the pointer or a held button — reads off, which is itself
        // visible as the ease back off the hover accent. Read-only still
        // hovers; only `is_disabled` guards.
        let interaction =
            crate::util::interaction(element_id::scoped(&self.id, "interaction"), window, cx);
        let (is_hovered, is_pressed) = if self.is_disabled {
            (false, false)
        } else {
            *interaction.read(cx)
        };

        // `.checkbox__control` is `size-4`, `.checkbox__indicator` `size-3`
        // around a `size-2.5` checkmark, and `.checkbox__content` `text-sm`.
        // The `Sm` step scales all three together; `Md` is pinned.
        let (box_px, icon_px, text) = self.size.metrics();
        let text = self.text_size.unwrap_or(text);
        let control_radius = if self.is_round {
            // `rounded-full` on the control: the fill matches it, the way the
            // control's `overflow-hidden` clips the pseudo-element upstream.
            box_px / 2.0
        } else {
            // An instance radius replaces the helper's value here and only
            // here: `is_round` above keeps its documented circle either way.
            self.radius.unwrap_or_else(|| crate::util::mark_radius(cx))
        };

        // Tween targets, read out before the keyed-state calls take `cx`
        // mutably. Selected paints the accent on the `::before` fill over the
        // control's resting background; indeterminate moves the control's own
        // `bg-accent` — pressed, its `bg-accent-hover` — with the fill still
        // mounted underneath.
        let control_bg_target = if self.is_indeterminate {
            if is_pressed {
                accent_hover
            } else {
                accent_color
            }
        } else {
            match self.variant {
                herogpui_core::FieldVariant::Primary => cx.colors().field.background,
                herogpui_core::FieldVariant::Secondary => cx.colors().default.color,
            }
        };
        let fill_hover = self.hover_bg.unwrap_or(accent_hover);
        let fill_bg_target = if is_hovered { fill_hover } else { accent_color };

        // The motion slots; every `use_keyed_state` here needs `cx` mutably.
        let reduce_motion = ActiveTheme::reduce_motion(cx);
        let mut fill_opacity =
            Tween::keyed(&self.id, "fill-fade", f32::from(fill_visible), window, cx);
        let mut fill_scale = Tween::keyed(
            &self.id,
            "fill-scale",
            if fill_visible { 1.0 } else { FILL_REST_SCALE },
            window,
            cx,
        );
        let mut check_stroke = Tween::keyed(
            &self.id,
            "check-motion",
            f32::from(check_visible),
            window,
            cx,
        );
        fill_opacity.snap_if_reduced(reduce_motion);
        fill_scale.snap_if_reduced(reduce_motion);
        check_stroke.snap_if_reduced(reduce_motion);
        let mut fill_background = Tween::keyed(&self.id, "fill-bg", fill_bg_target, window, cx);
        fill_background.snap_if_reduced(reduce_motion);
        let control_background = easing_bg_layer(
            &self.id,
            "control-bg",
            control_bg_target,
            control_radius,
            window,
            cx,
        );

        let checkbox_state = CheckboxState {
            is_selected: checked,
            is_indeterminate: self.is_indeterminate,
            is_disabled: self.is_disabled,
            is_read_only: self.is_read_only,
            is_invalid: display.is_invalid,
            is_required: self.is_required,
        };

        // Stateful because the hover/press tracking arms its listeners on it:
        // the fill's `bg-accent-hover` on hover and the control's
        // `bg-accent-hover` while indeterminate and pressed both read the slot
        // above. The id derives from the row's, the same way the checked and
        // focus slots derive theirs, so nothing collides.
        let mut boxel = gpui::div()
            .id(element_id::scoped(&self.id, "control"))
            .flex()
            .items_center()
            .justify_center()
            .size(box_px)
            .rounded(control_radius)
            // `.checkbox__control` is `overflow-hidden`, clipping both layers
            // below to the control's corners. The clip is not spelled here:
            // vanilla gpui clips to the box's *rectangle*, so both layers
            // carry `control_radius` themselves anyway (the resting fill
            // through `easing_bg_layer`, the checked fill through
            // `fill_layer`, which is additionally inset by its scale), and the
            // mark is centred well inside. Dropping it is what lets the focus
            // ring be the overlay below, which hangs outside the box.
            .flex_shrink_0()
            // The control itself keeps its resting background in every state:
            // the accent a selected or indeterminate box paints rides the
            // listener-free layers below, which is what makes the swap a
            // `background-color` transition the eye can follow.
            .bg(match self.variant {
                herogpui_core::FieldVariant::Primary => cx.colors().field.background,
                herogpui_core::FieldVariant::Secondary => cx.colors().default.color,
            });

        // `Primary` carries the field shadow; `Secondary` is the flat variant
        // meant for use on a surface. Held as a list rather than applied,
        // because the focus ring is applied to the same slot and `shadow()`
        // replaces: a focused checkbox would otherwise lose its shadow.
        let box_shadow: Vec<gpui::BoxShadow> =
            if self.variant == herogpui_core::FieldVariant::Primary {
                cx.layout().field_shadow.clone()
            } else {
                Vec::new()
            };

        // `status-invalid-field` draws a 1px danger outline over the fill, and
        // v3 applies it only while the box is neither selected nor
        // indeterminate.
        if display.is_invalid && !fill_visible && !self.is_indeterminate {
            boxel = boxel.border_1().border_color(cx.colors().danger.color);
        }

        // The hover/press listeners feeding the slot above. A disabled box
        // shows its state but does not react, so it does not track.
        if !self.is_disabled {
            boxel = crate::util::track_interaction(boxel, &interaction);
        }

        // The two animated backgrounds, then the mark: all listener-free, so
        // the ids their animations change never touch the interactive element.
        boxel = boxel.child(control_background);
        boxel = boxel.child(fill_layer(
            &self.id,
            fill_opacity,
            fill_scale,
            reduce_motion,
            control_radius,
            box_px,
            fill_background,
        ));

        // A caller-drawn indicator replaces both marks, the way
        // `Checkbox.Indicator`'s render prop does.
        if let Some(render) = &self.indicator {
            boxel = boxel.child(render(checkbox_state));
        } else if self.is_indeterminate {
            boxel = boxel.child(
                gpui::div()
                    .w(icon_px)
                    .h(px(2.))
                    .rounded_full()
                    .bg(accent_foreground),
            );
        } else {
            // The checkmark: a canvas stroke of the pinned upstream polyline,
            // revealed from its start point exactly as the CSS
            // `stroke-dashoffset` slide reveals it. The svg asset this
            // replaces could not animate a stroke — and draws nothing at all
            // where no asset source is installed, as in the tests. The CSS
            // marks the svg `size-2.5` inside the `size-3` indicator, a 2px
            // inset the canvas keeps at both size steps: 10px centred in 12px
            // for `Md`, 8px in 10px for `Sm`.
            boxel = boxel.child(
                gpui::div()
                    .size(icon_px)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(check_layer(
                        &self.id,
                        check_stroke,
                        reduce_motion,
                        icon_px - px(2.),
                        accent_foreground,
                    )),
            );
        }

        let boxel = crate::util::with_focus_ring_overlay(
            boxel,
            !self.is_disabled && focus_handle.is_focused(window) && crate::util::focus_visible(cx),
            true,
            control_radius,
            box_shadow,
            cx,
        );

        let children = self
            .content
            .map_or(self.children, |render| vec![render(checkbox_state)]);
        // `useCheckbox` renders a native `<input type="checkbox">`, whose role
        // is `checkbox`, and sets the DOM `indeterminate` property — which is
        // what makes a checkbox report `aria-checked="mixed"`.
        let name = a11y::Name::field(
            self.aria_label.as_ref().or(self.label_text.as_ref()),
            self.description.as_ref(),
            &display,
        );
        let row = gpui::div()
            .id(self.id.clone())
            .a11y_named(a11y::Role::CheckBox, &name)
            .a11y_checked(checked, self.is_indeterminate)
            .when(!self.is_disabled, |el| el.track_focus(&focus_handle))
            .flex()
            .items_center()
            // `.checkbox__content` is `gap-3`.
            .gap(px(12.))
            .when(!self.is_disabled && !self.is_read_only, |r| {
                r.cursor(crate::util::interactive_cursor(cx))
            })
            .children(
                std::iter::once(boxel.into_any_element())
                    .chain(children)
                    .chain(self.is_required.then(|| {
                        gpui::div()
                            .text_color(cx.colors().danger.color)
                            .child("*")
                            .into_any_element()
                    })),
            )
            .text_size(text)
            .line_height(crate::util::leading_for(text).unwrap_or(px(20.)))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(cx.colors().foreground);

        let row = if self.is_disabled {
            row
        } else {
            crate::util::record_focus_bounds(row, &focus_handle, window, cx)
        };
        let content = if !self.is_disabled
            && !self.is_read_only
            && (self.on_change.is_some() || own.is_some())
        {
            let on_change = self.on_change;
            let click_validation = validation_display.clone();
            row.on_click(move |event, window, cx| {
                if matches!(
                    event,
                    gpui::ClickEvent::Keyboard(event)
                        if event.button == gpui::KeyboardButton::Enter
                ) {
                    return;
                }
                // A toggle commits the gated validity on the next render —
                // React Stately's `commitValidation` on every change. The
                // notify re-renders the owning view even when a controlled
                // parent ignores `on_change`.
                crate::validation::queue_commit(&click_validation, cx);
                // Uncontrolled: flip our own copy, or nothing could ever
                // change it.
                if let Some(held) = &own {
                    held.update(cx, |v, cx| {
                        *v = !checked;
                        cx.notify();
                    });
                }
                if let Some(cb) = &on_change {
                    cb(&!checked, window, cx);
                }
            })
            .into_any_element()
        } else {
            row.into_any_element()
        };

        let message = display.first();
        let mut root = gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(4.))
            .when(self.is_disabled, |r| {
                r.opacity(cx.layout().disabled_opacity)
            })
            .child(content);
        if let Some(message) = message {
            root = root.child(
                gpui::div()
                    .w_full()
                    .pl(box_px + px(12.))
                    .child(crate::field::ErrorMessage::new(message)),
            );
        } else if let Some(description) = self.description {
            root = root.child(
                gpui::div()
                    .w_full()
                    .pl(box_px + px(12.))
                    .child(crate::field::Description::new(description)),
            );
        }
        root = crate::util::apply_sx(root, &self.sx);
        root.into_any_element()
    }
}

// ---------------------------------------------------------------------------
// CheckboxGroup
// ---------------------------------------------------------------------------

/// One option in a [`CheckboxGroup`].
#[must_use = "builder methods return a new value; pass the option to its component"]
#[derive(Clone)]
pub struct CheckboxOption {
    key: gpui::SharedString,
    label: gpui::SharedString,
    description: Option<gpui::SharedString>,
    is_disabled: bool,
}

impl CheckboxOption {
    /// Creates an option with the given key and label.
    pub fn new(key: impl Into<gpui::SharedString>, label: impl Into<gpui::SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            description: None,
            is_disabled: false,
        }
    }

    /// Sets the description text shown below the label.
    pub fn description(mut self, text: impl Into<gpui::SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Sets whether the option is disabled.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// The option's key.
    pub fn key(&self) -> &gpui::SharedString {
        &self.key
    }
}

type OnGroupChange =
    std::sync::Arc<dyn Fn(&std::collections::HashSet<gpui::SharedString>, &mut Window, &mut App)>;

/// CheckboxGroup — port of `@heroui/checkbox-group` (v3).
///
/// A set of checkboxes sharing a label, orientation, validation state and
/// selected-value set.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct CheckboxGroup {
    /// `name` — the name this control submits under; read back by
    /// [`Self::form_field`].
    name: Option<gpui::SharedString>,
    id: gpui::ElementId,
    options: Vec<CheckboxOption>,
    label: Option<gpui::SharedString>,
    description: Option<gpui::SharedString>,
    error_message: Option<gpui::SharedString>,
    value: Option<std::collections::HashSet<gpui::SharedString>>,
    default_value: std::collections::HashSet<gpui::SharedString>,
    orientation: herogpui_core::Orientation,
    variant: herogpui_core::FieldVariant,
    is_disabled: bool,
    is_read_only: bool,
    is_invalid: bool,
    is_required: bool,
    on_change: Option<OnGroupChange>,
    form_state: Rc<RefCell<crate::form::LiveFormFieldState>>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl CheckboxGroup {
    /// Creates a group with the given element id and options.
    pub fn new(id: impl Into<gpui::ElementId>, options: Vec<CheckboxOption>) -> Self {
        Self {
            name: None,
            id: id.into(),
            options,
            label: None,
            description: None,
            error_message: None,
            value: None,
            default_value: std::collections::HashSet::new(),
            orientation: herogpui_core::Orientation::Vertical,
            variant: herogpui_core::FieldVariant::Primary,
            is_disabled: false,
            is_read_only: false,
            is_invalid: false,
            is_required: false,
            on_change: None,
            form_state: Rc::new(RefCell::new(crate::form::LiveFormFieldState {
                commit_validation: None,
                reset_validation: None,
                value: crate::form::FormValue::Keys(Vec::new()),
                is_invalid: false,
                is_successful: true,
                focus: None,
                restore: None,
            })),
            sx: None,
        }
    }

    /// `name` — the name this control submits under.
    pub fn name(mut self, name: impl Into<gpui::SharedString>) -> Self {
        self.name = Some(name.into());
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
    /// # use herogpui_components::{CheckboxGroup, CheckboxOption, Form};
    /// # struct Demo;
    /// # impl Render for Demo {
    /// #     fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    /// #         let form = Form::new();
    /// #         let control = CheckboxGroup::new("langs", vec![CheckboxOption::new("rust", "Rust")])
    /// #             .name("langs");
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
        let selected = self.value.as_ref().unwrap_or(&self.default_value);
        let values = self
            .options
            .iter()
            .filter(|option| {
                !self.is_disabled && !option.is_disabled && selected.contains(&option.key)
            })
            .map(|option| option.key.clone())
            .collect();
        {
            let mut state = self.form_state.borrow_mut();
            state.value = crate::form::FormValue::Keys(values);
            state.is_invalid = self.is_invalid || self.error_message.is_some();
            state.is_successful = !self.is_disabled;
            state.focus = None;
        }
        Some(
            crate::form::FormField::live(name, self.form_state.clone())
                .is_required(self.is_required),
        )
    }

    /// Sets the group label.
    pub fn label(mut self, text: impl Into<gpui::SharedString>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// Sets the group description.
    pub fn description(mut self, text: impl Into<gpui::SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Sets the error message shown when the group is invalid.
    pub fn error_message(mut self, text: impl Into<gpui::SharedString>) -> Self {
        self.error_message = Some(text.into());
        self
    }

    /// `value` — the selected keys, controlled.
    pub fn value(mut self, keys: impl IntoIterator<Item = gpui::SharedString>) -> Self {
        self.value = Some(keys.into_iter().collect());
        self
    }

    /// `defaultValue` — the uncontrolled initial selection.
    ///
    /// Only consulted when `value` is not supplied; the group then owns the
    /// selection and each checkbox toggles its own key in it.
    pub fn default_value(mut self, keys: impl IntoIterator<Item = gpui::SharedString>) -> Self {
        self.default_value = keys.into_iter().collect();
        self
    }

    /// Sets the layout direction of the options.
    pub fn orientation(mut self, orientation: herogpui_core::Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Sets the field variant.
    pub fn variant(mut self, variant: herogpui_core::FieldVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the group's root element after every value the variant and the
    /// active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// Sets whether the group is disabled.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Sets whether the group is invalid.
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// `isReadOnly` — every option shows its state but cannot be toggled.
    pub fn is_read_only(mut self, v: bool) -> Self {
        self.is_read_only = v;
        self
    }

    /// Sets whether the group is required.
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// Called with the complete selection after any box is toggled.
    pub fn on_change(
        mut self,
        handler: impl Fn(&std::collections::HashSet<gpui::SharedString>, &mut Window, &mut App)
            + 'static,
    ) -> Self {
        self.on_change = Some(std::sync::Arc::new(handler));
        self
    }
}

impl RenderOnce for CheckboxGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        let (value, own) = crate::util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "value"),
            self.value.clone(),
            self.default_value.clone(),
        );
        let reset_own = own.clone();
        let reset_state = Rc::downgrade(&self.form_state);
        let reset_options = self.options.clone();
        let reset_change = self
            .value
            .is_some()
            .then(|| self.on_change.clone())
            .flatten();
        self.form_state.borrow_mut().restore = (reset_own.is_some() || reset_change.is_some())
            .then(|| {
                let default_value = self.default_value.clone();
                let reset_state = reset_state.clone();
                let reset_options = reset_options.clone();
                crate::util::shared(move |window: &mut Window, cx: &mut App| {
                    if let Some(state) = reset_state.upgrade() {
                        state.borrow_mut().value = crate::form::FormValue::Keys(
                            reset_options
                                .iter()
                                .filter(|option| {
                                    default_value.contains(&option.key) && !option.is_disabled
                                })
                                .map(|option| option.key.clone())
                                .collect(),
                        );
                    }
                    if let Some(held) = &reset_own {
                        held.update(cx, |value, cx| {
                            *value = default_value.clone();
                            cx.notify();
                        });
                    }
                    if let Some(on_change) = &reset_change {
                        on_change(&default_value, window, cx);
                    }
                }) as std::sync::Arc<dyn Fn(&mut Window, &mut App)>
            });
        let form_values = self
            .options
            .iter()
            .filter(|option| {
                !self.is_disabled && !option.is_disabled && value.contains(&option.key)
            })
            .map(|option| option.key.clone())
            .collect();

        let colors = cx.colors();
        let is_invalid = self.is_invalid || self.error_message.is_some();
        {
            let mut state = self.form_state.borrow_mut();
            state.value = crate::form::FormValue::Keys(form_values);
            state.is_invalid = is_invalid;
            state.is_successful = !self.is_disabled;
            state.focus = None;
        }
        let first_enabled = self
            .options
            .iter()
            .position(|option| !self.is_disabled && !option.is_disabled);

        // `useCheckboxGroup` is `role="group"`, named and described through
        // `useField` from the group's own label, description and messages.
        let group_name = a11y::Name::field(
            self.label.as_ref(),
            self.description.as_ref(),
            &crate::validation::resolve(self.is_invalid, &[], None, self.error_message.clone()),
        );
        let mut root = gpui::div()
            .id(self.id.clone())
            .a11y_named(a11y::Role::Group, &group_name)
            .flex()
            .flex_col()
            .gap(px(16.));

        if let Some(label) = &self.label {
            root = root.child(
                crate::field::Label::new(label.clone())
                    .is_required(self.is_required)
                    .is_disabled(self.is_disabled)
                    .is_invalid(is_invalid),
            );
        }

        // `.checkbox-group` gives each option `mt-4`.
        let mut list = gpui::div().flex().gap(px(16.));
        list = match self.orientation {
            herogpui_core::Orientation::Vertical => list.flex_col(),
            herogpui_core::Orientation::Horizontal => list.flex_row().flex_wrap(),
        };

        for (index, option) in self.options.iter().enumerate() {
            let key = option.key.clone();
            let checked = value.contains(&key);
            let disabled = self.is_disabled || option.is_disabled;

            let mut label_el = gpui::div()
                .flex()
                .flex_col()
                // `.checkbox` is `gap-1` between its content and description.
                .gap(px(4.))
                .child(gpui::div().child(option.label.to_string()));
            if let Some(description) = &option.description {
                label_el = label_el.child(
                    gpui::div()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .font_weight(gpui::FontWeight::NORMAL)
                        .text_color(colors.muted)
                        .child(description.to_string()),
                );
            }

            let selection = value.clone();
            let on_change = self.on_change.clone();
            let own = own.clone();
            let mut checkbox = Checkbox::new(element_id::indexed(&self.id, "opt", index))
                .is_selected(checked)
                .is_disabled(disabled)
                .is_read_only(self.is_read_only)
                .is_invalid(is_invalid)
                .variant(self.variant)
                .label(label_el)
                .a11y_label(option.label.clone())
                .on_change(move |_next, window, cx| {
                    let mut set = selection.clone();
                    if !set.remove(&key) {
                        set.insert(key.clone());
                    }
                    // Uncontrolled: keep the new set, or ticking a box would
                    // do nothing.
                    if let Some(held) = &own {
                        held.update(cx, |v, cx| {
                            *v = set.clone();
                            cx.notify();
                        });
                    }
                    if let Some(cb) = &on_change {
                        cb(&set, window, cx);
                    }
                });
            if first_enabled == Some(index) {
                checkbox = checkbox.form_focus_target(self.form_state.clone());
            }
            list = list.child(checkbox);
        }

        root = root.child(list);

        let error = is_invalid.then(|| self.error_message.clone()).flatten();
        if let Some(error) = crate::anim::field_error_panel(&self.id, error, window, cx) {
            root = root.child(error);
        } else if let Some(description) = self.description {
            root = root.child(crate::field::Description::new(description));
        }

        root = crate::util::apply_sx(root, &self.sx);
        root
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;

    #[test]
    fn md_is_the_pinned_geometry_and_sm_scales_together() {
        assert_eq!(CheckboxSize::default(), CheckboxSize::Md);
        assert_eq!(CheckboxSize::Md.metrics(), (px(16.), px(12.), px(14.)));
        assert_eq!(CheckboxSize::Sm.metrics(), (px(14.), px(10.), px(12.)));
        assert_eq!(
            crate::util::leading_for(CheckboxSize::Sm.metrics().2),
            Some(px(16.))
        );
    }
}

crate::util::impl_component_styled!(Checkbox, CheckboxGroup);
