//! Editor actions and key bindings — port of `useEditorToolShortcuts` and the
//! editor command shortcuts in `screenshot-window.tsx`.

use gpui::{actions, App, KeyBinding};

use crate::config::shortcuts::EditorShortcuts;

actions!(
    editor,
    [
        ToolSelect,
        ToolPen,
        ToolHighlight,
        ToolRectangle,
        ToolCircle,
        ToolLine,
        ToolArrow,
        ToolText,
        ToolNumber,
        ToolRedact,
        ToolCrop,
        ToolWallpaper,
        ToggleCaptureMode,
        Undo,
        Redo,
        CopyScreenshot,
        CopyAnnotation,
        CutAnnotation,
        PasteAnnotation,
        DeleteAnnotation,
        SaveScreenshot,
        DeleteScreenshot,
        PrintScreenshot,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        TogglePalette,
        ApplyCrop,
        CancelCrop,
    ]
);

/// The tool bindings, taken from the user's settings so a rebound tool key
/// works here exactly as it does in the Electron editor. The command bindings
/// below are fixed, as they are there.
pub fn tool_bindings(shortcuts: &EditorShortcuts) -> Vec<KeyBinding> {
    let context = Some("Editor");
    let mut bindings = Vec::new();

    // A shortcut the user cleared binds nothing, rather than binding an empty
    // keystroke that gpui would reject.
    macro_rules! bind {
        ($field:ident, $action:expr) => {
            if !shortcuts.$field.is_empty() {
                bindings.push(KeyBinding::new(&shortcuts.$field, $action, context));
            }
        };
    }

    bind!(select, ToolSelect);
    bind!(pen, ToolPen);
    bind!(highlight, ToolHighlight);
    bind!(rectangle, ToolRectangle);
    bind!(circle, ToolCircle);
    bind!(line, ToolLine);
    bind!(arrow, ToolArrow);
    bind!(text, ToolText);
    bind!(number, ToolNumber);
    bind!(redact, ToolRedact);
    bind!(crop, ToolCrop);
    bind!(wallpaper, ToolWallpaper);

    bindings
}

fn command_bindings() -> Vec<KeyBinding> {
    let editor = Some("Editor");
    vec![
        KeyBinding::new("ctrl-z", Undo, editor),
        KeyBinding::new("ctrl-shift-z", Redo, editor),
        KeyBinding::new("ctrl-c", CopyAnnotation, editor),
        KeyBinding::new("ctrl-x", CutAnnotation, editor),
        KeyBinding::new("ctrl-v", PasteAnnotation, editor),
        KeyBinding::new("delete", DeleteAnnotation, editor),
        KeyBinding::new("ctrl-s", SaveScreenshot, editor),
        KeyBinding::new("ctrl-equal", ZoomIn, editor),
        KeyBinding::new("ctrl-minus", ZoomOut, editor),
        KeyBinding::new("ctrl-0", ZoomReset, editor),
        KeyBinding::new("ctrl-p", PrintScreenshot, editor),
        KeyBinding::new("ctrl-backspace", DeleteScreenshot, editor),
        KeyBinding::new("enter", ApplyCrop, editor),
        KeyBinding::new("escape", CancelCrop, editor),
    ]
}

/// Installs the editor keymap. Called at startup and again whenever the tool
/// shortcuts change, so a rebound key takes effect without a restart.
pub fn init_bindings(cx: &mut App) {
    let shortcuts = crate::state::state(cx).config.get().shortcuts.editor;
    let mut bindings = tool_bindings(&shortcuts);
    bindings.extend(command_bindings());
    cx.bind_keys(bindings);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_gets_a_binding_from_the_defaults() {
        assert_eq!(tool_bindings(&EditorShortcuts::default()).len(), 12);
    }

    #[test]
    fn a_cleared_shortcut_binds_nothing() {
        let shortcuts = EditorShortcuts {
            pen: String::new(),
            crop: String::new(),
            ..EditorShortcuts::default()
        };
        assert_eq!(tool_bindings(&shortcuts).len(), 10);
    }

    #[test]
    fn the_command_bindings_are_fixed() {
        assert_eq!(command_bindings().len(), 14);
    }
}
