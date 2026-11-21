#[test]
fn source_resolution_redaction_crop_preserves_zoom_camera_and_text() {
    use crate::editor::annotations::Annotation;
    use crate::windows::video_editor::model::{DrawingSegment, FocusPoint, ZoomSegment};

    let mut config = Config::new(256.0, 144.0, state());
    config.state.camera_segments = vec![camera_segment(0.0, 3.0)];
    config.state.camera_style.visible = true;
    config.state.zoom_segments.push(ZoomSegment {
        id: "zoom".into(),
        start_time: 0.0,
        end_time: 3.0,
        zoom_level: 1.5,
        transition_in_duration: Some(0.0),
        transition_out_duration: Some(0.0),
        target_mode: Some("manual".into()),
        focus_point: Some(FocusPoint { x: 0.6, y: 0.4 }),
    });
    config.state.drawing_segments.push(DrawingSegment {
        id: "drawing".into(),
        start_time: 0.0,
        end_time: 3.0,
        canvas_width: 256.0,
        canvas_height: 144.0,
        annotations: vec![
            Annotation::Text {
                id: "text-before".into(),
                x: 120.0,
                y: 70.0,
                text: "Private".into(),
                font_size: 18.0,
                fill: "#ffffff".into(),
                font_family: None,
                background_color: None,
                background_opacity: None,
                background_padding: None,
                background_radius: None,
                rotation: None,
            },
            Annotation::Redact {
                id: "pixelate".into(),
                x: 120.0,
                y: 65.0,
                width: 100.0,
                height: 50.0,
                style: "pixelate".into(),
                intensity: 5.0,
            },
            Annotation::Text {
                id: "text-after".into(),
                x: 155.0,
                y: 100.0,
                text: "Note".into(),
                font_size: 14.0,
                fill: "#00ff00".into(),
                font_family: None,
                background_color: None,
                background_opacity: None,
                background_padding: None,
                background_radius: None,
                rotation: None,
            },
        ],
    });
    let mut video = source(256, 144, Color::from_rgba8(0, 0, 0, 255));
    for (index, pixel) in video
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        let x = index % 256;
        let y = index / 256;
        pixel[0] = (x * 13 % 256) as u8;
        pixel[1] = (y * 7 % 256) as u8;
        pixel[2] = ((x + y) * 11 % 256) as u8;
    }
    let camera = source(64, 64, Color::from_rgba8(255, 32, 80, 255));
    let frames = Frames {
        video: Some(video.as_ref()),
        camera: Some(camera.as_ref()),
    };
    let mut unzoomed_config = Config::new(256.0, 144.0, config.state.clone());
    unzoomed_config.state.zoom_segments.clear();
    let mut unzoomed = Engine::new(unzoomed_config);
    let mut unredacted_config = Config::new(256.0, 144.0, config.state.clone());
    unredacted_config.state.drawing_segments[0]
        .annotations
        .retain(|annotation| !matches!(annotation, Annotation::Redact { .. }));
    let mut unredacted = Engine::new(unredacted_config);
    let mut engine = Engine::new(config);
    let full = engine.render_frame(1.0, frames).unwrap();
    let without_camera = engine
        .render_frame(
            1.0,
            Frames {
                video: Some(video.as_ref()),
                camera: None,
            },
        )
        .unwrap();
    assert_ne!(full.data(), without_camera.data());
    let without_zoom = unzoomed.render_frame(1.0, frames).unwrap();
    assert_ne!(full.data(), without_zoom.data());
    let mut crop = Canvas::new(136, 100).unwrap();
    crop.translate(-100.0, -40.0);
    engine.render_into(&mut crop, 1.0, frames);
    let crop = crop.into_pixmap();
    for y in 50..130 {
        for x in 110..230 {
            let full_index = (y * 256 + x) * 4;
            let crop_index = ((y - 40) * 136 + x - 100) * 4;
            assert_eq!(
                &full.data()[full_index..full_index + 4],
                &crop.data()[crop_index..crop_index + 4],
                "{x},{y}"
            );
        }
    }
    let preview = engine.render_frame_at(1.0, frames, 93, 52).unwrap();
    let original = unredacted.render_frame_at(1.0, frames, 93, 52).unwrap();
    let mut corrected = Canvas::from_pixmap(preview.clone());
    let scale_x = 93.0 / 256.0;
    let scale_y = 52.0 / 144.0;
    corrected.draw_pixmap_blended(
        crop.as_ref(),
        100.0 * scale_x,
        40.0 * scale_y,
        136.0 * scale_x,
        100.0 * scale_y,
        tiny_skia::BlendMode::Source,
    );
    let mut changed = 0;
    let mut largest = 0;
    for y in 0..52 {
        for x in 0..93 {
            let source_x = (x as f32 + 0.5) / scale_x;
            let source_y = (y as f32 + 0.5) / scale_y;
            if (118.0..222.0).contains(&source_x) && (63.0..117.0).contains(&source_y) {
                continue;
            }
            let offset = (y * 93 + x) * 4;
            let difference = (0..4)
                .map(|channel| {
                    preview.data()[offset + channel]
                        .abs_diff(corrected.pixmap().data()[offset + channel])
                })
                .max()
                .unwrap();
            changed += usize::from(difference > 0);
            largest = largest.max(difference);
        }
    }
    println!("whole crop changed {changed} pixels outside the redaction, maximum channel difference {largest}");

    let clip_left = ((120.0 * scale_x).floor() as u32).saturating_sub(1);
    let clip_top = ((65.0 * scale_y).floor() as u32).saturating_sub(1);
    let clip_right = ((220.0 * scale_x).ceil() as u32 + 1).min(93);
    let clip_bottom = ((115.0 * scale_y).ceil() as u32 + 1).min(52);
    let mut patch = Canvas::new(clip_right - clip_left, clip_bottom - clip_top).unwrap();
    patch.translate(-(clip_left as f32), -(clip_top as f32));
    patch.draw_pixmap_blended(
        crop.as_ref(),
        100.0 * scale_x,
        40.0 * scale_y,
        136.0 * scale_x,
        100.0 * scale_y,
        tiny_skia::BlendMode::Source,
    );
    let mut masked = Canvas::from_pixmap(preview.clone());
    masked.draw_pixmap_blended(
        patch.pixmap().as_ref(),
        clip_left as f32,
        clip_top as f32,
        (clip_right - clip_left) as f32,
        (clip_bottom - clip_top) as f32,
        tiny_skia::BlendMode::Source,
    );
    let mut reference = Canvas::new(93, 52).unwrap();
    reference.draw_pixmap(full.as_ref(), 0.0, 0.0, 93.0, 52.0);
    for y in 0..52 {
        for x in 0..93 {
            let offset = ((y * 93 + x) * 4) as usize;
            let inside =
                (clip_left..clip_right).contains(&x) && (clip_top..clip_bottom).contains(&y);
            let expected = if inside {
                reference.pixmap().data()
            } else {
                preview.data()
            };
            for channel in 0..4 {
                let actual = masked.pixmap().data()[offset + channel];
                assert!(
                    actual.abs_diff(expected[offset + channel]) <= u8::from(inside),
                    "masked crop at {x},{y}, channel {channel}"
                );
                let expected = if inside {
                    reference.pixmap().data()[offset + channel]
                } else {
                    original.data()[offset + channel]
                };
                let actual = preview.data()[offset + channel];
                assert!(
                    actual.abs_diff(expected) <= u8::from(inside),
                    "production zoom crop at {x},{y}, channel {channel}: {actual} vs {expected}"
                );
            }
        }
    }
}

#[test]
fn separate_redaction_patches_preserve_transparent_neighbors() {
    use crate::editor::annotations::Annotation;
    use crate::windows::video_editor::model::DrawingSegment;

    let mut config = Config::new(128.0, 96.0, state());
    config.state.wallpaper.enabled = true;
    config.state.wallpaper.padding = 12.0;
    config.state.wallpaper.gradient = Some(serde_json::json!({
        "id": "transparent-patches",
        "colors": ["#80402080", "#20408080"],
        "angle": 135.0,
    }));
    config.state.drawing_segments.push(DrawingSegment {
        id: "drawing".into(),
        start_time: 0.0,
        end_time: 3.0,
        canvas_width: 152.0,
        canvas_height: 120.0,
        annotations: vec![
            Annotation::Redact {
                id: "first".into(),
                x: -4.5,
                y: 0.5,
                width: 35.0,
                height: 25.0,
                style: "pixelate".into(),
                intensity: 5.0,
            },
            Annotation::Redact {
                id: "second".into(),
                x: 120.5,
                y: 91.5,
                width: 40.0,
                height: 35.0,
                style: "blur".into(),
                intensity: 1.0,
            },
        ],
    });
    let mut video = source(128, 96, Color::from_rgba8(0, 0, 0, 128));
    for (index, pixel) in video
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        let x = index % 128;
        let y = index / 128;
        pixel[0] = if x % 2 == 0 { 128 } else { 0 };
        pixel[1] = (y * 7 % 129) as u8;
        pixel[2] = ((x + y) * 11 % 129) as u8;
    }
    let frames = Frames {
        video: Some(video.as_ref()),
        camera: None,
    };
    let mut unredacted_config = Config::new(128.0, 96.0, config.state.clone());
    unredacted_config.state.drawing_segments.clear();
    let mut unredacted = Engine::new(unredacted_config);
    let mut engine = Engine::new(config);
    let full = engine.render_frame(0.5, frames).unwrap();
    let preview = engine.render_frame_at(0.5, frames, 109, 86).unwrap();
    let original = unredacted.render_frame_at(0.5, frames, 109, 86).unwrap();
    let mut corrected = Canvas::from_pixmap(preview.clone());
    let scale_x = 109.0_f32 / 152.0;
    let scale_y = 86.0_f32 / 120.0;
    let mut clips = Vec::new();
    for (crop_left, crop_top, crop_right, crop_bottom, left, top, right, bottom) in [
        (0, 0, 45, 40, 0, 0, 31, 26),
        (108, 79, 152, 120, 120, 91, 152, 120),
    ] {
        let mut crop = Canvas::new(crop_right - crop_left, crop_bottom - crop_top).unwrap();
        crop.translate(-(crop_left as f32), -(crop_top as f32));
        engine.render_into(&mut crop, 0.5, frames);
        let clip_left = ((left as f32 * scale_x).floor() as u32).saturating_sub(1);
        let clip_top = ((top as f32 * scale_y).floor() as u32).saturating_sub(1);
        let clip_right = ((right as f32 * scale_x).ceil() as u32 + 1).min(109);
        let clip_bottom = ((bottom as f32 * scale_y).ceil() as u32 + 1).min(86);
        clips.push((clip_left, clip_top, clip_right, clip_bottom));
        let mut patch = Canvas::new(clip_right - clip_left, clip_bottom - clip_top).unwrap();
        patch.translate(-(clip_left as f32), -(clip_top as f32));
        patch.draw_pixmap_blended(
            crop.pixmap().as_ref(),
            crop_left as f32 * scale_x,
            crop_top as f32 * scale_y,
            (crop_right - crop_left) as f32 * scale_x,
            (crop_bottom - crop_top) as f32 * scale_y,
            tiny_skia::BlendMode::Source,
        );
        corrected.draw_pixmap_blended(
            patch.pixmap().as_ref(),
            clip_left as f32,
            clip_top as f32,
            (clip_right - clip_left) as f32,
            (clip_bottom - clip_top) as f32,
            tiny_skia::BlendMode::Source,
        );
    }
    let mut reference = Canvas::new(109, 86).unwrap();
    reference.draw_pixmap(full.as_ref(), 0.0, 0.0, 109.0, 86.0);
    for y in 0..86 {
        for x in 0..109 {
            let offset = ((y * 109 + x) * 4) as usize;
            let inside = clips.iter().any(|&(left, top, right, bottom)| {
                (left..right).contains(&x) && (top..bottom).contains(&y)
            });
            let expected = if inside {
                reference.pixmap().data()
            } else {
                preview.data()
            };
            for channel in 0..4 {
                let actual = corrected.pixmap().data()[offset + channel];
                assert!(
                    actual.abs_diff(expected[offset + channel]) <= u8::from(inside),
                    "transparent crop at {x},{y}, channel {channel}"
                );
                let expected = if inside {
                    reference.pixmap().data()[offset + channel]
                } else {
                    original.data()[offset + channel]
                };
                let actual = preview.data()[offset + channel];
                assert!(
                    actual.abs_diff(expected) <= u8::from(inside),
                    "production transparent crop at {x},{y}, channel {channel}: {actual} vs {expected}"
                );
            }
        }
    }
}
