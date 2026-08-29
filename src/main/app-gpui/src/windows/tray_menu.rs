use std::cell::RefCell;
use std::rc::Rc;

#[cfg(not(windows))]
use gpui::AnyWindowHandle;
#[cfg(windows)]
use gpui::Global;
use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, DisplayId, Entity, Pixels, Render,
    Subscription, WeakEntity, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions,
};

use crate::system::native::TrayRect;
use crate::system::tray::TrayMenuState;
use crate::ui::menu::{DismissHandler, MenuEntrance, MenuEntry, MenuView};
#[cfg(not(windows))]
use crate::ui::primitives::overlay_fade_out;
use crate::ui::primitives::OVERLAY_EXIT_MS;
#[cfg(not(windows))]
use crate::windows::registry::{self, WindowKind as RegistryKind};

const MENU_WIDTH: f32 = 304.0;
const ITEM_HEIGHT: f32 = 28.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const MENU_PADDING: f32 = 8.0;
const SCREEN_GAP: f32 = 8.0;
#[cfg(windows)]
const MENU_FADE_MS: u32 = 80;
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

struct MenuGeometry {
    bounds: Bounds<Pixels>,
    display_id: Option<DisplayId>,
    scale_factor: Option<f32>,
}

pub(crate) struct MenuDisplay {
    pub(crate) work_area: Bounds<Pixels>,
    pub(crate) tray_rect: Option<TrayRect>,
    pub(crate) id: DisplayId,
    pub(crate) scale_factor: Option<f32>,
}

#[cfg(windows)]
#[derive(Default)]
struct TrayMenuCache {
    handle: Option<WindowHandle<TrayMenuWindow>>,
}

#[cfg(windows)]
impl Global for TrayMenuCache {}

#[cfg(windows)]
fn tray_menu_cache(cx: &mut App) -> &mut TrayMenuCache {
    if cx.try_global::<TrayMenuCache>().is_none() {
        cx.set_global(TrayMenuCache::default());
    }
    cx.global_mut::<TrayMenuCache>()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrayMenuVisibility {
    Hidden,
    Revealing,
    Visible,
    Closing,
}

#[cfg(any(windows, test))]
fn reveal_is_current(
    visibility: TrayMenuVisibility,
    current_generation: u64,
    candidate_generation: u64,
) -> bool {
    visibility == TrayMenuVisibility::Revealing && current_generation == candidate_generation
}

impl TrayMenuVisibility {
    fn new(visible: bool) -> Self {
        if visible && cfg!(windows) {
            return Self::Revealing;
        }
        if visible {
            return Self::Visible;
        }
        Self::Hidden
    }

    fn is_shown(self) -> bool {
        matches!(self, Self::Revealing | Self::Visible)
    }

    fn begin_close(&mut self) -> bool {
        if !self.is_shown() {
            return false;
        }
        *self = Self::Closing;
        true
    }
}

pub struct TrayMenuWindow {
    menu: Entity<MenuView>,
    dismiss: DismissHandler,
    activation: Option<Subscription>,
    visibility: TrayMenuVisibility,
    #[cfg(windows)]
    reveal_generation: u64,
    #[cfg(not(windows))]
    window_handle: AnyWindowHandle,
}

impl TrayMenuWindow {
    fn new(
        menu: Entity<MenuView>,
        dismiss: DismissHandler,
        visible: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self {
            menu,
            dismiss,
            activation: None,
            visibility: TrayMenuVisibility::new(visible),
            #[cfg(windows)]
            reveal_generation: u64::from(visible),
            #[cfg(not(windows))]
            window_handle: window.window_handle(),
        };
        view.activation = Some(cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() && this.visibility == TrayMenuVisibility::Visible {
                *CLOSED_ON_DEACTIVATION.lock() = Some(std::time::Instant::now());
                this.begin_close(window, cx);
            }
        }));
        view
    }

    fn begin_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.visibility.begin_close() {
            return;
        }
        #[cfg(windows)]
        {
            self.reveal_generation = self.reveal_generation.wrapping_add(1);
        }
        self.finish_close(window, cx);
    }

    #[cfg(windows)]
    fn finish_close(&mut self, window: &mut Window, _: &mut Context<Self>) {
        hide_tray_menu_window(window);
        self.visibility = TrayMenuVisibility::Hidden;
    }

    #[cfg(not(windows))]
    fn finish_close(&mut self, _: &mut Window, cx: &mut Context<Self>) {
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

    #[cfg(windows)]
    pub fn prewarm(cx: &mut App) {
        if let Some(handle) = tray_menu_cache(cx).handle {
            if handle.update(cx, |_, _, _| ()).is_ok() {
                return;
            }
        }
        let opened = Self::open(None, false, cx);
        tray_menu_cache(cx).handle = opened;
    }

    pub fn toggle(tray_rect: Option<TrayRect>, cx: &mut App) {
        if recently_closed_on_deactivation() {
            return;
        }

        #[cfg(windows)]
        {
            if let Some(handle) = tray_menu_cache(cx).handle {
                let updated = handle.update(cx, |this, window, cx| {
                    if this.visibility.is_shown() {
                        this.begin_close(window, cx);
                        return;
                    }
                    this.show(tray_rect, window, cx);
                });
                if updated.is_ok() {
                    return;
                }
                tray_menu_cache(cx).handle = None;
            }
            let opened = Self::open(tray_rect, true, cx);
            tray_menu_cache(cx).handle = opened;
        }

        #[cfg(not(windows))]
        registry::toggle(RegistryKind::TrayMenu, cx, |cx| {
            Self::open(tray_rect, true, cx).map(Into::into)
        });
    }

    fn open(
        tray_rect: Option<TrayRect>,
        visible: bool,
        cx: &mut App,
    ) -> Option<WindowHandle<Self>> {
        let config = crate::state::state(cx).config.get();
        let state = TrayMenuState::from_config(&config);
        let entries = crate::system::tray::entries(&state, tray_rect);
        let geometry = menu_geometry(tray_rect, &entries, cx);
        let max_height = geometry.bounds.size.height;
        let initial_bounds = if cfg!(windows) && !visible {
            Bounds {
                origin: gpui::point(px(-32000.0), px(-32000.0)),
                size: geometry.bounds.size,
            }
        } else {
            geometry.bounds
        };
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(initial_bounds)),
                titlebar: None,
                focus: visible,
                show: !cfg!(windows) || !visible,
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
                        initial_bounds,
                        window.scale_factor(),
                        false,
                    );
                }
                let owner: Rc<RefCell<Option<WeakEntity<TrayMenuWindow>>>> =
                    Rc::new(RefCell::new(None));
                let dismiss: DismissHandler = {
                    let owner = owner.clone();
                    Rc::new(move |window, cx| {
                        let _ = owner
                            .borrow()
                            .clone()
                            .map(|owner| owner.update(cx, |this, cx| this.begin_close(window, cx)));
                    })
                };
                let menu = cx.new(|cx| build_menu(entries, dismiss.clone(), max_height, cx));
                if visible {
                    window.focus(&menu.read(cx).focus_handle());
                }
                let view = cx.new(|cx| Self::new(menu, dismiss, visible, window, cx));
                *owner.borrow_mut() = Some(view.downgrade());
                #[cfg(windows)]
                if visible {
                    let generation = view.read(cx).reveal_generation;
                    Self::schedule_reveal(view.clone(), window, generation);
                    window.activate_window();
                }
                view
            },
        )
        .ok()
    }

    #[cfg(windows)]
    fn show(&mut self, tray_rect: Option<TrayRect>, window: &mut Window, cx: &mut Context<Self>) {
        let config = crate::state::state(cx).config.get();
        let state = TrayMenuState::from_config(&config);
        let entries = crate::system::tray::entries(&state, tray_rect);
        let geometry = menu_geometry(tray_rect, &entries, cx);
        let max_height = geometry.bounds.size.height;
        if let Some(hwnd) = crate::windows::window_hwnd(window) {
            crate::system::window_composition::stage_window(
                hwnd,
                geometry.bounds,
                geometry
                    .scale_factor
                    .unwrap_or_else(|| window.scale_factor()),
                false,
            );
        }
        self.menu = cx.new(|cx| build_menu(entries, self.dismiss.clone(), max_height, cx));
        self.visibility = TrayMenuVisibility::Revealing;
        self.reveal_generation = self.reveal_generation.wrapping_add(1);
        let generation = self.reveal_generation;
        window.focus(&self.menu.read(cx).focus_handle());
        cx.notify();
        Self::schedule_reveal(cx.entity(), window, generation);
        window.activate_window();
    }

    #[cfg(windows)]
    fn schedule_reveal(view: Entity<Self>, window: &mut Window, generation: u64) {
        window.on_next_frame(move |window, cx| {
            if !view.read(cx).is_current_reveal(generation) {
                return;
            }
            configure_tray_menu_window(window);
            let reveal_view = view.clone();
            window.on_next_frame(move |window, cx| {
                if !reveal_view.read(cx).is_current_reveal(generation) {
                    return;
                }
                if let Some(hwnd) = crate::windows::window_hwnd(window) {
                    crate::system::window_composition::reveal_window(hwnd, true, MENU_FADE_MS);
                }
                window.activate_window();
                let settle_view = reveal_view.clone();
                window.on_next_frame(move |window, cx| {
                    let active = window.is_window_active();
                    settle_view.update(cx, |this, cx| {
                        if !this.is_current_reveal(generation) {
                            return;
                        }
                        this.visibility = TrayMenuVisibility::Visible;
                        if !active {
                            this.begin_close(window, cx);
                        }
                    });
                });
            });
        });
    }

    #[cfg(windows)]
    fn is_current_reveal(&self, generation: u64) -> bool {
        reveal_is_current(self.visibility, self.reveal_generation, generation)
    }
}

fn build_menu(
    entries: Vec<MenuEntry>,
    dismiss: DismissHandler,
    max_height: Pixels,
    cx: &mut Context<MenuView>,
) -> MenuView {
    MenuView::new(entries, dismiss, cx)
        .compact(true)
        .neutral_highlight(true)
        .entrance(if cfg!(windows) {
            MenuEntrance::Instant
        } else {
            MenuEntrance::Overlay
        })
        .min_width(px(MENU_WIDTH))
        .max_height(max_height)
}

impl Render for TrayMenuWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let surface = div().size_full().child(self.menu.clone());

        #[cfg(windows)]
        return surface.into_any_element();

        #[cfg(not(windows))]
        match self.visibility {
            TrayMenuVisibility::Closing => {
                overlay_fade_out("tray-menu-exit", surface).into_any_element()
            }
            _ => surface.into_any_element(),
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

fn menu_geometry(tray_rect: Option<TrayRect>, entries: &[MenuEntry], cx: &mut App) -> MenuGeometry {
    let Some(display) = menu_display(tray_rect, cx) else {
        let window_size = size(px(MENU_WIDTH), px(menu_height(entries)));
        return MenuGeometry {
            bounds: Bounds::centered(None, window_size, cx),
            display_id: None,
            scale_factor: None,
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
            scale_factor: display.scale_factor,
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
        scale_factor: display.scale_factor,
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
            scale_factor: None,
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
        scale_factor: Some(dpi_x as f32 / 96.0),
    })
}

#[cfg(not(windows))]
pub(crate) fn menu_display(tray_rect: Option<TrayRect>, cx: &mut App) -> Option<MenuDisplay> {
    let display = cx.displays().first()?.clone();
    Some(MenuDisplay {
        work_area: crate::system::work_area::work_area(display.bounds()),
        tray_rect,
        id: display.id(),
        scale_factor: None,
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
    fn shown_menu_enters_closing_state() {
        let mut visibility = TrayMenuVisibility::Visible;
        let started = visibility.begin_close();
        assert_eq!((started, visibility), (true, TrayMenuVisibility::Closing));
    }

    #[test]
    fn hidden_menu_ignores_duplicate_close() {
        let mut visibility = TrayMenuVisibility::Hidden;
        let started = visibility.begin_close();
        assert_eq!((started, visibility), (false, TrayMenuVisibility::Hidden));
    }

    #[test]
    fn stale_reveal_generation_is_rejected() {
        assert!(!reveal_is_current(TrayMenuVisibility::Revealing, 2, 1));
        assert!(!reveal_is_current(TrayMenuVisibility::Hidden, 2, 2));
        assert!(reveal_is_current(TrayMenuVisibility::Revealing, 2, 2));
    }

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
