use super::media_devices::{enumerate_cameras, enumerate_microphones, MediaDevice};
use crate::overlay::{
    add_key_handler, apply_round_region, create_popup_window, create_ui_font, default_wndproc,
    ensure_window_class, monitors, point_from_lparam, remove_key_handler, scale_for_dpi, to_wide,
    WM_MOUSELEAVE,
};
use crate::panel::{
    button_at, button_fill, button_rect, button_state, draw_label, draw_pill, paint_buffered,
    panel_height, panel_width, ACTIVE_BUTTON, BUTTON_TEXT, BUTTON_TEXT_MUTED, BUTTON_TEXT_ON_FILL,
    NEUTRAL_BUTTON, PANEL_ALPHA, PANEL_BUTTON_RADIUS, PANEL_CORNER_RADIUS, PANEL_FONT_SIZE,
    PANEL_FONT_WEIGHT, PRIMARY_BUTTON,
};
use crate::protocol::{
    param_bool, param_i32, param_str, respond_error, respond_success, send_event, Request,
};
use crate::router::{method_not_found, Module, Reply};
use crate::ui::run_on_ui;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{DeleteObject, InvalidateRect, ScreenToClient, HFONT};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, DestroyWindow, GetCursorPos, GetWindowRect,
    LoadCursorW, MessageBoxW, SetForegroundWindow, SetLayeredWindowAttributes,
    SetWindowDisplayAffinity, SetWindowPos, ShowWindow, TrackPopupMenu, HTCAPTION, HWND_TOPMOST,
    IDC_HAND, LWA_ALPHA, MB_ICONWARNING, MB_YESNO, MF_CHECKED, MF_STRING, SWP_NOACTIVATE,
    SWP_NOMOVE, SW_SHOWNOACTIVATE, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    WDA_EXCLUDEFROMCAPTURE, WM_DPICHANGED, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_NCHITTEST, WM_PAINT, WM_RBUTTONUP, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

const CLASS_NAME: &str = "CaptyRecordingControl";
const ITEM_WIDTH: i32 = 64;
const TIMER_WIDTH: i32 = 68;
const DRAG_WIDTH: i32 = 26;
const CONTROL_TOP_MARGIN: i32 = 24;

#[derive(Clone, Copy, PartialEq)]
enum ControlMode {
    PreRecording,
    Recording,
}

#[derive(Clone)]
struct ControlSettings {
    system_audio: bool,
    mic_enabled: bool,
    mic_muted: bool,
    camera_enabled: bool,
    keyboard_enabled: bool,
    selected_mic_id: Option<String>,
    selected_mic_name: Option<String>,
    selected_camera_id: Option<String>,
    selected_camera_name: Option<String>,
    camera_size: String,
    camera_shape: String,
    camera_flipped: bool,
}

impl Default for ControlSettings {
    fn default() -> Self {
        Self {
            system_audio: true,
            mic_enabled: false,
            mic_muted: false,
            camera_enabled: false,
            keyboard_enabled: false,
            selected_mic_id: None,
            selected_mic_name: None,
            selected_camera_id: None,
            selected_camera_name: None,
            camera_size: "medium".to_string(),
            camera_shape: "circle".to_string(),
            camera_flipped: false,
        }
    }
}

#[derive(Clone)]
struct ControlModel {
    mode: ControlMode,
    elapsed_seconds: i32,
    is_paused: bool,
    is_starting: bool,
    settings: ControlSettings,
    microphones: Vec<MediaDevice>,
    cameras: Vec<MediaDevice>,
}

impl Default for ControlModel {
    fn default() -> Self {
        Self {
            mode: ControlMode::PreRecording,
            elapsed_seconds: 0,
            is_paused: false,
            is_starting: false,
            settings: ControlSettings::default(),
            microphones: Vec::new(),
            cameras: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ControlItem {
    SystemAudio,
    Mic,
    Camera,
    AspectRatio,
    Timer,
    Record,
    Cancel,
    MicMute,
    PauseResume,
    Stop,
    Restart,
    Delete,
    Drag,
}

struct ControlUiState {
    window: Option<HWND>,
    font: Option<HFONT>,
    key_token: Option<usize>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    model: Option<Arc<Mutex<ControlModel>>>,
    visible: Option<Arc<AtomicBool>>,
}

thread_local! {
    static STATE: RefCell<ControlUiState> = const { RefCell::new(ControlUiState {
        window: None,
        font: None,
        key_token: None,
        hovered: None,
        pressed: None,
        model: None,
        visible: None,
    }) };
}

fn control_items(model: &ControlModel) -> Vec<ControlItem> {
    if model.mode == ControlMode::PreRecording {
        return vec![
            ControlItem::SystemAudio,
            ControlItem::Mic,
            ControlItem::Camera,
            ControlItem::AspectRatio,
            ControlItem::Timer,
            ControlItem::Record,
            ControlItem::Cancel,
            ControlItem::Drag,
        ];
    }

    let mut items = Vec::with_capacity(7);
    if model.settings.mic_enabled {
        items.push(ControlItem::MicMute);
    }
    items.extend([
        ControlItem::Timer,
        ControlItem::PauseResume,
        ControlItem::Stop,
        ControlItem::Restart,
        ControlItem::Delete,
        ControlItem::Drag,
    ]);
    items
}

fn item_widths(items: &[ControlItem]) -> Vec<i32> {
    items
        .iter()
        .map(|item| match item {
            ControlItem::Timer => TIMER_WIDTH,
            ControlItem::Drag => DRAG_WIDTH,
            _ => ITEM_WIDTH,
        })
        .collect()
}

fn item_label(item: ControlItem, model: &ControlModel) -> String {
    match item {
        ControlItem::SystemAudio if model.settings.system_audio => "Audio".into(),
        ControlItem::SystemAudio => "No audio".into(),
        ControlItem::Mic if model.settings.mic_enabled => "Mic".into(),
        ControlItem::Mic => "No mic".into(),
        ControlItem::Camera if model.settings.camera_enabled => "Camera".into(),
        ControlItem::Camera => "No cam".into(),
        ControlItem::AspectRatio => "Ratio".into(),
        ControlItem::Timer => {
            let seconds = model.elapsed_seconds.max(0);
            format!("{:02}:{:02}", seconds / 60, seconds % 60)
        }
        ControlItem::Record if model.is_starting => "…".into(),
        ControlItem::Record => "Record".into(),
        ControlItem::Cancel => "Cancel".into(),
        ControlItem::MicMute if model.settings.mic_muted => "Unmute".into(),
        ControlItem::MicMute => "Mute".into(),
        ControlItem::PauseResume if model.is_paused => "Resume".into(),
        ControlItem::PauseResume => "Pause".into(),
        ControlItem::Stop => "Stop".into(),
        ControlItem::Restart => "Restart".into(),
        ControlItem::Delete => "Delete".into(),
        ControlItem::Drag => "≡".into(),
    }
}

fn item_palette(item: ControlItem) -> [COLORREF; 3] {
    match item {
        ControlItem::Record => PRIMARY_BUTTON,
        ControlItem::Stop | ControlItem::Delete => ACTIVE_BUTTON,
        _ => NEUTRAL_BUTTON,
    }
}

fn item_text(item: ControlItem, enabled: bool) -> COLORREF {
    if !enabled {
        return BUTTON_TEXT_MUTED;
    }
    match item {
        ControlItem::Record | ControlItem::Stop | ControlItem::Delete => BUTTON_TEXT_ON_FILL,
        _ => BUTTON_TEXT,
    }
}

fn item_enabled(item: ControlItem, model: &ControlModel) -> bool {
    if item == ControlItem::Timer {
        return false;
    }
    model.mode != ControlMode::PreRecording || !model.is_starting
}

fn window_dpi(window: HWND) -> u32 {
    unsafe { GetDpiForWindow(window) }.max(96)
}

fn model_snapshot() -> Option<ControlModel> {
    let model = STATE.with(|state| state.borrow().model.clone())?;
    let snapshot = model.lock().ok()?.clone();
    Some(snapshot)
}

fn model_width(model: &ControlModel) -> i32 {
    panel_width(&item_widths(&control_items(model)))
}

fn emit(name: &str) {
    send_event(&format!("recording-control:{name}"), None);
}

fn emit_device(name: &str, device: Option<&MediaDevice>) {
    let data = match device {
        Some(device) => json!({ "deviceId": device.id, "deviceName": device.label }),
        None => json!({ "deviceId": null, "deviceName": null }),
    };
    send_event(&format!("recording-control:{name}"), Some(data));
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
        WM_ERASEBKGND => LRESULT(1),
        WM_MOUSEMOVE => {
            track_hover(window, point_from_lparam(lparam));
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            clear_hover(window);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            press_item(window, point_from_lparam(lparam));
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            release_item(window, point_from_lparam(lparam));
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            handle_context_click(window, point_from_lparam(lparam));
            LRESULT(0)
        }
        WM_NCHITTEST => {
            let mut point = point_from_lparam(lparam);
            unsafe {
                let _ = ScreenToClient(window, &mut point);
            }
            if item_at(window, point).map(|(_, item)| item) == Some(ControlItem::Drag) {
                return LRESULT(HTCAPTION as isize);
            }
            default_wndproc(window, message, wparam, lparam)
        }
        WM_DPICHANGED => {
            let suggested = unsafe { &*(lparam.0 as *const RECT) };
            unsafe {
                let _ = SetWindowPos(
                    window,
                    Some(HWND_TOPMOST),
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOACTIVATE,
                );
            }
            recreate_font(window);
            update_region(window);
            LRESULT(0)
        }
        _ => default_wndproc(window, message, wparam, lparam),
    }
}

fn recreate_font(window: HWND) {
    let old_font = STATE.with(|state| state.borrow_mut().font.take());
    if let Some(font) = old_font {
        unsafe {
            let _ = DeleteObject(font.into());
        }
    }

    let font = create_ui_font(window_dpi(window), PANEL_FONT_SIZE, PANEL_FONT_WEIGHT);
    STATE.with(|state| state.borrow_mut().font = Some(font));
}

fn update_region(window: HWND) {
    apply_round_region(
        window,
        scale_for_dpi(PANEL_CORNER_RADIUS, window_dpi(window)),
    );
}

fn item_at(window: HWND, point: POINT) -> Option<(usize, ControlItem)> {
    let model = model_snapshot()?;
    let items = control_items(&model);
    let index = button_at(&item_widths(&items), point, window_dpi(window))?;
    Some((index, items[index]))
}

fn repaint(window: HWND) {
    unsafe {
        let _ = InvalidateRect(Some(window), None, false);
    }
}

fn track_hover(window: HWND, point: POINT) {
    let hovered = item_at(window, point).map(|(index, _)| index);
    let changed = STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.hovered == hovered {
            return false;
        }
        state.hovered = hovered;
        true
    });

    let mut tracking = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE,
        hwndTrack: window,
        dwHoverTime: 0,
    };
    unsafe {
        let _ = TrackMouseEvent(&mut tracking);
    }

    if changed {
        repaint(window);
    }
}

fn clear_hover(window: HWND) {
    let changed = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.pressed = None;
        state.hovered.take().is_some()
    });

    if changed {
        repaint(window);
    }
}

fn press_item(window: HWND, point: POINT) {
    let pressed = item_at(window, point).map(|(index, _)| index);
    STATE.with(|state| state.borrow_mut().pressed = pressed);

    if pressed.is_some() {
        repaint(window);
    }
}

fn release_item(window: HWND, point: POINT) {
    let pressed = STATE.with(|state| state.borrow_mut().pressed.take());
    let Some(pressed) = pressed else {
        return;
    };

    repaint(window);

    let Some((index, item)) = item_at(window, point) else {
        return;
    };
    if index != pressed {
        return;
    }

    handle_click(window, item);
}

fn paint(window: HWND) {
    let Some(model) = model_snapshot() else {
        return;
    };
    let (font, hovered, pressed) = STATE.with(|state| {
        let state = state.borrow();
        (state.font, state.hovered, state.pressed)
    });
    let dpi = window_dpi(window);
    let items = control_items(&model);
    let widths = item_widths(&items);
    let radius = scale_for_dpi(PANEL_BUTTON_RADIUS, dpi);

    paint_buffered(window, font, |dc, _| {
        for (index, item) in items.iter().enumerate() {
            let rect = button_rect(&widths, index, dpi);
            let label = item_label(*item, &model);
            let enabled = item_enabled(*item, &model);

            if *item == ControlItem::Timer {
                draw_label(dc, &label, rect, BUTTON_TEXT);
                continue;
            }

            let state = button_state(
                enabled && hovered == Some(index),
                enabled && pressed == Some(index),
            );
            draw_pill(dc, rect, radius, button_fill(item_palette(*item), state));
            draw_label(dc, &label, rect, item_text(*item, enabled));
        }
    });
}

fn append_menu_item(
    menu: windows::Win32::UI::WindowsAndMessaging::HMENU,
    id: u32,
    label: &str,
    checked: bool,
) {
    let label = to_wide(label);
    let flags = if checked {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING
    };
    unsafe {
        let _ = AppendMenuW(menu, flags, id as usize, PCWSTR(label.as_ptr()));
    }
}

fn popup_command(
    window: HWND,
    build: impl FnOnce(windows::Win32::UI::WindowsAndMessaging::HMENU),
) -> u32 {
    let Ok(menu) = (unsafe { CreatePopupMenu() }) else {
        return 0;
    };
    build(menu);
    let mut point = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut point);
        let _ = SetForegroundWindow(window);
    }
    let command = unsafe {
        TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            Some(0),
            window,
            None,
        )
    };
    unsafe {
        let _ = DestroyMenu(menu);
    }
    command.0 as u32
}

fn device_menu_item_checked(
    enabled: bool,
    selected: Option<&str>,
    device_id: Option<&str>,
) -> bool {
    match device_id {
        Some(device_id) => enabled && selected == Some(device_id),
        None => !enabled,
    }
}

fn show_device_menu(
    window: HWND,
    kind: &str,
    devices: &[MediaDevice],
    enabled: bool,
    selected: Option<&str>,
) {
    let command = popup_command(window, |menu| {
        append_menu_item(
            menu,
            1,
            "None",
            device_menu_item_checked(enabled, selected, None),
        );
        for (index, device) in devices.iter().enumerate() {
            append_menu_item(
                menu,
                index as u32 + 10,
                &device.label,
                device_menu_item_checked(enabled, selected, Some(device.id.as_str())),
            );
        }
    });

    if command == 0 {
        return;
    }
    if command == 1 {
        emit_device(kind, None);
        return;
    }
    let Some(device) = devices.get(command.saturating_sub(10) as usize) else {
        return;
    };
    emit_device(kind, Some(device));
}

fn show_aspect_menu(window: HWND) {
    let ratios = [
        ("Free", 0, 0),
        ("16:9", 16, 9),
        ("9:16", 9, 16),
        ("4:3", 4, 3),
        ("1:1", 1, 1),
        ("21:9", 21, 9),
        ("4:5", 4, 5),
        ("3:2", 3, 2),
    ];
    let command = popup_command(window, |menu| {
        for (index, ratio) in ratios.iter().enumerate() {
            append_menu_item(menu, index as u32 + 10, ratio.0, false);
        }
    });
    let Some((name, width, height)) = ratios.get(command.saturating_sub(10) as usize) else {
        return;
    };
    send_event(
        "recording-control:select-aspect-ratio",
        Some(json!({ "name": name, "width": width, "height": height })),
    );
}

fn confirm(window: HWND, title: &str, message: &str) -> bool {
    let title = to_wide(title);
    let message = to_wide(message);
    let answer = unsafe {
        MessageBoxW(
            Some(window),
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_YESNO | MB_ICONWARNING,
        )
    };
    answer.0 == 6
}

fn toggle_mic_mute(window: HWND) {
    STATE.with(|state| {
        let shared = state.borrow().model.clone();
        if let Some(shared) = shared {
            if let Ok(mut current) = shared.lock() {
                current.settings.mic_muted = !current.settings.mic_muted;
            }
        }
    });
    repaint(window);
    emit("toggle-mic-mute");
}

fn handle_click(window: HWND, item: ControlItem) {
    let Some(model) = model_snapshot() else {
        return;
    };
    if !item_enabled(item, &model) {
        return;
    }

    match item {
        ControlItem::SystemAudio => emit("toggle-system-audio"),
        ControlItem::Mic => emit("toggle-mic"),
        ControlItem::Camera => emit("toggle-camera"),
        ControlItem::AspectRatio => show_aspect_menu(window),
        ControlItem::Record => emit("start"),
        ControlItem::Cancel => emit("cancel"),
        ControlItem::MicMute => toggle_mic_mute(window),
        ControlItem::PauseResume => emit(if model.is_paused { "resume" } else { "pause" }),
        ControlItem::Stop => emit("stop"),
        ControlItem::Restart => {
            if confirm(
                window,
                "Restart Recording?",
                "Discard the current recording and restart?",
            ) {
                emit("restart");
            }
        }
        ControlItem::Delete => {
            if confirm(window, "Delete Recording?", "Delete the current recording?") {
                emit("delete");
            }
        }
        ControlItem::Timer | ControlItem::Drag => {}
    }
}

fn handle_context_click(window: HWND, point: POINT) {
    let Some(model) = model_snapshot() else {
        return;
    };
    if model.mode != ControlMode::PreRecording || model.is_starting {
        return;
    }

    match item_at(window, point).map(|(_, item)| item) {
        Some(ControlItem::Mic) => show_device_menu(
            window,
            "select-mic",
            &model.microphones,
            model.settings.mic_enabled,
            model.settings.selected_mic_id.as_deref(),
        ),
        Some(ControlItem::Camera) => show_device_menu(
            window,
            "select-camera",
            &model.cameras,
            model.settings.camera_enabled,
            model.settings.selected_camera_id.as_deref(),
        ),
        _ => {}
    }
}

fn cancel_from_escape() {
    let pre_recording = model_snapshot()
        .map(|model| model.mode == ControlMode::PreRecording && !model.is_starting)
        .unwrap_or(false);
    if pre_recording {
        emit("cancel");
    }
}

fn refresh_escape_handler() -> bool {
    let old = STATE.with(|state| state.borrow_mut().key_token.take());
    if let Some(token) = old {
        remove_key_handler(token);
    }
    let pre_recording = model_snapshot()
        .map(|model| model.mode == ControlMode::PreRecording && !model.is_starting)
        .unwrap_or(false);
    if !pre_recording {
        return true;
    }
    let Ok(token) = add_key_handler(VK_ESCAPE.0 as u32, cancel_from_escape) else {
        return false;
    };
    STATE.with(|state| state.borrow_mut().key_token = Some(token));
    true
}

fn top_center_in_rect(rect: &RECT, width: i32, dpi: u32) -> (i32, i32) {
    let width = scale_for_dpi(width, dpi);
    (
        rect.left + (rect.right - rect.left - width) / 2,
        rect.top + scale_for_dpi(CONTROL_TOP_MARGIN, dpi),
    )
}

fn top_center(model: &ControlModel, dpi: u32) -> (i32, i32) {
    let monitor = monitors()
        .into_iter()
        .find(|monitor| monitor.is_primary)
        .or_else(|| monitors().into_iter().next());
    let Some(monitor) = monitor else {
        return (100, 100);
    };
    top_center_in_rect(&monitor.work_rect, model_width(model), dpi)
}

fn top_center_for_window(window: HWND, model: &ControlModel, dpi: u32) -> (i32, i32) {
    let mut window_rect = RECT::default();
    if unsafe { GetWindowRect(window, &mut window_rect) }.is_ok() {
        let center_x = window_rect.left + (window_rect.right - window_rect.left) / 2;
        let center_y = window_rect.top + (window_rect.bottom - window_rect.top) / 2;
        if let Some(monitor) = monitors().into_iter().find(|monitor| {
            center_x >= monitor.rect.left
                && center_x < monitor.rect.right
                && center_y >= monitor.rect.top
                && center_y < monitor.rect.bottom
        }) {
            return top_center_in_rect(&monitor.work_rect, model_width(model), dpi);
        }
    }
    top_center(model, dpi)
}

fn resize_window(window: HWND, x: i32, y: i32, move_window: bool) {
    let Some(model) = model_snapshot() else {
        return;
    };
    let dpi = unsafe { GetDpiForWindow(window) }.max(96);
    let flags = if move_window {
        SWP_NOACTIVATE
    } else {
        SWP_NOACTIVATE | SWP_NOMOVE
    };
    unsafe {
        let _ = SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            x,
            y,
            scale_for_dpi(model_width(&model), dpi),
            scale_for_dpi(panel_height(), dpi),
            flags,
        );
        let _ = InvalidateRect(Some(window), None, false);
    }
    update_region(window);
}

fn show_panel(
    model: Arc<Mutex<ControlModel>>,
    visible: Arc<AtomicBool>,
    position: Option<(i32, i32)>,
) -> bool {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.model = Some(model);
        state.visible = Some(visible.clone());
    });

    if let Some(window) = STATE.with(|state| state.borrow().window) {
        let dpi = unsafe { GetDpiForWindow(window) }.max(96);
        let (x, y) = position.unwrap_or_else(|| {
            model_snapshot()
                .map(|model| top_center(&model, dpi))
                .unwrap_or((100, 100))
        });
        resize_window(window, x, y, true);
        if unsafe { SetWindowDisplayAffinity(window, WDA_EXCLUDEFROMCAPTURE) }.is_err() {
            teardown();
            return false;
        }
        unsafe {
            let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        }
        if !refresh_escape_handler() {
            teardown();
            return false;
        }
        visible.store(true, Ordering::Release);
        return true;
    }

    let hand = unsafe { LoadCursorW(None, IDC_HAND) }.ok();
    ensure_window_class(CLASS_NAME, Some(wndproc), hand);
    let (x, y) = position.unwrap_or((100, 100));
    let rect = RECT {
        left: x,
        top: y,
        right: x + panel_width(&item_widths(&control_items(&ControlModel::default()))),
        bottom: y + panel_height(),
    };
    let styles = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED;
    let Some(window) = create_popup_window(CLASS_NAME, styles, &rect) else {
        visible.store(false, Ordering::Release);
        return false;
    };

    STATE.with(|state| state.borrow_mut().window = Some(window));
    recreate_font(window);
    let dpi = unsafe { GetDpiForWindow(window) }.max(96);
    let (x, y) = position.unwrap_or_else(|| {
        model_snapshot()
            .map(|model| top_center(&model, dpi))
            .unwrap_or((100, 100))
    });
    resize_window(window, x, y, true);
    unsafe {
        let _ = SetLayeredWindowAttributes(window, COLORREF(0), PANEL_ALPHA, LWA_ALPHA);
    }
    if unsafe { SetWindowDisplayAffinity(window, WDA_EXCLUDEFROMCAPTURE) }.is_err() {
        teardown();
        return false;
    }
    unsafe {
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
    }
    if !refresh_escape_handler() {
        teardown();
        return false;
    }
    visible.store(true, Ordering::Release);
    true
}

fn refresh_panel(reposition: bool) -> bool {
    let Some(window) = STATE.with(|state| state.borrow().window) else {
        return true;
    };
    let position = if reposition {
        let dpi = unsafe { GetDpiForWindow(window) }.max(96);
        model_snapshot().map(|model| top_center_for_window(window, &model, dpi))
    } else {
        None
    };
    let (x, y) = position.unwrap_or((0, 0));
    resize_window(window, x, y, reposition);
    if !refresh_escape_handler() {
        teardown();
        return false;
    }
    true
}

fn update_position(x: i32, y: i32) {
    let Some(window) = STATE.with(|state| state.borrow().window) else {
        return;
    };
    unsafe {
        let _ = SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            x,
            y,
            0,
            0,
            SWP_NOACTIVATE | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE,
        );
    }
}

fn teardown() {
    let (window, font, key_token, visible) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let result = (
            state.window.take(),
            state.font.take(),
            state.key_token.take(),
            state.visible.take(),
        );
        state.model = None;
        result
    });
    if let Some(window) = window {
        unsafe {
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
    if let Some(visible) = visible {
        visible.store(false, Ordering::Release);
    }
}

fn apply_settings(settings: &mut ControlSettings, value: &Value) {
    let Some(object) = value.as_object() else {
        return;
    };
    settings.system_audio = object
        .get("systemAudio")
        .and_then(Value::as_bool)
        .unwrap_or(settings.system_audio);
    settings.mic_enabled = object
        .get("micEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(settings.mic_enabled);
    settings.mic_muted = object
        .get("micMuted")
        .and_then(Value::as_bool)
        .unwrap_or(settings.mic_muted);
    settings.camera_enabled = object
        .get("cameraEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(settings.camera_enabled);
    settings.keyboard_enabled = object
        .get("keyboardEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(settings.keyboard_enabled);
    settings.camera_flipped = object
        .get("cameraFlipped")
        .and_then(Value::as_bool)
        .unwrap_or(settings.camera_flipped);

    update_optional_string(object.get("selectedMicId"), &mut settings.selected_mic_id);
    update_optional_string(
        object.get("selectedMicName"),
        &mut settings.selected_mic_name,
    );
    update_optional_string(
        object.get("selectedCameraId"),
        &mut settings.selected_camera_id,
    );
    update_optional_string(
        object.get("selectedCameraName"),
        &mut settings.selected_camera_name,
    );
    if let Some(value) = object.get("cameraSize").and_then(Value::as_str) {
        settings.camera_size = value.to_string();
    }
    if let Some(value) = object.get("cameraShape").and_then(Value::as_str) {
        settings.camera_shape = value.to_string();
    }
}

fn update_optional_string(value: Option<&Value>, target: &mut Option<String>) {
    let Some(value) = value else {
        return;
    };
    *target = value.as_str().map(str::to_string);
}

fn parse_devices(value: Option<&Value>) -> Vec<MediaDevice> {
    let Some(array) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|value| {
            Some(MediaDevice {
                id: value.get("id")?.as_str()?.to_string(),
                label: value.get("label")?.as_str()?.to_string(),
            })
        })
        .collect()
}

pub struct RecordingControlModule {
    model: Arc<Mutex<ControlModel>>,
    visible: Arc<AtomicBool>,
}

impl RecordingControlModule {
    pub fn new() -> Self {
        Self {
            model: Arc::new(Mutex::new(ControlModel::default())),
            visible: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Module for RecordingControlModule {
    fn name(&self) -> &'static str {
        "recording-control"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "show" => {
                let x = param_i32(&request.params, "x").unwrap_or(100);
                let y = param_i32(&request.params, "y").unwrap_or(100);
                let microphones = enumerate_microphones().unwrap_or_default();
                let cameras = enumerate_cameras().unwrap_or_default();
                if let Ok(mut model) = self.model.lock() {
                    model.microphones = microphones;
                    model.cameras = cameras;
                    model.mode = if param_str(&request.params, "mode") == Some("recording") {
                        ControlMode::Recording
                    } else {
                        ControlMode::PreRecording
                    };
                    if let Some(settings) = request
                        .params
                        .as_ref()
                        .and_then(|params| params.get("settings"))
                    {
                        apply_settings(&mut model.settings, settings);
                    }
                }
                let request_id = request.id.clone();
                let model = self.model.clone();
                let visible = self.visible.clone();
                run_on_ui(move || {
                    if show_panel(model, visible, Some((x, y))) {
                        respond_success(&request_id, json!({ "visible": true }));
                        return;
                    }
                    respond_error(
                        &request_id,
                        "WINDOW_ERROR",
                        "Failed to show recording control window",
                    );
                });
                Reply::Deferred
            }
            "hide" => {
                let request_id = request.id.clone();
                run_on_ui(move || {
                    teardown();
                    respond_success(&request_id, json!({ "visible": false }));
                });
                if let Ok(mut model) = self.model.lock() {
                    model.elapsed_seconds = 0;
                    model.is_paused = false;
                    model.is_starting = false;
                }
                Reply::Deferred
            }
            "update" => {
                let Some(x) = param_i32(&request.params, "x") else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".into(),
                        "update requires x, y".into(),
                    )));
                };
                let Some(y) = param_i32(&request.params, "y") else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".into(),
                        "update requires x, y".into(),
                    )));
                };
                let request_id = request.id.clone();
                run_on_ui(move || {
                    update_position(x, y);
                    respond_success(&request_id, json!({ "updated": true }));
                });
                Reply::Deferred
            }
            "setMode" => {
                let Some(mode) = param_str(&request.params, "mode") else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".into(),
                        "setMode requires mode".into(),
                    )));
                };
                let mode_name = mode.to_string();
                if let Ok(mut model) = self.model.lock() {
                    model.mode = if mode == "recording" {
                        ControlMode::Recording
                    } else {
                        ControlMode::PreRecording
                    };
                    if model.mode == ControlMode::Recording {
                        model.is_starting = false;
                    }
                }
                let request_id = request.id.clone();
                let model = self.model.clone();
                let visible = self.visible.clone();
                run_on_ui(move || {
                    let shown = if let Some(window) = STATE.with(|state| state.borrow().window) {
                        if unsafe { SetWindowDisplayAffinity(window, WDA_EXCLUDEFROMCAPTURE) }
                            .is_err()
                        {
                            teardown();
                            false
                        } else {
                            refresh_panel(true)
                        }
                    } else {
                        show_panel(model, visible, None)
                    };
                    if shown {
                        respond_success(&request_id, json!({ "mode": mode_name }));
                        return;
                    }
                    respond_error(
                        &request_id,
                        "WINDOW_ERROR",
                        "Failed to show recording control window",
                    );
                });
                Reply::Deferred
            }
            "updateTimer" => {
                let Some(seconds) = param_i32(&request.params, "seconds") else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".into(),
                        "updateTimer requires seconds".into(),
                    )));
                };
                if let Ok(mut model) = self.model.lock() {
                    model.elapsed_seconds = seconds;
                }
                let request_id = request.id.clone();
                run_on_ui(move || {
                    if !refresh_panel(false) {
                        respond_error(
                            &request_id,
                            "WINDOW_ERROR",
                            "Failed to update recording control window",
                        );
                        return;
                    }
                    respond_success(&request_id, json!({ "updated": true }));
                });
                Reply::Deferred
            }
            "updateState" => {
                if let Ok(mut model) = self.model.lock() {
                    model.is_paused = param_bool(&request.params, "isPaused").unwrap_or(false);
                    if let Some(starting) = param_bool(&request.params, "isStarting") {
                        model.is_starting = starting;
                    }
                }
                let request_id = request.id.clone();
                run_on_ui(move || {
                    if !refresh_panel(false) {
                        respond_error(
                            &request_id,
                            "WINDOW_ERROR",
                            "Failed to update recording control window",
                        );
                        return;
                    }
                    respond_success(&request_id, json!({ "updated": true }));
                });
                Reply::Deferred
            }
            "updateSettings" => {
                if let Ok(mut model) = self.model.lock() {
                    let value = request
                        .params
                        .as_ref()
                        .map(|params| json!(params))
                        .unwrap_or(Value::Null);
                    apply_settings(&mut model.settings, &value);
                }
                let request_id = request.id.clone();
                run_on_ui(move || {
                    if !refresh_panel(false) {
                        respond_error(
                            &request_id,
                            "WINDOW_ERROR",
                            "Failed to update recording control window",
                        );
                        return;
                    }
                    respond_success(&request_id, json!({ "updated": true }));
                });
                Reply::Deferred
            }
            "updateDevices" => {
                if let Ok(mut model) = self.model.lock() {
                    let params = request.params.as_ref();
                    if let Some(params) = params {
                        if params.contains_key("microphones") {
                            model.microphones = parse_devices(params.get("microphones"));
                        }
                        if params.contains_key("cameras") {
                            model.cameras = parse_devices(params.get("cameras"));
                        }
                    }
                }
                Reply::Now(Ok(Some(json!({ "updated": true }))))
            }
            "status" => {
                let model = self
                    .model
                    .lock()
                    .ok()
                    .map(|model| model.clone())
                    .unwrap_or_default();
                Reply::Now(Ok(Some(json!({
                    "visible": self.visible.load(Ordering::Acquire),
                    "mode": if model.mode == ControlMode::Recording { "recording" } else { "pre-recording" },
                    "isPaused": model.is_paused,
                    "elapsedSeconds": model.elapsed_seconds,
                }))))
            }
            method => method_not_found(method),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> RECT {
        RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        }
    }

    #[test]
    fn top_center_scales_geometry_at_96_dpi() {
        assert_eq!(top_center_in_rect(&screen(), 500, 96), (710, 24));
    }

    #[test]
    fn top_center_scales_geometry_at_192_dpi() {
        assert_eq!(top_center_in_rect(&screen(), 500, 192), (460, 48));
    }

    #[test]
    fn recording_row_drops_the_pre_recording_controls() {
        let mut model = ControlModel::default();
        assert_eq!(
            control_items(&model).first(),
            Some(&ControlItem::SystemAudio)
        );
        assert!(control_items(&model).contains(&ControlItem::Record));

        model.mode = ControlMode::Recording;
        let items = control_items(&model);
        assert!(!items.contains(&ControlItem::MicMute));
        assert_eq!(items.first(), Some(&ControlItem::Timer));
        assert_eq!(items.last(), Some(&ControlItem::Drag));

        model.settings.mic_enabled = true;
        let items = control_items(&model);
        assert_eq!(items.first(), Some(&ControlItem::MicMute));
        assert!(model_width(&model) > panel_width(&[ITEM_WIDTH]));
    }

    #[test]
    fn starting_state_disables_every_pre_recording_button() {
        let mut model = ControlModel::default();
        model.is_starting = true;

        assert!(!item_enabled(ControlItem::Record, &model));
        assert!(!item_enabled(ControlItem::Timer, &model));
        assert_eq!(item_label(ControlItem::Record, &model), "…");

        model.is_starting = false;
        assert!(item_enabled(ControlItem::Record, &model));
        assert!(!item_enabled(ControlItem::Timer, &model));
        assert_eq!(item_label(ControlItem::Record, &model), "Record");
    }

    #[test]
    fn device_menu_checkmarks_follow_enabled_state() {
        assert!(device_menu_item_checked(false, Some("mic-1"), None));
        assert!(!device_menu_item_checked(
            false,
            Some("mic-1"),
            Some("mic-1")
        ));
        assert!(!device_menu_item_checked(true, None, None));
        assert!(device_menu_item_checked(true, Some("mic-1"), Some("mic-1")));
    }
}
