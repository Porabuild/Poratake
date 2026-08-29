mod model;
mod view;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, AnyView, App, Corner, Pixels,
    Point, SharedString, Window,
};

pub use model::{MenuBuilder, MenuEntry, MenuItem};
pub use view::{DismissHandler, MenuEntrance, MenuView};

struct MenuPopup {
    view: AnyView,
    owner: Option<SharedString>,
    position: Option<Point<Pixels>>,
    anchor: Corner,
    offset: Point<Pixels>,
    animation_id: u64,
    closing_at: Option<std::time::Instant>,
}

/// A shared, cheaply cloned slot for the one menu a surface can have open at a
/// time. Windows keep a handle, pass clones to triggers, and render the layer
/// where the popup should be anchored.
#[derive(Default)]
struct MenuState {
    popup: Option<MenuPopup>,
    /// The trigger whose popup an outside click just dismissed. The dismiss
    /// runs on mouse-down before the same press reaches the trigger, so
    /// without this the trigger would immediately reopen what it just closed.
    suppressed: Option<SharedString>,
    next_animation_id: u64,
}

#[derive(Clone, Default)]
pub struct MenuHandle(Rc<RefCell<MenuState>>);

pub struct MenuPlacement {
    owner: Option<SharedString>,
    position: Option<Point<Pixels>>,
    anchor: Corner,
    offset: Point<Pixels>,
    min_width: Option<Pixels>,
    compact: bool,
}

impl MenuPlacement {
    /// A context menu pinned to a window-space point.
    pub fn at(position: Point<Pixels>) -> Self {
        Self {
            owner: None,
            position: Some(position),
            anchor: Corner::TopLeft,
            offset: point(px(0.0), px(0.0)),
            min_width: None,
            compact: false,
        }
    }

    /// A dropdown hanging below the trigger that renders it.
    pub fn below(owner: impl Into<SharedString>) -> Self {
        Self {
            owner: Some(owner.into()),
            position: None,
            anchor: Corner::TopLeft,
            offset: point(px(0.0), px(4.0)),
            min_width: None,
            compact: false,
        }
    }

    /// A dropdown rising above the trigger that renders it.
    #[allow(dead_code)]
    pub fn above(owner: impl Into<SharedString>) -> Self {
        Self {
            owner: Some(owner.into()),
            position: None,
            anchor: Corner::BottomLeft,
            offset: point(px(0.0), px(-4.0)),
            min_width: None,
            compact: false,
        }
    }

    #[allow(dead_code)]
    pub fn aligned_right(mut self) -> Self {
        self.anchor = match self.anchor {
            Corner::TopLeft => Corner::TopRight,
            Corner::BottomLeft => Corner::BottomRight,
            other => other,
        };
        self
    }

    pub fn min_width(mut self, width: Pixels) -> Self {
        self.min_width = Some(width);
        self
    }

    /// The app pins `.select__popover--sm` to tighter metrics (28px rows, 4px
    /// padding, 12px text) for the compact editor-panel selects.
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl MenuHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.finish_closing();
        self.0
            .borrow()
            .popup
            .as_ref()
            .is_some_and(|popup| popup.closing_at.is_none())
    }

    pub fn is_open_for(&self, owner: &str) -> bool {
        self.finish_closing();
        self.0.borrow().popup.as_ref().is_some_and(|popup| {
            popup.closing_at.is_none()
                && popup
                    .owner
                    .as_ref()
                    .is_some_and(|value| value.as_ref() == owner)
        })
    }

    pub fn close(&self, window: &mut Window) {
        let mut state = self.0.borrow_mut();
        if let Some(popup) = state.popup.as_mut() {
            if popup.closing_at.is_some() {
                return;
            }
            popup.closing_at = Some(std::time::Instant::now());
            state.suppressed = None;
            drop(state);
            window.refresh();
        }
    }

    fn take_suppressed(&self) -> Option<SharedString> {
        self.0.borrow_mut().suppressed.take()
    }

    pub fn open_at(
        &self,
        position: Point<Pixels>,
        entries: Vec<MenuEntry>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.open(MenuPlacement::at(position), entries, window, cx);
    }

    /// Opens the dropdown, or closes it when the same trigger is already open.
    pub fn toggle(
        &self,
        placement: MenuPlacement,
        entries: Vec<MenuEntry>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.toggled_closed(&placement, window) {
            return;
        }
        self.open(placement, entries, window, cx);
    }

    fn toggled_closed(&self, placement: &MenuPlacement, window: &mut Window) -> bool {
        let suppressed = self.take_suppressed();
        match placement.owner.as_ref() {
            Some(owner) => {
                if suppressed.as_ref() == Some(owner) {
                    return true;
                }
                if self.is_open_for(owner) {
                    self.close(window);
                    return true;
                }
            }
            None => {
                if self.is_open() {
                    self.close(window);
                    return true;
                }
            }
        }
        false
    }

    pub fn open(
        &self,
        placement: MenuPlacement,
        entries: Vec<MenuEntry>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if entries.is_empty() {
            return;
        }
        let min_width = placement.min_width;
        let compact = placement.compact;
        self.open_with(
            placement,
            move |dismiss, cx| {
                let view = cx.new(|cx| {
                    let menu = MenuView::new(entries, dismiss, cx).compact(compact);
                    match min_width {
                        Some(width) => menu.min_width(width),
                        None => menu,
                    }
                });
                let focus = view.read(cx).focus_handle();
                (view.into(), Some(focus))
            },
            window,
            cx,
        );
    }

    /// Opens an arbitrary popover view in the same anchored layer the menus
    /// use. The builder receives the dismiss handler the view must call when
    /// it wants to close, and returns an optional focus handle to focus.
    pub fn open_with(
        &self,
        placement: MenuPlacement,
        build: impl FnOnce(DismissHandler, &mut App) -> (AnyView, Option<gpui::FocusHandle>),
        window: &mut Window,
        cx: &mut App,
    ) {
        let shared = self.0.clone();
        let dismiss: DismissHandler = Rc::new(move |window: &mut Window, _cx: &mut App| {
            let mut state = shared.borrow_mut();
            let owner = state.popup.as_ref().and_then(|popup| popup.owner.clone());
            if let Some(popup) = state.popup.as_mut() {
                popup.closing_at = Some(std::time::Instant::now());
            }
            state.suppressed = owner;
            drop(state);
            window.refresh();
        });
        let (view, focus) = build(dismiss, cx);
        if let Some(focus) = focus {
            window.focus(&focus);
        }
        let mut state = self.0.borrow_mut();
        state.suppressed = None;
        state.next_animation_id = state.next_animation_id.wrapping_add(1);
        state.popup = Some(MenuPopup {
            view,
            owner: placement.owner,
            position: placement.position,
            anchor: placement.anchor,
            offset: placement.offset,
            animation_id: state.next_animation_id,
            closing_at: None,
        });
        drop(state);
        window.refresh();
    }

    pub fn toggle_with(
        &self,
        placement: MenuPlacement,
        build: impl FnOnce(DismissHandler, &mut App) -> (AnyView, Option<gpui::FocusHandle>),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.toggled_closed(&placement, window) {
            return;
        }
        self.open_with(placement, build, window, cx);
    }

    /// Renders the layer for a window-anchored menu opened with `open_at`.
    pub fn render(&self) -> Option<AnyElement> {
        self.finish_closing();
        self.clear_stale_suppression();
        let borrowed = self.0.borrow();
        let popup = borrowed.popup.as_ref()?;
        if popup.owner.is_some() {
            return None;
        }
        Some(layer(popup))
    }

    /// A dismissal only guards the press that caused it; once a frame has been
    /// drawn with nothing open, the trigger is free to open again.
    fn clear_stale_suppression(&self) {
        let mut state = self.0.borrow_mut();
        if state.popup.is_none() {
            state.suppressed = None;
        }
    }

    /// Renders the layer for the dropdown owned by `owner`, anchored to the
    /// trigger that calls it. The trigger must be `relative()`.
    pub fn render_dropdown(&self, owner: &str) -> AnyElement {
        self.finish_closing();
        let borrowed = self.0.borrow();
        let Some(popup) = borrowed.popup.as_ref().filter(|popup| {
            popup
                .owner
                .as_ref()
                .is_some_and(|value| value.as_ref() == owner)
        }) else {
            drop(borrowed);
            self.clear_stale_suppression();
            return div().into_any_element();
        };
        let hangs_below = matches!(popup.anchor, Corner::TopLeft | Corner::TopRight);
        div()
            .absolute()
            .left_0()
            .w_0()
            .h_0()
            .when(hangs_below, |el| el.bottom_0())
            .when(!hangs_below, |el| el.top_0())
            .child(layer(popup))
            .into_any_element()
    }

    fn finish_closing(&self) {
        let mut state = self.0.borrow_mut();
        let done = state
            .popup
            .as_ref()
            .and_then(|popup| popup.closing_at)
            .is_some_and(|started| {
                started.elapsed()
                    >= std::time::Duration::from_millis(crate::ui::primitives::OVERLAY_EXIT_MS)
            });
        if done {
            state.popup = None;
            state.suppressed = None;
        }
    }
}

fn layer(popup: &MenuPopup) -> AnyElement {
    let closing = popup.closing_at.is_some_and(|closing_at| {
        closing_at.elapsed()
            < std::time::Duration::from_millis(crate::ui::primitives::OVERLAY_EXIT_MS)
    });
    let view = if closing {
        crate::ui::primitives::overlay_exit(
            ("menu-exit", popup.animation_id),
            crate::ui::primitives::EnterFrom::Top,
            div().child(popup.view.clone()),
        )
        .into_any_element()
    } else {
        div().child(popup.view.clone()).into_any_element()
    };
    let mut anchor = anchored()
        .anchor(popup.anchor)
        .offset(popup.offset)
        .snap_to_window_with_margin(px(8.0))
        .child(view);
    if let Some(position) = popup.position {
        anchor = anchor.position(position);
    }
    deferred(anchor).with_priority(1).into_any_element()
}
