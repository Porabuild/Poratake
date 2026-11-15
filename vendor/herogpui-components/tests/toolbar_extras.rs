//! Toolbar extras (HeroGPUI extension, after gpui-kit's toolbar): `size`
//! reaches every `sized_child` whatever the order of the builder calls,
//! plain `child` elements keep their own size, and a sized child keeps its
//! own size when the toolbar sets none.

use crate::harness;

use gpui::{prelude::*, px, TestAppContext, VisualTestContext};
use harness::open_host;
use herogpui_components::{Button, Size, Toolbar};

/// Renders each toolbar in its own measured row.
fn host(cx: &mut TestAppContext, bars: fn() -> Vec<Toolbar>) -> &mut VisualTestContext {
    let cx = open_host(cx, move || {
        let mut col = gpui::div().flex().flex_col().gap(px(10.));
        for (ix, bar) in bars().into_iter().enumerate() {
            col = col.child(
                gpui::div()
                    .flex()
                    .debug_selector(move || format!("bar-{ix}"))
                    .child(bar),
            );
        }
        col.into_any_element()
    });
    for _ in 0..2 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    cx
}

fn height(cx: &mut VisualTestContext, ix: usize) -> gpui::Pixels {
    cx.debug_bounds(Box::leak(format!("bar-{ix}").into_boxed_str()))
        .expect("bar laid out")
        .size
        .height
}

#[gpui::test]
fn the_toolbar_size_reaches_sized_children_in_any_builder_order(cx: &mut TestAppContext) {
    let cx = host(cx, || {
        vec![
            // Size before the child.
            Toolbar::new()
                .size(Size::Sm)
                .sized_child(Button::new("a").label("A")),
            // Size after the child.
            Toolbar::new()
                .sized_child(Button::new("b").label("B"))
                .size(Size::Sm),
            Toolbar::new()
                .sized_child(Button::new("c").label("C"))
                .size(Size::Lg),
            // No toolbar size: the button's own (medium) size.
            Toolbar::new().sized_child(Button::new("d").label("D")),
            // A plain child is not resized by the toolbar.
            Toolbar::new()
                .size(Size::Sm)
                .child(Button::new("e").label("E")),
        ]
    });
    let (sm_before, sm_after, lg, own, plain) = (
        height(cx, 0),
        height(cx, 1),
        height(cx, 2),
        height(cx, 3),
        height(cx, 4),
    );
    assert_eq!(sm_before, sm_after, "builder order does not matter");
    assert!(
        sm_before < own && own < lg,
        "{sm_before:?} < {own:?} < {lg:?}"
    );
    assert_eq!(plain, own, "`child` keeps the element's own size");
}

#[gpui::test]
fn sized_children_size_every_control(cx: &mut TestAppContext) {
    let cx = host(cx, || {
        vec![
            Toolbar::new()
                .size(Size::Lg)
                .sized_children([Button::new("x").label("X"), Button::new("y").label("Y")]),
            Toolbar::new()
                .size(Size::Lg)
                .sized_child(Button::new("z").label("Z")),
        ]
    });
    assert_eq!(height(cx, 0), height(cx, 1));
}
