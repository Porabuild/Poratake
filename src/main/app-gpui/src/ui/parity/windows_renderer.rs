use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use gpui::{
    div, img, point, prelude::*, px, size, Bounds, Context, DevicePixels, Empty, GpuSpecs,
    Platform, PlatformAtlas, PlatformHeadlessRenderer, PlatformWindow, Render, Scene, Size, Window,
    WindowHandle, WindowId, WindowKind, WindowParams,
};
use herogpui::gpui;
use raw_window_handle::RawWindowHandle;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_SIZE};

static BASELINE_PROFILE: OnceLock<String> = OnceLock::new();

pub(super) fn baseline_profile() -> &'static str {
    BASELINE_PROFILE
        .get()
        .expect("the visual renderer is initialized")
}

fn profile_name(specs: &GpuSpecs) -> anyhow::Result<String> {
    let device = specs
        .device_name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("-");
    anyhow::ensure!(
        !device.is_empty(),
        "the renderer has no baseline device name"
    );
    let kind = if specs.is_software_emulated {
        "software"
    } else {
        "hardware"
    };
    Ok(format!("{kind}-{device}"))
}

pub(super) fn create(
    platform: &Rc<dyn Platform>,
) -> anyhow::Result<Option<Box<dyn PlatformHeadlessRenderer>>> {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let handle = WindowHandle::<Empty>::new(WindowId::from(id));
    let window = platform.open_window(
        handle.into(),
        WindowParams {
            bounds: Bounds::new(point(px(0.), px(0.)), size(px(64.), px(64.))),
            titlebar: None,
            kind: WindowKind::PopUp,
            is_movable: false,
            app_owns_titlebar_drag: false,
            is_resizable: false,
            is_minimizable: false,
            focus: false,
            show: false,
            icon: None,
            display_id: None,
            app_id: None,
            window_min_size: None,
        },
    )?;
    if id == 1 {
        let specs = window
            .gpu_specs()
            .ok_or_else(|| anyhow::anyhow!("the visual renderer did not report its device"))?;
        let profile = profile_name(&specs)?;
        println!("Visual renderer: {profile}; {specs:?}");
        let output = std::env::var("PORATAKE_PARITY_OUTPUT")?;
        std::fs::write(
            std::path::Path::new(&output).join("renderer.json"),
            serde_json::to_vec_pretty(&serde_json::json!({"profile": &profile, "gpu": specs}))?,
        )?;
        BASELINE_PROFILE.set(profile).unwrap();
    }
    Ok(Some(Box::new(Renderer { window })))
}

struct Renderer {
    window: Box<dyn PlatformWindow>,
}

impl Renderer {
    fn resize(&mut self, target: Size<DevicePixels>) -> anyhow::Result<()> {
        let handle = self
            .window
            .window_handle()
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            anyhow::bail!("the DirectX renderer requires a Win32 window");
        };
        let hwnd = HWND(handle.hwnd.get() as *mut _);
        unsafe {
            SendMessageW(
                hwnd,
                WM_SIZE,
                Some(WPARAM(0)),
                Some(LPARAM(
                    ((target.height.0 as u32) << 16 | target.width.0 as u32) as isize,
                )),
            );
        }
        Ok(())
    }
}

impl PlatformHeadlessRenderer for Renderer {
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        target: Size<DevicePixels>,
    ) -> anyhow::Result<image::RgbaImage> {
        self.resize(target)?;
        self.window.render_to_image(scene)
    }

    fn render_scene(&mut self, scene: &Scene, target: Size<DevicePixels>) -> anyhow::Result<()> {
        self.resize(target)?;
        self.window.draw(scene);
        Ok(())
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.window.sprite_atlas()
    }
}

pub(super) fn verify_image_sampling(
    cx: &mut super::HeadlessAppContext,
    output: &std::path::Path,
    scale: f32,
) {
    let images = [[64, 80, 240, 255], [255, 255, 255, 255]].map(|color| {
        Arc::new(gpui::RenderImage::new(smallvec::smallvec![
            image::Frame::new(image::RgbaImage::from_pixel(16, 16, image::Rgba(color)))
        ]))
    });
    let probe = cx
        .open_window(size(px(640.), px(360.)), |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|_| ImageSamplingProbe { images })
        })
        .unwrap();
    let probe = probe.into();
    super::settle(cx, probe);
    let bounds = super::measured("atlas-images", 0);
    for (actual, expected) in [
        (bounds.left(), 20.25),
        (bounds.top(), 20.25),
        (bounds.size.width, 275.2),
        (bounds.size.height, 275.2),
    ] {
        assert!(
            (f32::from(actual) - expected).abs() * scale <= 0.501,
            "atlas image metric {actual:?} differs from {expected} at scale {scale}; {bounds:?}"
        );
    }
    let image = cx.capture_screenshot(probe).unwrap();
    image.save(output.join("image-atlas-sampling.png")).unwrap();
    for x in [
        (f32::from(bounds.center().x) * scale).floor() as u32,
        (f32::from(bounds.right()) * scale).floor() as u32 - 3,
    ] {
        for fraction in [0.25, 0.5, 0.75] {
            let y = ((f32::from(bounds.top()) + f32::from(bounds.size.height) * fraction) * scale)
                .floor() as u32;
            let pixel = image.get_pixel(x, y);
            assert!(
                pixel.0[..3]
                    .iter()
                    .zip([240_u8, 80, 64])
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                "an image must not sample neighbouring atlas tiles at ({x}, {y}): {pixel:?}"
            );
        }
    }
    cx.update_window(probe, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    println!("scaled image sampling stayed inside its atlas tile");
}

struct ImageSamplingProbe {
    images: [Arc<gpui::RenderImage>; 2],
}

impl Render for ImageSamplingProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .relative()
            .bg(gpui::black())
            .children(self.images.iter().enumerate().map(|(index, image)| {
                img(image.clone())
                    .absolute()
                    .left(px(20.25 + index as f32 * 320.))
                    .top(px(20.25))
                    .w(px(275.2))
                    .h(px(275.2))
            }))
            .on_children_prepainted(|bounds, _, _| super::record("atlas-images", bounds))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_profiles_are_path_safe_and_separate_software() {
        let mut specs = GpuSpecs {
            device_name: "../../NVIDIA/GeForce RTX 5080".into(),
            ..Default::default()
        };
        assert_eq!(
            profile_name(&specs).unwrap(),
            "hardware-nvidia-geforce-rtx-5080"
        );
        specs.is_software_emulated = true;
        assert_eq!(
            profile_name(&specs).unwrap(),
            "software-nvidia-geforce-rtx-5080"
        );
        specs.device_name = "../..\\".into();
        assert!(profile_name(&specs).is_err());
    }
}
