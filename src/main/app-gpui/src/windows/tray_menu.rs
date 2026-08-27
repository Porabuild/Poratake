use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, DisplayId, Entity, Pixels, Render,
    Subscription, Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};

use crate::system::native::TrayRect;
use crate::system::tray::TrayMenuState;
use crate::ui::menu::{MenuEntry, MenuView};
use crate::windows::registry::{self, WindowKind as RegistryKind};

const MENU_WIDTH: f32 = 304.0;
const ITEM_HEIGHT: f32 = 28.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const MENU_PADDING: f32 = 8.0;
const SCREEN_GAP: f32 = 8.0;
const MENU_FADE_MS: u32 = 120;
/// Room for the menu's drop shadow, which must stay inside the window surface.
const MENU_SHADOW_MARGIN: f32 = 12.0;
/// Clicking the tray icon while the menu is open first deactivates the window
/// (closing it) and only then delivers the tray click; a toggle arriving this
/// soon after such a close is the same physical click and must not reopen.
const TRAY_CLICK_AFTER_CLOSE_MS: u64 = 250;

static CLOSED_ON_DEACTIVATION: parking_lot::Mutex<Option<std::time::Instant>> =
    parking_lot::Mutex::new(None);

fn recently_closed_on_deactivation() -> bool {
    CLOSED_ON_DEACTIVATION.lock().is_some_and(|closed_at| {
        closed_at.elapsed() < std::time::Duration::from_millis(TRAY_CLICK_AFTER_CLOSE_MS)
    })
}

fn close_tray_menu(window: &mut Window, cx: &mut App) {
    window.remove_window();
    registry::forget(RegistryKind::TrayMenu, cx);
}

struct MenuGeometry {
    bounds: Bounds<Pixels>,
    display_id: Option<DisplayId>,
}

struct MenuDisplay {
    work_area: Bounds<Pixels>,
    tray_rect: Option<TrayRect>,
    id: DisplayId,
}

pub struct TrayMenuWindow {
    menu: Entity<MenuView>,
    activation: Option<Subscription>,
    revealing: bool,
}

impl TrayMenuWindow {
    fn new(menu: Entity<MenuView>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            menu,
            activation: None,
            revealing: cfg!(windows),
        };
        view.activation = Some(cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() && !this.revealing {
                *CLOSED_ON_DEACTIVATION.lock() = Some(std::time::Instant::now());
                close_tray_menu(window, cx);
            }
        }));
        view
    }

    pub fn toggle(tray_rect: Option<TrayRect>, cx: &mut App) {
        if recently_closed_on_deactivation() {
            return;
        }
        registry::toggle(RegistryKind::TrayMenu, cx, |cx| {
            let config = crate::state::state(cx).config.get();
            let state = TrayMenuState::from_config(&config);
            let entries = crate::system::tray::entries(&state, tray_rect);
            let geometry = menu_geometry(tray_rect, &entries, cx);
            let max_height = px(f32::from(geometry.bounds.size.height) - MENU_SHADOW_MARGIN * 2.0);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(geometry.bounds)),
                    titlebar: None,
                    focus: !cfg!(windows),
                    show: !cfg!(windows),
                    kind: WindowKind::PopUp,
                    is_movable: false,
                    is_resizable: false,
                    is_minimizable: false,
                    display_id: geometry.display_id,
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                |window, cx| {
                    configure_tray_menu_window(window);
                    let dismiss = std::rc::Rc::new(|window: &mut Window, cx: &mut App| {
                        close_tray_menu(window, cx);
                    });
                    let menu = cx.new(|cx| {
                        MenuView::new(entries, dismiss, cx)
                            .compact(true)
                            .neutral_highlight(true)
                            .animate_entrance(!cfg!(windows))
                            .min_width(px(MENU_WIDTH))
                            .max_height(max_height)
                    });
                    window.focus(&menu.read(cx).focus_handle());
                    let view = cx.new(|cx| Self::new(menu, window, cx));
                    #[cfg(windows)]
                    {
                        let reveal_view = view.clone();
                        window.on_next_frame(move |window, _cx| {
                            window.on_next_frame(move |window, _cx| {
                                reveal_tray_menu_window(window);
                                window.activate_window();
                                window.on_next_frame(move |window, cx| {
                                    reveal_view.update(cx, |this, _cx| {
                                        this.revealing = false;
                                    });
                                    if !window.is_window_active() {
                                        close_tray_menu(window, cx);
                                    }
                                });
                                window.request_animation_frame();
                            });
                            window.request_animation_frame();
                        });
                        window.request_animation_frame();
                        window.activate_window();
                    }
                    view
                },
            )
            .ok()
            .map(Into::into)
        });
    }
}

impl Render for TrayMenuWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p(px(MENU_SHADOW_MARGIN))
            .child(self.menu.clone())
    }
}

fn configure_tray_menu_window(window: &Window) {
    #[cfg(windows)]
    {
        use windows::Win32::Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CLOAK, DWMWA_COLOR_NONE,
            DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
        };

        let Some(hwnd) = crate::windows::window_hwnd(window) else {
            return;
        };
        let border_color = DWMWA_COLOR_NONE;
        let corner_preference = DWMWCP_DONOTROUND;
        let cloaked = windows::core::BOOL(1);
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &border_color as *const _ as *const core::ffi::c_void,
                std::mem::size_of_val(&border_color) as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner_preference as *const _ as *const core::ffi::c_void,
                std::mem::size_of_val(&corner_preference) as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_CLOAK,
                &cloaked as *const _ as *const core::ffi::c_void,
                std::mem::size_of_val(&cloaked) as u32,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = window;
}

#[cfg(windows)]
fn reveal_tray_menu_window(window: &Window) {
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK};
    use windows::Win32::UI::WindowsAndMessaging::{
        AnimateWindow, ShowWindow, AW_ACTIVATE, AW_BLEND, SW_HIDE, SW_SHOW,
    };

    let Some(hwnd) = crate::windows::window_hwnd(window) else {
        return;
    };
    let cloaked = windows::core::BOOL(0);
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CLOAK,
            &cloaked as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&cloaked) as u32,
        );
        if AnimateWindow(hwnd, MENU_FADE_MS, AW_ACTIVATE | AW_BLEND).is_err() {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
    }
}

fn menu_geometry(tray_rect: Option<TrayRect>, entries: &[MenuEntry], cx: &mut App) -> MenuGeometry {
    let margin = MENU_SHADOW_MARGIN * 2.0;
    let Some(display) = menu_display(tray_rect, cx) else {
        let content_height = menu_height(entries);
        let window_size = size(px(MENU_WIDTH + margin), px(content_height + margin));
        return MenuGeometry {
            bounds: Bounds::centered(None, window_size, cx),
            display_id: None,
        };
    };
    let max_content_height = f32::from(display.work_area.size.height) - SCREEN_GAP * 2.0 - margin;
    let content_height = menu_height(entries).min(max_content_height);
    let window_width = MENU_WIDTH + margin;
    let window_height = content_height + margin;
    let window_size = size(px(window_width), px(window_height));
    let Some(rect) = display.tray_rect else {
        return MenuGeometry {
            bounds: Bounds::centered(Some(display.id), window_size, cx),
            display_id: Some(display.id),
        };
    };
    let min_x = f32::from(display.work_area.left()) + SCREEN_GAP;
    let max_x = f32::from(display.work_area.right()) - window_width - SCREEN_GAP;
    let x = (rect.x + rect.width / 2.0 - window_width / 2.0).clamp(min_x, max_x);
    let space_above = rect.y - f32::from(display.work_area.top());
    let y = if space_above >= window_height + SCREEN_GAP {
        rect.y - window_height - SCREEN_GAP
    } else {
        rect.y + rect.height + SCREEN_GAP
    }
    .clamp(
        f32::from(display.work_area.top()) + SCREEN_GAP,
        f32::from(display.work_area.bottom()) - window_height - SCREEN_GAP,
    );
    MenuGeometry {
        bounds: Bounds {
            origin: gpui::point(px(x), px(y)),
            size: window_size,
        },
        display_id: Some(display.id),
    }
}

#[cfg(windows)]
fn menu_display(tray_rect: Option<TrayRect>, cx: &mut App) -> Option<MenuDisplay> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

    let displays = cx.displays();
    let Some(rect) = tray_rect else {
        let display = displays.first()?;
        return Some(MenuDisplay {
            work_area: crate::system::work_area::work_area(display.bounds()),
            tray_rect: None,
            id: display.id(),
        });
    };
    let point = POINT {
        x: (rect.x + rect.width / 2.0).round() as i32,
        y: (rect.y + rect.height / 2.0).round() as i32,
    };
    let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return None;
    }
    let physical_width = (info.rcMonitor.right - info.rcMonitor.left) as f32;
    let physical_height = (info.rcMonitor.bottom - info.rcMonitor.top) as f32;
    if physical_width <= 0.0 || physical_height <= 0.0 {
        return None;
    }
    let mut dpi_x = 0;
    let mut dpi_y = 0;
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }.ok()?;
    if dpi_x == 0 || dpi_y == 0 {
        return None;
    }
    let expected_x = info.rcMonitor.left as f32 * 96.0 / dpi_x as f32;
    let expected_y = info.rcMonitor.top as f32 * 96.0 / dpi_y as f32;
    let expected_width = physical_width * 96.0 / dpi_x as f32;
    let expected_height = physical_height * 96.0 / dpi_y as f32;
    let display = displays.iter().find(|display| {
        let bounds = display.bounds();
        (f32::from(bounds.origin.x) - expected_x).abs() < 0.5
            && (f32::from(bounds.origin.y) - expected_y).abs() < 0.5
            && (f32::from(bounds.size.width) - expected_width).abs() < 0.5
            && (f32::from(bounds.size.height) - expected_height).abs() < 0.5
    })?;
    let logical_bounds = display.bounds();
    let scale_x = f32::from(logical_bounds.size.width) / physical_width;
    let scale_y = f32::from(logical_bounds.size.height) / physical_height;
    let to_logical_x = |value: f32| {
        f32::from(logical_bounds.origin.x) + (value - info.rcMonitor.left as f32) * scale_x
    };
    let to_logical_y = |value: f32| {
        f32::from(logical_bounds.origin.y) + (value - info.rcMonitor.top as f32) * scale_y
    };
    let tray_rect = physical_to_logical_rect(
        rect,
        info.rcMonitor.left as f32,
        info.rcMonitor.top as f32,
        f32::from(logical_bounds.origin.x),
        f32::from(logical_bounds.origin.y),
        scale_x,
        scale_y,
    );
    Some(MenuDisplay {
        work_area: Bounds {
            origin: gpui::point(
                px(to_logical_x(info.rcWork.left as f32)),
                px(to_logical_y(info.rcWork.top as f32)),
            ),
            size: size(
                px((info.rcWork.right - info.rcWork.left) as f32 * scale_x),
                px((info.rcWork.bottom - info.rcWork.top) as f32 * scale_y),
            ),
        },
        tray_rect: Some(tray_rect),
        id: display.id(),
    })
}

#[cfg(not(windows))]
fn menu_display(tray_rect: Option<TrayRect>, cx: &mut App) -> Option<MenuDisplay> {
    let display = cx.displays().first()?.clone();
    Some(MenuDisplay {
        work_area: crate::system::work_area::work_area(display.bounds()),
        tray_rect,
        id: display.id(),
    })
}

fn menu_height(entries: &[MenuEntry]) -> f32 {
    entries
        .iter()
        .map(|entry| match entry {
            MenuEntry::Separator => SEPARATOR_HEIGHT,
            MenuEntry::Item(_) | MenuEntry::Label(_) => ITEM_HEIGHT,
        })
        .sum::<f32>()
        + MENU_PADDING
}

fn physical_to_logical_rect(
    rect: TrayRect,
    physical_x: f32,
    physical_y: f32,
    logical_x: f32,
    logical_y: f32,
    scale_x: f32,
    scale_y: f32,
) -> TrayRect {
    TrayRect {
        x: logical_x + (rect.x - physical_x) * scale_x,
        y: logical_y + (rect.y - physical_y) * scale_y,
        width: rect.width * scale_x,
        height: rect.height * scale_y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_height_counts_items_and_separators() {
        let entries = vec![
            MenuEntry::Item(crate::ui::menu::MenuItem::new("One")),
            MenuEntry::Separator,
            MenuEntry::Item(crate::ui::menu::MenuItem::new("Two")),
        ];
        assert_eq!(menu_height(&entries), 73.0);
    }

    #[test]
    fn physical_tray_rect_scales_into_the_target_display() {
        let logical = physical_to_logical_rect(
            TrayRect {
                x: 2850.0,
                y: 1416.0,
                width: 24.0,
                height: 24.0,
            },
            1920.0,
            0.0,
            1280.0,
            0.0,
            2.0 / 3.0,
            2.0 / 3.0,
        );
        assert_eq!(logical.x, 1900.0);
        assert_eq!(logical.y, 944.0);
        assert_eq!(logical.width, 16.0);
        assert_eq!(logical.height, 16.0);
    }
}
