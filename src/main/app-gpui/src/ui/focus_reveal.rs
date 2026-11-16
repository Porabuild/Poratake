use std::cell::RefCell;
use std::rc::Rc;

use gpui::{div, prelude::*, px, AnyElement, App, ElementId, FocusHandle, ScrollHandle, Window};
use herogpui::gpui;

pub struct FocusReveal {
    pub scroll: ScrollHandle,
    changed: bool,
}

impl FocusReveal {
    pub fn new(id: &'static str, window: &mut Window, cx: &mut App) -> Self {
        let state = window.use_keyed_state(ElementId::Name(id.into()), cx, |_, _| {
            (
                ScrollHandle::new(),
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
        let focused = self.changed && scope.contains_focused(window, cx);
        let scroll = self.scroll.clone();
        div()
            .flex()
            .flex_shrink_0()
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
                let delta = if bounds.left() < viewport.left() + margin {
                    viewport.left() + margin - bounds.left()
                } else if bounds.right() > viewport.right() - margin {
                    viewport.right() - margin - bounds.right()
                } else {
                    px(0.)
                };
                if delta != px(0.) {
                    let mut offset = scroll.offset();
                    let next = (offset.x + delta).clamp(-scroll.max_offset().x, px(0.));
                    if next != offset.x {
                        offset.x = next;
                        scroll.set_offset(offset);
                        window.refresh();
                    }
                }
            })
            .into_any_element()
    }
}
