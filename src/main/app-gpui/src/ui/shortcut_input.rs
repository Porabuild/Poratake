//! Port of `renderer/components/settings/shortcut-input.tsx` — click to
//! record, press a combination, Escape to cancel, Backspace to clear. Settings
//! and onboarding both record shortcuts, so the widget is generic over the
//! window that owns the recording state.

use gpui::{
    div, prelude::*, px, AnyElement, Context, KeyDownEvent, Render, SharedString, Styled, Window,
};
use herogpui::gpui;

use crate::system::accelerator;
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use herogpui::components::{Button, Size, Variant};

/// A window that can put one shortcut field into recording mode. The window
/// passes its own recording state to [`render`], so the trait only carries the
/// transition.
pub trait ShortcutRecorder: Render + Sized {
    fn start_recording_shortcut(
        &mut self,
        id: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    );
}

pub const SHORTCUT_MIN_WIDTH_DEFAULT: f32 = 180.0;
pub const SHORTCUT_MIN_WIDTH_SINGLE_DEFAULT: f32 = 80.0;
pub const SHORTCUT_GAP_DEFAULT: f32 = 8.0;
const TEXT_BASE: f32 = 16.0;

#[derive(Clone, Copy)]
struct Metrics {
    size: Size,
    gap: f32,
    min_width: f32,
    text_size: f32,
}

fn metrics(compact: bool, single_key: bool) -> Metrics {
    if compact {
        return Metrics {
            size: Size::Sm,
            gap: chrome::SHORTCUT_GAP,
            min_width: if single_key {
                chrome::SHORTCUT_MIN_WIDTH_SINGLE
            } else {
                chrome::SHORTCUT_MIN_WIDTH
            },
            text_size: chrome::TEXT_SM,
        };
    }
    Metrics {
        size: Size::Md,
        gap: SHORTCUT_GAP_DEFAULT,
        min_width: if single_key {
            SHORTCUT_MIN_WIDTH_SINGLE_DEFAULT
        } else {
            SHORTCUT_MIN_WIDTH_DEFAULT
        },
        text_size: TEXT_BASE,
    }
}

pub fn render<V: ShortcutRecorder>(
    id: &'static str,
    value: &str,
    single_key: bool,
    recording: bool,
    cx: &mut Context<V>,
    apply: impl Fn(&mut V, String, &mut Context<V>) + 'static,
) -> AnyElement {
    render_sized(id, value, single_key, recording, false, cx, apply)
}

pub fn render_sized<V: ShortcutRecorder>(
    id: &'static str,
    value: &str,
    single_key: bool,
    // The owner is mid-render, so it cannot be read back out of the context:
    // `Entity::read` panics while an entity is being updated.
    recording: bool,
    compact: bool,
    cx: &mut Context<V>,
    apply: impl Fn(&mut V, String, &mut Context<V>) + 'static,
) -> AnyElement {
    let metrics = metrics(compact, single_key);
    let display = if recording {
        if single_key {
            "Press key...".to_string()
        } else {
            "Press keys...".to_string()
        }
    } else if value.is_empty() {
        if single_key {
            "Press a key".to_string()
        } else {
            "Record shortcut".to_string()
        }
    } else if single_key && value.chars().count() == 1 {
        value.to_uppercase()
    } else {
        accelerator::display_spaced(value)
    };

    let apply = std::rc::Rc::new(apply);
    let mut row = div().flex().flex_row().items_center().gap(px(metrics.gap));

    if !value.is_empty() && !recording {
        row = row.child(
            herogpui::components::Tooltip::new("Clear shortcut").child(
                Button::new(SharedString::from(format!("{id}-clear")))
                    .variant(Variant::Ghost)
                    .size(metrics.size)
                    .is_icon_only(true)
                    .child(icon_element("x", px(chrome::TOOL_BUTTON_ICON)))
                    .recipe("muted")
                    .on_press(cx.listener(move |this, _event, _window, cx| {
                        apply(this, String::new(), cx);
                    })),
            ),
        );
    }

    row.child(
        Button::new(SharedString::from(format!("{id}-shortcut")))
            .when(recording, |el| el.variant(Variant::Primary))
            .when(!recording, |el| el.variant(Variant::Outline))
            .size(metrics.size)
            .label(display)
            .sx(move |el| {
                el.min_w(px(metrics.min_width))
                    .text_size(px(metrics.text_size))
                    .font_weight(gpui::FontWeight::NORMAL)
            })
            .on_press(cx.listener(move |this, _event, window, cx| {
                this.start_recording_shortcut(id, window, cx);
            })),
    )
    .into_any_element()
}

/// Builds the Electron accelerator string for a keystroke, or `None` while the
/// user is still holding only modifiers.
///
/// `single_key` fields take a bare alphanumeric key and nothing else; every
/// other field is a global accelerator, which the renderer refuses to record
/// without at least one modifier.
pub fn combination_from(event: &KeyDownEvent, single_key: bool) -> Option<String> {
    let key = event.keystroke.key.as_str();
    if matches!(key, "shift" | "control" | "alt" | "platform" | "function") {
        return None;
    }

    let modifiers = event.keystroke.modifiers;
    if single_key {
        let mut chars = key.chars();
        let single = chars.next().filter(|_| chars.next().is_none())?;
        return single.is_ascii_alphanumeric().then(|| single.to_string());
    }
    if !(modifiers.control || modifiers.alt || modifiers.shift || modifiers.platform) {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    if modifiers.control {
        parts.push("Control");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    if modifiers.platform {
        parts.push("CommandOrControl");
    }

    let named = match key {
        "escape" => return None,
        "enter" => "Return",
        "space" => "Space",
        "up" => "Up",
        "down" => "Down",
        "left" => "Left",
        "right" => "Right",
        other => other,
    };
    let key_label = if named.chars().count() == 1 {
        named.to_uppercase()
    } else {
        let mut chars = named.chars();
        let first = chars.next()?;
        first.to_uppercase().collect::<String>() + chars.as_str()
    };

    let mut combination = parts.join("+");
    if !combination.is_empty() {
        combination.push('+');
    }
    combination.push_str(&key_label);

    // A bare key is only a valid global shortcut for the single-key editor
    // bindings; the accelerator parser is the source of truth either way.
    accelerator::parse(&combination).map(|_| combination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Keystroke, Modifiers};
    use herogpui::gpui;

    /// Most fields are global accelerators, so the tests below exercise the
    /// modifier-required path.
    fn combination_from_test(event: &KeyDownEvent) -> Option<String> {
        combination_from(event, false)
    }

    fn event(key: &str, modifiers: Modifiers) -> KeyDownEvent {
        KeyDownEvent {
            keystroke: Keystroke {
                modifiers,
                key: key.to_string(),
                key_char: None,
            },
            is_held: false,
            prefer_character_input: false,
        }
    }

    #[test]
    fn a_modifier_alone_records_nothing() {
        assert!(combination_from_test(&event("shift", Modifiers::default())).is_none());
        assert!(combination_from_test(&event("control", Modifiers::default())).is_none());
    }

    #[test]
    fn escape_cancels_rather_than_recording() {
        assert!(combination_from_test(&event("escape", Modifiers::default())).is_none());
    }

    #[test]
    fn modifiers_are_written_in_the_electron_order() {
        let combination = combination_from_test(&event(
            "4",
            Modifiers {
                control: true,
                alt: true,
                shift: true,
                platform: true,
                function: false,
            },
        ))
        .expect("combination");
        assert_eq!(combination, "Control+Alt+Shift+CommandOrControl+4");
    }

    #[test]
    fn the_compact_variant_is_the_only_small_one() {
        let compact = metrics(true, false);
        assert_eq!(compact.size, Size::Sm);
        assert_eq!(compact.min_width, chrome::SHORTCUT_MIN_WIDTH);
        assert_eq!(compact.gap, chrome::SHORTCUT_GAP);
        assert_eq!(compact.text_size, chrome::TEXT_SM);
        assert_eq!(
            metrics(true, true).min_width,
            chrome::SHORTCUT_MIN_WIDTH_SINGLE
        );

        let default = metrics(false, false);
        assert_eq!(default.size, Size::Md);
        assert_eq!(default.min_width, SHORTCUT_MIN_WIDTH_DEFAULT);
        assert_eq!(default.gap, SHORTCUT_GAP_DEFAULT);
        assert_eq!(default.text_size, TEXT_BASE);
        assert_eq!(
            metrics(false, true).min_width,
            SHORTCUT_MIN_WIDTH_SINGLE_DEFAULT
        );
    }

    #[test]
    fn named_keys_are_capitalized() {
        let combination = combination_from_test(&event(
            "enter",
            Modifiers {
                platform: true,
                ..Modifiers::default()
            },
        ))
        .expect("combination");
        assert_eq!(combination, "CommandOrControl+Return");
    }
}
