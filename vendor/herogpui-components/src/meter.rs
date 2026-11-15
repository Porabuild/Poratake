//! Meter — HeroUI v3 `Meter` (value within a known range).
//!
//! A meter reports a static measurement inside a known range (disk usage,
//! password strength), where a `ProgressBar` reports advancing work. The two
//! share their track geometry, so this delegates the bar rendering to
//! [`ProgressBar`].

use gpui::{App, IntoElement, Pixels, RenderOnce, SharedString, Window};
use herogpui_core::{Color, Size};

use crate::progress::ProgressBar;

/// HeroUI Meter. Supports `value` in `0..max` with
/// optional label and fill color.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Meter {
    id: gpui::ElementId,
    value: f32,
    min_value: f32,
    max_value: f32,
    size: Size,
    color: Color,
    label: Option<SharedString>,
    value_label: Option<SharedString>,
    /// `Meter.ValueLabel`'s render props (`percentage`, `valueText`), forwarded
    /// to the bar that draws them.
    value_content: Option<std::sync::Arc<dyn Fn(f32, &str) -> gpui::AnyElement + 'static>>,
    show_value: bool,
    /// `formatOptions` — forwarded to the bar, which writes the label.
    format: Option<herogpui_core::NumberFormat>,
    /// The corner radius, forwarded to the bar that paints the track and its
    /// fill.
    radius: Option<Pixels>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Meter {
    /// `value` — also accepted positionally by [`Meter::new`].
    pub fn value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }

    /// `Meter.ValueLabel`'s render function — handed `percentage` (0-100) and
    /// `valueText`, like the bar's.
    pub fn value_content(
        mut self,
        render: impl Fn(f32, &str) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.value_content = Some(std::sync::Arc::new(render));
        self
    }

    /// Creates a meter with the given id and value.
    pub fn new(id: impl Into<gpui::ElementId>, value: f32) -> Self {
        Self {
            id: id.into(),
            value_content: None,
            value,
            min_value: 0.0,
            max_value: 100.0,
            size: Size::Md,
            color: Color::Accent,
            label: None,
            value_label: None,
            show_value: false,
            format: None,
            radius: None,
            sx: None,
        }
    }

    /// Sets the minimum value of the range.
    pub fn min_value(mut self, v: f32) -> Self {
        self.min_value = v;
        self
    }

    /// Sets the maximum value of the range.
    pub fn max_value(mut self, v: f32) -> Self {
        self.max_value = v;
        self
    }

    /// `valueLabel` — replaces the generated percentage.
    pub fn value_label(mut self, text: impl Into<SharedString>) -> Self {
        self.value_label = Some(text.into());
        self
    }

    /// `formatOptions` — v3 defaults to `{style: "percent"}`.
    pub fn format_options(mut self, format: herogpui_core::NumberFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// The corner radius, forwarded to the bar the meter delegates to: the bar
    /// paints the track and its fill, so the override travels with the
    /// delegation the same way the `sx` slot does. Not a v3 prop; the removed
    /// v2 `radius` prop is prohibited and this is a per-component repository
    /// extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// Sets the meter size.
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }

    /// Sets the color role.
    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the meter's root element after every value the size and the
    /// active theme chose, so they win. The meter draws no root of its own —
    /// the bar it delegates to is the meter's root — so the override travels
    /// down with it.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// Sets the label text.
    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = Some(l.into());
        self
    }

    /// Show `value/max` next to the label (like `Meter.Output`).
    pub fn show_value(mut self, v: bool) -> Self {
        self.show_value = v;
        self
    }
}

impl RenderOnce for Meter {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // `useMeter` delegates every attribute to `useProgressBar` and
        // changes only the role, so the delegation carries it.
        let mut p = ProgressBar::new(self.id)
            .as_meter()
            .value(self.value)
            .min_value(self.min_value)
            .max_value(self.max_value)
            .size(self.size)
            .color(self.color)
            .show_value_label(self.show_value);
        if let Some(format) = self.format.clone() {
            p = p.format_options(format);
        }
        if let Some(render) = self.value_content {
            p = p.value_content(move |percentage, text, _| render(percentage, text));
        }
        if let Some(vl) = self.value_label {
            p = p.value_label(vl);
        }
        if let Some(l) = self.label {
            p = p.label(l.to_string());
        }
        // The bar owns the geometry: it paints the track and its fill, so the
        // radius rides the delegation like the `sx` refinement below.
        if let Some(radius) = self.radius {
            p = p.radius(radius);
        }
        // The bar's root is this meter's root: hand the refinement down rather
        // than box the bar in a wrapper of its own.
        if let Some(sx) = self.sx {
            p = p.sx_refinement(sx);
        }
        p.into_any_element()
    }
}

crate::util::impl_component_styled!(Meter);
