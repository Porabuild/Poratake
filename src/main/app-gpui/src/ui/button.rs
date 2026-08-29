//! Button kit — 1:1 port of the HeroUI button styles used by the Electron
//! renderer (`components/button.css` + `base.css` `.button--xs` overrides).

use gpui::{
    div, prelude::*, px, AnimationExt, ClickEvent, Div, ElementId, FontWeight, Hsla,
    InteractiveElement, Stateful, StatefulInteractiveElement, StyleRefinement, Styled,
};

use crate::theme::vars::{active_theme, ThemeVars};
use crate::ui::chrome;
use crate::ui::icon::Icon;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Tertiary,
    Ghost,
    Outline,
    #[allow(dead_code)]
    Danger,
    Link,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[allow(dead_code)]
pub enum ButtonSize {
    #[default]
    Md,
    Sm,
    Lg,
    Xs,
    Icon,
    IconXs,
    IconSm,
    IconLg,
}

impl ButtonSize {
    fn height(self) -> gpui::Pixels {
        match self {
            Self::Md | Self::Icon => px(chrome::BUTTON_MD_HEIGHT),
            Self::Sm | Self::IconSm => px(chrome::BUTTON_SM_HEIGHT),
            Self::IconXs => px(chrome::TOOL_BUTTON_SIZE),
            Self::Lg | Self::IconLg => px(chrome::BUTTON_LG_HEIGHT),
            Self::Xs => px(chrome::BUTTON_XS_HEIGHT),
        }
    }

    #[allow(dead_code)]
    pub fn chrome_size(self) -> f32 {
        f32::from(self.height())
    }

    fn padding_x(self) -> gpui::Pixels {
        match self {
            Self::Md | Self::Icon => px(chrome::BUTTON_MD_PAD_X),
            Self::Sm => px(chrome::BUTTON_SM_PAD_X),
            Self::Lg => px(chrome::BUTTON_MD_PAD_X),
            Self::Xs => px(chrome::BUTTON_XS_PAD_X),
            Self::IconXs | Self::IconSm | Self::IconLg => px(0.0),
        }
    }

    /// Icon-only buttons are square.
    fn width(self) -> Option<gpui::Pixels> {
        if matches!(
            self,
            Self::Icon | Self::IconXs | Self::IconSm | Self::IconLg
        ) {
            Some(self.height())
        } else {
            None
        }
    }

    fn text_size(self) -> gpui::Pixels {
        match self {
            Self::Lg => px(chrome::BUTTON_LG_TEXT),
            Self::Xs => px(chrome::BUTTON_XS_TEXT),
            _ => px(chrome::BUTTON_MD_TEXT),
        }
    }

    /// The size an icon gets when the call site does not pass one, mirroring
    /// `.button--xs svg:not([class*='size-'])` (0.875rem) and the base
    /// `.button svg` rule (`sm:size-4`). Renderer call sites that pass an
    /// explicit `size-*` on the icon must pass [`Button::icon_size`] here.
    fn icon_size(self) -> gpui::Pixels {
        match self {
            Self::Xs | Self::IconXs => px(chrome::BUTTON_XS_ICON),
            _ => px(chrome::TOOL_BUTTON_ICON),
        }
    }

    /// The `scale()` a press applies in `button.css`.
    pub fn press_scale(self) -> f32 {
        match self {
            Self::Sm | Self::IconSm | Self::Xs | Self::IconXs => chrome::BUTTON_PRESS_SCALE_SM,
            Self::Lg | Self::IconLg => chrome::BUTTON_PRESS_SCALE_LG,
            Self::Md | Self::Icon => chrome::BUTTON_PRESS_SCALE_MD,
        }
    }

    fn is_icon_only(self) -> bool {
        matches!(
            self,
            Self::Icon | Self::IconXs | Self::IconSm | Self::IconLg
        )
    }
}

fn variant_colors(variant: ButtonVariant, theme: &ThemeVars) -> (Hsla, Hsla, Hsla) {
    // (background, hover-background, foreground)
    match variant {
        ButtonVariant::Primary => (theme.accent, theme.accent_hover, theme.accent_foreground),
        ButtonVariant::Secondary | ButtonVariant::Tertiary => {
            (theme.default, theme.default_hover, theme.foreground)
        }
        ButtonVariant::Ghost | ButtonVariant::Link => (
            theme.background.opacity(0.0),
            theme.default,
            theme.default_foreground,
        ),
        ButtonVariant::Outline => (
            theme.background.opacity(0.0),
            mix_default_transparent(theme),
            theme.default_foreground,
        ),
        ButtonVariant::Danger => (theme.danger, theme.danger_hover, theme.danger_foreground),
    }
}

fn mix_default_transparent(theme: &ThemeVars) -> Hsla {
    // color-mix(in srgb, var(--default) 60%, transparent)
    let mut mixed = theme.default;
    mixed.a *= 0.6;
    mixed
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<gpui::SharedString>,
    icon: Option<&'static str>,
    trailing_icon: Option<&'static str>,
    extra_children: Vec<gpui::AnyElement>,
    variant: ButtonVariant,
    size: ButtonSize,
    icon_size: Option<gpui::Pixels>,
    icon_spinning: bool,
    height: Option<gpui::Pixels>,
    min_width: Option<gpui::Pixels>,
    font_weight: Option<FontWeight>,
    padding_x: Option<gpui::Pixels>,
    gap: Option<gpui::Pixels>,
    disabled: bool,
    selected: bool,
    full_width: bool,
    flex_1: bool,
    surface: Option<Hsla>,
    surface_hover: Option<Hsla>,
    foreground: Option<Hsla>,
    radius: Option<gpui::Pixels>,
    tooltip: Option<gpui::SharedString>,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static>>,
    on_press:
        Option<Box<dyn Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static>>,
    animate_press: bool,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            icon: None,
            trailing_icon: None,
            extra_children: Vec::new(),
            variant: ButtonVariant::default(),
            size: ButtonSize::default(),
            icon_size: None,
            icon_spinning: false,
            height: None,
            min_width: None,
            font_weight: None,
            padding_x: None,
            gap: None,
            disabled: false,
            selected: false,
            full_width: false,
            flex_1: false,
            surface: None,
            surface_hover: None,
            foreground: None,
            radius: None,
            tooltip: None,
            on_click: None,
            on_press: None,
            animate_press: true,
        }
    }

    /// Overrides the resting background, keeping the variant's hover behaviour.
    pub fn surface(mut self, color: Hsla) -> Self {
        self.surface = Some(color);
        self
    }

    pub fn surface_hover(mut self, color: Hsla) -> Self {
        self.surface_hover = Some(color);
        self
    }

    pub fn foreground(mut self, color: Hsla) -> Self {
        self.foreground = Some(color);
        self
    }

    pub fn radius(mut self, radius: gpui::Pixels) -> Self {
        self.radius = Some(radius);
        self
    }

    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// `flex-1`, for the call sites that share a row equally. `.button` is
    /// `shrink-0`, so two `w-full` buttons would overflow instead of splitting
    /// the row.
    pub fn flex_1(mut self) -> Self {
        self.flex_1 = true;
        self
    }

    /// Renders the button in its active state, mirroring the renderer's
    /// `variant={active ? 'secondary' : 'ghost'}` pattern.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn tooltip(mut self, text: impl Into<gpui::SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Appends arbitrary content (used by compact triggers like the stroke
    /// width selector).
    #[allow(dead_code)]
    pub fn child(mut self, element: impl Into<gpui::AnyElement>) -> Self {
        self.extra_children.push(element.into());
        self
    }

    pub fn label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn icon(mut self, name: &'static str) -> Self {
        self.icon = Some(name);
        self
    }

    pub fn trailing_icon(mut self, name: &'static str) -> Self {
        self.trailing_icon = Some(name);
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Overrides the glyph size, for the renderer call sites that put an
    /// explicit `size-*` on the icon instead of letting `button.css` size it.
    pub fn icon_size(mut self, size: gpui::Pixels) -> Self {
        self.icon_size = Some(size);
        self
    }

    /// Spins the leading glyph, for the renderer's `animate-spin` loaders.
    pub fn icon_spinning(mut self, spinning: bool) -> Self {
        self.icon_spinning = spinning;
        self
    }

    /// Overrides the height and, for icon-only buttons, the width — the
    /// `h-6`/`size-7!` utilities the renderer layers on top of a size variant.
    pub fn height(mut self, height: gpui::Pixels) -> Self {
        self.height = Some(height);
        self
    }

    /// The `min-w-*` some call sites add so a label of varying length does not
    /// resize the button.
    pub fn min_width(mut self, width: gpui::Pixels) -> Self {
        self.min_width = Some(width);
        self
    }

    /// `.button` is `font-medium`; a few call sites override it to
    /// `font-normal`.
    pub fn font_weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = Some(weight);
        self
    }

    /// Overrides the inline padding, e.g. the `px-2` on the history chips.
    pub fn padding_x(mut self, padding: gpui::Pixels) -> Self {
        self.padding_x = Some(padding);
        self
    }

    /// Overrides the icon-to-label gap. `button.css` is `gap-2`, but some call
    /// sites space the glyph with `mr-1` instead.
    pub fn gap(mut self, gap: gpui::Pixels) -> Self {
        self.gap = Some(gap);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    pub fn on_press(
        mut self,
        handler: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    ) -> Self {
        self.on_press = Some(Box::new(handler));
        self
    }

    pub fn animate_press(mut self, animate: bool) -> Self {
        self.animate_press = animate;
        self
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let theme = active_theme(cx);
        let variant = if self.selected && self.variant == ButtonVariant::Ghost {
            ButtonVariant::Secondary
        } else {
            self.variant
        };
        let (variant_bg, variant_bg_hover, variant_fg) = variant_colors(variant, &theme);
        // `self.id` moves into the element below; the per-element state keys
        // need a stable copy of it.
        let element_key = format!("{}", self.id);
        let focus = crate::ui::primitives::control_focus(&element_key, self.disabled, window, cx);

        let height = self.height.unwrap_or(self.size.height());
        let pad_x = self.padding_x.unwrap_or(self.size.padding_x());
        let width = self.size.width().map(|w| self.height.unwrap_or(w));
        // Half the shrink on each side, so the box contracts about its centre.
        let (press_inset, pressed_height, pressed_pad_x) =
            press_geometry(f32::from(height), f32::from(pad_x), self.size.press_scale());
        let press_height = px(pressed_height);
        let press_width = width.map(|w| w - px(press_inset * 2.0));
        let press_pad_x = px(pressed_pad_x);
        let bg = self.surface.unwrap_or(variant_bg);
        let bg_hover = self.surface_hover.unwrap_or(variant_bg_hover);
        let fg = self.foreground.unwrap_or(variant_fg);

        let mut element: Stateful<Div> = div()
            .id(self.id)
            .track_focus(&focus)
            .focus(|style| style.shadow(crate::ui::primitives::focus_ring(&theme, 2.0)))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(self.gap.unwrap_or(px(chrome::BUTTON_GAP)))
            .flex_shrink_0()
            .rounded(self.radius.unwrap_or(px(chrome::BUTTON_RADIUS)))
            .h(height)
            .px(pad_x)
            .text_size(self.size.text_size())
            .font_weight(self.font_weight.unwrap_or(FontWeight::MEDIUM))
            .text_color(if self.disabled { fg.opacity(0.5) } else { fg })
            .bg(if self.disabled { bg.opacity(0.5) } else { bg })
            .when(self.full_width, |el: Stateful<Div>| el.w_full())
            .when(self.flex_1, |el: Stateful<Div>| el.flex_1().w_0())
            .when_some(self.min_width, |el: Stateful<Div>, width| el.min_w(width))
            .when_some(width, |el: Stateful<Div>, width| el.w(width));

        if variant == ButtonVariant::Outline {
            element = element.border_1().border_color(theme.border);
        }
        if variant == ButtonVariant::Link && self.foreground.is_none() {
            element = element.text_color(theme.accent);
        }
        if let Some(text) = self.tooltip {
            element = element.tooltip(move |_window, cx| {
                cx.new(|_| crate::ui::tooltip::Tooltip::new(text.clone()))
                    .into()
            });
        }
        if !self.disabled && self.animate_press {
            // `button.css` presses scale the button. gpui cannot transform a
            // `div` — only `MonochromeSprite` carries a matrix, and that is
            // reachable only through `paint_svg` — so the geometry is
            // reproduced instead: shrink the painted box and add exactly the
            // margin the shrink freed, which leaves the element's footprint
            // unchanged so nothing around it shifts. The glyphs keep their
            // size, which at these magnitudes is well under a device pixel.
            let inset = px(press_inset);
            element = element.active(move |style: StyleRefinement| {
                let style = style.bg(bg_hover);
                let style = style.h(press_height).my(inset);
                match press_width {
                    Some(width) => style.w(width).mx(inset),
                    None => style.px(press_pad_x).mx(inset),
                }
            });
        }

        let icon_size = self.icon_size.unwrap_or_else(|| self.size.icon_size());
        if let Some(icon_name) = self.icon {
            if self.icon_spinning {
                element = element.child(crate::ui::icon::spinner_element(
                    gpui::ElementId::Name(format!("{icon_name}-spinner").into()),
                    icon_size,
                ));
            } else if let Some(icon_element) = Icon::new(icon_name) {
                element = element.child(icon_element.size(icon_size));
            }
        }
        let has_label = self.label.is_some();
        if let Some(label_text) = self.label {
            element = element.child(label_text);
        }
        if let Some(trailing) = self.trailing_icon {
            if let Some(icon_element) = Icon::new(trailing) {
                element = element.child(icon_element.size(icon_size));
            }
        }
        for child in self.extra_children {
            element = element.child(child);
        }

        // Icon-only buttons center their glyph.
        if self.size.is_icon_only() && self.icon.is_none() && !has_label {
            element = element.p_0();
        }

        if !self.disabled {
            if let Some(handler) = self.on_press {
                element =
                    element.on_mouse_down(gpui::MouseButton::Left, move |event, window, cx| {
                        handler(event, window, cx);
                    });
            }
            if let Some(handler) = self.on_click {
                element = element.on_click(move |event, window, cx| handler(event, window, cx));
            }
        }

        if self.disabled {
            return element.into_any_element();
        }

        // The disabled path above never reaches this, so its hover state is
        // dropped with the frame (gpui discards an element's state when it
        // skips one) and re-enabled buttons always mount resting.
        // `.button { transition: background-color 100ms var(--ease-out) }`.
        let (hover, hovered, (from, to)) =
            crate::ui::primitives::hover_fade(&element_key, bg, bg_hover, window, cx);

        element = element.on_hover({
            let hover = hover.clone();
            move |over: &bool, _window, cx| {
                crate::ui::primitives::track_hover(&hover, *over, cx);
            }
        });

        element
            .with_animation(
                gpui::ElementId::Name(format!("{element_key}-hover-{hovered}").into()),
                gpui::Animation::new(std::time::Duration::from_millis(chrome::BUTTON_HOVER_MS))
                    .with_easing(crate::ui::primitives::ease_out()),
                move |button, delta| button.bg(crate::theme::color::lerp_srgb(from, to, delta)),
            )
            .into_any_element()
    }
}

/// How a press shrinks the painted box, and by how much the freed space is
/// given back as margin. Extracted so the invariant that matters — the
/// element's footprint does not change, so neighbours never shift — is
/// asserted rather than assumed.
fn press_geometry(height: f32, pad_x: f32, scale: f32) -> (f32, f32, f32) {
    let inset = height * (1.0 - scale) / 2.0;
    (inset, height - inset * 2.0, (pad_x - inset).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_shrinks_the_box_without_moving_its_neighbours() {
        for size in [
            ButtonSize::Md,
            ButtonSize::Sm,
            ButtonSize::Lg,
            ButtonSize::Xs,
        ] {
            let height = f32::from(size.height());
            let pad_x = f32::from(size.padding_x());
            let (inset, pressed_height, pressed_pad_x) =
                press_geometry(height, pad_x, size.press_scale());

            // Vertically: the shrink plus the margin it frees is the original.
            assert_eq!(pressed_height + inset * 2.0, height, "{size:?} height");
            // Horizontally, for a label button the padding absorbs the shrink,
            // so the content box and the outer footprint are both unchanged.
            assert_eq!(pressed_pad_x + inset, pad_x, "{size:?} padding");
            // The box really does get smaller.
            assert!(pressed_height < height, "{size:?} shrinks");
            assert!(inset > 0.0, "{size:?} has an inset");
        }
    }

    #[test]
    fn press_scales_match_the_button_stylesheet() {
        assert_eq!(ButtonSize::Md.press_scale(), 0.97);
        assert_eq!(ButtonSize::Icon.press_scale(), 0.97);
        assert_eq!(ButtonSize::Sm.press_scale(), 0.98);
        assert_eq!(ButtonSize::Xs.press_scale(), 0.98);
        assert_eq!(ButtonSize::Lg.press_scale(), 0.96);
    }

    /// A tiny button must not end up with negative padding.
    #[test]
    fn the_shrink_never_drives_padding_below_zero() {
        let (_, _, pad) = press_geometry(28.0, 0.0, 0.9);
        assert_eq!(pad, 0.0);
    }

    #[test]
    fn press_animation_can_be_disabled() {
        assert!(!Button::new("stationary").animate_press(false).animate_press);
    }
}
