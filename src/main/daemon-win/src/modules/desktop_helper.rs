use crate::protocol::Request;
use crate::router::{Module, Reply, method_not_found};
use serde_json::json;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, SW_HIDE, SW_SHOW, ShowWindow,
};
use windows::core::w;

pub struct DesktopHelperModule;

impl Module for DesktopHelperModule {
    fn name(&self) -> &'static str {
        "desktop-helper"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "hide" => set_icons_visible(false)
                .map(|_| Some(json!({ "hidden": true })))
                .into(),
            "show" => set_icons_visible(true)
                .map(|_| Some(json!({ "hidden": false })))
                .into(),
            method => method_not_found(method),
        }
    }
}

fn set_icons_visible(visible: bool) -> Result<(), (String, String)> {
    let Some(view) = find_desktop_view() else {
        return Err((
            "DESKTOP_VIEW_NOT_FOUND".to_string(),
            "Could not locate the desktop icons window".to_string(),
        ));
    };

    let command = if visible { SW_SHOW } else { SW_HIDE };
    unsafe {
        let _ = ShowWindow(view, command);
    }
    Ok(())
}

fn find_desktop_view() -> Option<HWND> {
    unsafe {
        if let Ok(progman) = FindWindowW(w!("Progman"), None)
            && let Ok(view) = FindWindowExW(Some(progman), None, w!("SHELLDLL_DefView"), None)
        {
            return Some(view);
        }

        let mut worker: Option<HWND> = None;
        loop {
            let next = FindWindowExW(None, worker, w!("WorkerW"), None).ok()?;
            if let Ok(view) = FindWindowExW(Some(next), None, w!("SHELLDLL_DefView"), None) {
                return Some(view);
            }
            worker = Some(next);
        }
    }
}
