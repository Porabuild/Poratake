use gpui::AppContext;
use herogpui::gpui;

use crate::editor::annotations::Annotation;
use crate::editor::wallpaper::GradientOption;
use crate::editor::window::EditorWindow;

use super::{large_texture_sampling::pan, measured, px, settle, size, HeadlessAppContext, Path};

pub(super) fn run(cx: &mut HeadlessAppContext, scratch: &Path) {
    let scale = std::env::var("PORATAKE_PARITY_LARGE_DPI")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(1.);
    let zoom = std::env::var("PORATAKE_PARITY_LARGE_ZOOM")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(0.25);
    let style =
        std::env::var("PORATAKE_PARITY_LARGE_FRAME").unwrap_or_else(|_| "macos-dark".into());
    let shadow = std::env::var("PORATAKE_PARITY_LARGE_SHADOW")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.);
    let inset = std::env::var("PORATAKE_PARITY_LARGE_INSET")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.);
    let corners = std::env::var("PORATAKE_PARITY_LARGE_CORNERS")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.);
    let annotation = std::env::var("PORATAKE_PARITY_LARGE_ANNOTATION").ok();
    let pan_first = std::env::var_os("PORATAKE_PARITY_LARGE_PAN_FIRST").is_some();
    assert!(zoom == 0.25 || zoom == 0.5);
    for horizontal in [true, false] {
        for transparent in [false, true] {
            let (width, height) = if horizontal {
                (17_024, 128)
            } else {
                (128, 17_024)
            };
            let path = scratch.join(format!("framed-source-{horizontal}-{transparent}.png"));
            image::RgbaImage::from_fn(width, height, |x, y| {
                let (long, short) = if horizontal { (x, y) } else { (y, x) };
                let shade = if (long / 5 + short / 7) % 2 == 0 {
                    220
                } else {
                    30
                };
                let alpha = if transparent {
                    ((long * 11 + short * 7) % 256) as u8
                } else {
                    255
                };
                image::Rgba([shade, (long % 251) as u8, (short * 2) as u8, alpha])
            })
            .save(&path)
            .unwrap();
            let editor = cx
                .open_window(size(px(1280.), px(738.)), |window, cx| {
                    window.set_scale_factor(scale);
                    cx.new(|cx| EditorWindow::from_file(path.to_str().unwrap(), window, cx))
                })
                .unwrap();
            editor
                .update(cx, |editor, _, cx| {
                    editor.wallpaper.gradient = Some(GradientOption {
                        id: "framed".into(),
                        colors: vec!["#203060".into()],
                        angle: 0.,
                    });
                    editor.wallpaper.padding = 20.;
                    editor.wallpaper.inset = inset;
                    editor.wallpaper.corners = corners;
                    editor.wallpaper.shadow = shadow;
                    editor.wallpaper.window_frame.style = style.clone();
                    if let Some(style) = &annotation {
                        let (x, y, width, height) = if horizontal {
                            (8162., 42., 60., 44.)
                        } else {
                            (42., 8162., 44., 60.)
                        };
                        editor.history.push(vec![Annotation::Redact {
                            id: "redact".into(),
                            x,
                            y,
                            width,
                            height,
                            style: style.clone(),
                            intensity: 5.,
                        }]);
                    }
                    cx.notify();
                })
                .unwrap();
            settle(cx, editor.into());
            editor
                .update(cx, |editor, _, cx| {
                    editor.zoom = zoom;
                    cx.notify();
                })
                .unwrap();
            settle(cx, editor.into());
            let (png, frame_scale) = cx.update(|cx| {
                let editor = editor.read(cx).unwrap();
                let snapshot = editor.snapshot.borrow();
                assert!(snapshot.backdrop.is_some(), "backdrop was not ready");
                if snapshot.backdrop_composite {
                    assert!(
                        !snapshot.preview_edges.is_empty(),
                        "composed backdrop was not tiled"
                    );
                } else {
                    assert!(
                        !snapshot.image_tiles.is_empty(),
                        "oversized source was not tiled"
                    );
                    #[cfg(target_os = "macos")]
                    assert!(
                        !snapshot.backdrop_tiles.is_empty(),
                        "oversized backdrop was not tiled"
                    );
                }
                println!(
                    "large backdrop composed={} edge_tiles={}",
                    snapshot.backdrop_composite,
                    snapshot.preview_edges.len()
                );
                (editor.export_png().unwrap(), snapshot.frame_scale)
            });
            let output = image::load_from_memory(&png).unwrap().to_rgba8();
            if let Some(style) = &annotation {
                let control_png = editor
                    .update(cx, |editor, _, _| {
                        assert!(editor.history.undo());
                        let png = editor.export_png().unwrap();
                        assert!(editor.history.redo());
                        png
                    })
                    .unwrap();
                let control = image::load_from_memory(&control_png).unwrap().to_rgba8();
                assert_eq!(output.dimensions(), control.dimensions());
                let modified = output
                    .pixels()
                    .zip(control.pixels())
                    .filter(|(a, b)| a != b)
                    .count();
                println!("large framed control=annotation style={style} horizontal={horizontal} transparent={transparent} modified_pixels={modified}");
                assert!(modified > 0, "the annotation must change exported pixels");
            }
            let mut controls = Vec::new();
            if shadow > 0. {
                controls.push(("shadow", 0., corners));
            }
            if corners > 0. {
                controls.push(("corners", shadow, 0.));
            }
            for (name, control_shadow, control_corners) in controls {
                let control_png = editor
                    .update(cx, |editor, _, _| {
                        let old_shadow = editor.wallpaper.shadow;
                        let old_corners = editor.wallpaper.corners;
                        editor.wallpaper.shadow = control_shadow;
                        editor.wallpaper.corners = control_corners;
                        let png = editor.export_png().unwrap();
                        editor.wallpaper.shadow = old_shadow;
                        editor.wallpaper.corners = old_corners;
                        png
                    })
                    .unwrap();
                let control = image::load_from_memory(&control_png).unwrap().to_rgba8();
                assert_eq!(output.dimensions(), control.dimensions());
                let modified = output
                    .pixels()
                    .zip(control.pixels())
                    .filter(|(a, b)| a != b)
                    .count();
                println!("large framed control={name} horizontal={horizontal} transparent={transparent} modified_pixels={modified}");
                assert!(
                    modified > 0,
                    "the {name} setting must change the exported pixels"
                );
            }
            let settings =
                cx.update(|cx| editor.read(cx).unwrap().wallpaper.for_scale(frame_scale));
            let (_, (x, y, _, _)) = crate::editor::wallpaper::layout(
                &settings,
                f64::from(width),
                f64::from(height),
                frame_scale,
                false,
            );
            let ox = x.round() as u32;
            let oy = (y.round()
                + crate::render::window_frame::title_bar_height(
                    &settings.window_frame.style,
                    frame_scale,
                )) as u32;
            if annotation.as_deref() == Some("blackout") {
                let (x, y) = if horizontal {
                    (ox + 8192, oy + 64)
                } else {
                    (ox + 64, oy + 8192)
                };
                assert_eq!(output.get_pixel(x, y).0, [0, 0, 0, 255]);
            }
            let (a, b) = if horizontal {
                ((ox + 8000, oy + 45), (ox + 8005, oy + 45))
            } else {
                ((ox + 45, oy + 8000), (ox + 45, oy + 8005))
            };
            assert_ne!(
                output.get_pixel(a.0, a.1),
                output.get_pixel(b.0, b.1),
                "export must retain image detail"
            );
            let exported = scratch.join(format!("framed-export-{horizontal}-{transparent}.png"));
            std::fs::write(&exported, png).unwrap();
            let reopened = cx
                .open_window(size(px(1280.), px(738.)), |window, cx| {
                    window.set_scale_factor(scale);
                    cx.new(|cx| EditorWindow::from_file(exported.to_str().unwrap(), window, cx))
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
            for panned in if pan_first {
                &[true][..]
            } else {
                &[false, true]
            } {
                let panned = *panned;
                if panned {
                    pan(cx, editor, horizontal, zoom);
                    pan(cx, reopened, horizontal, zoom);
                }
                settle(cx, editor.into());
                let stage = measured("editor-stage", 0);
                assert_eq!(cx.update(|cx| editor.read(cx).unwrap().zoom), zoom);
                let actual = cx.capture_screenshot(editor.into()).unwrap();
                settle(cx, reopened.into());
                assert_eq!(cx.update(|cx| reopened.read(cx).unwrap().zoom), zoom);
                let flat_bounds =
                    cx.update(|cx| reopened.read(cx).unwrap().bounds.borrow().unwrap());
                let expected = cx.capture_screenshot(reopened.into()).unwrap();
                assert_eq!(stage, flat_bounds, "preview and reopened stage bounds");
                let left = (f32::from(stage.left()).max(200.) * scale).ceil() as u32;
                let top = (f32::from(stage.top()).max(120.) * scale).ceil() as u32;
                let right = (f32::from(stage.right()).min(1080.) * scale).floor() as u32;
                let bottom = (f32::from(stage.bottom()).min(650.) * scale).floor() as u32;
                assert!(right > left && bottom > top);
                if panned {
                    let source_x = ox + if horizontal { 8192 } else { 64 };
                    let source_y = oy + if horizontal { 64 } else { 8192 };
                    let seam_x =
                        ((f32::from(stage.left()) + source_x as f32 * zoom) * scale).round() as u32;
                    let seam_y =
                        ((f32::from(stage.top()) + source_y as f32 * zoom) * scale).round() as u32;
                    assert!(
                        (left..right).contains(&seam_x) && (top..bottom).contains(&seam_y),
                        "the tiled seam must be in the compared region"
                    );
                    if let Some(style) = &annotation {
                        assert!(
                            seam_x >= left + 2
                                && seam_x + 2 < right
                                && seam_y >= top + 2
                                && seam_y + 2 < bottom,
                            "the annotation sample must be visible"
                        );
                        if style == "blackout" {
                            assert_eq!(
                                expected.get_pixel(seam_x, seam_y).0,
                                [0, 0, 0, 255],
                                "the sampled point must lie inside the blackout"
                            );
                        }
                        let mut changed = 0;
                        let mut worst = 0;
                        for y in seam_y - 2..=seam_y + 2 {
                            for x in seam_x - 2..=seam_x + 2 {
                                let delta = actual
                                    .get_pixel(x, y)
                                    .0
                                    .iter()
                                    .zip(expected.get_pixel(x, y).0)
                                    .map(|(a, b)| a.abs_diff(b))
                                    .max()
                                    .unwrap();
                                changed += usize::from(delta > 2);
                                worst = worst.max(delta);
                            }
                        }
                        println!("large annotation style={style} horizontal={horizontal} transparent={transparent} zoom={zoom} scale={scale} differences={changed} max={worst}");
                        assert_eq!(changed, 0, "annotation differs from reopened PNG");
                    }
                }
                let (mut changed, mut worst) = (0, 0);
                for y in top..bottom {
                    for x in left..right {
                        let a = actual.get_pixel(x, y).0;
                        let b = expected.get_pixel(x, y).0;
                        let delta = a.iter().zip(b).map(|(a, b)| a.abs_diff(b)).max().unwrap();
                        changed += usize::from(delta > 2);
                        worst = worst.max(delta);
                    }
                }
                println!("large framed style={style} shadow={shadow} inset={inset} corners={corners} horizontal={horizontal} transparent={transparent} zoom={zoom} scale={scale} panned={panned} differences={changed} max={worst}");
                if changed > 0 {
                    actual
                        .save(scratch.join(format!(
                            "framed-preview-{horizontal}-{transparent}-{panned}.png"
                        )))
                        .unwrap();
                    expected
                        .save(scratch.join(format!(
                            "framed-reopened-{horizontal}-{transparent}-{panned}.png"
                        )))
                        .unwrap();
                }
                assert_eq!(changed, 0, "framed preview differs from reopened PNG");
            }
            for window in [editor, reopened] {
                cx.update_window(window.into(), |_, window, _| window.remove_window())
                    .unwrap();
            }
        }
    }
}
