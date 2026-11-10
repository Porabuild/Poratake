//! Typography — port of `@heroui/typography` (HeroUI v3.2.4).
//!
//! Semantic typography primitives for headings, body copy and inline code.
//! Mirrors the React API: `type`, `align`, `color`, `weight`, `truncate`,
//! plus the `Typography.Heading` / `Typography.Paragraph` / `Typography.Code`
//! / `Typography.Prose` convenience primitives.
//!
//! Paint-only omissions, each because GPUI 0.2.2 lacks the primitive: the
//! headings' `tracking-tight` (no letter-spacing), `align: justify` (falls
//! back to start alignment), and the semantic `h1`–`h6`/`p`/`code` elements
//! (rendered as plain divs). The mono family is Tailwind's default mono stack
//! resolved on Windows. Upstream's per-tag `Prose` descendant styles cannot
//! be ported because GPUI has no ancestor context propagation.

use gpui::{
    div, px, AnyElement, App, IntoElement, ParentElement, Pixels, RenderOnce, SharedString, Styled,
    Window,
};
use herogpui_theme::ActiveTheme;

use crate::util::MONO_FONT;

/// Semantic typography style (`type` prop).
///
/// `(font-size, line-height)` pairs resolve the pinned `typography.css`
/// through Tailwind 4.3.0's default text scale: headings use the default
/// `text-*` leading (`h1` 36/40 through `h6` 16/24), body `leading-7`
/// (16/28), body-sm `leading-6` (14/24), body-xs `leading-5` (12/20) and
/// code the plain `text-sm` leading (14/20).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TypographyType {
    /// Level-1 heading (36/40).
    H1,
    /// Level-2 heading (30/36).
    H2,
    /// Level-3 heading (24/32).
    H3,
    /// Level-4 heading (20/28).
    H4,
    /// Level-5 heading (18/28).
    H5,
    /// Level-6 heading (16/24).
    H6,
    /// Body text (16/28); the default.
    #[default]
    Body,
    /// Small body text (14/24).
    BodySm,
    /// Extra-small body text (12/20).
    BodyXs,
    /// Monospace code text (14/20).
    Code,
}

impl TypographyType {
    /// `(font-size, line-height)` in pixels.
    pub fn metrics(self) -> (Pixels, Pixels) {
        match self {
            Self::H1 => (px(36.), px(40.)),
            Self::H2 => (px(30.), px(36.)),
            Self::H3 => (px(24.), px(32.)),
            Self::H4 => (px(20.), px(28.)),
            Self::H5 => (px(18.), px(28.)),
            Self::H6 => (px(16.), px(24.)),
            Self::Body => (px(16.), px(28.)),
            Self::BodySm => (px(14.), px(24.)),
            Self::BodyXs => (px(12.), px(20.)),
            Self::Code => (px(14.), px(20.)),
        }
    }

    /// Default weight for this type — headings are semibold, body normal.
    pub fn default_weight(self) -> FontWeight {
        match self {
            Self::H1 | Self::H2 | Self::H3 | Self::H4 | Self::H5 | Self::H6 => FontWeight::Semibold,
            _ => FontWeight::Normal,
        }
    }

    /// Whether this type renders in the monospace family.
    pub fn is_mono(self) -> bool {
        matches!(self, Self::Code)
    }

    /// Maps a heading level (1-6) to the matching type; higher levels clamp.
    pub fn heading(level: u8) -> Self {
        match level {
            1 => Self::H1,
            2 => Self::H2,
            3 => Self::H3,
            4 => Self::H4,
            5 => Self::H5,
            _ => Self::H6,
        }
    }
}

/// Text alignment (`align` prop).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    /// Aligned to the start; renders left-aligned.
    #[default]
    Start,
    /// Centred.
    Center,
    /// Aligned to the end; renders right-aligned.
    End,
    /// Justified; currently renders left-aligned.
    Justify,
}

/// Text color (`color` prop).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextColor {
    /// The default foreground colour.
    #[default]
    Default,
    /// The muted foreground colour.
    Muted,
}

/// Font weight override (`weight` prop).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontWeight {
    /// Normal weight.
    Normal,
    /// Medium weight.
    Medium,
    /// Semibold weight.
    Semibold,
    /// Bold weight.
    Bold,
}

impl FontWeight {
    fn to_gpui(self) -> gpui::FontWeight {
        match self {
            Self::Normal => gpui::FontWeight::NORMAL,
            Self::Medium => gpui::FontWeight::MEDIUM,
            Self::Semibold => gpui::FontWeight::SEMIBOLD,
            Self::Bold => gpui::FontWeight::BOLD,
        }
    }
}

/// Paragraph size for [`Typography::paragraph`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ParagraphSize {
    /// Body size (`TypographyType::Body`).
    #[default]
    Base,
    /// Small size (`TypographyType::BodySm`).
    Sm,
    /// Extra-small size (`TypographyType::BodyXs`).
    Xs,
}

/// HeroUI Typography.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Typography {
    kind: TypographyType,
    align: TextAlign,
    color: TextColor,
    weight: Option<FontWeight>,
    truncate: bool,
    /// The family every kind is drawn with; unset keeps the kind's own
    /// family (mono for `Code`, inherited otherwise).
    font_family: Option<SharedString>,
    /// The `Code` chip's corner radius, in place of the owning `mark_radius`
    /// helper. `Code` is the only kind that paints a box, so the others
    /// ignore it.
    radius: Option<Pixels>,
    text: Option<SharedString>,
    children: Vec<AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Typography {
    /// Creates a text element with the given content and default styling.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            kind: TypographyType::default(),
            align: TextAlign::default(),
            color: TextColor::default(),
            weight: None,
            truncate: false,
            font_family: None,
            radius: None,
            text: Some(text.into()),
            children: Vec::new(),
            sx: None,
        }
    }

    /// `Typography.Heading level={1..6}`.
    pub fn heading(level: u8, text: impl Into<SharedString>) -> Self {
        Self::new(text).kind(TypographyType::heading(level))
    }

    /// `Typography.Paragraph size="base" | "sm" | "xs"`.
    pub fn paragraph(size: ParagraphSize, text: impl Into<SharedString>) -> Self {
        Self::new(text).kind(match size {
            ParagraphSize::Base => TypographyType::Body,
            ParagraphSize::Sm => TypographyType::BodySm,
            ParagraphSize::Xs => TypographyType::BodyXs,
        })
    }

    /// `Typography.Code`.
    pub fn code(text: impl Into<SharedString>) -> Self {
        Self::new(text).kind(TypographyType::Code)
    }

    /// The `type` prop — named `kind` because `type` is a Rust keyword.
    pub fn kind(mut self, kind: TypographyType) -> Self {
        self.kind = kind;
        self
    }

    /// Sets the text alignment.
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Sets the text colour.
    pub fn color(mut self, color: TextColor) -> Self {
        self.color = color;
        self
    }

    /// Sets the font weight, overriding the type's default weight.
    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = Some(weight);
        self
    }

    /// Sets whether overflowing text is truncated.
    pub fn truncate(mut self, v: bool) -> Self {
        self.truncate = v;
        self
    }

    /// The family the text is drawn with; unset keeps the kind's own family
    /// (mono for `Code`, inherited otherwise).
    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// The `code` chip's corner radius, in place of the owning `mark_radius`
    /// helper. `Typography::code` is the only kind that paints a box, so every
    /// other kind ignores this. Not a v3 prop; the removed v2 `radius` prop is
    /// prohibited and this is a per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the typography's root element after every value the kind, the
    /// color and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl ParentElement for Typography {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Typography {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.colors();
        let (size, line_height) = self.kind.metrics();
        let weight = self.weight.unwrap_or_else(|| self.kind.default_weight());

        let mut el = div()
            .text_size(size)
            .line_height(line_height)
            .font_weight(weight.to_gpui())
            .text_color(match self.color {
                TextColor::Default => colors.foreground,
                TextColor::Muted => colors.muted,
            });

        // `typography--code` paints the `rounded-md bg-default px-1.5
        // py-0.5` chip on top of the mono `text-sm` run.
        if self.kind.is_mono() {
            el = el
                .font_family(MONO_FONT)
                .bg(colors.default.color)
                .rounded(self.radius.unwrap_or_else(|| crate::util::mark_radius(cx)))
                .px(px(6.))
                .py(px(2.));
        }
        if let Some(family) = self.font_family.clone() {
            el = el.font_family(family);
        }

        // gpui has no `text-justify`; justify falls back to start alignment.
        el = match self.align {
            TextAlign::Start | TextAlign::Justify => el.text_left(),
            TextAlign::Center => el.text_center(),
            TextAlign::End => el.text_right(),
        };

        if self.truncate {
            el = el.truncate();
        }

        if let Some(text) = self.text {
            el = el.child(text.to_string());
        }
        el = el.children(self.children);
        el = crate::util::apply_sx(el, &self.sx);
        el
    }
}

/// `Typography.Prose` — upstream's `.typography-prose` block: a plain
/// container that sets `text-foreground` and renders its children in order.
/// The per-tag descendant styles (`p`, `code`, `a`, lists, …) cannot be
/// ported because GPUI has no ancestor context propagation; children must be
/// already-semantic elements such as [`Typography`], which carry their own
/// metrics.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Prose {
    children: Vec<AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Prose {
    /// Creates an empty prose container.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            sx: None,
        }
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the prose block's root element after every value the active
    /// theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for Prose {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for Prose {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Prose {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let el = div()
            .text_color(cx.colors().foreground)
            .children(self.children);
        crate::util::apply_sx(el, &self.sx)
    }
}

crate::util::impl_component_styled!(Typography, Prose);
