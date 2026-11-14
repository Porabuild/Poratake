//! Embedded assets: the SVGs HeroGPUI's built-in component chrome draws
//! (every path in [`icons`](crate::icons)) and the Lucide set named by
//! [`IconName`].

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

use crate::IconName;

/// Asset source for the SVG icons used by HeroGPUI components, and for every
/// [`IconName`].
///
/// Register it with [`gpui::Application::with_assets`] before opening a window.
pub struct HeroGpuiAssets;

impl HeroGpuiAssets {
    /// Combines HeroGPUI's icons with an application's own asset source.
    pub fn with_fallback<A: AssetSource>(fallback: A) -> HeroGpuiAssetSource<A> {
        HeroGpuiAssetSource { fallback }
    }
}

/// HeroGPUI's embedded icons followed by an application's asset source.
pub struct HeroGpuiAssetSource<A> {
    fallback: A,
}

macro_rules! svg_assets {
    ($($name:literal => $svg:literal),* $(,)?) => {
        const EMBEDDED: &[(&str, &[u8])] = &[
            $(($name, $svg.as_bytes())),*
        ];

        impl AssetSource for HeroGpuiAssets {
            fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
                Ok(EMBEDDED
                    .iter()
                    .find(|(asset_path, _)| *asset_path == path)
                    .map(|(_, data)| *data)
                    .or_else(|| IconName::from_path(path).map(IconName::svg))
                    .map(Cow::Borrowed)
                    .or_else(|| crate::icon::stroked_svg(path).map(Cow::Owned)))
            }

            fn list(&self, path: &str) -> Result<Vec<SharedString>> {
                Ok(EMBEDDED
                    .iter()
                    .map(|(asset_path, _)| *asset_path)
                    .chain(IconName::ALL.iter().map(|icon| icon.path()))
                    .filter(|asset_path| asset_path.starts_with(path))
                    .map(SharedString::new_static)
                    .collect())
            }
        }

        impl<A: AssetSource> AssetSource for HeroGpuiAssetSource<A> {
            fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
                match HeroGpuiAssets.load(path)? {
                    Some(data) => Ok(Some(data)),
                    None => self.fallback.load(path),
                }
            }

            fn list(&self, path: &str) -> Result<Vec<SharedString>> {
                let mut assets = HeroGpuiAssets.list(path)?;
                for asset in self.fallback.list(path)? {
                    if !assets.contains(&asset) {
                        assets.push(asset);
                    }
                }
                Ok(assets)
            }
        }
    };
}

svg_assets! {
    "herogpui/icons/alert_triangle.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0Z"/><path d="M12 9v4"/><path d="M12 17h.01"/></svg>"#,
    "herogpui/icons/arrow_left.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="19" y1="12" x2="5" y2="12"/><polyline points="12 19 5 12 12 5"/></svg>"#,
    "herogpui/icons/arrow_right.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="5" y1="12" x2="19" y2="12"/><polyline points="12 5 19 12 12 19"/></svg>"#,
    "herogpui/icons/calendar.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 13 14" fill="currentColor"><path fill-rule="evenodd" clip-rule="evenodd" d="M3.75 4.5A.75.75 0 0 1 3 3.75v-.748a1.5 1.5 0 0 0-1.5 1.5v1h10v-1a1.5 1.5 0 0 0-1.5-1.5v.75a.75.75 0 1 1-1.5 0v-.75h-4v.747a.75.75 0 0 1-.75.75ZM8.5 1.501h-4V.75a.75.75 0 0 0-1.5 0v.752a3 3 0 0 0-3 3v6a3 3 0 0 0 3 3h7a3 3 0 0 0 3-3v-6a3 3 0 0 0-3-3v-.75a.75.75 0 0 0-1.5 0v.75Zm-7 5.5v3.5a1.5 1.5 0 0 0 1.5 1.5h7a1.5 1.5 0 0 0 1.5-1.5v-3.5h-10Z"/></svg>"#,
    "herogpui/icons/check.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>"#,
    "herogpui/icons/check_circle.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor"><path fill-rule="evenodd" d="M13.5 8a5.5 5.5 0 1 1-11 0a5.5 5.5 0 0 1 11 0M15 8A7 7 0 1 1 1 8a7 7 0 0 1 14 0m-3.9-1.55a.75.75 0 1 0-1.2-.9L7.419 8.858L6.03 7.47a.75.75 0 0 0-1.06 1.06l2 2a.75.75 0 0 0 1.13-.08z" clip-rule="evenodd"/></svg>"#,
    "herogpui/icons/chevron_down.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>"#,
    "herogpui/icons/chevron_left.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>"#,
    "herogpui/icons/chevron_right.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"/></svg>"#,
    "herogpui/icons/chevron_up.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="18 15 12 9 6 15"/></svg>"#,
    "herogpui/icons/circle_exclamation.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor"><path fill-rule="evenodd" d="M8 13.5a5.5 5.5 0 1 0 0-11a5.5 5.5 0 0 0 0 11M8 15A7 7 0 1 0 8 1a7 7 0 0 0 0 14m1-4.5a1 1 0 1 1-2 0a1 1 0 0 1 2 0M8.75 5a.75.75 0 0 0-1.5 0v2.5a.75.75 0 0 0 1.5 0z" clip-rule="evenodd"/></svg>"#,
    "herogpui/icons/close.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>"#,
    "herogpui/icons/close_circle.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M9 9l6 6"/><path d="M15 9l-6 6"/></svg>"#,
    "herogpui/icons/copy.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>"#,
    "herogpui/icons/dots_vertical.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><circle cx="12" cy="5" r="1.8"/><circle cx="12" cy="12" r="1.8"/><circle cx="12" cy="19" r="1.8"/></svg>"#,
    "herogpui/icons/ellipsis.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><circle cx="12" cy="12" r="10" fill="none" stroke="currentColor" stroke-width="2"/><line x1="12" y1="10.5" x2="12" y2="16.5" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"/><circle cx="12" cy="7.2" r="1.35"/></svg>"#,
    "herogpui/icons/external_link.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>"#,
    "herogpui/icons/eye.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>"#,
    "herogpui/icons/eye_off.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/><line x1="1" y1="1" x2="23" y2="23"/></svg>"#,
    "herogpui/icons/gear.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path fill-rule="evenodd" clip-rule="evenodd" d="M9.94 5.10 L10.36 2.14 L13.64 2.14 L14.06 5.10 L15.42 5.66 L17.81 3.86 L20.14 6.19 L18.34 8.58 L18.90 9.94 L21.86 10.36 L21.86 13.64 L18.90 14.06 L18.34 15.42 L20.14 17.81 L17.81 20.14 L15.42 18.34 L14.06 18.90 L13.64 21.86 L10.36 21.86 L9.94 18.90 L8.58 18.34 L6.19 20.14 L3.86 17.81 L5.66 15.42 L5.10 14.06 L2.14 13.64 L2.14 10.36 L5.10 9.94 L5.66 8.58 L3.86 6.19 L6.19 3.86 L8.58 5.66 Z M15.20 12.00 A3.20 3.20 0 1 0 8.80 12.00 A3.20 3.20 0 1 0 15.20 12.00 Z"/></svg>"#,
    "herogpui/icons/globe.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3c2.6 2.4 4 5.6 4 9s-1.4 6.6-4 9c-2.6-2.4-4-5.6-4-9s1.4-6.6 4-9z"/></svg>"#,
    "herogpui/icons/heart.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20s-7-4.4-7-9.4A4.6 4.6 0 0 1 12 7.6 4.6 4.6 0 0 1 19 10.6c0 5-7 9.4-7 9.4z"/></svg>"#,
    "herogpui/icons/heart_fill.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M12 20s-7-4.4-7-9.4A4.6 4.6 0 0 1 12 7.6 4.6 4.6 0 0 1 19 10.6c0 5-7 9.4-7 9.4z"/></svg>"#,
    "herogpui/icons/info_circle.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor"><path fill-rule="evenodd" d="M8 13.5a5.5 5.5 0 1 0 0-11 5.5 5.5 0 0 0 0 11M8 15A7 7 0 1 0 8 1a7 7 0 0 0 0 14m1-9.5a1 1 0 1 1-2 0 1 1 0 0 1 2 0m-.25 3a.75.75 0 0 0-1.5 0V11a.75.75 0 0 0 1.5 0z" clip-rule="evenodd"/></svg>"#,
    "herogpui/icons/key.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="8" cy="12" r="4"/><path d="M12 12h9"/><path d="M17 12v3.5"/><path d="M20 12v2.5"/></svg>"#,
    "herogpui/icons/loader_2.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>"#,
    "herogpui/icons/mail.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="5" width="18" height="14" rx="3"/><path d="M4 7.5l7.4 5.2a1 1 0 0 0 1.2 0L20 7.5"/></svg>"#,
    "herogpui/icons/minus.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="5" y1="12" x2="19" y2="12"/></svg>"#,
    "herogpui/icons/moon.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>"#,
    "herogpui/icons/plus.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>"#,
    "herogpui/icons/search.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>"#,
    "herogpui/icons/spinner.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 50 50" fill="none"><circle cx="25" cy="25" r="20" stroke="currentColor" stroke-opacity="0.25" stroke-width="5"/><path d="M45 25 A20 20 0 0 0 25 5" stroke="currentColor" stroke-width="5" stroke-linecap="round"/></svg>"#,
    "herogpui/icons/sun.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="4" fill="currentColor"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>"#,
    "herogpui/icons/tooltip_arrow.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" fill="none"><path fill="currentColor" d="M0 0C5.48483 8 6.5 8 12 0Z"/></svg>"#,
    "herogpui/icons/trash.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16"/><path d="M10 4h4a1 1 0 0 1 1 1v2H9V5a1 1 0 0 1 1-1z"/><path d="M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12"/><path d="M10 11v6M14 11v6"/></svg>"#,
    "herogpui/icons/warning_triangle.svg" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor"><path fill-rule="evenodd" d="M7.134 2.994L2.217 11.5a1 1 0 0 0 .866 1.5h9.834a1 1 0 0 0 .866-1.5L8.866 2.993a1 1 0 0 0-1.732 0m3.03-.75c-.962-1.665-3.366-1.665-4.329 0L.918 10.749c-.963 1.666.24 3.751 2.165 3.751h9.834c1.925 0 3.128-2.085 2.164-3.751zM8 5a.75.75 0 0 1 .75.75v2a.75.75 0 0 1-1.5 0v-2A.75.75 0 0 1 8 5m1 5.75a1 1 0 1 1-2 0a1 1 0 0 1 2 0" clip-rule="evenodd"/></svg>"#,
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use gpui::AssetSource as _;

    use super::{HeroGpuiAssets, EMBEDDED};
    use crate::IconName;

    struct AppAssets;

    impl gpui::AssetSource for AppAssets {
        fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
            Ok((path == "app/logo.svg").then(|| Cow::Borrowed(&b"logo"[..])))
        }

        fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
            Ok((path == "app")
                .then(|| "app/logo.svg".into())
                .into_iter()
                .collect())
        }
    }

    #[test]
    fn embedded_assets_load_and_list() {
        let assets = HeroGpuiAssets;

        assert_eq!(
            assets.list("herogpui/icons").unwrap().len(),
            EMBEDDED.len() + IconName::ALL.len()
        );
        assert_eq!(
            assets.list(crate::LUCIDE_ICON_PREFIX).unwrap().len(),
            IconName::ALL.len()
        );
        for (path, expected) in EMBEDDED {
            assert_eq!(assets.load(path).unwrap().as_deref(), Some(*expected));
        }
        for icon in IconName::ALL {
            assert_eq!(
                assets.load(icon.path()).unwrap().as_deref(),
                Some(icon.svg())
            );
        }
        assert!(assets.load("herogpui/icons/missing.svg").unwrap().is_none());
        assert!(assets
            .load("herogpui/icons/lucide/missing.svg")
            .unwrap()
            .is_none());
    }

    /// Every chrome path the components draw must be served here too, not
    /// only by the gallery's own asset source: a downstream app registers
    /// `HeroGpuiAssets` alone.
    #[test]
    fn every_chrome_icon_constant_is_embedded() {
        let source = include_str!("icons.rs");
        let paths: Vec<&str> = source
            .lines()
            .filter_map(|line| line.strip_prefix("pub const "))
            .filter_map(|rest| rest.split('"').nth(1))
            .filter(|path| path.ends_with(".svg"))
            .collect();
        assert!(paths.len() > 30, "read only {} icon constants", paths.len());
        for path in paths {
            assert!(
                HeroGpuiAssets.load(path).unwrap().is_some(),
                "`icons.rs` names {path}, which `HeroGpuiAssets` does not embed"
            );
        }
    }

    #[test]
    fn fallback_preserves_application_assets() {
        let assets = HeroGpuiAssets::with_fallback(AppAssets);

        assert!(assets.load("herogpui/icons/check.svg").unwrap().is_some());
        assert_eq!(
            assets.load("app/logo.svg").unwrap().as_deref(),
            Some(&b"logo"[..])
        );
        assert_eq!(
            assets.list("app").unwrap(),
            vec![gpui::SharedString::from("app/logo.svg")]
        );
    }
}
