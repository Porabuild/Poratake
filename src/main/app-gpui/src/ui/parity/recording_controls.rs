use super::{
    capture, click_measured, escape, event, hover, key, measured, px, settle, HeadlessAppContext,
    Modifiers, MouseButton, MouseDownEvent, Path, PlatformInput,
};
use herogpui::gpui;

pub(super) fn run(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path, scale: f32) {
    use crate::video::recorder::{self, RecorderState, RecordingTarget};
    use crate::windows::recording_control::RecordingControl;
    use crate::windows::registry::{self, WindowKind};

    let recordings = scratch.join("recordings");
    std::fs::create_dir_all(&recordings).unwrap();
    let menu_painted = |cx: &mut HeadlessAppContext, window| {
        cx.update_window(window, |_, window, _| {
            window.painted_quads().iter().any(|quad| {
                let scale = window.scale_factor();
                let width = quad.bounds.size.width.0 / scale;
                let height = quad.bounds.size.height.0 / scale;
                (223.0..=257.0).contains(&width) && (80.0..=225.0).contains(&height)
            })
        })
        .unwrap()
    };
    for target in [RecordingTarget::Area, RecordingTarget::Window] {
        let area = target == RecordingTarget::Area;
        let window = cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.storage.recordings_path = recordings.to_string_lossy().into_owned();
                config.recording.start_delay = 3.;
                config.recording.mic_enabled = false;
                config.recording.camera.enabled = false;
                config.recording.system_audio = true;
            });
            RecordingControl::open(
                cx,
                target,
                crate::capture::overlay::ScreenRect {
                    x: 40,
                    y: 80,
                    width: 640,
                    height: 360,
                },
                None,
                (!area).then_some(1),
                (!area).then(|| "Fixture capture window with a long name".into()),
            );
            registry::handle(WindowKind::RecordingControl, cx).unwrap()
        });
        cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
            .unwrap();
        settle(cx, window);
        cx.update_window(window, |_, window, _| {
            let controls =
                super::GEOMETRY.with_borrow(|geometry| geometry["recording-controls"].clone());
            for bounds in &controls {
                assert!(
                    bounds.left() >= px(0.)
                        && bounds.right() <= window.viewport_size().width + px(0.001),
                    "recording control clipped: {bounds:?}"
                );
                assert!(
                    bounds.top() >= px(0.)
                        && bounds.bottom() <= window.viewport_size().height + px(0.001)
                );
            }
            assert!(controls
                .windows(2)
                .all(|pair| pair[0].right() <= pair[1].left()));
        })
        .unwrap();
        let prefix = if area {
            "record-control-area"
        } else {
            "record-control-window"
        };
        capture(cx, window, output, &format!("{prefix}-default"));
        if area {
            let image = cx.capture_screenshot(window).unwrap();
            let button = measured("recording-controls", 3);
            let bounds = measured("microphone-icon", 0);
            assert_eq!(bounds.size.width, px(crate::ui::chrome::TOOL_BUTTON_ICON));
            assert_eq!(bounds.size.height, px(crate::ui::chrome::TOOL_BUTTON_ICON));
            assert!(bounds.left() >= button.left() && bounds.right() <= button.right());
            assert!(bounds.top() >= button.top() && bounds.bottom() <= button.bottom());
            let x = (f32::from(bounds.left()) * scale).round() as u32;
            let y = (f32::from(bounds.top()) * scale).round() as u32;
            let luminance = |pixel: &image::Rgba<u8>| {
                let linear = |value: u8| {
                    let value = f32::from(value) / 255.;
                    if value <= 0.04045 {
                        value / 12.92
                    } else {
                        ((value + 0.055) / 1.055).powf(2.4)
                    }
                };
                0.2126 * linear(pixel[0]) + 0.7152 * linear(pixel[1]) + 0.0722 * linear(pixel[2])
            };
            let background = luminance(image.get_pixel(
                (f32::from(button.left()) * scale).round() as u32 + 2,
                (f32::from(button.top()) * scale).round() as u32 + 2,
            ));
            let mut visible = 0;
            for row in y..y + (f32::from(bounds.size.height) * scale).round() as u32 {
                for column in x..x + (f32::from(bounds.size.width) * scale).round() as u32 {
                    let foreground = luminance(image.get_pixel(column, row));
                    let contrast =
                        (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05);
                    if contrast >= 3. {
                        visible += 1;
                    }
                }
            }
            assert!(
                visible as f32 >= 8. * scale * scale,
                "the microphone glyph must contrast with the toolbar: {visible} visible pixels"
            );
            let mic = measured("recording-controls", 3).center();
            hover(cx, window, f32::from(mic.x), f32::from(mic.y));
            capture(cx, window, output, "record-control-microphone-hover");
            event(
                cx,
                window,
                PlatformInput::MouseDown(MouseDownEvent {
                    position: mic,
                    button: MouseButton::Left,
                    click_count: 1,
                    modifiers: Modifiers::none(),
                    first_mouse: false,
                }),
            );
            settle(cx, window);
            capture(cx, window, output, "record-control-microphone-pressed");
            let hovered = image::open(output.join("gpui-record-control-microphone-hover.png"))
                .unwrap()
                .to_rgba8();
            let pressed = image::open(output.join("gpui-record-control-microphone-pressed.png"))
                .unwrap()
                .to_rgba8();
            let width = (f32::from(bounds.size.width) * scale).round() as u32;
            let height = (f32::from(bounds.size.height) * scale).round() as u32;
            let hovered = image::imageops::crop_imm(&hovered, x, y, width, height).to_image();
            let pressed = image::imageops::crop_imm(&pressed, x, y, width, height).to_image();
            let changed = hovered
                .pixels()
                .zip(pressed.pixels())
                .filter(|(hovered, pressed)| {
                    hovered
                        .0
                        .iter()
                        .zip(pressed.0)
                        .any(|(hovered, pressed)| hovered.abs_diff(pressed) > 2)
                })
                .count();
            assert!(
                changed as f32 >= 4. * scale * scale,
                "the held microphone button must paint its pressed state"
            );
            event(
                cx,
                window,
                PlatformInput::MouseUp(gpui::MouseUpEvent {
                    position: mic,
                    button: MouseButton::Left,
                    click_count: 1,
                    modifiers: Modifiers::none(),
                }),
            );
            settle(cx, window);
            assert!(
                menu_painted(cx, window),
                "the microphone menu must be painted"
            );
            capture(cx, window, output, "record-control-microphone-open");
            escape(cx, window);
            key(cx, window, "tab");
            assert!(cx
                .update_window(window, |_, window, cx| window.focused(cx).is_some()
                    && herogpui::extend::focus_visible(cx))
                .unwrap());
            capture(cx, window, output, "record-control-keyboard-focused");
            click_measured(cx, window, "recording-controls", 0);
            capture(cx, window, output, "record-control-cancel-countdown");
            escape(cx, window);
            assert!(!cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
            continue;
        }
        for (index, name) in [(4, "camera"), (5, "microphone")] {
            click_measured(cx, window, "recording-controls", index);
            assert!(menu_painted(cx, window), "the {name} menu must be painted");
            capture(
                cx,
                window,
                output,
                &format!("record-control-{name}-configured-open"),
            );
            key(cx, window, "home");
            capture(
                cx,
                window,
                output,
                &format!("record-control-{name}-keyboard-focused"),
            );
            key(cx, window, "enter");
            settle(cx, window);
        }
        let configured = cx.update(|cx| {
            let recording = crate::state::state(cx).config.get().recording;
            serde_json::to_value(recording).unwrap()
        });
        capture(cx, window, output, "record-control-configured");
        click_measured(cx, window, "recording-controls", 2);
        capture(cx, window, output, "record-control-countdown");
        for _ in 0..3 {
            cx.advance_clock(std::time::Duration::from_secs(1));
            settle(cx, window);
        }
        assert_eq!(recorder::state(), RecorderState::Recording);
        capture(cx, window, output, "record-control-active");
        click_measured(cx, window, "recording-controls", 2);
        assert_eq!(recorder::state(), RecorderState::Paused);
        capture(cx, window, output, "record-control-paused");
        escape(cx, window);
        assert!(cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
        click_measured(cx, window, "recording-controls", 2);
        assert_eq!(recorder::state(), RecorderState::Recording);
        for (index, name) in [(7, "microphone"), (6, "camera-locked")] {
            click_measured(cx, window, "recording-controls", index);
            assert!(menu_painted(cx, window));
            capture(
                cx,
                window,
                output,
                &format!("record-control-{name}-active-open"),
            );
            key(cx, window, "home");
            key(cx, window, "enter");
            settle(cx, window);
        }
        click_measured(cx, window, "recording-controls", 8);
        cx.update(|cx| {
            assert_eq!(
                serde_json::to_value(crate::state::state(cx).config.get().recording).unwrap(),
                configured,
                "live toggles must keep the saved recording preferences"
            )
        });
        capture(cx, window, output, "record-control-muted");
        let projects: Vec<_> = std::fs::read_dir(&recordings)
            .unwrap()
            .map(|entry| entry.unwrap().path().canonicalize().unwrap())
            .collect();
        assert_eq!(projects.len(), 1);
        assert_eq!(
            projects[0].parent(),
            Some(recordings.canonicalize().unwrap().as_path())
        );
        click_measured(cx, window, "recording-controls", 10);
        assert_eq!(recorder::state(), RecorderState::Idle);
        assert!(!cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
        assert!(std::fs::read_dir(&recordings).unwrap().next().is_none());
    }
    let requests = std::fs::read_to_string(output.join("daemon-fixture.jsonl")).unwrap();
    let requests: Vec<serde_json::Value> = requests
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let started = requests
        .iter()
        .find(|request| request["module"] == "screen-recorder" && request["method"] == "start")
        .unwrap();
    assert_eq!(started["params"]["cameraEnabled"], true);
    assert_eq!(started["params"]["micEnabled"], true);
    for method in [
        "start",
        "pause",
        "resume",
        "setMicrophone",
        "setSystemAudio",
        "setCamera",
        "stop",
    ] {
        let calls: Vec<_> = requests
            .iter()
            .filter(|request| request["module"] == "screen-recorder" && request["method"] == method)
            .collect();
        assert_eq!(
            calls.len(),
            1,
            "the recording workflow must call {method} exactly once"
        );
        if method.starts_with("set") {
            assert_eq!(calls[0]["params"]["enabled"], false);
        }
    }
}
