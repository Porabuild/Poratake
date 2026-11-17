use herogpui::gpui;

use crate::editor::annotations::Annotation;
use crate::editor::window::EditorWindow;
use crate::ui::colors::Tool;

use super::{
    event, point, px, settle, HeadlessAppContext, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Path, PlatformInput,
};

pub(super) fn draw_rectangle(
    cx: &mut HeadlessAppContext,
    editor: gpui::WindowHandle<EditorWindow>,
    horizontal: bool,
    zoom: f32,
    scale: f32,
    scratch: &Path,
    transparent: bool,
) {
    editor
        .update(cx, |editor, _, cx| {
            editor.tool = Tool::Rectangle;
            cx.notify();
        })
        .unwrap();
    settle(cx, editor.into());
    let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
    let (start_x, start_y, end_x, end_y) = if horizontal {
        let x = f32::from(bounds.left()).max(300.) + 40. * zoom;
        let y = f32::from(bounds.top()) + 20. * zoom;
        (x, y, x + 60. * zoom, y + 80. * zoom)
    } else {
        let x = f32::from(bounds.left()) + 20. * zoom;
        let y = f32::from(bounds.top()).max(200.) + 40. * zoom;
        (x, y, x + 80. * zoom, y + 60. * zoom)
    };
    assert!(start_x >= 200. && start_y >= 120. && end_x < 1080. && end_y < 650.);
    let before = cx.capture_screenshot(editor.into()).unwrap();
    let start = point(px(start_x), px(start_y));
    let end = point(px(end_x), px(end_y));
    event(
        cx,
        editor.into(),
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    let begun = std::time::Instant::now();
    event(
        cx,
        editor.into(),
        PlatformInput::MouseMove(MouseMoveEvent {
            position: end,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, editor.into());
    let during = cx.capture_screenshot(editor.into()).unwrap();
    let elapsed = begun.elapsed();
    if std::env::var_os("PORATAKE_PARITY_LARGE_SAVE_IMAGES").is_some() {
        before
            .save(scratch.join(format!("interactive-before-{horizontal}-{transparent}.png")))
            .unwrap();
        during
            .save(scratch.join(format!("interactive-draft-{horizontal}-{transparent}.png")))
            .unwrap();
    }
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        assert!(editor.history.current().is_empty());
        assert!(matches!(
            editor.snapshot.borrow().draft,
            Some(Annotation::Rectangle { .. })
        ));
    });
    let mut changed = 0;
    for y in (start_y * scale).floor() as u32..=(end_y * scale).ceil() as u32 {
        for x in (start_x * scale).floor() as u32..=(end_x * scale).ceil() as u32 {
            changed += usize::from(before.get_pixel(x, y) != during.get_pixel(x, y));
        }
    }
    assert!(changed > 0, "the in-progress rectangle must be visible");
    event(
        cx,
        editor.into(),
        PlatformInput::MouseUp(MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, editor.into());
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        assert!(matches!(
            editor.history.current(),
            [Annotation::Rectangle { .. }]
        ));
        assert!(editor.snapshot.borrow().draft.is_none());
    });
    println!("large interactive horizontal={horizontal} zoom={zoom} scale={scale} changed_pixels={changed} move_settle_capture_ms={:.2}", elapsed.as_secs_f64() * 1000.);
}
