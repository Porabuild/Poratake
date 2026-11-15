//! Phase 6 layout seams: each container's `full_width` stores the flag and
//! expands its root, without redistributing the children; ComboBox's root
//! floor and a vertical Tabs list's per-tab floor are replaceable.
//!
//! Measured, not read from source: each container sits at the start of a
//! 400px flex row followed by a 10px probe. A root that hugs its content
//! leaves the probe right after that content; a root that took `w_full`
//! claims the row, which pushes the probe to its far end. The first child
//! stays where it was either way, since the flag widens the root only.

use crate::harness;

use gpui::{prelude::*, px, AnyElement, TestAppContext};
use harness::{open_host, probe, settle, still};
use herogpui_components::{
    Breadcrumbs, Button, Crumb, Pagination, RadioGroup, RadioOption, Tag, TagGroup, Toolbar,
};

const ROW: f32 = 400.;

type Build = fn(bool) -> AnyElement;

fn cases() -> Vec<(&'static str, Build)> {
    vec![
        ("TagGroup", |full| {
            TagGroup::new("tg", vec![Tag::new("a", "Alpha")])
                .full_width(full)
                .into_any_element()
        }),
        ("Pagination", |full| {
            Pagination::new("pg", 1, 3)
                .full_width(full)
                .into_any_element()
        }),
        ("Breadcrumbs", |full| {
            Breadcrumbs::new(vec![Crumb::new("Home"), Crumb::new("Docs")])
                .full_width(full)
                .into_any_element()
        }),
        ("RadioGroup", |full| {
            RadioGroup::new("rg", vec![RadioOption::new("One")])
                .full_width(full)
                .into_any_element()
        }),
        ("Toolbar", |full| {
            Toolbar::new()
                .id("tb")
                .full_width(full)
                .child(Button::new("b").label("Bold").into_any_element())
                .into_any_element()
        }),
    ]
}

/// The probe's left edge after one render of `build(full)`.
fn probe_x(cx: &mut TestAppContext, build: Build, full: bool) -> f32 {
    still();
    let vcx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_row()
            .items_start()
            .w(px(ROW))
            .child(build(full))
            .child(probe("after"))
            .into_any_element()
    });
    settle(vcx);
    let after = vcx
        .debug_bounds("after")
        .expect("the trailing probe must be laid out");
    f32::from(after.origin.x)
}

#[gpui::test]
fn full_width_builders_reach_their_roots(cx: &mut TestAppContext) {
    for (name, build) in cases() {
        let hugging = probe_x(cx, build, false);
        assert!(
            hugging < ROW / 2.,
            "{name}: without the flag the root must hug its content, the \
             probe sat at x={hugging}"
        );
        let full = probe_x(cx, build, true);
        assert!(
            full > ROW - 20.,
            "{name}: `full_width(true)` must expand the root across the row, \
             the probe sat at x={full} (hugging: {hugging})"
        );
    }
}

/// The probe's left edge after a ComboBox root with `min_width`, or the
/// stock floor for `None`. No placeholder, so the root's own content is far
/// narrower than any floor and the floor alone sets the width.
fn combo_box_width(cx: &mut TestAppContext, min_width: Option<f32>, full: bool) -> f32 {
    use herogpui_components::{ComboBox, InputState, PickerItem};
    still();
    let state = cx.new(|cx| InputState::new(cx));
    let vcx = open_host(cx, move || {
        let combo =
            ComboBox::new(state.clone(), vec![PickerItem::new("a", "Alpha")]).full_width(full);
        let combo = match min_width {
            Some(w) => combo.min_width(px(w)),
            None => combo,
        };
        gpui::div()
            .flex()
            .flex_row()
            .items_start()
            // A full-width combo box fills this box, so the probe after it
            // reads the container width, not a floor.
            .child(gpui::div().when(full, |d| d.w(px(100.))).child(combo))
            .child(probe("after"))
            .into_any_element()
    });
    settle(vcx);
    f32::from(
        vcx.debug_bounds("after")
            .expect("the trailing probe must be laid out")
            .origin
            .x,
    )
}

#[gpui::test]
fn combo_box_min_width_replaces_the_180px_root_floor(cx: &mut TestAppContext) {
    let stock = combo_box_width(cx, None, false);
    assert!(
        (stock - 180.).abs() < 0.5,
        "stock floor is 180px, got {stock}"
    );
    let narrow = combo_box_width(cx, Some(120.), false);
    assert!((narrow - 120.).abs() < 0.5, "a 120px floor, got {narrow}");
    let wide = combo_box_width(cx, Some(260.), false);
    assert!((wide - 260.).abs() < 0.5, "a 260px floor, got {wide}");
    // Full width keeps its contract: no floor, the container decides.
    let full = combo_box_width(cx, Some(260.), true);
    assert!(
        (full - 100.).abs() < 0.5,
        "a full-width combo box ignores the floor, got {full}"
    );
}

/// The width a vertical tab's content box spans: the trigger slot holds a
/// full-width probe, so its width is the tab's width less its fixed inline
/// padding, and the difference between two floors is the floors' difference.
fn vertical_tab_content_width(cx: &mut TestAppContext, floor: Option<f32>) -> f32 {
    use gpui::{canvas, Bounds, Pixels};
    use herogpui_components::{Orientation, TabItem, Tabs};
    use std::cell::Cell;
    use std::rc::Rc;
    still();
    let seen: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
    let sink = seen.clone();
    let vcx = open_host(cx, move || {
        let sink = sink.clone();
        let slot = gpui::div()
            .w_full()
            .h(px(4.))
            .child(canvas(move |bounds, _, _| sink.set(Some(bounds)), |_, _, _, _| {}).size_full());
        let tabs = Tabs::new(
            "vertical-floor",
            vec![TabItem::new("a", "A").trigger(slot), TabItem::new("b", "B")],
            "a",
        )
        .orientation(Orientation::Vertical);
        match floor {
            Some(w) => tabs.vertical_tab_min_width(px(w)),
            None => tabs,
        }
        .into_any_element()
    });
    settle(vcx);
    f32::from(
        seen.get()
            .expect("the trigger slot must be laid out")
            .size
            .width,
    )
}

#[gpui::test]
fn vertical_tab_min_width_replaces_the_80px_tab_floor(cx: &mut TestAppContext) {
    let stock = vertical_tab_content_width(cx, None);
    let wide = vertical_tab_content_width(cx, Some(200.));
    assert!(
        (wide - stock - 120.).abs() < 0.5,
        "a 200px floor widens the tab by 120px over the 80px one \
         (stock content {stock}, wide {wide})"
    );
    let narrow = vertical_tab_content_width(cx, Some(40.));
    // Below the floor the one-letter labels and the 16px inline padding
    // set the width, so the drop is most of the 40px but not exactly it.
    assert!(
        narrow < stock - 30.,
        "a 40px floor lets one-letter tabs narrow (stock {stock}, narrow {narrow})"
    );
}
