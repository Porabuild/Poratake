use gpui::{
    AppContext, KeyDownEvent, KeyUpEvent, Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PlatformInput,
};
use herogpui::gpui;
use poratake_daemon_common::geometry::CaptureRect;

use crate::editor::window::EditorWindow;
use crate::history_store::{HistoryItem, HistoryItemType};

use super::{
    capture, click_measured, event, hover, point, px, settle, size, take_notifications,
    HeadlessAppContext, Modifiers, Path,
};

#[cfg(any(windows, target_os = "macos"))]
use super::click;

#[cfg(windows)]
struct DuplicateDisplayTarget;

#[cfg(windows)]
impl DuplicateDisplayTarget {
    fn enable() -> Self {
        crate::capture::DUPLICATE_DISPLAY_FOR_PARITY
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Self
    }
}

#[cfg(windows)]
impl Drop for DuplicateDisplayTarget {
    fn drop(&mut self) {
        crate::capture::DUPLICATE_DISPLAY_FOR_PARITY
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(windows)]
fn select_all_in_one_mode(
    cx: &mut HeadlessAppContext,
    window: gpui::AnyWindowHandle,
    mode: crate::capture::all_in_one::Mode,
) {
    let modes = super::measured("all-in-one-controls", 0);
    let x = f32::from(modes.left())
        + f32::from(modes.size.width)
            * if mode == crate::capture::all_in_one::Mode::Record {
                0.75
            } else {
                0.25
            };
    click(cx, window, x, f32::from(modes.center().y));
}

pub(super) const SELECTED_RECT: CaptureRect = CaptureRect {
    x: 80,
    y: 90,
    width: 640,
    height: 360,
};

pub(super) fn editor_sizes(cx: &mut HeadlessAppContext, scratch: &Path) {
    let mut expected = None;
    for (index, width, height) in [(0, 1800, 900), (1, 320, 240), (2, 2200, 1200)] {
        let path = scratch.join(format!("editor-size-{index}.png"));
        image::RgbaImage::from_pixel(width, height, image::Rgba([64, 112, 192, 255]))
            .save(&path)
            .unwrap();
        assert!(cx.update(|cx| crate::open_editor_for(cx, path.to_str().unwrap())));
        let editor = cx.update(|cx| {
            cx.windows()
                .into_iter()
                .find(|handle| handle.downcast::<EditorWindow>().is_some())
                .unwrap()
        });
        settle(cx, editor);
        let viewport = cx
            .update_window(editor, |_, window, _| window.viewport_size())
            .unwrap();
        let actual = (f32::from(viewport.width), f32::from(viewport.height));
        assert_eq!(
            crate::config::window_state::get(crate::config::window_state::SCREENSHOT_EDITOR),
            Some(actual),
            "the first editor size must be remembered without requiring a resize"
        );
        if let Some(expected) = expected {
            assert_eq!(
                actual, expected,
                "opening another image must retain the editor size"
            );
        }
        expected = Some(actual);
        if index == 1 {
            cx.update_window(editor, |_, window, _| {
                window.resize(size(px(1200.), px(800.)))
            })
            .unwrap();
            settle(cx, editor);
            expected = Some((1200., 800.));
        }
        cx.update_window(editor, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
    }
    println!("editor initial and resized dimensions survive reopening different image sizes");
}

pub(super) fn area_picker(cx: &mut HeadlessAppContext) -> gpui::AnyWindowHandle {
    let picker = cx.update(|cx| crate::capture::overlay::parity_active_overlay(cx).unwrap());
    settle(cx, picker);
    picker
}

pub(super) fn select_area(cx: &mut HeadlessAppContext, picker: gpui::AnyWindowHandle) {
    select_area_rect(cx, picker, SELECTED_RECT);
}

fn select_area_rect(cx: &mut HeadlessAppContext, picker: gpui::AnyWindowHandle, rect: CaptureRect) {
    let scale = cx
        .update_window(picker, |_, window, _| window.scale_factor())
        .unwrap();
    assert_eq!(scale, 2.);
    let scale = crate::capture::overlay::capture_coordinate_scale(scale);
    let start = point(px(rect.x as f32 / scale), px(rect.y as f32 / scale));
    let end = point(
        px((rect.x + rect.width) as f32 / scale),
        px((rect.y + rect.height) as f32 / scale),
    );
    let drag = point(
        px((rect.x + rect.width - 20) as f32 / scale),
        px((rect.y + rect.height - 20) as f32 / scale),
    );
    for input in [
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
        PlatformInput::MouseMove(MouseMoveEvent {
            position: drag,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        }),
        PlatformInput::MouseUp(MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    ] {
        event(cx, picker, input);
    }
}

#[cfg(windows)]
fn wait_for_icon_restoration(cx: &mut HeadlessAppContext) {
    let started = std::time::Instant::now();
    loop {
        cx.run_until_parked();
        if crate::capture::desktop_icons::pending_transitions().is_empty() {
            return;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "desktop icon restoration did not settle"
        );
        std::thread::yield_now();
    }
}

#[cfg(target_os = "macos")]
fn configure_recording_fixture(fixture: &mut serde_json::Value, target: &str) -> f64 {
    let source = std::env::var_os(format!(
        "PORATAKE_PARITY_{}_RECORDING",
        target.to_uppercase()
    ));
    let duration = source.as_ref().map_or(2.0, |source| {
        let path = Path::new(source);
        let info = crate::video::decoder::VideoDecoder::open(path)
            .expect("open supplied recording")
            .info();
        assert!(info.is_valid() && info.duration > 1.5);
        info.duration
    });
    let prefix = match target {
        "area" => "recordingArea",
        "window" => "recordingWindow",
        "screen" => "recordingScreen",
        _ => panic!("invalid recording fixture target"),
    };
    fixture[format!("{prefix}FixtureVideo")] = true.into();
    fixture[format!("{prefix}Duration")] = duration.into();
    if let Some(source) = source {
        fixture[format!("{prefix}Source")] = source.to_str().expect("UTF-8 fixture path").into();
    }
    duration
}

#[cfg(target_os = "macos")]
fn set_recording_fixture_scale(cx: &mut HeadlessAppContext, window: gpui::AnyWindowHandle) {
    let Some(scale) = std::env::var("PORATAKE_PARITY_VIDEO_DPI")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
    else {
        return;
    };
    assert!(scale.is_finite() && scale > 0.);
    cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
        .unwrap();
}

#[cfg(target_os = "macos")]
fn recorded_editor_and_export(
    cx: &mut HeadlessAppContext,
    project: &Path,
    output: &Path,
    target: &str,
) {
    use crate::windows::registry::{self, WindowKind};
    use crate::windows::video_editor::{model, preview, sidebar::SidebarTab, VideoEditorWindow};

    if let Some(source) = std::env::var_os(format!(
        "PORATAKE_PARITY_{}_RECORDING",
        target.to_uppercase()
    )) {
        let expected = crate::video::decoder::VideoDecoder::open(Path::new(&source))
            .unwrap()
            .frame_at(1.)
            .unwrap();
        let actual = crate::video::decoder::VideoDecoder::open(
            &project.join(crate::video::project::RECORDING),
        )
        .unwrap()
        .frame_at(1.)
        .unwrap();
        assert!(
            actual.bgra == expected.bgra,
            "{target} project did not retain its supplied source frame"
        );
    }

    let video = cx.update(|cx| {
        registry::handle(WindowKind::VideoEditor, cx)
            .unwrap()
            .downcast::<VideoEditorWindow>()
            .unwrap()
    });
    set_recording_fixture_scale(cx, video.into());
    let started = std::time::Instant::now();
    loop {
        cx.run_until_parked();
        if cx.update(|cx| video.read(cx).unwrap().preview_image().is_some()) {
            break;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "{target} recording preview did not load"
        );
        std::thread::yield_now();
    }
    cx.update(|cx| {
        video
            .update(cx, |editor, _, cx| editor.set_timeline_zoom(100.0, cx))
            .unwrap()
    });
    let window = video.into();
    settle(cx, window);
    let lane = super::measured("video-timeline-lanes", 1);
    let lane_y = f32::from(lane.size.height) / 2.0;
    drag_rectangle(cx, window, lane.origin, (25.0, lane_y), (150.0, lane_y));
    settle(cx, window);
    cx.update(|cx| {
        video
            .update(cx, |editor, _, cx| {
                editor.set_zoom_target_mode("manual".into(), cx);
                editor.set_zoom_focus_point(0.7, 0.4, cx);
                editor.update_zoom_settings(cx, |settings| {
                    settings.transition_in_duration = 0.0;
                    settings.transition_out_duration = 0.0;
                });
                editor.update_export(cx, |settings| settings.resolution = "original".into());
                editor.set_playhead(1.0, cx);
            })
            .unwrap()
    });
    cx.advance_clock(std::time::Duration::from_secs(1));
    settle(cx, window);
    cx.update(|cx| {
        video
            .update(cx, |editor, _, cx| {
                editor.activate_tab(SidebarTab::Drawing, cx);
                editor.update_drawing_tools(cx, |tools| {
                    tools.active_tool = "rectangle".into();
                    tools.selected_color = "#ff0000".into();
                    tools.shape_fill_mode = "filled".into();
                });
            })
            .unwrap();
    });
    settle(cx, window);
    let content = cx.update(|cx| {
        video
            .read(cx)
            .unwrap()
            .preview_content_rect()
            .expect("recording preview bounds")
    });
    drag_rectangle(
        cx,
        window,
        point(px(content.x), px(content.y)),
        (content.width * 0.2, content.height * 0.2),
        (content.width * 0.4, content.height * 0.4),
    );
    settle(cx, window);
    let redaction = std::env::var_os("PORATAKE_PARITY_VIDEO_REDACTION").is_some();
    if redaction {
        cx.update(|cx| {
            video
                .update(cx, |editor, _, cx| {
                    editor.update_drawing_tools(cx, |tools| {
                        tools.active_tool = "redact".into();
                        tools.redact_style = "pixelate".into();
                        tools.redact_intensity = 8.0;
                    });
                })
                .unwrap();
        });
        settle(cx, window);
        drag_rectangle(
            cx,
            window,
            point(px(content.x), px(content.y)),
            (content.width * 0.35, content.height * 0.35),
            (content.width * 0.50, content.height * 0.50),
        );
        settle(cx, window);
    }
    cx.advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    if std::env::var_os("PORATAKE_PARITY_DISTINCT_RECORDINGS").is_some() {
        use crate::windows::video_editor::timeline::tracks::TrackKind;

        cx.update(|cx| {
            video
                .update(cx, |editor, _, cx| {
                    editor.cut_at(0.75, Some(TrackKind::Video), cx);
                    editor.cut_at(1.0, Some(TrackKind::Video), cx);
                })
                .unwrap();
        });
        cx.advance_clock(std::time::Duration::from_secs(1));
        cx.run_until_parked();
        let cut = model::load_state(project);
        assert_eq!(cut.segments.len(), 3);
        let removed = cut.segments[1].id.clone();
        cx.update(|cx| {
            video
                .update(cx, |editor, _, cx| {
                    editor.delete_clip(TrackKind::Video, removed.into(), cx);
                    editor.set_playhead(1.0, cx);
                })
                .unwrap();
        });
        cx.advance_clock(std::time::Duration::from_secs(1));
        settle(cx, window);
    }
    let state = model::load_state(project);
    assert_eq!(state.zoom_segments.len(), 1);
    if redaction {
        assert!(matches!(
            state.drawing_segments.as_slice(),
            [model::DrawingSegment { annotations: first, .. }, model::DrawingSegment { annotations: second, .. }]
                if matches!(first.as_slice(), [crate::editor::annotations::Annotation::Rectangle { .. }])
                && matches!(second.as_slice(), [crate::editor::annotations::Annotation::Redact { style, .. }] if style == "pixelate")
        ));
    } else {
        assert!(matches!(
            state.drawing_segments.as_slice(),
            [model::DrawingSegment { annotations, .. }]
                if matches!(annotations.as_slice(), [crate::editor::annotations::Annotation::Rectangle { .. }])
        ));
    }
    assert!((state.zoom_segments[0].start_time - 0.25).abs() < 0.02);
    assert!((state.zoom_segments[0].end_time - 1.5).abs() < 0.02);
    if std::env::var_os("PORATAKE_PARITY_DISTINCT_RECORDINGS").is_some() {
        assert_eq!(state.segments.len(), 2);
        assert!((state.segments[0].original_end - 0.75).abs() < 0.001);
        assert!((state.segments[1].original_start - 1.0).abs() < 0.001);
    }
    let mut preview = preview::Source::open(project, state.clone()).unwrap();
    let expected = preview::to_render_image_opaque(&preview.compose(1.0, None).unwrap()).unwrap();
    cx.update(|cx| {
        registry::close(WindowKind::VideoEditor, cx);
        VideoEditorWindow::open(cx, Some(project.to_string_lossy().into_owned()));
    });
    let video = cx.update(|cx| {
        registry::handle(WindowKind::VideoEditor, cx)
            .unwrap_or_else(|| {
                panic!(
                    "reopen of {target} recording failed: {:?}",
                    take_notifications()
                )
            })
            .downcast::<VideoEditorWindow>()
            .unwrap()
    });
    set_recording_fixture_scale(cx, video.into());
    let started = std::time::Instant::now();
    loop {
        cx.run_until_parked();
        if cx.update(|cx| video.read(cx).unwrap().preview_image().is_some()) {
            break;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "reopened {target} recording preview did not load"
        );
        std::thread::yield_now();
    }
    cx.update(|cx| {
        video
            .update(cx, |editor, _, cx| editor.set_playhead(1.0, cx))
            .unwrap()
    });
    cx.advance_clock(std::time::Duration::from_secs(1));
    let started = std::time::Instant::now();
    loop {
        cx.run_until_parked();
        let matches = cx.update(|cx| {
            video
                .read(cx)
                .unwrap()
                .preview_image()
                .is_some_and(|actual| actual.as_bytes(0) == expected.as_bytes(0))
        });
        if matches {
            break;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "edited {target} preview did not converge"
        );
        std::thread::yield_now();
    }
    let mut without_zoom = state.clone();
    without_zoom.zoom_segments.clear();
    let mut baseline = preview::Source::open(project, without_zoom).unwrap();
    let baseline = preview::to_render_image_opaque(&baseline.compose(1.0, None).unwrap()).unwrap();
    let mut without_drawing = state.clone();
    without_drawing.drawing_segments.clear();
    let mut undrawn = preview::Source::open(project, without_drawing).unwrap();
    let undrawn = preview::to_render_image_opaque(&undrawn.compose(1.0, None).unwrap()).unwrap();
    let drawing_changes = expected
        .as_bytes(0)
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(undrawn.as_bytes(0).unwrap().as_chunks::<4>().0.iter())
        .filter(|(edited, original)| {
            edited[..3]
                .iter()
                .zip(&original[..3])
                .any(|(edited, original)| edited.abs_diff(*original) >= 16)
        })
        .count();
    assert!(
        drawing_changes > 20,
        "{target} drawing did not change the preview"
    );
    assert_ne!(
        expected.as_bytes(0),
        baseline.as_bytes(0),
        "zoom must affect the preview"
    );
    let changed = expected
        .as_bytes(0)
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(baseline.as_bytes(0).unwrap().as_chunks::<4>().0.iter())
        .filter(|(edited, original)| {
            edited[..3]
                .iter()
                .zip(&original[..3])
                .any(|(edited, original)| edited.abs_diff(*original) >= 16)
        })
        .count();
    eprintln!("fixture {target} zoom changes {changed} preview pixels by at least 16 levels");
    let window = video.into();
    settle(cx, window);
    capture(
        cx,
        window,
        output,
        &format!("recording-{target}-edited-preview"),
    );
    let screenshot = cx.capture_screenshot(window).unwrap();
    let content = cx.update(|cx| video.read(cx).unwrap().preview_content_rect().unwrap());
    let display_scale = cx
        .update_window(window, |_, window, _| window.scale_factor())
        .unwrap();
    let scaled_preview = redaction.then(|| {
        super::recording_redaction::capture_scaled(cx, video, project, &state, output, target)
    });
    let exported = output.join(format!("recording-{target}-edited.mp4"));
    crate::video::export::run(
        crate::video::export::Request {
            project: project.to_path_buf(),
            output: exported.clone(),
            state,
        },
        &mut |_| {},
        &|| false,
    )
    .unwrap();
    let decoder = crate::video::decoder::VideoDecoder::open(&exported).unwrap();
    for time in [0.5, 1.0, 1.5] {
        let expected =
            preview::to_render_image_opaque(&preview.compose(time, None).unwrap()).unwrap();
        let expected = expected.as_bytes(0).unwrap();
        let decoded = decoder.frame_at(time).unwrap();
        assert_eq!(decoded.bgra.len(), expected.len());
        let error: u64 = decoded
            .bgra
            .iter()
            .zip(expected)
            .map(|(actual, expected)| u64::from(actual.abs_diff(*expected)))
            .sum();
        let mean = error as f64 / expected.len() as f64;
        assert!(
            mean < 3.0,
            "{target} preview and decoded export differ at {time}s: {mean}"
        );
        if time == 1.0 {
            if let Some(scaled) = &scaled_preview {
                scaled.compare_export(&decoded, target);
                if target == "area" {
                    let unencoded = crate::video::decoder::DecodedFrame {
                        width: decoded.width,
                        height: decoded.height,
                        bgra: std::sync::Arc::from(expected),
                    };
                    super::recording_preview_comparison::save_frame(
                        &unencoded,
                        &output.join("recording-area-redaction-composed.png"),
                    );
                    super::recording_preview_comparison::save_frame(
                        &decoded,
                        &output.join("recording-area-redaction-decoded.png"),
                    );
                    std::fs::write(
                        output.join("recording-area-redaction-region.json"),
                        serde_json::to_vec(&scaled.region()).unwrap(),
                    )
                    .unwrap();
                }
            }
            let rendered = super::recording_preview_comparison::compare(
                &screenshot,
                content,
                display_scale,
                &decoded,
            );
            assert!(
                rendered.mean < 5.,
                "{target} rendered preview differs from decoded export: {}",
                rendered.mean
            );
            let unencoded = crate::video::decoder::DecodedFrame {
                width: decoded.width,
                height: decoded.height,
                bgra: std::sync::Arc::from(expected),
            };
            let displayed = super::recording_preview_comparison::compare(
                &screenshot,
                content,
                display_scale,
                &unencoded,
            );
            let at_display_size = super::recording_preview_comparison::compare_at_display_size(
                &screenshot,
                content,
                display_scale,
                &unencoded,
            );
            if (content.width * display_scale - decoded.width as f32).abs() < 0.01
                && (content.height * display_scale - decoded.height as f32).abs() < 0.01
            {
                assert!(
                    at_display_size.maximum <= 2,
                    "{target} one-to-one preview differs from its composition by {} levels",
                    at_display_size.maximum
                );
            }
            eprintln!(
                "fixture {target} rendered preview/export mean RGB error: {:.3}, channels above 12: {}, above 32: {}, maximum: {}",
                rendered.mean, rendered.above_twelve, rendered.above_thirty_two, rendered.maximum
            );
            eprintln!(
                "fixture {target} rendered preview/composition mean RGB error: {:.3}, channels above 12: {}, above 32: {}, maximum: {}",
                displayed.mean, displayed.above_twelve, displayed.above_thirty_two, displayed.maximum
            );
            eprintln!(
                "fixture {target} display-size preview/composition mean RGB error: {:.3}, channels above 12: {}, above 32: {}, maximum: {}",
                at_display_size.mean, at_display_size.above_twelve, at_display_size.above_thirty_two, at_display_size.maximum
            );
            let undrawn = undrawn.as_bytes(0).unwrap();
            let mut drawing_pixels = 0_u64;
            let mut drawing_error = 0_u64;
            let mut undrawn_error = 0_u64;
            for ((edited, original), actual) in expected
                .as_chunks::<4>()
                .0
                .iter()
                .zip(undrawn.as_chunks::<4>().0.iter())
                .zip(decoded.bgra.as_chunks::<4>().0.iter())
            {
                if !(0..3).any(|channel| edited[channel].abs_diff(original[channel]) >= 16) {
                    continue;
                }
                drawing_pixels += 1;
                for channel in 0..3 {
                    drawing_error += u64::from(edited[channel].abs_diff(actual[channel]));
                    undrawn_error += u64::from(original[channel].abs_diff(actual[channel]));
                }
            }
            assert!(drawing_pixels > 20);
            assert!(
                drawing_error < undrawn_error,
                "{target} export does not contain the drawing: {drawing_error} vs {undrawn_error}"
            );
            eprintln!(
                "fixture {target} drawing export error: edited {drawing_error}, undrawn {undrawn_error} across {drawing_pixels} pixels"
            );
            let original = baseline.as_bytes(0).unwrap();
            let mut changed_pixels = 0;
            let mut edited_error = 0_u64;
            let mut original_error = 0_u64;
            for ((edited, original), actual) in expected
                .as_chunks::<4>()
                .0
                .iter()
                .zip(original.as_chunks::<4>().0.iter())
                .zip(decoded.bgra.as_chunks::<4>().0.iter())
            {
                if !(0..3).any(|channel| edited[channel].abs_diff(original[channel]) >= 16) {
                    continue;
                }
                changed_pixels += 1;
                for channel in 0..3 {
                    edited_error += u64::from(edited[channel].abs_diff(actual[channel]));
                    original_error += u64::from(original[channel].abs_diff(actual[channel]));
                }
            }
            assert!(
                changed_pixels > 0,
                "{target} zoom has no clear visual effect"
            );
            assert!(
                edited_error < original_error,
                "{target} export does not favor the edited preview: {edited_error} vs {original_error}"
            );
            eprintln!(
                "fixture {target} changed-pixel export error: edited {edited_error}, unzoomed {original_error} across {changed_pixels} pixels"
            );
        }
        eprintln!(
            "fixture {target} recording preview/export mean channel error at {time}s: {mean:.3}"
        );
    }
    cx.update(|cx| registry::close(WindowKind::VideoEditor, cx));
}

#[cfg(target_os = "macos")]
fn wait_for_recording_start(cx: &mut HeadlessAppContext) {
    let started = std::time::Instant::now();
    while crate::video::recorder::state() == crate::video::recorder::RecorderState::Starting {
        cx.run_until_parked();
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        std::thread::yield_now();
    }
}

#[cfg(target_os = "macos")]
fn wait_for_recording_stop(cx: &mut HeadlessAppContext) {
    let started = std::time::Instant::now();
    while crate::video::recorder::state() == crate::video::recorder::RecorderState::Stopping {
        cx.run_until_parked();
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        std::thread::yield_now();
    }
}

#[cfg(target_os = "macos")]
fn recorded_screen_handoff(
    cx: &mut HeadlessAppContext,
    control: gpui::AnyWindowHandle,
    output: &Path,
) {
    use crate::windows::registry::{self, WindowKind};

    let recordings = output.join("recording-screen-handoff");
    std::fs::create_dir_all(&recordings).unwrap();
    let previous = cx.update(|cx| {
        let config = crate::state::state(cx).config.get();
        (
            config.storage.recordings_path,
            config.recording.start_delay,
            config.recording.show_preview,
            config.history.enabled,
        )
    });
    let mut fixture = serde_json::json!({"freeze": "live"});
    let duration = configure_recording_fixture(&mut fixture, "screen");
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&fixture).unwrap(),
    )
    .unwrap();
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.recordings_path = recordings.to_string_lossy().into_owned();
            config.recording.start_delay = 0.0;
            config.recording.show_preview = false;
            config.history.enabled = true;
        })
    });
    let before = super::daemon_fixture::calls(output).len();
    click_measured(cx, control, "recording-controls", 0);
    settle(cx, control);
    wait_for_recording_start(cx);
    assert_eq!(
        crate::video::recorder::state(),
        crate::video::recorder::RecorderState::Recording
    );
    cx.advance_clock(std::time::Duration::from_secs(2));
    settle(cx, control);
    click_measured(cx, control, "recording-controls", 1);
    cx.run_until_parked();
    wait_for_recording_stop(cx);
    assert!(!cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
    let calls = super::daemon_fixture::calls(output);
    let recording = Path::new(
        calls[before..]
            .iter()
            .find(|call| call["module"] == "screen-recorder" && call["method"] == "start")
            .unwrap()["params"]["outputPath"]
            .as_str()
            .unwrap(),
    );
    let project = recording.parent().unwrap();
    assert!(recording.is_file());
    assert_eq!(
        crate::windows::video_editor::model::load_state(project).source_duration,
        Some(duration)
    );
    assert_eq!(
        crate::history_store::load_history()
            .iter()
            .find(|item| Path::new(&item.original_path) == project)
            .unwrap()
            .duration,
        Some(duration)
    );
    recorded_editor_and_export(cx, project, output, "screen");
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.recordings_path = previous.0;
            config.recording.start_delay = previous.1;
            config.recording.show_preview = previous.2;
            config.history.enabled = previous.3;
        })
    });
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({"freeze": "frozen"})).unwrap(),
    )
    .unwrap();
}

#[cfg(target_os = "macos")]
fn recording_failure_preserves_project(
    cx: &mut HeadlessAppContext,
    output: &Path,
    on_stop: bool,
    queued: bool,
    during_stop: bool,
) {
    use crate::video::recorder::RecordingTarget;
    use crate::windows::recording_control::{PreferenceScope, RecordingControl};
    use crate::windows::registry::{self, WindowKind};

    let recordings = output.join(if during_stop {
        "recording-terminal-error-during-stop"
    } else if queued {
        "recording-queued-terminal-failure"
    } else if on_stop {
        "recording-stop-audio-failure"
    } else {
        "recording-audio-failure"
    });
    std::fs::create_dir_all(&recordings).unwrap();
    let mut fixture = serde_json::json!({
        "freeze": "live",
        "recordingAudioFailure": !on_stop && !queued && !during_stop,
        "recordingStopAudioFailure": on_stop,
        "recordingErrorBeforeStopSuccess": during_stop,
    });
    configure_recording_fixture(&mut fixture, if queued { "window" } else { "area" });
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&fixture).unwrap(),
    )
    .unwrap();
    let previous = cx.update(|cx| crate::state::state(cx).config.get());
    let control = cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.recordings_path = recordings.to_string_lossy().into_owned();
            config.recording.start_delay = 0.0;
            config.recording.camera.enabled = false;
            config.recording.mic_enabled = false;
            config.recording.system_audio = false;
        });
        RecordingControl::open(
            cx,
            if queued {
                RecordingTarget::Window
            } else {
                RecordingTarget::Area
            },
            crate::capture::overlay::ScreenRect {
                x: 40,
                y: 80,
                width: 640,
                height: 360,
            },
            None,
            if queued { Some(43) } else { None },
            None,
            PreferenceScope::Global,
        );
        registry::handle(WindowKind::RecordingControl, cx).unwrap()
    });
    settle(cx, control);
    let before = super::daemon_fixture::calls(output).len();
    click_measured(cx, control, "recording-controls", 0);
    settle(cx, control);
    wait_for_recording_start(cx);
    let calls = super::daemon_fixture::calls(output);
    let start = calls[before..]
        .iter()
        .find(|call| call["module"] == "screen-recorder" && call["method"] == "start")
        .unwrap();
    let recording = Path::new(start["params"]["outputPath"].as_str().unwrap()).to_owned();
    let contents = std::fs::read(&recording).unwrap();
    if queued || during_stop {
        take_notifications();
    }
    if queued {
        cx.update(|cx| {
            let daemon = &crate::state::state(cx).daemon;
            for code in ["TARGET_CLOSED", "TARGET_CLOSED", "CAPTURE_ERROR"] {
                daemon.emit_parity_event(
                    poratake_daemon_common::contract::SCREEN_RECORDER_ERROR_EVENT,
                    &serde_json::json!({"code": code, "message": "Fixture terminal failure"}),
                );
            }
        });
    } else {
        click_measured(
            cx,
            control,
            "recording-controls",
            if on_stop || during_stop { 1 } else { 5 },
        );
    }
    cx.run_until_parked();
    wait_for_recording_stop(cx);
    assert!(!cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
    assert_eq!(std::fs::read(&recording).unwrap(), contents);
    assert_eq!(
        crate::video::recorder::state(),
        crate::video::recorder::RecorderState::Idle
    );
    if queued || during_stop {
        let notifications = take_notifications();
        assert!(notifications.iter().any(|(title, body, _)| {
            title == "Recording failed"
                && body.contains("Fixture terminal failure")
                && body.contains(recording.parent().unwrap().to_str().unwrap())
        }));
        assert_eq!(
            super::daemon_fixture::calls(output)[before..]
                .iter()
                .filter(|call| call["module"] == "screen-recorder" && call["method"] == "stop")
                .count(),
            usize::from(during_stop)
        );
    }
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.recordings_path = previous.storage.recordings_path;
            config.recording = previous.recording;
        })
    });
    std::fs::write(
        output.join("capture-fixture.json"),
        b"{\"freeze\":\"frozen\"}",
    )
    .unwrap();
}

#[cfg(target_os = "macos")]
fn recording_start_timeout_handoff(
    cx: &mut HeadlessAppContext,
    output: &Path,
    succeeds: bool,
    release_before_stop: bool,
    target_closed: bool,
    stop_first: bool,
) {
    use crate::video::recorder::{RecorderState, RecordingTarget};
    use crate::windows::recording_control::{PreferenceScope, RecordingControl};
    use crate::windows::registry::{self, WindowKind};

    let recordings = output.join(format!(
        "recording-held-start-{succeeds}-{release_before_stop}-{target_closed}-{stop_first}"
    ));
    std::fs::create_dir_all(&recordings).unwrap();
    let mut fixture = serde_json::json!({
        "freeze": "live",
        "recordingHoldStartUntilStop": true,
        "recordingHeldStartFails": !succeeds,
        "recordingReleaseHeldStartOnProtection": release_before_stop || stop_first,
        "recordingHeldStartStopFirst": stop_first,
    });
    configure_recording_fixture(&mut fixture, "area");
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&fixture).unwrap(),
    )
    .unwrap();
    let previous = cx.update(|cx| crate::state::state(cx).config.get());
    let control = cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.recordings_path = recordings.to_string_lossy().into_owned();
            config.recording.start_delay = 0.;
            config.recording.camera.enabled = false;
            config.recording.mic_enabled = false;
            config.recording.system_audio = false;
            config.recording.show_preview = false;
            config.history.enabled = false;
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
            PreferenceScope::Global,
        );
        registry::handle(WindowKind::RecordingControl, cx).unwrap()
    });
    settle(cx, control);
    let before = super::daemon_fixture::calls(output).len();
    take_notifications();
    let start_requested_at = std::time::Instant::now();
    click_measured(cx, control, "recording-controls", 0);
    assert!(start_requested_at.elapsed() < std::time::Duration::from_secs(5));
    settle(cx, control);
    assert_eq!(crate::video::recorder::state(), RecorderState::Starting);
    assert!(cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
    capture(
        cx,
        control,
        output,
        &format!("recording-held-start-{succeeds}"),
    );
    let calls = super::daemon_fixture::calls(output);
    let start = calls[before..]
        .iter()
        .find(|call| call["module"] == "screen-recorder" && call["method"] == "start")
        .unwrap();
    let recording = Path::new(start["params"]["outputPath"].as_str().unwrap()).to_owned();
    let bytes = std::fs::read(&recording).unwrap();
    assert!(!calls[before..].iter().any(|call| {
        call["module"] == "camera-preview"
            && call["method"] == "setContentProtection"
            && call["params"]["enabled"] == false
    }));
    if target_closed {
        cx.update(|cx| {
            crate::state::state(cx).daemon.emit_parity_event(
                poratake_daemon_common::contract::SCREEN_RECORDER_ERROR_EVENT,
                &serde_json::json!({"code": "TARGET_CLOSED"}),
            );
        });
        cx.run_until_parked();
        assert!(!super::daemon_fixture::calls(output)[before..]
            .iter()
            .any(|call| call["module"] == "screen-recorder" && call["method"] == "stop"));
    }
    if release_before_stop {
        cx.update(|cx| {
            crate::state::state(cx)
                .daemon
                .camera_preview()
                .set_content_protection(true)
                .unwrap();
        });
        let started = std::time::Instant::now();
        while crate::video::recorder::state() == RecorderState::Starting {
            cx.run_until_parked();
            assert!(started.elapsed() < std::time::Duration::from_secs(10));
            std::thread::yield_now();
        }
        if !target_closed {
            assert_eq!(
                crate::video::recorder::state(),
                if succeeds {
                    RecorderState::Recording
                } else {
                    RecorderState::Idle
                }
            );
        }
    }
    if (!target_closed || stop_first) && (succeeds || !release_before_stop) {
        click_measured(
            cx,
            control,
            "recording-controls",
            if target_closed { 7 } else { 1 },
        );
        if stop_first {
            wait_for_recording_stop(cx);
            assert_eq!(crate::video::recorder::state(), RecorderState::Starting);
            cx.update(|cx| {
                crate::state::state(cx)
                    .daemon
                    .camera_preview()
                    .set_content_protection(true)
                    .unwrap();
            });
        }
    } else if !target_closed {
        assert!(cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
        settle(cx, control);
        capture(cx, control, output, "recording-start-rejected-controls");
        assert!(cx.update(RecordingControl::cancel_pre_recording));
    }
    let started = std::time::Instant::now();
    loop {
        cx.run_until_parked();
        if !cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)) {
            break;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "start case: {succeeds} {release_before_stop} {target_closed} {stop_first}"
        );
        std::thread::yield_now();
    }
    assert_eq!(crate::video::recorder::state(), RecorderState::Idle);
    if target_closed && stop_first {
        assert!(!recording.parent().unwrap().exists());
    } else {
        assert_eq!(std::fs::read(&recording).unwrap(), bytes);
    }
    let notifications = take_notifications();
    if !succeeds {
        assert!(notifications.iter().any(|(title, body, _)| {
            title == "Recording failed"
                && body.contains("Fixture late start failure")
                && body.contains(recording.parent().unwrap().to_str().unwrap())
        }));
    }
    assert!(super::daemon_fixture::calls(output)[before..]
        .iter()
        .any(|call| call["module"] == "camera-preview"
            && call["method"] == "setContentProtection"
            && call["params"]["enabled"] == false));
    assert_eq!(
        super::daemon_fixture::calls(output)[before..]
            .iter()
            .filter(|call| call["module"] == "screen-recorder" && call["method"] == "stop")
            .count(),
        if target_closed && stop_first {
            2
        } else {
            usize::from(succeeds || !release_before_stop || target_closed)
        }
    );
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.recordings_path = previous.storage.recordings_path;
            config.recording = previous.recording;
            config.history = previous.history;
        })
    });
    std::fs::write(
        output.join("capture-fixture.json"),
        b"{\"freeze\":\"frozen\"}",
    )
    .unwrap();
}

#[cfg(target_os = "macos")]
fn recording_stop_timeout_handoff(
    cx: &mut HeadlessAppContext,
    output: &Path,
    case: (bool, bool, bool, bool, bool, bool),
) {
    use crate::video::recorder::{RecorderState, RecordingTarget};
    use crate::windows::recording_control::{PreferenceScope, RecordingControl};
    use crate::windows::registry::{self, WindowKind};

    let (fails, discard, terminal, start_pending, paused, target_closed) = case;
    let recordings = output.join(format!(
        "held-stop-{fails}-{discard}-{terminal}-{start_pending}-{paused}-{target_closed}"
    ));
    std::fs::create_dir_all(&recordings).unwrap();
    let mut fixture = serde_json::json!({
        "freeze": "live",
        "recordingHoldStop": true,
        "recordingHeldStopFails": fails,
        "recordingHeldStopTerminalError": terminal,
        "recordingReleaseHeldStopOnProtection": true,
        "recordingHoldStartUntilStop": start_pending,
    });
    configure_recording_fixture(&mut fixture, "area");
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&fixture).unwrap(),
    )
    .unwrap();
    let previous = cx.update(|cx| crate::state::state(cx).config.get());
    let control = cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.recordings_path = recordings.to_string_lossy().into_owned();
            config.recording.start_delay = 0.;
            config.recording.camera.enabled = false;
            config.recording.mic_enabled = false;
            config.recording.system_audio = false;
            config.recording.show_preview = false;
            config.history.enabled = false;
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
            PreferenceScope::Global,
        );
        registry::handle(WindowKind::RecordingControl, cx).unwrap()
    });
    settle(cx, control);
    let before = super::daemon_fixture::calls(output).len();
    click_measured(cx, control, "recording-controls", 0);
    settle(cx, control);
    if !start_pending {
        wait_for_recording_start(cx);
    }
    assert_eq!(
        crate::video::recorder::state(),
        if start_pending {
            RecorderState::Starting
        } else {
            RecorderState::Recording
        }
    );
    if paused {
        click_measured(cx, control, "recording-controls", 0);
        assert_eq!(crate::video::recorder::state(), RecorderState::Paused);
    }
    let calls = super::daemon_fixture::calls(output);
    let start = calls[before..]
        .iter()
        .find(|call| call["module"] == "screen-recorder" && call["method"] == "start")
        .unwrap();
    let recording = Path::new(start["params"]["outputPath"].as_str().unwrap()).to_owned();
    let bytes = std::fs::read(&recording).unwrap();
    take_notifications();
    let stop_requested_at = std::time::Instant::now();
    click_measured(
        cx,
        control,
        "recording-controls",
        if discard { 7 } else { 1 },
    );
    assert!(stop_requested_at.elapsed() < std::time::Duration::from_secs(5));
    settle(cx, control);
    assert_eq!(crate::video::recorder::state(), RecorderState::Stopping);
    assert!(cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
    assert_eq!(std::fs::read(&recording).unwrap(), bytes);
    capture(
        cx,
        control,
        output,
        &format!("recording-held-stop-{fails}-{discard}-{terminal}-{start_pending}"),
    );
    click_measured(cx, control, "recording-controls", 1);
    assert_eq!(
        super::daemon_fixture::calls(output)[before..]
            .iter()
            .filter(|call| call["module"] == "screen-recorder" && call["method"] == "stop")
            .count(),
        1
    );
    if target_closed {
        cx.update(|cx| {
            crate::state::state(cx).daemon.emit_parity_event(
                poratake_daemon_common::contract::SCREEN_RECORDER_ERROR_EVENT,
                &serde_json::json!({"code": "TARGET_CLOSED"}),
            );
        });
        cx.run_until_parked();
    }
    cx.update(|cx| {
        crate::state::state(cx)
            .daemon
            .camera_preview()
            .set_content_protection(true)
            .unwrap()
    });
    let started = std::time::Instant::now();
    while crate::video::recorder::state() == RecorderState::Stopping {
        cx.run_until_parked();
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        std::thread::yield_now();
    }
    if fails && !terminal && !target_closed {
        let started = std::time::Instant::now();
        while start_pending && crate::video::recorder::state() == RecorderState::Starting {
            cx.run_until_parked();
            assert!(started.elapsed() < std::time::Duration::from_secs(10));
            std::thread::yield_now();
        }
        assert_eq!(
            crate::video::recorder::state(),
            if paused {
                RecorderState::Paused
            } else {
                RecorderState::Recording
            }
        );
        assert_eq!(std::fs::read(&recording).unwrap(), bytes);
        settle(cx, control);
        click_measured(
            cx,
            control,
            "recording-controls",
            if discard { 7 } else { 1 },
        );
        cx.run_until_parked();
    }
    let started = std::time::Instant::now();
    while cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)) {
        cx.run_until_parked();
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        std::thread::yield_now();
    }
    assert_eq!(crate::video::recorder::state(), RecorderState::Idle);
    assert!(!cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
    if discard && !terminal {
        assert!(!recording.parent().unwrap().exists());
    } else {
        assert_eq!(std::fs::read(&recording).unwrap(), bytes);
    }
    if terminal {
        assert!(take_notifications()
            .iter()
            .any(|(title, body, _)| title == "Recording failed"
                && body.contains("Fixture held stop terminal failure")
                && body.contains(recording.parent().unwrap().to_str().unwrap())));
    }
    if target_closed {
        assert_eq!(
            super::daemon_fixture::calls(output)[before..]
                .iter()
                .filter(|call| call["module"] == "screen-recorder" && call["method"] == "stop")
                .count(),
            2
        );
    }
    cx.update(|cx| {
        registry::close(WindowKind::VideoEditor, cx);
        crate::state::state(cx).config.update(|config| {
            config.storage.recordings_path = previous.storage.recordings_path;
            config.recording = previous.recording;
            config.history = previous.history;
        });
    });
    std::fs::write(
        output.join("capture-fixture.json"),
        b"{\"freeze\":\"frozen\"}",
    )
    .unwrap();
}

pub(super) fn recording_preference_handoffs(cx: &mut HeadlessAppContext, output: &Path) {
    #[cfg(any(windows, target_os = "macos"))]
    use std::io::{Read, Write};

    use crate::windows::registry::{self, WindowKind};

    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({"freeze": "frozen"})).unwrap(),
    )
    .unwrap();
    for target in ["area", "window", "screen"] {
        #[cfg(any(windows, target_os = "macos"))]
        let before = super::daemon_fixture::calls(output).len();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.recording.system_audio = false;
                config.recording.mic_enabled = false;
                config.recording.camera.enabled = false;
                #[cfg(windows)]
                {
                    config.screenshot.freeze_screen = true;
                    config.screenshot.hide_desktop_icons = true;
                }
                config.all_in_one.remember_choices = true;
                config.all_in_one.last_mode = "record".into();
                config.all_in_one.last_targets.record = target.into();
                config.all_in_one.recording.system_audio = true;
                config.all_in_one.recording.mic_enabled = false;
                config.all_in_one.recording.camera_enabled = target == "window";
            });
            crate::capture::start_all_in_one(cx);
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        if target == "area" {
            select_area(cx, picker);
        } else {
            event(
                cx,
                picker,
                PlatformInput::MouseDown(MouseDownEvent {
                    position: point(px(240.), px(180.)),
                    button: MouseButton::Left,
                    click_count: 1,
                    modifiers: Modifiers::none(),
                    first_mouse: false,
                }),
            );
        }
        cx.run_until_parked();
        #[cfg(windows)]
        wait_for_icon_restoration(cx);
        #[cfg(windows)]
        {
            let calls = super::daemon_fixture::calls(output);
            let calls = &calls[before..];
            let position = |module: &str, method: &str| {
                calls
                    .iter()
                    .position(|call| call["module"] == module && call["method"] == method)
                    .unwrap()
            };
            assert!(position("desktop-helper", "hide") < position("freeze-screen", "freeze"));
            assert!(position("freeze-screen", "freeze") < position("desktop-helper", "show"));
            assert!(!crate::capture::desktop_icons::are_hidden());
        }
        let control = cx
            .update(|cx| registry::handle(WindowKind::RecordingControl, cx))
            .unwrap_or_else(|| panic!("{target} selection did not open recording controls"));
        settle(cx, control);
        let audio = if target == "window" { 6 } else { 4 };
        click_measured(cx, control, "recording-controls", audio);
        cx.update(|cx| {
            let config = crate::state::state(cx).config.get();
            assert!(
                !config.recording.system_audio,
                "{target} changed global preferences"
            );
            assert!(
                !config.all_in_one.recording.system_audio,
                "{target} did not update all-in-one preferences"
            );
        });
        if target == "area" {
            for input in [
                PlatformInput::MouseDown(MouseDownEvent {
                    position: point(px(200.), px(150.)),
                    button: MouseButton::Left,
                    click_count: 1,
                    modifiers: Modifiers::none(),
                    first_mouse: false,
                }),
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: point(px(210.), px(160.)),
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Modifiers::none(),
                }),
                PlatformInput::MouseUp(MouseUpEvent {
                    position: point(px(210.), px(160.)),
                    button: MouseButton::Left,
                    click_count: 1,
                    modifiers: Modifiers::none(),
                }),
            ] {
                event(cx, picker, input);
            }
            settle(cx, control);
            let rect = cx.update(|cx| {
                registry::handle(WindowKind::RecordingControl, cx)
                    .unwrap()
                    .downcast::<crate::windows::recording_control::RecordingControl>()
                    .unwrap()
                    .update(cx, |control, _, _| control.parity_rect())
                    .unwrap()
            });
            let scale = cx
                .update_window(picker, |_, window, _| window.scale_factor())
                .unwrap();
            let offset = (10. * crate::capture::overlay::capture_coordinate_scale(scale)) as i32;
            assert_eq!(
                rect,
                CaptureRect {
                    x: SELECTED_RECT.x + offset,
                    y: SELECTED_RECT.y + offset,
                    ..SELECTED_RECT
                },
                "the selected area remains adjustable after the recording controls open"
            );
            click_measured(cx, control, "recording-controls", audio);
            cx.update(|cx| {
                let config = crate::state::state(cx).config.get();
                assert!(!config.recording.system_audio);
                assert!(config.all_in_one.recording.system_audio);
            });
        }
        #[cfg(target_os = "macos")]
        if target == "area" {
            let recordings = output.join("recording-area-failed-start");
            std::fs::create_dir_all(&recordings).unwrap();
            let previous = cx.update(|cx| {
                let config = crate::state::state(cx).config.get();
                (
                    config.storage.recordings_path,
                    config.recording.start_delay,
                    config.recording.show_preview,
                    config.history.enabled,
                )
            });
            let mut fixture = serde_json::json!({
                "freeze": "live", "recordingAreaStartFails": true,
                "recordingAreaStartWritesPartial": true,
            });
            let expected_duration = configure_recording_fixture(&mut fixture, "area");
            std::fs::write(
                output.join("capture-fixture.json"),
                serde_json::to_vec(&fixture).unwrap(),
            )
            .unwrap();
            cx.update(|cx| {
                crate::state::state(cx).config.update(|config| {
                    config.storage.recordings_path = recordings.to_string_lossy().into_owned();
                    config.recording.start_delay = 0.0;
                    config.recording.show_preview = false;
                    config.history.enabled = true;
                })
            });
            let before = super::daemon_fixture::calls(output).len();
            take_notifications();
            click_measured(cx, control, "recording-controls", 0);
            cx.run_until_parked();
            wait_for_recording_start(cx);
            let calls = super::daemon_fixture::calls(output);
            let calls = &calls[before..];
            let show = calls
                .iter()
                .position(|call| call["module"] == "recording-overlay" && call["method"] == "show")
                .expect("area overlay shown");
            let start = calls
                .iter()
                .position(|call| call["module"] == "screen-recorder" && call["method"] == "start")
                .expect("area start attempted");
            let hide = calls
                .iter()
                .position(|call| call["module"] == "recording-overlay" && call["method"] == "hide")
                .expect("failed area overlay hidden");
            assert!(show < start && start < hide);
            assert_eq!(
                calls[start]["params"]["excludedWindowIds"],
                serde_json::json!([
                    crate::windows::recording_control::PARITY_RECORDING_CONTROL_WINDOW_ID
                ])
            );
            assert!(!calls
                .iter()
                .any(|call| call["module"] == "screen-recorder" && call["method"] == "stop"));
            assert!(cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
            assert_eq!(
                crate::video::recorder::state(),
                crate::video::recorder::RecorderState::Idle
            );
            let failed_recording =
                Path::new(calls[start]["params"]["outputPath"].as_str().unwrap());
            assert_eq!(
                std::fs::read(failed_recording).unwrap(),
                include_bytes!("fixtures/capture.mov")
            );
            let notifications = take_notifications();
            assert!(notifications.iter().any(|(title, body, _)| {
                title == "Recording failed"
                    && body.contains(failed_recording.parent().unwrap().to_str().unwrap())
            }));
            click_measured(cx, control, "recording-controls", 0);
            settle(cx, control);
            wait_for_recording_start(cx);
            assert_eq!(
                crate::video::recorder::state(),
                crate::video::recorder::RecorderState::Recording
            );
            cx.advance_clock(std::time::Duration::from_secs(2));
            settle(cx, control);
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            fixture["recordingHoldOutlineHide"] = true.into();
            fixture["barrier"] = listener.local_addr().unwrap().to_string().into();
            std::fs::write(
                output.join("capture-fixture.json"),
                serde_json::to_vec(&fixture).unwrap(),
            )
            .unwrap();
            let (ready, hide) = std::sync::mpsc::sync_channel(1);
            let accept_hide = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                    .unwrap();
                stream.read_exact(&mut [1]).unwrap();
                ready.send(stream).unwrap();
            });
            click_measured(cx, control, "recording-controls", 1);
            cx.run_until_parked();
            wait_for_recording_stop(cx);
            let mut hide = hide
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
            accept_hide.join().unwrap();
            cx.run_until_parked();
            assert!(
                cx.update(|cx| registry::is_open(WindowKind::VideoEditor, cx)),
                "the recording editor must open while outline cleanup is pending"
            );
            hide.set_nonblocking(true).unwrap();
            assert!(
                matches!(hide.peek(&mut [0]), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
                "the fixture must still be waiting for outline cleanup release"
            );
            hide.write_all(&[0]).unwrap();
            assert_eq!(
                crate::video::recorder::state(),
                crate::video::recorder::RecorderState::Idle
            );
            assert!(!cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
            let calls = super::daemon_fixture::calls(output);
            let starts: Vec<_> = calls[before..]
                .iter()
                .filter(|call| call["module"] == "screen-recorder" && call["method"] == "start")
                .collect();
            assert_eq!(starts.len(), 2);
            assert_eq!(
                starts[1]["params"]["excludedWindowIds"],
                serde_json::json!([
                    crate::windows::recording_control::PARITY_RECORDING_CONTROL_WINDOW_ID
                ])
            );
            let recording = Path::new(starts[1]["params"]["outputPath"].as_str().unwrap());
            let project = recording.parent().unwrap();
            assert!(recording.is_file());
            let history = crate::history_store::load_history();
            let item = history
                .iter()
                .find(|item| Path::new(&item.original_path) == project)
                .unwrap();
            assert_eq!(
                item.duration,
                Some(expected_duration),
                "history must use the finalized media duration"
            );
            assert_eq!(
                crate::windows::video_editor::model::load_state(project).source_duration,
                Some(expected_duration),
                "the editor must use the finalized media duration"
            );
            recorded_editor_and_export(cx, project, output, "area");
            cx.update(|cx| {
                crate::state::state(cx).config.update(|config| {
                    config.storage.recordings_path = previous.0;
                    config.recording.start_delay = previous.1;
                    config.recording.show_preview = previous.2;
                    config.history.enabled = previous.3;
                })
            });
            std::fs::write(
                output.join("capture-fixture.json"),
                serde_json::to_vec(&serde_json::json!({"freeze": "frozen"})).unwrap(),
            )
            .unwrap();
        }
        #[cfg(any(windows, target_os = "macos"))]
        if target == "window" {
            let recordings = output.join("recording-target-close-handoff");
            std::fs::create_dir_all(&recordings).unwrap();
            let previous_settings = cx.update(|cx| {
                let config = crate::state::state(cx).config.get();
                (
                    config.storage.recordings_path,
                    config.recording.start_delay,
                    config.recording.show_preview,
                    config.history.enabled,
                )
            });
            let fixture = serde_json::json!({
                "freeze": "live",
                "recordingStartFailsAfterQueuedError": true,
                "recordingTargetClosedDuringStart": true,
                "recordingStopFailsAfterQueuedTargetClosed": true,
            });
            #[cfg(target_os = "macos")]
            let (fixture, expected_duration) = {
                let mut fixture = fixture;
                let duration = configure_recording_fixture(&mut fixture, "window");
                (fixture, duration)
            };
            std::fs::write(
                output.join("capture-fixture.json"),
                serde_json::to_vec(&fixture).unwrap(),
            )
            .unwrap();
            cx.update(|cx| {
                crate::state::state(cx).config.update(|config| {
                    config.storage.recordings_path = recordings.to_string_lossy().into_owned();
                    config.recording.start_delay = 0.0;
                    config.recording.show_preview = false;
                    config.history.enabled = true;
                    config.all_in_one.remember_choices = true;
                    config.all_in_one.last_mode = "record".into();
                    config.all_in_one.last_targets.record = "window".into();
                    config.all_in_one.recording.system_audio = false;
                    config.all_in_one.recording.mic_enabled = false;
                    config.all_in_one.recording.camera_enabled = true;
                });
            });
            let recording_before = super::daemon_fixture::calls(output).len();
            click_measured(cx, control, "recording-controls", 2);
            cx.run_until_parked();
            #[cfg(target_os = "macos")]
            wait_for_recording_start(cx);
            let first_calls = super::daemon_fixture::calls(output);
            let first_attempt = &first_calls[recording_before..];
            assert_eq!(
                first_attempt
                    .iter()
                    .filter(|call| call["module"] == "screen-recorder" && call["method"] == "start")
                    .count(),
                1
            );
            let failed_output = first_attempt
                .iter()
                .find(|call| call["module"] == "screen-recorder" && call["method"] == "start")
                .and_then(|call| call["params"]["outputPath"].as_str())
                .map(Path::new)
                .expect("failed start output path");
            let failed_project = failed_output
                .parent()
                .expect("failed project path")
                .to_path_buf();
            assert!(
                !failed_project.exists(),
                "failed start project must be removed"
            );
            assert!(cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
            assert_eq!(
                crate::video::recorder::state(),
                crate::video::recorder::RecorderState::Idle
            );
            let protection: Vec<_> = first_attempt
                .iter()
                .filter(|call| {
                    call["module"] == "camera-preview" && call["method"] == "setContentProtection"
                })
                .map(|call| call["params"]["enabled"].as_bool().unwrap())
                .collect();
            assert_eq!(protection, [true, false]);
            assert!(!first_attempt.iter().any(|call| {
                call["module"] == "screen-recorder" && call["method"] == "stop"
                    || call["module"] == "recording-overlay" && call["method"] == "hide"
            }));

            click_measured(cx, control, "recording-controls", 2);
            let started = std::time::Instant::now();
            loop {
                cx.run_until_parked();
                let calls = super::daemon_fixture::calls(output);
                let calls = &calls[recording_before..];
                let stopped = calls
                    .iter()
                    .any(|call| call["module"] == "screen-recorder" && call["method"] == "stop");
                let overlay_hidden = calls
                    .iter()
                    .any(|call| call["module"] == "recording-overlay" && call["method"] == "hide");
                let finalized = crate::history_store::load_history().iter().any(|item| {
                    item.r#type == HistoryItemType::Video
                        && Path::new(&item.original_path).parent() == Some(recordings.as_path())
                });
                let controls_closed =
                    cx.update(|cx| !registry::is_open(WindowKind::RecordingControl, cx));
                let overlay_closed = !cx.update(crate::capture::overlay::parity_is_active);
                if stopped && overlay_hidden && finalized && controls_closed && overlay_closed {
                    break;
                }
                assert!(
                    started.elapsed() < std::time::Duration::from_secs(10),
                    "target-close handoff did not finalize and clean up"
                );
                std::thread::yield_now();
            }
            let calls = super::daemon_fixture::calls(output);
            let handoff_calls = &calls[before..];
            let outlines: Vec<_> = handoff_calls
                .iter()
                .filter(|call| {
                    call["module"] == "recording-overlay" && call["method"] == "showWindow"
                })
                .collect();
            assert_eq!(outlines.len(), 1, "the picked window keeps one outline");
            assert_eq!(outlines[0]["params"]["windowId"], 43);
            let outline = handoff_calls
                .iter()
                .position(|call| {
                    call["module"] == "recording-overlay" && call["method"] == "showWindow"
                })
                .unwrap();
            let start = handoff_calls
                .iter()
                .position(|call| call["module"] == "screen-recorder" && call["method"] == "start")
                .unwrap();
            assert!(
                outline < start,
                "the outline begins when the window is picked"
            );
            let calls = &calls[recording_before..];
            assert_eq!(
                calls
                    .iter()
                    .filter(|call| call["module"] == "screen-recorder" && call["method"] == "start")
                    .count(),
                2
            );
            assert_eq!(
                calls
                    .iter()
                    .filter(|call| call["module"] == "screen-recorder" && call["method"] == "stop")
                    .count(),
                2
            );
            let last_stop = calls
                .iter()
                .rposition(|call| call["module"] == "screen-recorder" && call["method"] == "stop")
                .unwrap();
            let preview_hide = calls
                .iter()
                .position(|call| call["module"] == "camera-preview" && call["method"] == "hide")
                .unwrap();
            assert!(
                last_stop < preview_hide,
                "the camera preview must survive a failed stop"
            );
            #[cfg(target_os = "macos")]
            {
                let recording = Path::new(
                    calls
                        .iter()
                        .rfind(|call| {
                            call["module"] == "screen-recorder" && call["method"] == "start"
                        })
                        .unwrap()["params"]["outputPath"]
                        .as_str()
                        .unwrap(),
                );
                let project = recording.parent().unwrap();
                assert!(recording.is_file());
                assert_eq!(
                    crate::windows::video_editor::model::load_state(project).source_duration,
                    Some(expected_duration)
                );
                let history = crate::history_store::load_history();
                assert_eq!(
                    history
                        .iter()
                        .find(|item| Path::new(&item.original_path) == project)
                        .unwrap()
                        .duration,
                    Some(expected_duration)
                );
                recorded_editor_and_export(cx, project, output, "window");
            }
            #[cfg(not(target_os = "macos"))]
            cx.update(|cx| registry::close(WindowKind::VideoEditor, cx));
            cx.update(|cx| {
                crate::state::state(cx).config.update(|config| {
                    config.storage.recordings_path = previous_settings.0;
                    config.recording.start_delay = previous_settings.1;
                    config.recording.show_preview = previous_settings.2;
                    config.history.enabled = previous_settings.3;
                });
            });
            std::fs::write(
                output.join("capture-fixture.json"),
                serde_json::to_vec(&serde_json::json!({"freeze": "frozen"})).unwrap(),
            )
            .unwrap();
        } else {
            #[cfg(target_os = "macos")]
            if target == "screen" {
                recorded_screen_handoff(cx, control, output);
            }
            cx.update(|cx| {
                if registry::is_open(WindowKind::RecordingControl, cx) {
                    assert!(
                        crate::windows::recording_control::RecordingControl::cancel_pre_recording(
                            cx
                        )
                    );
                }
                crate::capture::overlay::close_all(cx);
            });
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        cx.update(|cx| {
            assert!(crate::windows::recording_control::RecordingControl::cancel_pre_recording(cx));
            crate::capture::overlay::close_all(cx);
        });
        cx.run_until_parked();
    }
    #[cfg(target_os = "macos")]
    if std::env::var_os("PORATAKE_PARITY_DISTINCT_RECORDINGS").is_some() {
        super::recording_preview_comparison::assert_distinct_exports(output);
    }
    #[cfg(target_os = "macos")]
    for (on_stop, queued, during_stop) in [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        recording_failure_preserves_project(cx, output, on_stop, queued, during_stop);
    }
    #[cfg(target_os = "macos")]
    for (succeeds, release_before_stop, target_closed, stop_first) in [
        (true, false, false, false),
        (false, false, false, false),
        (true, true, false, false),
        (false, true, false, false),
        (true, true, true, false),
        (false, false, false, true),
        (true, false, true, true),
    ] {
        recording_start_timeout_handoff(
            cx,
            output,
            succeeds,
            release_before_stop,
            target_closed,
            stop_first,
        );
    }
    #[cfg(target_os = "macos")]
    for case in [
        (false, false, false, false, false, false),
        (true, false, false, false, false, false),
        (true, true, false, false, false, false),
        (false, true, false, false, false, false),
        (false, false, true, false, false, false),
        (true, false, true, false, false, false),
        (false, false, false, true, false, false),
        (true, false, false, true, false, false),
        (false, false, true, true, false, false),
        (true, false, false, false, true, false),
        (true, false, false, false, false, true),
        (true, true, false, false, false, true),
    ] {
        recording_stop_timeout_handoff(cx, output, case);
    }
    #[cfg(windows)]
    {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen",
                "desktopShowHeld": true,
                "desktopShowHoldKey": "aio-record-multidisplay",
                "barrier": listener.local_addr().unwrap().to_string(),
            }))
            .unwrap(),
        )
        .unwrap();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let show = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.read_exact(&mut [1]).unwrap();
            ready_tx.send(()).unwrap();
            release_rx
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
            stream.write_all(&[0]).unwrap();
        });
        let before = super::daemon_fixture::calls(output).len();
        let duplicate_display = DuplicateDisplayTarget::enable();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.screenshot.freeze_screen = true;
                config.screenshot.hide_desktop_icons = true;
                config.all_in_one.remember_choices = true;
                config.all_in_one.last_mode = "record".into();
                config.all_in_one.last_targets.record = "area".into();
            });
            crate::capture::start_all_in_one(cx);
        });
        cx.run_until_parked();
        drop(duplicate_display);
        let overlays = cx.update(crate::capture::overlay::parity_active_overlays);
        assert_eq!(
            overlays.len(),
            2,
            "the fixture must have two display overlays"
        );
        select_area(cx, overlays[0]);
        cx.run_until_parked();
        ready_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        assert!(
            cx.update(|cx| !registry::is_open(WindowKind::RecordingControl, cx)),
            "recording controls must wait for restoration on the selected display"
        );
        select_area(cx, overlays[1]);
        cx.run_until_parked();
        assert!(
            cx.update(|cx| !registry::is_open(WindowKind::RecordingControl, cx)),
            "the sibling display must be locked while restoration is pending"
        );
        assert!(!super::daemon_fixture::calls(output)[before..]
            .iter()
            .any(|call| call["module"] == "screen-recorder" && call["method"] == "start"));
        release_tx.send(()).unwrap();
        show.join().unwrap();
        wait_for_icon_restoration(cx);
        assert!(
            cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)),
            "only the selected display should open recording controls after restoration"
        );
        let calls = super::daemon_fixture::calls(output);
        let calls = &calls[before..];
        let show_index = calls
            .iter()
            .position(|call| call["module"] == "desktop-helper" && call["method"] == "show")
            .unwrap();
        let freeze_index = calls
            .iter()
            .position(|call| call["module"] == "freeze-screen" && call["method"] == "freeze")
            .unwrap();
        assert!(freeze_index < show_index);
        assert!(!calls
            .iter()
            .any(|call| call["module"] == "screen-recorder" && call["method"] == "start"));
        assert!(
            cx.update(crate::windows::recording_control::RecordingControl::cancel_pre_recording)
        );
        cx.update(crate::capture::overlay::close_all);
        cx.run_until_parked();
    }
    #[cfg(windows)]
    {
        super::take_notifications();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen", "desktopHideError": "not-applied",
            }))
            .unwrap(),
        )
        .unwrap();
        let before = super::daemon_fixture::calls(output).len();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.screenshot.freeze_screen = true;
                config.screenshot.hide_desktop_icons = true;
                config.all_in_one.remember_choices = true;
                config.all_in_one.last_mode = "record".into();
                config.all_in_one.last_targets.record = "area".into();
            });
            crate::capture::start_all_in_one(cx);
        });
        cx.run_until_parked();
        assert!(!cx.update(crate::capture::overlay::parity_is_active));
        assert!(!cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
        assert_eq!(
            super::take_notifications(),
            [(
                "Recording failed".into(),
                "Could not hide desktop icons".into(),
                false
            )]
        );
        let calls = super::daemon_fixture::calls(output);
        let calls = &calls[before..];
        let hide = calls
            .iter()
            .position(|call| call["module"] == "desktop-helper" && call["method"] == "hide")
            .unwrap();
        let show = calls
            .iter()
            .position(|call| call["module"] == "desktop-helper" && call["method"] == "show")
            .unwrap();
        assert!(hide < show);
        assert!(!calls
            .iter()
            .any(|call| call["module"] == "freeze-screen" && call["method"] == "freeze"));
        assert!(!crate::capture::desktop_icons::are_hidden());
        cx.update(|cx| {
            crate::state::state(cx)
                .config
                .update(|config| config.screenshot.hide_desktop_icons = false);
        });

        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen", "desktopShowError": true,
            }))
            .unwrap(),
        )
        .unwrap();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.screenshot.freeze_screen = true;
                config.screenshot.hide_desktop_icons = true;
                config.all_in_one.remember_choices = true;
                config.all_in_one.last_mode = "record".into();
                config.all_in_one.last_targets.record = "area".into();
            });
            crate::capture::start_all_in_one(cx);
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        select_area(cx, picker);
        wait_for_icon_restoration(cx);
        assert!(
            cx.update(|cx| !registry::is_open(WindowKind::RecordingControl, cx)),
            "recording controls must stay closed when desktop icon restoration fails"
        );
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({"freeze": "frozen"})).unwrap(),
        )
        .unwrap();
        let daemon = cx.update(|cx| crate::state::state(cx).daemon.clone());
        assert!(crate::capture::desktop_icons::restore_if_unowned(&daemon));
        assert!(!crate::capture::desktop_icons::are_hidden());

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen",
                "desktopShowHeld": true,
                "desktopShowHoldKey": "aio-record-replacement",
                "barrier": listener.local_addr().unwrap().to_string(),
            }))
            .unwrap(),
        )
        .unwrap();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let show = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.read_exact(&mut [1]).unwrap();
            ready_tx.send(()).unwrap();
            release_rx
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
            stream.write_all(&[0]).unwrap();
        });
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.screenshot.freeze_screen = true;
                config.screenshot.hide_desktop_icons = true;
                config.all_in_one.remember_choices = true;
                config.all_in_one.last_mode = "record".into();
                config.all_in_one.last_targets.record = "area".into();
            });
            crate::capture::start_all_in_one(cx);
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        select_area(cx, picker);
        cx.run_until_parked();
        ready_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        assert!(
            cx.update(|cx| !registry::is_open(WindowKind::RecordingControl, cx)),
            "recording controls must wait for desktop icon restoration"
        );
        select_area(cx, picker);
        cx.run_until_parked();
        assert!(
            cx.update(|cx| !registry::is_open(WindowKind::RecordingControl, cx)),
            "a duplicate selection must not open controls before restoration"
        );
        cx.update(crate::capture::start_area_capture);
        release_tx.send(()).unwrap();
        show.join().unwrap();
        wait_for_icon_restoration(cx);
        assert_eq!(
            cx.update(crate::capture::overlay::active_intents_for_test),
            [crate::capture::intent::CaptureIntent::Screenshot],
            "the replacement selection must survive completion of the stale recording handoff"
        );
        assert!(
            cx.update(|cx| !registry::is_open(WindowKind::RecordingControl, cx)),
            "a replaced overlay must not open stale recording controls"
        );
        let replacement = area_picker(cx);
        super::escape(cx, replacement);
        cx.run_until_parked();
        assert!(!crate::capture::desktop_icons::are_hidden());
        cx.update(|cx| {
            crate::state::state(cx)
                .config
                .update(|config| config.screenshot.hide_desktop_icons = false);
        });
    }
}

pub(super) fn selection_captures(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path) {
    #[cfg(target_os = "macos")]
    let display = cx.update(|cx| {
        let display = cx.primary_display().unwrap();
        let bounds = crate::system::work_area::display_bounds(display.as_ref());
        let scale = crate::capture::overlay::display_scale_factor(display.as_ref(), cx);
        crate::capture::overlay::physical_rect(bounds, scale)
    });
    #[cfg(not(target_os = "macos"))]
    let display = CaptureRect {
        x: 0,
        y: 0,
        width: 3840,
        height: 2160,
    };
    for (kind, start, rect) in [
        (
            "window",
            crate::capture::start_window_capture as fn(&mut gpui::App),
            SELECTED_RECT,
        ),
        (
            "area",
            crate::capture::start_area_capture as fn(&mut gpui::App),
            SELECTED_RECT,
        ),
        (
            "display",
            crate::capture::start_all_in_one as fn(&mut gpui::App),
            display,
        ),
    ] {
        for (name, freeze, color) in [
            ("live", false, [240, 80, 64, 255]),
            ("frozen", true, [64, 112, 192, 255]),
            ("freeze-failed", true, [240, 80, 64, 255]),
            ("freeze-unavailable", true, [240, 80, 64, 255]),
        ] {
            let captures = scratch.join(format!("{kind}-{name}"));
            std::fs::create_dir_all(&captures).unwrap();
            let sentinel = format!("clipboard before {kind}-{name}");
            std::fs::write(
                output.join("capture-fixture.json"),
                serde_json::to_vec(&serde_json::json!({
                    "freeze": name,
                    "rect": rect,
                    "windowId": (kind == "window").then_some(43),
                    "expectedExcludedWindowIds": (cfg!(target_os = "macos") && kind != "window")
                        .then_some(crate::capture::overlay::PARITY_OVERLAY_WINDOW_IDS),
                }))
                .unwrap(),
            )
            .unwrap();
            cx.update(|cx| {
                crate::state::state(cx).config.update(|config| {
                    config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                    config.screenshot.freeze_screen = freeze;
                    config.screenshot.show_preview = true;
                    config.preview.corner = "bottom-right".into();
                    config.screenshot.hide_desktop_icons = false;
                    config.screenshot.capture_to_clipboard = false;
                    config.screenshot.auto_copy_to_clipboard = kind == "area";
                    config.all_in_one.remember_choices = true;
                    config.all_in_one.last_mode = "screenshot".into();
                    config.all_in_one.last_targets.screenshot = "screen".into();
                });
                crate::system::clipboard::ClipboardService::write_text(cx, sentinel.clone());
                start(cx);
            });
            cx.run_until_parked();
            let picker = area_picker(cx);
            if kind == "area" {
                select_area(cx, picker);
            } else {
                event(
                    cx,
                    picker,
                    PlatformInput::MouseDown(MouseDownEvent {
                        position: point(px(240.), px(180.)),
                        button: MouseButton::Left,
                        click_count: 1,
                        modifiers: Modifiers::none(),
                        first_mouse: false,
                    }),
                );
            }
            cx.run_until_parked();
            if kind == "area" && name == "live" {
                let editor = open_verified_capture_editor(
                    cx,
                    output,
                    &captures,
                    (rect, color),
                    "workflow-area-live",
                );
                let source = std::fs::read_dir(&captures)
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path();
                annotate_capture_roundtrip(cx, editor, &source, scratch);
            } else {
                verify_saved_capture(
                    cx,
                    output,
                    &captures,
                    (rect, color),
                    &format!("workflow-{kind}-{name}"),
                );
            }
            if kind == "area" {
                let path = std::fs::read_dir(&captures)
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path();
                let clipboard = cx.update(|cx| {
                    crate::system::clipboard::ClipboardService::read_image(cx).unwrap()
                });
                assert_eq!(
                    image::load_from_memory(&clipboard.bytes)
                        .unwrap()
                        .to_rgba8(),
                    image::open(path).unwrap().to_rgba8(),
                    "{kind}-{name}: the copied pixels must match the saved capture"
                );
            } else {
                assert_eq!(
                    cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
                    Some(sentinel),
                    "{kind}-{name}: disabled automatic copy must not change the clipboard"
                );
            }
        }
    }

    let service_config = cx.update(|cx| crate::state::state(cx).config.clone());
    let previous_capture_settings = service_config.get();
    let captures = scratch.join("freeze-release-retry");
    std::fs::create_dir_all(&captures).unwrap();
    let release_failed_state = output.join("freeze-release-failed.json");
    let release_succeeded_state = output.join("freeze-release-succeeded.json");
    for path in [&release_failed_state, &release_succeeded_state] {
        let _ = std::fs::remove_file(path);
    }
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "frozen",
            "releaseFailure": "once",
            "rect": SELECTED_RECT,
            "windowId": null,
        }))
        .unwrap(),
    )
    .unwrap();
    let release_retry_before = super::daemon_fixture::calls(output).len();
    take_notifications();
    cx.update(|cx| {
        service_config.update(|config| {
            config.storage.screenshots_path = captures.to_string_lossy().into_owned();
            config.screenshot.freeze_screen = true;
            config.screenshot.hide_desktop_icons = false;
            config.screenshot.show_preview = true;
            config.screenshot.capture_to_clipboard = false;
            config.screenshot.auto_copy_to_clipboard = false;
        });
        crate::capture::start_area_capture(cx);
    });
    cx.run_until_parked();
    let picker = area_picker(cx);
    select_area(cx, picker);
    cx.run_until_parked();
    assert!(!cx.update(crate::capture::overlay::parity_is_active));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(&release_failed_state).unwrap())
            .unwrap()["frozen"],
        true,
        "the fake daemon must retain its frozen state after the first release failure"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(&release_succeeded_state).unwrap()
        )
        .unwrap()["frozen"],
        false,
        "the immediate retry must release the daemon's frozen state"
    );
    assert!(
        take_notifications().is_empty(),
        "successful release retry must not surface a capture failure"
    );
    let release_retry_calls = super::daemon_fixture::calls(output);
    let release_retry_calls = &release_retry_calls[release_retry_before..];
    let freeze_index = release_retry_calls
        .iter()
        .position(|call| call["module"] == "freeze-screen" && call["method"] == "freeze")
        .expect("the screenshot selection must freeze first");
    let capture_index = release_retry_calls
        .iter()
        .position(|call| call["module"] == "screenshot" && call["method"] == "capture-area")
        .expect("the screenshot must be captured");
    assert_eq!(release_retry_calls[capture_index]["params"]["cached"], true);
    let releases: Vec<_> = release_retry_calls
        .iter()
        .enumerate()
        .filter(|(_, call)| call["module"] == "freeze-screen" && call["method"] == "release")
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        releases.len(),
        2,
        "the failed release must be retried immediately"
    );
    assert!(
        freeze_index < capture_index && capture_index < releases[0] && releases[0] < releases[1]
    );
    verify_saved_capture(
        cx,
        scratch,
        &captures,
        (SELECTED_RECT, [64, 112, 192, 255]),
        "post-capture freeze release retry",
    );
    let next_selector_before = super::daemon_fixture::calls(output).len();
    cx.update(|cx| {
        crate::capture::start_area_selection(
            crate::capture::intent::CaptureIntent::ScrollCapture,
            cx,
        )
    });
    cx.run_until_parked();
    assert_eq!(
        cx.update(crate::capture::overlay::active_intents_for_test),
        [crate::capture::intent::CaptureIntent::ScrollCapture],
        "a live selector must open after release retry succeeds"
    );
    assert!(
        !super::daemon_fixture::calls(output)[next_selector_before..]
            .iter()
            .any(|call| call["module"] == "freeze-screen" && call["method"] == "freeze")
    );
    cx.update(crate::capture::overlay::close_all);
    cx.run_until_parked();
    service_config.update(|config| {
        config.storage.screenshots_path =
            previous_capture_settings.storage.screenshots_path.clone();
        config.screenshot.freeze_screen = previous_capture_settings.screenshot.freeze_screen;
        config.screenshot.hide_desktop_icons =
            previous_capture_settings.screenshot.hide_desktop_icons;
        config.screenshot.show_preview = previous_capture_settings.screenshot.show_preview;
        config.screenshot.capture_to_clipboard =
            previous_capture_settings.screenshot.capture_to_clipboard;
        config.screenshot.auto_copy_to_clipboard =
            previous_capture_settings.screenshot.auto_copy_to_clipboard;
    });
    println!("post-capture release failure was retried before the next live selector opened");

    let calls: Vec<serde_json::Value> =
        std::fs::read_to_string(output.join("daemon-fixture.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .filter(|call: &serde_json::Value| call["module"] == "screenshot")
            .collect();
    assert_eq!(
        calls
            .iter()
            .map(|call| call["method"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "capture-window",
            "capture-area",
            "capture-window",
            "capture-window",
            "capture-area",
            "capture-area",
            "capture-area",
            "capture-area",
            "capture-area",
            "capture-area",
            "capture-area",
            "capture-area",
            "capture-area",
        ]
    );

    let second_rect = CaptureRect {
        x: 240,
        y: 180,
        width: 400,
        height: 240,
    };
    let mut first_overlay: Option<image::RgbaImage> = None;
    for (name, rect) in [("first", SELECTED_RECT), ("second", second_rect)] {
        let captures = scratch.join(format!("selection-reset-{name}"));
        std::fs::create_dir_all(&captures).unwrap();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "live", "rect": rect, "windowId": null,
            }))
            .unwrap(),
        )
        .unwrap();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                config.screenshot.freeze_screen = false;
                config.screenshot.show_preview = false;
                config.screenshot.hide_desktop_icons = false;
                config.screenshot.capture_to_clipboard = false;
                config.screenshot.auto_copy_to_clipboard = false;
            });
            crate::capture::start_area_capture(cx);
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        assert_eq!(
            cx.update(crate::capture::overlay::parity_selection),
            None,
            "{name}: a newly opened selector must not carry a previous rectangle"
        );
        let overlay = cx.capture_screenshot(picker).unwrap();
        if let Some(first_overlay) = &first_overlay {
            if first_overlay != &overlay {
                first_overlay
                    .save(scratch.join("selection-reset-first-initial-overlay.png"))
                    .unwrap();
                overlay
                    .save(scratch.join("selection-reset-second-initial-overlay.png"))
                    .unwrap();
            }
            assert!(
                first_overlay == &overlay,
                "a reopened selector must render the same initial overlay pixels"
            );
        } else {
            first_overlay = Some(overlay);
        }
        select_area_rect(cx, picker, rect);
        cx.run_until_parked();
        assert!(!cx.update(crate::capture::overlay::parity_is_active));
        let files: Vec<_> = std::fs::read_dir(&captures)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(files.len(), 1, "{name}: selection should save one capture");
        let saved = image::open(&files[0]).unwrap().to_rgba8();
        assert_eq!(saved.dimensions(), (rect.width as u32, rect.height as u32));
        assert!(saved.pixels().all(|pixel| pixel.0 == [240, 80, 64, 255]));
    }

    #[cfg(windows)]
    {
        use std::io::{Read, Write};

        let captures = scratch.join("capture-held-icons");
        std::fs::create_dir_all(&captures).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "live", "rect": SELECTED_RECT, "windowId": null,
                "desktopHideHeld": true, "expectDesktopHidden": true,
                "barrier": listener.local_addr().unwrap().to_string(),
            }))
            .unwrap(),
        )
        .unwrap();
        let before = super::daemon_fixture::calls(output).len();
        let daemon_output = output.to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.read_exact(&mut [0]).unwrap();
            let captured = super::daemon_fixture::calls(&daemon_output)[before..]
                .iter()
                .any(|call| call["module"] == "screenshot");
            stream.write_all(&[1]).unwrap();
            tx.send(captured).unwrap();
        });
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                config.screenshot.freeze_screen = false;
                config.screenshot.show_preview = true;
                config.screenshot.hide_desktop_icons = true;
                config.screenshot.capture_to_clipboard = false;
                config.screenshot.auto_copy_to_clipboard = false;
            });
            crate::capture::start_area_capture(cx);
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        select_area(cx, picker);
        cx.run_until_parked();
        assert!(
            !rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap(),
            "capture started while the desktop icon hide was held"
        );
        let calls = super::daemon_fixture::calls(output);
        let calls = &calls[before..];
        let position = |module: &str, method: &str| {
            calls
                .iter()
                .position(|call| call["module"] == module && call["method"] == method)
                .unwrap()
        };
        assert!(position("desktop-helper", "hide") < position("screenshot", "capture-area"));
        assert!(position("screenshot", "capture-area") < position("desktop-helper", "show"));
        verify_saved_capture(
            cx,
            scratch,
            &captures,
            (SELECTED_RECT, [240, 80, 64, 255]),
            "workflow-capture-held-icons",
        );
        cx.update(|cx| {
            crate::state::state(cx)
                .config
                .update(|config| config.screenshot.hide_desktop_icons = false);
        });
    }

    #[cfg(windows)]
    for (kind, rect) in [("area", SELECTED_RECT), ("display", display)] {
        use std::sync::atomic::Ordering;

        for name in ["capture", "cancelled"] {
            let captures = scratch.join(format!("{kind}-desktop-icons-{name}"));
            std::fs::create_dir_all(&captures).unwrap();
            std::fs::write(
                output.join("capture-fixture.json"),
                serde_json::to_vec(&serde_json::json!({
                    "freeze": "frozen", "rect": rect, "windowId": null,
                    "frozenDesktopIcon": true, "expectDesktopHidden": true,
                }))
                .unwrap(),
            )
            .unwrap();
            let before = super::daemon_fixture::calls(output).len();
            crate::capture::DUPLICATE_SCREEN_TARGET_FOR_PARITY
                .store(kind == "display", Ordering::SeqCst);
            cx.update(|cx| {
                crate::state::state(cx).config.update(|config| {
                    config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                    config.screenshot.freeze_screen = true;
                    config.screenshot.show_preview = true;
                    config.screenshot.hide_desktop_icons = true;
                    config.screenshot.capture_to_clipboard = false;
                    config.screenshot.auto_copy_to_clipboard = false;
                });
                if kind == "display" {
                    crate::capture::start_screen_capture(cx);
                } else {
                    crate::capture::start_area_capture(cx);
                }
            });
            cx.run_until_parked();
            crate::capture::DUPLICATE_SCREEN_TARGET_FOR_PARITY.store(false, Ordering::SeqCst);
            if kind == "display" {
                assert_eq!(
                    cx.update(crate::capture::overlay::active_intents_for_test)
                        .len(),
                    2
                );
            }
            let picker = area_picker(cx);
            match (name, kind) {
                ("cancelled", _) => super::escape(cx, picker),
                (_, "display") => event(
                    cx,
                    picker,
                    PlatformInput::MouseDown(MouseDownEvent {
                        position: point(px(240.), px(180.)),
                        button: MouseButton::Left,
                        click_count: 1,
                        modifiers: Modifiers::none(),
                        first_mouse: false,
                    }),
                ),
                _ => select_area(cx, picker),
            }
            cx.run_until_parked();
            if name == "capture" {
                verify_saved_capture(
                    cx,
                    scratch,
                    &captures,
                    (rect, [64, 112, 192, 255]),
                    &format!("workflow-{kind}-desktop-icons"),
                );
            } else {
                assert_eq!(std::fs::read_dir(&captures).unwrap().count(), 0);
            }
            let calls = super::daemon_fixture::calls(output);
            let calls = &calls[before..];
            let position = |module: &str, method: &str| {
                calls
                    .iter()
                    .position(|call| call["module"] == module && call["method"] == method)
                    .unwrap()
            };
            assert!(position("desktop-helper", "hide") < position("freeze-screen", "freeze"));
            assert!(position("freeze-screen", "freeze") < position("desktop-helper", "show"));
            if name == "capture" {
                assert!(
                    position("screenshot", "capture-area") < position("desktop-helper", "show")
                );
            }
            assert_eq!(
                calls
                    .iter()
                    .filter(|call| call["module"] == "screenshot")
                    .count(),
                usize::from(name == "capture")
            );
            assert!(!crate::capture::desktop_icons::are_hidden());
        }
    }
    #[cfg(windows)]
    {
        let captures = scratch.join("all-in-one-desktop-icons");
        std::fs::create_dir_all(&captures).unwrap();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen", "rect": display, "windowId": null,
                "frozenDesktopIcon": true, "expectDesktopHidden": true,
            }))
            .unwrap(),
        )
        .unwrap();
        let before = super::daemon_fixture::calls(output).len();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                config.screenshot.freeze_screen = true;
                config.screenshot.show_preview = true;
                config.screenshot.hide_desktop_icons = true;
                config.screenshot.capture_to_clipboard = false;
                config.screenshot.auto_copy_to_clipboard = false;
                config.all_in_one.remember_choices = true;
                config.all_in_one.last_mode = "screenshot".into();
                config.all_in_one.last_targets.screenshot = "screen".into();
            });
            crate::capture::start_all_in_one(cx);
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        event(
            cx,
            picker,
            PlatformInput::MouseDown(MouseDownEvent {
                position: point(px(240.), px(180.)),
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Modifiers::none(),
                first_mouse: false,
            }),
        );
        cx.run_until_parked();
        verify_saved_capture(
            cx,
            scratch,
            &captures,
            (display, [64, 112, 192, 255]),
            "workflow-all-in-one-desktop-icons",
        );
        let calls = super::daemon_fixture::calls(output);
        let calls = &calls[before..];
        let position = |module: &str, method: &str| {
            calls
                .iter()
                .position(|call| call["module"] == module && call["method"] == method)
                .unwrap()
        };
        assert!(position("desktop-helper", "hide") < position("freeze-screen", "freeze"));
        assert!(position("freeze-screen", "freeze") < position("screenshot", "capture-area"));
        assert!(position("screenshot", "capture-area") < position("desktop-helper", "show"));
        assert!(!crate::capture::desktop_icons::are_hidden());
    }
    #[cfg(windows)]
    {
        let captures = scratch.join("all-in-one-switch-record");
        std::fs::create_dir_all(&captures).unwrap();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen",
                "rect": SELECTED_RECT,
                "windowId": null,
                "frozenDesktopIcon": true,
                "expectDesktopHidden": true,
            }))
            .unwrap(),
        )
        .unwrap();
        let before = super::daemon_fixture::calls(output).len();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                config.screenshot.freeze_screen = true;
                config.screenshot.show_preview = true;
                config.screenshot.hide_desktop_icons = true;
                config.screenshot.capture_to_clipboard = false;
                config.screenshot.auto_copy_to_clipboard = false;
                config.all_in_one.remember_choices = true;
                config.all_in_one.last_mode = "screenshot".into();
                config.all_in_one.last_targets.screenshot = "area".into();
                config.all_in_one.last_targets.record = "area".into();
            });
            crate::capture::start_all_in_one(cx);
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        select_all_in_one_mode(cx, picker, crate::capture::all_in_one::Mode::Record);
        assert_eq!(
            cx.update(crate::capture::overlay::active_intents_for_test),
            [crate::capture::intent::CaptureIntent::Recording]
        );
        assert!(crate::capture::desktop_icons::are_hidden());
        select_area(cx, picker);
        wait_for_icon_restoration(cx);
        assert!(cx.update(|cx| crate::windows::registry::is_open(
            crate::windows::registry::WindowKind::RecordingControl,
            cx
        )));
        assert!(!crate::capture::desktop_icons::are_hidden());
        let calls = super::daemon_fixture::calls(output);
        let calls = &calls[before..];
        let position = |module: &str, method: &str| {
            calls
                .iter()
                .position(|call| call["module"] == module && call["method"] == method)
                .unwrap()
        };
        assert!(position("desktop-helper", "hide") < position("freeze-screen", "freeze"));
        assert!(position("freeze-screen", "freeze") < position("desktop-helper", "show"));
        assert!(!calls.iter().any(|call| call["module"] == "screenshot"));
        assert!(!calls
            .iter()
            .any(|call| call["module"] == "screen-recorder" && call["method"] == "start"));
        assert!(
            cx.update(crate::windows::recording_control::RecordingControl::cancel_pre_recording)
        );
        cx.update(crate::capture::overlay::close_all);
        cx.run_until_parked();
    }
    #[cfg(windows)]
    {
        let captures = scratch.join("all-in-one-switch-back-to-screenshot");
        std::fs::create_dir_all(&captures).unwrap();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen",
                "rect": SELECTED_RECT,
                "windowId": null,
                "frozenDesktopIcon": true,
                "expectDesktopHidden": true,
            }))
            .unwrap(),
        )
        .unwrap();
        let before = super::daemon_fixture::calls(output).len();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                config.screenshot.freeze_screen = true;
                config.screenshot.show_preview = true;
                config.screenshot.hide_desktop_icons = true;
                config.screenshot.capture_to_clipboard = false;
                config.screenshot.auto_copy_to_clipboard = false;
                config.all_in_one.remember_choices = true;
                config.all_in_one.last_mode = "screenshot".into();
                config.all_in_one.last_targets.screenshot = "area".into();
                config.all_in_one.last_targets.record = "area".into();
            });
            crate::capture::start_all_in_one(cx);
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        select_all_in_one_mode(cx, picker, crate::capture::all_in_one::Mode::Record);
        select_all_in_one_mode(cx, picker, crate::capture::all_in_one::Mode::Screenshot);
        assert_eq!(
            cx.update(crate::capture::overlay::active_intents_for_test),
            [crate::capture::intent::CaptureIntent::Screenshot]
        );
        select_area(cx, picker);
        cx.run_until_parked();
        let files: Vec<_> = std::fs::read_dir(&captures)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(files.len(), 1, "one screenshot should be saved");
        let saved = image::open(&files[0]).unwrap().to_rgba8();
        assert_eq!(
            saved.dimensions(),
            (SELECTED_RECT.width as u32, SELECTED_RECT.height as u32)
        );
        assert!(
            saved.pixels().all(|pixel| pixel.0 == [64, 112, 192, 255]),
            "the cached screenshot must retain icon-free pixels after switching modes"
        );
        let calls = super::daemon_fixture::calls(output);
        let calls = &calls[before..];
        let position = |module: &str, method: &str| {
            calls
                .iter()
                .position(|call| call["module"] == module && call["method"] == method)
                .unwrap()
        };
        assert!(position("desktop-helper", "hide") < position("freeze-screen", "freeze"));
        assert!(position("freeze-screen", "freeze") < position("screenshot", "capture-area"));
        assert!(position("screenshot", "capture-area") < position("desktop-helper", "show"));
        assert!(!crate::capture::desktop_icons::are_hidden());
        cx.update(|cx| {
            crate::windows::registry::close(
                crate::windows::registry::WindowKind::CapturePreview,
                cx,
            );
            crate::state::state(cx).config.update(|config| {
                config.screenshot.freeze_screen = false;
                config.screenshot.hide_desktop_icons = false;
            });
        });
        cx.run_until_parked();
    }
    #[cfg(windows)]
    for (name, freeze) in [("live", false), ("frozen", true)] {
        let captures = scratch.join(format!("all-in-one-window-icons-{name}"));
        std::fs::create_dir_all(&captures).unwrap();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": if freeze { "frozen" } else { "live" },
                "rect": SELECTED_RECT,
                "windowId": 43,
                "frozenDesktopIcon": true,
                "expectDesktopHidden": true,
            }))
            .unwrap(),
        )
        .unwrap();
        let before = super::daemon_fixture::calls(output).len();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                config.screenshot.freeze_screen = freeze;
                config.screenshot.show_preview = true;
                config.screenshot.hide_desktop_icons = true;
                config.screenshot.capture_to_clipboard = false;
                config.screenshot.auto_copy_to_clipboard = false;
                config.all_in_one.remember_choices = true;
                config.all_in_one.last_mode = "screenshot".into();
                config.all_in_one.last_targets.screenshot = "window".into();
            });
            crate::capture::start_all_in_one(cx);
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        event(
            cx,
            picker,
            PlatformInput::MouseDown(MouseDownEvent {
                position: point(px(240.), px(180.)),
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Modifiers::none(),
                first_mouse: false,
            }),
        );
        cx.run_until_parked();
        verify_saved_capture(
            cx,
            scratch,
            &captures,
            (
                SELECTED_RECT,
                if freeze {
                    [64, 112, 192, 255]
                } else {
                    [240, 80, 64, 255]
                },
            ),
            &format!("workflow-all-in-one-window-icons-{name}"),
        );
        let calls = super::daemon_fixture::calls(output);
        let calls = &calls[before..];
        let position = |module: &str, method: &str| {
            calls
                .iter()
                .position(|call| call["module"] == module && call["method"] == method)
                .unwrap()
        };
        let capture_method = if freeze {
            "capture-area"
        } else {
            "capture-window"
        };
        if freeze {
            assert!(position("desktop-helper", "hide") < position("freeze-screen", "freeze"));
        }
        assert!(position("desktop-helper", "hide") < position("screenshot", capture_method));
        assert!(
            position("screenshot", capture_method) < position("desktop-helper", "show"),
            "{name} window capture must retain the hide lease until pixels are read"
        );
        assert!(!crate::capture::desktop_icons::are_hidden());
    }
    #[cfg(windows)]
    cx.update(|cx| {
        crate::state::state(cx)
            .config
            .update(|config| config.screenshot.hide_desktop_icons = false);
    });

    let captures = scratch.join("capture-collision");
    std::fs::create_dir_all(&captures).unwrap();
    let existing = captures.join("Collision.png");
    std::fs::write(&existing, b"previous capture").unwrap();
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "live", "rect": SELECTED_RECT, "windowId": null,
        }))
        .unwrap(),
    )
    .unwrap();
    let pattern = cx.update(|cx| {
        let state = crate::state::state(cx);
        let pattern = state.config.get().storage.naming_pattern;
        state.config.update(|config| {
            config.storage.screenshots_path = captures.to_string_lossy().into_owned();
            config.storage.naming_pattern = "Collision".into();
            config.screenshot.freeze_screen = false;
            config.screenshot.show_preview = true;
            config.screenshot.capture_to_clipboard = false;
            config.screenshot.auto_copy_to_clipboard = false;
        });
        crate::capture::start_area_capture(cx);
        pattern
    });
    cx.run_until_parked();
    let picker = area_picker(cx);
    select_area(cx, picker);
    cx.run_until_parked();
    assert_eq!(std::fs::read(&existing).unwrap(), b"previous capture");
    assert!(captures.join("Collision (1).png").is_file());
    std::fs::remove_file(&existing).unwrap();
    verify_saved_capture(
        cx,
        scratch,
        &captures,
        (SELECTED_RECT, [240, 80, 64, 255]),
        "workflow-capture-collision",
    );
    std::fs::write(&existing, b"previous capture").unwrap();
    let clipboard_pixels = [70, 120, 180, 255].repeat(20 * 10);
    cx.update(|cx| {
        crate::system::clipboard::ClipboardService::write_rgba(
            cx,
            20,
            10,
            clipboard_pixels.clone(),
        )
        .unwrap();
        crate::editor::open_clipboard(cx);
    });
    cx.run_until_parked();
    assert_eq!(std::fs::read(&existing).unwrap(), b"previous capture");
    let clipboard_path = captures.join("Collision (2).png");
    assert_eq!(
        image::open(&clipboard_path).unwrap().to_rgba8().into_raw(),
        clipboard_pixels
    );
    let editor = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .filter_map(|window| window.downcast::<EditorWindow>())
            .find(|editor| Path::new(&editor.read(cx).unwrap().file_path) == clipboard_path)
            .unwrap()
    });
    cx.update_window(editor.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        crate::state::state(cx)
            .config
            .update(|config| config.storage.naming_pattern = pattern);
    });

    let captures = scratch.join("clipboard-only");
    std::fs::create_dir_all(&captures).unwrap();
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "live", "rect": SELECTED_RECT, "windowId": null,
        }))
        .unwrap(),
    )
    .unwrap();
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.screenshots_path = captures.to_string_lossy().into_owned();
            config.screenshot.freeze_screen = false;
            config.screenshot.show_preview = false;
            config.screenshot.capture_to_clipboard = true;
            config.screenshot.auto_copy_to_clipboard = false;
        });
        crate::system::clipboard::ClipboardService::write_text(cx, "before capture".into());
        crate::capture::start_area_capture(cx);
    });
    cx.run_until_parked();
    let picker = area_picker(cx);
    select_area(cx, picker);
    cx.run_until_parked();
    let files: Vec<_> = std::fs::read_dir(&captures)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1, "clipboard-only capture must be saved");
    let path = &files[0];
    let copied = cx
        .update(|cx| crate::system::clipboard::ClipboardService::read_image(cx))
        .expect("clipboard-only capture must replace the clipboard with an image");
    let saved = image::open(path).unwrap().to_rgba8();
    assert_eq!(saved.dimensions(), (640, 360));
    assert!(saved.pixels().all(|pixel| pixel.0 == [240, 80, 64, 255]));
    assert_eq!(
        image::load_from_memory(&copied.bytes).unwrap().to_rgba8(),
        saved
    );
    assert!(crate::history_store::load_history()
        .iter()
        .any(|item| Path::new(&item.original_path) == path));
    assert!(cx.update(|cx| {
        crate::windows::registry::handle(crate::windows::registry::WindowKind::CapturePreview, cx)
            .is_none()
            && cx
                .windows()
                .into_iter()
                .filter_map(|window| window.downcast::<EditorWindow>())
                .all(|editor| Path::new(&editor.read(cx).unwrap().file_path) != path)
    }));
    let captures = scratch.join("screen-replaces-frozen-selection");
    std::fs::create_dir_all(&captures).unwrap();
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "frozen", "rect": display, "windowId": null,
        }))
        .unwrap(),
    )
    .unwrap();
    let before = super::daemon_fixture::calls(output).len();
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.screenshots_path = captures.to_string_lossy().into_owned();
            config.screenshot.freeze_screen = true;
        });
        crate::capture::start_area_capture(cx);
    });
    cx.run_until_parked();
    assert!(cx.update(crate::capture::overlay::parity_is_active));
    cx.update(crate::capture::start_screen_capture);
    cx.run_until_parked();
    assert!(!cx.update(crate::capture::overlay::parity_is_active));
    let files: Vec<_> = std::fs::read_dir(&captures)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    let saved = image::open(&files[0]).unwrap().to_rgba8();
    assert_eq!(
        saved.dimensions(),
        (display.width as u32, display.height as u32)
    );
    assert!(saved.pixels().all(|pixel| pixel.0 == [240, 80, 64, 255]));
    let copied = cx
        .update(|cx| crate::system::clipboard::ClipboardService::read_image(cx))
        .unwrap();
    assert_eq!(
        image::load_from_memory(&copied.bytes).unwrap().to_rgba8(),
        saved
    );
    let calls = super::daemon_fixture::calls(output);
    let calls = &calls[before..];
    let release = calls
        .iter()
        .position(|call| call["module"] == "freeze-screen" && call["method"] == "release")
        .unwrap();
    let capture = calls
        .iter()
        .position(|call| call["module"] == "screenshot" && call["method"] == "capture-area")
        .unwrap();
    assert!(release < capture);
    assert_eq!(calls[capture]["params"]["cached"], false);
    cx.update(|cx| {
        crate::state::state(cx)
            .config
            .update(|config| config.recording.camera.enabled = false);
        crate::capture::start_area_capture(cx);
    });
    cx.run_until_parked();
    assert!(cx.update(crate::capture::overlay::parity_is_active));
    let before = super::daemon_fixture::calls(output).len();
    cx.update(|cx| {
        use crate::windows::registry::{self, WindowKind};

        crate::capture::start_screen_recording(cx);
        assert!(!crate::capture::overlay::parity_is_active(cx));
        assert!(!registry::is_open(WindowKind::RecordingControl, cx));
    });
    cx.run_until_parked();
    cx.update(|cx| {
        use crate::windows::registry::{self, WindowKind};

        assert!(registry::is_open(WindowKind::RecordingControl, cx));
        registry::close(WindowKind::RecordingControl, cx);
    });
    assert!(super::daemon_fixture::calls(output)[before..]
        .iter()
        .any(|call| call["module"] == "freeze-screen" && call["method"] == "release"));
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.screenshot.show_preview = true;
            config.screenshot.capture_to_clipboard = false;
        });
    });
    println!("selection capture clipboard modes and saved pixels passed");
}

#[cfg(windows)]
pub(super) fn scaled_selection_capture(
    cx: &mut HeadlessAppContext,
    output: &Path,
    scratch: &Path,
    scale: f32,
) {
    let expected = CaptureRect {
        x: -517,
        y: -103,
        width: 517,
        height: 383,
    };
    let captures = scratch.join("scaled-selection");
    std::fs::create_dir_all(&captures).unwrap();
    std::fs::write(
        output.join("capture-fixture.json"),
        serde_json::to_vec(&serde_json::json!({
            "freeze": "live", "rect": expected, "windowId": null, "pattern": "quadrants",
        }))
        .unwrap(),
    )
    .unwrap();
    cx.update(|cx| {
        crate::state::state(cx).config.update(|config| {
            config.storage.screenshots_path = captures.to_string_lossy().into_owned();
            config.screenshot.freeze_screen = false;
            config.screenshot.show_preview = true;
            config.screenshot.hide_desktop_icons = false;
            config.screenshot.capture_to_clipboard = false;
            config.screenshot.auto_copy_to_clipboard = false;
            config.preview.corner = "bottom-right".into();
            config.preview.auto_dismiss = false;
        });
    });
    let bounds = gpui::Bounds::new(
        point(px(-640. / scale), px(-200. / scale)),
        size(px(640. / scale), px(480. / scale)),
    );
    let picker = cx
        .open_window(bounds.size, |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| {
                crate::capture::overlay::AreaOverlay::with_focus(
                    bounds,
                    scale,
                    crate::state::state(cx),
                    crate::capture::intent::CaptureIntent::Screenshot,
                    cx.focus_handle(),
                )
            })
        })
        .unwrap()
        .into();
    settle(cx, picker);
    let start = point(px(123. / scale), px(97. / scale));
    let end = point(px(639. / scale), px(479. / scale));
    event(
        cx,
        picker,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    event(
        cx,
        picker,
        PlatformInput::MouseMove(MouseMoveEvent {
            position: end,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, picker);
    assert!(
        cx.update_window(picker, |_, window, _| {
            window.painted_quads().iter().any(|quad| {
                [
                    (quad.bounds.left().0, 123.),
                    (quad.bounds.top().0, 97.),
                    (quad.bounds.right().0, 640.),
                    (quad.bounds.bottom().0, 480.),
                ]
                .iter()
                .all(|(actual, expected)| (actual - expected).abs() < 0.01)
            })
        })
        .unwrap(),
        "selection frame must reach the display edge at {scale}x"
    );
    event(
        cx,
        picker,
        PlatformInput::MouseUp(MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    cx.run_until_parked();
    let calls = super::daemon_fixture::calls(output);
    let call = calls
        .iter()
        .rev()
        .find(|call| call["module"] == "screenshot" && call["method"] == "capture-area")
        .expect("the selected area must reach the screenshot daemon");
    for (key, value) in [
        ("x", expected.x),
        ("y", expected.y),
        ("width", expected.width),
        ("height", expected.height),
    ] {
        assert_eq!(call["params"][key], value, "capture {key} at {scale}x");
    }
    let files: Vec<_> = std::fs::read_dir(&captures)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    let image = image::open(&files[0]).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (517, 383));
    assert!(cx.update(|cx| cx
        .windows()
        .into_iter()
        .filter_map(|window| window.downcast::<EditorWindow>())
        .all(|editor| Path::new(&editor.read(cx).unwrap().file_path) != files[0])));
    let preview = cx
        .update(|cx| {
            crate::windows::registry::handle(
                crate::windows::registry::WindowKind::CapturePreview,
                cx,
            )
        })
        .expect("capture must open a preview");
    cx.update_window(preview, |_, window, _| window.set_scale_factor(scale))
        .unwrap();
    settle(cx, preview);
    let bounds = super::measured("capture-previews", 0);
    let rendered = cx.capture_screenshot(preview).unwrap();
    for (x, y) in [(1, 1), (3, 1), (1, 3), (3, 3)] {
        let saved = image
            .get_pixel(image.width() * x / 4, image.height() * y / 4)
            .0;
        assert_eq!(
            saved,
            [
                if x == 1 { 240 } else { 40 },
                if y == 1 { 80 } else { 180 },
                64,
                255
            ]
        );
        let preview_x = f32::from(bounds.left()) + f32::from(bounds.size.width) * x as f32 / 4.;
        let preview_y = f32::from(bounds.top()) + f32::from(bounds.size.height) * y as f32 / 4.;
        assert_eq!(
            rendered
                .get_pixel(
                    (preview_x * scale).round() as u32,
                    (preview_y * scale).round() as u32,
                )
                .0,
            saved,
            "preview must match the saved capture at {scale}x ({x}, {y})"
        );
    }
    cx.update(|cx| {
        crate::windows::registry::close(crate::windows::registry::WindowKind::CapturePreview, cx)
    });
    cx.run_until_parked();
    println!("scaled selection, saved crop and preview passed at {scale}x");
}

pub(super) fn verify_saved_capture(
    cx: &mut HeadlessAppContext,
    output: &Path,
    captures: &Path,
    expected: (CaptureRect, [u8; 4]),
    name: &str,
) {
    let editor = open_verified_capture_editor(cx, output, captures, expected, name);
    cx.update_window(editor.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

fn open_verified_capture_editor(
    cx: &mut HeadlessAppContext,
    output: &Path,
    captures: &Path,
    expected: (CaptureRect, [u8; 4]),
    name: &str,
) -> gpui::WindowHandle<EditorWindow> {
    let files: Vec<_> = std::fs::read_dir(captures)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1, "{name}: exactly one capture must be saved");
    let path = &files[0];
    assert_eq!(
        path.parent().unwrap().canonicalize().unwrap(),
        captures.canonicalize().unwrap()
    );
    assert!(
        cx.update(|cx| {
            cx.windows()
                .into_iter()
                .filter_map(|window| window.downcast::<EditorWindow>())
                .all(|editor| Path::new(&editor.read(cx).unwrap().file_path) != path)
        }),
        "{name}: capturing must show a preview without opening an editor"
    );
    let preview = cx
        .update(|cx| {
            crate::windows::registry::handle(
                crate::windows::registry::WindowKind::CapturePreview,
                cx,
            )
        })
        .expect("capturing with previews enabled must open its preview");
    cx.update_window(preview, |_, window, _| window.set_scale_factor(1.))
        .unwrap();
    settle(cx, preview);
    let position = super::measured("capture-previews", 0).center();
    event(
        cx,
        preview,
        PlatformInput::MouseDown(MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 2,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    if cx.update(|cx| cx.windows().contains(&preview)) {
        event(
            cx,
            preview,
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                button: MouseButton::Left,
                click_count: 2,
                modifiers: Modifiers::none(),
            }),
        );
    }
    cx.advance_clock(std::time::Duration::from_millis(300));
    cx.run_until_parked();
    let editor = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .filter_map(|window| window.downcast::<EditorWindow>())
            .find(|editor| Path::new(&editor.read(cx).unwrap().file_path) == path)
            .unwrap()
    });
    let saved = image::open(path).unwrap().to_rgba8();
    assert_eq!(
        saved.dimensions(),
        (expected.0.width as u32, expected.0.height as u32)
    );
    assert!(
        saved.pixels().all(|pixel| pixel.0 == expected.1),
        "{name}: incorrect live or frozen pixels"
    );
    assert!(crate::history_store::load_history()
        .iter()
        .any(|item| Path::new(&item.original_path) == path));
    let preview = cx.update(|cx| editor.read(cx).unwrap().export_png().unwrap());
    assert_eq!(image::load_from_memory(&preview).unwrap().to_rgba8(), saved);
    let window = editor.into();
    cx.update_window(window, |_, window, _| {
        window.resize(size(px(1280.), px(738.)));
        window.set_scale_factor(1.);
    })
    .unwrap();
    settle(cx, window);
    hover(cx, window, 20., 700.);
    capture(cx, window, output, name);
    editor
}

pub(super) fn timer_captures(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path) {
    #[cfg(target_os = "macos")]
    let work_area = cx.update(|cx| {
        let display = cx.primary_display().expect("timer fixture display");
        crate::capture::overlay::physical_rect(
            crate::system::work_area::work_area(display.as_ref()),
            1.0,
        )
    });
    for name in [
        "completed",
        "cancelled",
        "timeout",
        "failed",
        "retry",
        "edge",
    ] {
        let rect = if name == "edge" {
            CaptureRect {
                x: 0,
                y: 90,
                width: 100,
                height: 100,
            }
        } else {
            SELECTED_RECT
        };
        #[cfg(target_os = "macos")]
        let timer_position = {
            use poratake_daemon_common::contract::{TIMER_CONTROL_TOP_MARGIN, TIMER_CONTROL_WIDTH};
            let x = if name == "edge" { work_area.x } else { 330 };
            let right = work_area.x + work_area.width - TIMER_CONTROL_WIDTH;
            [
                x.clamp(work_area.x, right),
                work_area.y + TIMER_CONTROL_TOP_MARGIN,
            ]
        };
        #[cfg(not(target_os = "macos"))]
        let timer_position = [if name == "edge" { 0 } else { 260 }, 40];
        let captures = scratch.join(format!("timer-{name}"));
        std::fs::create_dir_all(&captures).unwrap();
        std::fs::write(
            output.join("capture-fixture.json"),
            serde_json::to_vec(&serde_json::json!({
                "freeze": "frozen", "rect": rect, "windowId": null, "timer": if name == "edge" { "completed" } else { name },
                "timerPosition": timer_position,
                "expectedExcludedWindowIds": cfg!(target_os = "macos")
                    .then_some(crate::capture::overlay::PARITY_OVERLAY_WINDOW_IDS),
            }))
            .unwrap(),
        )
        .unwrap();
        let before = std::fs::read_to_string(output.join("daemon-fixture.jsonl"))
            .unwrap()
            .lines()
            .count();
        cx.update(|cx| {
            crate::state::state(cx).config.update(|config| {
                config.storage.screenshots_path = captures.to_string_lossy().into_owned();
                config.screenshot.freeze_screen = name != "retry";
            });
            crate::capture::start_area_selection(crate::capture::intent::CaptureIntent::Timer, cx);
        });
        cx.run_until_parked();
        let picker = area_picker(cx);
        select_area_rect(cx, picker, rect);
        cx.run_until_parked();
        if name == "timeout" {
            assert!(crate::capture::timer::is_active());
            cx.advance_clock(std::time::Duration::from_secs(10));
            cx.run_until_parked();
        }
        assert!(
            !crate::capture::timer::is_active(),
            "{name}: timer session must settle"
        );
        assert!(
            !cx.update(crate::capture::overlay::parity_is_active),
            "{name}: timer selector must be cleared"
        );
        let saved = matches!(name, "completed" | "retry" | "edge");
        if saved {
            verify_saved_capture(
                cx,
                output,
                &captures,
                (rect, [240, 80, 64, 255]),
                &format!("workflow-timer-{name}"),
            );
        } else {
            assert_eq!(std::fs::read_dir(&captures).unwrap().count(), 0);
            assert!(!crate::history_store::load_history()
                .iter()
                .any(|item| Path::new(&item.original_path).parent() == Some(captures.as_path())));
        }
        let calls: Vec<serde_json::Value> =
            std::fs::read_to_string(output.join("daemon-fixture.jsonl"))
                .unwrap()
                .lines()
                .skip(before)
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
        assert_eq!(
            calls
                .iter()
                .filter(|call| call["module"] == "screenshot")
                .count(),
            usize::from(saved)
        );
        assert_eq!(
            calls
                .iter()
                .filter(|call| call["module"] == "timer-control" && call["method"] == "show")
                .count(),
            1
        );
        assert_eq!(
            calls
                .iter()
                .filter(|call| call["module"] == "timer-control" && call["method"] == "hide")
                .count(),
            usize::from(name != "failed")
        );
    }
}

fn check_redaction_draft(
    cx: &mut HeadlessAppContext,
    editor: gpui::WindowHandle<EditorWindow>,
    draft_x: f64,
    draft_y: f64,
) {
    let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
    let left = f32::from(bounds.left()).round() as u32;
    let top = f32::from(bounds.top()).round() as u32;
    let x = left + ((draft_x as f32 + 8.5) * 1.5).floor() as u32;
    let y = top + ((draft_y as f32 + 8.5) * 1.5).floor() as u32;
    let baseline = cx
        .capture_screenshot(editor.into())
        .unwrap()
        .get_pixel(x, y)
        .0;
    assert!(baseline[..3].iter().any(|channel| *channel > 40));
    for style in ["blur", "pixelate", "blackout"] {
        editor
            .update(cx, |editor, _, cx| {
                editor.draft = Some(crate::editor::annotations::Annotation::Redact {
                    id: "draft-over-redaction".into(),
                    x: draft_x,
                    y: draft_y,
                    width: 24.,
                    height: 16.,
                    style: style.into(),
                    intensity: 5.,
                });
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
        let draft = cx
            .capture_screenshot(editor.into())
            .unwrap()
            .get_pixel(x, y)
            .0;
        let opacity = if style == "blackout" { 0. } else { 0.15 };
        for channel in 0..3 {
            let expected = (f32::from(baseline[channel]) * opacity).round() as u8;
            assert!(
                draft[channel].abs_diff(expected) <= 3,
                "{style} draft at {draft_x},{draft_y}: {draft:?} vs {baseline:?}"
            );
        }
        editor
            .update(cx, |editor, _, cx| {
                editor.draft = None;
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
    }
}

#[allow(clippy::too_many_arguments)]
fn check_inset_reopen(
    cx: &mut HeadlessAppContext,
    editor: gpui::WindowHandle<EditorWindow>,
    scale: f32,
    path: &Path,
    offset: (u32, u32),
    scratch: &Path,
    label: &str,
    zooms: &[f32],
) {
    let flattened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| EditorWindow::from_file(path.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, flattened.into());
    for effective_zoom in zooms.iter().copied() {
        let mut stage_bounds = None;
        for (index, window) in [editor, flattened].into_iter().enumerate() {
            window
                .update(cx, |editor, _, cx| {
                    editor.zoom = effective_zoom / scale;
                    cx.notify();
                })
                .unwrap();
            settle(cx, window.into());
            if index == 0 {
                stage_bounds = Some(super::measured("editor-stage", 0));
            }
        }
        let stage_bounds = stage_bounds.unwrap();
        let (preview_bounds, flattened_bounds) = cx.update(|cx| {
            assert_eq!(editor.read(cx).unwrap().zoom, effective_zoom / scale);
            assert_eq!(flattened.read(cx).unwrap().zoom, effective_zoom / scale);
            (
                editor.read(cx).unwrap().bounds.borrow().unwrap(),
                flattened.read(cx).unwrap().bounds.borrow().unwrap(),
            )
        });
        let preview = cx.capture_screenshot(editor.into()).unwrap();
        let reopened = cx.capture_screenshot(flattened.into()).unwrap();
        let snap = |value: f32| (value.abs() - 0.5).ceil().copysign(value) as u32;
        let left = snap(f32::from(stage_bounds.left()) * scale);
        let top = snap(f32::from(stage_bounds.top()) * scale);
        let flat_left = snap(f32::from(flattened_bounds.left()) * scale);
        let flat_top = snap(f32::from(flattened_bounds.top()) * scale);
        assert_eq!(
            (left, top),
            (flat_left, flat_top),
            "{label} at {effective_zoom}"
        );
        for (actual, expected) in [
            (
                f32::from(preview_bounds.left() - stage_bounds.left()) * scale,
                offset.0 as f32 * effective_zoom,
            ),
            (
                f32::from(preview_bounds.top() - stage_bounds.top()) * scale,
                offset.1 as f32 * effective_zoom,
            ),
        ] {
            assert!(
                (actual - expected).abs() <= 1.0,
                "{label} image offset at {effective_zoom}: {actual} vs {expected}"
            );
        }
        let width = snap(f32::from(flattened_bounds.right()) * scale) - flat_left;
        let height = snap(f32::from(flattened_bounds.bottom()) * scale) - flat_top;
        assert_eq!(
            (
                snap(f32::from(stage_bounds.right()) * scale) - left,
                snap(f32::from(stage_bounds.bottom()) * scale) - top
            ),
            (width, height),
            "{label} stage size at {effective_zoom}"
        );
        if matches!(
            label,
            "rounded-inset-transparent" | "transparent-macos-dark-50"
        ) && effective_zoom == 0.73
        {
            for (name, image, x, y) in [
                ("preview", &preview, left, top),
                ("reopened", &reopened, flat_left, flat_top),
            ] {
                image::imageops::crop_imm(image, x, y, width, height)
                    .to_image()
                    .save(scratch.join(format!("{label}-{name}-crop.png")))
                    .unwrap();
            }
            eprintln!("{label} crop: {left},{top} {width}x{height}");
        }
        let mismatched = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                preview
                    .get_pixel(left + x, top + y)
                    .0
                    .iter()
                    .zip(reopened.get_pixel(flat_left + x, flat_top + y).0)
                    .any(|(actual, expected)| actual.abs_diff(expected) > 2)
            })
            .count();
        if mismatched > 0 {
            preview
                .save(scratch.join(format!("{label}-{effective_zoom}-preview.png")))
                .unwrap();
            reopened
                .save(scratch.join(format!("{label}-{effective_zoom}-reopened.png")))
                .unwrap();
        }
        assert_eq!(mismatched, 0, "{label} reopen at {effective_zoom}x");
        if label == "rounded-inset-transparent" {
            let margin = 8;
            let mismatched = (0..height + margin * 2)
                .flat_map(|y| (0..width + margin * 2).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    preview
                        .get_pixel(left - margin + x, top - margin + y)
                        .0
                        .iter()
                        .zip(
                            reopened
                                .get_pixel(flat_left - margin + x, flat_top - margin + y)
                                .0,
                        )
                        .any(|(actual, expected)| actual.abs_diff(expected) > 2)
                })
                .count();
            if mismatched > 0 {
                preview
                    .save(scratch.join(format!("{label}-{effective_zoom}-outer-preview.png")))
                    .unwrap();
                reopened
                    .save(scratch.join(format!("{label}-{effective_zoom}-outer-reopened.png")))
                    .unwrap();
            }
            assert_eq!(mismatched, 0, "{label} outer pixels at {effective_zoom}x");
        }
    }
    cx.update_window(flattened.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

pub(super) fn fractional_annotation_draft(cx: &mut HeadlessAppContext, scratch: &Path) {
    let source = scratch.join("fractional-draft-source.png");
    image::RgbaImage::from_pixel(80, 60, image::Rgba([40, 120, 180, 255]))
        .save(&source)
        .unwrap();
    let editor = cx
        .open_window(size(px(950.), px(780.)), |window, cx| {
            window.set_scale_factor(2.);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    let shape = crate::editor::annotations::Annotation::Rectangle {
        id: "fractional-draft".into(),
        x: 20.,
        y: 15.,
        width: 30.,
        height: 20.,
        stroke: "#ff0000".into(),
        stroke_width: 1.,
        fill: None,
    };
    editor
        .update(cx, |editor, _, cx| {
            editor.zoom = 2.;
            editor.draft = Some(shape.clone());
            cx.notify();
        })
        .unwrap();
    settle(cx, editor.into());
    let draft = cx.capture_screenshot(editor.into()).unwrap();
    editor
        .update(cx, |editor, _, cx| {
            editor.draft = None;
            editor.history.push(vec![shape]);
            cx.notify();
        })
        .unwrap();
    settle(cx, editor.into());
    let committed = cx.capture_screenshot(editor.into()).unwrap();
    let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
    let left = (f32::from(bounds.left()) * 2.).round() as u32;
    let top = (f32::from(bounds.top()) * 2.).round() as u32;
    let mut differences = 0;
    for y in 0..240 {
        for x in 0..320 {
            let before = draft.get_pixel(left + x, top + y).0;
            let after = committed.get_pixel(left + x, top + y).0;
            differences += usize::from(before.iter().zip(after).any(|(a, b)| a.abs_diff(b) > 2));
        }
    }
    assert_eq!(
        differences, 0,
        "annotation changed between draft and commit"
    );
    cx.update_window(editor.into(), |_, window, _| window.remove_window())
        .unwrap();
}

#[cfg(target_os = "macos")]
pub(super) fn ordered_redaction_reopen(cx: &mut HeadlessAppContext, scratch: &Path) {
    use crate::editor::annotations::Annotation;

    let source = scratch.join("ordered-redaction-source.png");
    image::RgbaImage::from_pixel(200, 160, image::Rgba([255, 255, 255, 255]))
        .save(&source)
        .unwrap();
    for scale in [1., 2.] {
        for zoom in [0.73, 1., 1.37] {
            for style in ["pixelate", "blur"] {
                for shape_after in [false, true] {
                    let editor = cx
                        .open_window(size(px(1280.), px(738.)), |window, cx| {
                            window.set_scale_factor(scale);
                            cx.new(|cx| {
                                EditorWindow::from_file(source.to_str().unwrap(), window, cx)
                            })
                        })
                        .unwrap();
                    let shape = Annotation::Rectangle {
                        id: "stripe".into(),
                        x: 50.,
                        y: 40.,
                        width: 2.,
                        height: 40.,
                        stroke: "#ff0000".into(),
                        stroke_width: 1.,
                        fill: Some("#ff0000".into()),
                    };
                    let redact = Annotation::Redact {
                        id: style.into(),
                        x: 40.,
                        y: 30.,
                        width: 50.,
                        height: 60.,
                        style: style.into(),
                        intensity: 5.,
                    };
                    editor
                        .update(cx, |editor, _, cx| {
                            editor.zoom = zoom;
                            editor.history.push(if shape_after {
                                vec![redact, shape]
                            } else {
                                vec![shape, redact]
                            });
                            cx.notify();
                        })
                        .unwrap();
                    settle(cx, editor.into());
                    let png = cx.update(|cx| editor.read(cx).unwrap().export_png().unwrap());
                    let saved = scratch.join("ordered-redaction-saved.png");
                    std::fs::write(&saved, png).unwrap();
                    let output = image::open(&saved).unwrap().to_rgba8();
                    let stripe = output.get_pixel(51, 60).0;
                    if shape_after {
                        assert_eq!(stripe, [255, 0, 0, 255], "{style}, {scale}x");
                    } else {
                        assert!(stripe[1] > 0, "{style}, {scale}x: {stripe:?}");
                    }
                    let reopened = cx
                        .open_window(size(px(1280.), px(738.)), |window, cx| {
                            window.set_scale_factor(scale);
                            cx.new(|cx| {
                                EditorWindow::from_file(saved.to_str().unwrap(), window, cx)
                            })
                        })
                        .unwrap();
                    reopened
                        .update(cx, |editor, _, cx| {
                            editor.zoom = zoom;
                            cx.notify();
                        })
                        .unwrap();
                    settle(cx, reopened.into());
                    let (bounds, reopened_bounds) = cx.update(|cx| {
                        (
                            editor.read(cx).unwrap().bounds.borrow().unwrap(),
                            reopened.read(cx).unwrap().bounds.borrow().unwrap(),
                        )
                    });
                    assert_eq!(bounds, reopened_bounds);
                    let preview = cx.capture_screenshot(editor.into()).unwrap();
                    let reference = cx.capture_screenshot(reopened.into()).unwrap();
                    let left = (f32::from(bounds.left()) * scale).round() as u32;
                    let top = (f32::from(bounds.top()) * scale).round() as u32;
                    let width = (f32::from(bounds.size.width) * scale).round() as u32;
                    let height = (f32::from(bounds.size.height) * scale).round() as u32;
                    let mut differences = 0;
                    let mut worst = 0;
                    for y in 0..height {
                        for x in 0..width {
                            let actual = preview.get_pixel(left + x, top + y).0;
                            let expected = reference.get_pixel(left + x, top + y).0;
                            let difference = actual
                                .iter()
                                .zip(expected)
                                .map(|(a, b)| a.abs_diff(b))
                                .max()
                                .unwrap();
                            differences += usize::from(difference > 2);
                            worst = worst.max(difference);
                        }
                    }
                    println!("{style}, scale {scale}, zoom {zoom}, shape after {shape_after}: stripe {stripe:?}, {differences} pixels above two, maximum {worst}");
                    if let Some(output) = std::env::var_os("PORATAKE_PARITY_EVIDENCE") {
                        if scale == 2. && zoom == 1. && !shape_after {
                            let output = Path::new(&output);
                            image::imageops::crop_imm(&preview, left, top, width, height)
                                .to_image()
                                .save(output.join(format!("ordered-{style}-preview.png")))
                                .unwrap();
                            image::imageops::crop_imm(&reference, left, top, width, height)
                                .to_image()
                                .save(output.join(format!("ordered-{style}-reopened.png")))
                                .unwrap();
                        }
                    }
                    assert_eq!(
                        differences, 0,
                        "{style}, {scale}x, zoom {zoom}, shape after {shape_after}"
                    );
                    for handle in [editor, reopened] {
                        cx.update_window(handle.into(), |_, window, _| window.remove_window())
                            .unwrap();
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub(super) fn scaled_blackout_reopen(cx: &mut HeadlessAppContext, scratch: &Path) {
    use crate::editor::annotations::Annotation;

    let source = scratch.join("scaled-blackout-source.png");
    image::RgbaImage::from_pixel(200, 160, image::Rgba([255, 255, 255, 255]))
        .save(&source)
        .unwrap();
    let mut failed_cases = 0;
    for scale in [1., 1.25, 2.] {
        for zoom in [0.5, 0.73, 1., 1.37] {
            let editor = cx
                .open_window(size(px(1280.), px(738.)), |window, cx| {
                    window.set_scale_factor(scale);
                    cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
                })
                .unwrap();
            editor
                .update(cx, |editor, _, cx| {
                    editor.zoom = zoom;
                    editor.history.push(vec![
                        Annotation::Rectangle {
                            id: "under-blackout".into(),
                            x: 20.,
                            y: 12.,
                            width: 16.,
                            height: 16.,
                            stroke: "#ff0000".into(),
                            stroke_width: 2.,
                            fill: Some("#ff0000".into()),
                        },
                        Annotation::Redact {
                            id: "scaled-blackout".into(),
                            x: 17.6,
                            y: 7.6,
                            width: 24.6,
                            height: 24.6,
                            style: "blackout".into(),
                            intensity: 5.,
                        },
                    ]);
                    cx.notify();
                })
                .unwrap();
            settle(cx, editor.into());
            let png = cx.update(|cx| editor.read(cx).unwrap().export_png().unwrap());
            let saved = scratch.join("scaled-blackout-saved.png");
            std::fs::write(&saved, png).unwrap();
            let reopened = cx
                .open_window(size(px(1280.), px(738.)), |window, cx| {
                    window.set_scale_factor(scale);
                    cx.new(|cx| EditorWindow::from_file(saved.to_str().unwrap(), window, cx))
                })
                .unwrap();
            reopened
                .update(cx, |editor, _, cx| {
                    editor.zoom = zoom;
                    cx.notify();
                })
                .unwrap();
            settle(cx, reopened.into());
            let (bounds, reopened_bounds) = cx.update(|cx| {
                (
                    editor.read(cx).unwrap().bounds.borrow().unwrap(),
                    reopened.read(cx).unwrap().bounds.borrow().unwrap(),
                )
            });
            assert_eq!(bounds, reopened_bounds);
            let preview = cx.capture_screenshot(editor.into()).unwrap();
            let reference = cx.capture_screenshot(reopened.into()).unwrap();
            let left = (f32::from(bounds.left()) * scale).round() as u32;
            let top = (f32::from(bounds.top()) * scale).round() as u32;
            let width = (f32::from(bounds.size.width) * scale).round() as u32;
            let height = (f32::from(bounds.size.height) * scale).round() as u32;
            let mut differences = 0;
            let mut worst = 0;
            let mut black_preview_lighter_reopen = 0;
            let mut chromatic_preview = 0;
            for y in 0..height {
                for x in 0..width {
                    let actual = preview.get_pixel(left + x, top + y).0;
                    let expected = reference.get_pixel(left + x, top + y).0;
                    let difference = actual
                        .iter()
                        .zip(expected)
                        .map(|(a, b)| a.abs_diff(b))
                        .max()
                        .unwrap();
                    differences += usize::from(difference > 2);
                    worst = worst.max(difference);
                    if actual[..3] == [0, 0, 0] && expected[..3] != [0, 0, 0] {
                        black_preview_lighter_reopen += 1;
                    }
                    if actual[0] > actual[1].saturating_add(30)
                        && actual[0] > actual[2].saturating_add(30)
                    {
                        chromatic_preview += 1;
                    }
                }
            }
            println!("blackout scale {scale}, zoom {zoom}: {differences} pixels above two, maximum {worst}, black preview lighter reopen {black_preview_lighter_reopen}, chromatic preview {chromatic_preview}");
            if scale == 2. && zoom == 1.37 {
                if let Some(output) = std::env::var_os("PORATAKE_PARITY_EVIDENCE") {
                    let output = Path::new(&output);
                    image::imageops::crop_imm(&preview, left, top, width, height)
                        .to_image()
                        .save(output.join("scaled-blackout-preview.png"))
                        .unwrap();
                    image::imageops::crop_imm(&reference, left, top, width, height)
                        .to_image()
                        .save(output.join("scaled-blackout-reopened.png"))
                        .unwrap();
                }
            }
            if differences > 0 {
                failed_cases += 1;
                let preview =
                    image::imageops::crop_imm(&preview, left, top, width, height).to_image();
                let reference =
                    image::imageops::crop_imm(&reference, left, top, width, height).to_image();
                preview
                    .save(scratch.join(format!("blackout-{scale}-{zoom}-preview.png")))
                    .unwrap();
                reference
                    .save(scratch.join(format!("blackout-{scale}-{zoom}-reopened.png")))
                    .unwrap();
            }
            assert_eq!(
                chromatic_preview, 0,
                "underlying shape appeared at scale {scale}, zoom {zoom}"
            );
            let output = image::open(&saved).unwrap().to_rgba8();
            for y in 8..33 {
                for x in 18..43 {
                    assert_eq!(
                        output.get_pixel(x, y).0,
                        [0, 0, 0, 255],
                        "scale {scale}, zoom {zoom}, pixel {x},{y}"
                    );
                }
            }
            for handle in [editor, reopened] {
                cx.update_window(handle.into(), |_, window, _| window.remove_window())
                    .unwrap();
            }
        }
    }
    assert_eq!(
        failed_cases, 0,
        "scaled blackout cases differ from reopened PNG"
    );
    println!("scaled blackout and underlying annotation match reopened PNG at four zooms and three scales");
}

pub(super) fn fractional_annotation_reopen(cx: &mut HeadlessAppContext, scratch: &Path) {
    let source = scratch.join("fractional-annotation-source.png");
    image::RgbaImage::from_fn(800, 740, |_, y| {
        if (y / 60) % 2 == 0 {
            image::Rgba([130, 64, 164, 255])
        } else {
            image::Rgba([52, 90, 120, 255])
        }
    })
    .save(&source)
    .unwrap();
    let editor = cx
        .open_window(size(px(950.), px(780.)), |window, cx| {
            window.set_scale_factor(2.);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    editor
        .update(cx, |editor, _, cx| {
            editor.zoom = 0.89;
            editor
                .history
                .push(vec![crate::editor::annotations::Annotation::Rectangle {
                    id: "fractional-rectangle".into(),
                    x: 145.,
                    y: 121.,
                    width: 217.,
                    height: 183.,
                    stroke: "#ff0000".into(),
                    stroke_width: 2.,
                    fill: None,
                }]);
            cx.notify();
        })
        .unwrap();
    settle(cx, editor.into());
    let png = cx.update(|cx| editor.read(cx).unwrap().export_png().unwrap());
    let saved = scratch.join("fractional-annotation-saved.png");
    std::fs::write(&saved, png).unwrap();
    let reopened = cx
        .open_window(size(px(950.), px(780.)), |window, cx| {
            window.set_scale_factor(2.);
            cx.new(|cx| EditorWindow::from_file(saved.to_str().unwrap(), window, cx))
        })
        .unwrap();
    reopened
        .update(cx, |editor, _, cx| {
            editor.zoom = 0.89;
            cx.notify();
        })
        .unwrap();
    settle(cx, reopened.into());
    let (original_bounds, reopened_bounds) = cx.update(|cx| {
        (
            editor.read(cx).unwrap().bounds.borrow().unwrap(),
            reopened.read(cx).unwrap().bounds.borrow().unwrap(),
        )
    });
    assert_eq!(original_bounds, reopened_bounds);
    let original = cx.capture_screenshot(editor.into()).unwrap();
    let reference = cx.capture_screenshot(reopened.into()).unwrap();
    let left = (f32::from(original_bounds.left()) * 2.).round() as u32;
    let top = (f32::from(original_bounds.top()) * 2.).round() as u32;
    let width = (f32::from(original_bounds.size.width) * 2.).round() as u32;
    let height = (f32::from(original_bounds.size.height) * 2.).round() as u32;
    let mut differences = 0;
    for y in 2..height - 2 {
        for x in 2..width - 2 {
            let actual = original.get_pixel(left + x, top + y).0;
            let expected = reference.get_pixel(left + x, top + y).0;
            differences += usize::from(actual.iter().zip(expected).any(|(a, b)| a.abs_diff(b) > 2));
        }
    }
    assert_eq!(
        differences, 0,
        "fractional annotation differs from reopened export"
    );
    for handle in [editor, reopened] {
        cx.update_window(handle.into(), |_, window, _| window.remove_window())
            .unwrap();
    }
}

pub(super) fn simple_inset_preview(
    cx: &mut HeadlessAppContext,
    scratch: &Path,
    scale: f32,
    padding_setting: f64,
    with_background: bool,
    inset_setting: f64,
    clipped_blur: bool,
) {
    let source = scratch.join(format!(
        "simple-inset-{scale}-{padding_setting}-{with_background}-{inset_setting}-{clipped_blur}.png"
    ));
    let alpha = if std::env::var_os("PORATAKE_PARITY_OPAQUE_SOURCE").is_some() {
        255
    } else {
        128
    };
    image::RgbaImage::from_fn(100, 80, |x, y| {
        if (30..70).contains(&x) && (20..60).contains(&y) {
            image::Rgba([220, 70, 90, alpha])
        } else {
            image::Rgba([40, 120, 180, 255])
        }
    })
    .save(&source)
    .unwrap();
    crate::history_store::add_item(
        HistoryItem {
            id: format!("simple-inset-{scale}-{padding_setting}-{with_background}-{inset_setting}-{clipped_blur}"),
            timestamp: 0,
            original_path: source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: Some(serde_json::json!({"annotations": [
                crate::editor::annotations::Annotation::Rectangle {
                    id: "before-inset-blur".into(),
                    x: 37.,
                    y: 26.,
                    width: 16.,
                    height: 16.,
                    stroke: "#ff00ff".into(),
                    stroke_width: 1.,
                    fill: Some("#ff00ff".into()),
                },
                crate::editor::annotations::Annotation::Redact {
                    id: "inset-blur".into(),
                    x: if clipped_blur { -5. } else { 32. },
                    y: 22.,
                    width: if clipped_blur { 40. } else { 30. },
                    height: 30.,
                    style: "blur".into(),
                    intensity: 2.,
                }
            ]})),
            duration: None,
        },
        100,
        true,
    );
    let editor = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, editor.into());
    editor
        .update(cx, |editor, _, cx| {
            editor.wallpaper.set_gradient(with_background.then(|| {
                crate::editor::wallpaper::GradientOption {
                    id: "inset-background".into(),
                    colors: if scale == 1. && padding_setting == 20. && inset_setting == 8. {
                        vec!["#2468ac".into(), "#de7840".into()]
                    } else {
                        vec!["#2468ac".into()]
                    },
                    angle: 45.,
                }
            }));
            editor.wallpaper.padding = padding_setting;
            editor.wallpaper.inset = inset_setting;
            let mut annotations = editor.history.current().to_vec();
            annotations.push(crate::editor::annotations::Annotation::Rectangle {
                id: "inset-annotation".into(),
                x: if clipped_blur { 18. } else { 42. },
                y: 32.,
                width: 16.,
                height: 16.,
                stroke: "#00ff00".into(),
                stroke_width: 1.,
                fill: Some("#00ff00".into()),
            });
            annotations.push(crate::editor::annotations::Annotation::Redact {
                id: "after-inset-blackout".into(),
                x: if clipped_blur { 22. } else { 46. },
                y: 36.,
                width: 6.,
                height: 6.,
                style: "blackout".into(),
                intensity: 2.,
            });
            annotations.push(crate::editor::annotations::Annotation::Redact {
                id: "after-inset-pixelation".into(),
                x: if clipped_blur { 5. } else { 32. },
                y: 24.,
                width: 8.,
                height: 8.,
                style: "pixelate".into(),
                intensity: 3.,
            });
            editor.history.push(annotations);
            cx.notify();
        })
        .unwrap();
    settle(cx, editor.into());
    editor
        .update(cx, |editor, _, cx| {
            editor.zoom = 1. / scale;
            cx.notify();
        })
        .unwrap();
    settle(cx, editor.into());
    let (bounds, png, matte) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let theme = crate::theme::vars::active_theme(cx);
        let theme = gpui::Rgba::from(theme.background);
        (
            editor.bounds.borrow().unwrap(),
            editor.export_png().unwrap(),
            image::Rgba([
                (theme.r * 255.).round() as u8,
                (theme.g * 255.).round() as u8,
                (theme.b * 255.).round() as u8,
                255,
            ]),
        )
    });
    let saved = image::load_from_memory(&png).unwrap().to_rgba8();
    let native_inset = (inset_setting as f32 * scale).round() as u32;
    let margin = (padding_setting as f32 * scale).round() as u32 + native_inset;
    assert_eq!(saved.dimensions(), (100 + margin * 2, 80 + margin * 2));
    let original = image::open(&source).unwrap().to_rgba8();
    let mut unblurred = image::RgbaImage::from_pixel(
        100,
        80,
        if native_inset > 0 {
            image::Rgba([40, 120, 180, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        },
    );
    image::imageops::overlay(&mut unblurred, &original, 0, 0);
    assert_ne!(
        saved.get_pixel(margin + 33, margin + 23),
        unblurred.get_pixel(33, 23)
    );
    assert_eq!(
        saved
            .get_pixel(margin + if clipped_blur { 24 } else { 48 }, margin + 38)
            .0,
        [0, 0, 0, 255]
    );
    if native_inset > 0 {
        assert_eq!(
            saved.get_pixel(margin - 2, margin + 10).0,
            [40, 120, 180, 255]
        );
    }
    let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
    image::imageops::overlay(&mut expected, &saved, 0, 0);
    let screenshot = cx.capture_screenshot(editor.into()).unwrap();
    let left = (f32::from(bounds.left()) * scale).round() as u32 - margin;
    let top = (f32::from(bounds.top()) * scale).round() as u32 - margin;
    for y in 0..saved.height() {
        for x in 0..saved.width() {
            let actual = screenshot.get_pixel(left + x, top + y).0;
            let expected = expected.get_pixel(x, y).0;
            assert!(
                actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                "inset preview at {scale}x, padding {padding_setting}, inset {inset_setting}, background {with_background}, clipped {clipped_blur}, {x},{y}: {actual:?} vs {expected:?}"
            );
        }
    }
    if [1.25, 2., 1.3333333].contains(&scale) && inset_setting == 8. {
        for (name, corners, shadow) in [
            ("rounded", 24., 0.),
            ("shadow", 0., 50.),
            ("rounded-shadow", 24., 50.),
        ] {
            editor
                .update(cx, |editor, _, cx| {
                    editor.wallpaper.corners = corners;
                    editor.wallpaper.shadow = shadow;
                    cx.notify();
                })
                .unwrap();
            settle(cx, editor.into());
            editor
                .update(cx, |editor, _, cx| {
                    editor.zoom = 1. / scale;
                    cx.notify();
                })
                .unwrap();
            settle(cx, editor.into());
            let (bounds, saved, offset_x, offset_y) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                assert_eq!(editor.zoom, 1. / scale);
                let wallpaper = editor.wallpaper.for_scale(f64::from(scale));
                let (_, (x, y, _, _)) = crate::editor::wallpaper::layout(
                    &wallpaper,
                    100.,
                    80.,
                    f64::from(scale),
                    false,
                );
                (
                    editor.bounds.borrow().unwrap(),
                    image::load_from_memory(&editor.export_png().unwrap())
                        .unwrap()
                        .to_rgba8(),
                    x.round() as u32,
                    y.round() as u32,
                )
            });
            let screenshot = cx.capture_screenshot(editor.into()).unwrap();
            let left = (f32::from(bounds.left()) * scale).round() as u32 - offset_x;
            let top = (f32::from(bounds.top()) * scale).round() as u32 - offset_y;
            assert_eq!(
                saved.dimensions(),
                (100 + margin * 2, 80 + margin * 2),
                "inset {name} must surround the image in the saved PNG"
            );
            if scale == 1.25 && padding_setting == 20. && name == "rounded-shadow" {
                saved
                    .save(scratch.join("rounded-inset-output.png"))
                    .unwrap();
            }
            let preview =
                image::imageops::crop_imm(&screenshot, left, top, saved.width(), saved.height())
                    .to_image();
            let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
            image::imageops::overlay(&mut expected, &saved, 0, 0);
            let mismatched = preview
                .pixels()
                .zip(expected.pixels())
                .filter(|(actual, expected)| {
                    actual
                        .0
                        .iter()
                        .zip(expected.0)
                        .any(|(actual, expected)| actual.abs_diff(expected) > 2)
                })
                .count();
            if mismatched > 0 {
                preview
                    .save(scratch.join(format!(
                        "inset-{name}-{scale}-{padding_setting}-{with_background}-{clipped_blur}-preview.png"
                    )))
                    .unwrap();
                expected
                    .save(scratch.join(format!(
                        "inset-{name}-{scale}-{padding_setting}-{with_background}-{clipped_blur}-expected.png"
                    )))
                    .unwrap();
            }
            assert_eq!(mismatched, 0, "inset {name} preview differs from export at {scale}x, padding {padding_setting}, background {with_background}, clipped blur {clipped_blur}");
            if scale == 1.25 && padding_setting == 20. && name == "rounded-shadow" {
                let path = scratch.join("rounded-inset-output.png");
                check_inset_reopen(
                    cx,
                    editor,
                    scale,
                    &path,
                    (offset_x, offset_y),
                    scratch,
                    if with_background {
                        "rounded-inset"
                    } else {
                        "rounded-inset-transparent"
                    },
                    &[0.73, 1.0, 1.5],
                );
            }
        }
        for style in ["macos-light", "macos-dark", "windows-light", "windows-dark"] {
            for shadow in [0., 50.] {
                editor
                    .update(cx, |editor, _, cx| {
                        editor.wallpaper.window_frame.style = style.into();
                        editor.wallpaper.corners = 0.;
                        editor.wallpaper.shadow = shadow;
                        cx.notify();
                    })
                    .unwrap();
                settle(cx, editor.into());
                editor
                    .update(cx, |editor, _, cx| {
                        editor.zoom = 1. / scale;
                        cx.notify();
                    })
                    .unwrap();
                settle(cx, editor.into());
                let (bounds, saved) = cx.update(|cx| {
                    let editor = editor.read(cx).unwrap();
                    assert_eq!(editor.zoom, 1. / scale);
                    (
                        editor.bounds.borrow().unwrap(),
                        image::load_from_memory(&editor.export_png().unwrap())
                            .unwrap()
                            .to_rgba8(),
                    )
                });
                let title = (28. * scale).round() as u32;
                assert_eq!(
                    saved.dimensions(),
                    (100 + margin * 2, 80 + margin * 2 + title)
                );
                assert_eq!(
                    saved.get_pixel(margin - 2, margin + title + 10).0,
                    [40, 120, 180, 255]
                );
                assert_eq!(
                    saved
                        .get_pixel(
                            margin + if clipped_blur { 24 } else { 48 },
                            margin + title + 38
                        )
                        .0,
                    [0, 0, 0, 255]
                );
                let screenshot = cx.capture_screenshot(editor.into()).unwrap();
                let left = (f32::from(bounds.left()) * scale).round() as u32 - margin;
                let top = (f32::from(bounds.top()) * scale).round() as u32 - margin - title;
                let preview = image::imageops::crop_imm(
                    &screenshot,
                    left,
                    top,
                    saved.width(),
                    saved.height(),
                )
                .to_image();
                let mut expected =
                    image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
                image::imageops::overlay(&mut expected, &saved, 0, 0);
                let mismatched = preview
                    .pixels()
                    .zip(expected.pixels())
                    .filter(|(actual, expected)| {
                        actual
                            .0
                            .iter()
                            .zip(expected.0)
                            .any(|(actual, expected)| actual.abs_diff(expected) > 2)
                    })
                    .count();
                if mismatched > 0 {
                    preview
                        .save(scratch.join("framed-inset-preview.png"))
                        .unwrap();
                    expected
                        .save(scratch.join("framed-inset-expected.png"))
                        .unwrap();
                }
                assert_eq!(mismatched, 0, "framed inset {scale}x, {style}, shadow {shadow}, padding {padding_setting}, background {with_background}, clipped blur {clipped_blur}");
                if [1.25, 2.].contains(&scale)
                    && padding_setting == 20.
                    && !with_background
                    && !clipped_blur
                    && style.starts_with("macos-")
                {
                    if shadow == 0. {
                        assert_eq!(saved.get_pixel(0, 0).0[3], 0);
                    }
                    let path = scratch.join(format!("transparent-{style}-{shadow}.png"));
                    saved.save(&path).unwrap();
                    check_inset_reopen(
                        cx,
                        editor,
                        scale,
                        &path,
                        (margin, margin + title),
                        scratch,
                        &format!("transparent-{style}-{shadow}"),
                        &[0.73, 1.0, 1.5],
                    );
                }
                if scale == 1.25
                    && padding_setting == 20.
                    && with_background
                    && !clipped_blur
                    && style == "windows-dark"
                    && shadow == 50.
                {
                    let path = scratch.join("framed-inset-output.png");
                    saved.save(&path).unwrap();
                    check_inset_reopen(
                        cx,
                        editor,
                        scale,
                        &path,
                        (margin, margin + title),
                        scratch,
                        "framed-inset",
                        &[0.73, 1.0, 1.5],
                    );
                    let (original, original_tool) = cx.update(|cx| {
                        let editor = editor.read(cx).unwrap();
                        (editor.history.current().to_vec(), editor.tool)
                    });
                    editor
                        .update(cx, |editor, _, cx| {
                            let mut annotations = original.clone();
                            annotations.push(crate::editor::annotations::Annotation::Rectangle {
                                id: "framed-inset-selection".into(),
                                x: 2.,
                                y: 2.,
                                width: 32.,
                                height: 24.,
                                stroke: "#ffdb00".into(),
                                stroke_width: 1.,
                                fill: Some("#ffdb00".into()),
                            });
                            editor.history.push(annotations);
                            editor.tool = crate::ui::colors::Tool::Select;
                            editor.zoom = 1. / scale;
                            cx.notify();
                        })
                        .unwrap();
                    settle(cx, editor.into());
                    let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
                    let start = bounds.origin + point(px(18. / scale), px(14. / scale));
                    let end = bounds.origin + point(px(30. / scale), px(20. / scale));
                    super::click(cx, editor.into(), f32::from(start.x), f32::from(start.y));
                    cx.update(|cx| {
                        assert_eq!(
                            editor.read(cx).unwrap().snapshot.borrow().selected,
                            ["framed-inset-selection"]
                        );
                    });
                    let selected = cx.capture_screenshot(editor.into()).unwrap();
                    let foreground = cx.update(|cx| {
                        gpui::Rgba::from(crate::theme::vars::active_theme(cx).primary_foreground)
                    });
                    let left = (f32::from(bounds.left()) * scale).round() as i32 + 2;
                    let top = (f32::from(bounds.top()) * scale).round() as i32 + 2;
                    for (x, y) in [(-4, 0), (0, -4), (-2, -2), (2, 2)] {
                        assert!(
                            selected.get_pixel((left + x) as u32, (top + y) as u32).0[..3]
                                .iter()
                                .zip([foreground.r, foreground.g, foreground.b])
                                .all(|(actual, expected)| (f32::from(*actual) - expected * 255.)
                                    .abs()
                                    <= 2.),
                            "the framed inset selection handle must remain visible at {x},{y}"
                        );
                    }
                    for input in [
                        PlatformInput::MouseDown(MouseDownEvent {
                            position: start,
                            button: MouseButton::Left,
                            click_count: 1,
                            modifiers: Modifiers::none(),
                            first_mouse: false,
                        }),
                        PlatformInput::MouseMove(MouseMoveEvent {
                            position: end,
                            pressed_button: Some(MouseButton::Left),
                            modifiers: Modifiers::none(),
                        }),
                        PlatformInput::MouseUp(MouseUpEvent {
                            position: end,
                            button: MouseButton::Left,
                            click_count: 1,
                            modifiers: Modifiers::none(),
                        }),
                    ] {
                        event(cx, editor.into(), input);
                    }
                    settle(cx, editor.into());
                    cx.update(|cx| {
                        let editor = editor.read(cx).unwrap();
                        let annotation = editor
                            .history
                            .current()
                            .iter()
                            .find(|annotation| annotation.id() == "framed-inset-selection")
                            .unwrap();
                        let (left, top, right, bottom) = annotation.bounds();
                        for (actual, expected) in [left, top, right, bottom]
                            .into_iter()
                            .zip([14., 8., 46., 32.])
                        {
                            assert!(
                                (actual - expected).abs() < 0.01,
                                "framed inset drag: {actual} vs {expected}"
                            );
                        }
                    });
                    super::click(
                        cx,
                        editor.into(),
                        f32::from(bounds.left()) + 95. / scale,
                        f32::from(bounds.top()) + 75. / scale,
                    );
                    let png = cx.update(|cx| {
                        let editor = editor.read(cx).unwrap();
                        assert!(editor.snapshot.borrow().selected.is_empty());
                        editor.export_png().unwrap()
                    });
                    let path = scratch.join("framed-inset-drag.png");
                    std::fs::write(&path, png).unwrap();
                    check_inset_reopen(
                        cx,
                        editor,
                        scale,
                        &path,
                        (margin, margin + title),
                        scratch,
                        "framed-inset-drag",
                        &[0.73, 1.0, 1.5],
                    );
                    editor
                        .update(cx, |editor, _, cx| {
                            editor.history.push(original);
                            editor.tool = original_tool;
                            cx.notify();
                        })
                        .unwrap();
                    settle(cx, editor.into());
                    for (ratio_style, ratio_inset, ratio_shadow) in [
                        ("windows-dark", 8., 50.),
                        ("none", 8., 50.),
                        ("windows-dark", 0., 50.),
                        ("none", 0., 50.),
                        ("windows-dark", 0., 0.),
                    ] {
                        editor
                            .update(cx, |editor, _, cx| {
                                editor.wallpaper.window_frame.style = ratio_style.into();
                                editor.wallpaper.aspect_ratio = "9:16".into();
                                editor.wallpaper.inset = ratio_inset;
                                editor.wallpaper.shadow = ratio_shadow;
                                cx.notify();
                            })
                            .unwrap();
                        settle(cx, editor.into());
                        let (saved, offset) = cx.update(|cx| {
                            let editor = editor.read(cx).unwrap();
                            let wallpaper = editor.wallpaper.for_scale(f64::from(scale));
                            let ((width, height), (x, y, _, _)) = crate::editor::wallpaper::layout(
                                &wallpaper,
                                100.,
                                80.,
                                f64::from(scale),
                                false,
                            );
                            let title = crate::render::window_frame::title_bar_height(
                                ratio_style,
                                f64::from(scale),
                            ) as u32;
                            let inset = (ratio_inset as f32 * scale).round() as u32;
                            let frame_top = y - f64::from(inset);
                            let frame_height = 80. + f64::from(title + inset * 2);
                            assert!(
                                (frame_top - (height - frame_top - frame_height)).abs() < 0.001
                            );
                            let saved = image::load_from_memory(&editor.export_png().unwrap())
                                .unwrap()
                                .to_rgba8();
                            assert_eq!(
                                saved.dimensions(),
                                (width.round() as u32, height.round() as u32)
                            );
                            (saved, (x.round() as u32, y.round() as u32 + title))
                        });
                        let label = format!(
                            "{ratio_style}-inset-{ratio_inset}-shadow-{ratio_shadow}-ratio"
                        );
                        let path = scratch.join(format!("{label}-output.png"));
                        saved.save(&path).unwrap();
                        check_inset_reopen(
                            cx,
                            editor,
                            scale,
                            &path,
                            offset,
                            scratch,
                            &label,
                            &[0.73, 1.0, 1.5],
                        );
                    }
                    editor
                        .update(cx, |editor, _, cx| {
                            editor.wallpaper.shadow = 50.;
                            editor.wallpaper.window_frame.style = "none".into();
                            editor.push_image_layer(
                                source.to_string_lossy().into_owned(),
                                100.,
                                80.,
                                crate::editor::layers::Edge::Right,
                                cx,
                            );
                            cx.notify();
                        })
                        .unwrap();
                    settle(cx, editor.into());
                    let (saved, offset) = cx.update(|cx| {
                        let editor = editor.read(cx).unwrap();
                        let wallpaper = editor.wallpaper.for_scale(f64::from(scale));
                        let group = crate::editor::layers::compute(
                            100.,
                            80.,
                            &editor.snapshot.borrow().layers,
                            wallpaper.spacing.max(0.),
                        );
                        let ((width, height), (x, y, _, _)) = crate::editor::wallpaper::layout(
                            &wallpaper,
                            group.width,
                            group.height,
                            f64::from(scale),
                            true,
                        );
                        assert!((y - (height - y - group.height)).abs() < 0.001);
                        let saved = image::load_from_memory(&editor.export_png().unwrap())
                            .unwrap()
                            .to_rgba8();
                        assert_eq!(
                            saved.dimensions(),
                            (width.round() as u32, height.round() as u32)
                        );
                        (
                            saved,
                            (
                                (x.round() + group.primary.x) as u32,
                                (y.round() + group.primary.y) as u32,
                            ),
                        )
                    });
                    let path = scratch.join("attachment-ratio-output.png");
                    saved.save(&path).unwrap();
                    check_inset_reopen(
                        cx,
                        editor,
                        scale,
                        &path,
                        offset,
                        scratch,
                        "attachment-ratio",
                        &[0.73, 1.0, 1.5],
                    );
                    editor
                        .update(cx, |editor, _, cx| {
                            editor.clear_layers(cx);
                        })
                        .unwrap();
                    settle(cx, editor.into());
                }
            }
        }
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.corners = 0.;
                editor.wallpaper.shadow = 0.;
                editor.wallpaper.window_frame.style = "none".into();
                editor.wallpaper.aspect_ratio = "auto".into();
                editor.wallpaper.inset = inset_setting;
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
    }
    let flattened_path = scratch.join("simple-inset-blur-flattened.png");
    std::fs::write(&flattened_path, &png).unwrap();
    let flattened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| EditorWindow::from_file(flattened_path.to_str().unwrap(), window, cx))
        })
        .unwrap();
    flattened
        .update(cx, |editor, _, cx| {
            editor
                .wallpaper
                .set_gradient(Some(crate::editor::wallpaper::GradientOption {
                    id: "zoom-matte".into(),
                    colors: vec![format!(
                        "#{:02x}{:02x}{:02x}",
                        matte.0[0], matte.0[1], matte.0[2]
                    )],
                    angle: 0.,
                }));
            editor.wallpaper.padding = 0.;
            cx.notify();
        })
        .unwrap();
    settle(cx, flattened.into());
    for effective_zoom in [0.5, 0.73, 1.37, 1.5] {
        let zoom = effective_zoom / scale;
        for window in [editor, flattened] {
            window
                .update(cx, |editor, _, cx| {
                    editor.zoom = zoom;
                    cx.notify();
                })
                .unwrap();
            settle(cx, window.into());
        }
        let (preview_bounds, flattened_bounds) = cx.update(|cx| {
            (
                editor.read(cx).unwrap().bounds.borrow().unwrap(),
                flattened.read(cx).unwrap().bounds.borrow().unwrap(),
            )
        });
        let preview = cx.capture_screenshot(editor.into()).unwrap();
        let flattened_image = cx.capture_screenshot(flattened.into()).unwrap();
        let snap = |value: f32| (value.abs() - 0.5).ceil().copysign(value) as u32;
        let margin_offset = snap(margin as f32 * zoom * scale);
        let left = snap(f32::from(preview_bounds.left()) * scale) - margin_offset;
        let top = snap(f32::from(preview_bounds.top()) * scale) - margin_offset;
        let flat_left = snap(f32::from(flattened_bounds.left()) * scale);
        let flat_top = snap(f32::from(flattened_bounds.top()) * scale);
        if scale == 1. && padding_setting == 0. && !with_background && inset_setting == 8. {
            let width = (saved.width() as f32 * effective_zoom).round() as u32;
            let height = (saved.height() as f32 * effective_zoom).round() as u32;
            image::imageops::crop_imm(&preview, left, top, width, height)
                .to_image()
                .save(scratch.join(format!(
                    "blur-zoom-{effective_zoom}-{clipped_blur}-preview.png"
                )))
                .unwrap();
            image::imageops::crop_imm(&flattened_image, flat_left, flat_top, width, height)
                .to_image()
                .save(scratch.join(format!(
                    "blur-zoom-{effective_zoom}-{clipped_blur}-saved.png"
                )))
                .unwrap();
        }
        let mut worst = (0, 0, 0, [0; 4], [0; 4]);
        let mut mismatched = 0;
        let (start_x, end_x) = if clipped_blur { (0, 36) } else { (31, 63) };
        for y in (((margin + 21) as f32 * effective_zoom).ceil() as u32)
            ..(((margin + 53) as f32 * effective_zoom).floor() as u32)
        {
            for x in (((margin + start_x) as f32 * effective_zoom).ceil() as u32)
                ..(((margin + end_x) as f32 * effective_zoom).floor() as u32)
            {
                let actual = preview.get_pixel(left + x, top + y).0;
                let expected = flattened_image.get_pixel(flat_left + x, flat_top + y).0;
                let difference = actual
                    .iter()
                    .zip(expected)
                    .map(|(a, b)| a.abs_diff(b))
                    .max()
                    .unwrap();
                if difference > 2 {
                    mismatched += 1;
                }
                if difference > worst.0 {
                    worst = (difference, x, y, actual, expected);
                }
            }
        }
        assert_eq!(
                mismatched, 0,
                "blur at {effective_zoom}x, DPI {scale}, padding {padding_setting}, background {with_background}, inset {inset_setting}, clipped {clipped_blur}: worst {worst:?}"
            );
    }
    cx.update_window(flattened.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    if scale == 1. && padding_setting == 20. && with_background && inset_setting == 8. {
        let annotations = cx.update(|cx| editor.read(cx).unwrap().history.current().to_vec());
        let mut without_redactions = annotations
            .iter()
            .filter(|annotation| {
                !matches!(
                    annotation,
                    crate::editor::annotations::Annotation::Redact { .. }
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        without_redactions.push(crate::editor::annotations::Annotation::Rectangle {
            id: "outside-image".into(),
            x: -6.,
            y: 24.,
            width: 12.,
            height: 12.,
            stroke: "#ff00ff".into(),
            stroke_width: 1.,
            fill: Some("#ff00ff".into()),
        });
        let shape = crate::editor::annotations::Annotation::Rectangle {
            id: "ordered-shape".into(),
            x: 30.,
            y: 20.,
            width: 24.,
            height: 24.,
            stroke: "#ff00ff".into(),
            stroke_width: 1.,
            fill: Some("#ff00ff".into()),
        };
        let blur = crate::editor::annotations::Annotation::Redact {
            id: "ordered-blur".into(),
            x: 20.,
            y: 15.,
            width: 40.,
            height: 40.,
            style: "blur".into(),
            intensity: 2.,
        };
        let blackout = crate::editor::annotations::Annotation::Redact {
            id: "ordered-blackout".into(),
            x: 32.,
            y: 22.,
            width: 20.,
            height: 20.,
            style: "blackout".into(),
            intensity: 2.,
        };
        let highlight = crate::editor::annotations::Annotation::Highlight {
            id: "ordered-highlight".into(),
            points: vec![25., 30., 55., 30.],
            fill: "#ffff00".into(),
            opacity: 0.5,
            stroke_width: 10.,
        };
        for (restored, draft_check, selection, current) in [
            (false, false, None, without_redactions),
            (false, true, None, Vec::new()),
            (false, false, None, Vec::new()),
            (
                false,
                false,
                None,
                vec![crate::editor::annotations::Annotation::Redact {
                    id: "outside-image-blackout".into(),
                    x: -6.,
                    y: 24.,
                    width: 12.,
                    height: 12.,
                    style: "blackout".into(),
                    intensity: 2.,
                }],
            ),
            (
                false,
                false,
                None,
                vec![crate::editor::annotations::Annotation::Redact {
                    id: "fractional-boundary-blackout".into(),
                    x: 32.8,
                    y: 22.8,
                    width: 20.3,
                    height: 20.3,
                    style: "blackout".into(),
                    intensity: 2.,
                }],
            ),
            (
                false,
                false,
                Some("ordered-blur"),
                vec![shape.clone(), blur],
            ),
            (
                false,
                false,
                Some("ordered-blackout"),
                vec![shape.clone(), blackout.clone()],
            ),
            (
                false,
                false,
                Some("ordered-shape"),
                vec![blackout, shape.clone()],
            ),
            (false, false, Some("ordered-shape"), vec![highlight, shape]),
            (true, false, None, annotations),
        ] {
            let unannotated_baseline = current.is_empty() && !draft_check;
            let fractional_boundary = current
                .iter()
                .any(|item| item.id() == "fractional-boundary-blackout");
            let zoomed_blackout_order = current.iter().any(|item| item.id() == "ordered-blackout")
                && current.iter().any(|item| item.id() == "ordered-shape");
            editor
                .update(cx, |editor, _, cx| {
                    editor.history.push(current);
                    editor.zoom = 1. / scale;
                    cx.notify();
                })
                .unwrap();
            settle(cx, editor.into());
            let (bounds, current_png) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                assert!(editor.snapshot.borrow().backdrop_composite);
                (
                    editor.bounds.borrow().unwrap(),
                    editor.export_png().unwrap(),
                )
            });
            if restored {
                assert_eq!(current_png, png);
            }
            let saved = image::load_from_memory(&current_png).unwrap().to_rgba8();
            if fractional_boundary {
                assert_eq!(saved.get_pixel(margin + 32, margin + 32).0, [0, 0, 0, 255]);
                assert_eq!(saved.get_pixel(margin + 53, margin + 32).0, [0, 0, 0, 255]);
                assert_eq!(saved.get_pixel(margin + 40, margin + 43).0, [0, 0, 0, 255]);
            }
            let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
            image::imageops::overlay(&mut expected, &saved, 0, 0);
            let screenshot = cx.capture_screenshot(editor.into()).unwrap();
            let left = (f32::from(bounds.left()) * scale).round() as u32 - margin;
            let top = (f32::from(bounds.top()) * scale).round() as u32 - margin;
            for y in 0..saved.height() {
                for x in 0..saved.width() {
                    let actual = screenshot.get_pixel(left + x, top + y).0;
                    let expected = expected.get_pixel(x, y).0;
                    assert!(
                        actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                        "redaction transition {restored} at {x},{y}: {actual:?} vs {expected:?}"
                    );
                }
            }
            if zoomed_blackout_order || fractional_boundary || unannotated_baseline {
                let order_label = if unannotated_baseline {
                    "unannotated"
                } else {
                    selection.unwrap_or("fractional-boundary")
                };
                let flattened_path = scratch.join("ordered-blackout-flattened.png");
                std::fs::write(&flattened_path, &current_png).unwrap();
                let flattened = cx
                    .open_window(size(px(1280.), px(738.)), |window, cx| {
                        window.set_scale_factor(scale);
                        cx.new(|cx| {
                            EditorWindow::from_file(flattened_path.to_str().unwrap(), window, cx)
                        })
                    })
                    .unwrap();
                settle(cx, flattened.into());
                let zooms: &[f32] = if fractional_boundary {
                    &[0.5, 0.73, 1.5]
                } else {
                    &[0.73, 1.5]
                };
                for &zoom in zooms {
                    for window in [editor, flattened] {
                        window
                            .update(cx, |editor, _, cx| {
                                editor.zoom = zoom;
                                cx.notify();
                            })
                            .unwrap();
                        settle(cx, window.into());
                    }
                    let (preview_bounds, flattened_bounds) = cx.update(|cx| {
                        (
                            editor.read(cx).unwrap().bounds.borrow().unwrap(),
                            flattened.read(cx).unwrap().bounds.borrow().unwrap(),
                        )
                    });
                    let preview = cx.capture_screenshot(editor.into()).unwrap();
                    let flattened_image = cx.capture_screenshot(flattened.into()).unwrap();
                    let snap = |value: f32| (value.abs() - 0.5).ceil().copysign(value) as u32;
                    let left = snap(f32::from(preview_bounds.left())) - snap(margin as f32 * zoom);
                    let top = snap(f32::from(preview_bounds.top())) - snap(margin as f32 * zoom);
                    let flat_left = snap(f32::from(flattened_bounds.left()));
                    let flat_top = snap(f32::from(flattened_bounds.top()));
                    assert_eq!(
                        (left, top),
                        (flat_left, flat_top),
                        "ordered blackout {order_label} at {zoom}x"
                    );
                    if unannotated_baseline && zoom == 0.73 {
                        let width = ((100 + 2 * margin) as f32 * zoom).round() as u32;
                        let height = ((80 + 2 * margin) as f32 * zoom).round() as u32;
                        image::imageops::crop_imm(&preview, left, top, width, height)
                            .to_image()
                            .save(scratch.join("scaled-backdrop-preview.png"))
                            .unwrap();
                        image::imageops::crop_imm(
                            &flattened_image,
                            flat_left,
                            flat_top,
                            width,
                            height,
                        )
                        .to_image()
                        .save(scratch.join("scaled-backdrop-reopened.png"))
                        .unwrap();
                    }
                    let (x_range, y_range) = if unannotated_baseline {
                        (
                            0..((100 + 2 * margin) as f32 * zoom).round() as u32,
                            0..((80 + 2 * margin) as f32 * zoom).round() as u32,
                        )
                    } else {
                        (
                            ((margin + 29) as f32 * zoom).ceil() as u32
                                ..((margin + 55) as f32 * zoom).floor() as u32,
                            ((margin + 19) as f32 * zoom).ceil() as u32
                                ..((margin + 45) as f32 * zoom).floor() as u32,
                        )
                    };
                    for y in y_range {
                        for x in x_range.clone() {
                            let actual = preview.get_pixel(left + x, top + y).0;
                            let expected = flattened_image.get_pixel(flat_left + x, flat_top + y).0;
                            if actual.iter().zip(expected).any(|(a, e)| a.abs_diff(e) > 2) {
                                preview
                                    .save(scratch.join(format!(
                                        "ordered-blackout-preview-{order_label}-{zoom}.png"
                                    )))
                                    .unwrap();
                                flattened_image
                                    .save(scratch.join(format!(
                                        "ordered-blackout-reopened-{order_label}-{zoom}.png"
                                    )))
                                    .unwrap();
                            }
                            assert!(
                                actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                                "ordered blackout {order_label} at {zoom}x, {x},{y}: {actual:?} vs {expected:?}, bounds {preview_bounds:?} vs {flattened_bounds:?}, origins ({left},{top}) vs ({flat_left},{flat_top})"
                            );
                        }
                    }
                }
                cx.update_window(flattened.into(), |_, window, _| window.remove_window())
                    .unwrap();
                editor
                    .update(cx, |editor, _, cx| {
                        editor.zoom = 1.;
                        cx.notify();
                    })
                    .unwrap();
                settle(cx, editor.into());
            }
            if let Some(selection) = selection {
                editor
                    .update(cx, |editor, _, cx| {
                        editor.tool = crate::ui::colors::Tool::Select;
                        cx.notify();
                    })
                    .unwrap();
                super::click(
                    cx,
                    editor.into(),
                    f32::from(bounds.left()) + 40.,
                    f32::from(bounds.top()) + 30.,
                );
                settle(cx, editor.into());
                cx.update(|cx| {
                    assert_eq!(
                        editor.read(cx).unwrap().snapshot.borrow().selected,
                        [selection]
                    );
                });
                super::click(
                    cx,
                    editor.into(),
                    f32::from(bounds.left()) + 90.,
                    f32::from(bounds.top()) + 70.,
                );
                settle(cx, editor.into());
            }
            if draft_check {
                for zoom in [1_f32, 1.5] {
                    editor
                        .update(cx, |editor, _, cx| {
                            editor.zoom = zoom / scale;
                            cx.notify();
                        })
                        .unwrap();
                    settle(cx, editor.into());
                    let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
                    let left =
                        (f32::from(bounds.left()) * scale - margin as f32 * zoom).round() as u32;
                    let top =
                        (f32::from(bounds.top()) * scale - margin as f32 * zoom).round() as u32;
                    let before = cx.capture_screenshot(editor.into()).unwrap();
                    if zoom == 1.5 {
                        let reference_path = scratch.join("draft-reference.png");
                        std::fs::write(&reference_path, &current_png).unwrap();
                        let reference = cx
                            .open_window(size(px(1280.), px(738.)), |window, cx| {
                                window.set_scale_factor(scale);
                                cx.new(|cx| {
                                    EditorWindow::from_file(
                                        reference_path.to_str().unwrap(),
                                        window,
                                        cx,
                                    )
                                })
                            })
                            .unwrap();
                        reference
                            .update(cx, |editor, _, cx| {
                                editor.wallpaper.set_gradient(None);
                                editor.wallpaper.padding = 0.;
                                editor.zoom = zoom / scale;
                                cx.notify();
                            })
                            .unwrap();
                        settle(cx, reference.into());
                        let reference_bounds =
                            cx.update(|cx| reference.read(cx).unwrap().bounds.borrow().unwrap());
                        let reference_image = cx.capture_screenshot(reference.into()).unwrap();
                        let ref_left = (f32::from(reference_bounds.left()) * scale).round() as u32;
                        let ref_top = (f32::from(reference_bounds.top()) * scale).round() as u32;
                        let left_edge = (margin as f32 * zoom).round() as u32;
                        let right_edge = ((margin + 100) as f32 * zoom).round() as u32 - 1;
                        let bottom_edge = ((margin + 80) as f32 * zoom).round() as u32 - 1;
                        let inner = ((margin + 10) as f32 * zoom).round() as u32;
                        for (x, y) in [
                            (left_edge, inner),
                            (right_edge, inner),
                            (inner, bottom_edge),
                        ] {
                            let actual = before.get_pixel(left + x, top + y).0;
                            let expected = reference_image.get_pixel(ref_left + x, ref_top + y).0;
                            assert!(
                                actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                                "unannotated image edge differs from its reopened export at {x},{y}: {actual:?} vs {expected:?}"
                            );
                        }
                        for y in ((margin + 10) as f32 * zoom).round() as u32
                            ..((margin + 70) as f32 * zoom).round() as u32
                        {
                            for x in ((margin + 20) as f32 * zoom).round() as u32
                                ..((margin + 80) as f32 * zoom).round() as u32
                            {
                                let actual = before.get_pixel(left + x, top + y).0;
                                let expected =
                                    reference_image.get_pixel(ref_left + x, ref_top + y).0;
                                assert!(
                                    actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                                    "translucent image differs from its reopened export at {x},{y}: {actual:?} vs {expected:?}"
                                );
                            }
                        }
                        cx.update_window(reference.into(), |_, window, _| window.remove_window())
                            .unwrap();
                    }
                    editor
                        .update(cx, |editor, _, cx| {
                            editor.draft =
                                Some(crate::editor::annotations::Annotation::Rectangle {
                                    id: "draft-outside-image".into(),
                                    x: -6.,
                                    y: 24.,
                                    width: 12.,
                                    height: 12.,
                                    stroke: "#ff00ff".into(),
                                    stroke_width: 1.,
                                    fill: Some("#ff00ff".into()),
                                });
                            cx.notify();
                        })
                        .unwrap();
                    settle(cx, editor.into());
                    cx.update(|cx| {
                        assert!(
                            editor
                                .read(cx)
                                .unwrap()
                                .snapshot
                                .borrow()
                                .backdrop_composite
                        );
                    });
                    let screenshot = cx.capture_screenshot(editor.into()).unwrap();
                    assert_eq!(
                        screenshot
                            .get_pixel(
                                left + ((margin - 3) as f32 * zoom).round() as u32,
                                top + ((margin + 30) as f32 * zoom).round() as u32
                            )
                            .0,
                        [255, 0, 255, 255]
                    );
                    let mut changed = 0;
                    let mut first = None;
                    for y in 0..(saved.height() as f32 * zoom).round() as u32 {
                        for x in 0..(saved.width() as f32 * zoom).round() as u32 {
                            if ((margin - 8) as f32 * zoom).floor() as u32 <= x
                                && x < ((margin + 8) as f32 * zoom).ceil() as u32
                                && ((margin + 22) as f32 * zoom).floor() as u32 <= y
                                && y < ((margin + 38) as f32 * zoom).ceil() as u32
                            {
                                continue;
                            }
                            let actual = screenshot.get_pixel(left + x, top + y).0;
                            let previous = before.get_pixel(left + x, top + y).0;
                            if actual.iter().zip(previous).any(|(a, b)| a.abs_diff(b) > 2) {
                                changed += 1;
                                first.get_or_insert((x, y, actual, previous));
                            }
                        }
                    }
                    if changed > 0 {
                        before
                            .save(scratch.join(format!("draft-before-{zoom}.png")))
                            .unwrap();
                        screenshot
                            .save(scratch.join(format!("draft-after-{zoom}.png")))
                            .unwrap();
                    }
                    assert_eq!(
                        changed, 0,
                        "draft at {zoom}x changed pixels outside its bounds: {first:?}"
                    );
                    editor
                        .update(cx, |editor, _, cx| {
                            editor.draft = None;
                            cx.notify();
                        })
                        .unwrap();
                    settle(cx, editor.into());
                }
            }
        }
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.aspect_ratio = "4:3".into();
                editor
                    .history
                    .push(vec![crate::editor::annotations::Annotation::Redact {
                        id: "fractional-offset-blur".into(),
                        x: -5.,
                        y: 22.,
                        width: 40.,
                        height: 30.,
                        style: "blur".into(),
                        intensity: 2.,
                    }]);
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
        let (bounds, current_png, offset) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            assert_eq!(editor.zoom, 1. / scale);
            let (_, (x, y, _, _)) = crate::editor::wallpaper::layout(
                &editor.wallpaper.for_scale(f64::from(scale)),
                100.,
                80.,
                f64::from(scale),
                false,
            );
            assert_ne!(x, x.round());
            (
                editor.bounds.borrow().unwrap(),
                editor.export_png().unwrap(),
                (x.round() as u32, y.round() as u32),
            )
        });
        let saved = image::load_from_memory(&current_png).unwrap().to_rgba8();
        assert!(saved.pixels().all(|pixel| pixel.0[3] == 255));
        let flattened_path = scratch.join("fractional-wallpaper-flattened.png");
        std::fs::write(&flattened_path, &current_png).unwrap();
        let flattened = cx
            .open_window(size(px(1280.), px(738.)), |window, cx| {
                window.set_scale_factor(scale);
                cx.new(|cx| EditorWindow::from_file(flattened_path.to_str().unwrap(), window, cx))
            })
            .unwrap();
        settle(cx, flattened.into());
        flattened
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        settle(cx, flattened.into());
        let flattened_bounds = cx.update(|cx| {
            let flattened = flattened.read(cx).unwrap();
            assert_eq!(flattened.zoom, 1. / scale);
            flattened.bounds.borrow().unwrap()
        });
        let screenshot = cx.capture_screenshot(editor.into()).unwrap();
        let expected = cx.capture_screenshot(flattened.into()).unwrap();
        let left = f32::from(bounds.left()).round() as u32 - offset.0;
        let top = f32::from(bounds.top()).round() as u32 - offset.1;
        let flat_left = f32::from(flattened_bounds.left()).round() as u32;
        let flat_top = f32::from(flattened_bounds.top()).round() as u32;
        assert_eq!((left, top), (flat_left, flat_top));
        assert_eq!(
            (
                (f32::from(flattened_bounds.size.width) * scale).round() as u32,
                (f32::from(flattened_bounds.size.height) * scale).round() as u32,
            ),
            saved.dimensions()
        );
        let mut mismatched = 0;
        let mut first = None;
        for y in 0..saved.height() {
            for x in 0..saved.width() {
                let actual = screenshot.get_pixel(left + x, top + y).0;
                let expected = expected.get_pixel(flat_left + x, flat_top + y).0;
                if actual.iter().zip(expected).any(|(a, e)| a.abs_diff(e) > 2) {
                    mismatched += 1;
                    first.get_or_insert((x, y, actual, expected));
                }
            }
        }
        if mismatched > 0 {
            image::imageops::crop_imm(&screenshot, left, top, saved.width(), saved.height())
                .to_image()
                .save(scratch.join("fractional-wallpaper-preview.png"))
                .unwrap();
            image::imageops::crop_imm(
                &expected,
                flat_left,
                flat_top,
                saved.width(),
                saved.height(),
            )
            .to_image()
            .save(scratch.join("fractional-wallpaper-expected.png"))
            .unwrap();
        }
        assert_eq!(mismatched, 0, "fractional wallpaper offset: {first:?}");
        cx.update_window(flattened.into(), |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
    }
    if scale == 1.
        && padding_setting == 0.
        && !with_background
        && inset_setting == 8.
        && !clipped_blur
    {
        check_redaction_draft(cx, editor, 36., 26.);
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.inset = 0.;
                editor.push_image_layer(
                    source.to_string_lossy().into_owned(),
                    100.,
                    80.,
                    crate::editor::layers::Edge::Right,
                    cx,
                );
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 1.5;
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
        cx.update(|cx| {
            assert!(
                editor
                    .read(cx)
                    .unwrap()
                    .snapshot
                    .borrow()
                    .backdrop_composite
            );
        });
        check_redaction_draft(cx, editor, 36., 26.);
    }
    cx.update_window(editor.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

pub(super) fn balance_restoration(
    cx: &mut HeadlessAppContext,
    scratch: &Path,
    scale: f32,
    padding_setting: f64,
    with_background: bool,
    inset: f64,
) {
    let source = scratch.join(format!(
        "balance-restoration-{scale}-{padding_setting}-{with_background}-{inset}.png"
    ));
    let image = image::RgbaImage::from_fn(200, 160, |x, y| {
        if (100..120).contains(&x) && (85..105).contains(&y) {
            image::Rgba([64, 112, 192, 128])
        } else if (40..160).contains(&x) && (30..130).contains(&y) {
            image::Rgba([64, 112, 192, 255])
        } else if (5..195).contains(&x) && (5..155).contains(&y) {
            image::Rgba([230, 230, 230, 255])
        } else {
            image::Rgba([255, 255, 255, 255])
        }
    });
    image.save(&source).unwrap();
    let expected = crate::render::color_detection::ContentBounds {
        left: 30,
        top: 20,
        right: 30,
        bottom: 20,
    };
    assert_eq!(
        crate::editor::export::balance_bounds(&image),
        Some(expected)
    );
    crate::history_store::add_item(
        HistoryItem {
            id: format!("balance-restoration-{scale}-{padding_setting}-{with_background}-{inset}"),
            timestamp: 0,
            original_path: source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: Some(serde_json::json!({"wallpaper": {
                "padding": padding_setting,
                "balance": true,
                "inset": inset,
                "gradient": with_background.then(|| serde_json::json!({
                    "id": "balance-background",
                    "colors": ["#2468ac"],
                    "angle": 0
                }))
            }})),
            duration: None,
        },
        100,
        true,
    );
    let editor = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, editor.into());
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        assert_eq!(
            editor.snapshot.borrow().balance_crop,
            Some((
                expected.left as f32,
                expected.top as f32,
                expected.right as f32,
                expected.bottom as f32,
            )),
            "reopening a balanced screenshot must restore its crop"
        );
    });
    let window = editor.into();
    editor
        .update(cx, |editor, _, cx| {
            editor.history.push(vec![
                crate::editor::annotations::Annotation::Rectangle {
                    id: "balance-border-annotation".into(),
                    x: 0.,
                    y: 50.,
                    width: 12.,
                    height: 30.,
                    stroke: "#ff00ff".into(),
                    stroke_width: 1.,
                    fill: Some("#ff00ff".into()),
                },
                crate::editor::annotations::Annotation::Rectangle {
                    id: "balance-content-annotation".into(),
                    x: 60.,
                    y: 60.,
                    width: 20.,
                    height: 20.,
                    stroke: "#ff00ff".into(),
                    stroke_width: 1.,
                    fill: Some("#ff00ff".into()),
                },
            ]);
            editor.tool = crate::ui::colors::Tool::Select;
            editor.zoom = 1. / scale;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let (bounds, png, backdrop, matte) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
        (
            editor.bounds.borrow().unwrap(),
            editor.export_png().unwrap(),
            editor.snapshot.borrow().backdrop.clone().unwrap(),
            image::Rgba([
                (theme.r * 255.).round() as u8,
                (theme.g * 255.).round() as u8,
                (theme.b * 255.).round() as u8,
                255,
            ]),
        )
    });
    let saved = image::load_from_memory(&png).unwrap().to_rgba8();
    let padding = (padding_setting as f32 * scale).round() as u32;
    let native_inset = (inset as f32 * scale).round() as u32;
    let dimensions = (
        200 - expected.left - expected.right + (padding + native_inset) * 2,
        160 - expected.top - expected.bottom + (padding + native_inset) * 2,
    );
    assert_eq!(
        saved.dimensions(),
        dimensions,
        "annotations must not alter the balance crop"
    );
    if inset > 0. {
        assert_eq!(
            saved
                .get_pixel(padding + native_inset / 2, saved.height() / 2)
                .0,
            [230, 230, 230, 255],
            "the inset must use the balanced source edge"
        );
    }
    assert_eq!(
        (
            u32::from(backdrop.size(0).width),
            u32::from(backdrop.size(0).height)
        ),
        dimensions,
        "balanced preview and export must have matching canvas dimensions"
    );
    let mut composited = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
    image::imageops::overlay(&mut composited, &saved, 0, 0);
    let screenshot = cx.capture_screenshot(window).unwrap();
    let left =
        (f32::from(bounds.left()) * scale).round() as u32 + expected.left - padding - native_inset;
    let top =
        (f32::from(bounds.top()) * scale).round() as u32 + expected.top - padding - native_inset;
    for y in 0..saved.height() {
        for x in 0..saved.width() {
            let actual = screenshot.get_pixel(left + x, top + y).0;
            let expected = composited.get_pixel(x, y).0;
            assert!(
                actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                "balanced preview differs at {x},{y}: {actual:?} vs {expected:?}"
            );
        }
    }
    if inset > 0. {
        editor
            .update(cx, |editor, _, cx| {
                editor.tool = crate::ui::colors::Tool::Crop;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
        for input in [
            PlatformInput::MouseDown(MouseDownEvent {
                position: bounds.origin + point(px(20. / scale), px(10. / scale)),
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Modifiers::none(),
                first_mouse: false,
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                position: bounds.origin + point(px(180. / scale), px(150. / scale)),
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Modifiers::none(),
            }),
        ] {
            event(cx, window, input);
        }
        editor
            .update(cx, |editor, _, cx| editor.apply_crop(cx))
            .unwrap();
        settle(cx, window);
        cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            let refreshed = editor.snapshot.borrow().backdrop.clone().unwrap();
            assert_eq!(
                (
                    u32::from(refreshed.size(0).width),
                    u32::from(refreshed.size(0).height)
                ),
                dimensions
            );
            assert_ne!(
                refreshed.id, backdrop.id,
                "a new image must refresh the inset backdrop"
            );
        });
        cx.update_window(window, |_, window, cx| {
            window.dispatch_action(Box::new(crate::editor::actions::Undo), cx);
        })
        .unwrap();
        settle(cx, window);
        cx.update(|cx| assert_eq!(editor.read(cx).unwrap().export_png().unwrap(), png));
    }
    editor
        .update(cx, |editor, _, cx| {
            editor.tool = crate::ui::colors::Tool::Crop;
            editor.zoom = 1. / scale;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
    let start = bounds.origin + point(px(40. / scale), px(30. / scale));
    let end = bounds.origin + point(px(160. / scale), px(130. / scale));
    event(
        cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
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
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    editor
        .update(cx, |editor, _, cx| editor.apply_crop(cx))
        .unwrap();
    settle(cx, window);
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let snapshot = editor.snapshot.borrow();
        assert_eq!((snapshot.image_width, snapshot.image_height), (120., 100.));
        assert_eq!(
            snapshot.balance_crop, None,
            "cropping must discard the old balance margins"
        );
        assert_eq!(
            editor.base_image.as_ref().unwrap().to_rgba8(),
            image::imageops::crop_imm(&image, 40, 30, 120, 100).to_image(),
            "the crop must preserve the selected source pixels"
        );
    });
    cx.update_window(window, |_, window, cx| {
        window.dispatch_action(Box::new(crate::editor::actions::Undo), cx);
    })
    .unwrap();
    settle(cx, window);
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let snapshot = editor.snapshot.borrow();
        assert_eq!((snapshot.image_width, snapshot.image_height), (200., 160.));
        assert_eq!(
            snapshot.balance_crop,
            Some((
                expected.left as f32,
                expected.top as f32,
                expected.right as f32,
                expected.bottom as f32
            )),
            "undo must restore the balance margins"
        );
        assert_eq!(editor.export_png().unwrap(), png);
    });
    cx.update_window(editor.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    let reopened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, reopened.into());
    cx.update(|cx| {
        let editor = reopened.read(cx).unwrap();
        assert_eq!(
            editor.snapshot.borrow().balance_crop,
            Some((
                expected.left as f32,
                expected.top as f32,
                expected.right as f32,
                expected.bottom as f32
            ))
        );
        assert_eq!(editor.export_png().unwrap(), png);
    });
    if inset == 0. {
        let attached = scratch.join("balance-attachment.png");
        image::RgbaImage::from_pixel(100, 100, image::Rgba([240, 80, 64, 255]))
            .save(&attached)
            .unwrap();
        reopened
            .update(cx, |editor, _, cx| {
                editor.push_image_layer(
                    attached.to_string_lossy().into_owned(),
                    100.,
                    100.,
                    crate::editor::layers::Edge::Right,
                    cx,
                );
                let mut annotations = editor.history.current().to_vec();
                annotations.push(crate::editor::annotations::Annotation::Redact {
                    id: "balance-attachment-redaction".into(),
                    x: 155.,
                    y: 80.,
                    width: 30.,
                    height: 20.,
                    style: "pixelate".into(),
                    intensity: 5.,
                });
                editor.history.push(annotations);
                cx.notify();
            })
            .unwrap();
        settle(cx, reopened.into());
        reopened
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        settle(cx, reopened.into());
        let (bounds, png, matte) = cx.update(|cx| {
            let editor = reopened.read(cx).unwrap();
            let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
            (
                editor.bounds.borrow().unwrap(),
                editor.export_png().unwrap(),
                image::Rgba([
                    (theme.r * 255.).round() as u8,
                    (theme.g * 255.).round() as u8,
                    (theme.b * 255.).round() as u8,
                    255,
                ]),
            )
        });
        let saved = image::load_from_memory(&png).unwrap().to_rgba8();
        let cropped_width = 200 - expected.left - expected.right;
        let cropped_height = 160 - expected.top - expected.bottom;
        assert_eq!(
            saved.dimensions(),
            (
                cropped_width + cropped_height + padding * 2,
                cropped_height + padding * 2
            ),
            "attachments must be laid out against the balanced primary image"
        );
        let mut composited = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
        image::imageops::overlay(&mut composited, &saved, 0, 0);
        let screenshot = cx.capture_screenshot(reopened.into()).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32 + expected.left - padding;
        let top = (f32::from(bounds.top()) * scale).round() as u32 + expected.top - padding;
        assert_eq!(
            screenshot.get_pixel(left - 24, top + 40).0,
            matte.0,
            "annotations outside the balanced canvas must be clipped"
        );
        for y in 0..saved.height() {
            for x in 0..saved.width() {
                let actual = screenshot.get_pixel(left + x, top + y).0;
                let expected = composited.get_pixel(x, y).0;
                assert!(
                    actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                    "balanced attachment differs at {x},{y}: {actual:?} vs {expected:?}"
                );
            }
        }
        cx.update_window(reopened.into(), |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        let layered = cx
            .open_window(size(px(1280.), px(738.)), |window, cx| {
                window.set_scale_factor(scale);
                cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
            })
            .unwrap();
        settle(cx, layered.into());
        cx.update(|cx| {
            let editor = layered.read(cx).unwrap();
            assert_eq!(editor.export_png().unwrap(), png);
            assert_eq!(
                editor.snapshot.borrow().balance_crop,
                Some((
                    expected.left as f32,
                    expected.top as f32,
                    expected.right as f32,
                    expected.bottom as f32
                ))
            );
        });
        layered
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        settle(cx, layered.into());
        let bounds = cx.update(|cx| layered.read(cx).unwrap().bounds.borrow().unwrap());
        let screenshot = cx.capture_screenshot(layered.into()).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32 + expected.left - padding;
        let top = (f32::from(bounds.top()) * scale).round() as u32 + expected.top - padding;
        for y in 0..saved.height() {
            for x in 0..saved.width() {
                let actual = screenshot.get_pixel(left + x, top + y).0;
                let expected = composited.get_pixel(x, y).0;
                assert!(
                    actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                    "reopened balanced attachment differs at {x},{y}: {actual:?} vs {expected:?}"
                );
            }
        }
        cx.update_window(layered.into(), |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        return;
    }
    cx.update_window(reopened.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

pub(super) fn balanced_attachment_edges(
    cx: &mut HeadlessAppContext,
    scratch: &Path,
    scale: f32,
    with_background: bool,
) {
    use crate::editor::layers::Edge;

    for (edge, primary_x, primary_y, width, height, redact) in [
        (Edge::Left, 120, 0, 260, 120, (15., 80., 30., 20.)),
        (Edge::Top, 0, 140, 140, 260, (80., 5., 20., 30.)),
        (Edge::Bottom, 0, 0, 140, 260, (80., 125., 20., 30.)),
    ] {
        let name = format!("balance-edge-{scale}-{with_background}-{edge:?}");
        let source = scratch.join(format!("{name}.png"));
        image::RgbaImage::from_fn(200, 160, |x, y| {
            if (100..120).contains(&x) && (85..105).contains(&y) {
                image::Rgba([64, 112, 192, 128])
            } else if (40..160).contains(&x) && (30..130).contains(&y) {
                image::Rgba([64, 112, 192, 255])
            } else if (5..195).contains(&x) && (5..155).contains(&y) {
                image::Rgba([230, 230, 230, 255])
            } else {
                image::Rgba([255, 255, 255, 255])
            }
        })
        .save(&source)
        .unwrap();
        let attached = scratch.join(format!("{name}-attachment.png"));
        image::RgbaImage::from_pixel(100, 100, image::Rgba([240, 80, 64, 255]))
            .save(&attached)
            .unwrap();
        let padding_setting = if with_background { 20. } else { 0. };
        crate::history_store::add_item(
            HistoryItem {
                id: name.clone(),
                timestamp: 0,
                original_path: source.to_string_lossy().into_owned(),
                r#type: HistoryItemType::Screenshot,
                editor_state: Some(serde_json::json!({"wallpaper": {
                    "padding": padding_setting,
                    "balance": true,
                    "gradient": with_background.then(|| serde_json::json!({
                        "id": "balance-background",
                        "colors": ["#2468ac"],
                        "angle": 0
                    }))
                }})),
                duration: None,
            },
            100,
            true,
        );
        let padding = (padding_setting as f32 * scale).round() as u32;
        let mut previous = None;
        for reopened in [false, true] {
            let editor = cx
                .open_window(size(px(1280.), px(738.)), |window, cx| {
                    window.set_scale_factor(scale);
                    cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
                })
                .unwrap();
            settle(cx, editor.into());
            if !reopened {
                editor
                    .update(cx, |editor, _, cx| {
                        editor.push_image_layer(
                            attached.to_string_lossy().into_owned(),
                            100.,
                            100.,
                            edge,
                            cx,
                        );
                        editor
                            .history
                            .push(vec![crate::editor::annotations::Annotation::Redact {
                                id: "balance-edge-redaction".into(),
                                x: redact.0,
                                y: redact.1,
                                width: redact.2,
                                height: redact.3,
                                style: "pixelate".into(),
                                intensity: 5.,
                            }]);
                        cx.notify();
                    })
                    .unwrap();
                settle(cx, editor.into());
            }
            editor
                .update(cx, |editor, _, cx| {
                    editor.zoom = 1. / scale;
                    cx.notify();
                })
                .unwrap();
            settle(cx, editor.into());
            let (bounds, png, matte) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                assert_eq!(
                    editor.snapshot.borrow().balance_crop,
                    Some((30., 20., 30., 20.))
                );
                let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
                (
                    editor.bounds.borrow().unwrap(),
                    editor.export_png().unwrap(),
                    image::Rgba([
                        (theme.r * 255.).round() as u8,
                        (theme.g * 255.).round() as u8,
                        (theme.b * 255.).round() as u8,
                        255,
                    ]),
                )
            });
            if let Some(previous) = &previous {
                assert_eq!(&png, previous, "{name} must reopen with the same output");
            } else {
                previous = Some(png.clone());
            }
            let saved = image::load_from_memory(&png).unwrap().to_rgba8();
            assert_eq!(
                saved.dimensions(),
                (width + padding * 2, height + padding * 2),
                "{name} must use the balanced primary dimensions"
            );
            let (sample_x, sample_y) = match edge {
                Edge::Left | Edge::Top => (padding + 30, padding + 30),
                Edge::Bottom => (padding + 30, padding + 150),
                _ => unreachable!(),
            };
            assert_eq!(saved.get_pixel(sample_x, sample_y).0, [240, 80, 64, 255]);
            let mut composited = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
            image::imageops::overlay(&mut composited, &saved, 0, 0);
            let screenshot = cx.capture_screenshot(editor.into()).unwrap();
            let left = ((f32::from(bounds.left()) * scale).round() as i32 + 30
                - padding as i32
                - primary_x) as u32;
            let top = ((f32::from(bounds.top()) * scale).round() as i32 + 20
                - padding as i32
                - primary_y) as u32;
            for y in 0..saved.height() {
                for x in 0..saved.width() {
                    let actual = screenshot.get_pixel(left + x, top + y).0;
                    let expected = composited.get_pixel(x, y).0;
                    assert!(
                        actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                        "{name}, reopened {reopened}, at {x},{y}: {actual:?} vs {expected:?}"
                    );
                }
            }
            cx.update_window(editor.into(), |_, window, _| window.remove_window())
                .unwrap();
            cx.run_until_parked();
        }
    }
}

pub(super) fn framed_attachment_layout(cx: &mut HeadlessAppContext, scratch: &Path, scale: f32) {
    use crate::editor::annotations::Annotation;
    use crate::editor::layers::{Edge, ImageLayer};

    if scale == 1.0 {
        let reference_base = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            80,
            60,
            image::Rgba([64, 112, 192, 255]),
        ));
        let reference_top = scratch.join("framed-electron-reference-top.png");
        image::RgbaImage::from_pixel(40, 20, image::Rgba([240, 80, 64, 255]))
            .save(&reference_top)
            .unwrap();
        let reference_right = scratch.join("framed-electron-reference-right.png");
        image::RgbaImage::from_pixel(30, 40, image::Rgba([72, 192, 112, 255]))
            .save(&reference_right)
            .unwrap();
        let reference_layers = [
            ImageLayer {
                id: "top".into(),
                image_url: reference_top.to_string_lossy().into_owned(),
                natural_width: 40.0,
                natural_height: 20.0,
                edge: Edge::Top,
            },
            ImageLayer {
                id: "right".into(),
                image_url: reference_right.to_string_lossy().into_owned(),
                natural_width: 30.0,
                natural_height: 40.0,
                edge: Edge::Right,
            },
        ];
        let reference_annotations = [Annotation::Redact {
            id: "reference-blackout".into(),
            x: 10.0,
            y: 10.0,
            width: 5.0,
            height: 5.0,
            style: "blackout".into(),
            intensity: 5.0,
        }];
        let mut reference_wallpaper = crate::editor::wallpaper::WallpaperSettings::default();
        reference_wallpaper.window_frame.style = "windows-dark".into();
        let reference = crate::editor::export::compose_with_layers(
            Some(&reference_base),
            80,
            60,
            &reference_annotations,
            &reference_wallpaper,
            &reference_layers,
            1.0,
        );
        assert_eq!(reference.dimensions(), (119, 184));
        assert_eq!(reference.get_pixel(40, 38).0, [240, 80, 64, 255]);
        assert_eq!(reference.get_pixel(85, 134).0, [72, 192, 112, 255]);
        assert_eq!(reference.get_pixel(12, 136).0, [0, 0, 0, 255]);
        reference
            .save(scratch.join("gpui-framed-attachment-electron-reference.png"))
            .unwrap();
        std::fs::remove_file(reference_top).unwrap();
        std::fs::remove_file(reference_right).unwrap();
    }

    for (balanced, frame_style, transparent) in [
        (false, "windows-dark", false),
        (true, "windows-dark", false),
        (true, "macos-light", false),
        (true, "macos-dark", false),
        (false, "macos-light", true),
        (false, "macos-dark", true),
    ] {
        let name = format!("framed-attachments-{scale}-{balanced}-{frame_style}-{transparent}");
        let source = scratch.join(format!("{name}-source.png"));
        let (source_width, source_height, crop_left, crop_top, primary_width, primary_height) =
            if balanced {
                (200, 160, 30, 20, 140, 120)
            } else {
                (160, 120, 0, 0, 160, 120)
            };
        let image = image::RgbaImage::from_fn(source_width, source_height, |x, y| {
            if balanced {
                if (40..160).contains(&x) && (30..130).contains(&y) {
                    image::Rgba([64, 112, 192, 255])
                } else if (5..195).contains(&x) && (5..155).contains(&y) {
                    image::Rgba([230, 230, 230, 255])
                } else {
                    image::Rgba([255, 255, 255, 255])
                }
            } else if transparent && (70..130).contains(&x) && (70..100).contains(&y) {
                image::Rgba([64, 112, 192, 128])
            } else {
                image::Rgba([64, 112, 192, 255])
            }
        });
        if balanced {
            assert_eq!(
                crate::editor::export::balance_bounds(&image),
                Some(crate::render::color_detection::ContentBounds {
                    left: crop_left,
                    top: crop_top,
                    right: crop_left,
                    bottom: crop_top,
                }),
                "the bordered fixture must produce a known nonzero balance crop"
            );
        }
        image.save(&source).unwrap();

        let top_path = scratch.join(format!("{name}-top.png"));
        image::RgbaImage::from_pixel(80, 24, image::Rgba([240, 80, 64, 255]))
            .save(&top_path)
            .unwrap();
        let right_path = scratch.join(format!("{name}-right.png"));
        image::RgbaImage::from_pixel(30, 60, image::Rgba([72, 192, 112, 255]))
            .save(&right_path)
            .unwrap();

        let id = name.clone();
        crate::history_store::add_item(
            HistoryItem {
                id: id.clone(),
                timestamp: 0,
                original_path: source.to_string_lossy().into_owned(),
                r#type: HistoryItemType::Screenshot,
                editor_state: Some(serde_json::json!({
                    "wallpaper": {
                        "balance": balanced,
                        "windowFrame": {"style": frame_style}
                    },
                    "layers": [
                        {
                            "id": "top",
                            "imageUrl": top_path.to_string_lossy(),
                            "naturalWidth": 80.0,
                            "naturalHeight": 24.0,
                            "edge": "top"
                        },
                        {
                            "id": "right",
                            "imageUrl": right_path.to_string_lossy(),
                            "naturalWidth": 30.0,
                            "naturalHeight": 60.0,
                            "edge": "right"
                        }
                    ]
                })),
                duration: None,
            },
            100,
            true,
        );

        let editor = cx
            .open_window(size(px(1280.), px(738.)), |window, cx| {
                window.set_scale_factor(scale);
                cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
            })
            .unwrap();
        let window = editor.into();
        settle(cx, window);
        let native_scale = crate::editor::wallpaper::native_scale(
            f64::from(source_width),
            f64::from(source_height),
            f64::from(scale),
        );
        let zoom = (1.0 / native_scale) as f32;
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = zoom;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);

        std::fs::write(
            scratch.join(format!("gpui-{name}-unannotated-export.png")),
            cx.update(|cx| editor.read(cx).unwrap().export_png().unwrap()),
        )
        .unwrap();

        let title_bar = crate::render::window_frame::title_bar_height(frame_style, native_scale);
        let primary_frame_height = f64::from(primary_height) + title_bar;
        let top_height = ((24.0 + title_bar) * f64::from(primary_width) / 80.0).round();
        let right_width = (30.0 * primary_frame_height / (60.0 + title_bar)).round();
        let composition_width = (f64::from(primary_width) + right_width).round() as u32;
        let composition_height = (top_height + primary_frame_height).round() as u32;

        let tool_start = if balanced { (70.0, 70.0) } else { (30.0, 30.0) };
        let tool_end = (tool_start.0 + 20.0, tool_start.1 + 20.0);
        click_measured(cx, window, "editor-tools", 2);
        settle(cx, window);
        let image_bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
        drag_rectangle(
            cx,
            window,
            image_bounds.origin,
            (
                (tool_start.0 * f64::from(zoom)) as f32,
                (tool_start.1 * f64::from(zoom)) as f32,
            ),
            (
                (tool_end.0 * f64::from(zoom)) as f32,
                (tool_end.1 * f64::from(zoom)) as f32,
            ),
        );
        click_measured(cx, window, "editor-tools", 9);
        settle(cx, window);
        let selection_origin = (
            ((tool_start.0 + tool_end.0) / 2.0 * f64::from(zoom)) as f32,
            (tool_start.1 * f64::from(zoom)) as f32,
        );
        drag_rectangle(
            cx,
            window,
            image_bounds.origin,
            selection_origin,
            (
                selection_origin.0 + 8.0 * zoom,
                selection_origin.1 + 6.0 * zoom,
            ),
        );
        settle(cx, window);
        cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            assert!(
                matches!(
                    editor.history.current(),
                    [Annotation::Rectangle { x, y, width, height, .. }]
                        if (*x - (tool_start.0 + 8.0)).abs() < 0.01
                            && (*y - (tool_start.1 + 6.0)).abs() < 0.01
                            && (*width - 20.0).abs() < 0.01
                            && (*height - 20.0).abs() < 0.01
                ),
                "pointer selection and dragging must update the primary annotation coordinates"
            );
        });
        let escape = Keystroke::parse("escape").unwrap();
        event(
            cx,
            window,
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: escape.clone(),
                is_held: false,
                prefer_character_input: false,
            }),
        );
        event(
            cx,
            window,
            PlatformInput::KeyUp(KeyUpEvent { keystroke: escape }),
        );
        settle(cx, window);
        cx.update(|cx| {
            assert!(
                editor
                    .read(cx)
                    .unwrap()
                    .snapshot
                    .borrow()
                    .selected
                    .is_empty(),
                "Escape must clear the selected annotation before preview comparison"
            );
        });

        let (png, matte, stage_bounds) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            assert!(
                matches!(
                    editor.history.current(),
                    [Annotation::Rectangle { x, y, width, height, .. }]
                        if (*x - (tool_start.0 + 8.0)).abs() < 0.01
                            && (*y - (tool_start.1 + 6.0)).abs() < 0.01
                            && (*width - 20.0).abs() < 0.01
                            && (*height - 20.0).abs() < 0.01
                ),
                "drawing, selecting and dragging must use the framed primary image coordinates"
            );
            let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
            (
                editor.export_png().unwrap(),
                image::Rgba([
                    (theme.r * 255.0).round() as u8,
                    (theme.g * 255.0).round() as u8,
                    (theme.b * 255.0).round() as u8,
                    255,
                ]),
                super::measured("editor-stage", 0),
            )
        });
        let saved = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(
            saved.dimensions(),
            (composition_width, composition_height),
            "Electron's per-image title bars determine the full attachment composition"
        );
        std::fs::write(scratch.join(format!("gpui-{name}-export.png")), &png).unwrap();

        let (content_y, right_content_x) = (
            top_height.round() as u32 + title_bar.round() as u32 + 10,
            primary_width + 10,
        );
        assert_eq!(
            saved
                .get_pixel(primary_width / 2, title_bar.round() as u32 + 4)
                .0,
            [240, 80, 64, 255],
            "the top attachment content starts below its own title bar"
        );
        let frame_color = match frame_style {
            "macos-light" => [232, 232, 232, 255],
            "macos-dark" => [58, 58, 60, 255],
            _ => [32, 32, 32, 255],
        };
        assert_eq!(
            saved
                .get_pixel(primary_width / 2, top_height.round() as u32 + 2)
                .0,
            frame_color,
            "the primary image has a separate title bar"
        );
        assert_eq!(
            saved.get_pixel(primary_width / 2, content_y).0,
            [64, 112, 192, 255],
            "the primary image starts below its own title bar"
        );
        assert_eq!(
            saved
                .get_pixel(
                    right_content_x + 10,
                    top_height.round() as u32 + if transparent { 3 } else { 10 }
                )
                .0,
            frame_color,
            "the side attachment has its own title bar"
        );
        assert_eq!(
            saved.get_pixel(right_content_x, content_y).0,
            [72, 192, 112, 255],
            "the right attachment is laid out alongside the primary"
        );
        if balanced {
            assert_eq!(saved.get_pixel(2, content_y).0, [230, 230, 230, 255]);
        }
        if transparent {
            assert_eq!(
                saved
                    .get_pixel(
                        80,
                        top_height.round() as u32 + title_bar.round() as u32 + 80
                    )
                    .0[3],
                128,
                "the translucent primary image must stay translucent in the exported PNG"
            );
        }

        let mut composited = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
        image::imageops::overlay(&mut composited, &saved, 0, 0);
        let screenshot = cx.capture_screenshot(window).unwrap();
        let stage_left = (f32::from(stage_bounds.left()) * scale).round() as u32;
        let stage_top = (f32::from(stage_bounds.top()) * scale).round() as u32;
        image::imageops::crop_imm(
            &screenshot,
            stage_left,
            stage_top,
            saved.width(),
            saved.height(),
        )
        .to_image()
        .save(scratch.join(format!("gpui-{name}-scale-{scale}-preview.png")))
        .unwrap();
        composited
            .save(scratch.join(format!("gpui-{name}-scale-{scale}-expected.png")))
            .unwrap();
        for y in 0..saved.height() {
            for x in 0..saved.width() {
                let actual = screenshot.get_pixel(stage_left + x, stage_top + y).0;
                let expected = composited.get_pixel(x, y).0;
                assert!(
                    actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                    "{name} scale {scale} preview/export differs at {x},{y}: {actual:?} vs {expected:?}"
                );
            }
        }

        cx.update_window(window, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        let reopened = cx
            .open_window(size(px(1280.), px(738.)), |window, cx| {
                window.set_scale_factor(scale);
                cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
            })
            .unwrap();
        settle(cx, reopened.into());
        reopened
            .update(cx, |editor, _, cx| {
                editor.zoom = zoom;
                cx.notify();
            })
            .unwrap();
        settle(cx, reopened.into());
        let (reopened_png, reopened_matte, reopened_stage_bounds) = cx.update(|cx| {
            let editor = reopened.read(cx).unwrap();
            assert!(matches!(
                editor.history.current(),
                [Annotation::Rectangle { x, y, width, height, .. }]
                    if (*x - (tool_start.0 + 8.0)).abs() < 0.01
                        && (*y - (tool_start.1 + 6.0)).abs() < 0.01
                        && (*width - 20.0).abs() < 0.01
                        && (*height - 20.0).abs() < 0.01
            ));
            let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
            (
                editor.export_png().unwrap(),
                image::Rgba([
                    (theme.r * 255.0).round() as u8,
                    (theme.g * 255.0).round() as u8,
                    (theme.b * 255.0).round() as u8,
                    255,
                ]),
                super::measured("editor-stage", 0),
            )
        });
        assert_eq!(reopened_png, png);
        let reopened_saved = image::load_from_memory(&reopened_png).unwrap().to_rgba8();
        let mut reopened_composited = image::RgbaImage::from_pixel(
            reopened_saved.width(),
            reopened_saved.height(),
            reopened_matte,
        );
        image::imageops::overlay(&mut reopened_composited, &reopened_saved, 0, 0);
        let reopened_screenshot = cx.capture_screenshot(reopened.into()).unwrap();
        let reopened_stage_left = (f32::from(reopened_stage_bounds.left()) * scale).round() as u32;
        let reopened_stage_top = (f32::from(reopened_stage_bounds.top()) * scale).round() as u32;
        image::imageops::crop_imm(
            &reopened_screenshot,
            reopened_stage_left,
            reopened_stage_top,
            reopened_saved.width(),
            reopened_saved.height(),
        )
        .to_image()
        .save(scratch.join(format!("gpui-{name}-scale-{scale}-reopened-preview.png")))
        .unwrap();
        reopened_composited
            .save(scratch.join(format!("gpui-{name}-scale-{scale}-reopened-expected.png")))
            .unwrap();
        for y in 0..reopened_saved.height() {
            for x in 0..reopened_saved.width() {
                let actual = reopened_screenshot
                    .get_pixel(reopened_stage_left + x, reopened_stage_top + y)
                    .0;
                let expected = reopened_composited.get_pixel(x, y).0;
                assert!(
                    actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                    "reopened {name} scale {scale} preview/export differs at {x},{y}: {actual:?} vs {expected:?}"
                );
            }
        }
        if !balanced && frame_style.starts_with("macos") {
            reopened
                .update(cx, |editor, _, cx| {
                    editor.history.replace_current(Vec::new());
                    cx.notify();
                })
                .unwrap();
            settle(cx, reopened.into());
            let baseline = cx.update(|cx| reopened.read(cx).unwrap().export_png().unwrap());
            let baseline = image::load_from_memory(&baseline).unwrap().to_rgba8();
            for style in ["blur", "pixelate"] {
                let mut plain_stripe = None;
                for shape_order in ["none", "before", "after"] {
                    reopened
                        .update(cx, |editor, _, cx| {
                            let stripe = Annotation::Rectangle {
                                id: "framed-seam-stripe".into(),
                                x: 156.,
                                y: 70.,
                                width: 2.,
                                height: 24.,
                                stroke: "#ff0000".into(),
                                stroke_width: 1.,
                                fill: Some("#ff0000".into()),
                            };
                            let redact = Annotation::Redact {
                                id: format!("framed-seam-{style}"),
                                x: 145.,
                                y: 70.,
                                width: 40.,
                                height: 24.,
                                style: style.into(),
                                intensity: 5.,
                            };
                            let annotations = match shape_order {
                                "before" => vec![stripe, redact],
                                "after" => vec![redact, stripe],
                                _ => vec![redact],
                            };
                            editor.history.replace_current(annotations);
                            cx.notify();
                        })
                        .unwrap();
                    settle(cx, reopened.into());
                    let (png, matte, stage) = cx.update(|cx| {
                        let editor = reopened.read(cx).unwrap();
                        let theme =
                            gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
                        (
                            editor.export_png().unwrap(),
                            image::Rgba([
                                (theme.r * 255.).round() as u8,
                                (theme.g * 255.).round() as u8,
                                (theme.b * 255.).round() as u8,
                                255,
                            ]),
                            super::measured("editor-stage", 0),
                        )
                    });
                    let saved = image::load_from_memory(&png).unwrap().to_rgba8();
                    let stripe = saved
                        .get_pixel(
                            157,
                            top_height.round() as u32 + title_bar.round() as u32 + 80,
                        )
                        .0;
                    if shape_order == "none" {
                        plain_stripe = Some(stripe);
                    }
                    if shape_order == "before" {
                        assert!(stripe[1] > 0, "{name} {style} filtered stripe: {stripe:?}");
                        assert_ne!(
                            Some(stripe),
                            plain_stripe,
                            "{name} {style} must filter the earlier stripe"
                        );
                    }
                    if shape_order == "after" {
                        assert_eq!(stripe, [255, 0, 0, 255], "{name} {style} upper stripe");
                    }
                    let mut modified = [false, false];
                    for (x, y, pixel) in saved.enumerate_pixels() {
                        if pixel != baseline.get_pixel(x, y) {
                            modified[usize::from(x >= primary_width)] = true;
                        }
                    }
                    assert_eq!(
                        modified,
                        [true, true],
                        "{style} must affect both sides of the seam"
                    );
                    let mut expected =
                        image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
                    image::imageops::overlay(&mut expected, &saved, 0, 0);
                    let screenshot = cx.capture_screenshot(reopened.into()).unwrap();
                    let left = (f32::from(stage.left()) * scale).round() as u32;
                    let top = (f32::from(stage.top()) * scale).round() as u32;
                    let actual = image::imageops::crop_imm(
                        &screenshot,
                        left,
                        top,
                        saved.width(),
                        saved.height(),
                    )
                    .to_image();
                    actual
                        .save(
                            scratch.join(format!("gpui-{name}-{style}-{shape_order}-preview.png")),
                        )
                        .unwrap();
                    expected
                        .save(
                            scratch.join(format!("gpui-{name}-{style}-{shape_order}-expected.png")),
                        )
                        .unwrap();
                    for (x, y, actual) in actual.enumerate_pixels() {
                        let expected = expected.get_pixel(x, y);
                        assert!(
                        actual.0.iter().zip(expected.0).all(|(a, b)| a.abs_diff(b) <= 2),
                        "{name} {style} preview/export differs at {x},{y}: {actual:?} vs {expected:?}"
                    );
                    }
                    println!("{name} {style} shape {shape_order}: stripe {stripe:?}, preview matches export within two levels");
                }
            }
        }
        cx.update_window(reopened.into(), |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
        assert_eq!(
            crate::history_store::delete_item(&id),
            crate::history_store::DeleteOutcome::Deleted
        );
        std::fs::remove_file(top_path).unwrap();
        std::fs::remove_file(right_path).unwrap();
    }
}

pub(super) fn large_attachment_preview(cx: &mut HeadlessAppContext, scratch: &Path) {
    let blue = image::Rgba([64, 112, 192, 255]);
    let red = image::Rgba([240, 80, 64, 255]);
    let translucent = image::Rgba([40, 190, 80, 128]);
    for vertical in [false, true] {
        let source = scratch.join(format!("large-primary-{vertical}.png"));
        let attached = scratch.join(format!("large-attached-{vertical}.png"));
        let (width, height) = if vertical { (160, 9600) } else { (9600, 160) };
        image::RgbaImage::from_pixel(width, height, blue)
            .save(&source)
            .unwrap();
        image::RgbaImage::from_fn(width, height, |x, y| {
            let position = if vertical { y } else { x };
            if position < 100 {
                blue
            } else if (6700..6900).contains(&position) {
                translucent
            } else {
                red
            }
        })
        .save(&attached)
        .unwrap();
        let editor = cx
            .open_window(size(px(1280.), px(738.)), |window, cx| {
                window.set_scale_factor(1.);
                cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
            })
            .unwrap();
        let window = editor.into();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                editor.push_image_layer(
                    attached.to_string_lossy().into_owned(),
                    f64::from(width),
                    f64::from(height),
                    if vertical {
                        crate::editor::layers::Edge::Bottom
                    } else {
                        crate::editor::layers::Edge::Right
                    },
                    cx,
                );
                let (x, y, width, height) = if vertical {
                    (80., 9200., 80., 800.)
                } else {
                    (9200., 80., 800., 80.)
                };
                editor
                    .history
                    .push(vec![crate::editor::annotations::Annotation::Rectangle {
                        id: "large-join".into(),
                        x,
                        y,
                        width,
                        height,
                        stroke: "#ff00ff".into(),
                        stroke_width: 1.,
                        fill: Some("#ff00ff".into()),
                    }]);
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 0.035;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        let (bounds, png) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            (
                editor.bounds.borrow().unwrap(),
                editor.export_png().unwrap(),
            )
        });
        let saved = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(
            saved.dimensions(),
            if vertical { (160, 19200) } else { (19200, 160) }
        );
        let coordinate = |along, across| {
            if vertical {
                (across, along)
            } else {
                (along, across)
            }
        };
        let matte = cx.update(|cx| {
            let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
            image::Rgba([
                (theme.r * 255.).round() as u8,
                (theme.g * 255.).round() as u8,
                (theme.b * 255.).round() as u8,
                255,
            ])
        });
        let mut composited = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
        image::imageops::overlay(&mut composited, &saved, 0, 0);
        let screenshot = cx.capture_screenshot(window).unwrap();
        #[cfg(windows)]
        let tiles = cx.update(|cx| {
            editor
                .read(cx)
                .unwrap()
                .snapshot
                .borrow()
                .preview_edges
                .iter()
                .map(|tile| tile.image.clone())
                .collect::<Vec<_>>()
        });
        #[cfg(windows)]
        cx.update_window(window, |_, window, _| {
            assert_eq!(tiles.len(), 3);
            assert!(tiles
                .iter()
                .all(|image| window.has_image_atlas_entry(image)));
        })
        .unwrap();
        for (along, across, expected) in [
            (4100, 40, blue.0),
            (8190, 40, blue.0),
            (8210, 40, blue.0),
            (12300, 40, red.0),
            (16380, 40, translucent.0),
            (16400, 40, translucent.0),
            (17500, 40, red.0),
            (9400, 120, [255, 0, 255, 255]),
            (9800, 120, [255, 0, 255, 255]),
        ] {
            let (x, y) = coordinate(along, across);
            assert!(saved
                .get_pixel(x, y)
                .0
                .iter()
                .zip(expected)
                .all(|(a, e)| a.abs_diff(e) <= 2));
            let display = composited.get_pixel(x, y).0;
            let x = (f32::from(bounds.left()) + x as f32 * 0.035).round() as u32;
            let y = (f32::from(bounds.top()) + y as f32 * 0.035).round() as u32;
            let actual = screenshot.get_pixel(x, y).0;
            assert!(
                actual.iter().zip(display).all(|(a, e)| a.abs_diff(e) <= 2),
                "large attachment preview differs at {x},{y}: {actual:?} vs {display:?}"
            );
        }
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 0.25;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        for (scroll, join) in [(-1500., 8192.), (-2200., 16384.)] {
            event(
                cx,
                window,
                PlatformInput::Scroll(gpui::ScrollEvent {
                    position: point(px(600.), px(400.)),
                    delta: gpui::ScrollDelta::Pixels(if vertical {
                        point(px(0.), px(scroll))
                    } else {
                        point(px(scroll), px(0.))
                    }),
                    ..Default::default()
                }),
            );
            settle(cx, window);
            let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
            let seam = (f32::from(if vertical {
                bounds.top()
            } else {
                bounds.left()
            }) + join * 0.25)
                .round() as i32;
            let across = (f32::from(if vertical {
                bounds.left()
            } else {
                bounds.top()
            }) + 40. * 0.25)
                .round() as u32;
            assert!(
                (4..if vertical { 734 } else { 1276 }).contains(&seam),
                "the {join} tile join must be visible for vertical {vertical}: {seam}, {bounds:?}"
            );
            let screenshot = cx.capture_screenshot(window).unwrap();
            let (source_x, source_y) = coordinate(join as u32, 40);
            let color = composited.get_pixel(source_x, source_y).0;
            for delta in -3..=3 {
                let (x, y) = coordinate((seam + delta) as u32, across);
                let actual = screenshot.get_pixel(x, y).0;
                assert!(
                    actual.iter().zip(color).all(|(a, e)| a.abs_diff(e) <= 2),
                    "large tile seam differs at {delta}: {actual:?}"
                );
            }
        }
        #[cfg(windows)]
        {
            editor
                .update(cx, |editor, _, cx| {
                    editor.wallpaper.corners = 1.;
                    cx.notify();
                })
                .unwrap();
            settle(cx, window);
            cx.update_window(window, |_, window, _| {
                assert!(tiles
                    .iter()
                    .all(|image| !window.has_image_atlas_entry(image)));
            })
            .unwrap();
        }
        cx.update_window(window, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
    }
}

pub(super) fn restored_pixelation(
    cx: &mut HeadlessAppContext,
    scratch: &Path,
    scale: f32,
    transparent: bool,
    clipped: bool,
) {
    let source = scratch.join(format!(
        "restored-pixelation-{scale}-{transparent}-{clipped}.png"
    ));
    image::RgbaImage::from_fn(200, 120, |x, y| {
        let red = if x % 16 < 8 { 24 } else { 224 };
        image::Rgba([
            red,
            (y % 32 * 6) as u8,
            80,
            if transparent { 128 } else { 255 },
        ])
    })
    .save(&source)
    .unwrap();
    let annotation = crate::editor::annotations::Annotation::Redact {
        id: "restored-pixelation".into(),
        x: if clipped { -8. } else { 40. },
        y: 30.,
        width: 80.,
        height: 40.,
        style: "pixelate".into(),
        intensity: 5.,
    };
    let shape = crate::editor::annotations::Annotation::Rectangle {
        id: "before-pixelation".into(),
        x: 64.,
        y: 40.,
        width: 32.,
        height: 20.,
        stroke: "#00ff00".into(),
        stroke_width: 1.,
        fill: Some("#00ff00".into()),
    };
    let mut annotations = Vec::new();
    if scale == 1. && !clipped {
        annotations.push(shape);
    }
    annotations.push(annotation);
    annotations.push(crate::editor::annotations::Annotation::Rectangle {
        id: "after-pixelation".into(),
        x: if clipped { 50. } else { 100. },
        y: 48.,
        width: 12.,
        height: 12.,
        stroke: "#ff00ff".into(),
        stroke_width: 1.,
        fill: Some("#ff00ff".into()),
    });
    annotations.push(crate::editor::annotations::Annotation::Redact {
        id: "after-pixelation-blackout".into(),
        x: if clipped { 54. } else { 104. },
        y: 52.,
        width: 5.,
        height: 5.,
        style: "blackout".into(),
        intensity: 2.,
    });
    crate::history_store::add_item(
        HistoryItem {
            id: format!("restored-pixelation-{scale}-{transparent}-{clipped}"),
            timestamp: 0,
            original_path: source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: Some(serde_json::json!({"annotations": annotations})),
            duration: None,
        },
        100,
        true,
    );
    let editor = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, editor.into());
    editor
        .update(cx, |editor, _, cx| {
            editor.zoom = 1. / scale;
            cx.notify();
        })
        .unwrap();
    settle(cx, editor.into());
    let (bounds, png, matte) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
        (
            editor.bounds.borrow().unwrap(),
            editor.export_png().unwrap(),
            image::Rgba([
                (theme.r * 255.).round() as u8,
                (theme.g * 255.).round() as u8,
                (theme.b * 255.).round() as u8,
                255,
            ]),
        )
    });
    let saved = image::load_from_memory(&png).unwrap().to_rgba8();
    let original = image::open(&source).unwrap().to_rgba8();
    let left_edge = if clipped { 0 } else { 40 };
    let right_edge = if clipped { 72 } else { 120 };
    assert_ne!(
        saved.get_pixel(left_edge + 3, 32),
        original.get_pixel(left_edge + 3, 32)
    );
    assert_eq!(
        saved.get_pixel(if clipped { 56 } else { 106 }, 54).0,
        [0, 0, 0, 255]
    );
    let mut composited = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
    image::imageops::overlay(&mut composited, &saved, 0, 0);
    let screenshot = cx.capture_screenshot(editor.into()).unwrap();
    let left = (f32::from(bounds.left()) * scale).round() as u32;
    let top = (f32::from(bounds.top()) * scale).round() as u32;
    for y in 30..70 {
        for x in left_edge..right_edge {
            let actual = screenshot.get_pixel(left + x, top + y).0;
            let expected = composited.get_pixel(x, y).0;
            assert!(
                actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                "restored pixelation {scale}x, transparent={transparent}, clipped={clipped} at {x},{y}: {actual:?} vs {expected:?}"
            );
        }
    }
    let flattened_path = scratch.join(format!("restored-pixelation-flat-{transparent}.png"));
    std::fs::write(&flattened_path, &png).unwrap();
    let flattened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| EditorWindow::from_file(flattened_path.to_str().unwrap(), window, cx))
        })
        .unwrap();
    for effective_zoom in [0.5, 0.73, 1.37, 1.5] {
        let zoom = effective_zoom / scale;
        for window in [editor, flattened] {
            window
                .update(cx, |editor, _, cx| {
                    editor.zoom = zoom;
                    cx.notify();
                })
                .unwrap();
            settle(cx, window.into());
        }
        let (preview_bounds, flattened_bounds) = cx.update(|cx| {
            (
                editor.read(cx).unwrap().bounds.borrow().unwrap(),
                flattened.read(cx).unwrap().bounds.borrow().unwrap(),
            )
        });
        let preview = cx.capture_screenshot(editor.into()).unwrap();
        let flattened_image = cx.capture_screenshot(flattened.into()).unwrap();
        let snap = |value: f32| (value.abs() - 0.5).ceil().copysign(value) as u32;
        let left = snap(f32::from(preview_bounds.left()) * scale);
        let top = snap(f32::from(preview_bounds.top()) * scale);
        let flat_left = snap(f32::from(flattened_bounds.left()) * scale);
        let flat_top = snap(f32::from(flattened_bounds.top()) * scale);
        let width = snap(f32::from(preview_bounds.right()) * scale) - left;
        let height = snap(f32::from(preview_bounds.bottom()) * scale) - top;
        assert_eq!(
            (width, height),
            (
                snap(f32::from(flattened_bounds.right()) * scale) - flat_left,
                snap(f32::from(flattened_bounds.bottom()) * scale) - flat_top,
            ),
            "pixelation preview dimensions at zoom={effective_zoom}, DPI={scale}"
        );
        let mismatched = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                preview
                    .get_pixel(left + x, top + y)
                    .0
                    .iter()
                    .zip(flattened_image.get_pixel(flat_left + x, flat_top + y).0)
                    .any(|(actual, expected)| actual.abs_diff(expected) > 2)
            })
            .count();
        if mismatched > 0 {
            for (name, image, x, y) in [
                ("preview", &preview, left, top),
                ("saved", &flattened_image, flat_left, flat_top),
            ] {
                image::imageops::crop_imm(image, x, y, width, height)
                    .to_image()
                    .save(scratch.join(format!(
                        "pixelation-whole-{scale}-{effective_zoom}-{transparent}-{clipped}-{name}.png"
                    )))
                    .unwrap();
            }
        }
        assert_eq!(
            mismatched, 0,
            "whole pixelation image at zoom={effective_zoom}, DPI={scale}, transparent={transparent}, clipped={clipped}"
        );
        if scale == 1. {
            image::imageops::crop_imm(&preview, left, top, width, height)
                .to_image()
                .save(scratch.join(format!(
                    "pixelation-zoom-{effective_zoom}-{transparent}-{clipped}-preview.png"
                )))
                .unwrap();
            image::imageops::crop_imm(&flattened_image, flat_left, flat_top, width, height)
                .to_image()
                .save(scratch.join(format!(
                    "pixelation-zoom-{effective_zoom}-{transparent}-{clipped}-saved.png"
                )))
                .unwrap();
        }
    }
    cx.update_window(flattened.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    if scale == 1. && !clipped {
        check_redaction_draft(cx, editor, f64::from(left_edge + 4), 32.);
        cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            assert!(!editor
                .snapshot
                .borrow()
                .redact_patches
                .contains_key("restored-pixelation"));
        });
        for gradient in [false, true] {
            editor
                .update(cx, |editor, _, cx| {
                    editor.wallpaper.padding = 20.;
                    editor.wallpaper.set_gradient(
                        gradient
                            .then(|| crate::editor::wallpaper::preset("slate"))
                            .flatten(),
                    );
                    cx.notify();
                })
                .unwrap();
            settle(cx, editor.into());
            editor
                .update(cx, |editor, _, cx| {
                    editor.zoom = 1.;
                    cx.notify();
                })
                .unwrap();
            settle(cx, editor.into());
            let (bounds, png, background) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                assert!(!editor
                    .snapshot
                    .borrow()
                    .redact_patches
                    .contains_key("restored-pixelation"));
                let background = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
                (
                    editor.bounds.borrow().unwrap(),
                    editor.export_png().unwrap(),
                    image::Rgba([
                        (background.r * 255.).round() as u8,
                        (background.g * 255.).round() as u8,
                        (background.b * 255.).round() as u8,
                        255,
                    ]),
                )
            });
            let wallpaper = image::load_from_memory(&png).unwrap().to_rgba8();
            let mut composited =
                image::RgbaImage::from_pixel(wallpaper.width(), wallpaper.height(), background);
            image::imageops::overlay(&mut composited, &wallpaper, 0, 0);
            let screenshot = cx.capture_screenshot(editor.into()).unwrap();
            let left = f32::from(bounds.left()).round() as u32;
            let top = f32::from(bounds.top()).round() as u32;
            for y in 30..70 {
                for x in 40..120 {
                    let actual = screenshot.get_pixel(left + x, top + y).0;
                    let expected = composited.get_pixel(x + 20, y + 20).0;
                    assert!(
                        actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                        "pixelation with wallpaper gradient={gradient}, transparent={transparent} at {x},{y}: {actual:?} vs {expected:?}"
                    );
                }
            }
        }
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.set_gradient(None);
                editor.wallpaper.padding = 0.;
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
        cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            assert!(!editor
                .snapshot
                .borrow()
                .redact_patches
                .contains_key("restored-pixelation"));
        });
        cx.update_window(editor.into(), |_, window, _| window.set_scale_factor(2.))
            .unwrap();
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 0.5;
                cx.notify();
            })
            .unwrap();
        settle(cx, editor.into());
        let (bounds, png) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            (
                editor.bounds.borrow().unwrap(),
                editor.export_png().unwrap(),
            )
        });
        let changed = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_ne!(saved, changed, "DPI changes must resize pixelation blocks");
        let mut composited = image::RgbaImage::from_pixel(changed.width(), changed.height(), matte);
        image::imageops::overlay(&mut composited, &changed, 0, 0);
        let screenshot = cx.capture_screenshot(editor.into()).unwrap();
        let left = (f32::from(bounds.left()) * 2.).round() as u32;
        let top = (f32::from(bounds.top()) * 2.).round() as u32;
        for y in 30..70 {
            for x in 40..120 {
                let actual = screenshot.get_pixel(left + x, top + y).0;
                let expected = composited.get_pixel(x, y).0;
                assert!(
                    actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                    "pixelation after DPI change at {x},{y}: {actual:?} vs {expected:?}"
                );
            }
        }
    }
    cx.update_window(editor.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

fn draw_workflow_annotations(
    cx: &mut HeadlessAppContext,
    editor: gpui::WindowHandle<EditorWindow>,
) -> (
    Vec<crate::editor::annotations::Annotation>,
    Vec<u8>,
    gpui::Bounds<gpui::Pixels>,
) {
    let window = editor.into();
    settle(cx, window);
    click_measured(cx, window, "editor-tools", 0);
    cx.update_window(window, |_, window, cx| {
        window.dispatch_action(Box::new(crate::editor::actions::ZoomReset), cx)
    })
    .unwrap();
    settle(cx, window);
    let bounds = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        assert_eq!(editor.zoom, 1.);
        editor.bounds.borrow().unwrap()
    });
    for (index, start, end) in [
        (0, (80., 80.), (200., 140.)),
        (1, (240., 100.), (380., 100.)),
        (2, (60., 200.), (220., 300.)),
        (8, (400., 200.), (560., 300.)),
    ] {
        click_measured(cx, window, "editor-tools", index);
        let start = bounds.origin + point(px(start.0), px(start.1));
        let end = bounds.origin + point(px(end.0), px(end.1));
        event(
            cx,
            window,
            PlatformInput::MouseDown(MouseDownEvent {
                position: start,
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Modifiers::none(),
                first_mouse: false,
            }),
        );
        event(
            cx,
            window,
            PlatformInput::MouseMove(MouseMoveEvent {
                position: end - point(px(20.), px(10.)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            }),
        );
        event(
            cx,
            window,
            PlatformInput::MouseUp(MouseUpEvent {
                position: end,
                button: MouseButton::Left,
                click_count: 1,
                modifiers: Modifiers::none(),
            }),
        );
        settle(cx, window);
    }
    hover(cx, window, 20., 700.);
    let (annotations, png) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        assert_eq!(editor.history.current().len(), 4);
        assert!(matches!(&editor.history.current()[2], crate::editor::annotations::Annotation::Rectangle { width, height, .. } if *width == 160. && *height == 100.), "the release position must finish the shape");
        assert!(matches!(&editor.history.current()[0], crate::editor::annotations::Annotation::Pen { points, .. } if points.ends_with(&[200., 140.])), "the release position must finish the stroke");
        (
            editor.history.current().to_vec(),
            editor.export_png().unwrap(),
        )
    });
    (annotations, png, bounds)
}

fn annotate_capture_roundtrip(
    cx: &mut HeadlessAppContext,
    editor: gpui::WindowHandle<EditorWindow>,
    source: &Path,
    scratch: &Path,
) {
    let original = image::open(source).unwrap().to_rgba8();
    let (annotations, png, bounds) = draw_workflow_annotations(cx, editor);
    let exported = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(exported.dimensions(), original.dimensions());
    assert_ne!(
        exported, original,
        "annotations must change the captured pixels"
    );
    let rectangle = annotations
        .iter()
        .find_map(|annotation| match annotation {
            crate::editor::annotations::Annotation::Rectangle {
                x,
                y,
                width,
                height,
                fill,
                ..
            } => Some((*x, *y, *width, *height, fill.is_none())),
            _ => None,
        })
        .expect("the workflow must include its rectangle annotation");
    assert_eq!(rectangle, (60., 200., 160., 100., true));
    let background = original.get_pixel(140, 250).0;
    let changed_rectangle_edge = (130..150)
        .flat_map(|x| (197..204).map(move |y| (x, y)))
        .filter(|(x, y)| {
            exported
                .get_pixel(*x, *y)
                .0
                .iter()
                .zip(background)
                .any(|(actual, expected)| actual.abs_diff(expected) > 8)
        })
        .count();
    assert!(
        changed_rectangle_edge >= 10,
        "the rectangle's top edge must render over the captured source"
    );
    assert_eq!(exported.get_pixel(140, 250).0, background);
    let saved = scratch.join("capture-annotated.png");
    std::fs::write(&saved, &png).unwrap();
    assert_eq!(image::open(source).unwrap().to_rgba8(), original);

    let preview = cx.capture_screenshot(editor.into()).unwrap();
    let preview = image::imageops::crop_imm(
        &preview,
        f32::from(bounds.left()).round() as u32,
        f32::from(bounds.top()).round() as u32,
        exported.width(),
        exported.height(),
    )
    .to_image();
    assert!(
        preview
            .pixels()
            .zip(exported.pixels())
            .all(|(actual, expected)| {
                actual
                    .0
                    .iter()
                    .zip(expected.0)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 2)
            }),
        "captured editor preview must match the annotated PNG"
    );

    cx.update_window(editor.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    let reopened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(1.);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, reopened.into());
    cx.update(|cx| {
        let reopened = reopened.read(cx).unwrap();
        assert_eq!(reopened.history.current(), annotations);
        assert_eq!(
            image::load_from_memory(&reopened.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
            exported
        );
    });
    cx.update_window(reopened.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();

    let flattened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(1.);
            cx.new(|cx| EditorWindow::from_file(saved.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, flattened.into());
    cx.update(|cx| {
        let flattened = flattened.read(cx).unwrap();
        assert!(flattened.history.current().is_empty());
        assert_eq!(
            image::load_from_memory(&flattened.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
            exported
        );
    });
    assert_eq!(image::open(saved).unwrap().to_rgba8(), exported);
    cx.update_window(flattened.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    println!("captured screenshot annotations, preview/export, history reopen, and flattened PNG reopen passed");
}

pub(super) fn screenshot(cx: &mut HeadlessAppContext, output: &Path, scratch: &Path) {
    let source = scratch.join("workflow-capture.png");
    std::fs::copy(scratch.join("capture.png"), &source).unwrap();
    crate::history_store::add_item(
        HistoryItem {
            id: "visual-workflow".into(),
            timestamp: 0,
            original_path: source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: None,
            duration: None,
        },
        10,
        true,
    );
    let editor = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(1.);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    let (annotations, png, bounds) = draw_workflow_annotations(cx, editor);
    let window = editor.into();
    let saved = scratch.join("workflow-saved.png");
    std::fs::write(&saved, &png).unwrap();
    capture(cx, editor.into(), output, "workflow-annotated");
    let preview = cx.capture_screenshot(window).unwrap();
    let preview = image::imageops::crop_imm(
        &preview,
        f32::from(bounds.left()).round() as u32,
        f32::from(bounds.top()).round() as u32,
        640,
        360,
    )
    .to_image();
    preview.save(output.join("workflow-preview.png")).unwrap();
    let exported = image::load_from_memory(&png).unwrap().to_rgba8();
    exported.save(output.join("workflow-export.png")).unwrap();
    let significant = preview
        .pixels()
        .zip(exported.pixels())
        .filter(|(actual, expected)| {
            actual
                .0
                .iter()
                .zip(expected.0)
                .any(|(actual, expected)| actual.abs_diff(expected) > 2)
        })
        .count();
    assert_eq!(
        significant, 0,
        "preview/export differ beyond two channel levels"
    );
    cx.update_window(window, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    let reopened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(1.);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, reopened.into());
    cx.update(|cx| {
        let reopened = reopened.read(cx).unwrap();
        assert_eq!(reopened.history.current(), annotations);
        assert_eq!(
            image::load_from_memory(&reopened.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
            exported
        );
    });
    capture(cx, reopened.into(), output, "workflow-history-reopened");
    let flattened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(1.);
            cx.new(|cx| EditorWindow::from_file(saved.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, flattened.into());
    cx.update(|cx| {
        let flattened = flattened.read(cx).unwrap();
        assert!(flattened.history.current().is_empty());
        assert_eq!(
            image::load_from_memory(&flattened.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
            exported
        );
    });
    capture(cx, flattened.into(), output, "workflow-saved-reopened");
    println!("annotation, PNG save, history reopen and flattened reopen passed; preview/export changed pixels above two channel levels: {significant}");

    let delayed_source = scratch.join("workflow-delayed-state.png");
    std::fs::copy(scratch.join("capture.png"), &delayed_source).unwrap();
    crate::history_store::add_item(
        HistoryItem {
            id: "visual-debounced-editor-state".into(),
            timestamp: 0,
            original_path: delayed_source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: None,
            duration: None,
        },
        10,
        true,
    );
    let delayed_editor = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(1.);
            cx.new(|cx| EditorWindow::from_file(delayed_source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    let delayed_window = delayed_editor.into();
    settle(cx, delayed_window);
    click_measured(cx, delayed_window, "editor-tools", 2);
    let delayed_bounds = cx.update(|cx| delayed_editor.read(cx).unwrap().bounds.borrow().unwrap());
    drag_rectangle(
        cx,
        delayed_window,
        delayed_bounds.origin,
        (40., 40.),
        (100., 80.),
    );
    let (delayed_annotations, delayed_png) = cx.update(|cx| {
        let editor = delayed_editor.read(cx).unwrap();
        (
            editor.history.current().to_vec(),
            editor.export_png().unwrap(),
        )
    });
    assert!(crate::history_store::editor_state_for_path(&delayed_source).is_none());
    cx.run_until_parked();
    cx.advance_clock(std::time::Duration::from_millis(200));
    cx.update(|cx| delayed_editor.update(cx, |editor, _, cx| editor.select_all_annotations(cx)))
        .unwrap();
    cx.run_until_parked();
    cx.advance_clock(std::time::Duration::from_millis(299));
    cx.run_until_parked();
    assert!(crate::history_store::editor_state_for_path(&delayed_source).is_none());
    cx.advance_clock(std::time::Duration::from_millis(1));
    cx.run_until_parked();
    assert_eq!(
        crate::history_store::editor_state_for_path(&delayed_source)
            .and_then(|state| state["annotations"]
                .as_array()
                .map(|annotations| annotations.len())),
        Some(1),
        "the debounced editor state must reach history while its window remains open"
    );
    let delayed_reopened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(1.);
            cx.new(|cx| EditorWindow::from_file(delayed_source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, delayed_reopened.into());
    cx.update(|cx| {
        let reopened = delayed_reopened.read(cx).unwrap();
        assert_eq!(reopened.history.current(), delayed_annotations);
        assert_eq!(reopened.export_png().unwrap(), delayed_png);
    });
    cx.update_window(delayed_reopened.into(), |_, window, _| {
        window.remove_window()
    })
    .unwrap();
    cx.update_window(delayed_window, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        crate::history_store::delete_item("visual-debounced-editor-state"),
        crate::history_store::DeleteOutcome::Deleted
    );

    let draft_source = scratch.join("workflow-draft-state.png");
    std::fs::copy(scratch.join("capture.png"), &draft_source).unwrap();
    crate::history_store::add_item(
        HistoryItem {
            id: "visual-draft-flush".into(),
            timestamp: 0,
            original_path: draft_source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: None,
            duration: None,
        },
        10,
        true,
    );
    let draft_editor = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(1.);
            cx.new(|cx| EditorWindow::from_file(draft_source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    let draft_window = draft_editor.into();
    settle(cx, draft_window);
    click_measured(cx, draft_window, "editor-tools", 2);
    let draft_bounds = cx.update(|cx| draft_editor.read(cx).unwrap().bounds.borrow().unwrap());
    drag_rectangle(
        cx,
        draft_window,
        draft_bounds.origin,
        (40., 40.),
        (100., 80.),
    );
    let uncommitted_start = draft_bounds.origin + point(px(140.), px(100.));
    event(
        cx,
        draft_window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: uncommitted_start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    cx.update_window(draft_window, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        crate::history_store::editor_state_for_path(&draft_source)
            .and_then(|state| state["annotations"]
                .as_array()
                .map(|annotations| annotations.len())),
        Some(1),
        "closing during a new draft must persist the last committed annotation"
    );
    assert_eq!(
        crate::history_store::delete_item("visual-draft-flush"),
        crate::history_store::DeleteOutcome::Deleted
    );
    println!("editor history saves after 500ms and reopens while the original editor stays open");
}

fn drag_rectangle(
    cx: &mut HeadlessAppContext,
    window: gpui::AnyWindowHandle,
    origin: gpui::Point<gpui::Pixels>,
    start: (f32, f32),
    end: (f32, f32),
) {
    let start = origin + point(px(start.0), px(start.1));
    let end = origin + point(px(end.0), px(end.1));
    for input in [
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
        PlatformInput::MouseMove(MouseMoveEvent {
            position: end,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        }),
        PlatformInput::MouseUp(MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    ] {
        event(cx, window, input);
    }
}

fn check_circle_selection(
    cx: &mut HeadlessAppContext,
    editor: gpui::WindowHandle<EditorWindow>,
    scale: f32,
    scratch: &Path,
    label: &str,
) {
    let window = editor.into();
    let original = editor
        .update(cx, |editor, _, _| {
            std::mem::replace(
                &mut editor.history,
                crate::editor::annotations::AnnotationHistory::new(Vec::new()),
            )
        })
        .unwrap();
    for (weight, stroke_width) in [("thin", 1.), ("thick", 12.)] {
        editor
            .update(cx, |editor, _, cx| {
                editor.history.replace_current(vec![
                    crate::editor::annotations::Annotation::Circle {
                        id: "overlapped-circle".into(),
                        x: 300.,
                        y: 180.,
                        radius: 70.,
                        stroke: "#ff00ff".into(),
                        stroke_width,
                        fill: Some("#ff00ff".into()),
                    },
                    crate::editor::annotations::Annotation::Highlight {
                        id: "circle-overlap".into(),
                        points: vec![240., 110., 360., 110.],
                        fill: "#00ffff".into(),
                        opacity: 1.,
                        stroke_width: 40.,
                    },
                ]);
                let mut later = editor.history.current().to_vec();
                if let crate::editor::annotations::Annotation::Circle { x, .. } = &mut later[0] {
                    *x += 20.;
                }
                editor.history.push(later);
                assert!(editor.history.undo());
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
        let before = cx.capture_screenshot(window).unwrap();
        let saved = cx.update(|cx| editor.read(cx).unwrap().export_png().unwrap());
        super::click(
            cx,
            window,
            f32::from(bounds.left()) + 300. * 0.73 / scale,
            f32::from(bounds.top()) + 180. * 0.73 / scale,
        );
        cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            assert_eq!(editor.snapshot.borrow().selected, ["overlapped-circle"]);
            assert_eq!(editor.export_png().unwrap(), saved);
            assert!(editor.history.can_redo());
        });
        let selected = cx.capture_screenshot(window).unwrap();
        before
            .save(scratch.join(format!("circle-{label}-{weight}-before.png")))
            .unwrap();
        selected
            .save(scratch.join(format!("circle-{label}-{weight}-selected.png")))
            .unwrap();
        let left = (f32::from(bounds.left()) * scale + 260. * 0.73).round() as u32;
        let right = (f32::from(bounds.left()) * scale + 340. * 0.73).round() as u32;
        let top = (f32::from(bounds.top()) * scale + 150. * 0.73).round() as u32;
        let bottom = (f32::from(bounds.top()) * scale + 210. * 0.73).round() as u32;
        for y in top..bottom {
            for x in left..right {
                assert!(
                    before
                        .get_pixel(x, y)
                        .0
                        .iter()
                        .zip(selected.get_pixel(x, y).0)
                        .all(|(before, selected)| before.abs_diff(selected) <= 2),
                    "selecting {label} {weight} circle changed its interior"
                );
            }
        }
        let content_radius = if stroke_width < 2. { 68. } else { 70. };
        for (x, y) in [
            (300., 180.),
            (300., 180. - content_radius),
            (300. - content_radius, 180.),
            (300. + content_radius, 180.),
            (300., 180. + content_radius),
        ] {
            let x = (f32::from(bounds.left()) * scale + x * 0.73).round() as u32;
            let y = (f32::from(bounds.top()) * scale + y * 0.73).round() as u32;
            assert!(
                before
                    .get_pixel(x, y)
                    .0
                    .iter()
                    .zip(selected.get_pixel(x, y).0)
                    .all(|(before, selected)| before.abs_diff(selected) <= 2),
                "selecting {label} {weight} circle changed its content"
            );
        }
        let inner = 70. + stroke_width as f32 / 2.;
        for (side, x, y, horizontal) in [
            ("top", 300., 180. - inner - 4., false),
            ("bottom", 300., 180. + inner, false),
            ("left", 300. - inner - 4., 180., true),
            ("right", 300. + inner, 180., true),
        ] {
            let difference = (0..=4)
                .flat_map(|step| {
                    let x = (f32::from(bounds.left()) * scale
                        + (x + if horizontal { step as f32 } else { 0. }) * 0.73)
                        .round() as u32;
                    let y = (f32::from(bounds.top()) * scale
                        + (y + if horizontal { 0. } else { step as f32 }) * 0.73)
                        .round() as u32;
                    before
                        .get_pixel(x, y)
                        .0
                        .into_iter()
                        .zip(selected.get_pixel(x, y).0)
                        .map(|(before, selected)| before.abs_diff(selected))
                })
                .max()
                .unwrap();
            assert!(
                difference > 10,
                "{label} {weight} circle {side} halo is hidden"
            );
        }
        cx.update_window(window, |_, window, cx| {
            window.dispatch_action(Box::new(crate::editor::actions::Redo), cx);
        })
        .unwrap();
        cx.update(|cx| {
            assert!(matches!(
                &editor.read(cx).unwrap().history.current()[0],
                crate::editor::annotations::Annotation::Circle { x, .. } if *x == 320.
            ));
        });
        cx.update_window(window, |_, window, cx| {
            window.dispatch_action(Box::new(crate::editor::actions::Undo), cx);
        })
        .unwrap();
        cx.update(|cx| {
            assert!(matches!(
                &editor.read(cx).unwrap().history.current()[0],
                crate::editor::annotations::Annotation::Circle { x, .. } if *x == 300.
            ));
        });
        super::click(
            cx,
            window,
            f32::from(bounds.left()) + 500. * 0.73 / scale,
            f32::from(bounds.top()) + 300. * 0.73 / scale,
        );
        cx.update(|cx| {
            assert!(editor
                .read(cx)
                .unwrap()
                .snapshot
                .borrow()
                .selected
                .is_empty());
        });
    }
    editor
        .update(cx, |editor, _, cx| {
            editor.history = original;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
}

pub(super) fn scaled_wallpaper_preview(cx: &mut HeadlessAppContext, scratch: &Path) {
    let source = scratch.join("scaled-wallpaper.png");
    image::RgbaImage::from_fn(640, 360, |x, y| {
        if (240..320).contains(&x) && (120..180).contains(&y) {
            return image::Rgba([240, 80, 64, 128]);
        }
        if (x / 80 + y / 60) % 2 == 0 {
            image::Rgba([240, 80, 64, 255])
        } else {
            image::Rgba([64, 112, 192, 255])
        }
    })
    .save(&source)
    .unwrap();
    crate::history_store::add_item(
        HistoryItem {
            id: "layered-wallpaper-workflow".into(),
            timestamp: 0,
            original_path: source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: None,
            duration: None,
        },
        100,
        true,
    );
    let editor = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(2.);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    let window = editor.into();
    editor
        .update(cx, |editor, _, cx| {
            editor
                .wallpaper
                .set_gradient(crate::editor::wallpaper::preset("slate"));
            editor.wallpaper.padding = 20.;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let previous = cx.update(|cx| {
        editor
            .read(cx)
            .unwrap()
            .snapshot
            .borrow()
            .backdrop
            .clone()
            .unwrap()
    });
    let original_mode = cx.update(|cx| crate::theme::vars::active_mode(cx));
    let other_mode = match original_mode {
        crate::theme::presets::ThemeMode::Light => crate::theme::presets::ThemeMode::Dark,
        _ => crate::theme::presets::ThemeMode::Light,
    };
    cx.update(|cx| crate::theme::vars::update_theme(cx, other_mode, "default"));
    super::frame(cx, window);
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        assert!(editor
            .snapshot
            .borrow()
            .backdrop
            .as_ref()
            .is_some_and(|backdrop| { std::sync::Arc::ptr_eq(backdrop, &previous) }));
    });
    cx.update(|cx| crate::theme::vars::update_theme(cx, original_mode, "default"));
    let stage_matte = cx.update(|cx| {
        let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
        image::Rgba([
            (theme.r * 255.).round() as u8,
            (theme.g * 255.).round() as u8,
            (theme.b * 255.).round() as u8,
            255,
        ])
    });
    super::frame(cx, window);
    editor
        .update(cx, |editor, _, cx| {
            editor.wallpaper.background_blur = 30.;
            cx.notify();
        })
        .unwrap();
    super::frame(cx, window);
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let snapshot = editor.snapshot.borrow();
        assert!(snapshot
            .backdrop
            .as_ref()
            .is_some_and(|backdrop| std::sync::Arc::ptr_eq(backdrop, &previous)));
    });
    cx.update_window(window, |_, window, _| window.set_scale_factor(1.25))
        .unwrap();
    super::frame(cx, window);
    cx.update(|cx| {
        assert!(editor
            .read(cx)
            .unwrap()
            .snapshot
            .borrow()
            .backdrop
            .is_none());
    });
    settle(cx, window);
    cx.update(|cx| {
        assert!(editor
            .read(cx)
            .unwrap()
            .snapshot
            .borrow()
            .backdrop
            .is_some());
    });
    editor
        .update(cx, |editor, _, cx| {
            editor.wallpaper.padding = 40.;
            cx.notify();
        })
        .unwrap();
    super::frame(cx, window);
    cx.update(|cx| {
        assert!(editor
            .read(cx)
            .unwrap()
            .snapshot
            .borrow()
            .backdrop
            .is_none());
    });
    let alpha_background = scratch.join("scaled-wallpaper-alpha-background.png");
    image::RgbaImage::from_pixel(64, 64, image::Rgba([32, 96, 160, 128]))
        .save(&alpha_background)
        .unwrap();
    let pattern_background = scratch.join("scaled-wallpaper-pattern-background.png");
    image::RgbaImage::from_fn(64, 48, |x, y| {
        image::Rgba([
            ((x * 37 + y * 3) % 256) as u8,
            ((y * 53 + x * 5) % 256) as u8,
            ((x * 11 + y * 7) % 256) as u8,
            ((x * 19 + y * 17) % 256) as u8,
        ])
    })
    .save(&pattern_background)
    .unwrap();
    for (scale, padding, background, blurred) in [
        (1.25_f32, 25_u32, "slate", false),
        (2., 40, "slate", false),
        (1.25, 25, "none", false),
        (2., 40, "none", false),
        (1.25, 25, "alpha-gradient", false),
        (2., 40, "alpha-gradient", false),
        (1.25, 25, "alpha-image", false),
        (2., 40, "alpha-image", false),
        (1.25, 25, "pattern-image", false),
        (2., 40, "pattern-image", false),
        (1.25, 25, "pattern-image", true),
        (2., 40, "pattern-image", true),
        (1.25, 25, "slate", true),
        (2., 40, "slate", true),
    ] {
        editor
            .update(cx, |editor, _, cx| {
                match background {
                    "none" => editor.wallpaper.set_gradient(None),
                    "slate" => editor
                        .wallpaper
                        .set_gradient(crate::editor::wallpaper::preset("slate")),
                    "alpha-gradient" => editor.wallpaper.set_gradient(Some(
                        crate::editor::wallpaper::GradientOption {
                            id: "alpha-background".into(),
                            colors: vec!["#2060a080".into(), "#2060a080".into()],
                            angle: 0.,
                        },
                    )),
                    "alpha-image" => editor.wallpaper.set_background_image(Some(
                        alpha_background.to_string_lossy().into_owned(),
                    )),
                    "pattern-image" => editor.wallpaper.set_background_image(Some(
                        pattern_background.to_string_lossy().into_owned(),
                    )),
                    _ => unreachable!(),
                }
                editor.wallpaper.padding = 20.;
                editor.wallpaper.corners = 0.;
                editor.wallpaper.background_blur = if blurred { 30. } else { 0. };
                editor.wallpaper.shadow = 0.;
                cx.notify();
            })
            .unwrap();
        cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
            .unwrap();
        settle(cx, window);
        cx.update(|cx| {
            editor
                .update(cx, |editor, _, cx| {
                    editor.zoom = 1. / scale;
                    cx.notify();
                })
                .unwrap();
        });
        cx.advance_clock(std::time::Duration::from_millis(32));
        settle(cx, window);
        let (bounds, saved) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            (
                editor.bounds.borrow().unwrap(),
                image::load_from_memory(&editor.export_png().unwrap())
                    .unwrap()
                    .to_rgba8(),
            )
        });
        assert_eq!(saved.dimensions(), (640 + 2 * padding, 360 + 2 * padding));
        if background == "pattern-image" {
            saved
                .save(scratch.join(format!("scaled-wallpaper-pattern-{scale}-{blurred}.png")))
                .unwrap();
        }
        if background != "slate" && background != "pattern-image" {
            assert_eq!(
                saved.get_pixel(0, 0).0[3],
                if background == "none" { 0 } else { 128 }
            );
        }
        let mut expected = saved.clone();
        if saved.pixels().any(|pixel| pixel.0[3] != 255) {
            expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), stage_matte);
            image::imageops::overlay(&mut expected, &saved, 0, 0);
        }
        let screenshot = cx.capture_screenshot(window).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32 - padding;
        let top = (f32::from(bounds.top()) * scale).round() as u32 - padding;
        let preview =
            image::imageops::crop_imm(&screenshot, left, top, saved.width(), saved.height())
                .to_image();
        let significant = preview
            .pixels()
            .zip(expected.pixels())
            .filter(|(actual, expected)| {
                actual
                    .0
                    .iter()
                    .zip(expected.0)
                    .any(|(actual, expected)| actual.abs_diff(expected) > 2)
            })
            .count();
        if significant > 0 {
            preview
                .save(scratch.join("scaled-wallpaper-preview.png"))
                .unwrap();
            expected
                .save(scratch.join("scaled-wallpaper-expected.png"))
                .unwrap();
        }
        assert_eq!(
            significant, 0,
            "wallpaper preview/export at {scale}x, background: {background}, blurred: {blurred}"
        );
    }
    for (scale, transparent, radius, shadow, padding_setting) in [
        (1.25_f32, true, 24., 0., 0.),
        (2., true, 24., 0., 0.),
        (1.25, true, 0., 50., 0.),
        (2., true, 24., 50., 0.),
        (1.25, false, 1., 0., 20.),
        (1.25, false, 24., 0., 20.),
        (1.25, true, 24., 0., 20.),
        (2., false, 200., 0., 20.),
        (2., false, 24., 0., 20.),
        (1.25, false, 24., 50., 20.),
        (1.25, true, 0., 50., 20.),
        (2., false, 0., 150., 20.),
        (2., true, 24., 50., 20.),
    ] {
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.set_gradient((!transparent).then(|| {
                    crate::editor::wallpaper::GradientOption {
                        id: "visual-solid".into(),
                        colors: vec!["#2468ac".into()],
                        angle: 0.,
                    }
                }));
                editor.wallpaper.padding = padding_setting;
                editor.wallpaper.corners = radius;
                editor.wallpaper.background_blur = 0.;
                editor.wallpaper.shadow = shadow;
                cx.notify();
            })
            .unwrap();
        cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
            .unwrap();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        if padding_setting == 0. && radius == 24. && shadow == 0. && scale == 1.25 {
            let previous = cx.update(|cx| {
                editor
                    .read(cx)
                    .unwrap()
                    .snapshot
                    .borrow()
                    .backdrop
                    .clone()
                    .unwrap()
            });
            cx.update(|cx| crate::theme::vars::update_theme(cx, other_mode, "default"));
            settle(cx, window);
            let (bounds, new_backdrop, other_matte) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
                (
                    editor.bounds.borrow().unwrap(),
                    editor.snapshot.borrow().backdrop.clone().unwrap(),
                    [
                        (theme.r * 255.).round() as u8,
                        (theme.g * 255.).round() as u8,
                        (theme.b * 255.).round() as u8,
                    ],
                )
            });
            assert_ne!(
                previous.id, new_backdrop.id,
                "theme change must replace the transparent backdrop"
            );
            let screenshot = cx.capture_screenshot(window).unwrap();
            let x = (f32::from(bounds.left()) * scale).round() as u32;
            let y = (f32::from(bounds.top()) * scale).round() as u32;
            assert!(
                screenshot.get_pixel(x, y).0[..3]
                    .iter()
                    .zip(other_matte)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                "transparent corner must use the new theme background"
            );
            #[cfg(windows)]
            cx.update_window(window, |_, window, _| {
                assert!(
                    !window.has_image_atlas_entry(&previous),
                    "the old theme backdrop must leave the GPU atlas"
                );
            })
            .unwrap();
            cx.update(|cx| crate::theme::vars::update_theme(cx, original_mode, "default"));
            settle(cx, window);
        }
        let (bounds, saved) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            (
                editor.bounds.borrow().unwrap(),
                image::load_from_memory(&editor.export_png().unwrap())
                    .unwrap()
                    .to_rgba8(),
            )
        });
        let screenshot = cx.capture_screenshot(window).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32;
        let top = (f32::from(bounds.top()) * scale).round() as u32;
        let padding = (padding_setting as f32 * scale).round() as u32;
        let mut expected = saved.clone();
        if transparent {
            expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), stage_matte);
            image::imageops::overlay(&mut expected, &saved, 0, 0);
        }
        let mut worst = 0;
        let mut edge_outliers = 0;
        let mismatched = (0..expected.height())
            .flat_map(|y| (0..expected.width()).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let difference = screenshot
                    .get_pixel(left - padding + x, top - padding + y)
                    .0
                    .iter()
                    .zip(expected.get_pixel(x, y).0)
                    .map(|(actual, expected)| actual.abs_diff(expected))
                    .max()
                    .unwrap_or(0);
                worst = worst.max(difference);
                if difference > 2 && transparent && radius > 0. && shadow > 0. {
                    let radius = (radius * f64::from(scale)).min(180.);
                    let x = f64::from(x) - f64::from(padding) + 0.5;
                    let y = f64::from(y) - f64::from(padding) + 0.5;
                    let dx = x - x.clamp(radius, 640. - radius);
                    let dy = y - y.clamp(radius, 360. - radius);
                    if (dx.hypot(dy) - radius).abs() <= 1.5 {
                        edge_outliers += 1;
                    }
                }
                difference > 2
            })
            .count();
        if mismatched > 0 {
            image::imageops::crop_imm(
                &screenshot,
                left - padding,
                top - padding,
                expected.width(),
                expected.height(),
            )
            .to_image()
            .save(scratch.join("rounded-preview.png"))
            .unwrap();
            expected.save(scratch.join("rounded-export.png")).unwrap();
        }
        assert!(
            mismatched == edge_outliers && edge_outliers <= 1 && worst <= 3,
            "rounded preview differs from saved pixels at {scale}x, transparent {transparent}, radius {radius}, shadow {shadow}, padding {padding_setting}: {mismatched} pixels, worst difference {worst}"
        );
    }
    editor
        .update(cx, |editor, _, cx| {
            editor
                .history
                .push(vec![crate::editor::annotations::Annotation::Rectangle {
                    id: "rounded-edge".into(),
                    x: 0.,
                    y: 330.,
                    width: 40.,
                    height: 30.,
                    stroke: "#00ff00".into(),
                    stroke_width: 1.,
                    fill: Some("#00ff00".into()),
                }]);
            editor.tool = crate::ui::colors::Tool::Select;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let (bounds, saved) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        (
            editor.bounds.borrow().unwrap(),
            image::load_from_memory(&editor.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
        )
    });
    let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), stage_matte);
    image::imageops::overlay(&mut expected, &saved, 0, 0);
    let screenshot = cx.capture_screenshot(window).unwrap();
    let left = (f32::from(bounds.left()) * 2.).round() as u32;
    let top = (f32::from(bounds.top()) * 2.).round() as u32;
    for y in 348..360 {
        for x in 0..12 {
            assert!(
                screenshot
                    .get_pixel(left + x, top + y)
                    .0
                    .iter()
                    .zip(expected.get_pixel(40 + x, 40 + y).0)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                "rounded annotation differs at {x},{y}"
            );
        }
    }
    super::click(
        cx,
        window,
        f32::from(bounds.left()) + 10.,
        f32::from(bounds.top()) + 172.5,
    );
    settle(cx, window);
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).unwrap().snapshot.borrow().selected,
            ["rounded-edge"]
        );
    });
    let selected = cx.capture_screenshot(window).unwrap();
    let foreground =
        cx.update(|cx| gpui::Rgba::from(crate::theme::vars::active_theme(cx).primary_foreground));
    for (x, y) in [(-2, -6), (2, -6), (-2, 2), (2, 2)] {
        assert!(
            selected
                .get_pixel((left as i32 + x) as u32, (top as i32 + 360 + y) as u32)
                .0[..3]
                .iter()
                .zip([foreground.r, foreground.g, foreground.b])
                .all(|(actual, expected)| (f32::from(*actual) - expected * 255.).abs() <= 2.),
            "the rounded selection handle must remain visible at {x},{y}"
        );
    }
    editor
        .update(cx, |editor, _, cx| {
            editor.history.push(Vec::new());
            editor.tool = crate::ui::colors::Tool::Crop;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let start = bounds.origin + point(px(50.), px(50.));
    let end = bounds.origin + point(px(150.), px(110.));
    event(
        cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
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
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, window);
    let cropped = cx.capture_screenshot(window).unwrap();
    for (actual, original) in cropped.get_pixel(left, top).0[..3]
        .iter()
        .zip(screenshot.get_pixel(left, top).0[..3].iter())
    {
        assert!(
            actual.abs_diff(*original / 2) <= 1,
            "crop must dim the rounded cutout"
        );
    }
    editor
        .update(cx, |editor, _, cx| editor.cancel_crop(cx))
        .unwrap();
    let start = bounds.origin + point(px(0.5), px(0.5));
    let end = bounds.origin + point(px(35.), px(25.));
    event(
        cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
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
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, window);
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).unwrap().snapshot.borrow().crop,
            Some((1., 1., 69., 49.))
        );
    });
    let cropped = cx.capture_screenshot(window).unwrap();
    for (x, y) in [(-2, -2), (2, -2), (-2, 2), (2, 2)] {
        assert!(
            cropped
                .get_pixel((left as i32 + x) as u32, (top as i32 + y) as u32)
                .0[..3]
                .iter()
                .zip([foreground.r, foreground.g, foreground.b])
                .all(|(actual, expected)| (f32::from(*actual) - expected * 255.).abs() <= 2.),
            "the rounded crop handle must remain visible at {x},{y}"
        );
    }
    editor
        .update(cx, |editor, _, cx| editor.cancel_crop(cx))
        .unwrap();
    for (scale, style, content) in [
        (1.25_f32, "windows-dark", [18_u8, 18, 18]),
        (2., "windows-dark", [18, 18, 18]),
        (1.25, "windows-light", [255, 255, 255]),
        (2., "windows-light", [255, 255, 255]),
        (1.25, "macos-dark", [28, 28, 30]),
        (2., "macos-dark", [28, 28, 30]),
        (1.25, "macos-light", [255, 255, 255]),
        (2., "macos-light", [255, 255, 255]),
    ] {
        for shadow in [0., 50.] {
            #[cfg(windows)]
            let old_edges = cx.update(|cx| {
                editor
                    .read(cx)
                    .unwrap()
                    .snapshot
                    .borrow()
                    .preview_edges
                    .iter()
                    .map(|edge| edge.image.clone())
                    .collect::<Vec<_>>()
            });
            #[cfg(windows)]
            if !old_edges.is_empty() {
                cx.update_window(window, |_, window, _| {
                    assert!(
                        old_edges
                            .iter()
                            .any(|image| window.has_image_atlas_entry(image)),
                        "the previous frame edges must have been painted"
                    );
                })
                .unwrap();
            }
            editor
                .update(cx, |editor, _, cx| {
                    editor
                        .wallpaper
                        .set_gradient(Some(crate::editor::wallpaper::GradientOption {
                            id: "visual-solid".into(),
                            colors: vec!["#2468ac".into()],
                            angle: 0.,
                        }));
                    editor.wallpaper.padding = 20.;
                    editor.wallpaper.corners = 0.;
                    editor.wallpaper.background_blur = 0.;
                    editor.wallpaper.window_frame.style = style.into();
                    editor.wallpaper.shadow = shadow;
                    cx.notify();
                })
                .unwrap();
            cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
                .unwrap();
            settle(cx, window);
            editor
                .update(cx, |editor, _, cx| {
                    editor.zoom = 1. / scale;
                    cx.notify();
                })
                .unwrap();
            cx.advance_clock(std::time::Duration::from_millis(32));
            settle(cx, window);
            #[cfg(windows)]
            cx.update_window(window, |_, window, _| {
                for image in &old_edges {
                    assert!(
                        !window.has_image_atlas_entry(image),
                        "replaced frame edges must leave the GPU atlas"
                    );
                }
            })
            .unwrap();
            let (bounds, saved) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                (
                    editor.bounds.borrow().unwrap(),
                    image::load_from_memory(&editor.export_png().unwrap())
                        .unwrap()
                        .to_rgba8(),
                )
            });
            let padding = (20. * scale).round() as u32;
            let title = (28. * scale).round() as u32;
            let screenshot = cx.capture_screenshot(window).unwrap();
            let left = (f32::from(bounds.left()) * scale).round() as u32;
            let top = (f32::from(bounds.top()) * scale).round() as u32;
            let expected = saved.get_pixel(padding + 250, padding + title + 130);
            let background = if shadow > 0. { content } else { [36, 104, 172] };
            for (actual, (foreground, background)) in expected.0[..3]
                .iter()
                .zip([240_u8, 80, 64].into_iter().zip(background))
            {
                let blended = (u16::from(foreground) * 128 + u16::from(background) * 127) / 255;
                assert!(actual.abs_diff(blended as u8) <= 2);
            }
            let significant = (120..180)
                .flat_map(|y| (240..320).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    screenshot
                        .get_pixel(left + x, top + y)
                        .0
                        .iter()
                        .zip(saved.get_pixel(padding + x, padding + title + y).0)
                        .any(|(actual, expected)| actual.abs_diff(expected) > 2)
                })
                .count();
            assert_eq!(
                significant, 0,
                "transparent framed preview at {scale}x, {style}, shadow {shadow}: preview {:?}, saved {:?}",
                screenshot.get_pixel(left + 250, top + 130),
                saved.get_pixel(padding + 250, padding + title + 130)
            );
            let mismatched = (0..360)
                .flat_map(|y| (0..640).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    screenshot
                        .get_pixel(left + x, top + y)
                        .0
                        .iter()
                        .zip(saved.get_pixel(padding + x, padding + title + y).0)
                        .any(|(actual, expected)| actual.abs_diff(expected) > 2)
                })
                .count();
            if mismatched > 0 {
                image::imageops::crop_imm(&screenshot, left, top, 640, 360)
                    .to_image()
                    .save(scratch.join("framed-preview.png"))
                    .unwrap();
                image::imageops::crop_imm(&saved, padding, padding + title, 640, 360)
                    .to_image()
                    .save(scratch.join("framed-export.png"))
                    .unwrap();
            }
            assert_eq!(
                mismatched, 0,
                "framed preview differs from saved pixels at {scale}x, {style}, shadow {shadow}"
            );
        }
    }
    editor
        .update(cx, |editor, _, cx| {
            editor.history.push(
                [("left", 0.), ("right", 600.)]
                    .into_iter()
                    .map(
                        |(id, x)| crate::editor::annotations::Annotation::Rectangle {
                            id: id.into(),
                            x,
                            y: 330.,
                            width: 40.,
                            height: 30.,
                            stroke: "#00ff00".into(),
                            stroke_width: 1.,
                            fill: Some("#00ff00".into()),
                        },
                    )
                    .collect(),
            );
            cx.notify();
        })
        .unwrap();
    for (scale, style) in [(1.25_f32, "macos-light"), (2., "windows-dark")] {
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.window_frame.style = style.into();
                editor.wallpaper.shadow = 0.;
                cx.notify();
            })
            .unwrap();
        cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
            .unwrap();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        let (bounds, saved) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            (
                editor.bounds.borrow().unwrap(),
                image::load_from_memory(&editor.export_png().unwrap())
                    .unwrap()
                    .to_rgba8(),
            )
        });
        let padding = (20. * scale).round() as u32;
        let title = (28. * scale).round() as u32;
        let screenshot = cx.capture_screenshot(window).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32;
        let top = (f32::from(bounds.top()) * scale).round() as u32;
        for y in 348..360 {
            for x in (0..12).chain(628..640) {
                assert!(
                    screenshot
                        .get_pixel(left + x, top + y)
                        .0
                        .iter()
                        .zip(saved.get_pixel(padding + x, padding + title + y).0)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                    "framed annotation differs at {x},{y}, {scale}x, {style}"
                );
            }
        }
    }
    editor
        .update(cx, |editor, _, cx| {
            editor.tool = crate::ui::colors::Tool::Select;
            cx.notify();
        })
        .unwrap();
    let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
    super::click(
        cx,
        window,
        f32::from(bounds.left()) + 20. * 0.5,
        f32::from(bounds.top()) + 345. * 0.5,
    );
    settle(cx, window);
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).unwrap().snapshot.borrow().selected,
            ["left"]
        );
    });
    let selected = cx.capture_screenshot(window).unwrap();
    let left = (f32::from(bounds.left()) * 2.).round() as u32;
    let top = (f32::from(bounds.top()) * 2.).round() as u32;
    let foreground =
        cx.update(|cx| gpui::Rgba::from(crate::theme::vars::active_theme(cx).primary_foreground));
    for (x, y) in [(-2, -6), (2, -6), (-2, 2), (2, 2)] {
        assert!(
            selected
                .get_pixel((left as i32 + x) as u32, (top as i32 + 360 + y) as u32)
                .0[..3]
                .iter()
                .zip([foreground.r, foreground.g, foreground.b])
                .all(|(actual, expected)| (f32::from(*actual) - expected * 255.).abs() <= 2.),
            "the selected bottom-left handle must remain visible at {x},{y}"
        );
    }
    let start = bounds.origin + point(px(40. * 0.5), px(330. * 0.5));
    let end = bounds.origin + point(px(50. * 0.5), px(320. * 0.5));
    event(
        cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
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
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, window);
    cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        assert_eq!(
            editor.history.current()[0].bounds(),
            (0., 320., 50., 360.),
            "the framed annotation handle must resize the shape"
        );
    });
    let start = bounds.origin + point(px(-1.), px(180.5));
    let end = bounds.origin + point(px(5.), px(175.5));
    event(
        cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
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
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, window);
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).unwrap().history.current()[0].bounds(),
            (10., 320., 50., 350.),
            "the part of the handle outside the image must resize the shape"
        );
    });
    editor
        .update(cx, |editor, _, cx| {
            editor.history.push(Vec::new());
            editor.tool = crate::ui::colors::Tool::Crop;
            cx.notify();
        })
        .unwrap();
    let start = bounds.origin + point(px(0.5), px(0.5));
    let end = bounds.origin + point(px(35.), px(25.));
    event(
        cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
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
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, window);
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).unwrap().snapshot.borrow().crop,
            Some((1., 1., 69., 49.)),
            "the framed crop must track the pointer"
        );
    });
    let cropped = cx.capture_screenshot(window).unwrap();
    for (x, y) in [(-2, -2), (2, -2), (-2, 2), (2, 2)] {
        assert!(
            cropped
                .get_pixel((left as i32 + x) as u32, (top as i32 + y) as u32)
                .0[..3]
                .iter()
                .zip([foreground.r, foreground.g, foreground.b])
                .all(|(actual, expected)| (f32::from(*actual) - expected * 255.).abs() <= 2.),
            "the crop handle must remain visible at {x},{y}"
        );
    }
    let transparent_wallpaper = scratch.join("transparent-wallpaper.png");
    image::RgbaImage::from_fn(32, 32, |x, y| {
        image::Rgba([100, 150, 200, [0, 128, 255][((x / 8 + y / 8) % 3) as usize]])
    })
    .save(&transparent_wallpaper)
    .unwrap();
    editor
        .update(cx, |editor, _, cx| {
            editor.cancel_crop(cx);
            editor
                .wallpaper
                .set_background_image(Some(transparent_wallpaper.to_string_lossy().into_owned()));
            editor.wallpaper.padding = 20.;
            cx.notify();
        })
        .unwrap();
    let original_mode = cx.update(|cx| crate::theme::vars::active_mode(cx));
    for mode in [
        crate::theme::presets::ThemeMode::Light,
        crate::theme::presets::ThemeMode::Dark,
    ] {
        cx.update(|cx| crate::theme::vars::update_theme(cx, mode, "default"));
        for (scale, style, shadow, blur) in [
            (1.25_f32, "windows-dark", 0., 0.),
            (2., "macos-light", 50., 0.),
            (1.25, "macos-light", 0., 30.),
            (2., "windows-dark", 50., 30.),
            (1.25, "none", 0., 0.),
            (1.25, "none", 50., 0.),
            (2., "none", 50., 30.),
            (2., "none", 0., 30.),
        ] {
            editor
                .update(cx, |editor, _, cx| {
                    editor.wallpaper.window_frame.style = style.into();
                    editor.wallpaper.corners = if style == "none" { 24. } else { 0. };
                    editor.wallpaper.shadow = shadow;
                    editor.wallpaper.background_blur = blur;
                    cx.notify();
                })
                .unwrap();
            cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
                .unwrap();
            settle(cx, window);
            editor
                .update(cx, |editor, _, cx| {
                    editor.zoom = 1. / scale;
                    cx.notify();
                })
                .unwrap();
            settle(cx, window);
            let (bounds, saved, background) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                let background = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
                (
                    editor.bounds.borrow().unwrap(),
                    image::load_from_memory(&editor.export_png().unwrap())
                        .unwrap()
                        .to_rgba8(),
                    image::Rgba([
                        (background.r * 255.).round() as u8,
                        (background.g * 255.).round() as u8,
                        (background.b * 255.).round() as u8,
                        255,
                    ]),
                )
            });
            let mut expected =
                image::RgbaImage::from_pixel(saved.width(), saved.height(), background);
            image::imageops::overlay(&mut expected, &saved, 0, 0);
            let screenshot = cx.capture_screenshot(window).unwrap();
            let left = (f32::from(bounds.left()) * scale).round() as u32;
            let top = (f32::from(bounds.top()) * scale).round() as u32;
            let padding = (20. * scale).round() as u32;
            let title = if style == "none" {
                0
            } else {
                (28. * scale).round() as u32
            };
            let mismatched = (0..360)
                .flat_map(|y| (0..640).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    screenshot
                        .get_pixel(left + x, top + y)
                        .0
                        .iter()
                        .zip(expected.get_pixel(padding + x, padding + title + y).0)
                        .any(|(actual, expected)| {
                            actual.abs_diff(expected)
                                > if style != "none" && (!(20..620).contains(&x) || y >= 340) {
                                    2
                                } else {
                                    3
                                }
                        })
                })
                .count();
            if mismatched > 0 {
                image::imageops::crop_imm(&screenshot, left, top, 640, 360)
                    .to_image()
                    .save(scratch.join("transparent-custom-preview.png"))
                    .unwrap();
                image::imageops::crop_imm(&expected, padding, padding + title, 640, 360)
                    .to_image()
                    .save(scratch.join("transparent-custom-expected.png"))
                    .unwrap();
            }
            assert_eq!(
                mismatched, 0,
                "transparent custom framed wallpaper differs at {scale}x, {style}, shadow {shadow}, blur {blur}, {mode:?}"
            );
            if style == "none" && shadow > 0. {
                let mismatched = (0..expected.height())
                    .flat_map(|y| (0..expected.width()).map(move |x| (x, y)))
                    .filter(|&(x, y)| {
                        screenshot
                            .get_pixel(left - padding + x, top - padding + y)
                            .0
                            .iter()
                            .zip(expected.get_pixel(x, y).0)
                            .any(|(actual, expected)| actual.abs_diff(expected) > 3)
                    })
                    .count();
                assert_eq!(
                    mismatched, 0,
                    "transparent custom shadow differs at {scale}x, blur {blur}, {mode:?}"
                );
            }
        }
    }
    cx.update(|cx| crate::theme::vars::update_theme(cx, original_mode, "default"));
    editor
        .update(cx, |editor, _, cx| {
            editor.wallpaper.set_gradient(None);
            editor.wallpaper.padding = 20.;
            editor.wallpaper.shadow = 0.;
            editor.wallpaper.corners = 0.;
            editor.wallpaper.window_frame.style = "macos-light".into();
            cx.notify();
        })
        .unwrap();
    cx.update_window(window, |_, window, _| window.set_scale_factor(1.25))
        .unwrap();
    settle(cx, window);
    editor
        .update(cx, |editor, _, cx| {
            editor.zoom = 1. / 1.25;
            cx.notify();
        })
        .unwrap();
    cx.advance_clock(std::time::Duration::from_millis(32));
    settle(cx, window);
    let (bounds, saved) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        (
            editor.bounds.borrow().unwrap(),
            image::load_from_memory(&editor.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
        )
    });
    let screenshot = cx.capture_screenshot(window).unwrap();
    let left = (f32::from(bounds.left()) * 1.25).round() as u32;
    let top = (f32::from(bounds.top()) * 1.25).round() as u32;
    let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), stage_matte);
    image::imageops::overlay(&mut expected, &saved, 0, 0);
    for y in 0..360 {
        for x in 0..640 {
            assert!(
                screenshot
                    .get_pixel(left + x, top + y)
                    .0
                    .iter()
                    .zip(expected.get_pixel(25 + x, 25 + 35 + y).0)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                "transparent framed preview differs at {x},{y}"
            );
        }
    }
    for y in 120..180 {
        for x in 240..320 {
            let pixel = saved.get_pixel(25 + x, 25 + 35 + y).0;
            assert_eq!(pixel[3], 128);
            for (actual, (foreground, background)) in screenshot.get_pixel(left + x, top + y).0[..3]
                .iter()
                .zip(pixel[..3].iter().zip(stage_matte.0[..3].iter().copied()))
            {
                let blended = (u16::from(*foreground) * 128 + u16::from(background) * 127) / 255;
                assert!(actual.abs_diff(blended as u8) <= 2);
            }
        }
    }
    for scale in [1.25_f32, 2.] {
        editor
            .update(cx, |editor, _, cx| {
                editor.wallpaper.padding = 0.;
                editor.wallpaper.window_frame.style = "windows-dark".into();
                cx.notify();
            })
            .unwrap();
        cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
            .unwrap();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        cx.advance_clock(std::time::Duration::from_millis(32));
        settle(cx, window);
        let (bounds, saved) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            (
                editor.bounds.borrow().unwrap(),
                image::load_from_memory(&editor.export_png().unwrap())
                    .unwrap()
                    .to_rgba8(),
            )
        });
        let title = (28. * scale).round() as u32;
        assert_eq!(saved.dimensions(), (640, 360 + title));
        let screenshot = cx.capture_screenshot(window).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32;
        let top = (f32::from(bounds.top()) * scale).round() as u32;
        assert_eq!(saved.get_pixel(320, 10).0, [32, 32, 32, 255]);
        assert_eq!(
            screenshot.get_pixel(left + 320, top - title + 10),
            saved.get_pixel(320, 10)
        );
        let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), stage_matte);
        image::imageops::overlay(&mut expected, &saved, 0, 0);
        for y in 0..360 {
            for x in 0..640 {
                assert!(
                    screenshot
                        .get_pixel(left + x, top + y)
                        .0
                        .iter()
                        .zip(expected.get_pixel(x, title + y).0)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                    "frame-only preview differs at {x},{y}, {scale}x"
                );
            }
        }
        for y in 120..180 {
            for x in 240..320 {
                let pixel = saved.get_pixel(x, title + y).0;
                assert_eq!(pixel[3], 128);
                for (actual, (foreground, background)) in screenshot.get_pixel(left + x, top + y).0
                    [..3]
                    .iter()
                    .zip(pixel[..3].iter().zip(stage_matte.0[..3].iter().copied()))
                {
                    let blended =
                        (u16::from(*foreground) * 128 + u16::from(background) * 127) / 255;
                    assert!(actual.abs_diff(blended as u8) <= 2);
                }
            }
        }
    }
    let side = scratch.join("wallpaper-layer-side.png");
    let vertical = scratch.join("wallpaper-layer-vertical.png");
    for (path, width, height) in [(&side, 120, 360), (&vertical, 640, 90)] {
        image::RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([
                40,
                190,
                80,
                if (x / 30 + y / 30) % 3 == 0 { 128 } else { 255 },
            ])
        })
        .save(path)
        .unwrap();
    }
    let inset_side = scratch.join("wallpaper-layer-inset-side.png");
    image::RgbaImage::from_fn(120, 360, |x, y| {
        if y == 0 || (x == 0 && (30..60).contains(&y)) {
            image::Rgba([240, 20, 40, 0])
        } else if (20..100).contains(&x)
            && ((105..155).contains(&y) || (275..325).contains(&y))
            && (x + y) % 2 == 0
        {
            image::Rgba([200, 20, 120, 255])
        } else {
            image::Rgba([40, 190, 80, 255])
        }
    })
    .save(&inset_side)
    .unwrap();
    for (name, scale, shadow, corners, spacing, background, aspect) in [
        ("plain125", 1.25_f32, 0., 0., 0., "none", "auto"),
        ("plain200", 2., 0., 0., 0., "none", "auto"),
        ("zero125", 1.25_f32, 50., 24., 0., "none", "auto"),
        ("zero200", 2., 50., 24., 0., "none", "auto"),
        ("inset125", 1.25_f32, 0., 0., 0., "none", "auto"),
        ("inset200", 2., 0., 0., 0., "none", "auto"),
        ("right", 1.25_f32, 50., 24., 10., "solid", "auto"),
        ("left", 2., 50., 24., 0., "solid", "auto"),
        ("top", 1.25, 0., 24., 0., "none", "auto"),
        ("bottom", 2., 50., 0., 10., "solid", "auto"),
        ("wide", 1.25, 50., 24., 10., "solid", "21:9"),
        ("tall", 1.25, 50., 24., 10., "solid", "4:3"),
        ("stacked", 1.25, 50., 24., 0., "alpha", "auto"),
    ] {
        let edges = match name {
            "plain125" | "plain200" | "zero125" | "zero200" | "inset125" | "inset200" | "right"
            | "wide" | "tall" => {
                vec![crate::editor::layers::Edge::Right]
            }
            "left" => vec![crate::editor::layers::Edge::Left],
            "top" => vec![crate::editor::layers::Edge::Top],
            "bottom" => vec![crate::editor::layers::Edge::Bottom],
            _ => vec![
                crate::editor::layers::Edge::Left,
                crate::editor::layers::Edge::Right,
                crate::editor::layers::Edge::Top,
                crate::editor::layers::Edge::Bottom,
                crate::editor::layers::Edge::Right,
            ],
        };
        #[cfg(windows)]
        let old_images = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            let snapshot = editor.snapshot.borrow();
            snapshot
                .preview_edges
                .iter()
                .map(|edge| edge.image.clone())
                .chain(snapshot.backdrop.iter().cloned())
                .collect::<Vec<_>>()
        });
        #[cfg(windows)]
        if !old_images.is_empty() {
            cx.update_window(window, |_, window, _| {
                assert!(
                    old_images
                        .iter()
                        .any(|image| window.has_image_atlas_entry(image)),
                    "the previous layered images must have been painted"
                );
            })
            .unwrap();
        }
        editor
            .update(cx, |editor, _, cx| {
                editor.history.push(Vec::new());
                editor.clear_layers(cx);
                if background == "alpha" {
                    editor.wallpaper.set_background_image(Some(
                        transparent_wallpaper.to_string_lossy().into_owned(),
                    ));
                } else {
                    editor
                        .wallpaper
                        .set_gradient((background == "solid").then(|| {
                            crate::editor::wallpaper::GradientOption {
                                id: "visual-solid".into(),
                                colors: vec!["#2468ac".into()],
                                angle: 0.,
                            }
                        }));
                }
                editor.wallpaper.window_frame.style = "none".into();
                editor.wallpaper.padding = if name.starts_with("zero")
                    || name.starts_with("plain")
                    || name.starts_with("inset")
                {
                    0.
                } else {
                    20.
                };
                editor.wallpaper.inset = if name.starts_with("inset") { 6. } else { 0. };
                editor.wallpaper.spacing = spacing;
                editor.wallpaper.corners = corners;
                editor.wallpaper.shadow = shadow;
                editor.wallpaper.aspect_ratio = aspect.into();
                editor.wallpaper.background_blur = if background == "alpha" { 30. } else { 0. };
                if name.starts_with("inset") {
                    editor.history.push(vec![
                        crate::editor::annotations::Annotation::Redact {
                            id: "inset-redact-blackout".into(),
                            x: 40.,
                            y: 40.,
                            width: 100.,
                            height: 60.,
                            style: "blackout".into(),
                            intensity: 2.,
                        },
                        crate::editor::annotations::Annotation::Redact {
                            id: "inset-redact-blur".into(),
                            x: 200.,
                            y: 40.,
                            width: 100.,
                            height: 60.,
                            style: "blur".into(),
                            intensity: 2.,
                        },
                        crate::editor::annotations::Annotation::Redact {
                            id: "inset-redact-pixelate".into(),
                            x: 360.,
                            y: 40.,
                            width: 100.,
                            height: 60.,
                            style: "pixelate".into(),
                            intensity: 2.,
                        },
                        crate::editor::annotations::Annotation::Redact {
                            id: "inset-edge-blackout".into(),
                            x: 630.,
                            y: 20.,
                            width: 100.,
                            height: 50.,
                            style: "blackout".into(),
                            intensity: 2.,
                        },
                        crate::editor::annotations::Annotation::Redact {
                            id: "inset-edge-blur".into(),
                            x: 630.,
                            y: 105.,
                            width: 100.,
                            height: 50.,
                            style: "blur".into(),
                            intensity: 2.,
                        },
                        crate::editor::annotations::Annotation::Redact {
                            id: "inset-edge-pixelate".into(),
                            x: 630.,
                            y: 275.,
                            width: 100.,
                            height: 50.,
                            style: "pixelate".into(),
                            intensity: 2.,
                        },
                        crate::editor::annotations::Annotation::Rectangle {
                            id: "inset-layer-annotation".into(),
                            x: 280.,
                            y: 160.,
                            width: 100.,
                            height: 60.,
                            stroke: "#ff00ff".into(),
                            stroke_width: 1.,
                            fill: Some("#ff00ff".into()),
                        },
                        crate::editor::annotations::Annotation::Rectangle {
                            id: "inset-thick-rectangle".into(),
                            x: 40.,
                            y: 160.,
                            width: 100.,
                            height: 60.,
                            stroke: "#00ff00".into(),
                            stroke_width: 12.,
                            fill: Some("#ff00ff".into()),
                        },
                        crate::editor::annotations::Annotation::Rectangle {
                            id: "inset-layer-edge-annotation".into(),
                            x: 630.,
                            y: 200.,
                            width: 100.,
                            height: 30.,
                            stroke: "#ff00ff".into(),
                            stroke_width: 1.,
                            fill: Some("#ff00ff".into()),
                        },
                        crate::editor::annotations::Annotation::Highlight {
                            id: "inset-rectangle-overlap".into(),
                            points: vec![295., 158., 315., 158.],
                            fill: "#00ffff".into(),
                            opacity: 1.,
                            stroke_width: 8.,
                        },
                        crate::editor::annotations::Annotation::Highlight {
                            id: "inset-thick-rectangle-overlap".into(),
                            points: vec![65., 152., 85., 152.],
                            fill: "#00ffff".into(),
                            opacity: 1.,
                            stroke_width: 8.,
                        },
                        crate::editor::annotations::Annotation::Highlight {
                            id: "inset-layer-highlight".into(),
                            points: vec![60., 280., 180., 280.],
                            fill: "#ffff00".into(),
                            opacity: 0.5,
                            stroke_width: 20.,
                        },
                        crate::editor::annotations::Annotation::Highlight {
                            id: "inset-layer-later-highlight".into(),
                            points: vec![90., 269., 110., 269.],
                            fill: "#00ffff".into(),
                            opacity: 0.5,
                            stroke_width: 8.,
                        },
                    ]);
                }
                for edge in edges {
                    let (path, width, height) = match edge {
                        crate::editor::layers::Edge::Left | crate::editor::layers::Edge::Right => (
                            if name.starts_with("inset") {
                                &inset_side
                            } else {
                                &side
                            },
                            120.,
                            360.,
                        ),
                        _ => (&vertical, 640., 90.),
                    };
                    editor.push_image_layer(
                        path.to_string_lossy().into_owned(),
                        width,
                        height,
                        edge,
                        cx,
                    );
                }
                cx.notify();
            })
            .unwrap();
        cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
            .unwrap();
        settle(cx, window);
        editor
            .update(cx, |editor, _, cx| {
                editor.zoom = 1. / scale;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        #[cfg(windows)]
        cx.update_window(window, |_, window, _| {
            for image in &old_images {
                assert!(
                    !window.has_image_atlas_entry(image),
                    "replaced layered images must leave the GPU atlas"
                );
            }
        })
        .unwrap();
        let (bounds, saved, offset_x, offset_y, matte, rectangles, layer_inset) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            let wallpaper = editor.wallpaper.for_scale(f64::from(scale));
            let layer_inset = wallpaper.layer_inset(true);
            let group = crate::editor::layers::compute_with_inset(
                640.,
                360.,
                &editor.snapshot.borrow().layers,
                wallpaper.spacing,
                layer_inset,
            );
            let (_, (x, y, _, _)) = crate::editor::wallpaper::layout(
                &wallpaper,
                group.width,
                group.height,
                f64::from(scale),
                true,
            );
            let matte = stage_matte;
            let rectangles = std::iter::once(group.primary)
                .chain(group.layers.iter().map(|(_, rect)| *rect))
                .map(|rect| {
                    (
                        x.round() + rect.x,
                        y.round() + rect.y,
                        rect.width,
                        rect.height,
                    )
                })
                .collect::<Vec<_>>();
            (
                editor.bounds.borrow().unwrap(),
                image::load_from_memory(&editor.export_png().unwrap())
                    .unwrap()
                    .to_rgba8(),
                (x.round() + group.primary.x + layer_inset).round() as u32,
                (y.round() + group.primary.y + layer_inset).round() as u32,
                matte,
                rectangles,
                layer_inset,
            )
        });
        if name.starts_with("inset") {
            let original = editor
                .update(cx, |editor, _, _| {
                    let annotations = editor.history.current().to_vec();
                    editor.history.replace_current(
                        annotations
                            .iter()
                            .filter(|annotation| {
                                !matches!(
                                    annotation.id(),
                                    "inset-edge-blackout"
                                        | "inset-edge-blur"
                                        | "inset-edge-pixelate"
                                )
                            })
                            .cloned()
                            .collect(),
                    );
                    let png = editor.export_png().unwrap();
                    editor.history.replace_current(annotations);
                    image::load_from_memory(&png).unwrap().to_rgba8()
                })
                .unwrap();
            original
                .save(scratch.join(format!("layered-{name}-unredacted.png")))
                .unwrap();
            for y in 20..70 {
                for x in 630..730 {
                    assert_eq!(
                        saved.get_pixel(offset_x + x, offset_y + y).0,
                        [0, 0, 0, 255],
                        "blackout missed {x},{y}"
                    );
                }
            }
            let changed_outside = saved
                .enumerate_pixels()
                .filter(|&(x, y, pixel)| {
                    let in_redaction = x >= offset_x + 630
                        && x < offset_x + 730
                        && [20, 105, 275]
                            .into_iter()
                            .any(|top| y >= offset_y + top && y < offset_y + top + 50);
                    !in_redaction && pixel != original.get_pixel(x, y)
                })
                .count();
            assert_eq!(
                changed_outside, 0,
                "redaction changed pixels outside its bounds"
            );
            for x in [635, 680] {
                assert_ne!(
                    original.get_pixel(offset_x + x, offset_y + 40).0,
                    [0, 0, 0, 255]
                );
            }
            for (y, style) in [(120, "blur"), (300, "pixelate")] {
                let before = original.get_pixel(offset_x + 635, offset_y + y).0;
                let redacted = saved.get_pixel(offset_x + 635, offset_y + y).0;
                assert!(
                    before
                        .into_iter()
                        .zip(redacted)
                        .any(|(before, redacted)| before.abs_diff(redacted) > 10),
                    "{style} did not change the original image"
                );
            }
            let left = (rectangles[1].0 + layer_inset).round() as u32;
            let top = (rectangles[1].1 + layer_inset).round() as u32;
            for (y, style) in [(130, "blur"), (300, "pixelate")] {
                let original = original.get_pixel(left + 30, top + y).0;
                let redacted = saved.get_pixel(left + 30, top + y).0;
                assert!(
                    original
                        .into_iter()
                        .zip(redacted)
                        .any(|(original, redacted)| original.abs_diff(redacted) > 10),
                    "{style} did not change the attached image"
                );
            }
        }
        match name {
            "inset125" => {
                assert_eq!(saved.dimensions(), (792, 376));
                assert_eq!(saved.get_pixel(303, 183).0, [255, 0, 255, 255]);
                assert_eq!(saved.get_pixel(670, 223).0, [255, 0, 255, 255]);
                assert_eq!(saved.get_pixel(665, 8).0, [40, 190, 80, 255]);
            }
            "inset200" => {
                assert_eq!(saved.dimensions(), (808, 384));
                assert_eq!(saved.get_pixel(307, 187).0, [255, 0, 255, 255]);
                assert_eq!(saved.get_pixel(682, 227).0, [255, 0, 255, 255]);
                assert_eq!(saved.get_pixel(677, 12).0, [40, 190, 80, 255]);
            }
            _ => {}
        }
        let screenshot = cx.capture_screenshot(window).unwrap();
        let left = (f32::from(bounds.left()) * scale).round() as u32 - offset_x;
        let top = (f32::from(bounds.top()) * scale).round() as u32 - offset_y;
        assert!(
            left + saved.width() <= screenshot.width()
                && top + saved.height() <= screenshot.height(),
            "{name} composition must fit in the viewport"
        );
        let preview =
            image::imageops::crop_imm(&screenshot, left, top, saved.width(), saved.height())
                .to_image();
        let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
        image::imageops::overlay(&mut expected, &saved, 0, 0);
        let mut mismatched = 0;
        let mut overlap_rounding = 0;
        let shadow_extent = crate::editor::export::image_shadow(shadow).map_or(0., |shadow| {
            (crate::render::blur::box_radius(crate::render::blur::sigma_for_shadow_blur(
                shadow.blur,
            )) * 3
                + 2) as f64
                + f64::from(shadow.offset_x.abs().max(shadow.offset_y.abs()).ceil())
                + 2.
        });
        for (index, (actual, expected)) in preview.pixels().zip(expected.pixels()).enumerate() {
            let difference = actual
                .0
                .iter()
                .zip(expected.0)
                .map(|(actual, expected)| actual.abs_diff(expected))
                .max()
                .unwrap();
            if difference <= if background == "alpha" { 3 } else { 2 } {
                continue;
            }
            let x = (index as u32 % saved.width()) as f64;
            let y = (index as u32 / saved.width()) as f64;
            let overlap = rectangles
                .iter()
                .filter(|&&(left, top, width, height)| {
                    x >= left - shadow_extent
                        && x < left + width + shadow_extent
                        && y >= top - shadow_extent
                        && y < top + height + shadow_extent
                })
                .count();
            if name == "stacked" && difference == 4 && overlap >= 2 {
                overlap_rounding += 1;
            } else {
                mismatched += 1;
            }
        }
        if mismatched > 0 {
            preview
                .save(scratch.join(format!("layered-wallpaper-{name}-preview.png")))
                .unwrap();
            expected
                .save(scratch.join(format!("layered-wallpaper-{name}-export.png")))
                .unwrap();
        }
        assert_eq!(
            mismatched, 0,
            "{name} attached wallpaper preview differs from export at {scale}x"
        );
        assert!(
            overlap_rounding <= 16,
            "{name} has {overlap_rounding} four-level overlap differences"
        );
        if name.starts_with("inset") {
            let path = scratch.join(format!("layered-{name}.png"));
            saved.save(&path).unwrap();
            check_inset_reopen(
                cx,
                editor,
                scale,
                &path,
                (offset_x, offset_y),
                scratch,
                name,
                &[0.73, 1.0],
            );
            if name == "inset125" {
                editor
                    .update(cx, |editor, _, cx| {
                        editor.tool = crate::ui::colors::Tool::Select;
                        editor.zoom = 0.73 / scale;
                        cx.notify();
                    })
                    .unwrap();
                settle(cx, window);
                let bounds = cx.update(|cx| editor.read(cx).unwrap().bounds.borrow().unwrap());
                let before = cx.capture_screenshot(window).unwrap();
                super::click(
                    cx,
                    window,
                    f32::from(bounds.left()) + 330. * 0.73 / scale,
                    f32::from(bounds.top()) + 190. * 0.73 / scale,
                );
                settle(cx, window);
                cx.update(|cx| {
                    assert_eq!(
                        editor.read(cx).unwrap().snapshot.borrow().selected,
                        ["inset-layer-annotation"]
                    );
                });
                let selected = cx.capture_screenshot(window).unwrap();
                selected
                    .save(scratch.join("selected-overlapped-rectangle.png"))
                    .unwrap();
                let x = (f32::from(bounds.left()) * scale + 305. * 0.73).round() as u32;
                let y = (f32::from(bounds.top()) * scale + 158. * 0.73).round() as u32;
                let difference = before
                    .get_pixel(x, y)
                    .0
                    .iter()
                    .zip(selected.get_pixel(x, y).0)
                    .map(|(before, selected)| before.abs_diff(selected))
                    .max()
                    .unwrap();
                assert!(difference > 10, "selected inset annotation halo is hidden");
                super::click(
                    cx,
                    window,
                    f32::from(bounds.left()) + 500. * 0.73 / scale,
                    f32::from(bounds.top()) + 300. * 0.73 / scale,
                );
                cx.update(|cx| {
                    assert!(editor
                        .read(cx)
                        .unwrap()
                        .snapshot
                        .borrow()
                        .selected
                        .is_empty());
                });
                let before = cx.capture_screenshot(window).unwrap();
                super::click(
                    cx,
                    window,
                    f32::from(bounds.left()) + 90. * 0.73 / scale,
                    f32::from(bounds.top()) + 190. * 0.73 / scale,
                );
                cx.update(|cx| {
                    assert_eq!(
                        editor.read(cx).unwrap().snapshot.borrow().selected,
                        ["inset-thick-rectangle"]
                    );
                    let selected_export =
                        image::load_from_memory(&editor.read(cx).unwrap().export_png().unwrap())
                            .unwrap()
                            .to_rgba8();
                    assert!(
                        selected_export == saved,
                        "selection must not change export pixels"
                    );
                });
                let selected = cx.capture_screenshot(window).unwrap();
                selected
                    .save(scratch.join("selected-thick-rectangle.png"))
                    .unwrap();
                for (x, y) in [(110., 160.), (90., 190.), (75., 149.), (75., 155.)] {
                    let x = (f32::from(bounds.left()) * scale + x * 0.73).round() as u32;
                    let y = (f32::from(bounds.top()) * scale + y * 0.73).round() as u32;
                    assert!(before
                        .get_pixel(x, y)
                        .0
                        .iter()
                        .zip(selected.get_pixel(x, y).0)
                        .all(|(before, selected)| before.abs_diff(selected) <= 2));
                }
                let left = (f32::from(bounds.left()) * scale + 60. * 0.73).round() as u32;
                let right = (f32::from(bounds.left()) * scale + 120. * 0.73).round() as u32;
                let top = (f32::from(bounds.top()) * scale + 160. * 0.73).round() as u32;
                let bottom = (f32::from(bounds.top()) * scale + 220. * 0.73).round() as u32;
                let content_difference = {
                    let before = &before;
                    let selected = &selected;
                    (top..=bottom)
                        .flat_map(|y| {
                            (left..=right).flat_map(move |x| {
                                before
                                    .get_pixel(x, y)
                                    .0
                                    .into_iter()
                                    .zip(selected.get_pixel(x, y).0)
                                    .map(|(before, selected)| before.abs_diff(selected))
                            })
                        })
                        .max()
                        .unwrap()
                };
                assert!(
                    content_difference <= 2,
                    "selecting the thick rectangle changed its stroke or fill"
                );
                let x = (f32::from(bounds.left()) * scale + 75. * 0.73).round() as u32;
                let y = (f32::from(bounds.top()) * scale + 152. * 0.73).round() as u32;
                let difference = before
                    .get_pixel(x, y)
                    .0
                    .iter()
                    .zip(selected.get_pixel(x, y).0)
                    .map(|(before, selected)| before.abs_diff(selected))
                    .max()
                    .unwrap();
                assert!(difference > 10, "selected thick rectangle halo is hidden");
                for (side, x, y, horizontal) in [
                    ("top", 100., 151., true),
                    ("bottom", 100., 226., true),
                    ("left", 31., 190., false),
                    ("right", 146., 190., false),
                ] {
                    let difference = (0..=3)
                        .flat_map(|step| {
                            let x = (f32::from(bounds.left()) * scale
                                + (x + if horizontal { 0. } else { step as f32 }) * 0.73)
                                .round() as u32;
                            let y = (f32::from(bounds.top()) * scale
                                + (y + if horizontal { step as f32 } else { 0. }) * 0.73)
                                .round() as u32;
                            before
                                .get_pixel(x, y)
                                .0
                                .into_iter()
                                .zip(selected.get_pixel(x, y).0)
                                .map(|(before, selected)| before.abs_diff(selected))
                        })
                        .max()
                        .unwrap();
                    assert!(
                        difference > 10,
                        "selected thick rectangle {side} halo is hidden"
                    );
                }
                super::click(
                    cx,
                    window,
                    f32::from(bounds.left()) + 500. * 0.73 / scale,
                    f32::from(bounds.top()) + 300. * 0.73 / scale,
                );
                let before = cx.capture_screenshot(window).unwrap();
                super::click(
                    cx,
                    window,
                    f32::from(bounds.left()) + 635. * 0.73 / scale,
                    f32::from(bounds.top()) + 215. * 0.73 / scale,
                );
                cx.update(|cx| {
                    assert_eq!(
                        editor.read(cx).unwrap().snapshot.borrow().selected,
                        ["inset-layer-edge-annotation"]
                    );
                });
                let selected = cx.capture_screenshot(window).unwrap();
                let x = (f32::from(bounds.left()) * scale + 660. * 0.73).round() as u32;
                selected
                    .save(scratch.join("selected-attached-rectangle.png"))
                    .unwrap();
                let top = (f32::from(bounds.top()) * scale + 196. * 0.73).round() as u32;
                let bottom = (f32::from(bounds.top()) * scale + 198. * 0.73).round() as u32;
                let difference = (top..=bottom)
                    .flat_map(|y| {
                        before
                            .get_pixel(x, y)
                            .0
                            .into_iter()
                            .zip(selected.get_pixel(x, y).0)
                            .map(|(before, selected)| before.abs_diff(selected))
                    })
                    .max()
                    .unwrap();
                assert!(difference > 10, "attached inset annotation halo is hidden");
                super::click(
                    cx,
                    window,
                    f32::from(bounds.left()) + 500. * 0.73 / scale,
                    f32::from(bounds.top()) + 300. * 0.73 / scale,
                );
                let before = cx.capture_screenshot(window).unwrap();
                let x = (f32::from(bounds.left()) * scale + 100. * 0.73).round() as u32;
                let y = (f32::from(bounds.top()) * scale + 282. * 0.73).round() as u32;
                let expected_highlight = saved.get_pixel(offset_x + 100, offset_y + 282).0;
                assert_eq!(expected_highlight, [64, 112, 96, 255]);
                assert!(before
                    .get_pixel(x, y)
                    .0
                    .iter()
                    .zip(expected_highlight)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 2));
                super::click(
                    cx,
                    window,
                    f32::from(bounds.left()) + 130. * 0.73 / scale,
                    f32::from(bounds.top()) + 280. * 0.73 / scale,
                );
                cx.update(|cx| {
                    assert_eq!(
                        editor.read(cx).unwrap().snapshot.borrow().selected,
                        ["inset-layer-highlight"]
                    );
                });
                let selected = cx.capture_screenshot(window).unwrap();
                selected
                    .save(scratch.join("selected-highlight-halo.png"))
                    .unwrap();
                assert!(
                    before
                        .get_pixel(x, y)
                        .0
                        .iter()
                        .zip(selected.get_pixel(x, y).0)
                        .all(|(before, selected)| before.abs_diff(selected) <= 2),
                    "selecting an inset highlight must preserve its blended color"
                );
                let halo_top = (f32::from(bounds.top()) * scale + 268. * 0.73).round() as u32;
                let halo_bottom = (f32::from(bounds.top()) * scale + 270. * 0.73).round() as u32;
                let halo_difference = (halo_top..=halo_bottom)
                    .flat_map(|y| {
                        before
                            .get_pixel(x, y)
                            .0
                            .into_iter()
                            .zip(selected.get_pixel(x, y).0)
                            .map(|(before, selected)| before.abs_diff(selected))
                    })
                    .max()
                    .unwrap();
                assert!(halo_difference > 10, "selected highlight halo is hidden");
                super::click(
                    cx,
                    window,
                    f32::from(bounds.left()) + 500. * 0.73 / scale,
                    f32::from(bounds.top()) + 300. * 0.73 / scale,
                );
                cx.update(|cx| {
                    assert!(editor
                        .read(cx)
                        .unwrap()
                        .snapshot
                        .borrow()
                        .selected
                        .is_empty());
                });
                for (id, left) in [
                    ("inset-redact-blackout", 40.),
                    ("inset-redact-blur", 200.),
                    ("inset-redact-pixelate", 360.),
                ] {
                    let before = cx.capture_screenshot(window).unwrap();
                    super::click(
                        cx,
                        window,
                        f32::from(bounds.left()) + (left + 50.) * 0.73 / scale,
                        f32::from(bounds.top()) + 70. * 0.73 / scale,
                    );
                    cx.update(|cx| {
                        assert_eq!(editor.read(cx).unwrap().snapshot.borrow().selected, [id]);
                    });
                    let selected = cx.capture_screenshot(window).unwrap();
                    selected
                        .save(scratch.join(format!("selected-{id}.png")))
                        .unwrap();
                    let center_x =
                        (f32::from(bounds.left()) * scale + (left + 50.) * 0.73).round() as u32;
                    let center_y = (f32::from(bounds.top()) * scale + 70. * 0.73).round() as u32;
                    assert!(before
                        .get_pixel(center_x, center_y)
                        .0
                        .iter()
                        .zip(selected.get_pixel(center_x, center_y).0)
                        .all(|(before, selected)| before.abs_diff(selected) <= 2));
                    for (side, x, y, horizontal) in [
                        ("top", left + 25., 38., true),
                        ("bottom", left + 25., 100., true),
                        ("left", left - 2., 55., false),
                        ("right", left + 100., 55., false),
                    ] {
                        let difference = (0..=2)
                            .flat_map(|step| {
                                let x = (f32::from(bounds.left()) * scale
                                    + (x + if horizontal { 0. } else { step as f32 }) * 0.73)
                                    .round() as u32;
                                let y = (f32::from(bounds.top()) * scale
                                    + (y + if horizontal { step as f32 } else { 0. }) * 0.73)
                                    .round() as u32;
                                before
                                    .get_pixel(x, y)
                                    .0
                                    .into_iter()
                                    .zip(selected.get_pixel(x, y).0)
                                    .map(|(before, selected)| before.abs_diff(selected))
                            })
                            .max()
                            .unwrap();
                        assert!(difference > 10, "{id} {side} selection halo is hidden");
                    }
                    super::click(
                        cx,
                        window,
                        f32::from(bounds.left()) + 500. * 0.73 / scale,
                        f32::from(bounds.top()) + 300. * 0.73 / scale,
                    );
                }
                check_circle_selection(cx, editor, scale, scratch, "inset");
                editor
                    .update(cx, |editor, _, cx| {
                        editor.wallpaper.inset = 0.;
                        editor.clear_layers(cx);
                        cx.notify();
                    })
                    .unwrap();
                settle(cx, window);
                editor
                    .update(cx, |editor, _, cx| {
                        editor.zoom = 0.73 / scale;
                        cx.notify();
                    })
                    .unwrap();
                settle(cx, window);
                let bounds = cx.update(|cx| {
                    let editor = editor.read(cx).unwrap();
                    assert_eq!(editor.zoom, 0.73 / scale);
                    let snapshot = editor.snapshot.borrow();
                    assert!(!snapshot.backdrop_composite && snapshot.layers.is_empty());
                    drop(snapshot);
                    editor.bounds.borrow().unwrap()
                });
                for (id, click_x, click_y, sample_x, sample_top) in [
                    ("inset-redact-blackout", 90., 70., 65., 38.),
                    ("inset-layer-highlight", 130., 280., 100., 268.),
                    ("inset-layer-annotation", 330., 190., 305., 156.),
                ] {
                    let before = cx.capture_screenshot(window).unwrap();
                    super::click(
                        cx,
                        window,
                        f32::from(bounds.left()) + click_x * 0.73 / scale,
                        f32::from(bounds.top()) + click_y * 0.73 / scale,
                    );
                    cx.update(|cx| {
                        assert_eq!(editor.read(cx).unwrap().snapshot.borrow().selected, [id]);
                    });
                    let selected = cx.capture_screenshot(window).unwrap();
                    let x = (f32::from(bounds.left()) * scale + sample_x * 0.73).round() as u32;
                    let top = (f32::from(bounds.top()) * scale + sample_top * 0.73).round() as u32;
                    let bottom =
                        (f32::from(bounds.top()) * scale + (sample_top + 2.) * 0.73).round() as u32;
                    let difference = (top..=bottom)
                        .flat_map(|y| {
                            before
                                .get_pixel(x, y)
                                .0
                                .into_iter()
                                .zip(selected.get_pixel(x, y).0)
                                .map(|(before, selected)| before.abs_diff(selected))
                        })
                        .max()
                        .unwrap();
                    assert!(difference > 10, "plain {id} selection halo is hidden");
                    super::click(
                        cx,
                        window,
                        f32::from(bounds.left()) + 500. * 0.73 / scale,
                        f32::from(bounds.top()) + 300. * 0.73 / scale,
                    );
                }
                check_circle_selection(cx, editor, scale, scratch, "plain");
            }
        }
        if name.starts_with("plain") {
            editor
                .update(cx, |editor, _, cx| {
                    editor.history.push(vec![
                        crate::editor::annotations::Annotation::Rectangle {
                            id: "plain-edge".into(),
                            x: 620.,
                            y: 140.,
                            width: 30.,
                            height: 30.,
                            stroke: "#ff00ff".into(),
                            stroke_width: 1.,
                            fill: Some("#ff00ff".into()),
                        },
                        crate::editor::annotations::Annotation::Redact {
                            id: "plain-black".into(),
                            x: 625.,
                            y: 200.,
                            width: 30.,
                            height: 30.,
                            style: "black".into(),
                            intensity: 5.,
                        },
                        crate::editor::annotations::Annotation::Redact {
                            id: "plain-pixelate".into(),
                            x: 623.,
                            y: 240.,
                            width: 30.,
                            height: 30.,
                            style: "pixelate".into(),
                            intensity: 10.,
                        },
                        crate::editor::annotations::Annotation::Redact {
                            id: "plain-blur".into(),
                            x: 625.,
                            y: 280.,
                            width: 30.,
                            height: 30.,
                            style: "blur".into(),
                            intensity: 2.,
                        },
                    ]);
                    editor.tool = crate::ui::colors::Tool::Select;
                    cx.notify();
                })
                .unwrap();
            settle(cx, window);
            let annotated = cx.update(|cx| {
                image::load_from_memory(&editor.read(cx).unwrap().export_png().unwrap())
                    .unwrap()
                    .to_rgba8()
            });
            assert_eq!(
                annotated.get_pixel(offset_x + 645, offset_y + 150).0,
                [255, 0, 255, 255]
            );
            assert_eq!(
                annotated.get_pixel(offset_x + 645, offset_y + 215).0,
                [0, 0, 0, 255]
            );
            let mut expected =
                image::RgbaImage::from_pixel(annotated.width(), annotated.height(), matte);
            image::imageops::overlay(&mut expected, &annotated, 0, 0);
            let screenshot = cx.capture_screenshot(window).unwrap();
            for (start_x, start_y, width, height) in [
                (620, 140, 30, 30),
                (625, 200, 30, 30),
                (623, 240, 30, 30),
                (625, 280, 30, 30),
            ] {
                for y in start_y..start_y + height {
                    for x in start_x..start_x + width {
                        let actual = screenshot
                            .get_pixel(left + offset_x + x, top + offset_y + y)
                            .0;
                        let expected_pixel = expected.get_pixel(offset_x + x, offset_y + y).0;
                        assert!(
                            actual.iter()
                                .zip(expected_pixel)
                                .all(|(actual, expected)| actual.abs_diff(expected) <= 4),
                            "{name} annotation differs at {x},{y}: {actual:?} vs {expected_pixel:?}"
                        );
                    }
                }
            }
            for start_y in [200, 240, 280] {
                assert!(
                    (start_y..start_y + 30).any(|y| (625..655).any(|x| {
                        annotated.get_pixel(offset_x + x, offset_y + y)
                            != saved.get_pixel(offset_x + x, offset_y + y)
                    })),
                    "{name} redaction at y={start_y} must change the saved image"
                );
            }
        }
    }
    let unannotated = cx.update(|cx| {
        image::load_from_memory(&editor.read(cx).unwrap().export_png().unwrap())
            .unwrap()
            .to_rgba8()
    });
    editor
        .update(cx, |editor, _, cx| {
            editor.history.push(vec![
                crate::editor::annotations::Annotation::Rectangle {
                    id: "layered-corner".into(),
                    x: 0.,
                    y: 0.,
                    width: 40.,
                    height: 30.,
                    stroke: "#ff00ff".into(),
                    stroke_width: 1.,
                    fill: Some("#ff00ff".into()),
                },
                crate::editor::annotations::Annotation::Rectangle {
                    id: "layered-edge".into(),
                    x: 620.,
                    y: 140.,
                    width: 30.,
                    height: 30.,
                    stroke: "#ff00ff".into(),
                    stroke_width: 1.,
                    fill: Some("#ff00ff".into()),
                },
                crate::editor::annotations::Annotation::Redact {
                    id: "layered-black".into(),
                    x: 625.,
                    y: 200.,
                    width: 30.,
                    height: 30.,
                    style: "black".into(),
                    intensity: 5.,
                },
                crate::editor::annotations::Annotation::Redact {
                    id: "layered-pixelate".into(),
                    x: 625.,
                    y: 240.,
                    width: 30.,
                    height: 30.,
                    style: "pixelate".into(),
                    intensity: 10.,
                },
                crate::editor::annotations::Annotation::Redact {
                    id: "layered-blur".into(),
                    x: 625.,
                    y: 280.,
                    width: 30.,
                    height: 30.,
                    style: "blur".into(),
                    intensity: 2.,
                },
            ]);
            editor.tool = crate::ui::colors::Tool::Select;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let (bounds, png, layers, annotations, offset_x, offset_y, matte) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let wallpaper = editor.wallpaper.for_scale(1.25);
        let layers = editor.snapshot.borrow().layers.clone();
        let group = crate::editor::layers::compute(640., 360., &layers, wallpaper.spacing);
        let (_, (x, y, _, _)) =
            crate::editor::wallpaper::layout(&wallpaper, group.width, group.height, 1.25, true);
        let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
        (
            editor.bounds.borrow().unwrap(),
            editor.export_png().unwrap(),
            layers,
            editor.history.current().to_vec(),
            (x.round() + group.primary.x).round() as u32,
            (y.round() + group.primary.y).round() as u32,
            image::Rgba([
                (theme.r * 255.).round() as u8,
                (theme.g * 255.).round() as u8,
                (theme.b * 255.).round() as u8,
                255,
            ]),
        )
    });
    let saved = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(
        saved.get_pixel(offset_x + 630, offset_y + 150).0,
        [255, 0, 255, 255]
    );
    assert_eq!(
        saved.get_pixel(offset_x + 645, offset_y + 150).0,
        [255, 0, 255, 255]
    );
    assert_eq!(
        saved.get_pixel(offset_x + 645, offset_y + 215).0,
        [0, 0, 0, 255]
    );
    for start_y in [200, 240, 280] {
        assert!(
            (start_y..start_y + 30).any(|y| (625..655).any(|x| {
                saved.get_pixel(offset_x + x, offset_y + y)
                    != unannotated.get_pixel(offset_x + x, offset_y + y)
            })),
            "redaction at y={start_y} must change the saved image"
        );
    }
    let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
    image::imageops::overlay(&mut expected, &saved, 0, 0);
    let screenshot = cx.capture_screenshot(window).unwrap();
    let left = (f32::from(bounds.left()) * 1.25).round() as u32;
    let top = (f32::from(bounds.top()) * 1.25).round() as u32;
    for (start_x, start_y, width, height) in [
        (0, 0, 40, 30),
        (620, 140, 35, 30),
        (625, 200, 30, 30),
        (625, 240, 30, 30),
        (625, 280, 30, 30),
    ] {
        for y in start_y..start_y + height {
            for x in start_x..start_x + width {
                assert!(
                    screenshot
                        .get_pixel(left + x, top + y)
                        .0
                        .iter()
                        .zip(expected.get_pixel(offset_x + x, offset_y + y).0)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 4),
                    "layered annotation differs at {x},{y}"
                );
            }
        }
    }
    cx.update_window(window, |_, window, _| window.set_scale_factor(2.))
        .unwrap();
    editor
        .update(cx, |editor, _, cx| {
            editor.zoom = 0.5;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    let (bounds_2x, saved_2x, offset_x_2x, offset_y_2x, matte_2x) = cx.update(|cx| {
        let editor = editor.read(cx).unwrap();
        let wallpaper = editor.wallpaper.for_scale(2.);
        let group = crate::editor::layers::compute(
            640.,
            360.,
            &editor.snapshot.borrow().layers,
            wallpaper.spacing,
        );
        let (_, (x, y, _, _)) =
            crate::editor::wallpaper::layout(&wallpaper, group.width, group.height, 2., true);
        let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
        (
            editor.bounds.borrow().unwrap(),
            image::load_from_memory(&editor.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
            (x.round() + group.primary.x).round() as u32,
            (y.round() + group.primary.y).round() as u32,
            image::Rgba([
                (theme.r * 255.).round() as u8,
                (theme.g * 255.).round() as u8,
                (theme.b * 255.).round() as u8,
                255,
            ]),
        )
    });
    assert_eq!(
        saved_2x.get_pixel(offset_x_2x + 645, offset_y_2x + 150).0,
        [255, 0, 255, 255]
    );
    assert_eq!(
        saved_2x.get_pixel(offset_x_2x + 645, offset_y_2x + 215).0,
        [0, 0, 0, 255]
    );
    let mut expected_2x =
        image::RgbaImage::from_pixel(saved_2x.width(), saved_2x.height(), matte_2x);
    image::imageops::overlay(&mut expected_2x, &saved_2x, 0, 0);
    let screenshot_2x = cx.capture_screenshot(window).unwrap();
    let left_2x = (f32::from(bounds_2x.left()) * 2.).round() as u32;
    let top_2x = (f32::from(bounds_2x.top()) * 2.).round() as u32;
    for (start_x, start_y, width, height) in [
        (0, 0, 40, 30),
        (620, 140, 35, 30),
        (625, 200, 30, 30),
        (625, 240, 30, 30),
        (625, 280, 30, 30),
    ] {
        for y in start_y..start_y + height {
            for x in start_x..start_x + width {
                assert!(
                    screenshot_2x
                        .get_pixel(left_2x + x, top_2x + y)
                        .0
                        .iter()
                        .zip(expected_2x.get_pixel(offset_x_2x + x, offset_y_2x + y).0)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 4),
                    "layered 2x annotation differs at {x},{y}"
                );
            }
        }
    }
    cx.update_window(window, |_, window, _| window.set_scale_factor(1.25))
        .unwrap();
    editor
        .update(cx, |editor, _, cx| {
            editor.zoom = 0.8;
            cx.notify();
        })
        .unwrap();
    settle(cx, window);
    super::click(
        cx,
        window,
        f32::from(bounds.left()) + 8.,
        f32::from(bounds.top()) + 12.,
    );
    settle(cx, window);
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).unwrap().snapshot.borrow().selected,
            ["layered-corner"]
        );
    });
    let selected = cx.capture_screenshot(window).unwrap();
    let foreground =
        cx.update(|cx| gpui::Rgba::from(crate::theme::vars::active_theme(cx).primary_foreground));
    for (dx, dy) in [(-2, -2), (2, -2), (-2, 2), (2, 2)] {
        assert!(
            selected
                .get_pixel((left as i32 + dx) as u32, (top as i32 + dy) as u32)
                .0[..3]
                .iter()
                .zip([foreground.r, foreground.g, foreground.b])
                .all(|(actual, expected)| (f32::from(*actual) - expected * 255.).abs() <= 2.),
            "layered selection handle is clipped at {dx},{dy}"
        );
    }
    cx.update_window(window, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    let reopened = cx
        .open_window(size(px(1280.), px(738.)), |window, cx| {
            window.set_scale_factor(1.25);
            cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
        })
        .unwrap();
    settle(cx, reopened.into());
    cx.update(|cx| {
        let reopened = reopened.read(cx).unwrap();
        assert_eq!(reopened.snapshot.borrow().layers, layers);
        assert_eq!(reopened.history.current(), annotations);
        assert_eq!(reopened.export_png().unwrap(), png);
    });
    reopened
        .update(cx, |editor, _, cx| {
            editor.tool = crate::ui::colors::Tool::Crop;
            editor.zoom = 0.8;
            cx.notify();
        })
        .unwrap();
    settle(cx, reopened.into());
    let (crop_bounds, old_backdrop) = cx.update(|cx| {
        let editor = reopened.read(cx).unwrap();
        (
            editor.bounds.borrow().unwrap(),
            editor.snapshot.borrow().backdrop.clone().unwrap(),
        )
    });
    let start = crop_bounds.origin + point(px(8.), px(8.));
    let end = crop_bounds.origin + point(px(208.), px(128.));
    event(
        cx,
        reopened.into(),
        PlatformInput::MouseDown(MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
            first_mouse: false,
        }),
    );
    event(
        cx,
        reopened.into(),
        PlatformInput::MouseUp(MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    settle(cx, reopened.into());
    let (expected_width, expected_height) = cx.update(|cx| {
        let editor = reopened.read(cx).unwrap();
        let (_, _, width, height) = editor.snapshot.borrow().crop.unwrap();
        (width.abs() as u32, height.abs() as u32)
    });
    assert!((249..=250).contains(&expected_width) && (149..=150).contains(&expected_height));
    reopened
        .update(cx, |editor, _, cx| editor.apply_crop(cx))
        .unwrap();
    settle(cx, reopened.into());
    reopened
        .update(cx, |editor, _, cx| {
            editor.zoom = 0.8;
            cx.notify();
        })
        .unwrap();
    settle(cx, reopened.into());
    let (cropped_bounds, cropped_png, cropped_offset_x, cropped_offset_y) = cx.update(|cx| {
        let editor = reopened.read(cx).unwrap();
        assert_eq!(
            (editor.image_width as u32, editor.image_height as u32),
            (expected_width, expected_height)
        );
        assert!(editor.snapshot.borrow().backdrop_composite);
        assert_ne!(
            editor.snapshot.borrow().backdrop.as_ref().unwrap().id,
            old_backdrop.id
        );
        let snapshot = editor.snapshot.borrow();
        let wallpaper = &snapshot.wallpaper;
        let group = crate::editor::layers::compute(
            f64::from(expected_width),
            f64::from(expected_height),
            &snapshot.layers,
            wallpaper.spacing,
        );
        let (_, (x, y, _, _)) = crate::editor::wallpaper::layout(
            wallpaper,
            group.width,
            group.height,
            snapshot.frame_scale,
            true,
        );
        (
            editor.bounds.borrow().unwrap(),
            image::load_from_memory(&editor.export_png().unwrap())
                .unwrap()
                .to_rgba8(),
            (x.round() + group.primary.x).round() as u32,
            (y.round() + group.primary.y).round() as u32,
        )
    });
    #[cfg(windows)]
    cx.update_window(reopened.into(), |_, window, _| {
        assert!(
            !window.has_image_atlas_entry(&old_backdrop),
            "the pre-crop composite must leave the GPU atlas"
        );
    })
    .unwrap();
    let mut cropped_expected =
        image::RgbaImage::from_pixel(cropped_png.width(), cropped_png.height(), matte);
    image::imageops::overlay(&mut cropped_expected, &cropped_png, 0, 0);
    let cropped_screenshot = cx.capture_screenshot(reopened.into()).unwrap();
    let cropped_left = (f32::from(cropped_bounds.left()) * 1.25).round() as u32;
    let cropped_top = (f32::from(cropped_bounds.top()) * 1.25).round() as u32;
    for y in 0..expected_height {
        for x in 0..expected_width {
            assert!(
                cropped_screenshot
                    .get_pixel(cropped_left + x, cropped_top + y)
                    .0
                    .iter()
                    .zip(
                        cropped_expected
                            .get_pixel(cropped_offset_x + x, cropped_offset_y + y)
                            .0
                    )
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 4),
                "cropped layered preview differs at {x},{y}"
            );
        }
    }
    cx.update_window(reopened.into(), |_, window, cx| {
        window.dispatch_action(Box::new(crate::editor::actions::Undo), cx);
    })
    .unwrap();
    settle(cx, reopened.into());
    cx.update(|cx| {
        let editor = reopened.read(cx).unwrap();
        assert_eq!((editor.image_width, editor.image_height), (640., 360.));
        assert_eq!(editor.export_png().unwrap(), png);
    });
    cx.update_window(reopened.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    println!(
        "wallpaper, rounded and framed previews match saved pixels at 1.25x and 2x, including transparent content and edge annotations"
    );
}

#[cfg(any(windows, target_os = "macos"))]
pub(super) fn video(
    cx: &mut HeadlessAppContext,
    output: &Path,
    scratch: &Path,
    width: f32,
    height: f32,
    scale: f32,
) {
    use crate::windows::registry::{self, WindowKind};
    use crate::windows::video_editor::{model, preview, sidebar::SidebarTab, VideoEditorWindow};

    let project = scratch.join("Visual regression.poratake");
    std::fs::create_dir_all(&project).unwrap();
    let recording = crate::video::project::recording_video_path(&project);
    std::fs::write(&recording, include_bytes!("fixtures/capture.mov")).unwrap();
    let mut state = model::VideoEditorState {
        source_duration: Some(2.),
        segments: vec![model::Segment::spanning(2.)],
        ..Default::default()
    };
    state.drawing_segments.push(model::DrawingSegment {
        id: "visual-drawing".into(),
        start_time: 0.,
        end_time: 2.,
        canvas_width: 320.,
        canvas_height: 180.,
        annotations: vec![crate::editor::annotations::Annotation::Rectangle {
            id: "visual-shape".into(),
            x: 24.,
            y: 24.,
            width: 72.,
            height: 48.,
            stroke: "#ffffff".into(),
            stroke_width: 3.,
            fill: Some("#e04040".into()),
        }],
    });
    model::save_state(&project, &state);
    let video = cx.update(|cx| {
        VideoEditorWindow::open(cx, Some(project.to_string_lossy().into_owned()));
        registry::handle(WindowKind::VideoEditor, cx)
            .unwrap()
            .downcast::<VideoEditorWindow>()
            .unwrap()
    });
    let window = video.into();
    cx.update_window(window, |_, window, _| {
        let width = if cfg!(target_os = "macos") {
            width
        } else {
            width.max(1200.)
        };
        window.resize(size(px(width), px(height)));
        window.set_scale_factor(scale);
    })
    .unwrap();
    settle(cx, window);
    cx.update(|cx| {
        video
            .update(cx, |editor, _, cx| {
                editor.update_wallpaper(cx, |wallpaper| {
                    wallpaper.enabled = true;
                    wallpaper.padding = 12.;
                    wallpaper.corners = 8.;
                });
                editor.update_export(cx, |export| export.resolution = "original".into());
            })
            .unwrap();
    });
    cx.run_until_parked();
    cx.advance_clock(std::time::Duration::from_secs(1));
    settle(cx, window);
    let saved = model::load_state(&project);
    assert!(saved.wallpaper.enabled);
    assert_eq!(saved.wallpaper.padding, 12.);
    let mut expected = preview::Source::open(&project, saved).unwrap();
    let expected = preview::to_render_image_opaque(&expected.compose(0., None).unwrap()).unwrap();
    cx.update(|cx| {
        let actual = video.read(cx).unwrap().preview_image().unwrap();
        assert_eq!(actual.size(0), expected.size(0));
        let actual = actual.as_bytes(0).unwrap();
        let expected = expected.as_bytes(0).unwrap();
        assert_eq!(actual.len(), expected.len());
        if let Some(index) = actual.iter().zip(expected).position(|(a, b)| a != b) {
            panic!(
                "video preview differs at byte {index}: {} != {}",
                actual[index], expected[index]
            );
        }
    });
    hover(cx, window, 300., 300.);
    capture(cx, window, output, "video-loaded-default");
    #[cfg(target_os = "macos")]
    {
        let screenshot = image::open(output.join("gpui-video-loaded-default.png"))
            .unwrap()
            .to_rgba8();
        let stage_right = ((width
            - crate::ui::chrome::VIDEO_SIDEBAR_WIDTH
            - crate::ui::chrome::VIDEO_TAB_RAIL_WIDTH)
            * scale) as u32;
        let preview_pixels = screenshot
            .enumerate_pixels()
            .filter(|(x, y, _)| *x < stage_right && *y < screenshot.height() * 2 / 3);
        let (red, blue) = preview_pixels.fold((0, 0), |(red, blue), (_, _, pixel)| {
            let [r, g, b, _] = pixel.0;
            (
                red + usize::from(r > 170 && g < 120 && b < 120),
                blue + usize::from(b > 150 && u16::from(b) > u16::from(r) + 40),
            )
        });
        assert!(
            red > 100 && blue > 100,
            "rendered preview lacks fixture colors: red={red}, blue={blue}"
        );
        for (group, label_index) in [
            ("video-timeline-video-label", 0),
            ("video-timeline-audio-label", 2),
        ] {
            let label = super::measured(group, label_index);
            let pixel = |position: gpui::Pixels| (f32::from(position) * scale).round() as u32;
            let padding_x = pixel(super::measured(group, 0).left() - px(3.));
            let mut visible = false;
            for y in pixel(label.top())..pixel(label.bottom()) {
                let background = screenshot.get_pixel(padding_x, y);
                for x in pixel(label.left())..pixel(label.right()) {
                    let foreground = screenshot.get_pixel(x, y);
                    if foreground.0[..3].iter().all(|channel| *channel >= 245) {
                        visible = true;
                        let contrast = super::pixel_contrast(foreground, background);
                        assert!(contrast >= 4.5, "{group} contrast was {contrast:.2}");
                    }
                }
            }
            assert!(visible, "{group} did not paint its duration");
        }
    }
    if cfg!(target_os = "macos") && width == 720. {
        let stage_right = px(width
            - crate::ui::chrome::VIDEO_SIDEBAR_WIDTH
            - crate::ui::chrome::VIDEO_TAB_RAIL_WIDTH);
        let before = super::measured("video-timeline-controls", 9);
        assert!(before.right() > stage_right);
        let play = super::measured("video-timeline-controls", 0);
        event(
            cx,
            window,
            PlatformInput::Scroll(gpui::ScrollEvent {
                position: point(px(150.), play.center().y),
                delta: gpui::ScrollDelta::Pixels(point(px(-1000.), px(0.))),
                ..Default::default()
            }),
        );
        settle(cx, window);
        let scrub = super::measured("video-timeline-controls", 9);
        assert!(
            scrub.left() >= px(0.) && scrub.right() <= stage_right,
            "scrub {before:?} -> {scrub:?}, stage right {stage_right:?}"
        );
        capture(cx, window, output, "video-loaded-controls-scrolled");
        click(
            cx,
            window,
            f32::from(scrub.right() - px(14.)),
            f32::from(scrub.center().y),
        );
        cx.advance_clock(std::time::Duration::from_secs(1));
        settle(cx, window);
        assert!(model::load_state(&project).ui.scrub_audio_enabled);
        click(
            cx,
            window,
            f32::from(scrub.right() - px(14.)),
            f32::from(scrub.center().y),
        );
        cx.advance_clock(std::time::Duration::from_secs(1));
        settle(cx, window);
        assert!(!model::load_state(&project).ui.scrub_audio_enabled);
        let scrub_focus = cx
            .update_window(window, |_, window, cx| window.focused(cx))
            .unwrap()
            .expect("Scrub Audio should receive focus");
        event(
            cx,
            window,
            PlatformInput::Scroll(gpui::ScrollEvent {
                position: point(px(150.), play.center().y),
                delta: gpui::ScrollDelta::Pixels(point(px(1000.), px(0.))),
                ..Default::default()
            }),
        );
        let restored = super::measured("video-timeline-controls", 0);
        assert!(
            restored.left() >= px(0.) && restored.right() <= stage_right,
            "play control did not return into view: {restored:?}"
        );
        let play_focus = super::focus_measured(cx, window, "video-timeline-controls", 0);
        assert_ne!(
            play_focus, scrub_focus,
            "Play and Scrub Audio must retain their own focus after pointer interaction"
        );
        assert!(cx
            .update_window(window, |_, window, _| window
                .context_stack()
                .last()
                .is_some_and(|context| context.contains("Button")))
            .unwrap());
        assert!(!cx.update(|cx| video.read(cx).unwrap().parity_cut_tool_active()));
        super::key(cx, window, "c");
        assert!(
            cx.update(|cx| video.read(cx).unwrap().parity_cut_tool_active()),
            "C must activate Cut while Play has focus"
        );
        super::key(cx, window, "c");
        assert!(!cx.update(|cx| video.read(cx).unwrap().parity_cut_tool_active()));
        super::key(cx, window, "cmd-s");
        cx.advance_clock(std::time::Duration::from_secs(1));
        settle(cx, window);
        assert_eq!(model::load_state(&project).ui.sidebar_tab, "export");
        cx.update(|cx| {
            video
                .update(cx, |editor, _, cx| {
                    editor.activate_tab(SidebarTab::Cursor, cx)
                })
                .unwrap();
        });
        settle(cx, window);
        let cut_focus = super::focus_measured(cx, window, "video-timeline-controls", 2);
        assert_ne!(cut_focus, play_focus);
        super::key(cx, window, "space");
        assert!(cx.update(|cx| video.read(cx).unwrap().parity_cut_tool_active()));
        super::key(cx, window, "space");
        assert!(!cx.update(|cx| video.read(cx).unwrap().parity_cut_tool_active()));
        assert_eq!(
            super::focus_measured(cx, window, "video-timeline-controls", 0),
            play_focus
        );
        assert!(super::measured("video-timeline-controls", 9).right() > stage_right);
        let mut reached_scrub = false;
        let mut reached_slider = false;
        for _ in 0..30 {
            super::key(cx, window, "tab");
            let slider_focused = cx
                .update_window(window, |_, window, _| {
                    window
                        .context_stack()
                        .last()
                        .is_some_and(|context| context.contains("Slider"))
                })
                .unwrap();
            if slider_focused && !reached_slider {
                reached_slider = true;
                super::key(cx, window, "c");
                assert!(
                    !cx.update(|cx| video.read(cx).unwrap().parity_cut_tool_active()),
                    "C must not activate Cut while the slider has focus"
                );
                let zoom_before = model::load_state(&project).timeline_zoom;
                super::key(cx, window, "right");
                cx.advance_clock(std::time::Duration::from_secs(1));
                settle(cx, window);
                assert_ne!(model::load_state(&project).timeline_zoom, zoom_before);
            }
            reached_scrub = cx
                .update_window(window, |_, window, cx| {
                    window.focused(cx).is_some_and(|focus| focus == scrub_focus)
                })
                .unwrap();
            if reached_scrub {
                break;
            }
        }
        assert!(reached_slider, "Tab must reach the timeline zoom slider");
        assert!(reached_scrub, "Tab must reach Scrub Audio");
        assert!(!cx
            .update_window(window, |_, window, _| window
                .context_stack()
                .last()
                .is_some_and(|context| context.contains("Button")))
            .unwrap());
        super::key(cx, window, "c");
        assert!(
            !cx.update(|cx| video.read(cx).unwrap().parity_cut_tool_active()),
            "C must not activate Cut while the switch has focus"
        );
        cx.advance_clock(std::time::Duration::from_secs(1));
        settle(cx, window);
        let tab_before = model::load_state(&project).ui.sidebar_tab;
        super::key(cx, window, "cmd-s");
        cx.advance_clock(std::time::Duration::from_secs(1));
        settle(cx, window);
        assert_eq!(model::load_state(&project).ui.sidebar_tab, tab_before);
        let scrub = super::measured("video-timeline-controls", 9);
        assert!(
            scrub.left() >= px(0.) && scrub.right() <= stage_right,
            "keyboard-focused Scrub Audio is clipped: {scrub:?}, stage right {stage_right:?}"
        );
        capture(cx, window, output, "video-loaded-keyboard-scrub");
        super::key(cx, window, "space");
        cx.advance_clock(std::time::Duration::from_secs(1));
        settle(cx, window);
        assert!(model::load_state(&project).ui.scrub_audio_enabled);
        super::key(cx, window, "space");
        cx.advance_clock(std::time::Duration::from_secs(1));
        settle(cx, window);
        assert!(!model::load_state(&project).ui.scrub_audio_enabled);
        assert!(super::measured("video-timeline-controls", 0).left() < px(0.));
        let mut reached_play = false;
        for _ in 0..30 {
            super::key(cx, window, "shift-tab");
            reached_play = cx
                .update_window(window, |_, window, cx| {
                    window.focused(cx).is_some_and(|focus| focus == play_focus)
                })
                .unwrap();
            if reached_play {
                break;
            }
        }
        assert!(reached_play, "Shift-Tab must reach Play");
        let play = super::measured("video-timeline-controls", 0);
        assert!(
            play.left() >= px(0.) && play.right() <= stage_right,
            "keyboard-focused Play is clipped: {play:?}, stage right {stage_right:?}"
        );
        click(cx, window, 150., 100.);
        assert!(cx
            .update_window(window, |_, window, cx| window.focused(cx)
                != Some(play_focus))
            .unwrap());
        assert!(!cx.update(|cx| video.read(cx).unwrap().parity_cut_tool_active()));
        super::key(cx, window, "c");
        assert!(
            cx.update(|cx| video.read(cx).unwrap().parity_cut_tool_active()),
            "C must activate the cut control after a background click"
        );
        super::key(cx, window, "c");
        assert!(!cx.update(|cx| video.read(cx).unwrap().parity_cut_tool_active()));
    }
    for tab in SidebarTab::ALL {
        cx.update(|cx| {
            video
                .update(cx, |editor, _, cx| editor.activate_tab(tab, cx))
                .unwrap();
        });
        cx.run_until_parked();
        cx.advance_clock(std::time::Duration::from_secs(1));
        settle(cx, window);
        let saved = model::load_state(&project);
        assert!(saved.ui.sidebar_open);
        assert_eq!(saved.ui.sidebar_tab, tab.id());
        capture(cx, window, output, &format!("video-loaded-{}", tab.id()));
    }
    let button = super::measured("video-export-start", 0);
    let viewport = cx
        .update_window(window, |_, window, _| window.viewport_size())
        .unwrap();
    assert!(button.size.width > px(0.) && button.size.height > px(0.));
    assert!(button.left() >= px(0.) && button.right() <= viewport.width);
    assert!(button.top() >= px(0.) && button.bottom() <= viewport.height);
    cx.update(|cx| {
        video
            .update(cx, |editor, _, cx| {
                editor.begin_export_for_parity(cx);
                editor.cancel_export(cx);
            })
            .unwrap();
    });
    settle(cx, window);
    capture(cx, window, output, "video-loaded-export-canceling");
    let footer = super::measured("video-export-panel", 1);
    let status = super::measured("video-export-canceling", 0);
    assert!(status.left() >= footer.left() && status.right() <= footer.right());
    assert!(status.top() >= footer.top() && status.bottom() <= footer.bottom());
    let viewport = cx
        .update_window(window, |_, window, _| window.viewport_size())
        .unwrap();
    assert!(footer.left() >= px(0.) && footer.right() <= viewport.width);
    assert!(footer.top() >= px(0.) && footer.bottom() <= viewport.height);
    cx.update(|cx| registry::close(WindowKind::VideoEditor, cx));
}
