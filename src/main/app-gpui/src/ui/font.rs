use std::borrow::Cow;

use gpui::{div, prelude::*, App, Div};
use herogpui::gpui;

pub const UI_FONT: &str = "Geist";
pub const MONO_FONT: &str = "Geist Mono";

const FACES: [&[u8]; 6] = [
    include_bytes!("../../fonts/Geist-Regular.ttf"),
    include_bytes!("../../fonts/Geist-Medium.ttf"),
    include_bytes!("../../fonts/Geist-SemiBold.ttf"),
    include_bytes!("../../fonts/Geist-Bold.ttf"),
    include_bytes!("../../fonts/GeistMono-Regular.ttf"),
    include_bytes!("../../fonts/GeistMono-Medium.ttf"),
];

pub fn register(cx: &mut App) {
    let faces = FACES.iter().map(|face| Cow::Borrowed(*face)).collect();
    if let Err(error) = cx.text_system().add_fonts(faces) {
        eprintln!("[fonts] Geist could not be registered: {error}");
    }
}

pub fn root() -> Div {
    div().font_family(UI_FONT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Styled;

    fn face_names() -> Vec<String> {
        FACES
            .iter()
            .map(|face| {
                let font = fontdue::Font::from_bytes(*face, fontdue::FontSettings::default())
                    .expect("an embedded face must be a TTF gpui can register");
                font.name()
                    .expect("an embedded face must carry its full name")
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn the_shell_embeds_every_geist_weight_it_asks_for() {
        assert_eq!(
            face_names(),
            vec![
                "Geist Regular",
                "Geist Medium",
                "Geist SemiBold",
                "Geist Bold",
                "Geist Mono Regular",
                "Geist Mono Medium",
            ]
        );
    }

    #[test]
    fn the_shell_registers_its_faces_at_startup() {
        let startup = include_str!("../main.rs");
        assert!(
            startup.contains("ui::font::register(cx)"),
            "main.rs must register the embedded faces before any window opens"
        );
    }

    #[test]
    fn every_window_root_names_the_family() {
        let roots = [
            ("capture/overlay.rs", include_str!("../capture/overlay.rs")),
            ("editor/window.rs", include_str!("../editor/window.rs")),
            ("ui/color_picker.rs", include_str!("color_picker.rs")),
            ("ui/menu/mod.rs", include_str!("menu/mod.rs")),
            (
                "windows/capture_preview.rs",
                include_str!("../windows/capture_preview.rs"),
            ),
            (
                "windows/history/mod.rs",
                include_str!("../windows/history/mod.rs"),
            ),
            (
                "windows/keepalive.rs",
                include_str!("../windows/keepalive.rs"),
            ),
            (
                "windows/onboarding.rs",
                include_str!("../windows/onboarding.rs"),
            ),
            ("windows/pin.rs", include_str!("../windows/pin.rs")),
            (
                "windows/recording_control.rs",
                include_str!("../windows/recording_control.rs"),
            ),
            (
                "windows/scroll_capture.rs",
                include_str!("../windows/scroll_capture.rs"),
            ),
            (
                "windows/settings/mod.rs",
                include_str!("../windows/settings/mod.rs"),
            ),
            (
                "windows/tray_menu.rs",
                include_str!("../windows/tray_menu.rs"),
            ),
        ];
        for (name, source) in roots {
            assert!(
                source.contains("font::root()") || source.contains("font::UI_FONT"),
                "{name} must set the family at its window root"
            );
        }
    }

    #[test]
    fn a_window_root_resolves_to_geist() {
        let mut root = root();
        let family = root
            .style()
            .text
            .font_family
            .clone()
            .expect("a window root must name its family");
        assert_eq!(family.as_ref(), UI_FONT);
        assert_eq!(MONO_FONT, "Geist Mono");
    }
}
