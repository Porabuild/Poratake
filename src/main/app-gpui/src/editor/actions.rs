//! Editor actions and key bindings — port of `useEditorToolShortcuts` and the
//! editor command shortcuts in `screenshot-window.tsx`.

use gpui::{App, KeyBinding, Modifiers};
use herogpui::actions;
use herogpui::gpui;

use crate::config::shortcuts::{EditorActionShortcuts, EditorShortcuts};

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
        SelectAllAnnotations,
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
        CloudUpload,
    ]
);

pub fn is_meta_held(modifiers: &Modifiers) -> bool {
    modifiers.secondary()
}

pub fn capture_overlay_visible(capture_mode: bool, meta_held: bool) -> bool {
    capture_mode || meta_held
}

pub fn action_bindings(shortcuts: &EditorActionShortcuts) -> Vec<KeyBinding> {
    crate::system::accelerator::keystroke(&shortcuts.upload_to_cloud)
        .map(|keystroke| vec![KeyBinding::new(&keystroke, CloudUpload, Some("Editor"))])
        .unwrap_or_default()
}

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
    let mut bindings = vec![
        KeyBinding::new("ctrl-z", Undo, editor),
        KeyBinding::new("ctrl-shift-z", Redo, editor),
        KeyBinding::new("ctrl-c", CopyAnnotation, editor),
        KeyBinding::new("ctrl-x", CutAnnotation, editor),
        KeyBinding::new("ctrl-v", PasteAnnotation, editor),
        KeyBinding::new("ctrl-a", SelectAllAnnotations, editor),
        KeyBinding::new("delete", DeleteAnnotation, editor),
        KeyBinding::new("backspace", DeleteAnnotation, editor),
        KeyBinding::new("ctrl-s", SaveScreenshot, editor),
        KeyBinding::new("ctrl-equal", ZoomIn, editor),
        KeyBinding::new("ctrl-minus", ZoomOut, editor),
        KeyBinding::new("ctrl-0", ZoomReset, editor),
        KeyBinding::new("ctrl-p", PrintScreenshot, editor),
        KeyBinding::new("ctrl-backspace", DeleteScreenshot, editor),
        KeyBinding::new("enter", ApplyCrop, editor),
        KeyBinding::new("escape", CancelCrop, editor),
    ];
    // The renderer answers both Cmd and Control (`e.metaKey || e.ctrlKey`) while
    // GPUI leaves `ctrl` on Control everywhere, so macOS gets the Cmd twins.
    if cfg!(target_os = "macos") {
        bindings.extend([
            KeyBinding::new("cmd-z", Undo, editor),
            KeyBinding::new("cmd-shift-z", Redo, editor),
            KeyBinding::new("cmd-c", CopyAnnotation, editor),
            KeyBinding::new("cmd-x", CutAnnotation, editor),
            KeyBinding::new("cmd-v", PasteAnnotation, editor),
            KeyBinding::new("cmd-a", SelectAllAnnotations, editor),
            KeyBinding::new("cmd-s", SaveScreenshot, editor),
            KeyBinding::new("cmd-equal", ZoomIn, editor),
            KeyBinding::new("cmd-minus", ZoomOut, editor),
            KeyBinding::new("cmd-0", ZoomReset, editor),
            KeyBinding::new("cmd-p", PrintScreenshot, editor),
            KeyBinding::new("cmd-backspace", DeleteScreenshot, editor),
        ]);
    }
    bindings
}

/// Installs the editor keymap. Called at startup and again whenever the tool
/// shortcuts change, so a rebound key takes effect without a restart.
pub fn init_bindings(cx: &mut App) {
    let shortcuts = crate::state::state(cx).config.get().shortcuts;
    let mut bindings = tool_bindings(&shortcuts.editor);
    bindings.extend(action_bindings(&shortcuts.editor_actions));
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
    fn the_modifier_reveals_the_capture_picker_until_it_is_released() {
        let mut meta_held = false;
        assert!(!capture_overlay_visible(false, meta_held));

        meta_held = is_meta_held(&Modifiers::secondary_key());
        assert!(meta_held);
        assert!(capture_overlay_visible(false, meta_held));

        meta_held = is_meta_held(&Modifiers::none());
        assert!(!meta_held);
        assert!(!capture_overlay_visible(false, meta_held));

        assert!(capture_overlay_visible(true, false));
        assert!(is_meta_held(&Modifiers::secondary_key()));
        assert!(!is_meta_held(&Modifiers::shift()));
    }

    #[test]
    fn the_cloud_upload_shortcut_is_bound_from_config() {
        assert_eq!(action_bindings(&EditorActionShortcuts::default()).len(), 1);
        let cleared = EditorActionShortcuts {
            upload_to_cloud: String::new(),
        };
        assert!(action_bindings(&cleared).is_empty());
    }

    #[test]
    fn the_command_bindings_are_fixed() {
        let expected = if cfg!(target_os = "macos") {
            16 + 12
        } else {
            16
        };
        assert_eq!(command_bindings().len(), expected);
    }
}
