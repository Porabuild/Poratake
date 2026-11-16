use gpui::AppContext;
use herogpui::gpui;

use crate::editor::annotations::Annotation;
use crate::editor::window::EditorWindow;
use crate::history_store::{HistoryItem, HistoryItemType};

use super::{measured, px, settle, size, HeadlessAppContext, Path};

pub(super) fn run(cx: &mut HeadlessAppContext, scratch: &Path) {
    for scale in [1., 1.25, 1.3333333, 2.] {
        for style in ["macos-light", "macos-dark"] {
            for shadow in [0., 50.] {
                for background in [false, true] {
                    for inset in [0., 8.] {
                        for transparent in [false, true] {
                            for translucent_background in [false, true] {
                                if translucent_background && !background {
                                    continue;
                                }
                                run_case(
                                    cx,
                                    scratch,
                                    scale,
                                    style,
                                    shadow,
                                    background,
                                    (inset, transparent, translucent_background),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

fn run_case(
    cx: &mut HeadlessAppContext,
    scratch: &Path,
    scale: f32,
    style: &str,
    shadow: f64,
    background: bool,
    variant: (f64, bool, bool),
) {
    let (inset, transparent, translucent_background) = variant;
    let name = format!(
        "balanced-frame-{scale}-{style}-{shadow}-{background}-{inset}{}{}",
        if transparent { "-alpha" } else { "" },
        if translucent_background {
            "-translucent-bg"
        } else {
            ""
        }
    );
    let source = scratch.join(format!("{name}.png"));
    image::RgbaImage::from_fn(200, 160, |x, y| {
        if transparent && (110..120).contains(&x) && (70..90).contains(&y) {
            image::Rgba([0, 0, 0, 0])
        } else if transparent && (90..110).contains(&x) && (70..90).contains(&y) {
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
    crate::history_store::add_item(
        HistoryItem {
            id: name.clone(),
            timestamp: 0,
            original_path: source.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: Some(serde_json::json!({"wallpaper": {
                "balance": true,
                "shadow": shadow,
                "inset": inset,
                "padding": if background { 20. } else { 0. },
                "gradient": background.then(|| serde_json::json!({
                    "id": "balanced-frame-background",
                    "colors": if translucent_background { vec!["#2468ac80", "#2468ac80"] } else { vec!["#2468ac"] },
                    "angle": 0
                })),
                "windowFrame": {"style": style}
            }})),
            duration: None,
        },
        100,
        true,
    );
    let mut previous = None;
    for reopened in [false, true] {
        let editor = cx
            .open_window(size(px(1280.), px(738.)), |window, cx| {
                window.set_scale_factor(scale);
                cx.new(|cx| EditorWindow::from_file(source.to_str().unwrap(), window, cx))
            })
            .unwrap();
        let window = editor.into();
        settle(cx, window);
        if !reopened
            && style == "macos-light"
            && shadow == 0.
            && ((!background && (inset > 0. || transparent))
                || (translucent_background && transparent && inset == 0. && scale == 1.))
        {
            std::fs::write(
                scratch.join(format!("{name}-unannotated.png")),
                cx.update(|cx| editor.read(cx).unwrap().export_png().unwrap()),
            )
            .unwrap();
        }
        editor
            .update(cx, |editor, _, cx| {
                if !reopened {
                    let annotations = [
                        ("inside", 70., 70., 20., 20., "#ff00ff"),
                        ("left", 20., 70., 20., 20., "#00ff00"),
                        ("top", 100., 10., 15., 20., "#ff0000"),
                        ("right", 160., 80., 20., 15., "#ffff00"),
                        ("bottom", 80., 130., 15., 20., "#00ffff"),
                        ("outside", 0., 70., 10., 20., "#ff0000"),
                    ]
                    .into_iter()
                    .map(|(id, x, y, width, height, color)| Annotation::Rectangle {
                        id: id.into(),
                        x,
                        y,
                        width,
                        height,
                        stroke: color.into(),
                        stroke_width: 1.,
                        fill: Some(color.into()),
                    })
                    .collect();
                    editor.history.push(annotations);
                }
                let native_scale =
                    crate::editor::wallpaper::native_scale(200., 160., f64::from(scale));
                editor.zoom = (1. / native_scale) as f32;
                cx.notify();
            })
            .unwrap();
        settle(cx, window);
        let (png, matte, stage) = cx.update(|cx| {
            let editor = editor.read(cx).unwrap();
            assert_eq!(
                editor.snapshot.borrow().balance_crop,
                Some((30., 20., 30., 20.))
            );
            let theme = gpui::Rgba::from(crate::theme::vars::active_theme(cx).background);
            (
                editor.export_png().unwrap(),
                image::Rgba([
                    (theme.r * 255.).round() as u8,
                    (theme.g * 255.).round() as u8,
                    (theme.b * 255.).round() as u8,
                    255,
                ]),
                measured("editor-stage", 0),
            )
        });
        if let Some(previous) = &previous {
            assert_eq!(&png, previous, "{name} must reopen unchanged");
        } else {
            previous = Some(png.clone());
        }
        let saved = image::load_from_memory(&png).unwrap().to_rgba8();
        let native_scale = crate::editor::wallpaper::native_scale(200., 160., f64::from(scale));
        let native_inset = (inset * native_scale).round() as u32;
        let padding = if background {
            (20. * native_scale).round() as u32
        } else {
            0
        };
        let title = (28. * native_scale).round() as u32;
        assert_eq!(
            saved.dimensions(),
            (
                140 + 2 * (padding + native_inset),
                120 + title + 2 * (padding + native_inset)
            ),
            "{name} must preserve the inset around the balanced image"
        );
        if native_inset > 0 {
            assert_eq!(
                saved
                    .get_pixel(
                        padding + native_inset / 2,
                        padding + title + native_inset + 40
                    )
                    .0,
                [230, 230, 230, 255],
                "{name} must use the balanced edge color for the inset"
            );
        }
        if transparent && !background && shadow == 0. && inset == 0. {
            let translucent = saved
                .get_pixel(native_inset + 65, title + native_inset + 55)
                .0;
            let clear = saved
                .get_pixel(native_inset + 85, title + native_inset + 55)
                .0;
            assert_eq!(
                translucent[3], 128,
                "{name} must preserve translucent image pixels"
            );
            assert_eq!(clear[3], 0, "{name} must preserve transparent image pixels");
        }
        if translucent_background && shadow == 0. {
            assert_eq!(
                saved.get_pixel(0, 0).0[3],
                128,
                "{name} must preserve translucent wallpaper pixels"
            );
        }
        let mut expected = image::RgbaImage::from_pixel(saved.width(), saved.height(), matte);
        image::imageops::overlay(&mut expected, &saved, 0, 0);
        let screenshot = cx.capture_screenshot(window).unwrap();
        let left = (f32::from(stage.left()) * scale).round() as u32;
        let top = (f32::from(stage.top()) * scale).round() as u32;
        let preview =
            image::imageops::crop_imm(&screenshot, left, top, saved.width(), saved.height())
                .to_image();
        let differences: Vec<_> = preview
            .pixels()
            .zip(expected.pixels())
            .map(|(actual, expected)| {
                actual
                    .0
                    .iter()
                    .zip(expected.0)
                    .map(|(a, e)| a.abs_diff(e))
                    .max()
                    .unwrap()
            })
            .collect();
        let mismatched = differences
            .iter()
            .filter(|difference| **difference > 2)
            .count();
        let worst = differences.into_iter().max().unwrap_or_default();
        if mismatched > 1
            || worst > 3
            || (scale == 1.3333333
                && style == "macos-light"
                && shadow == 0.
                && !background
                && !reopened)
            || (transparent
                && scale == 1.
                && style == "macos-light"
                && shadow == 0.
                && !background
                && inset == 0.
                && !reopened)
            || (transparent
                && translucent_background
                && scale == 1.3333333
                && style == "macos-light"
                && shadow == 0.
                && inset == 0.
                && !reopened)
        {
            preview
                .save(scratch.join(format!("{name}-preview.png")))
                .unwrap();
            expected
                .save(scratch.join(format!("{name}-expected.png")))
                .unwrap();
        }
        assert!(
            mismatched <= 1 && worst <= 3,
            "{name}, reopened {reopened}: {mismatched} pixels above two, maximum {worst}"
        );
        cx.update_window(window, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
    }
}
