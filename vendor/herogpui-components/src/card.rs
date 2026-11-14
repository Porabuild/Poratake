//! Card — port of `@heroui/card`.

use gpui::{
    prelude::*, px, AnyElement, App, IntoElement, ParentElement, Pixels, RenderOnce, Window,
};
use herogpui_theme::ActiveTheme;

/// Card prominence level. Every fill level paints its surface shade and
/// carries the surface shadow; `transparent` paints nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CardVariant {
    /// No background — for cards with custom painting.
    Transparent,
    /// `bg-surface`
    #[default]
    Default,
    /// `bg-surface-secondary`
    Secondary,
    /// `bg-surface-tertiary`
    Tertiary,
}

impl CardVariant {
    /// Every card variant, in declaration order.
    pub const ALL: [CardVariant; 4] = [
        CardVariant::Transparent,
        CardVariant::Default,
        CardVariant::Secondary,
        CardVariant::Tertiary,
    ];

    /// A human-readable label for this variant.
    pub fn label(self) -> &'static str {
        match self {
            CardVariant::Transparent => "Transparent",
            CardVariant::Default => "Default",
            CardVariant::Secondary => "Secondary",
            CardVariant::Tertiary => "Tertiary",
        }
    }
}

/// HeroUI Card container.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Card {
    variant: CardVariant,
    width: Option<Pixels>,
    children: Vec<AnyElement>,
    /// The corner radius, in place of `min(32px, --radius-3xl)`.
    radius: Option<Pixels>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Card {
    /// Creates an empty card.
    pub fn new() -> Self {
        Self {
            variant: CardVariant::Default,
            width: None,
            children: Vec::new(),
            radius: None,
            sx: None,
        }
    }

    /// Sets the card variant.
    pub fn variant(mut self, variant: CardVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Fixed card width.
    pub fn w(mut self, v: impl Into<Pixels>) -> Self {
        self.width = Some(v.into());
        self
    }

    /// The corner radius, in place of `min(32px, --radius-3xl)`. Not a v3
    /// prop; the removed v2 `radius` prop is prohibited and this is a
    /// per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the card's root element after every value the variant and
    /// the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for Card {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.colors();
        let layout = cx.layout();
        // `.card` is `flex flex-col gap-3 p-4`: the card is the padded box and
        // its parts (`__header`, `__content`, `__footer`) carry none of their
        // own, which is why this used to double the inset on every section.
        let mut el = gpui::div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(16.))
            .rounded(
                self.radius
                    .unwrap_or_else(|| crate::util::container_radius(cx)),
            )
            .children(self.children);
        // Upstream `.card` is `overflow-visible`: no clipping call here.

        if let Some(w) = self.width {
            el = el.w(w);
        }

        // `card--default`/`--secondary`/`--tertiary` set the background;
        // `card--transparent` is `border-none bg-transparent shadow-none`, so
        // it paints nothing and keeps the full content box for its parts.
        el = match self.variant {
            CardVariant::Transparent => el,
            CardVariant::Default => el.bg(colors.surface.background),
            CardVariant::Secondary => el.bg(colors.surface_secondary),
            CardVariant::Tertiary => el.bg(colors.surface_tertiary),
        };
        if self.variant != CardVariant::Transparent && !layout.surface_shadow.is_empty() {
            el = el.shadow(layout.surface_shadow.clone());
        }

        el = crate::util::apply_sx(el, &self.sx);
        el
    }
}

/// Card header section (`CardHeader`).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct CardHeader {
    children: Vec<AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl CardHeader {
    /// Creates an empty instance.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            sx: None,
        }
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the header's root element after every value the card and the
    /// active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for CardHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for CardHeader {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for CardHeader {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // `.card__header` is `flex flex-col` and nothing else: the title's
        // text style belongs to `CardTitle` and the description's to
        // `CardDescription`.
        let el = gpui::div().flex().flex_col().children(self.children);
        crate::util::apply_sx(el, &self.sx)
    }
}

/// Card title (`CardTitle`, upstream `.card__title`).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct CardTitle {
    children: Vec<AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl CardTitle {
    /// Creates an empty instance.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            sx: None,
        }
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the title's root element after every value the card and the
    /// active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for CardTitle {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for CardTitle {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for CardTitle {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `.card__title` is `text-sm leading-6 font-medium text-foreground`.
        let colors = cx.colors();
        let el = gpui::div()
            .text_size(px(14.))
            .line_height(px(24.))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(colors.foreground)
            .children(self.children);
        crate::util::apply_sx(el, &self.sx)
    }
}

/// Card description (`CardDescription`, upstream `.card__description`).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct CardDescription {
    children: Vec<AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl CardDescription {
    /// Creates an empty instance.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            sx: None,
        }
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the description's root element after every value the card and
    /// the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for CardDescription {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for CardDescription {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for CardDescription {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `.card__description` is `text-sm leading-5 text-muted`.
        let colors = cx.colors();
        let el = gpui::div()
            .text_size(px(14.))
            .line_height(px(20.))
            .text_color(colors.muted)
            .children(self.children);
        crate::util::apply_sx(el, &self.sx)
    }
}

/// Content section (`CardContent`, upstream `.card__content`).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct CardContent {
    children: Vec<AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl CardContent {
    /// Creates an empty instance.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            sx: None,
        }
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the content's root element after every value the card and the
    /// active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for CardContent {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for CardContent {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for CardContent {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // `.card__content` is `flex flex-1 flex-col gap-1`. The upstream
        // `flex-1` is dropped: the pinned-geometry test in `tests/card_deep.rs`
        // measures the card as an auto-height column hugging its parts, which
        // flex-1 regresses.
        let el = gpui::div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .children(self.children);
        crate::util::apply_sx(el, &self.sx)
    }
}

/// Card footer section (`CardFooter`).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct CardFooter {
    children: Vec<AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl CardFooter {
    /// Creates an empty instance.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            sx: None,
        }
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the footer's root element after every value the card and the
    /// active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for CardFooter {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for CardFooter {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for CardFooter {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // `.card__footer` is `flex flex-row items-center` -- no padding, gap,
        // or text size of its own; the card's gap separates the parts and the
        // caller composes the row's contents.
        let el = gpui::div().flex().items_center().children(self.children);
        crate::util::apply_sx(el, &self.sx)
    }
}

crate::util::impl_component_styled!(
    Card,
    CardHeader,
    CardTitle,
    CardDescription,
    CardContent,
    CardFooter
);
