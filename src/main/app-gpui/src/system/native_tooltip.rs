#[cfg(test)]
use std::cell::RefCell;
use std::ffi::c_void;

use gpui::{point, px, size, Bounds, Pixels, Window};
use herogpui::components::{position_tooltip_popup, TooltipPlacement, TooltipPresentationRequest};
use herogpui::gpui;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_WIN95_CLASSES, INITCOMMONCONTROLSEX, TOOLTIPS_CLASSW, TTF_ABSOLUTE,
    TTF_TRACK, TTM_ADDTOOLW, TTM_DELTOOLW, TTM_POP, TTM_TRACKACTIVATE, TTM_UPDATETIPTEXTW,
    TTS_ALWAYSTIP, TTS_NOPREFIX, TTTOOLINFOW,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetWindowRect, IsWindow, SendMessageW,
    SetLayeredWindowAttributes, SetWindowDisplayAffinity, SetWindowPos, HWND_TOPMOST, LWA_ALPHA,
    SWP_NOACTIVATE, SWP_NOOWNERZORDER, WDA_EXCLUDEFROMCAPTURE, WDA_MONITOR,
    WINDOW_DISPLAY_AFFINITY, WINDOW_EX_STYLE, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

const TOOL_ID: usize = 1;
const MONITOR_GAP: i32 = 8;

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TooltipRequest {
    Show(&'static str, String),
    Hide(&'static str),
}

#[cfg(test)]
thread_local! {
    static TOOLTIP_REQUESTS: RefCell<Vec<TooltipRequest>> = const { RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(crate) fn record_test_show(owner: &'static str, text: &str) {
    TOOLTIP_REQUESTS.with(|requests| {
        let mut requests = requests.borrow_mut();
        requests.push(TooltipRequest::Show(owner, text.to_owned()));
    });
}

#[cfg(test)]
pub(crate) fn record_test_hide(owner: &'static str) {
    TOOLTIP_REQUESTS.with(|requests| requests.borrow_mut().push(TooltipRequest::Hide(owner)));
}

#[cfg(test)]
pub(crate) fn take_test_requests() -> Vec<TooltipRequest> {
    TOOLTIP_REQUESTS.with(|requests| std::mem::take(&mut *requests.borrow_mut()))
}

pub struct WindowsTooltip {
    window: HWND,
    owner: HWND,
    text: Box<[u16]>,
    active: bool,
}

impl WindowsTooltip {
    pub fn new(window: &Window) -> Option<Self> {
        let owner = window_hwnd(window)?;
        Self::new_for_owner(owner)
    }

    fn new_for_owner(owner: HWND) -> Option<Self> {
        let controls = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_WIN95_CLASSES,
        };
        if !unsafe { InitCommonControlsEx(&controls) }.as_bool() {
            return None;
        }
        let tooltip = unsafe {
            CreateWindowExW(
                tooltip_ex_style(),
                TOOLTIPS_CLASSW,
                PCWSTR::null(),
                WS_POPUP
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                        TTS_ALWAYSTIP | TTS_NOPREFIX,
                    ),
                0,
                0,
                0,
                0,
                Some(owner),
                None,
                None,
                None,
            )
            .ok()?
        };
        if let Err(error) = unsafe {
            SetLayeredWindowAttributes(
                tooltip,
                windows::Win32::Foundation::COLORREF(0),
                255,
                LWA_ALPHA,
            )
        } {
            eprintln!("[native-tooltip] failed to configure layered window: {error}");
            unsafe {
                let _ = DestroyWindow(tooltip);
            }
            return None;
        }
        let affinity = capture_affinity_with(|affinity| unsafe {
            SetWindowDisplayAffinity(tooltip, affinity).is_ok()
        });
        if affinity.is_none() {
            eprintln!("[native-tooltip] both display-affinity modes failed; tooltip suppressed");
            unsafe {
                let _ = DestroyWindow(tooltip);
            }
            return None;
        }
        if affinity == Some(WDA_MONITOR) {
            eprintln!("[native-tooltip] WDA_EXCLUDEFROMCAPTURE failed; using WDA_MONITOR fallback");
        }
        let mut result = Self {
            window: tooltip,
            owner,
            text: vec![0].into_boxed_slice(),
            active: false,
        };
        let mut info = result.tool_info();
        let added = unsafe {
            SendMessageW(
                result.window,
                TTM_ADDTOOLW,
                Some(WPARAM(0)),
                Some(LPARAM(
                    (&mut info as *mut TTTOOLINFOW).cast::<c_void>() as isize
                )),
            )
        };
        if added.0 == 0 {
            eprintln!("[native-tooltip] failed to register tooltip tool");
            return None;
        }
        Some(result)
    }

    pub fn show(&mut self, window: &Window, request: &TooltipPresentationRequest) {
        if request.content.is_empty() || window_hwnd(window) != Some(self.owner) {
            self.hide();
            return;
        }
        if !unsafe { IsWindow(Some(self.owner)) }.as_bool() {
            self.hide();
            return;
        }
        self.show_at(
            &request.content,
            request.trigger_screen_bounds,
            request.placement,
            request.offset,
            request.scale_factor,
        );
    }

    fn show_at(
        &mut self,
        text: &str,
        trigger: Bounds<Pixels>,
        placement: TooltipPlacement,
        offset: Pixels,
        scale: f32,
    ) {
        if text.is_empty() || !unsafe { IsWindow(Some(self.owner)) }.as_bool() {
            self.hide();
            return;
        }
        self.hide();
        self.text = text
            .encode_utf16()
            .chain([0])
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut info = self.tool_info();
        unsafe {
            SendMessageW(
                self.window,
                TTM_UPDATETIPTEXTW,
                Some(WPARAM(0)),
                Some(LPARAM(
                    (&mut info as *mut TTTOOLINFOW).cast::<c_void>() as isize
                )),
            );
        }
        let center = trigger.center();
        let cursor = POINT {
            x: (center.x.as_f32() * scale).round() as i32,
            y: (center.y.as_f32() * scale).round() as i32,
        };
        let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if monitor.0.is_null() || !unsafe { GetMonitorInfoW(monitor, &mut monitor_info) }.as_bool()
        {
            return;
        }
        let dpi = unsafe { GetDpiForWindow(self.owner) };
        if dpi == 0 {
            return;
        }
        unsafe {
            SendMessageW(
                self.window,
                windows::Win32::UI::Controls::TTM_SETMAXTIPWIDTH,
                Some(WPARAM(0)),
                Some(LPARAM(
                    (monitor_info.rcWork.right
                        - monitor_info.rcWork.left
                        - scale_for_dpi(MONITOR_GAP, dpi) * 2)
                        .max(1) as isize,
                )),
            );
            SendMessageW(
                self.window,
                TTM_TRACKACTIVATE,
                Some(WPARAM(1)),
                Some(LPARAM(
                    (&mut info as *mut TTTOOLINFOW).cast::<c_void>() as isize
                )),
            );
        }
        self.active = true;
        let mut bounds = RECT::default();
        if unsafe { GetWindowRect(self.window, &mut bounds) }.is_err() {
            self.hide();
            return;
        }
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        let work = monitor_info.rcWork;
        let origin = position_tooltip_popup(
            trigger,
            size(px(width as f32 / scale), px(height as f32 / scale)),
            Bounds {
                origin: point(px(work.left as f32 / scale), px(work.top as f32 / scale)),
                size: size(
                    px((work.right - work.left) as f32 / scale),
                    px((work.bottom - work.top) as f32 / scale),
                ),
            },
            placement,
            offset,
            scale,
        );
        if unsafe {
            SetWindowPos(
                self.window,
                Some(HWND_TOPMOST),
                (origin.x.as_f32() * scale).round() as i32,
                (origin.y.as_f32() * scale).round() as i32,
                width,
                height,
                SWP_NOACTIVATE | SWP_NOOWNERZORDER,
            )
        }
        .is_err()
        {
            self.hide();
        }
        #[cfg(feature = "parity")]
        if self.active {
            self.write_parity_paint_proof();
        }
    }

    #[cfg(feature = "parity")]
    fn write_parity_paint_proof(&self) {
        use std::path::PathBuf;
        use windows::Win32::Graphics::Gdi::{
            BitBlt, ClientToScreen, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC,
            DeleteObject, GetDC, GetPixel, ReleaseDC, SelectObject, UpdateWindow, BLACKNESS,
            HGDIOBJ,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            GetClientRect, GetWindowDisplayAffinity, PRF_CLIENT, WM_PRINTCLIENT,
        };

        let Some(path) = std::env::var_os("PORATAKE_NATIVE_TOOLTIP_PARITY_PROOF") else {
            return;
        };
        let path = PathBuf::from(path);
        let result = (|| -> Result<(image::RgbaImage, serde_json::Value), String> {
            let mut tooltip_bounds = RECT::default();
            unsafe { GetWindowRect(self.window, &mut tooltip_bounds) }
                .map_err(|error| error.to_string())?;
            let mut owner_bounds = RECT::default();
            unsafe { GetWindowRect(self.owner, &mut owner_bounds) }
                .map_err(|error| error.to_string())?;
            let mut client = RECT::default();
            unsafe { GetClientRect(self.window, &mut client) }
                .map_err(|error| error.to_string())?;
            let width = client.right - client.left;
            let height = client.bottom - client.top;
            if width <= 0 || height <= 0 {
                return Err("Tooltip has no client pixels to inspect".into());
            }
            let mut client_origin = POINT { x: 0, y: 0 };
            if !unsafe { ClientToScreen(self.window, &mut client_origin) }.as_bool() {
                return Err("Could not locate the tooltip client origin".into());
            }
            let mut affinity = 0;
            unsafe { GetWindowDisplayAffinity(self.window, &mut affinity) }
                .map_err(|error| error.to_string())?;

            let screen = unsafe { GetDC(None) };
            if screen.0.is_null() {
                return Err("Could not get the screen device context".into());
            }
            let memory = unsafe { CreateCompatibleDC(Some(screen)) };
            if memory.0.is_null() {
                unsafe { ReleaseDC(None, screen) };
                return Err("Could not create the tooltip proof device context".into());
            }
            let bitmap = unsafe { CreateCompatibleBitmap(screen, width, height) };
            if bitmap.0.is_null() {
                unsafe {
                    let _ = DeleteDC(memory);
                    ReleaseDC(None, screen);
                }
                return Err("Could not create the tooltip proof bitmap".into());
            }
            let bitmap_object = HGDIOBJ(bitmap.0);
            let previous = unsafe { SelectObject(memory, bitmap_object) };
            if previous.0.is_null() {
                unsafe {
                    let _ = DeleteObject(bitmap_object);
                    let _ = DeleteDC(memory);
                    ReleaseDC(None, screen);
                }
                return Err("Could not select the tooltip proof bitmap".into());
            }

            let mut proof_image = image::RgbaImage::new(width as u32, height as u32);
            let mut paint_error = None;
            unsafe {
                if let Err(error) = BitBlt(memory, 0, 0, width, height, None, 0, 0, BLACKNESS) {
                    paint_error = Some(error.to_string());
                }
                let _ = UpdateWindow(self.window);
                if paint_error.is_none() {
                    SendMessageW(
                        self.window,
                        WM_PRINTCLIENT,
                        Some(WPARAM(memory.0 as usize)),
                        Some(LPARAM(PRF_CLIENT as isize)),
                    );
                }
            }
            if paint_error.is_none() {
                'pixels: for y in 0..height {
                    for x in 0..width {
                        let color = unsafe { GetPixel(memory, x, y) }.0;
                        if color == u32::MAX {
                            paint_error = Some(format!("Tooltip pixel {x},{y} could not be read"));
                            break 'pixels;
                        }
                        proof_image.put_pixel(
                            x as u32,
                            y as u32,
                            image::Rgba([
                                color as u8,
                                (color >> 8) as u8,
                                (color >> 16) as u8,
                                255,
                            ]),
                        );
                    }
                }
            }
            unsafe {
                let _ = SelectObject(memory, previous);
                let _ = DeleteObject(bitmap_object);
                let _ = DeleteDC(memory);
                ReleaseDC(None, screen);
            }
            if let Some(error) = paint_error {
                return Err(error);
            }
            let text = String::from_utf16_lossy(&self.text[..self.text.len().saturating_sub(1)]);
            let proof = serde_json::json!({
                "tooltipHwnd": self.window.0 as isize,
                "ownerHwnd": self.owner.0 as isize,
                "text": text,
                "affinity": affinity,
                "tooltipBounds": {
                    "left": tooltip_bounds.left,
                    "top": tooltip_bounds.top,
                    "right": tooltip_bounds.right,
                    "bottom": tooltip_bounds.bottom
                },
                "ownerBounds": {
                    "left": owner_bounds.left,
                    "top": owner_bounds.top,
                    "right": owner_bounds.right,
                    "bottom": owner_bounds.bottom
                },
                "clientBounds": {
                    "screenLeft": client_origin.x,
                    "screenTop": client_origin.y,
                    "width": width,
                    "height": height
                }
            });
            Ok((proof_image, proof))
        })();

        match result {
            Ok((proof_image, proof)) => {
                if let Err(error) = proof_image.save(&path) {
                    eprintln!("[native-tooltip] failed to save parity paint proof: {error}");
                    return;
                }
                let proof_path = path.with_extension("json");
                match serde_json::to_vec(&proof) {
                    Ok(bytes) => {
                        if let Err(error) = std::fs::write(proof_path, bytes) {
                            eprintln!(
                                "[native-tooltip] failed to write parity paint metadata: {error}"
                            );
                        }
                    }
                    Err(error) => eprintln!(
                        "[native-tooltip] failed to encode parity paint metadata: {error}"
                    ),
                }
            }
            Err(error) => eprintln!("[native-tooltip] parity paint proof failed: {error}"),
        }
    }

    pub fn hide(&mut self) {
        if !std::mem::replace(&mut self.active, false) {
            return;
        }
        if unsafe { IsWindow(Some(self.window)) }.as_bool() {
            let mut info = self.tool_info();
            unsafe {
                SendMessageW(
                    self.window,
                    TTM_TRACKACTIVATE,
                    Some(WPARAM(0)),
                    Some(LPARAM(
                        (&mut info as *mut TTTOOLINFOW).cast::<c_void>() as isize
                    )),
                );
                SendMessageW(self.window, TTM_POP, None, None);
            }
        }
    }

    fn tool_info(&mut self) -> TTTOOLINFOW {
        TTTOOLINFOW {
            cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
            uFlags: TTF_TRACK | TTF_ABSOLUTE,
            hwnd: self.owner,
            uId: TOOL_ID,
            lpszText: PWSTR(self.text.as_mut_ptr()),
            ..Default::default()
        }
    }
}

impl Drop for WindowsTooltip {
    fn drop(&mut self) {
        self.hide();
        if unsafe { IsWindow(Some(self.window)) }.as_bool() {
            let mut info = self.tool_info();
            unsafe {
                SendMessageW(
                    self.window,
                    TTM_DELTOOLW,
                    Some(WPARAM(0)),
                    Some(LPARAM(
                        (&mut info as *mut TTTOOLINFOW).cast::<c_void>() as isize
                    )),
                );
                let _ = DestroyWindow(self.window);
            }
        }
    }
}

fn tooltip_ex_style() -> WINDOW_EX_STYLE {
    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT
}

fn window_hwnd(window: &Window) -> Option<HWND> {
    crate::windows::window_hwnd(window)
}

fn scale_for_dpi(value: i32, dpi: u32) -> i32 {
    ((i64::from(value) * i64::from(dpi) + 48) / 96) as i32
}

fn capture_affinity_with(
    mut set: impl FnMut(WINDOW_DISPLAY_AFFINITY) -> bool,
) -> Option<WINDOW_DISPLAY_AFFINITY> {
    if set(WDA_EXCLUDEFROMCAPTURE) {
        return Some(WDA_EXCLUDEFROMCAPTURE);
    }
    set(WDA_MONITOR).then_some(WDA_MONITOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct HeadlessView;

    impl gpui::Render for HeadlessView {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            gpui::div()
        }
    }

    #[herogpui::test]
    fn headless_windows_do_not_create_native_tooltip_windows(cx: &mut gpui::TestAppContext) {
        let (_view, cx) = herogpui::test::open_window(cx, |_, _| HeadlessView);
        assert!(cx.update(|window, _| WindowsTooltip::new(window)).is_none());
    }

    #[test]
    fn tooltip_text_is_null_terminated_and_stably_owned() {
        let text = "Camera settings"
            .encode_utf16()
            .chain([0])
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let pointer = text.as_ptr();
        assert_eq!(text.last(), Some(&0));
        assert_eq!(
            String::from_utf16(&text[..text.len() - 1]).unwrap(),
            "Camera settings"
        );
        assert_eq!(pointer, text.as_ptr());
    }

    #[test]
    fn tooltip_offsets_follow_window_dpi() {
        assert_eq!(scale_for_dpi(MONITOR_GAP, 144), 12);
    }

    #[test]
    fn capture_affinity_uses_monitor_fallback_and_never_unprotected_mode() {
        let mut requested = Vec::new();
        let fallback = capture_affinity_with(|affinity| {
            requested.push(affinity);
            affinity == WDA_MONITOR
        });
        assert_eq!(requested, [WDA_EXCLUDEFROMCAPTURE, WDA_MONITOR]);
        assert_eq!(fallback, Some(WDA_MONITOR));

        assert_eq!(capture_affinity_with(|_| false), None);
    }

    #[test]
    fn native_tooltip_window_has_capture_affinity_and_raii_ownership() {
        use crate::capture::overlay::windows_build;
        use windows::core::w;
        use windows::Win32::Foundation::HINSTANCE;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, GetWindowDisplayAffinity, GetWindowLongPtrW, IsWindowVisible,
            GWL_EXSTYLE, WDA_MONITOR, WS_POPUP,
        };

        let owner = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                160,
                52,
                None,
                None,
                Some(HINSTANCE::default()),
                None,
            )
        }
        .unwrap();
        let tooltip = WindowsTooltip::new_for_owner(owner).unwrap();
        let tooltip_hwnd = tooltip.window;
        let ex_style = unsafe { GetWindowLongPtrW(tooltip_hwnd, GWL_EXSTYLE) } as u32;
        assert_eq!(ex_style & tooltip_ex_style().0, tooltip_ex_style().0);
        let mut affinity = 0;
        assert!(unsafe { GetWindowDisplayAffinity(tooltip_hwnd, &mut affinity) }.is_ok());
        if windows_build() < 19041 {
            assert_eq!(affinity, WDA_MONITOR.0);
        } else {
            assert_eq!(affinity, WDA_EXCLUDEFROMCAPTURE.0);
        }
        assert!(!unsafe { IsWindowVisible(tooltip_hwnd) }.as_bool());
        drop(tooltip);
        assert!(!unsafe { IsWindow(Some(tooltip_hwnd)) }.as_bool());
        unsafe {
            let _ = DestroyWindow(owner);
        }
    }

    #[test]
    fn native_tooltip_shows_on_an_isolated_desktop() {
        use std::collections::HashSet;
        use std::thread;
        use windows::core::{w, PCWSTR};
        use windows::Win32::Foundation::{HINSTANCE, POINT};
        use windows::Win32::Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
            GetMonitorInfoW, GetPixel, MonitorFromPoint, ReleaseDC, SelectObject, UpdateWindow,
            BLACKNESS, HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
        };
        use windows::Win32::System::StationsAndDesktops::{
            CloseDesktop, CreateDesktopW, SetThreadDesktop, DESKTOP_CONTROL_FLAGS,
            DESKTOP_CREATEWINDOW, DESKTOP_READOBJECTS, DESKTOP_WRITEOBJECTS, HDESK,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, GetClientRect, GetForegroundWindow,
            GetWindowDisplayAffinity, IsWindowVisible, SetWindowDisplayAffinity, SetWindowPos,
            ShowWindow, HWND_TOP, PRF_CLIENT, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE,
            WM_PRINTCLIENT, WS_POPUP,
        };

        let desktop_name = format!("PoratakeNativeTooltipTest{}", std::process::id());
        let desktop_name = desktop_name.encode_utf16().chain([0]).collect::<Vec<_>>();
        let desktop = unsafe {
            CreateDesktopW(
                PCWSTR(desktop_name.as_ptr()),
                PCWSTR::null(),
                None,
                DESKTOP_CONTROL_FLAGS(0),
                DESKTOP_CREATEWINDOW.0 | DESKTOP_READOBJECTS.0 | DESKTOP_WRITEOBJECTS.0,
                None,
            )
        }
        .unwrap();
        let desktop_handle = desktop.0 as usize;
        let test = thread::spawn(move || {
            unsafe { SetThreadDesktop(HDESK(desktop_handle as *mut c_void)) }.unwrap();
            let monitor =
                unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
            let mut monitor_info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            assert!(unsafe { GetMonitorInfoW(monitor, &mut monitor_info) }.as_bool());
            let owner_bounds = RECT {
                left: monitor_info.rcMonitor.left + 100,
                top: monitor_info.rcMonitor.top + 20,
                right: monitor_info.rcMonitor.left + 420,
                bottom: monitor_info.rcMonitor.top + 72,
            };
            let owner = unsafe {
                CreateWindowExW(
                    WS_EX_NOACTIVATE,
                    w!("STATIC"),
                    PCWSTR::null(),
                    WS_POPUP,
                    owner_bounds.left,
                    owner_bounds.top,
                    owner_bounds.right - owner_bounds.left,
                    owner_bounds.bottom - owner_bounds.top,
                    None,
                    None,
                    Some(HINSTANCE::default()),
                    None,
                )
            }
            .unwrap();
            unsafe {
                SetWindowPos(
                    owner,
                    Some(HWND_TOP),
                    owner_bounds.left,
                    owner_bounds.top,
                    owner_bounds.right - owner_bounds.left,
                    owner_bounds.bottom - owner_bounds.top,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )
                .unwrap();
                let _ = ShowWindow(owner, SW_SHOWNOACTIVATE);
            }
            let mut tooltip = WindowsTooltip::new_for_owner(owner).unwrap();
            let foreground = unsafe { GetForegroundWindow() };
            let scale = unsafe { GetDpiForWindow(owner) } as f32 / 96.;
            tooltip.show_at(
                "Camera settings",
                Bounds {
                    origin: point(
                        px(owner_bounds.left as f32 / scale),
                        px(owner_bounds.top as f32 / scale),
                    ),
                    size: size(
                        px((owner_bounds.right - owner_bounds.left) as f32 / scale),
                        px((owner_bounds.bottom - owner_bounds.top) as f32 / scale),
                    ),
                },
                TooltipPlacement::Bottom,
                px(8.),
                scale,
            );
            assert!(tooltip.active);
            assert!(unsafe { IsWindowVisible(tooltip.window) }.as_bool());
            assert_eq!(unsafe { GetForegroundWindow() }, foreground);
            let mut tooltip_bounds = RECT::default();
            assert!(unsafe { GetWindowRect(tooltip.window, &mut tooltip_bounds) }.is_ok());
            assert!(tooltip_bounds.left >= monitor_info.rcMonitor.left);
            assert!(tooltip_bounds.top >= monitor_info.rcMonitor.top);
            assert!(tooltip_bounds.right <= monitor_info.rcMonitor.right);
            assert!(tooltip_bounds.bottom <= monitor_info.rcMonitor.bottom);
            assert!(
                tooltip_bounds.bottom <= owner_bounds.top
                    || tooltip_bounds.top >= owner_bounds.bottom
            );
            let mut client = RECT::default();
            unsafe { GetClientRect(tooltip.window, &mut client) }.unwrap();
            let width = client.right - client.left;
            let height = client.bottom - client.top;
            let screen = unsafe { GetDC(None) };
            assert!(!screen.0.is_null());
            let memory = unsafe { CreateCompatibleDC(Some(screen)) };
            assert!(!memory.0.is_null());
            let bitmap = unsafe { CreateCompatibleBitmap(screen, width, height) };
            assert!(!bitmap.0.is_null());
            let previous = unsafe { SelectObject(memory, HGDIOBJ(bitmap.0)) };
            assert!(!previous.0.is_null());
            unsafe {
                BitBlt(memory, 0, 0, width, height, None, 0, 0, BLACKNESS).unwrap();
                let _ = UpdateWindow(tooltip.window);
                SendMessageW(
                    tooltip.window,
                    WM_PRINTCLIENT,
                    Some(WPARAM(memory.0 as usize)),
                    Some(LPARAM(PRF_CLIENT as isize)),
                );
            }
            let mut colors = HashSet::new();
            let mut pixels = image::RgbaImage::new(width as u32, height as u32);
            for y in 0..height {
                for x in 0..width {
                    let color = unsafe { GetPixel(memory, x, y) }.0;
                    assert_ne!(color, u32::MAX, "tooltip pixels must be readable");
                    colors.insert(color);
                    pixels.put_pixel(
                        x as u32,
                        y as u32,
                        image::Rgba([color as u8, (color >> 8) as u8, (color >> 16) as u8, 255]),
                    );
                }
            }
            assert!(
                colors.len() > 2,
                "native tooltip content must paint: {colors:?}"
            );
            eprintln!(
                "isolated native tooltip WM_PRINTCLIENT produced {} distinct pixel colors",
                colors.len()
            );
            if let Some(path) = std::env::var_os("PORATAKE_NATIVE_TOOLTIP_PROOF") {
                pixels.save(path).unwrap();
            }
            unsafe {
                SelectObject(memory, previous);
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
                let _ = DeleteDC(memory);
                ReleaseDC(None, screen);
            }
            tooltip.hide();
            assert!(!unsafe { IsWindowVisible(tooltip.window) }.as_bool());
            unsafe { SetWindowDisplayAffinity(tooltip.window, WDA_MONITOR) }.unwrap();
            let mut fallback_affinity = 0;
            unsafe { GetWindowDisplayAffinity(tooltip.window, &mut fallback_affinity) }.unwrap();
            assert_eq!(fallback_affinity, WDA_MONITOR.0);
            drop(tooltip);
            unsafe { DestroyWindow(owner) }.unwrap();
        })
        .join();
        unsafe { CloseDesktop(desktop) }.unwrap();
        test.unwrap();
    }
}
