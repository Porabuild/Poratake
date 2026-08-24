//! 1:1 port of `src/types/theme.ts`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn parse(value: &str) -> Self {
        match value {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }
}

#[cfg(windows)]
pub fn system_theme_mode() -> ThemeMode {
    if read_apps_use_light_theme() == Some(true) {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    }
}

/// `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`
/// is a `REG_DWORD`: `1` for the light OS theme, `0` for dark. Chromium reads
/// the same value for `nativeTheme`, so this follows Electron exactly.
#[cfg(windows)]
fn read_apps_use_light_theme() -> Option<bool> {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_DWORD,
    };

    let mut key = HKEY::default();
    // SAFETY: every out-parameter is initialised here and the key is closed on
    // both paths below.
    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            None,
            KEY_READ,
            &mut key,
        )
        .is_err()
        {
            return None;
        }

        let mut value: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let mut kind = REG_DWORD;
        let result = RegQueryValueExW(
            key,
            w!("AppsUseLightTheme"),
            None,
            Some(&mut kind),
            Some(&mut value as *mut u32 as *mut u8),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if result.is_err() || kind != REG_DWORD {
            return None;
        }

        Some(value != 0)
    }
}

#[cfg(not(windows))]
pub fn system_theme_mode() -> ThemeMode {
    ThemeMode::Dark
}

pub fn resolve_theme_mode(mode: ThemeMode) -> ThemeMode {
    match mode {
        ThemeMode::System => system_theme_mode(),
        _ => mode,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ThemeVariant {
    pub bg: &'static str,
    pub surface: &'static str,
    pub fg: &'static str,
    pub accent: &'static str,
    pub accent_fg: &'static str,
    pub border: &'static str,
    pub sidebar: &'static str,
    pub content: Option<&'static str>,
}

#[derive(Clone, Copy, Debug)]
pub struct AppThemePreset {
    pub id: &'static str,
    pub label: &'static str,
    pub light: ThemeVariant,
    pub dark: ThemeVariant,
}

pub const DEFAULT_THEME_ID: &str = "default";

pub const APP_THEME_PRESETS: &[AppThemePreset] = &[
    AppThemePreset {
        id: "default",
        label: "Poracode",
        light: ThemeVariant {
            bg: "#f1f1f4",
            surface: "#fafafb",
            fg: "#18181b",
            accent: "#5f6cd9",
            accent_fg: "#ffffff",
            border: "#cacace",
            sidebar: "#ececef",
            content: Some("#f6f6f9"),
        },
        dark: ThemeVariant {
            bg: "#070709",
            surface: "#0e0e14",
            fg: "#fafafa",
            accent: "#8892ef",
            accent_fg: "#0a0a12",
            border: "#24242e",
            sidebar: "#0e0e14",
            content: Some("#0b0b11"),
        },
    },
    AppThemePreset {
        id: "poracode-legacy",
        label: "Poracode Legacy",
        light: ThemeVariant {
            bg: "#f1f1f4",
            surface: "#fafafb",
            fg: "#18181b",
            accent: "#478cc4",
            accent_fg: "#000000",
            border: "#cacace",
            sidebar: "#ececef",
            content: Some("#f6f6f9"),
        },
        dark: ThemeVariant {
            bg: "#141416",
            surface: "#1a1a1c",
            fg: "#fcfcfc",
            accent: "#88bae4",
            accent_fg: "#111113",
            border: "#303033",
            sidebar: "#1a1a1c",
            content: Some("#161618"),
        },
    },
    AppThemePreset {
        id: "catppuccin",
        label: "Catppuccin",
        light: ThemeVariant {
            bg: "#eff1f5",
            surface: "#ffffff",
            fg: "#3d3f54",
            accent: "#8839ef",
            accent_fg: "#ffffff",
            border: "#bcc0cc",
            sidebar: "#e6e9ef",
            content: None,
        },
        dark: ThemeVariant {
            bg: "#1e1e2e",
            surface: "#27273a",
            fg: "#d2daf5",
            accent: "#cba6f7",
            accent_fg: "#1e1e2e",
            border: "#313244",
            sidebar: "#181825",
            content: None,
        },
    },
    AppThemePreset {
        id: "github",
        label: "GitHub",
        light: ThemeVariant {
            bg: "#ffffff",
            surface: "#f6f8fa",
            fg: "#1f2328",
            accent: "#0969da",
            accent_fg: "#ffffff",
            border: "#d0d7de",
            sidebar: "#f6f8fa",
            content: Some("#ffffff"),
        },
        dark: ThemeVariant {
            bg: "#0d1117",
            surface: "#161b22",
            fg: "#e6edf3",
            accent: "#2f81f7",
            accent_fg: "#000000",
            border: "#30363d",
            sidebar: "#0d1117",
            content: None,
        },
    },
    AppThemePreset {
        id: "one",
        label: "One",
        light: ThemeVariant {
            bg: "#fafafa",
            surface: "#ffffff",
            fg: "#383a42",
            accent: "#4078f2",
            accent_fg: "#000000",
            border: "#e5e5e6",
            sidebar: "#eaeaeb",
            content: None,
        },
        dark: ThemeVariant {
            bg: "#282c34",
            surface: "#2c313a",
            fg: "#dee0e6",
            accent: "#61afef",
            accent_fg: "#282c34",
            border: "#3b4048",
            sidebar: "#21252b",
            content: None,
        },
    },
    AppThemePreset {
        id: "dracula",
        label: "Dracula",
        light: ThemeVariant {
            bg: "#fffbeb",
            surface: "#ffffff",
            fg: "#1f1f1f",
            accent: "#644ac9",
            accent_fg: "#ffffff",
            border: "#d4cfc0",
            sidebar: "#f3eedd",
            content: None,
        },
        dark: ThemeVariant {
            bg: "#282a36",
            surface: "#343746",
            fg: "#f8f8f2",
            accent: "#bd93f9",
            accent_fg: "#282a36",
            border: "#44475a",
            sidebar: "#21222c",
            content: None,
        },
    },
    AppThemePreset {
        id: "nord",
        label: "Nord",
        light: ThemeVariant {
            bg: "#eceff4",
            surface: "#ffffff",
            fg: "#2e3440",
            accent: "#5e81ac",
            accent_fg: "#000000",
            border: "#d8dee9",
            sidebar: "#e5e9f0",
            content: None,
        },
        dark: ThemeVariant {
            bg: "#2e3440",
            surface: "#3b4252",
            fg: "#eff2f6",
            accent: "#88c0d0",
            accent_fg: "#2e3440",
            border: "#434c5e",
            sidebar: "#2b303b",
            content: None,
        },
    },
    AppThemePreset {
        id: "tokyo-night",
        label: "Tokyo Night",
        light: ThemeVariant {
            bg: "#e1e2e7",
            surface: "#ffffff",
            fg: "#303651",
            accent: "#2e7de9",
            accent_fg: "#000000",
            border: "#c4c8da",
            sidebar: "#d6d8df",
            content: None,
        },
        dark: ThemeVariant {
            bg: "#1a1b26",
            surface: "#1f2335",
            fg: "#cdd5f7",
            accent: "#7aa2f7",
            accent_fg: "#1a1b26",
            border: "#292e42",
            sidebar: "#16161e",
            content: None,
        },
    },
    AppThemePreset {
        id: "gruvbox",
        label: "Gruvbox",
        light: ThemeVariant {
            bg: "#fbf1c7",
            surface: "#f9f5d7",
            fg: "#3c3836",
            accent: "#d65d0e",
            accent_fg: "#000000",
            border: "#d5c4a1",
            sidebar: "#ebdbb2",
            content: None,
        },
        dark: ThemeVariant {
            bg: "#282828",
            surface: "#32302f",
            fg: "#f0e5c7",
            accent: "#fe8019",
            accent_fg: "#282828",
            border: "#504945",
            sidebar: "#1d2021",
            content: None,
        },
    },
    AppThemePreset {
        id: "solarized",
        label: "Solarized",
        light: ThemeVariant {
            bg: "#fdf6e3",
            surface: "#eee8d5",
            fg: "#2e3c41",
            accent: "#268bd2",
            accent_fg: "#000000",
            border: "#ddd6c1",
            sidebar: "#eee8d5",
            content: None,
        },
        dark: ThemeVariant {
            bg: "#002b36",
            surface: "#073642",
            fg: "#e3e8e8",
            accent: "#268bd2",
            accent_fg: "#000000",
            border: "#0a4a5a",
            sidebar: "#002028",
            content: None,
        },
    },
    AppThemePreset {
        id: "rose-pine",
        label: "Rosé Pine",
        light: ThemeVariant {
            bg: "#faf4ed",
            surface: "#fffaf3",
            fg: "#423e5c",
            accent: "#907aa9",
            accent_fg: "#000000",
            border: "#dfdad9",
            sidebar: "#f2e9e1",
            content: None,
        },
        dark: ThemeVariant {
            bg: "#232136",
            surface: "#2a273f",
            fg: "#e0def4",
            accent: "#c4a7e7",
            accent_fg: "#232136",
            border: "#44415a",
            sidebar: "#1f1d2e",
            content: None,
        },
    },
    AppThemePreset {
        id: "everforest",
        label: "Everforest",
        light: ThemeVariant {
            bg: "#fdf6e3",
            surface: "#f4f0d9",
            fg: "#374147",
            accent: "#677700",
            accent_fg: "#ffffff",
            border: "#e0dcc7",
            sidebar: "#efebd4",
            content: None,
        },
        dark: ThemeVariant {
            bg: "#2d353b",
            surface: "#343f44",
            fg: "#eee8dd",
            accent: "#a7c080",
            accent_fg: "#2d353b",
            border: "#475258",
            sidebar: "#272e33",
            content: None,
        },
    },
    AppThemePreset {
        id: "monokai",
        label: "Monokai",
        light: ThemeVariant {
            bg: "#fbfbf8",
            surface: "#ffffff",
            fg: "#2c2b29",
            accent: "#e0156d",
            accent_fg: "#ffffff",
            border: "#e4e3da",
            sidebar: "#f1f1ea",
            content: None,
        },
        dark: ThemeVariant {
            bg: "#272822",
            surface: "#2f302a",
            fg: "#f8f8f2",
            accent: "#f92672",
            accent_fg: "#000000",
            border: "#3e3d32",
            sidebar: "#1d1e19",
            content: None,
        },
    },
];

pub fn get_theme_preset(id: &str) -> &'static AppThemePreset {
    APP_THEME_PRESETS
        .iter()
        .find(|preset| preset.id == id)
        .or_else(|| {
            APP_THEME_PRESETS
                .iter()
                .find(|preset| preset.id == DEFAULT_THEME_ID)
        })
        .unwrap_or(&APP_THEME_PRESETS[0])
}

pub fn theme_options() -> Vec<(&'static str, &'static str)> {
    APP_THEME_PRESETS
        .iter()
        .map(|preset| (preset.id, preset.label))
        .collect()
}

/// The destructive token from base.css: oklch(0.65 0.24 26.65).
pub const DESTRUCTIVE: &str = "oklch(0.65 0.24 26.65)";

#[cfg(test)]
mod tests {
    use super::*;

    /// Whatever the machine is set to, the registry read has to land on one of
    /// the two real modes — that is the contract the theme resolver relies on.
    #[test]
    fn the_system_mode_is_always_light_or_dark() {
        assert!(matches!(
            system_theme_mode(),
            ThemeMode::Light | ThemeMode::Dark
        ));
    }
}
