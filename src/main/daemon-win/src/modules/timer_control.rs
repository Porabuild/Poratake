use crate::overlay::{
    add_key_handler, create_popup_window, default_wndproc, ensure_window_class, remove_key_handler,
    scale_for_dpi, to_wide,
};
use crate::panel::parse_color;
use crate::protocol::{Request, param_i32, param_str, respond_error, respond_success, send_event};
use crate::router::{Module, Reply, method_not_found};
use crate::ui::run_on_ui;
use serde_json::json;
use std::cell::RefCell;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateRoundRectRgn, CreateSolidBrush, DT_CENTER, DT_SINGLELINE,
    DT_VCENTER, DeleteObject, DrawTextW, EndPaint, FONT_CHARSET, FONT_CLIP_PRECISION,
    FONT_OUTPUT_PRECISION, FONT_QUALITY, FW_SEMIBOLD, FillRect, HFONT, InvalidateRect, PAINTSTRUCT,
    SelectObject, SetBkMode, SetTextColor, SetWindowRgn, TRANSPARENT,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, HWND_TOPMOST, KillTimer, LWA_ALPHA, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SWP_NOSIZE, SetLayeredWindowAttributes, SetTimer, SetWindowDisplayAffinity, SetWindowPos,
    ShowWindow, WDA_EXCLUDEFROMCAPTURE, WM_LBUTTONDOWN, WM_PAINT, WM_TIMER, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};
use windows::core::PCWSTR;

const CLASS_NAME: &str = "PoratakeTimerControl";
const PANEL_WIDTH: i32 = 140;
const PANEL_HEIGHT: i32 = 52;
const CORNER_RADIUS: i32 = 10;
const COUNTDOWN_TIMER_ID: usize = 1;

struct TimerUiState {
    window: Option<HWND>,
    font: Option<HFONT>,
    remaining: i32,
    background: COLORREF,
    foreground: COLORREF,
    key_token: Option<usize>,
}

thread_local! {
    static STATE: RefCell<TimerUiState> = const { RefCell::new(TimerUiState {
        window: None,
        font: None,
        remaining: 0,
        background: COLORREF(0),
        foreground: COLORREF(0),
        key_token: None,
    }) };
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            paint(window);
            LRESULT(0)
        }
        WM_TIMER => {
            handle_tick();
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            cancel();
            LRESULT(0)
        }
        _ => default_wndproc(window, message, wparam, lparam),
    }
}

fn paint(window: HWND) {
    let (font, remaining, background, foreground) = STATE.with(|state| {
        let state = state.borrow();
        (
            state.font,
            state.remaining,
            state.background,
            state.foreground,
        )
    });

    unsafe {
        let mut paint_struct = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint_struct);

        let background_brush = CreateSolidBrush(background);
        FillRect(dc, &paint_struct.rcPaint, background_brush);
        let _ = DeleteObject(background_brush.into());

        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, foreground);

        let previous_font = font.map(|font| SelectObject(dc, font.into()));

        let text = to_wide(&format!("{remaining}s"));
        let mut rect = paint_struct.rcPaint;
        let mut buffer: Vec<u16> = text[..text.len() - 1].to_vec();
        DrawTextW(
            dc,
            &mut buffer,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        if let Some(previous) = previous_font {
            SelectObject(dc, previous);
        }

        let _ = EndPaint(window, &paint_struct);
    }
}

fn handle_tick() {
    let remaining = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.remaining -= 1;
        state.remaining
    });

    if remaining > 0 {
        let window = STATE.with(|state| state.borrow().window);
        if let Some(window) = window {
            unsafe {
                let _ = InvalidateRect(Some(window), None, true);
            }
        }
        return;
    }

    teardown();
    send_event("timer-control:completed", None);
}

fn cancel() {
    let was_visible = STATE.with(|state| state.borrow().window.is_some());
    if !was_visible {
        return;
    }

    teardown();
    send_event("timer-control:cancel", None);
}

fn show_panel(
    x: i32,
    y: i32,
    duration: i32,
    background: COLORREF,
    foreground: COLORREF,
) -> Result<(), String> {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.remaining = duration;
        state.background = background;
        state.foreground = foreground;
    });

    let existing = STATE.with(|state| state.borrow().window);
    if let Some(window) = existing {
        unsafe {
            let _ = SetWindowPos(
                window,
                Some(HWND_TOPMOST),
                x,
                y,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOSIZE,
            );
            if SetTimer(Some(window), COUNTDOWN_TIMER_ID, 1000, None) == 0 {
                teardown();
                return Err("Failed to start the countdown timer".to_string());
            }
            let _ = InvalidateRect(Some(window), None, true);
        }
        return Ok(());
    }

    ensure_window_class(CLASS_NAME, Some(wndproc), None);

    let nominal = RECT {
        left: x,
        top: y,
        right: x + PANEL_WIDTH,
        bottom: y + PANEL_HEIGHT,
    };

    let ex_style = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED;
    let Some(window) = create_popup_window(CLASS_NAME, ex_style, &nominal) else {
        teardown();
        return Err("Failed to create the timer control window".to_string());
    };

    let dpi = unsafe { GetDpiForWindow(window) }.max(96);
    let width = scale_for_dpi(PANEL_WIDTH, dpi);
    let height = scale_for_dpi(PANEL_HEIGHT, dpi);
    let radius = scale_for_dpi(CORNER_RADIUS, dpi) * 2;

    unsafe {
        let _ = SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE,
        );

        let _ = SetWindowDisplayAffinity(window, WDA_EXCLUDEFROMCAPTURE);

        let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, radius, radius);
        let _ = SetWindowRgn(window, Some(region), true);

        let _ = SetLayeredWindowAttributes(window, COLORREF(0), 255, LWA_ALPHA);

        let font_name = to_wide("Segoe UI");
        let font = CreateFontW(
            -scale_for_dpi(22, dpi),
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            FONT_CHARSET(0),
            FONT_OUTPUT_PRECISION(0),
            FONT_CLIP_PRECISION(0),
            FONT_QUALITY(0),
            0,
            PCWSTR(font_name.as_ptr()),
        );

        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.window = Some(window);
            state.font = Some(font);
        });

        if SetTimer(Some(window), COUNTDOWN_TIMER_ID, 1000, None) == 0 {
            teardown();
            return Err("Failed to start the countdown timer".to_string());
        }
    }

    let token = match add_key_handler(VK_ESCAPE.0 as u32, cancel) {
        Ok(token) => token,
        Err(message) => {
            teardown();
            return Err(message);
        }
    };
    STATE.with(|state| {
        state.borrow_mut().key_token = Some(token);
    });
    unsafe {
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
    }
    Ok(())
}

fn teardown() {
    let (window, font, key_token) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        (
            state.window.take(),
            state.font.take(),
            state.key_token.take(),
        )
    });

    if let Some(window) = window {
        unsafe {
            let _ = KillTimer(Some(window), COUNTDOWN_TIMER_ID);
            let _ = DestroyWindow(window);
        }
    }

    if let Some(font) = font {
        unsafe {
            let _ = DeleteObject(font.into());
        }
    }

    if let Some(token) = key_token {
        remove_key_handler(token);
    }
}

pub struct TimerControlModule;

impl Module for TimerControlModule {
    fn name(&self) -> &'static str {
        "timer-control"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "show" => {
                let x = param_i32(&request.params, "x").unwrap_or(100);
                let y = param_i32(&request.params, "y").unwrap_or(100);
                let duration = param_i32(&request.params, "duration").unwrap_or(5);
                let (Some(background), Some(foreground)) = (
                    parse_color(param_str(&request.params, "color")),
                    parse_color(param_str(&request.params, "foregroundColor")),
                ) else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "show requires theme colors".to_string(),
                    )));
                };
                let request_id = request.id.clone();

                run_on_ui(move || {
                    match show_panel(x, y, duration.max(1), background, foreground) {
                        Ok(()) => respond_success(&request_id, json!({ "visible": true })),
                        Err(message) => respond_error(&request_id, "UI_ERROR", &message),
                    }
                });

                Reply::Deferred
            }
            "hide" => {
                let request_id = request.id.clone();

                run_on_ui(move || {
                    teardown();
                    respond_success(&request_id, json!({ "visible": false }));
                });

                Reply::Deferred
            }
            method => method_not_found(method),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    fn show_request(params: Option<HashMap<String, serde_json::Value>>) -> Request {
        Request {
            id: "timer-theme".to_string(),
            module: "timer-control".to_string(),
            method: "show".to_string(),
            params,
        }
    }

    #[test]
    fn show_rejects_missing_or_invalid_theme_colors() {
        let cases = [
            None,
            Some(HashMap::from([
                ("color".to_string(), json!("invalid")),
                ("foregroundColor".to_string(), json!("#ffffff")),
            ])),
            Some(HashMap::from([
                ("color".to_string(), json!("#000000")),
                ("foregroundColor".to_string(), json!("invalid")),
            ])),
        ];

        for params in cases {
            let mut module = TimerControlModule;
            let Reply::Now(result) = module.handle(&show_request(params)) else {
                panic!("invalid theme colors should respond immediately");
            };
            assert_eq!(
                result,
                Err((
                    "INVALID_PARAMS".to_string(),
                    "show requires theme colors".to_string(),
                ))
            );
        }
    }
}
