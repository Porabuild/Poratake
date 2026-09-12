//! Bridge from the app's resolved theme tokens to HeroGPUI's `Theme`.
//!
//! The app computes its palette in `theme::vars::ThemeVars`, a port of
//! `app-theme.ts` resolving the CSS custom properties the design system is
//! built on. HeroGPUI components read their colours from the `ThemeProvider`
//! global, so every time the app's tokens change (`init_theme` / `update_theme`)
//! we rebuild the equivalent `herogpui::theme::Theme` and publish it with
//! `set_theme`. The two layers then agree on every token the components touch.

use super::vars::ThemeVars;
use gpui::prelude::*;
use herogpui::gpui;
use herogpui::theme::{
    ButtonStyle, ComponentColor, ComponentTheme, ComponentThemes, MenuStyle, SelectStyle,
    SliderStyle, SwitchStyle, TextFieldStyle,
};
use herogpui::{Color, FieldVariant};

fn component_themes() -> ComponentThemes {
    ComponentThemes::default()
        .slider(ComponentTheme::new(
            SliderStyle::default().radius(gpui::px(9999.0)),
        ))
        .switch(ComponentTheme::new(
            SwitchStyle::default().radius(gpui::px(9999.0)),
        ))
        .select(
            ComponentTheme::new(SelectStyle::default().variant(FieldVariant::Secondary)).recipe(
                "compact",
                SelectStyle::default()
                    .height(gpui::px(28.0))
                    .padding_x(gpui::px(8.0))
                    .trigger_text_size(gpui::px(12.0))
                    .row_height(gpui::px(28.0))
                    .row_padding_x(gpui::px(8.0))
                    .row_padding_y(gpui::px(2.0))
                    .row_text_size(gpui::px(12.0))
                    .panel_padding(gpui::px(4.0)),
            ),
        )
        .menu(
            ComponentTheme::new(
                MenuStyle::default()
                    .panel_gap(gpui::px(0.0))
                    .panel_padding(gpui::px(6.0))
                    .row_height(gpui::px(36.0))
                    .row_padding_x(gpui::px(10.0))
                    .row_padding_y(gpui::px(6.0))
                    .row_text_size(gpui::px(14.0))
                    .row_gap(gpui::px(12.0)),
            )
            .recipe(
                "compact",
                MenuStyle::default()
                    .panel_padding(gpui::px(4.0))
                    .row_height(gpui::px(28.0))
                    .row_padding_x(gpui::px(8.0))
                    .row_padding_y(gpui::px(2.0))
                    .row_text_size(gpui::px(12.0))
                    .row_gap(gpui::px(8.0)),
            )
            .recipe(
                "accent",
                MenuStyle::default()
                    .row_hover_bg(ComponentColor::Role(Color::Accent))
                    .row_hover_foreground(ComponentColor::RoleForeground(Color::Accent)),
            )
            .recipe(
                "neutral",
                MenuStyle::default().row_hover_bg(ComponentColor::RoleHover(Color::Default)),
            ),
        )
        .button(
            ComponentTheme::new(ButtonStyle::default())
                .recipe(
                    "compact",
                    ButtonStyle::default().style(|el| {
                        el.h(gpui::px(28.0))
                            .px(gpui::px(10.0))
                            .text_size(gpui::px(12.0))
                            .line_height(gpui::px(16.0))
                    }),
                )
                .recipe(
                    "compact-icon",
                    ButtonStyle::default().style(|el| el.size(gpui::px(28.0)).p(gpui::px(0.0))),
                )
                .recipe(
                    "muted",
                    ButtonStyle::default().foreground(ComponentColor::Muted),
                )
                .recipe(
                    "danger",
                    ButtonStyle::default().foreground(ComponentColor::Role(Color::Danger)),
                )
                .recipe("overlay", ButtonStyle::default().radius(gpui::px(6.0))),
        )
        .text_field(
            ComponentTheme::new(TextFieldStyle::default())
                .recipe(
                    "search",
                    TextFieldStyle::default()
                        .height(gpui::px(32.0))
                        .padding_x(gpui::px(10.0))
                        .text_size(gpui::px(13.0)),
                )
                .recipe(
                    "compact",
                    TextFieldStyle::default()
                        .height(gpui::px(28.0))
                        .text_size(gpui::px(12.0)),
                ),
        )
}

pub fn to_herogpui(vars: &ThemeVars, dark: bool) -> herogpui::theme::Theme {
    // Build from the matching base so the derived hover/soft mix weights and
    // the un-set roles (success, warning, link, backdrop, shadows) keep their
    // HeroUI defaults; only the inputs the app resolves explicitly change.
    let base = if dark {
        herogpui::theme::Theme::dark()
    } else {
        herogpui::theme::Theme::light()
    };
    let mut theme = herogpui::theme::Theme::builder("poratake", base)
        .radius(gpui::px(2.0))
        .field_radius(gpui::px(6.0))
        // `base.css:25` sets `--cursor-interactive: default`, and `:126` forces
        // `cursor: default` on `a`, `button`, `[role='button']` and the input
        // button types, so every control in this app shows an arrow where stock
        // HeroUI shows a hand. The default is `PointingHand`, so this is a
        // deliberate divergence and the bridge test pins both sides of it.
        .cursor_interactive(gpui::CursorStyle::Arrow)
        // HeroUI's stock `--tooltip-delay` is 1500ms, but the renderer opens
        // tooltips at 150: `renderer/components/ui/tooltip.tsx` wraps every
        // tooltip in a `TooltipProvider` whose `delayDuration` defaults to 150
        // and forwards it as `delay`. The close delay already matches HeroUI's
        // `--tooltip-close-delay: 500ms`, so it is left alone.
        .tooltip_delay_ms(150)
        .background(vars.background)
        .foreground(vars.foreground)
        .muted(vars.muted)
        .border(vars.border)
        .separator(vars.separator)
        .surface(vars.surface, vars.foreground)
        .surface_levels(vars.surface_secondary, vars.surface_tertiary)
        .overlay(vars.overlay, vars.foreground)
        .segment(vars.segment, vars.foreground)
        // `role` for "default" also points fields at the role colour, so the
        // explicit `field` tokens below win.
        .role("default", vars.default, vars.default_foreground)
        .role("accent", vars.accent, vars.accent_foreground)
        .role("danger", vars.danger, vars.danger_foreground)
        .field(vars.field_background, vars.field_foreground)
        .field_placeholder(vars.field_placeholder)
        .field_border(vars.field_border)
        .components(component_themes())
        .build();

    // Tokens with no builder method: `foreground` derives scrollbar at 15%
    // alpha, but the app resolves it directly, and the app's ring (accent at
    // 52%) is what its own focus-ring helper paints.
    theme.colors.scrollbar = vars.scrollbar;
    theme.colors.focus = vars.ring;
    theme.appearance = if dark {
        herogpui::theme::Appearance::Dark
    } else {
        herogpui::theme::Appearance::Light
    };

    theme
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::presets::{get_theme_preset, APP_THEME_PRESETS, DEFAULT_THEME_ID};
    use crate::theme::vars::ThemeVars;
    use gpui::px;
    use herogpui::gpui;

    /// The whole point of the bridge: HeroGPUI resolves its radii from
    /// `--radius`, so the app's `base.css` override has to arrive with it. If
    /// this drifts, every HeroGPUI corner in the shell silently becomes a
    /// quarter of a stock-Tailwind scale too large — the exact defect the first
    /// pass of this audit found in the hand-rolled components.
    #[test]
    fn the_app_radius_override_reaches_herogpui() {
        let vars = ThemeVars::for_preset(
            get_theme_preset(DEFAULT_THEME_ID),
            crate::theme::presets::ThemeMode::Dark,
        );
        let theme = to_herogpui(&vars, true);
        let layout = &theme.layout;

        assert_eq!(layout.radius, px(2.0), "base.css: --radius: 0.125rem");
        assert_eq!(
            layout.field_radius,
            px(6.0),
            "base.css: --field-radius: calc(var(--radius) * 3)"
        );
        // The audited scale that `--radius: 2px` produces.
        assert_eq!(layout.radius_sm(), px(1.0));
        assert_eq!(layout.radius_md(), px(1.5));
        assert_eq!(layout.radius_lg(), px(2.0));
        assert_eq!(layout.radius_xl(), px(3.0));
        assert_eq!(layout.radius_2xl(), px(4.0));
        assert_eq!(layout.radius_3xl(), px(6.0));
        assert_eq!(layout.radius_4xl(), px(8.0));
    }

    /// Every colour the app resolves explicitly must reach HeroGPUI unchanged,
    /// or a swap would repaint the surface with HeroUI's stock palette.
    #[test]
    fn the_app_tokens_reach_herogpui_for_every_preset() {
        for preset in APP_THEME_PRESETS {
            for (variant, dark) in [(&preset.dark, true), (&preset.light, false)] {
                let vars = ThemeVars::resolve(
                    variant,
                    if dark {
                        crate::theme::presets::ThemeMode::Dark
                    } else {
                        crate::theme::presets::ThemeMode::Light
                    },
                );
                let theme = to_herogpui(&vars, dark);
                let colors = &theme.colors;
                let id = preset.id;

                assert_eq!(colors.background, vars.background, "{id} background");
                assert_eq!(colors.foreground, vars.foreground, "{id} foreground");
                assert_eq!(colors.muted, vars.muted, "{id} muted");
                assert_eq!(colors.border, vars.border, "{id} border");
                assert_eq!(colors.separator, vars.separator, "{id} separator");
                assert_eq!(colors.surface.background, vars.surface, "{id} surface");
                assert_eq!(
                    colors.surface_secondary, vars.surface_secondary,
                    "{id} surface-secondary"
                );
                assert_eq!(
                    colors.surface_tertiary, vars.surface_tertiary,
                    "{id} surface-tertiary"
                );
                assert_eq!(colors.overlay.background, vars.overlay, "{id} overlay");
                assert_eq!(colors.segment.background, vars.segment, "{id} segment");
                assert_eq!(colors.default.color, vars.default, "{id} default");
                assert_eq!(
                    colors.default.foreground, vars.default_foreground,
                    "{id} default-foreground"
                );
                assert_eq!(colors.accent.color, vars.accent, "{id} accent");
                assert_eq!(
                    colors.accent.foreground, vars.accent_foreground,
                    "{id} accent-foreground"
                );
                assert_eq!(colors.danger.color, vars.danger, "{id} danger");
                assert_eq!(
                    colors.field.background, vars.field_background,
                    "{id} field-background"
                );
                assert_eq!(
                    colors.field.foreground, vars.field_foreground,
                    "{id} field-foreground"
                );
                assert_eq!(
                    colors.field.placeholder, vars.field_placeholder,
                    "{id} field-placeholder"
                );
                assert_eq!(colors.field.border, vars.field_border, "{id} field-border");
                assert_eq!(colors.scrollbar, vars.scrollbar, "{id} scrollbar");
                assert_eq!(colors.focus, vars.ring, "{id} focus ring");
                assert_eq!(theme.is_dark(), dark, "{id} appearance");
            }
        }
    }

    /// A theme published by the bridge must be buildable without a `TestApp` —
    /// `init_theme` runs on the real startup path, so a panic here is a launch
    /// failure rather than a rendering one.
    #[test]
    fn the_bridge_does_not_panic_on_an_unknown_preset() {
        // `get_theme_preset` falls back to the default for an unknown id.
        let vars = ThemeVars::for_preset(
            get_theme_preset("does-not-exist"),
            crate::theme::presets::ThemeMode::Light,
        );
        let theme = to_herogpui(&vars, false);
        assert_eq!(theme.id.as_ref(), "poratake");
    }

    /// `base.css` shows an arrow where HeroUI shows a hand. Both sides are
    /// asserted: the app's override, and the stock default it diverges from —
    /// without the second the first would pass even if HeroGPUI changed its
    /// default to `Arrow`, and the divergence would have silently disappeared.
    #[test]
    fn the_app_cursor_override_reaches_herogpui() {
        let vars = ThemeVars::for_preset(
            get_theme_preset(DEFAULT_THEME_ID),
            crate::theme::presets::ThemeMode::Dark,
        );
        let theme = to_herogpui(&vars, true);

        assert_eq!(theme.layout.cursor_interactive, gpui::CursorStyle::Arrow);
        assert_eq!(
            herogpui::theme::LayoutTheme::light().cursor_interactive,
            gpui::CursorStyle::PointingHand,
            "stock HeroGPUI still points; the app is the one opting out"
        );
    }

    /// The renderer opens tooltips at 150ms; HeroUI's own default is 1500. This
    /// is a token rather than a per-tooltip argument so every tooltip in the
    /// shell moves together.
    #[test]
    fn tooltips_open_on_the_renderers_delay() {
        let vars = ThemeVars::for_preset(
            get_theme_preset(DEFAULT_THEME_ID),
            crate::theme::presets::ThemeMode::Dark,
        );
        let theme = to_herogpui(&vars, true);

        assert_eq!(theme.layout.tooltip_delay_ms, 150);
        assert_eq!(
            herogpui::theme::LayoutTheme::light().tooltip_delay_ms,
            1500,
            "the renderer's 150ms is an override of HeroUI's default, not the default"
        );
        assert_eq!(theme.layout.tooltip_close_delay_ms, 500);
    }

    #[test]
    fn component_defaults_live_on_the_theme() {
        let vars = ThemeVars::for_preset(
            get_theme_preset(DEFAULT_THEME_ID),
            crate::theme::presets::ThemeMode::Dark,
        );
        let theme = to_herogpui(&vars, true);
        let slider = theme.components.slider.resolve(&[]);
        assert_eq!(slider.radius, Some(gpui::px(9999.0)));
        let select = theme.components.select.resolve(&[]);
        assert_eq!(select.variant, Some(FieldVariant::Secondary));
        let compact = theme.components.select.resolve(&["compact".into()]);
        assert_eq!(compact.height, Some(gpui::px(28.0)));
        assert_eq!(compact.variant, Some(FieldVariant::Secondary));
        let menu = theme.components.menu.resolve(&[]);
        assert_eq!(menu.panel_gap, Some(gpui::px(0.0)));
        assert_eq!(menu.row_height, Some(gpui::px(36.0)));
        let compact_menu = theme.components.menu.resolve(&["compact".into()]);
        assert_eq!(compact_menu.row_height, Some(gpui::px(28.0)));
        assert_eq!(compact_menu.panel_gap, Some(gpui::px(0.0)));
        let accent = theme.components.menu.resolve(&["accent".into()]);
        assert_eq!(
            accent.row_hover_bg,
            Some(ComponentColor::Role(Color::Accent))
        );
        let button = theme.components.button.resolve(&["compact".into()]);
        assert!(button.style.is_some());
        let missing = theme.components.button.resolve(&["does-not-exist".into()]);
        assert!(missing.style.is_none());
        let search = theme.components.text_field.resolve(&["search".into()]);
        assert_eq!(search.height, Some(gpui::px(32.0)));
        assert_eq!(search.padding_x, Some(gpui::px(10.0)));
        let compact_field = theme.components.text_field.resolve(&["compact".into()]);
        assert_eq!(compact_field.height, Some(gpui::px(28.0)));
        let overlay = theme.components.button.resolve(&["overlay".into()]);
        assert_eq!(
            overlay.radius,
            Some(gpui::px(crate::ui::chrome::OVERLAY_BUTTON_RADIUS))
        );
    }
}
