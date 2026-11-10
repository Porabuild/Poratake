//! `extend::app_focus_root` claims the focus only when nothing holds it.
//!
//! A handle focused before its first paint is not yet inside the root's
//! rendered frame, so "the root does not contain the focus" is not the same
//! as "nothing is focused". The root must leave such a handle alone, take the
//! focus when the window has none, and take it back when the focused element
//! has left the tree.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{prelude::*, px, Context, FocusHandle, TestAppContext, VisualTestContext, Window};
use herogpui_components::extend;
use herogpui_theme::ThemeProvider;

use crate::harness::{events, Events};

/// A root with two tab stops and an optional third element that holds
/// `inner`, the handle under test.
struct FocusView {
    inner: FocusHandle,
    first: FocusHandle,
    second: FocusHandle,
    show_inner: Rc<Cell<bool>>,
    keys: Events,
}

impl Render for FocusView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let keys = self.keys.clone();
        let mut root = gpui::div()
            .size_full()
            .child(gpui::div().size(px(10.)).track_focus(&self.first))
            .child(gpui::div().size(px(10.)).track_focus(&self.second));
        if self.show_inner.get() {
            root = root.child(
                gpui::div()
                    .size(px(10.))
                    .track_focus(&self.inner)
                    .on_key_down(move |event, _, _| {
                        keys.borrow_mut()
                            .push(format!("inner:{}", event.keystroke.key));
                    }),
            );
        }
        extend::app_focus_root(root, window, cx)
    }
}

fn open(
    cx: &mut TestAppContext,
    focus_inner_on_open: bool,
    show_inner: Rc<Cell<bool>>,
    keys: Events,
) -> (gpui::Entity<FocusView>, &mut VisualTestContext) {
    cx.update(ThemeProvider::init);
    cx.add_window_view(move |window, cx| {
        let inner = cx.focus_handle();
        if focus_inner_on_open {
            window.focus(&inner, cx);
        }
        FocusView {
            inner,
            first: cx.focus_handle().tab_stop(true),
            second: cx.focus_handle().tab_stop(true),
            show_inner,
            keys,
        }
    })
}

fn frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn is_focused(
    cx: &mut VisualTestContext,
    view: &gpui::Entity<FocusView>,
    pick: fn(&FocusView) -> &FocusHandle,
) -> bool {
    cx.update(|window, cx| pick(view.read(cx)).is_focused(window))
}

#[gpui::test]
fn a_handle_focused_before_the_first_paint_keeps_the_focus(cx: &mut TestAppContext) {
    let keys = events();
    let (view, cx) = open(cx, true, Rc::new(Cell::new(true)), keys.clone());
    frame(cx);
    frame(cx);
    assert!(
        is_focused(cx, &view, |v| &v.inner),
        "the root stole a focus the view claimed while its window opened"
    );

    cx.simulate_keystrokes("a down escape");
    assert_eq!(
        *keys.borrow(),
        ["inner:a", "inner:down", "inner:escape"],
        "key events stop reaching the focused handle"
    );
    assert!(is_focused(cx, &view, |v| &v.inner));
}

#[gpui::test]
fn with_nothing_focused_the_root_takes_the_focus_and_tab_reaches_the_first_stop(
    cx: &mut TestAppContext,
) {
    let (view, cx) = open(cx, false, Rc::new(Cell::new(true)), events());
    frame(cx);
    assert!(
        cx.update(|window, cx| window.focused(cx).is_some()),
        "the root must hold the focus when nothing else does"
    );
    assert!(!is_focused(cx, &view, |v| &v.first));

    cx.simulate_keystrokes("tab");
    assert!(
        is_focused(cx, &view, |v| &v.first),
        "the first Tab must reach the first tab stop"
    );
    cx.simulate_keystrokes("tab");
    assert!(is_focused(cx, &view, |v| &v.second));
    cx.simulate_keystrokes("shift-tab");
    assert!(is_focused(cx, &view, |v| &v.first));
}

#[gpui::test]
fn focus_on_an_element_that_left_the_tree_returns_to_the_root(cx: &mut TestAppContext) {
    let show_inner = Rc::new(Cell::new(true));
    let (view, cx) = open(cx, true, show_inner.clone(), events());
    frame(cx);
    assert!(is_focused(cx, &view, |v| &v.inner));

    show_inner.set(false);
    frame(cx);
    frame(cx);
    assert!(
        !is_focused(cx, &view, |v| &v.inner),
        "focus on an unmounted element is focus on nothing"
    );

    cx.simulate_keystrokes("tab");
    assert!(
        is_focused(cx, &view, |v| &v.first),
        "Tab must still move the focus once its element is gone"
    );
}

/// The root inside the shared harness, with a caller-owned handle that the
/// test drops: gpui reports a released handle as no focus at all.
struct DroppedView {
    held: Rc<RefCell<Option<FocusHandle>>>,
    first: FocusHandle,
}

impl Render for DroppedView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mut root = gpui::div()
            .size_full()
            .child(gpui::div().size(px(10.)).track_focus(&self.first));
        if let Some(held) = self.held.borrow().as_ref() {
            root = root.child(gpui::div().size(px(10.)).track_focus(held));
        }
        extend::app_focus_root(root, window, cx)
    }
}

#[gpui::test]
fn focus_on_a_released_handle_returns_to_the_root(cx: &mut TestAppContext) {
    cx.update(ThemeProvider::init);
    let held = Rc::new(RefCell::new(None));
    let slot = held.clone();
    let (view, cx) = cx.add_window_view(move |window, cx| {
        let handle = cx.focus_handle();
        window.focus(&handle, cx);
        *slot.borrow_mut() = Some(handle);
        DroppedView {
            held: slot,
            first: cx.focus_handle().tab_stop(true),
        }
    });
    frame(cx);

    held.borrow_mut().take();
    frame(cx);
    frame(cx);
    assert!(
        cx.update(|window, cx| window.focused(cx).is_some()),
        "the root must take the focus a released handle gave up"
    );
    cx.simulate_keystrokes("tab");
    assert!(cx.update(|window, cx| view.read(cx).first.is_focused(window)));
}
