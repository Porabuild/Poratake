use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use gpui::{
    point, px, size, Bounds, DevicePixels, Empty, GpuSpecs, Platform, PlatformAtlas,
    PlatformHeadlessRenderer, PlatformWindow, Scene, Size, WindowHandle, WindowId, WindowKind,
    WindowParams,
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
