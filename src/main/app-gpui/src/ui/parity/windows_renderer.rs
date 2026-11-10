use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use gpui::{
    point, px, size, Bounds, DevicePixels, Empty, Platform, PlatformAtlas,
    PlatformHeadlessRenderer, PlatformWindow, Scene, Size, WindowHandle, WindowId, WindowKind,
    WindowParams,
};
use herogpui::gpui;
use raw_window_handle::RawWindowHandle;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_SIZE};

pub(super) fn create(
    platform: &Rc<dyn Platform>,
) -> anyhow::Result<Option<Box<dyn PlatformHeadlessRenderer>>> {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let handle =
        WindowHandle::<Empty>::new(WindowId::from(NEXT_ID.fetch_add(1, Ordering::Relaxed)));
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
