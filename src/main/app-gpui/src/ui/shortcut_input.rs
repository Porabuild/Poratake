//! Port of `renderer/components/settings/shortcut-input.tsx` — click to
//! record, press a combination, Escape to cancel, Backspace to clear. Settings
//! and onboarding both record shortcuts, so the widget is generic over the
//! window that owns the recording state.

use gpui::{
    div, prelude::*, px, AnyElement, Context, KeyDownEvent, Render, SharedString, Styled, Window,
};

use crate::system::accelerator;
use crate::theme::vars::ThemeVars;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;

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

/// `shortcut-input.tsx`: an `outline` button (`primary` while recording) that
/// reads the current binding, preceded by a ghost clear button. The compact
/// variant used by the shortcuts category is `size="sm"` at `min-w-36`.
pub fn render<V: ShortcutRecorder>(
    id: &'static str,
    value: &str,
    single_key: bool,
    // The owner is mid-render, so it cannot be read back out of the context:
    // `Entity::read` panics while an entity is being updated.
    recording: bool,
    theme: &ThemeVars,
    cx: &mut Context<V>,
    apply: impl Fn(&mut V, String, &mut Context<V>) + 'static,
) -> AnyElement {
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
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(chrome::SHORTCUT_GAP));

    if !value.is_empty() && !recording {
        row = row.child(
            Button::new(SharedString::from(format!("{id}-clear")))
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconSm)
                .icon("x")
                .icon_size(px(chrome::TOOL_BUTTON_ICON))
                .tooltip("Clear shortcut")
                .foreground(theme.muted_foreground)
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    apply(this, String::new(), cx);
                })),
        );
    }

    row.child(
        Button::new(SharedString::from(format!("{id}-shortcut")))
            .variant(if recording {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Outline
            })
            .size(ButtonSize::Sm)
            .label(display)
            .min_width(px(if single_key {
                chrome::SHORTCUT_MIN_WIDTH_SINGLE
            } else {
                chrome::SHORTCUT_MIN_WIDTH
            }))
            .font_weight(gpui::FontWeight::NORMAL)
            .on_click(cx.listener(move |this, _event, window, cx| {
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
        match chars.next() {
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            None => return None,
        }
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
