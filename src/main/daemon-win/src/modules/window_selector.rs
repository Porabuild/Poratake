use crate::overlay::{rect_height, rect_width};
use crate::protocol::Request;
use crate::router::{Module, Reply, method_not_found};
use serde_json::json;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GetClassNameW, GetWindowLongW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, WS_EX_TOOLWINDOW,
};

const MIN_WINDOW_SIZE: i32 = 50;
const EXCLUDED_CLASSES: [&str; 4] = [
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
];

#[derive(Clone)]
struct TargetWindow {
    window_id: isize,
    title: String,
    owner_name: String,
    owner_pid: u32,
    rect: RECT,
}

fn window_process_name(pid: u32) -> String {
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::from("Unknown");
        };

        let mut buffer = [0u16; 1024];
        let mut length = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut length,
        );
        let _ = windows::Win32::Foundation::CloseHandle(process);

        if result.is_err() {
            return String::from("Unknown");
        }

        let full_path = String::from_utf16_lossy(&buffer[..length as usize]);
        std::path::Path::new(&full_path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| String::from("Unknown"))
    }
}

fn window_class_name(window: HWND) -> String {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetClassNameW(window, &mut buffer) };
    if length <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..length as usize])
}

fn window_title(window: HWND) -> String {
    let mut buffer = [0u16; 512];
    let length = unsafe { GetWindowTextW(window, &mut buffer) };
    if length <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..length as usize])
}

fn is_cloaked(window: HWND) -> bool {
    let mut cloaked: u32 = 0;
    let result = unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        )
    };
    result.is_ok() && cloaked != 0
}

pub fn window_bounds(window: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    let result = unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
    };
    if result.is_err() {
        return None;
    }
    Some(rect)
}

fn collect_target_windows() -> Vec<TargetWindow> {
    let mut targets: Vec<TargetWindow> = Vec::new();

    unsafe extern "system" fn enum_proc(window: HWND, lparam: LPARAM) -> windows::core::BOOL {
        let targets = unsafe { &mut *(lparam.0 as *mut Vec<TargetWindow>) };
        let keep_enumerating = windows::core::BOOL(1);

        if !unsafe { IsWindowVisible(window) }.as_bool() {
            return keep_enumerating;
        }
        if unsafe { IsIconic(window) }.as_bool() {
            return keep_enumerating;
        }

        let ex_style = unsafe { GetWindowLongW(window, GWL_EXSTYLE) } as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return keep_enumerating;
        }
        if is_cloaked(window) {
            return keep_enumerating;
        }

        let class_name = window_class_name(window);
        if EXCLUDED_CLASSES.contains(&class_name.as_str()) {
            return keep_enumerating;
        }

        let Some(rect) = window_bounds(window) else {
            return keep_enumerating;
        };
        if rect_width(&rect) < MIN_WINDOW_SIZE || rect_height(&rect) < MIN_WINDOW_SIZE {
            return keep_enumerating;
        }

        let mut pid: u32 = 0;
        unsafe {
            GetWindowThreadProcessId(window, Some(&mut pid));
        }
        if pid == 0 || pid == unsafe { GetCurrentProcessId() } {
            return keep_enumerating;
        }

        let owner_name = window_process_name(pid);
        let raw_title = window_title(window);
        let title = if raw_title.is_empty() {
            owner_name.clone()
        } else {
            raw_title
        };

        targets.push(TargetWindow {
            window_id: window.0 as isize,
            title,
            owner_name,
            owner_pid: pid,
            rect,
        });
        keep_enumerating
    }

    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut targets as *mut _ as isize));
    }
    targets
}

pub struct WindowSelectorModule;

impl WindowSelectorModule {
    pub fn new() -> Self {
        Self
    }
}

impl Module for WindowSelectorModule {
    fn name(&self) -> &'static str {
        "window-selector"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "list" => {
                let windows: Vec<serde_json::Value> = collect_target_windows()
                    .iter()
                    .map(|target| {
                        json!({
                            "windowId": target.window_id as i64,
                            "title": target.title.as_str(),
                            "ownerName": target.owner_name.as_str(),
                            "ownerPid": target.owner_pid,
                            "bounds": {
                                "x": target.rect.left,
                                "y": target.rect.top,
                                "width": rect_width(&target.rect),
                                "height": rect_height(&target.rect),
                            },
                        })
                    })
                    .collect();
                Reply::Now(Ok(Some(json!({ "windows": windows }))))
            }
            method => method_not_found(method),
        }
    }
}
