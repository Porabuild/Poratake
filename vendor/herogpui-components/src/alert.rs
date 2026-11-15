//! Alert — port of `@heroui/alert`.

use gpui::{
    px, AnyElement, App, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
    SharedString, Styled, Window,
};
use herogpui_core::Color;
use herogpui_theme::ActiveTheme;

use crate::icons;

/// HeroUI Alert.
///
/// v3.2.4's API table carries only `status`/`className`/`children`: the
/// migration guide explicitly removes `isClosable`, `onClose` and
/// `closeButtonProps`, so a close affordance is composed by the caller as an
/// ordinary child (a `CloseButton`) instead of being built in.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Alert {
    title: SharedString,
    description: Option<SharedString>,
    color: Color,
    /// `Alert.Indicator` children replace the status glyph while retaining
    /// the pinned 24px indicator box and status-owned default fallback.
    indicator: Option<AnyElement>,
    /// Rich content inside the content column after the description — v3's
    /// `Alert.Description` accepts ReactNode, so a bulleted list or link
    /// can follow the description text. The `description` builder stays
    /// text-only; this is its element tail.
    description_content: Option<AnyElement>,
    /// Composed children — v3's "Additional content like buttons, close
    /// button, etc.", appended after the content column.
    children: Vec<AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
    /// The corner radius, in place of the owning `control_radius` helper.
    radius: Option<Pixels>,
}

impl Alert {
    /// `status` — the v3 name for `color`; the values are the same
    /// semantic roles.
    pub fn status(mut self, status: Color) -> Self {
        self.color = status;
        self
    }

    /// Creates an alert with the given title.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            color: Color::Default,
            indicator: None,
            description_content: None,
            children: Vec::new(),
            sx: None,
            radius: None,
        }
    }

    /// Sets the description shown under the title.
    pub fn description(mut self, d: impl Into<SharedString>) -> Self {
        self.description = Some(d.into());
        self
    }

    /// Replaces the default status glyph inside the indicator slot. The
    /// caller owns the child content; the surrounding box keeps HeroUI's
    /// 4px inset and fixed 16px glyph footprint.
    pub fn indicator(mut self, content: impl IntoElement) -> Self {
        self.indicator = Some(content.into_any_element());
        self
    }

    /// Rich content rendered inside the content column after the
    /// description — the element tail of v3's `Alert.Description`, whose
    /// ReactNode children can be a list, a link, or any markup the
    /// text-only `description` builder cannot express.
    pub fn description_content(mut self, content: impl IntoElement) -> Self {
        self.description_content = Some(content.into_any_element());
        self
    }

    /// The corner radius, in place of the owning `control_radius` helper. Not a
    /// v3 prop; the removed v2 `radius` prop is prohibited and this is a
    /// per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the alert's root element after every value the status and
    /// the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl ParentElement for Alert {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        // End content; see the struct doc for the v3 composition rationale.
        self.children.extend(elements);
    }
}

impl RenderOnce for Alert {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let sem = cx.role(self.color);
        let colors = cx.colors();
        let layout = cx.layout();

        // v3 dropped Alert's `variant`: the color role is the only axis.
        // `.alert` is `bg-surface` for every status -- the role never paints
        // the container.
        let bg = colors.surface.background;
        let fg = colors.foreground;
        // `.alert--default` paints the title *and* the indicator
        // `text-foreground`; every status paints `text-{role}-soft-foreground`.
        let role_fg = if self.color == Color::Default {
            colors.foreground
        } else {
            sem.soft_foreground(colors.foreground)
        };

        // `alert.tsx`'s `getDefaultIcon`: accent and the `default` fall-through
        // both draw the Info glyph; success the circled check (the pinned
        // `SuccessIcon`), warning the triangle, danger the circle-exclamation.
        let glyph = match self.color {
            Color::Default | Color::Accent => icons::INFO_CIRCLE,
            Color::Success => icons::CHECK_CIRCLE,
            Color::Warning => icons::WARNING_TRIANGLE,
            Color::Danger => icons::CIRCLE_EXCLAMATION,
        };

        let radius = self
            .radius
            .unwrap_or_else(|| crate::util::control_radius(cx));
        let mut alert = gpui::div()
            .flex()
            .items_start()
            .justify_start()
            .gap(px(16.))
            .w_full()
            .px(px(16.))
            .py(px(12.))
            .rounded(radius)
            .bg(bg)
            .text_color(fg)
            .debug_selector(|| "alert-root".to_owned());
        // `shadow-surface` is the surface elevation token; dark mode leaves
        // the token empty, and GPUI 0.2.2 paints an empty shadow list as
        // nothing, so the token is applied unconditionally.
        alert = alert.shadow(layout.surface_shadow.clone());

        let default_indicator_glyph = gpui::svg()
            .size(px(16.))
            .path(glyph)
            .text_color(role_fg)
            .flex_shrink_0();
        let indicator_glyph = self
            .indicator
            .unwrap_or_else(|| default_indicator_glyph.into_any_element());
        alert = alert.child(
            // `.alert__indicator` is a `p-1` box around the 16px glyph, centered.
            gpui::div()
                .p(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .debug_selector(|| "alert-indicator".to_owned())
                .child(indicator_glyph),
        );

        // `.alert__content` is the column that holds the title and the
        // description, beside the indicator. It carries no gap: the pinned
        // rule is only `flex h-full grow flex-col items-start`.
        // `min_w_0` is what lets the description wrap. A flex item's automatic
        // minimum size is its content's, and gpui measures a text child's
        // minimum as the whole string, so `grow` alone pushed the column --
        // and the copy -- straight out through the alert's right edge.
        let mut text_col = gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .flex_1()
            .min_w_0();
        text_col = text_col.child(
            gpui::div()
                .text_size(px(14.)) // `.alert__title` is `text-sm leading-6 font-medium`.
                .line_height(px(24.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(role_fg)
                .debug_selector(|| "alert-title".to_owned())
                .child(self.title.to_string()),
        );
        text_col = text_col.debug_selector(|| "alert-content".to_owned());
        if let Some(desc) = self.description {
            text_col = text_col.child(
                gpui::div()
                    // `.alert__description` is `text-sm`.
                    .text_size(px(14.))
                    // `text-sm`'s own line height is 20px.
                    .line_height(px(20.))
                    .text_color(colors.muted)
                    .debug_selector(|| "alert-description".to_owned())
                    .child(desc.to_string()),
            );
        }
        if let Some(extra) = self.description_content {
            text_col = text_col.child(extra);
        }
        alert = alert.child(text_col);

        // Composed children go last; see the struct doc.
        alert = alert.children(self.children);

        alert = crate::util::apply_sx(alert, &self.sx);
        alert
    }
}

// The pinned `.alert` is `bg-surface` for every status: the role paints the
// indicator and the title only, never the container. A soft wash looks
// plausible on screen, so the check is mechanical.
#[cfg(test)]
mod painted_tokens {
    fn implementation() -> &'static str {
        include_str!("alert.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present")
    }

    #[test]
    fn the_alert_container_is_always_surface() {
        let source = implementation();
        assert!(
            source.contains("let bg = colors.surface.background;"),
            "every status must paint `bg-surface` (pinned `.alert`)"
        );
        assert!(
            !source.contains("sem.soft()"),
            "no alert container may paint a role soft background"
        );
    }

    #[test]
    fn the_indicator_follows_the_status_soft_foreground() {
        let source = implementation();
        // The glyph's own `let` declaration is the structural unit: everything
        // between `let indicator_glyph = gpui::svg()` and the statement's
        // terminating `;` is the full builder chain that paints the glyph.
        let chain = source
            .split("let default_indicator_glyph = gpui::svg()")
            .nth(1)
            .expect("the indicator must paint the glyph as a declared svg element")
            .split(';')
            .next()
            .expect("the declaration must terminate");
        assert!(
            chain.contains(".text_color(role_fg)"),
            "the indicator must paint the same token as the title: \
             `text-foreground` on default, `text-{{role}}-soft-foreground` \
             otherwise (pinned `.alert__indicator`)"
        );
        assert!(
            !chain.contains("colors.muted"),
            "the default indicator is `text-foreground`, not the muted tone"
        );
    }

    #[test]
    fn custom_indicator_keeps_the_shared_indicator_box() {
        let source = implementation();
        assert!(source.contains("pub fn indicator(mut self, content: impl IntoElement)"));
        assert!(source.contains(".indicator\n            .unwrap_or_else"));
        assert!(source.contains(".p(px(4.))"));
        assert!(source.contains(".size(px(16.))"));
    }
}

crate::util::impl_component_styled!(Alert);
