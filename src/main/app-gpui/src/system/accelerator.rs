use global_hotkey::hotkey::{Code, HotKey, Modifiers};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Accelerator {
    pub modifiers: Modifiers,
    pub code: Code,
}

impl Accelerator {
    pub fn hotkey(self) -> HotKey {
        HotKey::new(Some(self.modifiers), self.code)
    }

    pub fn menu(self) -> muda::accelerator::Accelerator {
        muda::accelerator::Accelerator::new(Some(self.modifiers), self.code)
    }
}

pub fn parse(value: &str) -> Option<Accelerator> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut tokens = trimmed
        .split('+')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let key = tokens.pop()?;

    let mut modifiers = Modifiers::empty();
    for token in tokens {
        modifiers |= parse_modifier(token)?;
    }

    Some(Accelerator {
        modifiers,
        code: parse_code(key)?,
    })
}

fn parse_modifier(token: &str) -> Option<Modifiers> {
    match token.to_ascii_lowercase().as_str() {
        "alt" | "option" => Some(Modifiers::ALT),
        "ctrl" | "control" => Some(Modifiers::CONTROL),
        "shift" => Some(Modifiers::SHIFT),
        "cmd" | "command" | "super" | "meta" => Some(Modifiers::SUPER),
        "commandorcontrol" | "cmdorctrl" | "commandorctrl" | "cmdorcontrol" => {
            if cfg!(target_os = "macos") {
                Some(Modifiers::SUPER)
            } else {
                Some(Modifiers::CONTROL)
            }
        }
        _ => None,
    }
}

fn parse_code(key: &str) -> Option<Code> {
    let upper = key.to_ascii_uppercase();

    if let Some(digit) = upper.strip_prefix("DIGIT").or_else(|| {
        (upper.len() == 1 && upper.as_bytes()[0].is_ascii_digit()).then_some(upper.as_str())
    }) {
        return match digit {
            "0" => Some(Code::Digit0),
            "1" => Some(Code::Digit1),
            "2" => Some(Code::Digit2),
            "3" => Some(Code::Digit3),
            "4" => Some(Code::Digit4),
            "5" => Some(Code::Digit5),
            "6" => Some(Code::Digit6),
            "7" => Some(Code::Digit7),
            "8" => Some(Code::Digit8),
            "9" => Some(Code::Digit9),
            _ => None,
        };
    }

    if let Some(letter) = upper.strip_prefix("KEY").or_else(|| {
        (upper.len() == 1 && upper.as_bytes()[0].is_ascii_alphabetic()).then_some(upper.as_str())
    }) {
        return letter_code(letter);
    }

    if let Some(number) = upper.strip_prefix('F') {
        if let Ok(index) = number.parse::<u8>() {
            return function_code(index);
        }
    }

    match upper.as_str() {
        "SPACE" => Some(Code::Space),
        "ENTER" | "RETURN" => Some(Code::Enter),
        "TAB" => Some(Code::Tab),
        "ESC" | "ESCAPE" => Some(Code::Escape),
        "BACKSPACE" => Some(Code::Backspace),
        "DELETE" | "DEL" => Some(Code::Delete),
        "INSERT" => Some(Code::Insert),
        "HOME" => Some(Code::Home),
        "END" => Some(Code::End),
        "PAGEUP" => Some(Code::PageUp),
        "PAGEDOWN" => Some(Code::PageDown),
        "UP" | "ARROWUP" => Some(Code::ArrowUp),
        "DOWN" | "ARROWDOWN" => Some(Code::ArrowDown),
        "LEFT" | "ARROWLEFT" => Some(Code::ArrowLeft),
        "RIGHT" | "ARROWRIGHT" => Some(Code::ArrowRight),
        "PRINTSCREEN" => Some(Code::PrintScreen),
        "PLUS" => Some(Code::Equal),
        "-" | "MINUS" => Some(Code::Minus),
        "=" | "EQUAL" => Some(Code::Equal),
        "[" | "BRACKETLEFT" => Some(Code::BracketLeft),
        "]" | "BRACKETRIGHT" => Some(Code::BracketRight),
        "\\" | "BACKSLASH" => Some(Code::Backslash),
        ";" | "SEMICOLON" => Some(Code::Semicolon),
        "'" | "QUOTE" => Some(Code::Quote),
        "," | "COMMA" => Some(Code::Comma),
        "." | "PERIOD" => Some(Code::Period),
        "/" | "SLASH" => Some(Code::Slash),
        "`" | "BACKQUOTE" => Some(Code::Backquote),
        _ => None,
    }
}

fn letter_code(letter: &str) -> Option<Code> {
    match letter {
        "A" => Some(Code::KeyA),
        "B" => Some(Code::KeyB),
        "C" => Some(Code::KeyC),
        "D" => Some(Code::KeyD),
        "E" => Some(Code::KeyE),
        "F" => Some(Code::KeyF),
        "G" => Some(Code::KeyG),
        "H" => Some(Code::KeyH),
        "I" => Some(Code::KeyI),
        "J" => Some(Code::KeyJ),
        "K" => Some(Code::KeyK),
        "L" => Some(Code::KeyL),
        "M" => Some(Code::KeyM),
        "N" => Some(Code::KeyN),
        "O" => Some(Code::KeyO),
        "P" => Some(Code::KeyP),
        "Q" => Some(Code::KeyQ),
        "R" => Some(Code::KeyR),
        "S" => Some(Code::KeyS),
        "T" => Some(Code::KeyT),
        "U" => Some(Code::KeyU),
        "V" => Some(Code::KeyV),
        "W" => Some(Code::KeyW),
        "X" => Some(Code::KeyX),
        "Y" => Some(Code::KeyY),
        "Z" => Some(Code::KeyZ),
        _ => None,
    }
}

fn function_code(index: u8) -> Option<Code> {
    match index {
        1 => Some(Code::F1),
        2 => Some(Code::F2),
        3 => Some(Code::F3),
        4 => Some(Code::F4),
        5 => Some(Code::F5),
        6 => Some(Code::F6),
        7 => Some(Code::F7),
        8 => Some(Code::F8),
        9 => Some(Code::F9),
        10 => Some(Code::F10),
        11 => Some(Code::F11),
        12 => Some(Code::F12),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_electron_style_accelerators() {
        let parsed = parse("Alt+Shift+4").expect("alt shift 4");
        assert_eq!(parsed.modifiers, Modifiers::ALT | Modifiers::SHIFT);
        assert_eq!(parsed.code, Code::Digit4);

        let parsed = parse("CommandOrControl+Shift+S").expect("cmd or ctrl shift s");
        assert_eq!(parsed.code, Code::KeyS);

        assert!(parse("").is_none());
        assert!(parse("Alt+Nope").is_none());
    }

    #[test]
    fn parses_bare_keys_and_named_keys() {
        assert_eq!(parse("p").expect("p").code, Code::KeyP);
        assert_eq!(parse("F5").expect("f5").code, Code::F5);
        assert_eq!(parse("Alt+Left").expect("alt left").code, Code::ArrowLeft);
    }
}

const MAC_MODIFIER_SYMBOLS: [(&str, &str); 11] = [
    ("COMMANDORCONTROL", "\u{2318}"),
    ("CMDORCTRL", "\u{2318}"),
    ("COMMAND", "\u{2318}"),
    ("CMD", "\u{2318}"),
    ("META", "\u{2318}"),
    ("SUPER", "\u{2318}"),
    ("CONTROL", "\u{2303}"),
    ("CTRL", "\u{2303}"),
    ("ALT", "\u{2325}"),
    ("OPTION", "\u{2325}"),
    ("SHIFT", "\u{21e7}"),
];

const PC_MODIFIER_SYMBOLS: [(&str, &str); 11] = [
    ("COMMANDORCONTROL", "Ctrl"),
    ("CMDORCTRL", "Ctrl"),
    ("COMMAND", "Win"),
    ("CMD", "Win"),
    ("META", "Win"),
    ("SUPER", "Win"),
    ("CONTROL", "Ctrl"),
    ("CTRL", "Ctrl"),
    ("ALT", "Alt"),
    ("OPTION", "Alt"),
    ("SHIFT", "Shift"),
];

fn modifier_symbols() -> &'static [(&'static str, &'static str)] {
    if cfg!(target_os = "macos") {
        &MAC_MODIFIER_SYMBOLS
    } else {
        &PC_MODIFIER_SYMBOLS
    }
}

pub fn primary_modifier_label() -> &'static str {
    modifier_symbols()[0].1
}

/// Port of `formatAccelerator` in `renderer/utils/shortcuts.ts`.
pub fn display(value: &str) -> String {
    display_with(value, "")
}

/// `formatAccelerator(value, ' ')` — the shortcut fields separate the parts
/// with a space rather than a plus.
pub fn display_spaced(value: &str) -> String {
    display_with(value, " ")
}

fn display_with(value: &str, separator: &str) -> String {
    if value.trim().is_empty() {
        return String::new();
    }
    let symbols = modifier_symbols();
    let mut parts: Vec<String> = Vec::new();
    let mut key: Option<&str> = None;

    for token in value.split('+') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let upper = token.to_ascii_uppercase();
        match symbols.iter().find(|(name, _)| *name == upper) {
            Some((_, symbol)) => parts.push((*symbol).to_string()),
            None => key = Some(token),
        }
    }

    if let Some(key) = key {
        parts.push(key.to_string());
    }

    // The mac symbols are glyphs, so they join with the caller separator (empty
    // by default); elsewhere an empty separator falls back to a plus.
    if cfg!(target_os = "macos") || !separator.is_empty() {
        parts.join(separator)
    } else {
        parts.join("+")
    }
}

#[cfg(test)]
mod display_tests {
    use super::*;

    #[test]
    fn formats_accelerators_with_a_space_separator() {
        assert_eq!(display_spaced(""), "");
        if cfg!(target_os = "macos") {
            assert_eq!(
                display_spaced("CommandOrControl+Shift+Z"),
                "\u{2318} \u{21e7} Z"
            );
        } else {
            assert_eq!(display_spaced("CommandOrControl+Shift+Z"), "Ctrl Shift Z");
            assert_eq!(display_spaced("p"), "p");
        }
    }

    #[test]
    fn formats_accelerators_for_display() {
        assert_eq!(display(""), "");
        if cfg!(target_os = "macos") {
            assert_eq!(display("CommandOrControl+Shift+Z"), "\u{2318}\u{21e7}Z");
        } else {
            assert_eq!(display("CommandOrControl+Shift+Z"), "Ctrl+Shift+Z");
            assert_eq!(display("Alt+Shift+4"), "Alt+Shift+4");
            assert_eq!(display("p"), "p");
        }
    }
}
