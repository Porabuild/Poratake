use std::path::Path;
use std::sync::Arc;

use herogpui::gpui::{HeadlessAppContext, WindowHandle};

use crate::editor::annotations::{normalize_rect, Annotation};
use crate::video::decoder::DecodedFrame;
use crate::windows::video_editor::{drawing_overlay::Rect, model, preview, VideoEditorWindow};

pub(super) struct Captured {
    screenshot: image::RgbaImage,
    content: Rect,
    display_scale: f32,
    expected: DecodedFrame,
    region: (f32, f32, f32, f32),
}

pub(super) fn capture_scaled(
    cx: &mut HeadlessAppContext,
    video: WindowHandle<VideoEditorWindow>,
    project: &Path,
    state: &model::VideoEditorState,
    output: &Path,
    target: &str,
) -> Captured {
    let [first, second] = state.drawing_segments.as_slice() else {
        panic!("expected a rectangle and a redaction");
    };
    let [Annotation::Rectangle {
        x,
        y,
        width,
        height,
        ..
    }] = first.annotations.as_slice()
    else {
        panic!("expected a rectangle");
    };
    let (x, y, width, height) = normalize_rect(*x, *y, *width, *height);
    let [Annotation::Redact {
        x: rx,
        y: ry,
        width: rw,
        height: rh,
        ..
    }] = second.annotations.as_slice()
    else {
        panic!("expected a redaction");
    };
    let (rx, ry, rw, rh) = normalize_rect(*rx, *ry, *rw, *rh);
    assert_eq!(first.canvas_width, second.canvas_width);
    assert_eq!(first.canvas_height, second.canvas_height);
    assert!(rx < x + width && x + width < rx + rw);
    assert!(ry < y + height && y + height < ry + rh);
    let region = (
        (rx / second.canvas_width) as f32,
        (ry / second.canvas_height) as f32,
        (rw / second.canvas_width) as f32,
        (rh / second.canvas_height) as f32,
    );

    let mut source = preview::Source::open(project, state.clone()).unwrap();
    assert_eq!(source.composition_size(), (1280.0, 720.0));
    let composed = source.compose(1.0, Some((640, 360))).unwrap();
    assert_eq!((composed.width(), composed.height()), (640, 360));
    let expected = preview::to_render_image_opaque(&composed).unwrap();
    let mut without = state.clone();
    without.drawing_segments.pop();
    let mut without = preview::Source::open(project, without).unwrap();
    let without =
        preview::to_render_image_opaque(&without.compose(1.0, Some((640, 360))).unwrap()).unwrap();
    let changed = expected
        .as_bytes(0)
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(without.as_bytes(0).unwrap().as_chunks::<4>().0.iter())
        .filter(|(actual, baseline)| {
            (0..3).any(|channel| actual[channel].abs_diff(baseline[channel]) >= 16)
        })
        .count();
    assert!(
        changed > 20,
        "pixelation must change the scaled preview: {changed}"
    );

    cx.update(|cx| {
        video
            .update(cx, |editor, _, cx| {
                editor.begin_scrub();
                editor.set_playhead(1.0, cx);
            })
            .unwrap();
    });
    let started = std::time::Instant::now();
    loop {
        cx.run_until_parked();
        let matches = cx.update(|cx| {
            video
                .read(cx)
                .unwrap()
                .preview_image()
                .is_some_and(|image| image.as_bytes(0) == expected.as_bytes(0))
        });
        if matches {
            break;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "{target} scaled preview did not converge"
        );
        std::thread::yield_now();
    }
    let window = video.into();
    super::settle(cx, window);
    super::capture(
        cx,
        window,
        output,
        &format!("recording-{target}-redaction-scaled-preview"),
    );
    let screenshot = cx.capture_screenshot(window).unwrap();
    let content = cx.update(|cx| video.read(cx).unwrap().preview_content_rect().unwrap());
    let display_scale = cx
        .update_window(window, |_, window, _| window.scale_factor())
        .unwrap();
    let expected = DecodedFrame {
        width: 640,
        height: 360,
        bgra: Arc::from(expected.as_bytes(0).unwrap()),
    };
    let without = DecodedFrame {
        width: 640,
        height: 360,
        bgra: Arc::from(without.as_bytes(0).unwrap()),
    };
    let rendered = super::recording_preview_comparison::compare_at_display_size(
        &screenshot,
        content,
        display_scale,
        &expected,
    );
    eprintln!("fixture {target} scaled preview/composition mean RGB error: {:.3}, max {}, changed pixels {changed}", rendered.mean, rendered.maximum);
    assert!(
        rendered.mean < 3.0,
        "{target} displayed scaled preview differs from composition: {}",
        rendered.mean
    );
    let effect = super::recording_preview_comparison::compare_region_at_display_size(
        &screenshot,
        content,
        display_scale,
        &expected,
        region,
    );
    let absent = super::recording_preview_comparison::compare_region_at_display_size(
        &screenshot,
        content,
        display_scale,
        &without,
        region,
    );
    eprintln!("fixture {target} displayed redaction/composition mean RGB error: {:.3}, without redaction {:.3}", effect.mean, absent.mean);
    assert!(
        effect.mean < 3.0 && absent.mean > effect.mean * 2.0 && absent.mean > 2.0,
        "{target} displayed redaction must match the edited composition: {} vs {}",
        effect.mean,
        absent.mean
    );
    Captured {
        screenshot,
        content,
        display_scale,
        expected,
        region,
    }
}

impl Captured {
    pub(super) fn region(&self) -> (f32, f32, f32, f32) {
        self.region
    }

    pub(super) fn compare_export(&self, frame: &DecodedFrame, target: &str) {
        let comparison = super::recording_preview_comparison::compare(
            &self.screenshot,
            self.content,
            self.display_scale,
            frame,
        );
        let unencoded = super::recording_preview_comparison::compare(
            &self.screenshot,
            self.content,
            self.display_scale,
            &self.expected,
        );
        let region = super::recording_preview_comparison::compare_region_at_display_size(
            &self.screenshot,
            self.content,
            self.display_scale,
            frame,
            self.region,
        );
        eprintln!("fixture {target} scaled displayed preview/export mean RGB error: {:.3}, unencoded {:.3}", comparison.mean, unencoded.mean);
        eprintln!(
            "fixture {target} displayed redaction/export mean RGB error: {:.3}, max {}",
            region.mean, region.maximum
        );
        assert!(
            comparison.mean < 5.0,
            "{target} displayed scaled preview differs from decoded export: {}",
            comparison.mean
        );
        assert!(
            region.mean < 8.0,
            "{target} displayed redaction differs from decoded export: {}",
            region.mean
        );
    }
}
