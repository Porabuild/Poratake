//! Editor actions and key bindings — port of `useEditorToolShortcuts` and the
//! editor command shortcuts in `screenshot-window.tsx`.

use std::rc::Rc;

use gpui::{Action, App, KeyBinding, KeyBindingContextPredicate, Modifiers};
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

pub fn is_meta_held(modifiers: &Modifiers, cx: &App) -> bool {
    modifiers.secondary(cx)
}

pub fn capture_overlay_visible(capture_mode: bool, meta_held: bool) -> bool {
    capture_mode || meta_held
}

fn editor_context() -> Rc<KeyBindingContextPredicate> {
    Rc::new(KeyBindingContextPredicate::parse("Editor").expect("the Editor key context parses"))
}

fn load_user_binding(
    keystroke: &str,
    action: Box<dyn Action>,
    context: &Rc<KeyBindingContextPredicate>,
    cx: &App,
) -> Option<KeyBinding> {
    match KeyBinding::load(keystroke, action, Some(context.clone()), false, None, cx) {
        Ok(binding) => Some(binding),
        Err(error) => {
            eprintln!("[shortcuts] ignoring invalid '{keystroke}' keystroke: {error}");
            None
        }
    }
}

pub fn action_bindings(shortcuts: &EditorActionShortcuts, cx: &App) -> Vec<KeyBinding> {
    let context = editor_context();
    crate::system::accelerator::keystroke(&shortcuts.upload_to_cloud)
        .and_then(|keystroke| load_user_binding(&keystroke, Box::new(CloudUpload), &context, cx))
        .into_iter()
        .collect()
}

/// The tool bindings, taken from the user's settings so a rebound tool key
/// works here exactly as it does in the Electron editor. The command bindings
/// below are fixed, as they are there.
pub fn tool_bindings(shortcuts: &EditorShortcuts, cx: &App) -> Vec<KeyBinding> {
    let context = editor_context();
    let mut bindings = Vec::new();

    // A shortcut the user cleared binds nothing, rather than binding an empty
    // keystroke that gpui would reject.
    macro_rules! bind {
        ($field:ident, $action:expr) => {
            if !shortcuts.$field.is_empty() {
                bindings.extend(load_user_binding(
                    &shortcuts.$field,
                    Box::new($action),
                    &context,
                    cx,
                ));
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
    let mut bindings = tool_bindings(&shortcuts.editor, cx);
    bindings.extend(action_bindings(&shortcuts.editor_actions, cx));
    bindings.extend(command_bindings());
    cx.bind_keys(bindings);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[herogpui::test]
    fn every_tool_gets_a_binding_from_the_defaults(cx: &mut gpui::TestAppContext) {
        let count = cx.update(|cx| tool_bindings(&EditorShortcuts::default(), cx).len());
        assert_eq!(count, 12);
    }

    #[herogpui::test]
    fn a_cleared_shortcut_binds_nothing(cx: &mut gpui::TestAppContext) {
        let shortcuts = EditorShortcuts {
            pen: String::new(),
            crop: String::new(),
            ..EditorShortcuts::default()
        };
        let count = cx.update(|cx| tool_bindings(&shortcuts, cx).len());
        assert_eq!(count, 10);
    }

    #[herogpui::test]
    fn an_invalid_shortcut_is_skipped(cx: &mut gpui::TestAppContext) {
        let shortcuts = EditorShortcuts {
            pen: "ctrl-x-y".into(),
            ..EditorShortcuts::default()
        };
        let count = cx.update(|cx| tool_bindings(&shortcuts, cx).len());
        assert_eq!(count, 11);
    }

    #[herogpui::test]
    fn the_modifier_reveals_the_capture_picker_until_it_is_released(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let mut meta_held = false;
            assert!(!capture_overlay_visible(false, meta_held));

            meta_held = is_meta_held(&Modifiers::secondary_key(cx), cx);
            assert!(meta_held);
            assert!(capture_overlay_visible(false, meta_held));

            meta_held = is_meta_held(&Modifiers::none(), cx);
            assert!(!meta_held);
            assert!(!capture_overlay_visible(false, meta_held));

            assert!(capture_overlay_visible(true, false));
            assert!(is_meta_held(&Modifiers::secondary_key(cx), cx));
            assert!(!is_meta_held(&Modifiers::shift(), cx));
        });
    }

    #[herogpui::test]
    fn the_cloud_upload_shortcut_is_bound_from_config(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            assert_eq!(
                action_bindings(&EditorActionShortcuts::default(), cx).len(),
                1
            );
            let cleared = EditorActionShortcuts {
                upload_to_cloud: String::new(),
            };
            assert!(action_bindings(&cleared, cx).is_empty());
        });
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
