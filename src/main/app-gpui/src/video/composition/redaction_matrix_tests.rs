#[test]
fn generated_redaction_matrix_matches_full_composition_at_effect_boundaries() {
    use crate::editor::annotations::{normalize_rect, Annotation};
    use crate::windows::video_editor::model::{DrawingSegment, FocusPoint, ZoomSegment};

    fn next(seed: &mut u64) -> u32 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (*seed >> 32) as u32
    }

    let mut video = source(96, 64, Color::from_rgba8(0, 0, 0, 255));
    for (index, pixel) in video
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        let x = index % 96;
        let y = index / 96;
        *pixel = [
            if x.is_multiple_of(2) { 255 } else { 0 },
            (y * 11 % 256) as u8,
            ((x + y) * 17 % 256) as u8,
            255,
        ];
    }
    let camera = source(24, 24, Color::from_rgba8(240, 40, 80, 255));
    let frames = Frames {
        video: Some(video.as_ref()),
        camera: Some(camera.as_ref()),
    };
    let mut seed = 0x8ce0a80b;
    let mut checked_inside = 0;
    let mut checked_outside = 0;
    let mut blurred = 0;
    let mut pixelated = 0;
    let mut clipped = 0;
    let mut reversed = 0;
    let mut overlapping = 0;
    let mut intensities = [false; 10];
    for case in 0_u32..316 {
        let selected = case < 48 || case == 315;
        let mut config = Config::new(96.0, 64.0, state());
        let (author_width, author_height) = match case % 3 {
            0 => (96.0, 64.0),
            1 => (80.0, 120.0),
            _ => (240.0, 135.0),
        };
        if case.is_multiple_of(3) {
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
        }
        let mut annotations = vec![Annotation::Rectangle {
            id: "before".into(),
            x: 15.0,
            y: 12.0,
            width: 45.0,
            height: 35.0,
            stroke: "#ffffff".into(),
            stroke_width: 2.0,
            fill: Some("#a03060".into()),
        }];
        let mut regions = Vec::new();
        for effect in 0..1 + next(&mut seed) % 3 {
            let x = (next(&mut seed) % 116) as f64 - 10.0 + 0.25;
            let y = (next(&mut seed) % 84) as f64 - 10.0 + 0.5;
            let mut width = (next(&mut seed) % 38 + 6) as f64;
            let mut height = (next(&mut seed) % 30 + 6) as f64;
            if next(&mut seed).is_multiple_of(2) {
                width = -width;
            }
            if next(&mut seed).is_multiple_of(2) {
                height = -height;
            }
            let style = if next(&mut seed).is_multiple_of(2) {
                "blur"
            } else {
                "pixelate"
            };
            let intensity = (next(&mut seed) % 10 + 1) as f64;
            let region = normalize_rect(x, y, width, height);
            if selected {
                blurred += usize::from(style == "blur");
                pixelated += usize::from(style == "pixelate");
                reversed += usize::from(width < 0.0 || height < 0.0);
                intensities[intensity as usize - 1] = true;
                clipped += usize::from(
                    region.0 < 0.0
                        || region.1 < 0.0
                        || region.0 + region.2 > 96.0
                        || region.1 + region.3 > 64.0,
                );
                overlapping += usize::from(regions.iter().any(|&(left, top, w, h)| {
                    left < region.0 + region.2
                        && region.0 < left + w
                        && top < region.1 + region.3
                        && region.1 < top + h
                }));
            }
            regions.push(region);
            annotations.push(Annotation::Redact {
                id: format!("effect-{effect}"),
                x: x * author_width / 96.0,
                y: y * author_height / 64.0,
                width: width * author_width / 96.0,
                height: height * author_height / 64.0,
                style: style.into(),
                intensity,
            });
        }
        annotations.push(Annotation::Rectangle {
            id: "after".into(),
            x: 55.0,
            y: 30.0,
            width: 18.0,
            height: 18.0,
            stroke: "#00ff00".into(),
            stroke_width: 2.0,
            fill: None,
        });
        if !selected {
            continue;
        }
        config.state.drawing_segments.push(DrawingSegment {
            id: "drawing".into(),
            start_time: 0.0,
            end_time: 3.0,
            canvas_width: author_width,
            canvas_height: author_height,
            annotations,
        });
        let mut unredacted_config = Config::new(96.0, 64.0, config.state.clone());
        unredacted_config.state.drawing_segments[0]
            .annotations
            .retain(|annotation| !matches!(annotation, Annotation::Redact { .. }));
        let mut unredacted = Engine::new(unredacted_config);
        let mut engine = Engine::new(config);
        let full = engine.render_frame(1.0, frames).unwrap();
        for (width, height) in [(48, 32), (73, 49), (29, 19), (145, 97)] {
            let scale_x = width as f32 / 96.0;
            let scale_y = height as f32 / 64.0;
            let mut reference = Canvas::new(width, height).unwrap();
            reference.draw_pixmap(full.as_ref(), 0.0, 0.0, width as f32, height as f32);
            let mut raw = Canvas::new(width, height).unwrap();
            raw.set_shadow_scale((scale_x + scale_y) / 2.0);
            raw.scale(scale_x, scale_y);
            engine.render_into(&mut raw, 1.0, frames);
            let mut without_effects = Canvas::new(width, height).unwrap();
            without_effects.set_shadow_scale((scale_x + scale_y) / 2.0);
            without_effects.scale(scale_x, scale_y);
            unredacted.render_into(&mut without_effects, 1.0, frames);
            let preview = engine.render_frame_at(1.0, frames, width, height).unwrap();
            for y in 0..height {
                for x in 0..width {
                    let inside = regions.iter().any(|&(left, top, w, h)| {
                        let right = ((left as f32 + w as f32).ceil().clamp(0.0, 96.0)) as u32;
                        let bottom = ((top as f32 + h as f32).ceil().clamp(0.0, 64.0)) as u32;
                        let left = (left as f32).floor().clamp(0.0, 96.0) as u32;
                        let top = (top as f32).floor().clamp(0.0, 64.0) as u32;
                        if left >= right || top >= bottom {
                            return false;
                        }
                        let output_left =
                            ((left as f32 * scale_x).floor() as u32).saturating_sub(1);
                        let output_top = ((top as f32 * scale_y).floor() as u32).saturating_sub(1);
                        let output_right = ((right as f32 * scale_x).ceil() as u32 + 1).min(width);
                        let output_bottom =
                            ((bottom as f32 * scale_y).ceil() as u32 + 1).min(height);
                        (output_left..output_right).contains(&x)
                            && (output_top..output_bottom).contains(&y)
                    });
                    let outside = !inside;
                    let expected = if inside {
                        reference.pixmap().data()
                    } else {
                        raw.pixmap().data()
                    };
                    let offset = ((y * width + x) * 4) as usize;
                    for channel in 0..4 {
                        let actual = preview.data()[offset + channel];
                        let expected = expected[offset + channel];
                        assert!(actual.abs_diff(expected) <= u8::from(inside),
                            "case {case} output {width}x{height} at {x},{y}, channel {channel}, inside={inside}: {actual} vs {expected}, regions={regions:?}");
                        if outside {
                            let untouched = without_effects.pixmap().data()[offset + channel];
                            assert_eq!(actual, untouched, "redaction changed neighboring pixel: case {case}, output {width}x{height}, {x},{y}, channel {channel}");
                        }
                    }
                    checked_inside += usize::from(inside);
                    checked_outside += usize::from(outside);
                }
            }
        }
    }
    assert!(checked_inside > 1000 && checked_outside > 1000);
    assert!(blurred > 0 && pixelated > 0 && clipped > 0 && reversed > 0 && overlapping > 0);
    assert!(intensities.into_iter().all(|seen| seen));
    println!("Checked {checked_inside} effect pixels and {checked_outside} outside pixels");
    println!("Effects: {blurred} blur, {pixelated} pixelation, {clipped} clipped, {reversed} reverse-dragged, {overlapping} overlapping");
}
