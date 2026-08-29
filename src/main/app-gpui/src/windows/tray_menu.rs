use std::cell::RefCell;
use std::rc::Rc;

#[cfg(not(windows))]
use gpui::AnyWindowHandle;
use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, DisplayId, Entity, Pixels, Render,
    Subscription, WeakEntity, Window, WindowBackgroundAppearance, WindowBounds, WindowKind,
    WindowOptions,
};

use crate::system::native::TrayRect;
use crate::system::tray::TrayMenuState;
use crate::ui::menu::{DismissHandler, MenuEntrance, MenuEntry, MenuView};
#[cfg(not(windows))]
use crate::ui::primitives::overlay_fade_out;
#[cfg(windows)]
use crate::ui::primitives::OVERLAY_ENTER_MS;
use crate::ui::primitives::OVERLAY_EXIT_MS;
use crate::windows::registry::{self, WindowKind as RegistryKind};

const MENU_WIDTH: f32 = 304.0;
const ITEM_HEIGHT: f32 = 28.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const MENU_PADDING: f32 = 8.0;
const SCREEN_GAP: f32 = 8.0;
#[cfg(windows)]
const MENU_FADE_MS: u32 = OVERLAY_ENTER_MS as u32;
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

/// Destroys the window without any exit animation. Only for the path where the
/// window never managed to activate, so there is nothing on screen to fade.
fn discard_tray_menu(window: &mut Window, cx: &mut App) {
    window.remove_window();
    registry::forget(RegistryKind::TrayMenu, cx);
}

struct MenuGeometry {
    bounds: Bounds<Pixels>,
    display_id: Option<DisplayId>,
}

pub(crate) struct MenuDisplay {
    pub(crate) work_area: Bounds<Pixels>,
    pub(crate) tray_rect: Option<TrayRect>,
    pub(crate) id: DisplayId,
}

pub struct TrayMenuWindow {
    menu: Entity<MenuView>,
    activation: Option<Subscription>,
    revealing: bool,
    #[cfg(not(windows))]
    window_handle: AnyWindowHandle,
    closing_at: Option<std::time::Instant>,
}

impl TrayMenuWindow {
    fn new(menu: Entity<MenuView>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            menu,
            activation: None,
            revealing: cfg!(windows),
            #[cfg(not(windows))]
            window_handle: window.window_handle(),
            closing_at: None,
        };
        view.activation = Some(cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() && !this.revealing && this.closing_at.is_none() {
                *CLOSED_ON_DEACTIVATION.lock() = Some(std::time::Instant::now());
                this.begin_close(window, cx);
            }
        }));
        view
    }

    fn begin_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing_at.is_some() {
            return;
        }
        self.closing_at = Some(std::time::Instant::now());

        #[cfg(windows)]
        {
            registry::forget(RegistryKind::TrayMenu, cx);
            hide_tray_menu_window(window);
            window.remove_window();
        }

        #[cfg(not(windows))]
        {
            let handle = self.window_handle;
            cx.spawn(async move |_this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(OVERLAY_EXIT_MS))
                    .await;
                let _ = cx.update(|cx| {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                    registry::forget(RegistryKind::TrayMenu, cx);
                });
            })
            .detach();
            cx.notify();
        }
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
            let max_height = geometry.bounds.size.height;
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(geometry.bounds)),
                    titlebar: None,
                    focus: true,
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
                    #[cfg(windows)]
                    if let Some(hwnd) = crate::windows::window_hwnd(window) {
                        crate::system::window_composition::stage_window(
                            hwnd,
                            geometry.bounds,
                            window.scale_factor(),
                            true,
                        );
                    }
                    // The menu has to be built before the view that owns it, so
                    // the dismiss handler reaches that view through this slot,
                    // which is filled a few lines below.
                    let owner: Rc<RefCell<Option<WeakEntity<TrayMenuWindow>>>> =
                        Rc::new(RefCell::new(None));
                    let dismiss: DismissHandler = {
                        let owner = owner.clone();
                        Rc::new(move |window, cx| {
                            let _ = owner.borrow().clone().map(|owner| {
                                owner.update(cx, |this, cx| this.begin_close(window, cx))
                            });
                        })
                    };
                    let menu = cx.new(|cx| {
                        MenuView::new(entries, dismiss, cx)
                            .compact(true)
                            .neutral_highlight(true)
                            .entrance(MenuEntrance::Instant)
                            .min_width(px(MENU_WIDTH))
                            .max_height(max_height)
                    });
                    window.focus(&menu.read(cx).focus_handle());
                    let view = cx.new(|cx| Self::new(menu, window, cx));
                    *owner.borrow_mut() = Some(view.downgrade());
                    #[cfg(windows)]
                    {
                        // The window is created inactive, so the activation
                        // observer would otherwise read that initial state as a
                        // deactivation and close the menu before it had ever been
                        // seen.
                        let settled = view.clone();
                        window.on_next_frame(move |window, _cx| {
                            configure_tray_menu_window(window);
                            window.on_next_frame(move |window, _cx| {
                                if let Some(hwnd) = crate::windows::window_hwnd(window) {
                                    crate::system::window_composition::reveal_window(
                                        hwnd,
                                        true,
                                        MENU_FADE_MS,
                                    );
                                }
                                window.activate_window();
                                window.on_next_frame(move |window, cx| {
                                    settled.update(cx, |this, _cx| this.revealing = false);
                                    if !window.is_window_active() {
                                        discard_tray_menu(window, cx);
                                    }
                                });
                            });
                        });
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
        let surface = div().size_full().child(self.menu.clone());

        #[cfg(windows)]
        return surface.into_any_element();

        #[cfg(not(windows))]
        match self.closing_at {
            Some(_) => overlay_fade_out("tray-menu-exit", surface).into_any_element(),
            None => surface.into_any_element(),
        }
    }
}

#[cfg(windows)]
fn hide_tray_menu_window(window: &Window) {
    use windows::Win32::UI::WindowsAndMessaging::{AnimateWindow, AW_BLEND, AW_HIDE};

    let Some(hwnd) = crate::windows::window_hwnd(window) else {
        return;
    };
    unsafe {
        let _ = AnimateWindow(hwnd, OVERLAY_EXIT_MS as u32, AW_HIDE | AW_BLEND);
    }
}

fn configure_tray_menu_window(window: &Window) {
    #[cfg(windows)]
    {
        let Some(hwnd) = crate::windows::window_hwnd(window) else {
            return;
        };
        crate::system::window_composition::configure_transparent_surface(hwnd);
        crate::system::window_composition::set_rounded_client_region(
            window,
            crate::ui::chrome::RADIUS_3XL,
        );
    }
    #[cfg(not(windows))]
    let _ = window;
}

/// The window is exactly the menu surface. It used to carry a 12px margin on
/// every side to give `shadow_md` room, but the transparent margin of a
/// borderless window is DWM's to fill, and it filled it -- the menu sat inside a
/// visible rounded box of its own, with a border and a shadow. The drop shadow
/// now comes from DWM around the surface itself, which is what the recording bar
/// does.
fn menu_geometry(tray_rect: Option<TrayRect>, entries: &[MenuEntry], cx: &mut App) -> MenuGeometry {
    let Some(display) = menu_display(tray_rect, cx) else {
        let window_size = size(px(MENU_WIDTH), px(menu_height(entries)));
        return MenuGeometry {
            bounds: Bounds::centered(None, window_size, cx),
            display_id: None,
        };
    };
    let max_content_height = f32::from(display.work_area.size.height) - SCREEN_GAP * 2.0;
    let content_height = menu_height(entries).min(max_content_height);
    let window_width = MENU_WIDTH;
    let window_height = content_height;
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
pub(crate) fn menu_display(tray_rect: Option<TrayRect>, cx: &mut App) -> Option<MenuDisplay> {
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
pub(crate) fn menu_display(tray_rect: Option<TrayRect>, cx: &mut App) -> Option<MenuDisplay> {
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
