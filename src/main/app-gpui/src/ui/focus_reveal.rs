use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, Along, AnyElement, App, Axis, ElementId, FocusHandle, ScrollHandle, Window,
};
use herogpui::gpui;

pub struct FocusReveal {
    pub scroll: ScrollHandle,
    changed: bool,
    axis: Axis,
}

impl FocusReveal {
    pub fn new(id: &'static str, window: &mut Window, cx: &mut App) -> Self {
        Self::for_axis(id, Axis::Horizontal, None, window, cx)
    }

    pub fn vertical(
        id: &'static str,
        scroll: &ScrollHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        Self::for_axis(id, Axis::Vertical, Some(scroll), window, cx)
    }

    fn for_axis(
        id: &'static str,
        axis: Axis,
        scroll: Option<&ScrollHandle>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let state = window.use_keyed_state(ElementId::Name(id.into()), cx, |_, _| {
            (
                scroll.cloned().unwrap_or_else(ScrollHandle::new),
                Rc::new(RefCell::new(None::<FocusHandle>)),
            )
        });
        let (scroll, last_focus) = state.read(cx);
        let focus = window.focused(cx);
        let changed = *last_focus.borrow() != focus;
        *last_focus.borrow_mut() = focus;
        Self {
            scroll: scroll.clone(),
            changed,
            axis,
        }
    }

    pub fn wrap(
        &self,
        id: String,
        child: AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let scope =
            window.use_keyed_state(ElementId::Name(id.into()), cx, |_, cx| cx.focus_handle());
        let scope = scope.read(cx).clone();
        let focused = self.changed
            && (self.axis == Axis::Horizontal || window.last_input_was_keyboard())
            && scope.contains_focused(window, cx);
        let scroll = self.scroll.clone();
        let axis = self.axis;
        let wrapper = div().flex().flex_shrink_0();
        let wrapper = match axis {
            Axis::Horizontal => wrapper,
            Axis::Vertical => wrapper.flex_col(),
        };
        wrapper
            .track_focus(&scope)
            .child(child)
            .on_children_prepainted(move |children, window, _| {
                if !focused {
                    return;
                }
                let Some(bounds) = children.first() else {
                    return;
                };
                let viewport = scroll.bounds();
                let margin = px(4.);
                let start = bounds.origin.along(axis);
                let end = start + bounds.size.along(axis);
                let viewport_start = viewport.origin.along(axis);
                let viewport_end = viewport_start + viewport.size.along(axis);
                let delta = if start < viewport_start + margin {
                    viewport_start + margin - start
                } else if end > viewport_end - margin {
                    viewport_end - margin - end
                } else {
                    px(0.)
                };
                if delta != px(0.) {
                    let offset = scroll.offset();
                    let next = offset.apply_along(axis, |value| {
                        (value + delta).clamp(-scroll.max_offset().along(axis), px(0.))
                    });
                    if next != offset {
                        scroll.set_offset(next);
                        window.refresh();
                    }
                }
            })
            .into_any_element()
    }
}
