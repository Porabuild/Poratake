use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, LoadCursorW, RegisterClassExW,
    SetWindowsHookExW, UnhookWindowsHookEx, HCURSOR, HHOOK, IDC_ARROW, KBDLLHOOKSTRUCT,
    MONITORINFOF_PRIMARY, WH_KEYBOARD_LL, WINDOW_EX_STYLE, WM_KEYDOWN, WM_SYSKEYDOWN, WNDCLASSEXW,
    WNDPROC, WS_POPUP,
};

pub const WM_MOUSELEAVE: u32 = 0x02A3;

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
