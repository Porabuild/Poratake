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
                crate::windows::recording_control::PreferenceScope::Global,
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
            let bounds = measured("microphone-icon", 0);
            let x = (f32::from(bounds.left()) * scale).round() as u32;
            let y = (f32::from(bounds.top()) * scale).round() as u32;
            for (name, index) in [
                ("camera-icon", 2),
                ("microphone-icon", 3),
                ("system-audio-icon", 4),
                ("ios-device-icon", 5),
            ] {
                let button = measured("recording-controls", index);
                let icon = measured(name, 0);
                assert_eq!(icon.size.width, px(crate::ui::chrome::TOOL_BUTTON_ICON));
                assert_eq!(icon.size.height, px(crate::ui::chrome::TOOL_BUTTON_ICON));
                assert!(icon.left() >= button.left() && icon.right() <= button.right());
                assert!(icon.top() >= button.top() && icon.bottom() <= button.bottom());
                let background = image.get_pixel(
                    (f32::from(button.left()) * scale).round() as u32 + 2,
                    (f32::from(button.top()) * scale).round() as u32 + 2,
                );
                let mut visible = 0;
                let left = (f32::from(icon.left()) * scale).round() as u32;
                let top = (f32::from(icon.top()) * scale).round() as u32;
                for row in top..top + (f32::from(icon.size.height) * scale).round() as u32 {
                    for column in left..left + (f32::from(icon.size.width) * scale).round() as u32 {
                        let contrast =
                            super::pixel_contrast(image.get_pixel(column, row), background);
                        if contrast >= 3. {
                            visible += 1;
                        }
                    }
                }
                assert!(
                    visible as f32 >= 8. * scale * scale,
                    "{name} must contrast with the toolbar: {visible} visible pixels"
                );
            }
            let mic = measured("recording-controls", 3).center();
            hover(cx, window, f32::from(mic.x), f32::from(mic.y));
            #[cfg(any(windows, target_os = "macos"))]
            {
                let control = window.downcast::<RecordingControl>().unwrap();
                let tooltip = |cx: &mut HeadlessAppContext| {
                    cx.update(|cx| {
                        control
                            .update(cx, |control, _, _| control.parity_hovered_tooltip())
                            .unwrap()
                    })
                };
                assert_eq!(tooltip(cx).as_deref(), Some("Select microphone"));
            }
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
            #[cfg(any(windows, target_os = "macos"))]
            assert!(cx.update(|cx| {
                window
                    .downcast::<RecordingControl>()
                    .unwrap()
                    .update(cx, |control, _, _| {
                        control.parity_hovered_tooltip().is_none()
                    })
                    .unwrap()
            }));
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
            #[cfg(any(windows, target_os = "macos"))]
            let tooltip = |cx: &mut HeadlessAppContext| {
                let control = window.downcast::<RecordingControl>().unwrap();
                cx.update(|cx| {
                    control
                        .update(cx, |control, _, _| control.parity_hovered_tooltip())
                        .unwrap()
                })
            };
            #[cfg(any(windows, target_os = "macos"))]
            {
                let audio = measured("recording-controls", 4).center();
                hover(cx, window, f32::from(audio.x), f32::from(audio.y));
                assert_eq!(tooltip(cx).as_deref(), Some("Turn system sounds off"));
                hover(cx, window, 1., 1.);
                assert!(tooltip(cx).is_none());
                hover(cx, window, f32::from(mic.x), f32::from(mic.y));
                assert_eq!(tooltip(cx).as_deref(), Some("Select microphone"));
            }
            #[cfg(any(windows, target_os = "macos"))]
            let focus_tips = |cx: &mut HeadlessAppContext| {
                cx.update_window(window, |_, window, cx| {
                    let background =
                        gpui::Background::from(crate::theme::vars::active_theme(cx).overlay);
                    window
                        .painted_quads()
                        .into_iter()
                        .filter(|quad| {
                            let bounds = quad.bounds;
                            quad.background == background
                                && (31. ..=35.).contains(&(bounds.size.height.0 / scale))
                                && bounds.size.width.0 / scale > 35.
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap()
            };
            #[cfg(any(windows, target_os = "macos"))]
            {
                assert!(!cx.update(|cx| herogpui::extend::focus_visible(cx)));
                assert!(
                    focus_tips(cx).is_empty(),
                    "mouse hover must not open a keyboard tooltip"
                );
            }
            key(cx, window, "tab");
            assert!(cx
                .update_window(window, |_, window, cx| window.focused(cx).is_some()
                    && herogpui::extend::focus_visible(cx))
                .unwrap());
            #[cfg(any(windows, target_os = "macos"))]
            let assert_focus_tooltip = |cx: &mut HeadlessAppContext, index: usize, right: bool| {
                let control = measured("recording-controls", index);
                let tips = focus_tips(cx);
                assert_eq!(
                    tips.len(),
                    1,
                    "one keyboard tooltip must be painted: {tips:?}"
                );
                let bounds = tips[0].bounds;
                let mask = tips[0].content_mask.bounds;
                cx.update_window(window, |_, window, _| {
                    if right {
                        assert!(bounds.left().0 >= f32::from(control.right()) * scale - 1.);
                    } else {
                        assert!(bounds.right().0 <= f32::from(control.left()) * scale + 1.);
                    }
                    assert!(
                        bounds.left().0 >= 0.
                            && bounds.top().0 >= 0.
                            && bounds.right().0 <= f32::from(window.viewport_size().width) * scale
                            && bounds.bottom().0
                                <= f32::from(window.viewport_size().height) * scale
                            && mask.left().0 <= bounds.left().0
                            && mask.top().0 <= bounds.top().0
                            && mask.right().0 >= bounds.right().0
                            && mask.bottom().0 >= bounds.bottom().0,
                        "keyboard tooltip is clipped: {bounds:?}, mask {mask:?}"
                    );
                })
                .unwrap();
            };
            #[cfg(any(windows, target_os = "macos"))]
            assert_focus_tooltip(cx, 0, true);
            capture(cx, window, output, "record-control-keyboard-focused");
            #[cfg(any(windows, target_os = "macos"))]
            for (index, right) in [(2, true), (3, false), (4, false), (6, false)] {
                key(cx, window, "tab");
                assert_focus_tooltip(cx, index, right);
            }
            #[cfg(target_os = "macos")]
            {
                let focused = cx
                    .update_window(window, |_, window, cx| window.focused(cx))
                    .unwrap()
                    .expect("recording button should keep keyboard focus");
                let camera = measured("recording-controls", 2).center();
                hover(cx, window, f32::from(camera.x), f32::from(camera.y));
                assert_eq!(tooltip(cx).as_deref(), Some("Select camera"));
                assert!(
                    focus_tips(cx).is_empty(),
                    "hover should replace the keyboard tooltip"
                );
                cx.update_window(window, |_, window, cx| {
                    assert_eq!(window.focused(cx), Some(focused));
                    assert!(herogpui::extend::focus_visible(cx));
                })
                .unwrap();
                hover(cx, window, 1., 1.);
                assert!(tooltip(cx).is_none());
                assert_focus_tooltip(cx, 6, false);

                click_measured(cx, window, "recording-controls", 1);
                settle(cx, window);
                let width = cx.update(|cx| {
                    let handle = window.downcast::<RecordingControl>().unwrap();
                    handle
                        .update(cx, |control, window, _| {
                            (
                                control.parity_bar_width().unwrap(),
                                f32::from(window.viewport_size().width),
                            )
                        })
                        .unwrap()
                });
                assert!(
                    (width.0 - width.1).abs() <= 0.5,
                    "toolbar did not shrink after pointer input: {width:?}"
                );
            }
            click_measured(cx, window, "recording-controls", 0);
            capture(cx, window, output, "record-control-cancel-countdown");
            escape(cx, window);
            assert!(!cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
            continue;
        }
        #[cfg(any(windows, target_os = "macos"))]
        {
            let target = measured("recording-controls", 0).center();
            hover(cx, window, f32::from(target.x), f32::from(target.y));
            assert_eq!(
                cx.update(|cx| window
                    .downcast::<RecordingControl>()
                    .unwrap()
                    .update(cx, |control, _, _| control.parity_hovered_tooltip())
                    .unwrap())
                    .as_deref(),
                Some("Fixture capture window with a long name")
            );
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
            assert!(recording.mic_enabled);
            assert!(recording.camera.enabled);
            assert!(recording.system_audio);
            serde_json::to_value(recording).unwrap()
        });
        capture(cx, window, output, "record-control-configured");
        click_measured(cx, window, "recording-controls", 6);
        assert!(!cx.update(|cx| crate::state::state(cx).config.get().recording.system_audio));
        click_measured(cx, window, "recording-controls", 6);
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
        cx.update(|cx| {
            let daemon = crate::state::state(cx).daemon;
            daemon.emit_parity_event(
                "camera-preview:position-changed",
                &serde_json::json!({"x": -315, "y": 240}),
            );
            daemon.emit_parity_event(
                "camera-preview:position-changed",
                &serde_json::json!({"x": "invalid", "y": 1}),
            );
        });
        let fixture_path = output.join("capture-fixture.json");
        let previous_fixture = std::fs::read(&fixture_path).ok();
        std::fs::write(
            &fixture_path,
            serde_json::to_vec(&serde_json::json!({"recordingCameraDisableFails": true})).unwrap(),
        )
        .unwrap();
        let before_failed_toggle = super::daemon_fixture::calls(output).len();
        click_measured(cx, window, "recording-controls", 6);
        key(cx, window, "home");
        key(cx, window, "enter");
        settle(cx, window);
        let calls = super::daemon_fixture::calls(output);
        let failed_toggle = &calls[before_failed_toggle..];
        assert!(failed_toggle
            .iter()
            .any(|call| { call["module"] == "screen-recorder" && call["method"] == "setCamera" }));
        assert!(
            !failed_toggle
                .iter()
                .any(|call| { call["module"] == "camera-preview" && call["method"] == "hide" }),
            "a rejected camera disable must leave its preview visible"
        );
        if let Some(contents) = previous_fixture {
            std::fs::write(&fixture_path, contents).unwrap();
        } else {
            std::fs::remove_file(&fixture_path).unwrap();
        }
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
            if name == "camera-locked" {
                cx.update(|cx| {
                    crate::state::state(cx).daemon.emit_parity_event(
                        "camera-preview:position-changed",
                        &serde_json::json!({"x": 700, "y": 800}),
                    );
                });
            }
        }
        for _ in 0..2 {
            click_measured(cx, window, "recording-controls", 6);
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
    #[cfg(any(windows, target_os = "macos"))]
    {
        let start = requests
            .iter()
            .position(|request| {
                request["module"] == "screen-recorder" && request["method"] == "start"
            })
            .unwrap();
        let protected = requests[..start]
            .iter()
            .rposition(|request| {
                request["module"] == "camera-preview"
                    && request["method"] == "setContentProtection"
                    && request["params"]["enabled"] == true
            })
            .expect("camera preview must be protected before recording starts");
        let stop = requests
            .iter()
            .position(|request| {
                request["module"] == "screen-recorder" && request["method"] == "stop"
            })
            .unwrap();
        assert!(!requests[protected + 1..stop].iter().any(|request| {
            request["module"] == "camera-preview"
                && request["method"] == "setContentProtection"
                && request["params"]["enabled"] == false
        }));
    }
    let camera_calls: Vec<_> = requests
        .iter()
        .enumerate()
        .filter(|(_, request)| {
            request["module"] == "screen-recorder" && request["method"] == "setCamera"
        })
        .map(|(index, _)| index)
        .collect();
    assert_eq!(camera_calls.len(), 4);
    assert_eq!(
        camera_calls
            .iter()
            .map(|index| requests[*index]["params"]["enabled"].as_bool().unwrap())
            .collect::<Vec<_>>(),
        [false, false, true, false]
    );
    let stop = requests
        .iter()
        .position(|request| request["module"] == "screen-recorder" && request["method"] == "stop")
        .unwrap();
    assert!(requests[camera_calls[1] + 1..stop]
        .iter()
        .any(|request| { request["module"] == "camera-preview" && request["method"] == "hide" }));
    assert!(
        requests[camera_calls[1] + 1..camera_calls[2]]
            .iter()
            .any(|request| {
                request["module"] == "camera-preview"
                    && request["method"] == "show"
                    && request["params"]["x"] == -315
                    && request["params"]["y"] == 240
            }),
        "re-enabling the camera must restore its dragged position"
    );
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
        let expected = if method == "setCamera" { 4 } else { 1 };
        assert_eq!(calls.len(), expected, "unexpected {method} call count");
        if method.starts_with("set") && method != "setCamera" {
            assert_eq!(calls[0]["params"]["enabled"], false);
        }
    }

    #[cfg(any(windows, target_os = "macos"))]
    {
        let fixture_path = output.join("capture-fixture.json");
        let previous_fixture = std::fs::read(&fixture_path).ok();
        std::fs::write(
            &fixture_path,
            serde_json::to_vec(&serde_json::json!({"recordingProtectionFails": true})).unwrap(),
        )
        .unwrap();
        let window = cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.recording.start_delay = 0.;
                config.recording.camera.enabled = false;
            });
            RecordingControl::open(
                cx,
                RecordingTarget::Window,
                crate::capture::overlay::ScreenRect {
                    x: 40,
                    y: 80,
                    width: 640,
                    height: 360,
                },
                None,
                Some(1),
                Some("Fixture capture window".into()),
                crate::windows::recording_control::PreferenceScope::Global,
            );
            registry::handle(WindowKind::RecordingControl, cx).unwrap()
        });
        settle(cx, window);
        let before = super::daemon_fixture::calls(output).len();
        click_measured(cx, window, "recording-controls", 2);
        settle(cx, window);
        let calls = super::daemon_fixture::calls(output);
        let calls = &calls[before..];
        let protection: Vec<_> = calls
            .iter()
            .filter(|call| {
                call["module"] == "camera-preview" && call["method"] == "setContentProtection"
            })
            .map(|call| call["params"]["enabled"].as_bool().unwrap())
            .collect();
        assert_eq!(protection, [true, false]);
        assert!(!calls
            .iter()
            .any(|call| { call["module"] == "screen-recorder" && call["method"] == "start" }));
        assert_eq!(recorder::state(), RecorderState::Idle);
        assert!(cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
        assert!(std::fs::read_dir(&recordings).unwrap().next().is_none());
        escape(cx, window);
        if let Some(contents) = previous_fixture {
            std::fs::write(&fixture_path, contents).unwrap();
        } else {
            std::fs::remove_file(&fixture_path).unwrap();
        }
    }

    for remember in [true, false] {
        let window = cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.recording.system_audio = false;
                config.recording.mic_enabled = false;
                config.recording.camera.enabled = false;
                config.all_in_one.remember_choices = remember;
                config.all_in_one.recording.system_audio = remember;
                config.all_in_one.recording.mic_enabled = !remember;
                config.all_in_one.recording.camera_enabled = !remember;
            });
            RecordingControl::open(
                cx,
                RecordingTarget::Area,
                crate::capture::overlay::ScreenRect {
                    x: 40,
                    y: 80,
                    width: 640,
                    height: 360,
                },
                None,
                None,
                None,
                crate::windows::recording_control::PreferenceScope::AllInOne,
            );
            registry::handle(WindowKind::RecordingControl, cx).unwrap()
        });
        settle(cx, window);
        for index in [2, 3] {
            click_measured(cx, window, "recording-controls", index);
            key(cx, window, "home");
            key(cx, window, "enter");
            settle(cx, window);
        }
        click_measured(cx, window, "recording-controls", 4);
        cx.update(|cx| {
            let config = crate::state::state(cx).config.get();
            assert!(!config.recording.system_audio);
            assert!(!config.recording.mic_enabled);
            assert!(!config.recording.camera.enabled);
            assert!(!config.all_in_one.recording.system_audio);
            assert!(config.all_in_one.recording.mic_enabled);
            assert!(config.all_in_one.recording.camera_enabled);
        });
        let hides_before = std::fs::read_to_string(output.join("daemon-fixture.jsonl"))
            .unwrap()
            .lines()
            .filter(|line| {
                line.contains("\"module\":\"camera-preview\"")
                    && line.contains("\"method\":\"hide\"")
            })
            .count();
        let close_index =
            super::GEOMETRY.with_borrow(|geometry| geometry["recording-controls"].len() - 1);
        click_measured(cx, window, "recording-controls", close_index);
        assert!(!cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
        let hides_after = std::fs::read_to_string(output.join("daemon-fixture.jsonl"))
            .unwrap()
            .lines()
            .filter(|line| {
                line.contains("\"module\":\"camera-preview\"")
                    && line.contains("\"method\":\"hide\"")
            })
            .count();
        assert_eq!(hides_after, hides_before + 1);
    }
}
