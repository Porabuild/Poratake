use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    point, px, size, AnyWindowHandle, AppContext, HeadlessAppContext, KeyDownEvent, Keystroke,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput,
};
use herogpui::gpui;

fn frame(cx: &mut HeadlessAppContext, window: AnyWindowHandle) {
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .unwrap();
}

fn settle(cx: &mut HeadlessAppContext, window: AnyWindowHandle) {
    for _ in 0..40 {
        cx.advance_clock(Duration::from_millis(10));
        cx.update_window(window, |_, window, cx| window.simulate_next_frame(cx))
            .unwrap();
        frame(cx, window);
    }
}

fn event(cx: &mut HeadlessAppContext, window: AnyWindowHandle, event: PlatformInput) {
    cx.update_window(window, |_, window, cx| window.dispatch_event(event, cx))
        .unwrap();
    frame(cx, window);
}

fn hover(cx: &mut HeadlessAppContext, window: AnyWindowHandle, x: f32, y: f32) {
    event(
        cx,
        window,
        PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(x), px(y)),
            pressed_button: None,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, window);
}

fn click(cx: &mut HeadlessAppContext, window: AnyWindowHandle, x: f32, y: f32) {
    hover(cx, window, x, y);
    event(
        cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: point(px(x), px(y)),
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    event(
        cx,
        window,
        PlatformInput::MouseUp(MouseUpEvent {
            position: point(px(x), px(y)),
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, window);
}

fn escape(cx: &mut HeadlessAppContext, window: AnyWindowHandle) {
    event(
        cx,
        window,
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke: Keystroke::parse("escape").unwrap(),
            is_held: false,
            prefer_character_input: false,
        }),
    );
    settle(cx, window);
}

fn capture(cx: &mut HeadlessAppContext, window: AnyWindowHandle, output: &Path, name: &str) {
    frame(cx, window);
    cx.capture_screenshot(window)
        .unwrap()
        .save(output.join(format!("gpui-{name}.png")))
        .unwrap();
}

pub fn capture_editor_interaction_states() {
    let output = std::path::PathBuf::from(std::env::var("PORATAKE_PARITY_OUTPUT").unwrap());
    let source = std::env::var("PORATAKE_PARITY_IMAGE").unwrap();
    std::fs::create_dir_all(&output).unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scratch = output.join(format!("profile-{nonce}"));
    std::fs::create_dir_all(&scratch).unwrap();
    unsafe { std::env::set_var(crate::config::store::CONFIG_DIR_ENV, &scratch) };
    let image = scratch.join("capture.png");
    std::fs::copy(source, &image).unwrap();
    let store =
        Arc::new(crate::config::store::ConfigStore::load_at(scratch.join("config.json")).unwrap());
    store.update(|config| {
        config.appearance.mode = "dark".into();
        config.appearance.theme = "default".into();
        config.editor.color = "#f43f5e".into();
    });
    let mut cx = HeadlessAppContext::with_platform(
        herogpui::platform::current_platform(true).text_system(),
        Arc::new(herogpui::HeroGpuiAssets),
        herogpui::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        crate::state::set_test_state(cx, store);
        crate::editor::actions::init_bindings(cx);
        crate::ui::font::register(cx);
    });
    let editor = cx
        .open_window(size(px(1280.0), px(738.0)), |window, cx| {
            cx.new(|cx| {
                crate::editor::window::EditorWindow::from_file(image.to_str().unwrap(), window, cx)
            })
        })
        .unwrap();
    let window = editor.into();
    settle(&mut cx, window);
    hover(&mut cx, window, 300.0, 300.0);
    capture(&mut cx, window, &output, "image-default");
    hover(&mut cx, window, 896.0, 20.0);
    capture(&mut cx, window, &output, "image-redact-hover");
    event(
        &mut cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: point(px(896.0), px(20.0)),
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    settle(&mut cx, window);
    capture(&mut cx, window, &output, "image-redact-pressed");
    event(
        &mut cx,
        window,
        PlatformInput::MouseUp(MouseUpEvent {
            position: point(px(896.0), px(20.0)),
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    click(&mut cx, window, 640.0, 20.0);
    cx.update(|cx| assert_eq!(editor.read(cx).unwrap().tool, crate::ui::colors::Tool::Pen));
    capture(&mut cx, window, &output, "image-pen-selected");
    hover(&mut cx, window, 520.0, 20.0);
    capture(&mut cx, window, &output, "image-thickness-hover");
    click(&mut cx, window, 520.0, 20.0);
    let panel_open = |cx: &mut HeadlessAppContext| {
        cx.update_window(window, |_, window, _| {
            let scale = window.scale_factor();
            window.painted_quads().iter().any(|quad| {
                quad.bounds.size.width.0 / scale <= 200.0
                    && quad.bounds.size.height.0 / scale >= 150.0
                    && quad.bounds.size.height.0 / scale <= 260.0
            })
        })
        .unwrap()
    };
    assert!(
        panel_open(&mut cx),
        "the thickness option list must be painted"
    );
    capture(&mut cx, window, &output, "image-thickness-open");
    escape(&mut cx, window);
    assert!(!panel_open(&mut cx), "Escape must dismiss the option list");
    click(&mut cx, window, 580.0, 20.0);
    capture(&mut cx, window, &output, "image-color-open");
    escape(&mut cx, window);
    for (tool, x) in [
        (crate::ui::colors::Tool::Highlight, 672.0),
        (crate::ui::colors::Tool::Rectangle, 704.0),
        (crate::ui::colors::Tool::Arrow, 800.0),
        (crate::ui::colors::Tool::Text, 832.0),
        (crate::ui::colors::Tool::Number, 864.0),
        (crate::ui::colors::Tool::Redact, 896.0),
        (crate::ui::colors::Tool::Wallpaper, 1005.0),
    ] {
        click(&mut cx, window, x, 20.0);
        cx.update(|cx| assert_eq!(editor.read(cx).unwrap().tool, tool));
        capture(
            &mut cx,
            window,
            &output,
            &format!("image-{}-selected", tool.id()),
        );
        if matches!(
            tool,
            crate::ui::colors::Tool::Number
                | crate::ui::colors::Tool::Text
                | crate::ui::colors::Tool::Redact
        ) {
            click(&mut cx, window, 520.0, 20.0);
            assert!(panel_open(&mut cx), "tool popover must open");
            capture(
                &mut cx,
                window,
                &output,
                &format!("image-{}-open", tool.id()),
            );
            let nested = match tool {
                crate::ui::colors::Tool::Number => (569.0, 225.0, "number-start"),
                crate::ui::colors::Tool::Text => (551.0, 185.0, "text-size"),
                _ => (568.0, 185.0, "redact-intensity"),
            };
            click(&mut cx, window, nested.0, nested.1);
            capture(
                &mut cx,
                window,
                &output,
                &format!("image-{}-open", nested.2),
            );
            escape(&mut cx, window);
            escape(&mut cx, window);
        } else if matches!(
            tool,
            crate::ui::colors::Tool::Arrow
                | crate::ui::colors::Tool::Rectangle
                | crate::ui::colors::Tool::Highlight
        ) {
            click(&mut cx, window, 520.0, 20.0);
            capture(
                &mut cx,
                window,
                &output,
                &format!("image-{}-open", tool.id()),
            );
            escape(&mut cx, window);
        }
    }
    let video = cx
        .open_window(size(px(1280.0), px(738.0)), |_, cx| {
            cx.new(|cx| crate::windows::video_editor::VideoEditorWindow::new_for_test(None, cx))
        })
        .unwrap();
    let window = video.into();
    settle(&mut cx, window);
    hover(&mut cx, window, 300.0, 300.0);
    capture(&mut cx, window, &output, "video-default");
    hover(&mut cx, window, 1258.0, 20.0);
    capture(&mut cx, window, &output, "video-header-hover");
    hover(&mut cx, window, 1260.0, 208.0);
    capture(&mut cx, window, &output, "video-audio-hover");
    click(&mut cx, window, 1260.0, 208.0);
    capture(&mut cx, window, &output, "video-audio-open");
}
