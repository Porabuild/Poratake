#[cfg_attr(target_os = "macos", ignore = "requires development FFmpeg")]
#[test]
fn rounded_camera_redaction_matches_scaled_preview_and_decoded_export() {
    use crate::editor::annotations::Annotation;
    use crate::render::canvas::Canvas;
    use crate::windows::video_editor::{model, preview};

    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("camera-redaction.poratake");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(
        project::recording_video_path(&project),
        include_bytes!("../ui/parity/fixtures/capture.mov"),
    )
    .unwrap();
    let mut camera = Encoder::create(
        &project::camera_video_path(&project),
        Settings {
            width: 160,
            height: 120,
            frame_rate: 12,
            bitrate: 12_000_000,
            has_audio: false,
        },
    )
    .unwrap();
    let mut camera_frame = vec![0; 160 * 120 * 4];
    for y in 0_usize..120 {
        for x in 0_usize..160 {
            let shade = if (x / 6 + y / 6).is_multiple_of(2) {
                32
            } else {
                224
            };
            let index = (y * 160 + x) * 4;
            camera_frame[index..index + 4].copy_from_slice(&[shade, shade, shade, 255]);
        }
    }
    for _ in 0..24 {
        camera.write_frame(camera_frame.clone()).unwrap();
    }
    camera.finish().unwrap();
    let mut state = VideoEditorState {
        source_duration: Some(2.0),
        segments: vec![model::Segment::spanning(2.0)],
        camera_segments: vec![model::CameraSegment {
            id: "camera".into(),
            start_time: 0.0,
            end_time: 2.0,
        }],
        ..VideoEditorState::default()
    };
    state.wallpaper.enabled = false;
    state.export_settings.resolution = "original".into();
    state.export_settings.frame_rate = "12".into();
    state.camera_style.position = "bottom-right".into();
    state.camera_style.size = "large".into();
    state.camera_style.border_radius = 50.0;
    state.camera_style.shadow = 100.0;
    state.zoom_segments.push(model::ZoomSegment {
        id: "zoom".into(),
        start_time: 0.0,
        end_time: 2.0,
        zoom_level: 1.5,
        transition_in_duration: Some(0.0),
        transition_out_duration: Some(0.0),
        target_mode: Some("manual".into()),
        focus_point: Some(model::FocusPoint { x: 0.6, y: 0.4 }),
    });
    state.drawing_segments.push(model::DrawingSegment {
        id: "drawing".into(),
        start_time: 0.0,
        end_time: 2.0,
        canvas_width: 320.0,
        canvas_height: 180.0,
        annotations: vec![Annotation::Redact {
            id: "pixelate".into(),
            x: 280.25,
            y: 140.5,
            width: 40.0,
            height: 40.0,
            style: "pixelate".into(),
            intensity: 8.0,
        }],
    });
    model::save_state(&project, &state);
    assert_eq!(model::load_state(&project), state);
    let mut preview = preview::Source::open(&project, model::load_state(&project)).unwrap();
    let mut without_redaction = state.clone();
    without_redaction.drawing_segments.clear();
    let mut without_redaction = preview::Source::open(&project, without_redaction).unwrap();
    let mut without_camera = state.clone();
    without_camera.camera_segments.clear();
    without_camera.drawing_segments.clear();
    let mut without_camera = preview::Source::open(&project, without_camera).unwrap();
    let output = directory.path().join("camera-redaction.mp4");
    run(
        Request {
            project,
            output: output.clone(),
            state,
        },
        &mut |_| {},
        &|| false,
    )
    .unwrap();
    let exported = VideoDecoder::open(&output).unwrap();
    assert_eq!((exported.info().width, exported.info().height), (320, 180));
    for time in [0.25, 0.5, 1.0] {
        let full = preview.compose(time, None).unwrap();
        let scaled = preview.compose(time, Some((160, 90))).unwrap();
        let plain = without_redaction.compose(time, Some((160, 90))).unwrap();
        let no_camera = without_camera.compose(time, Some((160, 90))).unwrap();
        let decoded = exported.frame_at(time).unwrap();
        let mut reference = Canvas::new(160, 90).unwrap();
        reference.draw_pixmap(full.as_ref(), 0.0, 0.0, 160.0, 90.0);
        let expected = to_bgra(reference.pixmap());
        let scaled = to_bgra(&scaled);
        let plain = to_bgra(&plain);
        let no_camera = to_bgra(&no_camera);
        let mut rendered_error = 0_u64;
        let mut encoded_error = 0_u64;
        let mut effect = 0_u64;
        let mut camera_effect = 0_u64;
        let mut camera_pixels = 0_u64;
        let mut camera_redaction = 0_u64;
        for y in 70..90 {
            for x in 140..160 {
                let index = (y * 160 + x) * 4;
                let camera_difference: u16 = (0..3)
                    .map(|channel| {
                        u16::from(plain[index + channel].abs_diff(no_camera[index + channel]))
                    })
                    .sum();
                if camera_difference > 48 {
                    camera_pixels += 1;
                    camera_redaction += (0..3)
                        .map(|channel| {
                            u64::from(scaled[index + channel].abs_diff(plain[index + channel]))
                        })
                        .sum::<u64>();
                }
                for channel in 0..3 {
                    rendered_error +=
                        u64::from(scaled[index + channel].abs_diff(expected[index + channel]));
                    effect += u64::from(scaled[index + channel].abs_diff(plain[index + channel]));
                    camera_effect +=
                        u64::from(plain[index + channel].abs_diff(no_camera[index + channel]));
                    let mut sum = 0_u16;
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let source = ((y * 2 + dy) * 320 + x * 2 + dx) * 4 + channel;
                            sum += u16::from(decoded.bgra[source]);
                        }
                    }
                    let decoded = ((sum + 2) / 4) as u8;
                    encoded_error += u64::from(scaled[index + channel].abs_diff(decoded));
                }
            }
        }
        let channels = 20.0 * 20.0 * 3.0;
        let rendered_error = rendered_error as f64 / channels;
        let encoded_error = encoded_error as f64 / channels;
        let effect = effect as f64 / channels;
        let camera_effect = camera_effect as f64 / channels;
        println!("{time}s: full-to-scaled {rendered_error:.3}, decoded-to-scaled {encoded_error:.3}, effect {effect:.3}, camera {camera_effect:.3}");
        assert!(camera_effect > 5.0, "camera must be visible at {time}s");
        assert!(effect > 5.0, "pixelation must change the camera at {time}s");
        assert!(
            camera_pixels > 50,
            "camera must cover the selected region at {time}s"
        );
        assert!(
            camera_redaction as f64 / (camera_pixels * 3) as f64 > 5.0,
            "pixelation must change camera pixels at {time}s"
        );
        assert!(
            rendered_error < 1.0,
            "scaled preview must match full composition at {time}s"
        );
        assert!(
            encoded_error < 8.0,
            "scaled preview must match the decoded export at {time}s"
        );
    }
}
