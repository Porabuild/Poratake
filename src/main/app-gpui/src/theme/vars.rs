//! 1:1 port of `src/renderer/theme/app-theme.ts` — resolves the CSS custom
//! properties the whole design system is built on into concrete colors.

use gpui::{App, Hsla};

use crate::theme::color::{mix_oklab, mix_parsed, mix_parsed_weights, Srgba};
use crate::theme::presets::{
    get_theme_preset, AppThemePreset, ThemeMode, ThemeVariant, DEFAULT_THEME_ID, DESTRUCTIVE,
};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ThemeVars {
    pub background: Hsla,
    pub foreground: Hsla,
    pub surface: Hsla,
    pub surface_secondary: Hsla,
    pub surface_tertiary: Hsla,
    pub overlay: Hsla,
    pub muted: Hsla,
    pub scrollbar: Hsla,
    pub default: Hsla,
    pub accent: Hsla,
    pub accent_foreground: Hsla,
    pub accent_hover: Hsla,
    pub field_background: Hsla,
    pub field_foreground: Hsla,
    pub field_placeholder: Hsla,
    pub field_border: Hsla,
    pub field_hover: Hsla,
    pub segment: Hsla,
    pub border: Hsla,
    pub separator: Hsla,
    pub sidebar_background: Hsla,
    pub content_background: Hsla,
    pub row_hover: Hsla,
    pub row_active: Hsla,
    pub hairline: Hsla,
    pub card: Hsla,
    pub card_foreground: Hsla,
    pub popover: Hsla,
    pub popover_foreground: Hsla,
    pub primary: Hsla,
    pub primary_foreground: Hsla,
    pub secondary: Hsla,
    pub secondary_foreground: Hsla,
    pub muted_background: Hsla,
    pub muted_foreground: Hsla,
    pub input: Hsla,
    pub ring: Hsla,
    pub destructive: Hsla,
    pub destructive_foreground: Hsla,
    pub default_hover: Hsla,
    pub default_foreground: Hsla,
    pub danger: Hsla,
    pub danger_hover: Hsla,
    pub danger_foreground: Hsla,
}

impl Default for ThemeVars {
    fn default() -> Self {
        // base.css :root defaults (dark).
        Self::resolve(&get_theme_preset(DEFAULT_THEME_ID).dark, ThemeMode::Dark)
    }
}

fn m(color_a: &str, percentage: f32, color_b: &str) -> Hsla {
    mix_oklab(color_a, percentage, color_b).to_hsla()
}

fn mix_oklab_parsed_color_mix(
    base: crate::theme::color::Srgba,
    percentage: f32,
    target: crate::theme::color::Srgba,
) -> crate::theme::color::Srgba {
    mix_parsed(base, percentage, target)
}

impl ThemeVars {
    /// Port of `applyVariant` in app-theme.ts.
    pub fn resolve(variant: &ThemeVariant, mode: ThemeMode) -> Self {
        let dark = matches!(mode, ThemeMode::Dark);

        let content = match variant.content {
            Some(value) => Srgba::parse(value).to_hsla(),
            None => mix_oklab(variant.bg, 84.0, variant.surface).to_hsla(),
        };

        let field_border = if dark {
            m(variant.border, 84.0, variant.fg)
        } else {
            m(variant.border, 72.0, variant.surface)
        };
        let border_srgb = if dark {
            crate::theme::color::mix_oklab(variant.border, 90.0, variant.fg)
        } else {
            crate::theme::color::mix_oklab(variant.border, 72.0, variant.surface)
        };
        let border = border_srgb.to_hsla();
        let default_surface_srgb =
            mix_oklab(variant.surface, if dark { 91.0 } else { 86.0 }, variant.fg);
        let default_surface = default_surface_srgb.to_hsla();
        // Dark fields sit on the button surface so they read as button-like
        // rather than as black holes; light fields stay near the surface tone.
        let field_background_srgb = if dark {
            default_surface_srgb
        } else {
            mix_oklab(variant.surface, 97.0, variant.bg)
        };
        let field_background = field_background_srgb.to_hsla();
        // HeroUI `--field-hover: color-mix(in oklab, var(--field-background)
        // 90%, var(--field-foreground) 2%)`; `--field-foreground` is the
        // variant foreground in both modes.
        let field_hover =
            mix_parsed_weights(field_background_srgb, 90.0, Srgba::parse(variant.fg), 2.0)
                .to_hsla();
        let accent_hover_target = if variant.accent_fg.to_lowercase() == "#ffffff" {
            "#000000"
        } else {
            "#ffffff"
        };

        // HeroUI default-theme tokens (themes/default/variables.css): the
        // dark palette uses snow/eclipse neutrals, light uses the inverse.
        let (snow, eclipse) = (
            Srgba::parse("oklch(0.9911 0 0)"),
            Srgba::parse("oklch(0.2103 0.0059 285.89)"),
        );
        let (default_foreground_srgb, danger_srgb) = if dark {
            (snow, Srgba::parse("oklch(0.6532 0.2328 25.74)"))
        } else {
            (eclipse, Srgba::parse("oklch(0.594 0.1967 24.63)"))
        };
        let default_hover =
            mix_oklab_parsed_color_mix(default_surface_srgb, 96.0, default_foreground_srgb)
                .to_hsla();
        let danger_foreground = snow;
        let danger_hover =
            mix_oklab_parsed_color_mix(danger_srgb, 90.0, danger_foreground).to_hsla();

        let separator = mix_parsed(border_srgb, 85.0, Srgba::TRANSPARENT).to_hsla();

        Self {
            background: Srgba::parse(variant.bg).to_hsla(),
            foreground: Srgba::parse(variant.fg).to_hsla(),
            surface: Srgba::parse(variant.surface).to_hsla(),
            surface_secondary: m(variant.surface, 88.0, variant.bg),
            surface_tertiary: m(variant.surface, 74.0, variant.bg),
            overlay: m(variant.surface, 94.0, variant.bg),
            muted: m(variant.fg, 76.0, variant.bg),
            scrollbar: m(variant.fg, 24.0, variant.bg),
            default: default_surface,
            accent: Srgba::parse(variant.accent).to_hsla(),
            accent_foreground: Srgba::parse(variant.accent_fg).to_hsla(),
            accent_hover: m(variant.accent, 90.0, accent_hover_target),
            field_background,
            field_foreground: Srgba::parse(variant.fg).to_hsla(),
            field_placeholder: m(variant.fg, if dark { 82.0 } else { 73.0 }, variant.bg),
            field_border,
            field_hover,
            segment: m(variant.surface, 82.0, variant.bg),
            border,
            separator,
            sidebar_background: Srgba::parse(variant.sidebar).to_hsla(),
            content_background: content,
            row_hover: m(variant.fg, 6.0, "transparent"),
            row_active: m(variant.fg, 11.0, "transparent"),
            hairline: m(variant.fg, 9.0, "transparent"),
            card: Srgba::parse(variant.surface).to_hsla(),
            card_foreground: Srgba::parse(variant.fg).to_hsla(),
            popover: Srgba::parse(variant.surface).to_hsla(),
            popover_foreground: Srgba::parse(variant.fg).to_hsla(),
            // `useAccentColor` sets `--primary` to the operating system's
            // accent on every window, so `--primary` is *not* the theme accent
            // -- it is whatever colour the user picked in Windows. Only the
            // foreground stays with the theme, as Electron never overrides it.
            primary: Srgba::parse(&crate::system::accent::system_accent()).to_hsla(),
            primary_foreground: Srgba::parse(variant.accent_fg).to_hsla(),
            secondary: default_surface,
            secondary_foreground: Srgba::parse(variant.fg).to_hsla(),
            muted_background: m(variant.surface, 88.0, variant.bg),
            muted_foreground: m(variant.fg, 76.0, variant.bg),
            input: field_border,
            ring: m(variant.accent, 52.0, "transparent"),
            destructive: Srgba::parse(DESTRUCTIVE).to_hsla(),
            destructive_foreground: Srgba::WHITE.to_hsla(),
            default_hover,
            default_foreground: default_foreground_srgb.to_hsla(),
            danger: danger_srgb.to_hsla(),
            danger_hover,
            danger_foreground: danger_foreground.to_hsla(),
        }
    }

    pub fn for_preset(preset: &AppThemePreset, mode: ThemeMode) -> Self {
        let variant = match mode {
            ThemeMode::Light => &preset.light,
            _ => &preset.dark,
        };
        Self::resolve(variant, mode)
    }
}

pub struct ActiveTheme(ThemeVars);

impl gpui::Global for ActiveTheme {}

/// The resolved mode the current `ThemeVars` were built with — `System` is
/// never stored, only the light/dark it resolved to. The system-theme watcher
/// compares against it so a registry event that did not change the applied
/// mode repaints nothing.
struct ActiveMode(ThemeMode);

impl gpui::Global for ActiveMode {}

pub fn init_theme(cx: &mut App, appearance_mode: ThemeMode, theme_id: &str) {
    let preset = get_theme_preset(theme_id);
    let vars = ThemeVars::for_preset(preset, appearance_mode);
    cx.set_global(ActiveTheme(vars));
    cx.set_global(ActiveMode(appearance_mode));
}

pub fn update_theme(cx: &mut App, appearance_mode: ThemeMode, theme_id: &str) {
    init_theme(cx, appearance_mode, theme_id);
    cx.refresh_windows();
}

pub fn active_theme(cx: &App) -> ThemeVars {
    cx.try_global::<ActiveTheme>()
        .map(|theme| theme.0.clone())
        .unwrap_or_default()
}

pub fn active_mode(cx: &App) -> ThemeMode {
    cx.try_global::<ActiveMode>()
        .map(|mode| mode.0)
        .unwrap_or(ThemeMode::Dark)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::color::{mix_oklab, mix_parsed, Srgba};
    use crate::theme::presets::{
        get_theme_preset, ThemeMode, ThemeVariant, APP_THEME_PRESETS, DEFAULT_THEME_ID,
    };
    use gpui::Hsla;

    fn expected_content(variant: &ThemeVariant) -> Hsla {
        match variant.content {
            Some(value) => Srgba::parse(value).to_hsla(),
            None => mix_oklab(variant.bg, 84.0, variant.surface).to_hsla(),
        }
    }

    fn expected_mix(a: &str, amount: f32, b: &str) -> Hsla {
        mix_oklab(a, amount, b).to_hsla()
    }

    fn assert_apply_variant(preset_id: &str, variant: &ThemeVariant, mode: ThemeMode) {
        let vars = ThemeVars::resolve(variant, mode);
        let dark = matches!(mode, ThemeMode::Dark);
        let field_border = if dark {
            expected_mix(variant.border, 84.0, variant.fg)
        } else {
            expected_mix(variant.border, 72.0, variant.surface)
        };
        let border_srgb = if dark {
            mix_oklab(variant.border, 90.0, variant.fg)
        } else {
            mix_oklab(variant.border, 72.0, variant.surface)
        };
        let border = border_srgb.to_hsla();
        let default_surface = if dark {
            expected_mix(variant.surface, 91.0, variant.fg)
        } else {
            expected_mix(variant.surface, 86.0, variant.fg)
        };
        let field_background = if dark {
            default_surface
        } else {
            expected_mix(variant.surface, 97.0, variant.bg)
        };
        let accent_hover_target = if variant.accent_fg.to_lowercase() == "#ffffff" {
            "#000000"
        } else {
            "#ffffff"
        };
        let separator = mix_parsed(border_srgb, 85.0, Srgba::TRANSPARENT).to_hsla();
        let (snow, eclipse) = (
            Srgba::parse("oklch(0.9911 0 0)"),
            Srgba::parse("oklch(0.2103 0.0059 285.89)"),
        );
        let (default_foreground_srgb, danger_srgb) = if dark {
            (snow, Srgba::parse("oklch(0.6532 0.2328 25.74)"))
        } else {
            (eclipse, Srgba::parse("oklch(0.594 0.1967 24.63)"))
        };
        let default_surface_parsed =
            mix_oklab(variant.surface, if dark { 91.0 } else { 86.0 }, variant.fg);
        let default_hover =
            mix_parsed(default_surface_parsed, 96.0, default_foreground_srgb).to_hsla();
        let danger_hover = mix_parsed(danger_srgb, 90.0, snow).to_hsla();

        assert_eq!(
            vars.background,
            Srgba::parse(variant.bg).to_hsla(),
            "{preset_id} background"
        );
        assert_eq!(
            vars.foreground,
            Srgba::parse(variant.fg).to_hsla(),
            "{preset_id} foreground"
        );
        assert_eq!(
            vars.surface,
            Srgba::parse(variant.surface).to_hsla(),
            "{preset_id} surface"
        );
        assert_eq!(
            vars.surface_secondary,
            expected_mix(variant.surface, 88.0, variant.bg),
            "{preset_id} surface-secondary"
        );
        assert_eq!(
            vars.surface_tertiary,
            expected_mix(variant.surface, 74.0, variant.bg),
            "{preset_id} surface-tertiary"
        );
        assert_eq!(
            vars.overlay,
            expected_mix(variant.surface, 94.0, variant.bg),
            "{preset_id} overlay"
        );
        assert_eq!(
            vars.muted,
            expected_mix(variant.fg, 76.0, variant.bg),
            "{preset_id} muted"
        );
        assert_eq!(
            vars.scrollbar,
            expected_mix(variant.fg, 24.0, variant.bg),
            "{preset_id} scrollbar"
        );
        assert_eq!(vars.default, default_surface, "{preset_id} default");
        assert_eq!(
            vars.accent,
            Srgba::parse(variant.accent).to_hsla(),
            "{preset_id} accent"
        );
        assert_eq!(
            vars.accent_foreground,
            Srgba::parse(variant.accent_fg).to_hsla(),
            "{preset_id} accent-foreground"
        );
        assert_eq!(
            vars.accent_hover,
            expected_mix(variant.accent, 90.0, accent_hover_target),
            "{preset_id} accent-hover"
        );
        assert_eq!(
            vars.field_background, field_background,
            "{preset_id} field-background"
        );
        assert_eq!(
            vars.field_foreground,
            Srgba::parse(variant.fg).to_hsla(),
            "{preset_id} field-foreground"
        );
        assert_eq!(
            vars.field_placeholder,
            expected_mix(variant.fg, if dark { 82.0 } else { 73.0 }, variant.bg),
            "{preset_id} field-placeholder"
        );
        assert_eq!(vars.field_border, field_border, "{preset_id} field-border");
        assert_eq!(
            vars.segment,
            expected_mix(variant.surface, 82.0, variant.bg),
            "{preset_id} segment"
        );
        assert_eq!(vars.border, border, "{preset_id} border");
        assert_eq!(vars.separator, separator, "{preset_id} separator");
        assert_eq!(
            vars.sidebar_background,
            Srgba::parse(variant.sidebar).to_hsla(),
            "{preset_id} sidebar"
        );
        assert_eq!(
            vars.content_background,
            expected_content(variant),
            "{preset_id} content"
        );
        assert_eq!(
            vars.row_hover,
            expected_mix(variant.fg, 6.0, "transparent"),
            "{preset_id} row-hover"
        );
        assert_eq!(
            vars.row_active,
            expected_mix(variant.fg, 11.0, "transparent"),
            "{preset_id} row-active"
        );
        assert_eq!(
            vars.hairline,
            expected_mix(variant.fg, 9.0, "transparent"),
            "{preset_id} hairline"
        );
        assert_eq!(
            vars.card,
            Srgba::parse(variant.surface).to_hsla(),
            "{preset_id} card"
        );
        assert_eq!(
            vars.card_foreground,
            Srgba::parse(variant.fg).to_hsla(),
            "{preset_id} card-foreground"
        );
        assert_eq!(
            vars.popover,
            Srgba::parse(variant.surface).to_hsla(),
            "{preset_id} popover"
        );
        assert_eq!(
            vars.popover_foreground,
            Srgba::parse(variant.fg).to_hsla(),
            "{preset_id} popover-foreground"
        );
        // `--primary` deliberately does not follow the preset: `useAccentColor`
        // overwrites it with the operating system's accent, so it is the same
        // colour under every theme.
        assert_eq!(
            vars.primary,
            Srgba::parse(&crate::system::accent::system_accent()).to_hsla(),
            "{preset_id} primary follows the system accent, not the preset"
        );
        assert_eq!(
            vars.primary_foreground,
            Srgba::parse(variant.accent_fg).to_hsla(),
            "{preset_id} primary-foreground"
        );
        assert_eq!(vars.secondary, default_surface, "{preset_id} secondary");
        assert_eq!(
            vars.secondary_foreground,
            Srgba::parse(variant.fg).to_hsla(),
            "{preset_id} secondary-foreground"
        );
        assert_eq!(
            vars.muted_background,
            expected_mix(variant.surface, 88.0, variant.bg),
            "{preset_id} muted-background"
        );
        assert_eq!(
            vars.muted_foreground,
            expected_mix(variant.fg, 76.0, variant.bg),
            "{preset_id} muted-foreground"
        );
        assert_eq!(vars.input, field_border, "{preset_id} input");
        assert_eq!(
            vars.ring,
            expected_mix(variant.accent, 52.0, "transparent"),
            "{preset_id} ring"
        );
        assert_eq!(
            vars.default_hover, default_hover,
            "{preset_id} default-hover"
        );
        assert_eq!(
            vars.default_foreground,
            default_foreground_srgb.to_hsla(),
            "{preset_id} default-foreground"
        );
        assert_eq!(vars.danger, danger_srgb.to_hsla(), "{preset_id} danger");
        assert_eq!(vars.danger_hover, danger_hover, "{preset_id} danger-hover");
        assert_eq!(
            vars.danger_foreground,
            snow.to_hsla(),
            "{preset_id} danger-foreground"
        );
    }

    #[test]
    fn apply_variant_matches_electron_mix_for_every_preset() {
        for preset in APP_THEME_PRESETS {
            assert_apply_variant(preset.id, &preset.light, ThemeMode::Light);
            assert_apply_variant(preset.id, &preset.dark, ThemeMode::Dark);
            assert_eq!(
                ThemeVars::for_preset(preset, ThemeMode::Light).background,
                Srgba::parse(preset.light.bg).to_hsla()
            );
            assert_eq!(
                ThemeVars::for_preset(preset, ThemeMode::Dark).accent,
                Srgba::parse(preset.dark.accent).to_hsla()
            );
        }
        assert_eq!(APP_THEME_PRESETS.len(), 13);
        assert_eq!(APP_THEME_PRESETS[0].id, DEFAULT_THEME_ID);
        assert_eq!(get_theme_preset("missing").id, DEFAULT_THEME_ID);
    }

    #[test]
    fn separator_mixes_the_resolved_border_token() {
        let preset = get_theme_preset(DEFAULT_THEME_ID);
        let vars = ThemeVars::resolve(&preset.dark, ThemeMode::Dark);
        let border = mix_oklab(preset.dark.border, 90.0, preset.dark.fg);
        let expected = mix_parsed(border, 85.0, Srgba::TRANSPARENT).to_hsla();
        assert_eq!(vars.separator, expected);
        let raw = mix_parsed(Srgba::parse(preset.dark.border), 85.0, Srgba::TRANSPARENT).to_hsla();
        assert_ne!(vars.separator, raw);
    }

    #[test]
    fn theme_preset_literals_match_electron() {
        let default = get_theme_preset("default");
        assert_eq!(default.label, "Poracode");
        assert_eq!(default.light.bg, "#f1f1f4");
        assert_eq!(default.light.surface, "#fafafb");
        assert_eq!(default.light.fg, "#18181b");
        assert_eq!(default.light.accent, "#5f6cd9");
        assert_eq!(default.light.accent_fg, "#ffffff");
        assert_eq!(default.light.border, "#cacace");
        assert_eq!(default.light.sidebar, "#ececef");
        assert_eq!(default.light.content, Some("#f6f6f9"));
        assert_eq!(default.dark.bg, "#070709");
        assert_eq!(default.dark.surface, "#0e0e14");
        assert_eq!(default.dark.fg, "#fafafa");
        assert_eq!(default.dark.accent, "#8892ef");
        assert_eq!(default.dark.accent_fg, "#0a0a12");
        assert_eq!(default.dark.border, "#24242e");
        assert_eq!(default.dark.sidebar, "#0e0e14");
        assert_eq!(default.dark.content, Some("#0b0b11"));
        let ids: Vec<&str> = APP_THEME_PRESETS.iter().map(|preset| preset.id).collect();
        assert_eq!(
            ids,
            vec![
                "default",
                "poracode-legacy",
                "catppuccin",
                "github",
                "one",
                "dracula",
                "nord",
                "tokyo-night",
                "gruvbox",
                "solarized",
                "rose-pine",
                "everforest",
                "monokai",
            ]
        );
    }
}
