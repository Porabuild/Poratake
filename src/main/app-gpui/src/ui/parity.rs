use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    point, px, size, AnyWindowHandle, AppContext, Bounds, HeadlessAppContext, KeyDownEvent,
    KeyUpEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, PlatformInput,
};
use herogpui::gpui;

mod analysis_workflows;
mod attachment_workflows;
mod baselines;
pub(crate) mod daemon_fixture;
mod overlays;
mod pin_workflows;
mod preview_workflows;
mod recording_controls;
mod scroll_workflows;
#[cfg(windows)]
mod windows_renderer;
mod workflows;

thread_local! {
    static GEOMETRY: RefCell<BTreeMap<&'static str, Vec<Bounds<Pixels>>>> = RefCell::default();
    static NOTIFICATIONS: RefCell<Vec<(String, String, bool)>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn record_notification(title: &str, body: &str, transient: bool) -> bool {
    if std::env::var_os(daemon_fixture::PROCESS_ENV).is_none() {
        return false;
    }
    NOTIFICATIONS.with_borrow_mut(|notifications| {
        notifications.push((title.into(), body.into(), transient));
    });
    true
}

fn take_notifications() -> Vec<(String, String, bool)> {
    NOTIFICATIONS.with_borrow_mut(std::mem::take)
}

pub(crate) fn record(name: &'static str, bounds: Vec<Bounds<Pixels>>) {
    GEOMETRY.with_borrow_mut(|geometry| {
        geometry.insert(name, bounds);
    });
}

fn measured(group: &str, index: usize) -> Bounds<Pixels> {
    GEOMETRY.with_borrow(|geometry| geometry[group][index])
}

fn click_measured(cx: &mut HeadlessAppContext, window: AnyWindowHandle, group: &str, index: usize) {
    if group.starts_with("editor-") && !crate::ui::chrome::is_macos() {
        let viewport = measured("editor-chrome", 0);
        let bounds = measured(group, index);
        let delta = if bounds.left() < viewport.left() {
            viewport.left() - bounds.left()
        } else if bounds.right() > viewport.right() {
            viewport.right() - bounds.right()
        } else {
            px(0.)
        };
        if delta != px(0.) {
            event(
                cx,
                window,
                PlatformInput::Scroll(gpui::ScrollEvent {
                    position: viewport.center(),
                    delta: gpui::ScrollDelta::Pixels(point(delta, px(0.))),
                    ..Default::default()
                }),
            );
            settle(cx, window);
        }
        let bounds = measured(group, index);
        assert!(
            bounds.left() + px(0.001) >= viewport.left()
                && bounds.right() <= viewport.right() + px(0.001),
            "the toolbar control must scroll fully into view: {bounds:?}, {viewport:?}"
        );
    }
    let center = measured(group, index).center();
    click(cx, window, f32::from(center.x), f32::from(center.y));
}

fn assert_toolbar(width: f32, scale: f32) {
    GEOMETRY.with_borrow(|geometry| {
        if !crate::ui::chrome::is_macos() {
            let captions = geometry["editor-chrome"][2];
            assert!(
                captions.right() <= px(width),
                "window buttons clipped: {captions:?}"
            );
            let caption_width =
                (crate::ui::chrome::WINDOW_CONTROL_WIDTH * scale - 0.5).ceil() / scale;
            assert!((f32::from(captions.size.width) - 3. * caption_width).abs() < 0.001);
        }
        let tools = &geometry["editor-tools"];
        assert_eq!(tools.len(), 21);
        for (index, bounds) in tools.iter().enumerate() {
            assert!(
                bounds.top() >= px(0.) && bounds.bottom() <= px(40.),
                "tool {index}: {bounds:?}"
            );
            if index != 10 && index != 14 {
                let edge = if index >= 17 { 32. } else { 28. };
                assert_eq!(bounds.size, size(px(edge), px(edge)), "tool {index}");
            }
            if let Some(previous) = index.checked_sub(1).map(|index| tools[index]) {
                assert!(
                    bounds.left() + px(0.001) >= previous.right() + px(4.),
                    "tools overlap: {previous:?}, {bounds:?}"
                );
            }
        }
        for pair in geometry["editor-options"].windows(2) {
            assert!(
                pair[0].right() <= pair[1].left(),
                "tool options overlap: {pair:?}"
            );
        }
    });
}

fn frame(cx: &mut HeadlessAppContext, window: AnyWindowHandle) {
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        if window.viewport_size() != window.bounds().size {
            let scale = window.scale_factor();
            window.bounds_changed(cx);
            window.set_scale_factor(scale);
        }
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .unwrap();
}

fn assert_text_submenu_focus(
    cx: &mut HeadlessAppContext,
    window: AnyWindowHandle,
    index: usize,
    output: &Path,
) {
    use herogpui::theme::ActiveTheme;

    let (panel, scale, padding, row_height, focus, background) = cx
        .update_window(window, |_, window, cx| {
            let style = cx.components().select.resolve(&["tool-popover".into()]);
            let padding = f32::from(style.panel_padding.unwrap());
            let row_height = f32::from(style.row_height.unwrap());
            let scale = window.scale_factor();
            let height =
                crate::editor::options::FONT_SIZES.len() as f32 * row_height + 2. * padding;
            let panel = window
                .painted_quads()
                .into_iter()
                .rev()
                .find(|quad| {
                    (quad.bounds.size.height.0 / scale - height).abs() <= 3.
                        && (64. ..=200.).contains(&(quad.bounds.size.width.0 / scale))
                })
                .expect("the font-size submenu must be painted at its row height");
            let theme = crate::theme::vars::active_theme(cx);
            (
                panel,
                scale,
                padding,
                row_height,
                gpui::Rgba::from(theme.accent),
                gpui::Rgba::from(theme.popover),
            )
        })
        .unwrap();
    let image = cx.capture_screenshot(window).unwrap();
    image
        .save(output.join(format!("submenu-text-focus-{index}.png")))
        .unwrap();
    let left = panel.bounds.left().0 + padding * scale + panel.border_widths.left.0;
    let right = panel.bounds.right().0 - padding * scale - panel.border_widths.right.0;
    let top = panel.bounds.top().0
        + (padding + index as f32 * row_height) * scale
        + panel.border_widths.top.0;
    let bottom = top + row_height * scale;
    let distance = |pixel: &image::Rgba<u8>, color: gpui::Rgba| {
        pixel.0[..3]
            .iter()
            .zip([color.r, color.g, color.b])
            .map(|(actual, expected)| (f32::from(*actual) - expected * 255.).abs())
            .sum::<f32>()
    };
    for (side, x, y) in [
        ("left", left - 3. * scale, (top + bottom) / 2.),
        ("right", right + 3. * scale, (top + bottom) / 2.),
        ("top", (left + right) / 2., top - 3. * scale),
        ("bottom", (left + right) / 2., bottom + 3. * scale),
    ] {
        let mut visible = 0;
        for dx in -1..=1 {
            for dy in -1..=1 {
                let pixel = image.get_pixel(
                    (x.floor() as i32 + dx) as u32,
                    (y.floor() as i32 + dy) as u32,
                );
                if distance(pixel, focus) < distance(pixel, background) {
                    visible += 1;
                }
            }
        }
        assert!(visible >= 3, "font-size option {index} clips its {side} focus ring: {visible} focus pixels; panel={:?}, scale={scale}", panel.bounds);
    }
}

#[cfg(windows)]
fn capture_tray_menu(cx: &mut HeadlessAppContext, output: &Path, scale: f32) {
    let tray =
        cx.update(|cx| crate::windows::tray_menu::TrayMenuWindow::open(None, false, cx).unwrap());
    let window = tray.into();
    cx.update_window(window, |_, window, _| window.set_scale_factor(scale))
        .unwrap();
    settle(cx, window);
    cx.update_window(window, |_, window, _| {
        let panel = window
            .painted_quads()
            .into_iter()
            .filter(|quad| (quad.bounds.size.width.0 / scale - 304.).abs() < 0.01)
            .max_by(|left, right| {
                left.bounds
                    .size
                    .height
                    .partial_cmp(&right.bounds.size.height)
                    .unwrap()
            })
            .unwrap();
        let viewport = window.viewport_size();
        assert!(
            (f32::from(viewport.height) * scale - panel.bounds.size.height.0).abs() < 0.01,
            "the tray window must fit its painted menu: {viewport:?}, {:?}",
            panel.bounds
        );
    })
    .unwrap();
    capture(cx, window, output, "tray-default");
    cx.update_window(window, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

fn capture_history(
    cx: &mut HeadlessAppContext,
    output: &Path,
    scratch: &Path,
    scale: f32,
    history_width: f32,
) {
    use crate::history_store::{HistoryItem, HistoryItemType};
    use crate::windows::history::{model::HistoryLayout, HistoryWindow};

    let colors = [
        [64, 112, 192],
        [232, 88, 56],
        [48, 160, 104],
        [184, 96, 192],
    ];
    let items: Vec<_> = colors
        .iter()
        .enumerate()
        .map(|(index, color)| {
            let path = scratch.join(format!("history-visual-{index}.png"));
            image::RgbImage::from_pixel(640, 360, image::Rgb(*color))
                .save(&path)
                .unwrap();
            assert!(crate::thumbnails::ensure(&path, HistoryItemType::Screenshot).is_some());
            HistoryItem {
                id: format!("visual-{index}"),
                timestamp: 2_000_000_000_000,
                original_path: path.to_string_lossy().into_owned(),
                r#type: HistoryItemType::Screenshot,
                editor_state: None,
                duration: None,
            }
        })
        .collect();
    crate::history_store::save_history(&items);
    let store = cx.update(|cx| crate::state::state(cx).config);
    store.update(|settings| {
        settings.history.filter = "all".into();
        settings.history.sort_order = "newest".into();
        settings.history.layout = "grid".into();
    });
    let handle = cx
        .open_window(size(px(history_width), px(364.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| HistoryWindow::new(store, window, cx))
        })
        .unwrap()
        .into();
    let history = cx
        .update_window(handle, |view, _, _| {
            view.downcast::<HistoryWindow>().unwrap()
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        history.update(cx, |history, cx| history.focus_for_parity(window, cx));
    })
    .unwrap();
    hover(cx, handle, -100., -100.);
    settle(cx, handle);
    assert!(cx.update(|cx| history.read(cx).parity_state().0));
    let (first, second, fourth) = cx.update(|cx| {
        let history = history.read(cx);
        (
            history.parity_item_bounds(0),
            history.parity_item_bounds(1),
            history.parity_item_bounds(3),
        )
    });
    let first = first.expect("first history item must be rendered");
    let second = second.expect("second history item must be rendered");
    assert!(fourth.is_some(), "all four history items must be rendered");
    let single_column = first.top() != second.top();
    assert_eq!(single_column, history_width == 140.);
    if single_column {
        assert!(second.top() >= first.bottom());
    } else {
        assert!(first.right() < second.left());
    }
    assert!(second.right() <= px(history_width));
    let image = cx.capture_screenshot(handle).unwrap();
    let x = ((f32::from(first.left()) + f32::from(first.size.width) / 2.) * scale).round() as u32;
    let y =
        ((f32::from(first.top()) + f32::from(first.size.width) * 9. / 32.) * scale).round() as u32;
    let pixel = image.get_pixel(x, y);
    assert!(
        pixel.0[..3]
            .iter()
            .zip(colors[0])
            .all(|(actual, expected)| actual.abs_diff(expected) <= 8),
        "history thumbnail was not rendered: {pixel:?}"
    );
    capture(cx, handle, output, &format!("history-{history_width}-grid"));
    key(cx, handle, "down");
    if single_column {
        assert_eq!(cx.update(|cx| history.read(cx).parity_state().2), 1);
        key(cx, handle, "down");
    }
    let state = cx.update(|cx| history.read(cx).parity_state());
    assert!(state.1 && state.2 == 2 && state.3 == HistoryLayout::Grid);
    if single_column {
        let selected = measured("history-selected-grid-card", 0);
        let (focus, background) = cx.update(|cx| {
            let theme = crate::theme::vars::active_theme(cx);
            (
                gpui::Rgba::from(theme.primary),
                gpui::Rgba::from(theme.background),
            )
        });
        let image = cx.capture_screenshot(handle).unwrap();
        let distance = |pixel: &image::Rgba<u8>, color: gpui::Rgba| {
            pixel.0[..3]
                .iter()
                .zip([color.r, color.g, color.b])
                .map(|(actual, expected)| (f32::from(*actual) - expected * 255.).abs())
                .sum::<f32>()
        };
        for (side, x, y) in [
            ("left", selected.left() - px(2.), selected.center().y),
            ("right", selected.right() + px(2.), selected.center().y),
            ("top", selected.center().x, selected.top() - px(2.)),
            ("bottom", selected.center().x, selected.bottom() + px(2.)),
        ] {
            let (x, y) = (f32::from(x) * scale, f32::from(y) * scale);
            assert!(
                x >= 1.
                    && y >= 1.
                    && x + 1. < image.width() as f32
                    && y + 1. < image.height() as f32,
                "history selection ring leaves the window at {side}: {selected:?}"
            );
            let mut visible = 0;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let pixel = image.get_pixel(
                        (x.floor() as i32 + dx) as u32,
                        (y.floor() as i32 + dy) as u32,
                    );
                    if distance(pixel, focus) < distance(pixel, background) {
                        visible += 1;
                    }
                }
            }
            assert!(visible >= 3, "history selection clips its {side} ring: {visible} focus pixels, bounds {selected:?}, scale {scale}");
        }
    }
    capture(
        cx,
        handle,
        output,
        &format!("history-{history_width}-grid-selected"),
    );
    capture_history_tooltips(
        cx,
        handle,
        output,
        history_width,
        scale,
        "history-grid-actions",
        if single_column {
            "grid-third"
        } else {
            "grid-left"
        },
    );
    key(cx, handle, "right");
    assert_eq!(cx.update(|cx| history.read(cx).parity_state().2), 3);
    capture_history_tooltips(
        cx,
        handle,
        output,
        history_width,
        scale,
        "history-grid-actions",
        if single_column {
            "grid-fourth"
        } else {
            "grid-right"
        },
    );
    key(cx, handle, "left");
    assert_eq!(cx.update(|cx| history.read(cx).parity_state().2), 2);
    click_measured(cx, handle, "history-actions", 1);
    assert_eq!(
        cx.update(|cx| history.read(cx).parity_state().3),
        HistoryLayout::List
    );
    cx.update_window(handle, |_, window, cx| {
        history.update(cx, |history, cx| history.focus_for_parity(window, cx));
    })
    .unwrap();
    click(cx, handle, 5., 140.);
    capture(
        cx,
        handle,
        output,
        &format!("history-{history_width}-list-selected"),
    );
    capture_history_tooltips(
        cx,
        handle,
        output,
        history_width,
        scale,
        "history-row-actions",
        "list",
    );
    let windows_before = cx.update(|cx| cx.windows());
    click_measured(cx, handle, "history-row-actions", 1);
    let deleted = std::path::Path::new(&items[2].original_path);
    assert!(!deleted.exists());
    assert!(!crate::thumbnails::thumbnail_path(deleted).exists());
    assert_eq!(
        crate::history_store::load_history()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["visual-0", "visual-1", "visual-3"]
    );
    let state = cx.update(|cx| history.read(cx).parity_state());
    assert!(state.1 && state.2 == 2 && state.3 == HistoryLayout::List);
    assert_eq!(cx.update(|cx| cx.windows()), windows_before);
    println!(
        "history {history_width}px grid, list, tooltips and isolated deletion passed at {scale}x"
    );
    cx.update_window(handle, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
}

fn capture_history_tooltips(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    output: &Path,
    history_width: f32,
    scale: f32,
    group: &str,
    state: &str,
) {
    for (index, name) in [(0, "reveal"), (1, "delete")] {
        let action = measured(group, index).center();
        hover(cx, handle, f32::from(action.x), f32::from(action.y));
        cx.update_window(handle, |_, window, _| {
            let tips: Vec<_> = window
                .painted_quads()
                .into_iter()
                .filter(|quad| {
                    let bounds = quad.bounds;
                    (31. ..=35.).contains(&(bounds.size.height.0 / scale))
                        && bounds.size.width.0 / scale > 30.
                        && bounds.bottom().0 <= f32::from(action.y) * scale
                        && bounds.bottom().0 >= (f32::from(action.y) - 30.) * scale
                })
                .collect();
            assert_eq!(
                tips.len(),
                1,
                "one history tooltip must be painted: {tips:?}"
            );
            let tip = tips[0];
            let bounds = tip.bounds;
            let mask = tip.content_mask.bounds;
            assert!(
                bounds.left().0 >= 0.
                    && bounds.right().0 <= history_width * scale
                    && bounds.top().0 >= 0.
                    && bounds.bottom().0 <= 364. * scale
                    && mask.left().0 <= bounds.left().0
                    && mask.right().0 >= bounds.right().0
                    && mask.top().0 <= bounds.top().0
                    && mask.bottom().0 >= bounds.bottom().0,
                "history tooltip is clipped: {bounds:?}, mask {mask:?}"
            );
        })
        .unwrap();
        capture(
            cx,
            handle,
            output,
            &format!("history-{history_width}-{state}-tooltip-{name}"),
        );
    }
}

fn settle(cx: &mut HeadlessAppContext, window: AnyWindowHandle) {
    if !cx.update(|cx| cx.windows().contains(&window)) {
        return;
    }
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
    if cx.update(|cx| cx.windows().contains(&window)) {
        frame(cx, window);
    }
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

fn key(cx: &mut HeadlessAppContext, window: AnyWindowHandle, key: &'static str) {
    let keystroke = Keystroke::parse(key).unwrap();
    event(
        cx,
        window,
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        }),
    );
    if !cx.update(|cx| cx.windows().contains(&window)) {
        return;
    }
    event(cx, window, PlatformInput::KeyUp(KeyUpEvent { keystroke }));
    settle(cx, window);
}

fn escape(cx: &mut HeadlessAppContext, window: AnyWindowHandle) {
    key(cx, window, "escape");
}

fn capture(cx: &mut HeadlessAppContext, window: AnyWindowHandle, output: &Path, name: &str) {
    frame(cx, window);
    let (viewport, scale) = cx
        .update_window(window, |_, window, _| {
            (window.viewport_size(), window.scale_factor())
        })
        .unwrap();
    let image = cx.capture_screenshot(window).unwrap();
    let width = (f32::from(viewport.width) * scale).round() as u32;
    let logical_height = if name.starts_with("image-")
        && name != "image-wallpaper-selected"
        && !name.ends_with("-open")
    {
        80.
    } else {
        f32::from(viewport.height)
    };
    let height = (logical_height * scale).round() as u32;
    assert!(
        image.width() >= width && image.height() >= height,
        "renderer target {:?} is smaller than requested {width}x{height}",
        image.dimensions()
    );
    image::imageops::crop_imm(&image, 0, 0, width, height)
        .to_image()
        .save(output.join(format!("gpui-{name}.png")))
        .unwrap();
}

pub fn capture_editor_interaction_states() {
    let output = std::path::PathBuf::from(std::env::var("PORATAKE_PARITY_OUTPUT").unwrap());
    let source = std::env::var("PORATAKE_PARITY_IMAGE")
        .ok()
        .filter(|source| !source.is_empty());
    let theme = std::env::var("PORATAKE_PARITY_THEME").unwrap_or_else(|_| "dark".into());
    assert!(matches!(theme.as_str(), "light" | "dark"));
    let width: f32 = std::env::var("PORATAKE_PARITY_WIDTH")
        .unwrap_or_else(|_| "1280".into())
        .parse()
        .unwrap();
    let scale: f32 = std::env::var("PORATAKE_PARITY_SCALE")
        .unwrap_or_else(|_| "2".into())
        .parse()
        .unwrap();
    assert!((1.0..=2.0).contains(&scale));
    std::fs::create_dir_all(&output).unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scratch = output.join(format!("profile-{nonce}"));
    std::fs::create_dir_all(&scratch).unwrap();
    unsafe {
        std::env::set_var(crate::config::store::CONFIG_DIR_ENV, &scratch);
        std::env::set_var("PORATAKE_DAEMON_PATH", std::env::current_exe().unwrap());
        std::env::set_var(daemon_fixture::PROCESS_ENV, "1");
    }
    let image = scratch.join("capture.png");
    if let Some(source) = &source {
        std::fs::copy(source, &image).unwrap();
    } else {
        image::RgbaImage::from_fn(640, 360, |x, y| {
            if (x / 40 + y / 40) % 2 == 0 {
                image::Rgba([64, 112, 192, 255])
            } else {
                image::Rgba([224, 232, 240, 255])
            }
        })
        .save(&image)
        .unwrap();
    }
    let store =
        Arc::new(crate::config::store::ConfigStore::load_at(scratch.join("config.json")).unwrap());
    store.update(|config| {
        config.appearance.mode = theme;
        config.appearance.theme = "default".into();
        config.editor.color = "#f43f5e".into();
        config.editor.last_tool = "pen".into();
    });
    let platform = herogpui::platform::current_platform(!cfg!(windows));
    #[cfg(windows)]
    smol::block_on(platform.request_windowing(gpui::WindowingRequest::Windowed(
        gpui::GraphicalEnvironment::detect(),
    )))
    .unwrap();
    let mut cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(herogpui::HeroGpuiAssets),
        move || {
            #[cfg(windows)]
            return windows_renderer::create(&platform);
            #[cfg(not(windows))]
            herogpui::platform::current_headless_renderer()
        },
    );
    cx.update(|cx| {
        crate::state::set_test_state(cx, store);
        crate::editor::actions::init_bindings(cx);
        crate::capture::overlay::init_bindings(cx);
        crate::ui::font::register(cx);
    });
    #[cfg(windows)]
    windows_renderer::verify_image_sampling(&mut cx, &output, scale);
    #[cfg(windows)]
    if source.is_none() && width == 1280. {
        scroll_workflows::preview_corners(&mut cx, &output, scale);
    }
    if source.is_none() && width == 1280. && scale == 1. {
        workflows::editor_sizes(&mut cx, &scratch);
    }
    let editor = cx
        .open_window(size(px(width), px(738.0)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| {
                crate::editor::window::EditorWindow::from_file(image.to_str().unwrap(), window, cx)
            })
        })
        .unwrap();
    let window = editor.into();
    settle(&mut cx, window);
    assert_toolbar(width, scale);
    hover(&mut cx, window, 300.0, 300.0);
    capture(&mut cx, window, &output, "image-default");
    attachment_workflows::check_edge_overlay(&mut cx, editor, &output, scale);
    let redact = measured("editor-tools", 8).center();
    hover(&mut cx, window, f32::from(redact.x), f32::from(redact.y));
    capture(&mut cx, window, &output, "image-redact-hover");
    event(
        &mut cx,
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position: redact,
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
            position: redact,
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Modifiers::none(),
        }),
    );
    click_measured(&mut cx, window, "editor-tools", 0);
    cx.update(|cx| assert_eq!(editor.read(cx).unwrap().tool, crate::ui::colors::Tool::Pen));
    capture(&mut cx, window, &output, "image-pen-selected");
    key(&mut cx, window, "tab");
    assert!(cx
        .update_window(window, |_, window, cx| window.focused(cx).is_some()
            && herogpui::extend::focus_visible(cx))
        .unwrap());
    capture(&mut cx, window, &output, "image-keyboard-focused");
    let thickness = measured("editor-options", 0).center();
    hover(
        &mut cx,
        window,
        f32::from(thickness.x),
        f32::from(thickness.y),
    );
    capture(&mut cx, window, &output, "image-thickness-hover");
    click_measured(&mut cx, window, "editor-options", 0);
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
    let color_index = GEOMETRY.with_borrow(|geometry| geometry["editor-options"].len() - 1);
    click_measured(&mut cx, window, "editor-options", color_index);
    capture(&mut cx, window, &output, "image-color-open");
    escape(&mut cx, window);
    for (tool, index) in [
        (crate::ui::colors::Tool::Highlight, 1),
        (crate::ui::colors::Tool::Rectangle, 2),
        (crate::ui::colors::Tool::Arrow, 5),
        (crate::ui::colors::Tool::Text, 6),
        (crate::ui::colors::Tool::Number, 7),
        (crate::ui::colors::Tool::Redact, 8),
        (crate::ui::colors::Tool::Wallpaper, 12),
    ] {
        click_measured(&mut cx, window, "editor-tools", index);
        cx.update(|cx| assert_eq!(editor.read(cx).unwrap().tool, tool));
        assert_toolbar(width, scale);
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
            click_measured(&mut cx, window, "editor-options", 0);
            assert!(panel_open(&mut cx), "tool popover must open");
            capture(
                &mut cx,
                window,
                &output,
                &format!("image-{}-open", tool.id()),
            );
            key(&mut cx, window, "tab");
            key(&mut cx, window, "enter");
            assert!(
                panel_open(&mut cx),
                "keyboard activation must leave the tool panel open"
            );
            capture(
                &mut cx,
                window,
                &output,
                &format!("image-{}-keyboard-open", tool.id()),
            );
            if tool == crate::ui::colors::Tool::Text {
                let index = cx.update(|cx| {
                    let value = editor.read(cx).unwrap().text_font_size;
                    crate::editor::options::FONT_SIZES
                        .iter()
                        .position(|size| *size == value)
                        .unwrap()
                });
                assert_text_submenu_focus(&mut cx, window, index, &output);
                key(&mut cx, window, "home");
                assert_text_submenu_focus(&mut cx, window, 0, &output);
                key(&mut cx, window, "end");
                assert_text_submenu_focus(
                    &mut cx,
                    window,
                    crate::editor::options::FONT_SIZES.len() - 1,
                    &output,
                );
            }
            escape(&mut cx, window);
            escape(&mut cx, window);
        } else if matches!(
            tool,
            crate::ui::colors::Tool::Arrow
                | crate::ui::colors::Tool::Rectangle
                | crate::ui::colors::Tool::Highlight
        ) {
            click_measured(&mut cx, window, "editor-options", 0);
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
        .open_window(size(px(width), px(738.0)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| crate::windows::video_editor::VideoEditorWindow::new_for_test(None, cx))
        })
        .unwrap();
    let window = video.into();
    settle(&mut cx, window);
    hover(&mut cx, window, 300.0, 300.0);
    capture(&mut cx, window, &output, "video-default");
    hover(&mut cx, window, width - 22.0, 20.0);
    capture(&mut cx, window, &output, "video-header-hover");
    hover(&mut cx, window, width - 20.0, 208.0);
    capture(&mut cx, window, &output, "video-audio-hover");
    click(&mut cx, window, width - 20.0, 208.0);
    capture(&mut cx, window, &output, "video-audio-open");
    overlays::run(&mut cx, &output, width, scale);
    if source.is_none() {
        pin_workflows::run(&mut cx, &output, &scratch, scale);
        preview_workflows::run(&mut cx, &output, &scratch, scale);
    }
    recording_controls::run(&mut cx, &output, &scratch, scale);
    #[cfg(windows)]
    workflows::video(&mut cx, &output, &scratch, width, scale);
    if source.is_none() && width == 1280. && scale == 1. {
        workflows::selection_captures(&mut cx, &output, &scratch);
        workflows::recording_preference_handoffs(&mut cx, &output);
        workflows::timer_captures(&mut cx, &output, &scratch);
        analysis_workflows::run(&mut cx, &output, &scratch);
        scroll_workflows::run(&mut cx, &output, &scratch);
        attachment_workflows::run(&mut cx, &output, &scratch);
        pin_workflows::editor_roundtrip(&mut cx, &output, &scratch);
        workflows::screenshot(&mut cx, &output, &scratch);
        workflows::scaled_wallpaper_preview(&mut cx, &scratch);
        workflows::large_attachment_preview(&mut cx, &scratch);
        for scale in [1., 1.25, 2.] {
            for padding in [0., 20.] {
                for with_background in [false, true] {
                    workflows::balance_restoration(
                        &mut cx,
                        &scratch,
                        scale,
                        padding,
                        with_background,
                        0.,
                    );
                    workflows::simple_inset_preview(
                        &mut cx,
                        &scratch,
                        scale,
                        padding,
                        with_background,
                        8.,
                        false,
                    );
                }
            }
            for with_background in [false, true] {
                workflows::balanced_attachment_edges(&mut cx, &scratch, scale, with_background);
            }
            for transparent in [false, true] {
                for clipped in [false, true] {
                    workflows::restored_pixelation(&mut cx, &scratch, scale, transparent, clipped);
                }
            }
            workflows::simple_inset_preview(&mut cx, &scratch, scale, 0., false, 0., false);
            workflows::simple_inset_preview(&mut cx, &scratch, scale, 0., false, 8., true);
            workflows::balance_restoration(&mut cx, &scratch, scale, 0., false, 8.);
        }
        workflows::balance_restoration(&mut cx, &scratch, 1., 20., false, 8.);
        workflows::balance_restoration(&mut cx, &scratch, 1.25, 20., true, 8.);
    }
    #[cfg(windows)]
    if source.is_none() {
        workflows::scaled_selection_capture(&mut cx, &output, &scratch, scale);
    }
    #[cfg(windows)]
    capture_tray_menu(&mut cx, &output, scale);
    if source.is_none() && width == 950. {
        for history_width in [140., 200., 240.] {
            capture_history(&mut cx, &output, &scratch, scale, history_width);
        }
    }
    baselines::check(&output);
    cx.update(|cx| crate::state::state(cx).daemon.stop());
}
