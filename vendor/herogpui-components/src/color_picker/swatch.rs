//! ColorSwatch.

use super::*;

// ColorSwatch
// ---------------------------------------------------------------------------

/// ColorSwatch — previews one color value.
///
/// Translucent colors are drawn over a checkerboard so the alpha is visible.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ColorSwatch {
    color: PickerColor,
    size: SizeXl,
    shape: SwatchShape,
    /// `colorName` — a caller-provided accessible name. When omitted, the
    /// swatch reports the color's canonical hex string, matching the
    /// primitive's generated description.
    color_name: Option<SharedString>,
    /// `ColorSwatchPicker.Item.isDisabled` — the item's own flag, drawn on
    /// the swatch it wraps.
    is_disabled: bool,
    id: Option<ElementId>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl ColorSwatch {
    /// `color` — also accepted positionally by [`ColorSwatch::new`].
    pub fn color(mut self, color: PickerColor) -> Self {
        self.color = color;
        self
    }

    /// Creates a swatch showing `color`, at the default size and shape.
    pub fn new(color: PickerColor) -> Self {
        Self {
            color,
            // `.color-swatch` is `size-8` (32px), which is `SizeXl::Md` on v3's
            // own swatch scale (16/24/32/36/40).
            size: SizeXl::Md,
            shape: SwatchShape::Circle,
            color_name: None,
            is_disabled: false,
            id: None,
            sx: None,
        }
    }

    /// Names this swatch so it can report `role="img"`. Unnamed swatches
    /// produce no AccessKit node — a constant id would fold every instance
    /// into one image.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
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

    /// `colorName` — overrides the generated hex name exposed to assistive
    /// technology. This is useful when a design token or a human color name
    /// is more meaningful than the raw value.
    pub fn color_name(mut self, name: impl Into<SharedString>) -> Self {
        self.color_name = Some(name.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the swatch's root element after every value the size, the
    /// shape and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }

    /// `ColorSwatchPicker.Item.isDisabled` — the swatch an item wraps when it
    /// cannot be chosen.
    ///
    /// The picker draws each item around its swatch and the item's disabled
    /// state is the part's prop; a standalone preview honours the same flag
    /// by dimming — the reduced-opacity look the picker's sheet gives a
    /// disabled item.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }
}

impl RenderOnce for ColorSwatch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.colors();
        let layout = cx.layout();
        let edge = self.size.swatch_px();
        // `.color-swatch--circle` names a radius per size -- `rounded-lg` at 16px
        // through `rounded-3xl` at 40 -- and every one of them is at least half
        // the edge, so the shape is a circle at every size. `--square` is
        // `rounded-md` throughout.
        let radius = match self.shape {
            SwatchShape::Circle => px(f32::from(edge) / 2.),
            SwatchShape::Square => cx.layout().radius_md(),
        };

        // The fill stack sits inside the border, so it is `border_width`
        // smaller on every side than `edge`: a silhouette sized to `edge`
        // would overhang the corner by the border width. Every layer keeps
        // the swatch's own radius, as CSS paints the background under the
        // border with the outer curve, so the layers' curves coincide and no
        // corner sliver of one shows past another.
        let border = layout.border_width;
        let inner = px((f32::from(edge) - 2.0 * f32::from(border)).max(0.0));
        // `.color-swatch` always has the checker background in CSS, and its
        // light base stays here too: the colour fill's antialiased edge
        // blends over it exactly as the CSS layers do. The dark-cell
        // silhouette is only painted when the colour is translucent: an
        // opaque colour never lets it through in CSS (the swatch's `opacity`
        // dims the group as one), while here every layer dims on its own.
        let translucent = self.color.to_hsla().a < 1.0;
        let checker_opacity = if self.is_disabled {
            layout.disabled_opacity
        } else {
            1.0
        };
        let el = div()
            .size(edge)
            .rounded(radius)
            .flex_shrink_0()
            .overflow_hidden()
            .border(border)
            .border_color(colors.border)
            .when(self.is_disabled, |el| el.opacity(layout.disabled_opacity))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .rounded(radius)
                    .overflow_hidden()
                    // Light cells are the base's own background (always
                    // radius-true); dark cells are one clipped silhouette
                    // (see `transparency_checker_cells`), so translucent
                    // colors keep exact corners on vanilla GPUI.
                    .bg(gpui::rgb(CHECKER_LIGHT))
                    .when(translucent, |base| {
                        base.child(transparency_checker_cells(
                            inner,
                            inner,
                            radius,
                            checker_opacity,
                        ))
                    })
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .rounded(radius)
                            .bg(self.color.to_hsla()),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .rounded(radius)
                            .shadow(vec![color_inner_shadow()]),
                    ),
            );
        let el = util::apply_sx(el, &self.sx);
        match self.id {
            Some(id) => {
                let accessible_name = self
                    .color_name
                    .unwrap_or_else(|| SharedString::from(self.color.to_hex()));
                el.id(id)
                    .a11y_named(a11y::Role::Image, &a11y::Name::labelled(accessible_name))
                    .into_any_element()
            }
            None => el.into_any_element(),
        }
    }
}

// ---------------------------------------------------------------------------

crate::util::impl_component_styled!(ColorSwatch);
