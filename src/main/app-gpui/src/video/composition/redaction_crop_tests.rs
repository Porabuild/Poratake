#[test]
fn source_resolution_crop_preserves_overlapping_video_redactions() {
    use crate::editor::annotations::Annotation;
    use crate::windows::video_editor::model::DrawingSegment;

    let mut config = Config::new(128.0, 96.0, state());
    config.state.wallpaper.enabled = true;
    config.state.wallpaper.padding = 12.0;
    config.state.wallpaper.corners = 8.0;
    config.state.wallpaper.gradient = Some(serde_json::json!({
        "id": "redaction-crop",
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
            Annotation::Rectangle {
                id: "before".into(),
                x: 40.0,
                y: 30.0,
                width: 50.0,
                height: 40.0,
                stroke: "#ffffff".into(),
                stroke_width: 2.0,
                fill: Some("#e04040".into()),
            },
            Annotation::Redact {
                id: "pixelate".into(),
                x: 75.5,
                y: 35.5,
                width: 40.0,
                height: 30.0,
                style: "pixelate".into(),
                intensity: 5.0,
            },
            Annotation::Redact {
                id: "blur".into(),
                x: 60.5,
                y: 45.5,
                width: 35.0,
                height: 25.0,
                style: "blur".into(),
                intensity: 1.0,
            },
            Annotation::Rectangle {
                id: "after".into(),
                x: 80.0,
                y: 55.0,
                width: 20.0,
                height: 20.0,
                stroke: "#00ff00".into(),
                stroke_width: 2.0,
                fill: None,
            },
        ],
    });
    let mut video = source(128, 96, Color::from_rgba8(0, 0, 0, 255));
    for (index, pixel) in video
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        let x = index % 128;
        let y = index / 128;
        pixel[0] = if x % 2 == 0 { 255 } else { 0 };
        pixel[1] = (y * 7 % 256) as u8;
        pixel[2] = (x * 11 % 256) as u8;
    }
    let frames = Frames {
        video: Some(video.as_ref()),
        camera: None,
    };
    let mut engine = Engine::new(config);
    let full = engine.render_frame(0.5, frames).unwrap();
    let mut crop = Canvas::new(110, 100).unwrap();
    crop.translate(-20.0, -10.0);
    engine.render_into(&mut crop, 0.5, frames);
    let crop = crop.into_pixmap();
    for y in 15..85 {
        for x in 25..115 {
            let full_index = (y * 152 + x) * 4;
            let crop_index = ((y - 10) * 110 + x - 20) * 4;
            assert_eq!(
                &full.data()[full_index..full_index + 4],
                &crop.data()[crop_index..crop_index + 4],
                "{x},{y}"
            );
        }
    }
    let mut reference = Canvas::new(76, 60).unwrap();
    reference.draw_pixmap(full.as_ref(), 0.0, 0.0, 76.0, 60.0);
    let preview = engine.render_frame_at(0.5, frames, 76, 60).unwrap();
    let mut corrected = Canvas::from_pixmap(preview.clone());
    corrected.draw_pixmap_blended(
        crop.as_ref(),
        10.0,
        5.0,
        55.0,
        50.0,
        tiny_skia::BlendMode::Source,
    );
    let mut before = 0_u64;
    let mut after = 0_u64;
    for y in 15..40 {
        for x in 20..55 {
            let index = (y * 76 + x) * 4;
            for channel in 0..4 {
                let expected = reference.pixmap().data()[index + channel];
                before += u64::from(preview.data()[index + channel].abs_diff(expected));
                after += u64::from(corrected.pixmap().data()[index + channel].abs_diff(expected));
            }
        }
    }
    assert!(before > 0);
    assert_eq!(after, 0);
    for y in 0..60 {
        for x in 0..76 {
            let inside = ((36..59).contains(&x) && (16..34).contains(&y))
                || ((29..49).contains(&x) && (21..37).contains(&y));
            if !inside {
                continue;
            }
            let offset = (y * 76 + x) * 4;
            for channel in 0..4 {
                let actual = preview.data()[offset + channel];
                let expected = reference.pixmap().data()[offset + channel];
                assert!(actual.abs_diff(expected) <= 1, "overlapping production redactions at {x},{y}, channel {channel}: {actual} vs {expected}");
            }
        }
    }
}

#[test]
fn source_resolution_redaction_crops_match_scaled_full_frames_at_edges() {
    use crate::editor::annotations::{normalize_rect, Annotation};
    use crate::windows::video_editor::model::DrawingSegment;

    let mut video = source(128, 96, Color::from_rgba8(0, 0, 0, 255));
    for (index, pixel) in video
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        let x = index % 128;
        let y = index / 128;
        pixel[0] = if x % 2 == 0 { 255 } else { 0 };
        pixel[1] = (y * 7 % 256) as u8;
        pixel[2] = ((x + y) * 11 % 256) as u8;
    }
    let frames = Frames {
        video: Some(video.as_ref()),
        camera: None,
    };
    for (x, y, width, height) in [
        (-10.5, 30.25, 30.0, 20.0),
        (110.5, 30.25, 30.0, 20.0),
        (40.5, -8.25, 40.0, 18.0),
        (40.5, 86.25, 40.0, 24.0),
        (75.3, 53.7, -35.0, -20.0),
    ] {
        let (left, top, region_width, region_height) = normalize_rect(x, y, width, height);
        let left = left.floor().max(0.0) as u32;
        let top = top.floor().max(0.0) as u32;
        let right = (x.max(x + width)).ceil().min(128.0) as u32;
        let bottom = (y.max(y + height)).ceil().min(96.0) as u32;
        assert!(region_width > 0.0 && region_height > 0.0);
        let crop_left = left.saturating_sub(12);
        let crop_top = top.saturating_sub(12);
        let crop_right = (right + 12).min(128);
        let crop_bottom = (bottom + 12).min(96);
        for style in ["pixelate", "blur"] {
            let mut config = Config::new(128.0, 96.0, state());
            config.state.drawing_segments.push(DrawingSegment {
                id: "drawing".into(),
                start_time: 0.0,
                end_time: 3.0,
                canvas_width: 128.0,
                canvas_height: 96.0,
                annotations: vec![Annotation::Redact {
                    id: "redact".into(),
                    x,
                    y,
                    width,
                    height,
                    style: style.into(),
                    intensity: 1.0,
                }],
            });
            let mut engine = Engine::new(config);
            let full = engine.render_frame(0.5, frames).unwrap();
            let mut crop = Canvas::new(crop_right - crop_left, crop_bottom - crop_top).unwrap();
            crop.translate(-(crop_left as f32), -(crop_top as f32));
            engine.render_into(&mut crop, 0.5, frames);
            let crop = crop.into_pixmap();
            for row in top..bottom {
                for column in left..right {
                    let full_index = ((row * 128 + column) * 4) as usize;
                    let crop_index =
                        (((row - crop_top) * crop.width() + column - crop_left) * 4) as usize;
                    assert_eq!(
                        &full.data()[full_index..full_index + 4],
                        &crop.data()[crop_index..crop_index + 4],
                        "{style} at {column},{row} for {x},{y},{width},{height}"
                    );
                }
            }
            for (output_width, output_height) in
                [(64, 48), (93, 70), (43, 32), (17, 13), (192, 144)]
            {
                let scale_x = output_width as f32 / 128.0;
                let scale_y = output_height as f32 / 96.0;
                let mut reference = Canvas::new(output_width, output_height).unwrap();
                reference.draw_pixmap(
                    full.as_ref(),
                    0.0,
                    0.0,
                    output_width as f32,
                    output_height as f32,
                );
                let production = engine
                    .render_frame_at(0.5, frames, output_width, output_height)
                    .unwrap();
                let mut corrected = Canvas::new(output_width, output_height).unwrap();
                corrected.draw_pixmap_blended(
                    crop.as_ref(),
                    crop_left as f32 * scale_x,
                    crop_top as f32 * scale_y,
                    crop.width() as f32 * scale_x,
                    crop.height() as f32 * scale_y,
                    tiny_skia::BlendMode::Source,
                );
                let output_left = (left as f32 * scale_x).ceil() as u32;
                let output_top = (top as f32 * scale_y).ceil() as u32;
                let output_right = ((right as f32 * scale_x).floor() as u32).min(output_width);
                let output_bottom = ((bottom as f32 * scale_y).floor() as u32).min(output_height);
                for row in output_top..output_bottom {
                    for column in output_left..output_right {
                        let offset = ((row * output_width + column) * 4) as usize;
                        for channel in 0..4 {
                            let expected = reference.pixmap().data()[offset + channel];
                            let actual = corrected.pixmap().data()[offset + channel];
                            assert!(expected.abs_diff(actual) <= 1, "{style} at {column},{row}, output {output_width}x{output_height}: {expected} vs {actual}");
                            let actual = production.data()[offset + channel];
                            assert!(expected.abs_diff(actual) <= 1, "production {style} at {column},{row}, output {output_width}x{output_height}: {expected} vs {actual}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn scaled_video_redaction_matches_resized_full_composition() {
    use crate::editor::annotations::Annotation;
    use crate::windows::video_editor::model::DrawingSegment;

    let mut config = Config::new(128.0, 96.0, state());
    config.state.drawing_segments.push(DrawingSegment {
        id: "drawing".into(),
        start_time: 0.0,
        end_time: 3.0,
        canvas_width: 128.0,
        canvas_height: 96.0,
        annotations: vec![Annotation::Redact {
            id: "pixelate".into(),
            x: 40.5,
            y: 30.5,
            width: 30.0,
            height: 20.0,
            style: "pixelate".into(),
            intensity: 5.0,
        }],
    });
    let mut video = source(128, 96, Color::from_rgba8(0, 0, 0, 255));
    for (index, pixel) in video
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        let x = index % 128;
        let y = index / 128;
        pixel[0] = if x % 2 == 0 { 255 } else { 0 };
        pixel[1] = (y * 7 % 256) as u8;
        pixel[2] = (x * 11 % 256) as u8;
    }
    let frames = Frames {
        video: Some(video.as_ref()),
        camera: None,
    };
    let mut engine = Engine::new(config);
    let full = engine.render_frame(0.5, frames).unwrap();
    for (width, height) in [(64, 48), (93, 70), (17, 13), (192, 144)] {
        let mut reference = Canvas::new(width, height).unwrap();
        reference.draw_pixmap(full.as_ref(), 0.0, 0.0, width as f32, height as f32);
        let preview = engine.render_frame_at(0.5, frames, width, height).unwrap();
        let scale_x = width as f32 / 128.0;
        let scale_y = height as f32 / 96.0;
        let left = ((40.0 * scale_x).floor() as u32).saturating_sub(1);
        let top = ((30.0 * scale_y).floor() as u32).saturating_sub(1);
        let right = ((71.0 * scale_x).ceil() as u32 + 1).min(width);
        let bottom = ((51.0 * scale_y).ceil() as u32 + 1).min(height);
        for y in top..bottom {
            for x in left..right {
                let offset = ((y * width + x) * 4) as usize;
                for channel in 0..4 {
                    let actual = preview.data()[offset + channel];
                    let expected = reference.pixmap().data()[offset + channel];
                    assert!(
                        actual.abs_diff(expected) <= 1,
                        "{width}x{height} at {x},{y}, channel {channel}: {actual} vs {expected}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore]
fn source_resolution_redaction_crop_timing() {
    use crate::editor::annotations::Annotation;
    use crate::windows::video_editor::model::DrawingSegment;
    use std::time::Instant;

    let mut config = Config::new(3840.0, 2160.0, state());
    config.state.drawing_segments.push(DrawingSegment {
        id: "drawing".into(),
        start_time: 0.0,
        end_time: 3.0,
        canvas_width: 3840.0,
        canvas_height: 2160.0,
        annotations: vec![Annotation::Redact {
            id: "pixelate".into(),
            x: 1000.0,
            y: 500.0,
            width: 800.0,
            height: 600.0,
            style: "pixelate".into(),
            intensity: 5.0,
        }],
    });
    let video = source(3840, 2160, Color::from_rgba8(32, 64, 128, 255));
    let frames = Frames {
        video: Some(video.as_ref()),
        camera: None,
    };
    let mut engine = Engine::new(config);
    let mut unredacted = Engine::new(Config::new(3840.0, 2160.0, state()));
    let mut preview_times = Vec::new();
    let mut patch_times = Vec::new();
    let mut unredacted_patch_times = Vec::new();
    let mut full_times = Vec::new();
    for _ in 0..8 {
        let start = Instant::now();
        engine.render_frame_scaled(0.5, frames, 960, 540).unwrap();
        preview_times.push(start.elapsed().as_secs_f64() * 1000.0);
        let start = Instant::now();
        let mut crop = Canvas::new(832, 632).unwrap();
        crop.translate(-984.0, -484.0);
        engine.render_into(&mut crop, 0.5, frames);
        patch_times.push(start.elapsed().as_secs_f64() * 1000.0);
        let start = Instant::now();
        let mut crop = Canvas::new(832, 632).unwrap();
        crop.translate(-984.0, -484.0);
        unredacted.render_into(&mut crop, 0.5, frames);
        unredacted_patch_times.push(start.elapsed().as_secs_f64() * 1000.0);
        let start = Instant::now();
        engine.render_frame(0.5, frames).unwrap();
        full_times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    for (name, values) in [
        ("preview", preview_times),
        ("patch", patch_times),
        ("unredacted patch", unredacted_patch_times),
        ("full", full_times),
    ] {
        let mut values = values[1..].to_vec();
        values.sort_by(f64::total_cmp);
        println!("{name}: median {:.2}ms, samples {values:?}", values[3]);
    }
}
