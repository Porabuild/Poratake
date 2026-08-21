use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_TRANSITIONS_FORCEDISABLED, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreateRoundRectRgn, EnumDisplayMonitors, FONT_CHARSET, FONT_CLIP_PRECISION,
    FONT_OUTPUT_PRECISION, FONT_QUALITY, GetMonitorInfoW, HDC, HFONT, HMONITOR, MONITORINFO,
    MONITORINFOEXW, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, GWL_EXSTYLE, GetClientRect, GetWindowLongPtrW,
    HCURSOR, HHOOK, IDC_ARROW, KBDLLHOOKSTRUCT, LoadCursorW, MONITORINFOF_PRIMARY,
    RegisterClassExW, SetWindowLongPtrW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    WINDOW_EX_STYLE, WM_KEYDOWN, WM_SYSKEYDOWN, WNDCLASSEXW, WNDPROC, WS_EX_NOACTIVATE, WS_POPUP,
};
use windows::core::PCWSTR;

pub const WM_MOUSELEAVE: u32 = 0x02A3;

const CLEARTYPE_QUALITY: u8 = 5;

pub struct MonitorEntry {
    pub handle: isize,
    pub rect: RECT,
    pub is_primary: bool,
    pub device: String,
    pub device_number: i32,
}

pub fn rect_width(rect: &RECT) -> i32 {
    rect.right - rect.left
}

pub fn rect_height(rect: &RECT) -> i32 {
    rect.bottom - rect.top
}

pub fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn scale_for_dpi(value: i32, dpi: u32) -> i32 {
    ((value as i64 * dpi.max(96) as i64) / 96) as i32
}

pub fn point_from_lparam(lparam: LPARAM) -> POINT {
    let value = lparam.0 as u32;
    POINT {
        x: (value as u16 as i16) as i32,
        y: ((value >> 16) as u16 as i16) as i32,
    }
}

pub fn rects_intersect(first: &RECT, second: &RECT) -> bool {
    first.left < second.right
        && first.right > second.left
        && first.top < second.bottom
        && first.bottom > second.top
}

pub fn create_ui_font(dpi: u32, point_size: i32, weight: i32) -> HFONT {
    let face = to_wide("Segoe UI");

    unsafe {
        CreateFontW(
            -scale_for_dpi(point_size, dpi),
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            FONT_CHARSET(0),
            FONT_OUTPUT_PRECISION(0),
            FONT_CLIP_PRECISION(0),
            FONT_QUALITY(CLEARTYPE_QUALITY),
            0,
            PCWSTR(face.as_ptr()),
        )
    }
}

pub fn apply_round_region(window: HWND, radius: i32) {
    let mut client = RECT::default();

    unsafe {
        let _ = GetClientRect(window, &mut client);
        let region = CreateRoundRectRgn(
            0,
            0,
            client.right + 1,
            client.bottom + 1,
            radius * 2,
            radius * 2,
        );
        let _ = SetWindowRgn(window, Some(region), true);
    }
}

pub fn disable_window_transitions(window: HWND) -> windows::core::Result<()> {
    let disabled = windows::core::BOOL(1);

    unsafe {
        DwmSetWindowAttribute(
            window,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            &disabled as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&disabled) as u32,
        )
    }
}

pub fn configure_overlay_window(window: HWND) -> windows::core::Result<()> {
    unsafe {
        let style = GetWindowLongPtrW(window, GWL_EXSTYLE);
        SetWindowLongPtrW(window, GWL_EXSTYLE, style | WS_EX_NOACTIVATE.0 as isize);
    }

    disable_window_transitions(window)
}

pub fn monitors() -> Vec<MonitorEntry> {
    let mut entries: Vec<MonitorEntry> = Vec::new();

    unsafe extern "system" fn enum_proc(
        monitor: HMONITOR,
        _dc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> windows::core::BOOL {
        let entries = unsafe { &mut *(lparam.0 as *mut Vec<MonitorEntry>) };

        let mut info = MONITORINFOEXW {
            monitorInfo: MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
                ..Default::default()
            },
            ..Default::default()
        };

        let ok = unsafe { GetMonitorInfoW(monitor, &mut info.monitorInfo as *mut MONITORINFO) };
        if ok.as_bool() {
            let device: String = String::from_utf16_lossy(&info.szDevice)
                .trim_end_matches('\0')
                .to_string();
            let device_number: i32 = device
                .chars()
                .filter(|character| character.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0);

            entries.push(MonitorEntry {
                handle: monitor.0 as isize,
                rect: info.monitorInfo.rcMonitor,
                is_primary: (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
                device,
                device_number,
            });
        }

        windows::core::BOOL(1)
    }

    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut entries as *mut _ as isize),
        );
    }

    entries
}

thread_local! {
    static REGISTERED_CLASSES: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

pub fn ensure_window_class(name: &str, wndproc: WNDPROC, cursor: Option<HCURSOR>) {
    let already = REGISTERED_CLASSES.with(|classes| classes.borrow().contains(name));
    if already {
        return;
    }

    let wide_name = to_wide(name);
    let instance = unsafe { GetModuleHandleW(None) }.unwrap_or_default();
    let arrow = unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default();

    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: wndproc,
        hInstance: instance.into(),
        hCursor: cursor.unwrap_or(arrow),
        lpszClassName: PCWSTR(wide_name.as_ptr()),
        ..Default::default()
    };

    unsafe {
        RegisterClassExW(&class);
    }

    REGISTERED_CLASSES.with(|classes| {
        classes.borrow_mut().insert(name.to_string());
    });
}

pub fn create_popup_window(class: &str, ex_style: WINDOW_EX_STYLE, rect: &RECT) -> Option<HWND> {
    let wide_class = to_wide(class);
    let instance = unsafe { GetModuleHandleW(None) }.unwrap_or_default();

    unsafe {
        CreateWindowExW(
            ex_style,
            PCWSTR(wide_class.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            rect.left,
            rect.top,
            rect_width(rect),
            rect_height(rect),
            None,
            None,
            Some(instance.into()),
            None,
        )
        .ok()
    }
}

pub fn default_wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

type KeyHandler = Rc<dyn Fn()>;

struct KeyHandlerEntry {
    token: usize,
    virtual_key: u32,
    handler: KeyHandler,
}

thread_local! {
    static KEY_HANDLERS: RefCell<Vec<KeyHandlerEntry>> = RefCell::new(Vec::new());
    static KEYBOARD_HOOK: RefCell<Option<HHOOK>> = RefCell::new(None);
    static NEXT_TOKEN: RefCell<usize> = RefCell::new(1);
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let message = wparam.0 as u32;
        if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
            let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let pressed = info.vkCode;

            let handlers: Vec<KeyHandler> = KEY_HANDLERS.with(|entries| {
                entries
                    .borrow()
                    .iter()
                    .filter(|entry| entry.virtual_key == pressed)
                    .map(|entry| entry.handler.clone())
                    .collect()
            });

            for handler in handlers {
                handler();
            }
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

pub fn add_key_handler(virtual_key: u32, handler: impl Fn() + 'static) -> Result<usize, String> {
    KEYBOARD_HOOK.with(|hook| {
        let mut hook = hook.borrow_mut();
        if hook.is_some() {
            return Ok::<(), String>(());
        }

        let module = unsafe { GetModuleHandleW(None) }
            .map_err(|error| format!("Failed to get module handle: {error}"))?;
        let installed = unsafe {
            SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_hook_proc),
                Some(module.into()),
                0,
            )
        }
        .map_err(|error| format!("Failed to install keyboard hook: {error}"))?;
        *hook = Some(installed);
        Ok(())
    })?;

    let token = NEXT_TOKEN.with(|next| {
        let mut next = next.borrow_mut();
        let token = *next;
        *next += 1;
        token
    });

    KEY_HANDLERS.with(|entries| {
        entries.borrow_mut().push(KeyHandlerEntry {
            token,
            virtual_key,
            handler: Rc::new(handler),
        });
    });

    Ok(token)
}

pub fn remove_key_handler(token: usize) {
    let is_empty = KEY_HANDLERS.with(|entries| {
        let mut entries = entries.borrow_mut();
        entries.retain(|entry| entry.token != token);
        entries.is_empty()
    });

    if !is_empty {
        return;
    }

    KEYBOARD_HOOK.with(|hook| {
        if let Some(installed) = hook.borrow_mut().take() {
            unsafe {
                let _ = UnhookWindowsHookEx(installed);
            }
        }
    });
}
