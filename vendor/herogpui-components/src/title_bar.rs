//! `TitleBar` and `WindowBorder` — custom window chrome for frameless windows
//! (HeroGPUI extension; HeroUI v3 is a web library and has no window chrome).
//! The platform split follows gpui-kit's `title_bar.rs` and
//! `window_border.rs`, checked against the pinned `gpui-pre` 0.3.11 platform
//! sources.
//!
//! Open the window with [`TitleBar::window_options`] (or copy its fields into
//! your own `WindowOptions`): it hides the system title bar
//! (`appears_transparent`), positions the macOS traffic lights inside the
//! bar, marks the title bar as app-owned (`app_owns_titlebar_drag`, so AppKit
//! neither drags from it nor delays its clicks) and asks Linux compositors for
//! client-side decorations. Then render a `TitleBar` at the top of the root
//! view, inside a [`WindowBorder`] on Linux.
//!
//! **Dragging.** The bar's empty area moves the window. On macOS, X11 and
//! Wayland a press followed by a move calls `Window::start_window_move`; on
//! Windows the area is reported to the OS as the caption
//! (`WindowControlArea::Drag`, `HTCAPTION`), which drags natively. Children
//! of the bar stop presses from reaching the drag area, so buttons in the
//! bar stay clickable.
//!
//! **Double-click** zooms: on macOS through `Window::titlebar_double_click`,
//! which honours the user's "double-click a window's title bar to" setting;
//! on Linux through `Window::zoom_window`; on Windows the OS does it on its
//! caption. Right-click on a Linux client-decorated bar opens the
//! compositor's window menu.
//!
//! **Controls** ([`TitleBarControls`]):
//!
//! | | `Auto` (default) | `Custom` | `Hidden` |
//! |---|---|---|---|
//! | macOS | none: the native traffic lights, with 80px reserved for them | drawn buttons | none |
//! | Windows | drawn buttons reported as `Min`/`Max`/`Close` control areas; the OS performs them (and shows the Windows 11 snap layouts on Max) | drawn buttons | none |
//! | Linux | drawn buttons, only when the window is client-decorated | drawn buttons | none |
//! | Web | none | drawn buttons | none |
//!
//! Drawn buttons respect `Window::window_controls()` (a tiling compositor may
//! offer no minimize or maximize) and show Restore while the window is
//! maximized. What each drawn button, the drag and the double-click do is a
//! [`WindowAction`]; [`TitleBar::on_window_action`] replaces the default
//! (`WindowAction::perform`) — to confirm before closing, say. The OS-handled
//! Windows `Auto` buttons and caption never reach it; intercept a close there
//! with `Window::on_window_should_close`.
//!
//! **Limits.** The pinned GPUI draws no resize border on Windows or macOS
//! (the OS keeps resizing), and none on Linux unless the window is wrapped in
//! [`WindowBorder`]. The browser has no window to move, zoom or close: the web
//! platform's `start_window_move` is a no-op and its `minimize`/`zoom` log a
//! warning. The macOS traffic lights stay whenever the window has a title
//! bar; `Custom` there suits a window opened with `titlebar: None`.
//!
//! **Accessibility.** The bar reports `Role::TitleBar` named by its `title`,
//! and each drawn control `Role::Button` named "Minimize", "Maximize" or
//! "Restore", and the `Close` UI string. The controls are not tab stops, as
//! native caption buttons are not.

use std::sync::Arc;

use gpui::{
    div, prelude::FluentBuilder as _, px, AnyElement, App, Decorations, Edges, ElementId,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point, RenderOnce,
    ResizeEdge, SharedString, StatefulInteractiveElement, Styled, Tiling, TitlebarOptions, Window,
    WindowControlArea, WindowDecorations, WindowOptions,
};
use herogpui_core::element_id;
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};
use crate::{util, IconName};

/// The default height of a [`TitleBar`].
pub const TITLE_BAR_HEIGHT: Pixels = px(36.);
/// The width reserved on macOS for the native traffic lights.
pub const TRAFFIC_LIGHTS_WIDTH: Pixels = px(80.);
/// The width of one drawn window control.
const CONTROL_WIDTH: Pixels = px(46.);

type ActionCallback = Arc<dyn Fn(&WindowAction, &mut Window, &mut App) + 'static>;

/// Which window controls a [`TitleBar`] draws. See the [module docs](self).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TitleBarControls {
    /// The platform's convention.
    #[default]
    Auto,
    /// Drawn buttons on every platform, performed by the bar.
    Custom,
    /// No controls.
    Hidden,
}

/// Something the title bar asks the window to do.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum WindowAction {
    /// The minimize button.
    Minimize,
    /// The maximize / restore button.
    Zoom,
    /// The close button.
    Close,
    /// A drag began on the bar's empty area.
    Move,
    /// A double-click on the bar's empty area.
    DoubleClick,
    /// A right-click on a Linux client-decorated bar, at this window position.
    ShowWindowMenu(Point<Pixels>),
}

impl WindowAction {
    /// The platform default: minimize, zoom, remove the window, start a
    /// window move, the double-click action (the macOS user setting;
    /// zoom elsewhere), or open the window menu.
    pub fn perform(&self, window: &mut Window) {
        match *self {
            WindowAction::Minimize => window.minimize_window(),
            WindowAction::Zoom => window.zoom_window(),
            WindowAction::Close => window.remove_window(),
            WindowAction::Move => window.start_window_move(),
            WindowAction::DoubleClick => {
                if cfg!(target_os = "macos") {
                    window.titlebar_double_click();
                } else {
                    window.zoom_window();
                }
            }
            WindowAction::ShowWindowMenu(at) => window.show_window_menu(at),
        }
    }
}

/// The host platform, as far as window chrome cares.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Host {
    Mac,
    Windows,
    Linux,
    Web,
}

impl Host {
    fn current() -> Self {
        if cfg!(target_family = "wasm") {
            Host::Web
        } else if cfg!(target_os = "macos") {
            Host::Mac
        } else if cfg!(target_os = "windows") {
            Host::Windows
        } else {
            Host::Linux
        }
    }
}

/// Whether the bar draws buttons, and whether the OS performs them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ControlsPlan {
    None,
    /// Buttons whose presses the bar performs.
    Drawn,
    /// Buttons the OS hit-tests and performs (Windows).
    Native,
}

fn controls_plan(controls: TitleBarControls, host: Host, client_decorated: bool) -> ControlsPlan {
    match controls {
        TitleBarControls::Hidden => ControlsPlan::None,
        TitleBarControls::Custom => ControlsPlan::Drawn,
        TitleBarControls::Auto => match host {
            Host::Mac | Host::Web => ControlsPlan::None,
            Host::Windows => ControlsPlan::Native,
            Host::Linux if client_decorated => ControlsPlan::Drawn,
            Host::Linux => ControlsPlan::None,
        },
    }
}

/// A custom window title bar. See the [module docs](self).
#[must_use]
#[derive(IntoElement)]
pub struct TitleBar {
    id: ElementId,
    title: Option<SharedString>,
    children: Vec<AnyElement>,
    height: Pixels,
    controls: TitleBarControls,
    on_window_action: Option<ActionCallback>,
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl TitleBar {
    /// A title bar keyed by `id`.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            title: None,
            children: Vec::new(),
            height: TITLE_BAR_HEIGHT,
            controls: TitleBarControls::Auto,
            on_window_action: None,
            sx: None,
        }
    }

    /// The window title drawn at the start of the bar, and the bar's
    /// accessible name.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The bar's height; [`TITLE_BAR_HEIGHT`] by default.
    pub fn height(mut self, height: impl Into<Pixels>) -> Self {
        self.height = height.into();
        self
    }

    /// Which window controls to draw; [`TitleBarControls::Auto`] by default.
    pub fn controls(mut self, controls: TitleBarControls) -> Self {
        self.controls = controls;
        self
    }

    /// Replaces the default [`WindowAction::perform`] for every action the
    /// bar itself performs. Call `action.perform(window)` to keep the
    /// default after your own handling.
    pub fn on_window_action(
        mut self,
        f: impl Fn(&WindowAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_window_action = Some(Arc::new(f));
        self
    }

    /// Caller-owned styling refined over the bar after the theme's values.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }

    /// `WindowOptions` for a window that draws a `TitleBar`: no system title
    /// bar, traffic lights inside the bar on macOS, an app-owned title-bar
    /// drag, and client-side decorations requested on Linux.
    pub fn window_options() -> WindowOptions {
        WindowOptions {
            titlebar: Some(Self::titlebar_options()),
            app_owns_titlebar_drag: true,
            window_decorations: Some(WindowDecorations::Client),
            ..Default::default()
        }
    }

    /// The `TitlebarOptions` half of [`TitleBar::window_options`].
    pub fn titlebar_options() -> TitlebarOptions {
        TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: Some(gpui::point(px(12.), px(12.))),
        }
    }
}

impl ParentElement for TitleBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// Runs `action` through the caller's hook, or performs it.
fn dispatch(
    hook: &Option<ActionCallback>,
    action: WindowAction,
    window: &mut Window,
    cx: &mut App,
) {
    match hook {
        Some(f) => f(&action, window, cx),
        None => action.perform(window),
    }
}

/// The debug-selector spelling of an id: its name when it has one.
fn selector_base(id: &ElementId) -> String {
    match id {
        ElementId::Name(name) => name.to_string(),
        other => format!("{other:?}"),
    }
}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base = self.id.clone();
        let selector = selector_base(&base);
        let host = Host::current();
        let decorations = window.window_decorations();
        let client_decorated = matches!(decorations, Decorations::Client { .. });
        let plan = controls_plan(self.controls, host, client_decorated);
        // A press armed for a window move: the move starts on the first
        // pointer motion after it, so a plain click never becomes a drag.
        let armed = window.use_keyed_state(element_id::scoped(&base, "armed"), cx, |_, _| false);
        let colors = cx.colors().clone();
        let layout = cx.layout().clone();
        let hook = self.on_window_action.clone();
        let leading = if host == Host::Mac
            && self.controls == TitleBarControls::Auto
            && !window.is_fullscreen()
        {
            TRAFFIC_LIGHTS_WIDTH
        } else {
            px(12.)
        };

        // The drag area lies behind the content, so a child's own hitbox
        // keeps its presses (children block the mouse below).
        let mut drag = div()
            .id(element_id::scoped(&base, "drag"))
            .absolute()
            .inset_0()
            .debug_selector({
                let name = format!("{selector}-drag");
                move || name
            });
        if host != Host::Web {
            drag = drag.window_control_area(WindowControlArea::Drag);
        }
        {
            let armed_down = armed.clone();
            let hook_down = hook.clone();
            drag = drag.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                if event.click_count >= 2 {
                    armed_down.update(cx, |a, _| *a = false);
                    if host != Host::Windows {
                        dispatch(&hook_down, WindowAction::DoubleClick, window, cx);
                    }
                } else {
                    armed_down.update(cx, |a, _| *a = true);
                }
            });
            let armed_up = armed.clone();
            drag = drag.on_mouse_up(MouseButton::Left, move |_, _, cx| {
                armed_up.update(cx, |a, _| *a = false);
            });
            let armed_out = armed.clone();
            drag = drag.on_mouse_down_out(move |_, _, cx| {
                armed_out.update(cx, |a, _| *a = false);
            });
            let hook_move = hook.clone();
            drag = drag.on_mouse_move(move |event, window, cx| {
                if event.pressed_button != Some(MouseButton::Left) || !*armed.read(cx) {
                    return;
                }
                armed.update(cx, |a, _| *a = false);
                dispatch(&hook_move, WindowAction::Move, window, cx);
            });
            if host == Host::Linux && client_decorated {
                let hook_menu = hook.clone();
                drag = drag.on_mouse_down(MouseButton::Right, move |event, window, cx| {
                    dispatch(
                        &hook_menu,
                        WindowAction::ShowWindowMenu(event.position),
                        window,
                        cx,
                    );
                });
            }
        }

        let mut content = div()
            .relative()
            .flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .items_center()
            .gap(px(8.))
            .pl(leading)
            .pr(px(8.));
        if let Some(title) = &self.title {
            content = content.child(
                div()
                    .flex_shrink_0()
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.foreground)
                    .whitespace_nowrap()
                    .child(title.clone()),
            );
        }
        if !self.children.is_empty() {
            content = content.child(
                div()
                    .id(element_id::scoped(&base, "content"))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .min_w_0()
                    // Presses on the caller's controls stay theirs: nothing
                    // below them (the drag area) sees them.
                    .block_mouse_except_scroll()
                    .children(self.children),
            );
        }

        let mut bar = div()
            .id(base.clone())
            .a11y_named(a11y::Role::TitleBar, &a11y::Name::maybe(self.title.clone()))
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(self.height)
            .flex_shrink_0()
            .bg(colors.surface.background)
            .border_b(layout.border_width)
            .border_color(colors.separator)
            .debug_selector({
                let name = format!("{selector}-title-bar");
                move || name
            })
            .child(drag)
            .child(content);

        if plan != ControlsPlan::None {
            let supported = window.window_controls();
            let maximized = window.is_maximized();
            let mut controls = div()
                .relative()
                .flex()
                .h_full()
                .flex_shrink_0()
                .block_mouse_except_scroll();
            let mut buttons: Vec<(WindowAction, &'static str, SharedString, IconName)> = Vec::new();
            if supported.minimize {
                buttons.push((
                    WindowAction::Minimize,
                    "minimize",
                    "Minimize".into(),
                    IconName::Minus,
                ));
            }
            if supported.maximize {
                let (name, icon) = if maximized {
                    ("Restore", IconName::Copy)
                } else {
                    ("Maximize", IconName::Square)
                };
                buttons.push((WindowAction::Zoom, "zoom", name.into(), icon));
            }
            buttons.push((
                WindowAction::Close,
                "close",
                crate::i18n::ui_string(crate::i18n::UiString::Close, cx),
                IconName::X,
            ));
            for (action, part, name, icon) in buttons {
                let is_close = action == WindowAction::Close;
                let (hover_bg, hover_fg) = if is_close {
                    (colors.danger.color, colors.danger.foreground)
                } else {
                    (colors.default.color, colors.foreground)
                };
                let mut button = div()
                    .id(element_id::scoped(&base, part))
                    .a11y_named(a11y::Role::Button, &a11y::Name::labelled(name))
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(CONTROL_WIDTH)
                    .h_full()
                    .text_color(colors.foreground)
                    .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
                    .debug_selector({
                        let name = format!("{selector}-{part}");
                        move || name
                    })
                    .child(
                        gpui::svg()
                            .size(px(14.))
                            .path(icon)
                            .text_color(colors.foreground),
                    );
                match plan {
                    ControlsPlan::Native => {
                        button = button.window_control_area(match action {
                            WindowAction::Minimize => WindowControlArea::Min,
                            WindowAction::Zoom => WindowControlArea::Max,
                            _ => WindowControlArea::Close,
                        });
                    }
                    _ => {
                        let hook = hook.clone();
                        button = button
                            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                                window.prevent_default();
                                cx.stop_propagation();
                            })
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                dispatch(&hook, action, window, cx);
                            });
                    }
                }
                controls = controls.child(button);
            }
            bar = bar.child(controls);
        }
        util::apply_sx(bar, &self.sx)
    }
}

/// The default shadow reach of a Linux client-decorated [`WindowBorder`].
pub const WINDOW_SHADOW_SIZE: Pixels = px(12.);
/// Half the width of the band around the frame edge that starts a resize.
const RESIZE_HIT: Pixels = px(4.);

/// A frame for a client-decorated Linux window: shadow, a 1px border and
/// resize edges. Everywhere else, and on Linux under server-side
/// decorations, it renders its children unchanged — the OS draws the frame.
/// See the [module docs](self).
#[must_use]
#[derive(IntoElement)]
pub struct WindowBorder {
    children: Vec<AnyElement>,
    shadow_size: Pixels,
}

impl WindowBorder {
    /// A frame around the window's content.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            shadow_size: WINDOW_SHADOW_SIZE,
        }
    }

    /// How far the shadow reaches outside the frame (the client inset the
    /// compositor is told about); [`WINDOW_SHADOW_SIZE`] by default.
    pub fn shadow_size(mut self, size: impl Into<Pixels>) -> Self {
        self.shadow_size = size.into();
        self
    }
}

impl Default for WindowBorder {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for WindowBorder {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// Per side, how far the frame sits inside the window: the shadow, except on
/// a side tiled against the screen edge or another window.
fn frame_insets(shadow: Pixels, tiling: Tiling) -> Edges<Pixels> {
    Edges {
        top: if tiling.top { px(0.) } else { shadow },
        right: if tiling.right { px(0.) } else { shadow },
        bottom: if tiling.bottom { px(0.) } else { shadow },
        left: if tiling.left { px(0.) } else { shadow },
    }
}

/// Which edge a press at `pos` resizes: within [`RESIZE_HIT`] of the frame
/// edge (inside or out), corners first. Tiled sides do not resize.
fn resize_edge(
    pos: Point<Pixels>,
    size: gpui::Size<Pixels>,
    insets: Edges<Pixels>,
    tiling: Tiling,
) -> Option<ResizeEdge> {
    let near = |distance: Pixels| distance.abs() <= RESIZE_HIT;
    let top = !tiling.top && near(pos.y - insets.top);
    let bottom = !tiling.bottom && near(size.height - insets.bottom - pos.y);
    let left = !tiling.left && near(pos.x - insets.left);
    let right = !tiling.right && near(size.width - insets.right - pos.x);
    // A corner is a band's width either way along both edges.
    let corner = RESIZE_HIT * 4.;
    let near_top = !tiling.top && (pos.y - insets.top).abs() <= corner;
    let near_bottom = !tiling.bottom && (size.height - insets.bottom - pos.y).abs() <= corner;
    let near_left = !tiling.left && (pos.x - insets.left).abs() <= corner;
    let near_right = !tiling.right && (size.width - insets.right - pos.x).abs() <= corner;
    match () {
        _ if (top && near_left) || (left && near_top) => Some(ResizeEdge::TopLeft),
        _ if (top && near_right) || (right && near_top) => Some(ResizeEdge::TopRight),
        _ if (bottom && near_left) || (left && near_bottom) => Some(ResizeEdge::BottomLeft),
        _ if (bottom && near_right) || (right && near_bottom) => Some(ResizeEdge::BottomRight),
        _ if top => Some(ResizeEdge::Top),
        _ if bottom => Some(ResizeEdge::Bottom),
        _ if left => Some(ResizeEdge::Left),
        _ if right => Some(ResizeEdge::Right),
        _ => None,
    }
}

fn resize_cursor(edge: ResizeEdge) -> gpui::CursorStyle {
    use gpui::CursorStyle;
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    }
}

impl RenderOnce for WindowBorder {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Decorations::Client { tiling } = window.window_decorations() else {
            return div().size_full().flex().flex_col().children(self.children);
        };
        let shadow = self.shadow_size;
        // The platform inset stays the full shadow even while tiled, so the
        // first resize after a restore does not count the shadow twice.
        window.set_client_inset(shadow);
        let insets = frame_insets(shadow, tiling);
        let colors = cx.colors().clone();
        let layout = cx.layout().clone();
        let size = window.window_bounds().get_bounds().size;
        let active = window.is_window_active();
        let cursor = resize_edge(window.mouse_position(), size, insets, tiling)
            .map(resize_cursor)
            .unwrap_or_default();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(gpui::transparent_black())
            .pt(insets.top)
            .pr(insets.right)
            .pb(insets.bottom)
            .pl(insets.left)
            .cursor(cursor)
            .on_mouse_move(|_, window, _| window.refresh())
            .on_mouse_down(MouseButton::Left, move |event, window, _| {
                if let Some(edge) = resize_edge(event.position, size, insets, tiling) {
                    window.start_window_resize(edge);
                }
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .cursor(gpui::CursorStyle::default())
                    .bg(colors.background)
                    .border_color(colors.border)
                    .when(!tiling.top, |el| el.border_t(layout.border_width))
                    .when(!tiling.right, |el| el.border_r(layout.border_width))
                    .when(!tiling.bottom, |el| el.border_b(layout.border_width))
                    .when(!tiling.left, |el| el.border_l(layout.border_width))
                    .when(!tiling.is_tiled(), |el| {
                        el.shadow(vec![gpui::BoxShadow {
                            color: gpui::black().opacity(if active { 0.2 } else { 0.12 }),
                            blur_radius: shadow / 2.,
                            spread_radius: px(0.),
                            offset: gpui::point(px(0.), px(1.)),
                            inset: false,
                        }])
                    })
                    .children(self.children),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_controls_follow_the_platform_convention() {
        use ControlsPlan::*;
        let auto = TitleBarControls::Auto;
        assert_eq!(controls_plan(auto, Host::Mac, false), None);
        assert_eq!(controls_plan(auto, Host::Web, true), None);
        assert_eq!(controls_plan(auto, Host::Windows, false), Native);
        assert_eq!(controls_plan(auto, Host::Linux, true), Drawn);
        assert_eq!(controls_plan(auto, Host::Linux, false), None);
        for host in [Host::Mac, Host::Windows, Host::Linux, Host::Web] {
            assert_eq!(controls_plan(TitleBarControls::Custom, host, false), Drawn);
            assert_eq!(controls_plan(TitleBarControls::Hidden, host, true), None);
        }
    }

    #[test]
    fn resize_edges_are_bands_around_the_frame_and_corners_win() {
        let size = gpui::size(px(800.), px(600.));
        let tiling = Tiling::default();
        let insets = frame_insets(px(12.), tiling);
        let at = |x: f32, y: f32| resize_edge(gpui::point(px(x), px(y)), size, insets, tiling);
        assert_eq!(at(400., 12.), Some(ResizeEdge::Top));
        assert_eq!(at(400., 9.), Some(ResizeEdge::Top), "outside the frame too");
        assert_eq!(at(400., 588.), Some(ResizeEdge::Bottom));
        assert_eq!(at(12., 300.), Some(ResizeEdge::Left));
        assert_eq!(at(788., 300.), Some(ResizeEdge::Right));
        assert_eq!(at(14., 14.), Some(ResizeEdge::TopLeft));
        assert_eq!(at(786., 586.), Some(ResizeEdge::BottomRight));
        assert_eq!(at(400., 300.), None, "the content is not an edge");
        assert_eq!(at(400., 40.), None);
    }

    #[test]
    fn tiled_sides_neither_inset_nor_resize() {
        let size = gpui::size(px(800.), px(600.));
        let tiling = Tiling {
            top: true,
            left: true,
            right: false,
            bottom: false,
        };
        let insets = frame_insets(px(12.), tiling);
        assert_eq!(insets.top, px(0.));
        assert_eq!(insets.right, px(12.));
        let at = |x: f32, y: f32| resize_edge(gpui::point(px(x), px(y)), size, insets, tiling);
        assert_eq!(at(400., 0.), None);
        assert_eq!(at(0., 300.), None);
        assert_eq!(at(788., 300.), Some(ResizeEdge::Right));
    }
}

crate::util::impl_component_styled!(TitleBar);
