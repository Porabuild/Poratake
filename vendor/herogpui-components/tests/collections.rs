//! Behaviour tests for the collection and navigation components: ListBox,
//! TagGroup, Tabs, Accordion, Pagination and Breadcrumbs.
//!
//! Everything static about them is measured by the `.shots/*.py` audits; these
//! tests drive the controls and assert on recorded callbacks and behavioural
//! probes only — never on appearance.
//!
//! Geometry is derived from the components' own constants, not guessed. Where a
//! click target's x depends on a label's width (Tabs, Breadcrumbs), the width is
//! *measured* with the same text system the renderer shapes with
//! (`Window::text_system().shape_line`), so the derivation holds for the machine
//! the tests run on instead of assuming one font metric:
//!
//! - ListBox: `.list-box` is `p-1` (4px) with `mt-1` (4px) between children and
//!   each row is `min-h(util::FIELD_HEIGHT)` = 36px, so row *i*'s centre sits at
//!   y = 4 + i*(36+4) + 18 = 22 + 40i. Rows stretch the full window width.
//! - TagGroup: `.tag--md` is `px-2 py-1` (8/4), the list gaps chips by 6px, and
//!   the remove button is `size-3` (12px). The test's `tag_content` draws the
//!   label as a fixed 40x20 box so *every* distance is a constant: a chip is
//!   8+40+4+12+8 = 72px wide and 20+8 = 28px tall, and its remove button spans
//!   x 52..64 at y 8..20 — centre (58, 14); chip *i* starts at 78i.
//! - Tabs: the list is `p-1` with the tabs shoulder to shoulder, each tab
//!   `h-8 px-4` (32px tall, 16px side padding). Tab *i* is `w_i + 32` wide where
//!   `w_i` is the measured label width, so tab 1's centre is at
//!   4 + w_0 + 32 + (w_1 + 32)/2 at y = 4 + 16 = 20.
//! - Accordion: a trigger is `px-4 py-4` (16px all round) around a single line
//!   of `line_height(px(20.))`, so it is 52px tall; items are joined by a 1px
//!   separator. Closed headers *i* centre at 26 + 53i. In the single-expand test
//!   each body is given a fixed 40px-tall content so the second header's centre
//!   is exactly 52 + (2+40+16) + 1 + 26 = 137.
//! - Pagination: `size-md` cells are 32px squares; a nav button is `px-2.5`
//!   (10px each side) around a 14px glyph, so 34px wide; the row gaps items by
//!   4px. Prev spans x 0..34, then page *n*'s cell starts at 38+36(n-1), and the
//!   next button starts at 146 (three cells later) and centres at 163; y = 16.
//! - Breadcrumbs (v3.2.6): each crumb row is the unpadded measured label plus
//!   `gap-1` (4px) and a 12px separator slot, and the root gaps rows by
//!   `gap-1.5` (6px), so the second label starts at w_0 + 22 and a label
//!   starts flush in its row; the label line is `leading-5` = 20px, so the
//!   centre y is 10.
//!
//! Each instance gets its own element id; two components sharing an id share
//! their keyed state, which AGENTS.md documents as a silent failure.

use crate::harness;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::{rc::Rc, time::Duration};

use gpui::{
    prelude::*, px, Font, FontFeatures, FontStyle, FontWeight, TestAppContext, VisualTestContext,
};
use herogpui_components::{
    extend::FIELD_HEIGHT, Accordion, AccordionItem, Breadcrumbs, Crumb, ListBox, ListBoxItem,
    Pagination, SelectionMode, TabItem, Tabs, Tag, TagGroup,
};

use harness::{click, events, open_host, press};

/// The y centre of ListBox row `i`.
///
/// Derived from the component's own numbers, in the order the code applies
/// them: `.list-box` is `p-1` (4px top padding), each row is
/// `min-h(util::FIELD_HEIGHT)` = 36px tall, and the children are separated by
/// `mt-1` = 4px (`gap`), so row *i* sits at 4 + i*(36 + 4) and its centre is
/// half a row further down.
fn list_row_centre(i: usize) -> f32 {
    let row = f32::from(FIELD_HEIGHT);
    4. + i as f32 * (row + 4.) + row / 2.
}

/// The keys of a selection joined in a stable order.
///
/// A `HashSet` iterates in no particular order, so asserting on a raw join
/// would be flaky; sorting makes the recorded report deterministic.
fn sorted_join(keys: &HashSet<gpui::SharedString>) -> String {
    let mut keys: Vec<String> = keys.iter().map(ToString::to_string).collect();
    keys.sort();
    keys.join(",")
}

/// Drives the measured Accordion panel to its settled endpoint before the
/// second header's coordinate is used. Since gpui-pre 0.3.11 GPUI's animation
/// phase and the retained exit timer both use the test executor clock.
fn settle_collapsible(cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_millis(220));
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    cx.executor().advance_clock(Duration::from_millis(210));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
}

/// The advance width of `text` shaped the way the components shape it: gpui's
/// default `.SystemUIFont` stack at `size` px and `weight`.
///
/// Tabs labels are `text_size(14)` + MEDIUM; breadcrumb labels are 14px at
/// MEDIUM (`.breadcrumbs__link` is `font-medium`). Both are laid out by the
/// window's own `WindowTextSystem`, so this measurement is the render's
/// measurement.
fn text_width(system: &gpui::WindowTextSystem, text: &str, size: f32, weight: FontWeight) -> f32 {
    let run = gpui::TextRun {
        len: text.len(),
        font: Font {
            family: ".SystemUIFont".into(),
            features: FontFeatures::default(),
            weight,
            style: FontStyle::default(),
            fallbacks: None,
        },
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = system.shape_line(text.to_owned().into(), px(size), &[run], None);
    f32::from(line.width)
}

// ---------------------------------------------------------------------------
// ListBox
// ---------------------------------------------------------------------------
//
// Row geometry, from ListBox::new's own comment (`.list-box` is `p-1` with
// `mt-1` between children) and the row code (`min-h(util::FIELD_HEIGHT)`):
// row *i* occupies y 4+40i .. 44+40i, centre y = 22 + 40i. All clicks use
// x = 60, well inside a full-width row.

#[gpui::test]
fn list_box_click_selects_and_arrows_move(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        ListBox::new(
            "lb-single",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
                ListBoxItem::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Single)
        .on_selection_change(move |keys, _, _| events.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    // `list_row_centre(0)` = 4 + 0 + 36/2 = y 22. The report carries the
    // row's *key* (the value v3 hands `onSelectionChange`), here "alpha".
    click(cx, 60., list_row_centre(0));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha"],
        "clicking the first row must select it"
    );

    // The click's mouse-down focused Alpha, so the arrows walk from that row.
    // Two Downs reach Gamma, which Enter then takes.
    press(cx, "down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "gamma"],
        "Down Down Enter must move from Alpha to Gamma and choose it"
    );
}

#[gpui::test]
fn list_box_multiple_selection_accumulates(cx: &mut TestAppContext) {
    // ListBox is the *controlled* kind (like Select): `selected_keys` is a
    // prop and `on_selection_change` hands the full next set back, so the
    // caller owns the selection and feeds it to the next render. The test
    // plays that role, which is also what makes the third click's toggle
    // meaningful: the row can only remove Beta's pick if it was told Alpha and
    // Beta were selected.
    let selection: Rc<RefCell<HashSet<gpui::SharedString>>> = Rc::new(RefCell::new(HashSet::new()));
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let selection = selection.clone();
        let events = events.clone();
        let held = selection.borrow().clone();
        ListBox::new(
            "lb-multi",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
                ListBoxItem::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Multiple)
        .selected_keys(held)
        .on_selection_change(move |keys, _, _| {
            events.borrow_mut().push(sorted_join(keys));
            *selection.borrow_mut() = keys.clone();
        })
        .into_any_element()
    });

    // Rows 0 and 1 centre at y 22 and 62 (`list_row_centre`); reports carry
    // the row keys, and each one is the *whole* held selection.
    click(cx, 60., list_row_centre(0));
    assert_eq!(recorded.borrow().as_slice(), ["alpha"]);
    click(cx, 60., list_row_centre(1));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,beta"],
        "a second click must report the first pick and the second together"
    );
    click(cx, 60., list_row_centre(0));
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "alpha,beta", "beta"],
        "re-clicking a picked row must toggle it off, which requires the \
         row to have been told it was already selected"
    );
}

#[gpui::test]
fn list_box_typeahead_moves_the_cursor(cx: &mut TestAppContext) {
    // The typeahead lives in `crate::list_nav`, which AGENTS.md describes as the
    // listbox/menu/select sharing one search. The cursor itself is internal, so
    // the assertion is behavioural: type a letter, then Enter must select the
    // row the letter found.
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        ListBox::new(
            "lb-typeahead",
            vec![
                ListBoxItem::new("rust", "Rust"),
                ListBoxItem::new("belgium", "Belgium"),
                ListBoxItem::new("brazil", "Brazil"),
                ListBoxItem::new("go", "Go"),
            ],
        )
        .selection_mode(SelectionMode::Single)
        .on_selection_change(move |keys, _, _| events.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    // Row 0 at `list_row_centre(0)`. Clicking also gives the list the focus
    // the keys need; the report carries the key "rust".
    click(cx, 60., list_row_centre(0));
    assert_eq!(recorded.borrow().as_slice(), ["rust"]);

    // "b" must search the *labels* and move the cursor to the first row
    // starting with b — Belgium is index 1, not the clicked row 0. Enter then
    // takes it, reporting the key "belgium".
    press(cx, "b");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["rust", "belgium"],
        "typing must move the keyboard to the matching row, which Enter then chooses"
    );
}

#[gpui::test]
fn list_box_disabled_key_cannot_be_chosen(cx: &mut TestAppContext) {
    let events = events();
    let recorded = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        ListBox::new(
            "lb-disabled",
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
                ListBoxItem::new("gamma", "Gamma"),
            ],
        )
        .selection_mode(SelectionMode::Single)
        .disabled_keys(["beta".into()])
        .on_selection_change(move |keys, _, _| events.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    // Row 1 (Beta, disabled) at `list_row_centre(1)`: nothing may record.
    click(cx, 60., list_row_centre(1));
    assert!(
        recorded.borrow().is_empty(),
        "clicking a disabledKeys row must record nothing"
    );

    // Row 0 at `list_row_centre(0)` selects Alpha and puts the list in the key
    // chain. From the top, the first Down lands the cursor on Alpha and the
    // second steps to the next *stop* — which skips disabled Beta and lands on
    // Gamma.
    click(cx, 60., list_row_centre(0));
    press(cx, "down down");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha", "gamma"],
        "the arrows must skip a disabledKeys row: Enter after two Downs \
         chooses Gamma, never Beta"
    );
}

/// A plain ListBox is nonselecting. Pinned `useListState`
/// (`useMultipleSelectionState`) defaults `selectionMode` to `none` and
/// HeroUI's v3 ListBox wrapper forwards props to React Aria untouched, so
/// clicks and activation keys must neither report a selection nor seat one —
/// which is also what withholds the item indicator (`isSelected` drives both).
/// The same list with an explicit `single` mode still selects.
#[gpui::test]
fn list_box_plain_default_does_not_select(cx: &mut TestAppContext) {
    let recorded = events();
    let recorded_for_view = recorded.clone();
    // Per-row `isSelected` snapshots for the plain and explicit-single lists.
    let plain_selected: Rc<RefCell<HashMap<String, bool>>> = Rc::new(RefCell::new(HashMap::new()));
    let single_selected: Rc<RefCell<HashMap<String, bool>>> = Rc::new(RefCell::new(HashMap::new()));
    let plain_for_view = plain_selected.clone();
    let single_for_view = single_selected.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded_for_view.clone();
        let plain_for_view = plain_for_view.clone();
        let single_for_view = single_for_view.clone();
        let items = || {
            vec![
                ListBoxItem::new("alpha", "Alpha"),
                ListBoxItem::new("beta", "Beta"),
                ListBoxItem::new("gamma", "Gamma"),
            ]
        };
        // The plain list occupies y 0..124 (three 36px rows in `p-1` with
        // `mt-1` gaps); the explicit-single list starts at 124, so its first
        // row centres at 124 + `list_row_centre(0)`.
        gpui::div()
            .flex()
            .flex_col()
            .child(
                ListBox::new("lb-plain-default", items())
                    .item_content(move |key, state| {
                        plain_for_view
                            .borrow_mut()
                            .insert(key.to_string(), state.is_selected);
                        gpui::div().w(px(40.)).h(px(20.)).into_any_element()
                    })
                    .on_selection_change({
                        let recorded = recorded.clone();
                        move |keys, _, _| recorded.borrow_mut().push(sorted_join(keys))
                    })
                    .into_any_element(),
            )
            .child(
                ListBox::new("lb-explicit-single", items())
                    .selection_mode(SelectionMode::Single)
                    .item_content(move |key, state| {
                        single_for_view
                            .borrow_mut()
                            .insert(key.to_string(), state.is_selected);
                        gpui::div().w(px(40.)).h(px(20.)).into_any_element()
                    })
                    .on_selection_change(move |keys, _, _| {
                        recorded.borrow_mut().push(sorted_join(keys));
                    })
                    .into_any_element(),
            )
            .into_any_element()
    });

    // Click, Enter and Space against the plain list must leave it inert.
    click(cx, 60., list_row_centre(0));
    press(cx, "enter");
    press(cx, "space");
    cx.update(|window, _| window.refresh());
    assert!(
        recorded.borrow().is_empty(),
        "a plain ListBox must never report a selection"
    );
    assert!(
        plain_selected.borrow().values().all(|selected| !selected),
        "no plain ListBox row may ever see isSelected"
    );

    // The same list with an explicit single mode still selects on click.
    click(cx, 60., 124. + list_row_centre(0));
    cx.update(|window, _| window.refresh());
    assert_eq!(
        recorded.borrow().as_slice(),
        ["alpha"],
        "an explicit single mode must still select the clicked row"
    );
    assert!(
        single_selected.borrow()["alpha"],
        "the explicitly single list must seat the pick in the row state"
    );
    assert!(
        !single_selected.borrow()["beta"] && !single_selected.borrow()["gamma"],
        "the pick must not spill onto the unpicked rows"
    );
}

/// Resizes the window and drains the layout queue, so a bounded virtual list
/// re-resolves its viewport before the next probe reads it.
fn settle_window(cx: &mut VisualTestContext, width: f32, height: f32) {
    cx.simulate_resize(gpui::size(px(width), px(height)));
    for _ in 0..4 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

/// `debug_bounds` wants a `&'static str`; the virtual viewport's probe name
/// is the component's own `{base}-rows` selector, so one short leak per call.
fn virtual_rows_probe(list_id: &'static str) -> &'static str {
    let name = format!("{:?}-rows", gpui::ElementId::from(list_id));
    let leaked: &'static mut str = Box::leak(name.into_boxed_str());
    &*leaked
}

/// Pinned `ListKeyboardDelegate` pages a virtual list by one *visible*
/// rectangle. A window shorter than this list's 160px cap shows fewer 40px
/// rows, so PageDown must step by the shown viewport -- two rows here -- and
/// not by the cap's three-row ruler; the old ruler lands a row too far at
/// every step and skips past the disabled row differently. Growing the window
/// restores the cap step, and a near-zero viewport keeps the cursor in place.
#[gpui::test]
fn fixed_height_virtual_list_box_pages_by_the_shown_viewport(cx: &mut TestAppContext) {
    let recorded = events();
    let for_view = recorded.clone();
    let cx = open_host(cx, move || {
        let recorded = for_view.clone();
        let items: Vec<ListBoxItem> = (0..20)
            .map(|i| {
                ListBoxItem::new(format!("key-{i:02}"), format!("Item {i}")).is_disabled(i == 5)
            })
            .collect();
        gpui::div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                ListBox::new("vlb-bounded-page", items)
                    .row_height(px(40.))
                    .max_h(px(160.))
                    .on_action(move |key, _, _| recorded.borrow_mut().push(key.to_string()))
                    .into_any_element(),
            )
            .into_any_element()
    });

    // A 120px window shows fewer than the 160px cap allows, so one page from
    // the top is shorter than the roomy-window three-row step.
    settle_window(cx, 640., 120.);
    let list = cx
        .debug_bounds(virtual_rows_probe("vlb-bounded-page"))
        .expect("the virtual list must be laid out");
    let shown = f32::from(list.size.height);
    assert!(
        shown < 160. - 1.5,
        "the bounded virtual list must shrink below its cap, got {list:?}"
    );
    let step = ((shown / 40.).ceil() as usize).saturating_sub(1);
    let capped_step = ((160f32 / 40.).ceil() as usize).saturating_sub(1);
    assert!(
        step < capped_step,
        "a shorter viewport must page shorter than the cap ruler: {step} vs {capped_step}"
    );

    // Two Downs seat row 2 (the first Down moves off the unfocused entry row
    // 0). Each page then moves `step` rows: 2 -> 4, 4 -> 6 across disabled
    // row 5, and back 6 -> 4. The cap ruler would walk 2 -> 6 -> 9 -> 6.
    press(cx, "tab");
    press(cx, "down");
    press(cx, "down");
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-04"],
        "a bounded PageDown must step by the shown viewport, not the cap"
    );
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-04", "key-06"],
        "a bounded PageDown must still skip the disabled row"
    );
    press(cx, "pageup");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-04", "key-06", "key-04"],
        "a bounded PageUp must step back by the shown viewport"
    );

    // Growing the window restores the cap viewport, so the same key pages
    // further: row 4 reaches row 7 with the three-row step.
    settle_window(cx, 640., 800.);
    let grown = cx
        .debug_bounds(virtual_rows_probe("vlb-bounded-page"))
        .expect("the virtual list must be laid out");
    assert!(
        (f32::from(grown.size.height) - 160.).abs() < 1.5,
        "the grown virtual list must return to its cap, got {grown:?}"
    );
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-04", "key-06", "key-04", "key-07"],
        "PageDown after resize must page by the grown viewport"
    );

    // A near-zero viewport has no page to turn: the cursor stays on row 7,
    // so Enter reports it again instead of a neighbour.
    settle_window(cx, 640., 20.);
    press(cx, "pagedown");
    press(cx, "enter");
    assert_eq!(
        recorded.borrow().as_slice(),
        ["key-04", "key-06", "key-04", "key-07", "key-07"],
        "a page key with no viewport to cross must not move the cursor"
    );
}

/// Gallery Virtualization regression: a 1000x50px fixed-row list caps at
/// `max_h(400)` inside a natural/unbounded page parent. A fixed viewport
/// height sizes the unbounded parent to cap + padding instead of the rows'
/// full natural height (50_000px) -- the gallery would otherwise show eight
/// rows followed by a huge blank instead of the next example -- while
/// `min_h_0` lets it shrink with a bounded parent. The virtual viewport stays
/// at the cap while the marker -- and the second list below it -- directly
/// follow at cap+insets.
#[gpui::test]
fn fixed_height_virtual_list_box_caps_in_an_unbounded_parent(cx: &mut TestAppContext) {
    fn virtual_list(id: &'static str) -> gpui::AnyElement {
        let items: Vec<ListBoxItem> = (0..1000)
            .map(|i| ListBoxItem::new(format!("key-{i:04}"), format!("User {i}")))
            .collect();
        ListBox::new(id, items)
            .row_height(px(50.))
            .max_h(px(400.))
            .into_any_element()
    }

    let cx = open_host(cx, move || {
        gpui::div()
            .size_full()
            .child(
                gpui::div()
                    .id("unbounded-page-scroll")
                    .overflow_y_scroll()
                    .size_full()
                    .child(
                        gpui::div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .child(virtual_list("vlb-unbounded"))
                            .child(
                                gpui::div()
                                    .h(px(8.))
                                    .w_full()
                                    .debug_selector(|| "after-first-list".to_owned()),
                            )
                            .child(virtual_list("vlb-second")),
                    ),
            )
            .into_any_element()
    });
    cx.run_until_parked();

    let body = cx
        .debug_bounds(virtual_rows_probe("vlb-unbounded"))
        .expect("the first virtual list must be laid out");
    assert!(
        (f32::from(body.size.height) - 400.).abs() < 1.5,
        "the unbounded virtual list must cap at 400, got {body:?}"
    );

    let after = cx
        .debug_bounds("after-first-list")
        .expect("the marker below the first list must be laid out");
    let after_top = f32::from(after.origin.y);
    let body_bottom = f32::from(body.origin.y) + f32::from(body.size.height);
    assert!(
        after_top < 800.,
        "the first list must end near 400+insets, marker at y={after_top} body={body:?}"
    );
    assert!(
        (after_top - body_bottom) < 120.,
        "the marker must directly follow the first body, marker y={after_top} body bottom={body_bottom}"
    );

    let second = cx
        .debug_bounds(virtual_rows_probe("vlb-second"))
        .expect("the second virtual list must be laid out");
    let second_top = f32::from(second.origin.y);
    assert!(
        second_top < 1200.,
        "the second list must directly follow the first, second at y={second_top} marker at y={after_top}"
    );
    assert!(
        (f32::from(second.size.height) - 400.).abs() < 1.5,
        "the second virtual list must cap at 400, got {second:?}"
    );
}

// ---------------------------------------------------------------------------
// TagGroup
// ---------------------------------------------------------------------------

/// The distinct fills painted exactly over `target` (a control's skin and
/// the hover-fade layer that covers it).
fn fills_over(scene: &harness::Painted, target: gpui::Bounds<gpui::Pixels>) -> Vec<gpui::Hsla> {
    let mut fills: Vec<gpui::Hsla> = Vec::new();
    for q in &scene.quads {
        let b = scene.bounds(q);
        if harness::contains(b, target) && harness::contains(target, b) {
            if let Some(c) = q.background.as_solid().filter(|c| c.a > 0.) {
                if !harness::has_color(&fills, c) {
                    fills.push(c);
                }
            }
        }
    }
    fills
}

/// TagGroup's background transition is its stylesheet's own 100ms, not the
/// theme's shared hover duration, and its fade layer shares the tag radius
/// so a mid-transition frame cannot leak square corners. Observed on the
/// executor clock with the theme's shared duration stretched to a minute: after
/// 300ms the tag's fade has settled (its layer paints the skin's endpoint),
/// while a Button — which rides the theme duration — is still mid-fade.
#[gpui::test]
fn tag_group_uses_the_pinned_smooth_hover_transition(cx: &mut TestAppContext) {
    use gpui::Modifiers;
    use herogpui_components::Button;
    use herogpui_theme::{set_theme, ActiveTheme, Theme};

    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(40.))
            .p(px(24.))
            .child(TagGroup::new("tg-fade", vec![Tag::new("a", "Alpha")]))
            .child(Button::new("b-fade").label("Save"))
            .into_any_element()
    });
    cx.update(|window, cx| {
        set_theme(
            Theme::builder("slow", Theme::light())
                .hover_fade_ms(60_000)
                .build(),
            cx,
        );
        window.refresh();
    });
    let resting = harness::painted(cx);
    let radius = cx.update(|_, cx| f32::from(herogpui_components::extend::small_radius(cx)));
    let default = cx.update(|_, cx| cx.colors().default.color);
    let chip = resting
        .filled(default)
        .into_iter()
        .find(|q| resting.is_uniform(q, radius))
        .map(|q| resting.bounds(q))
        .expect("the tag chip must paint its resting fill at the tag radius");
    let button = resting
        .quads
        .iter()
        .filter(|q| {
            f32::from(resting.bounds(q).origin.y) > f32::from(chip.origin.y + chip.size.height)
        })
        .find(|q| q.background.as_solid().is_some_and(|c| c.a > 0.))
        .map(|q| resting.bounds(q))
        .expect("the button must paint its skin");

    cx.simulate_mouse_move(chip.center(), None, Modifiers::none());
    harness::wait_ms(cx, 300);
    let scene = harness::painted(cx);
    let tag_fills = fills_over(&scene, chip);
    assert_eq!(
        tag_fills.len(),
        1,
        "after 300ms the tag's own 100ms fade must have settled on its \
         endpoint, painted {tag_fills:?}"
    );
    let layers = scene
        .quads
        .iter()
        .filter(|q| {
            let b = scene.bounds(q);
            harness::contains(b, chip) && harness::contains(chip, b)
        })
        .collect::<Vec<_>>();
    assert!(
        !layers.is_empty() && layers.iter().all(|q| scene.is_uniform(q, radius)),
        "the fade layer must carry the tag radius\n{}",
        scene.describe()
    );

    cx.simulate_mouse_move(button.center(), None, Modifiers::none());
    harness::wait_ms(cx, 300);
    let scene = harness::painted(cx);
    let button_fills = fills_over(&scene, button);
    assert_eq!(
        button_fills.len(),
        1,
        "control: a Button on the theme's minute-long fade is still mid-way \
         after 300ms — its single fade layer paints the mid colour, painted \
         {button_fills:?}"
    );
}

#[gpui::test]
fn tag_group_remove_reports_the_key(cx: &mut TestAppContext) {
    let events = events();
    let removed = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        // The label slot is drawn as a fixed 40x20 box (via `tag_content`, v3's
        // children-as-a-function) so every distance the click depends on is a
        // constant — see the header comment: a chip is 8+40+4+12+8 = 72px wide,
        // 28px tall, and its size-3 remove button spans x 52..64 at y 8..20.
        TagGroup::new(
            "tg-remove",
            vec![
                Tag::new("alpha", "Alpha"),
                Tag::new("beta", "Beta"),
                Tag::new("gamma", "Gamma"),
            ],
        )
        .tag_content(|_, _| gpui::div().w(px(40.)).h(px(20.)).into_any_element())
        .on_remove(move |keys, _, _| events.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    // Chip 0's remove button centre: (58, 14); chip 1 starts at 72+6 = 78 and
    // its button centre is (58+78, 14) = (136, 14).
    click(cx, 58., 14.);
    assert_eq!(removed.borrow().as_slice(), ["alpha"]);
    click(cx, 136., 14.);
    assert_eq!(
        removed.borrow().as_slice(),
        ["alpha", "beta"],
        "each tag's own remove button must report that tag's key"
    );
}

#[gpui::test]
fn tag_group_roving_focus_survives_removal(cx: &mut TestAppContext) {
    // The group is ONE tab stop with a keyed cursor (AGENTS.md: "TagGroup keeps
    // its cursor in a keyed usize… clamp that cursor to the enabled tags —
    // Delete shortens the list, and a stop pointing past the end… takes the
    // group out of the tab order"). This test walks the cursor to the last tag,
    // deletes it, and then asks the group to keep answering the arrows: without
    // the clamp, nothing would claim the group's handle any more and the keys
    // would reach no tag at all.
    //
    // A TagGroup never removes tags itself — `on_remove` only reports the keys
    // and the *caller* drops it — so the test owns the list and shortens it
    // inside the handler, which is how the component is actually used.
    let tags = Rc::new(RefCell::new(vec![
        Tag::new("alpha", "Alpha"),
        Tag::new("beta", "Beta"),
        Tag::new("gamma", "Gamma"),
    ]));
    let events = events();
    let removed = events.clone();
    let cx = open_host(cx, move || {
        let tags = tags.clone();
        let events = events.clone();
        let current = tags.borrow().clone();
        TagGroup::new("tg-roving", current)
            .on_remove(move |keys, _, cx| {
                events.borrow_mut().push(sorted_join(keys));
                tags.borrow_mut().retain(|tag| !keys.contains(tag.key()));
                // A real app drops the tag from its own state here and asks for
                // a redraw; without the refresh the next keypress would arrive
                // against the frame that still shows the removed tag.
                cx.refresh_windows();
            })
            .into_any_element()
    });

    // Tab enters the group on the first tag; two Rights walk the cursor to the
    // last one (each render passes the handle to the tag the cursor moved to).
    press(cx, "tab");
    press(cx, "right right");

    // Delete removes the focused tag: the cursor was 2 (Gamma) and now points
    // past the end of the two remaining tags.
    press(cx, "delete");
    assert_eq!(removed.borrow().as_slice(), ["gamma"]);

    // The clamp has to have parked the roving stop back on the first enabled
    // tag; a stale cursor would leave the group outside the tab order and this
    // Right would reach nothing. The Right then Delete must take Beta.
    press(cx, "right");
    press(cx, "delete");
    assert_eq!(
        removed.borrow().as_slice(),
        ["gamma", "beta"],
        "after removing the last tag the group must still answer the arrows \
         and remove the newly focused tag"
    );
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

#[gpui::test]
fn tabs_uncontrolled_selection_changes(cx: &mut TestAppContext) {
    // The regression behind this test: `Tabs::new` used to seed the *controlled*
    // prop, `selected_key`, so `util::controlled` handed the caller's value back
    // with no state entity to write to — every demo that passed a literal was
    // inert while looking perfect. `Tabs::new`'s positional key is
    // `defaultSelectedKey`, so the tabs must switch themselves on press.
    let events = events();
    let selected = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        Tabs::new(
            "tb-click",
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
            ],
            "first",
        )
        .on_selection_change(move |key, _, _| events.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    // The labels are measured with the window's own text system (see
    // `text_width`). Second tab centre: list p-1 (4) + the first tab's
    // label+2*16px padding, then half of the second tab's own label+2*16px.
    // Y: list p-1 top + h-8/2 = 4 + 16.
    let w_first =
        cx.update(|window, _| text_width(window.text_system(), "First", 14.0, FontWeight::MEDIUM));
    let w_second =
        cx.update(|window, _| text_width(window.text_system(), "Second", 14.0, FontWeight::MEDIUM));
    let tab1_centre_x = 4. + w_first + 32. + (w_second + 32.) / 2.;
    click(cx, tab1_centre_x, 20.);
    assert_eq!(
        selected.borrow().as_slice(),
        ["second"],
        "clicking the second tab must move the uncontrolled selection to it"
    );
}

#[gpui::test]
fn tabs_arrows_move_selection(cx: &mut TestAppContext) {
    // A tab list is one tab stop: the selected tab claims the group's handle,
    // and the arrows select as they rove (React Aria's automatic activation).
    let events = events();
    let selected = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        Tabs::new(
            "tb-arrows",
            vec![
                TabItem::new("first", "First"),
                TabItem::new("second", "Second"),
                TabItem::new("third", "Third"),
            ],
            "first",
        )
        .on_selection_change(move |key, _, _| events.borrow_mut().push(key.to_string()))
        .into_any_element()
    });

    // Tab lands on the selected tab, whose handle the keyboard travels with as
    // the selection moves. Right walks first -> second -> third; a Left returns
    // to second. (The ends wrap, which is why a third Right is not tested.)
    press(cx, "tab");
    press(cx, "right");
    press(cx, "right");
    press(cx, "left");
    assert_eq!(
        selected.borrow().as_slice(),
        ["second", "third", "second"],
        "the arrows must move the selection: Right Right from the first tab \
         lands on third, Left brings it back to second"
    );
}

// ---------------------------------------------------------------------------
// Accordion
// ---------------------------------------------------------------------------

#[gpui::test]
fn accordion_toggles_one_item(cx: &mut TestAppContext) {
    let events = events();
    let expanded = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        Accordion::new(vec![
            AccordionItem::new("one", "Item one"),
            AccordionItem::new("two", "Item two"),
        ])
        .id("acc-one")
        .on_expanded_change(move |keys, _, _| events.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    // Header 0 is 52px tall starting at the origin (py-4 twice + a 20px line),
    // so its centre is y 26; the header spans the full window width.
    click(cx, 60., 26.);
    assert_eq!(
        expanded.borrow().as_slice(),
        ["one"],
        "clicking a closed trigger must expand that item"
    );
    click(cx, 60., 26.);
    assert_eq!(
        expanded.borrow().as_slice(),
        ["one", ""],
        "clicking the same trigger again must collapse it"
    );
}

#[gpui::test]
fn accordion_single_mode_closes_the_previous(cx: &mut TestAppContext) {
    let events = events();
    let expanded = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        // `allows_multiple_expanded(false)` is v3's single-expand behaviour:
        // expanding one item collapses the rest (`next_expanded` in
        // accordion.rs). The bodies are given a fixed 40px content so the
        // second header's position is a constant while the first is open.
        Accordion::new(vec![
            AccordionItem::new("one", "Item one").content(gpui::div().h(px(40.))),
            AccordionItem::new("two", "Item two").content(gpui::div().h(px(40.))),
        ])
        .id("acc-single")
        .allows_multiple_expanded(false)
        .on_expanded_change(move |keys, _, _| events.borrow_mut().push(sorted_join(keys)))
        .into_any_element()
    });

    // Header 0 centre: y 26. With item one open, item two's header sits below
    // 52 (header) + 2+40+16 (body) + 1 (separator) + 26 (half header) = 137.
    click(cx, 60., 26.);
    assert_eq!(expanded.borrow().as_slice(), ["one"]);
    settle_collapsible(cx);
    click(cx, 60., 137.);
    assert_eq!(
        expanded.borrow().as_slice(),
        ["one", "two"],
        "with allowsMultipleExpanded off, opening the second item must report \
         exactly that one — never both"
    );
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

#[gpui::test]
fn pagination_next_and_page_click(cx: &mut TestAppContext) {
    let events = events();
    let pages = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        Pagination::new("pg-nav", 1, 3)
            .on_change(move |page, _, _| events.borrow_mut().push(page.to_string()))
            .into_any_element()
    });

    // size-md cells are 32px squares at y 0..32 (centre y 16). The prev button
    // spans x 0..34 (px-2.5 each side of a 14px glyph); with 4px gaps the page
    // cells start at x 38, 74 and 110, so page 3's centre is x 126, and the
    // next button starts at x 146 (centre 163). All maths in the header comment.
    click(cx, 163., 16.);
    assert_eq!(
        pages.borrow().as_slice(),
        ["2"],
        "clicking the next arrow must report page 2"
    );
    click(cx, 126., 16.);
    assert_eq!(
        pages.borrow().as_slice(),
        ["2", "3"],
        "clicking the third page cell must report page 3"
    );
}

// ---------------------------------------------------------------------------
// Breadcrumbs
// ---------------------------------------------------------------------------

#[gpui::test]
fn breadcrumbs_item_press_reports(cx: &mut TestAppContext) {
    let events = events();
    let navigated = events.clone();
    let cx = open_host(cx, move || {
        let events = events.clone();
        // The last crumb is the current page and never navigates (no `on_click`
        // is attached when `is_last`), so the clickable targets are the first
        // and the middle crumb.
        Breadcrumbs::new(vec![
            Crumb::new("Build"),
            Crumb::new("Deploy"),
            Crumb::new("Live"),
        ])
        .on_navigate(move |index, crumb, _, _, _| {
            events.borrow_mut().push(format!("{index}:{}", crumb.label));
        })
        .into_any_element()
    });

    // The labels are measured with the window's own text system at the link's
    // MEDIUM weight. The first label starts flush in its unpadded row on a
    // 20px line, so its centre is (w/2, 10). Each non-last row is a 4px gap +
    // a 12px separator wider than its label and the root gaps rows by 6px, so
    // the second label centres at (w_build + 22 + w_deploy/2, 10).
    let w_build =
        cx.update(|window, _| text_width(window.text_system(), "Build", 14.0, FontWeight::MEDIUM));
    let w_deploy =
        cx.update(|window, _| text_width(window.text_system(), "Deploy", 14.0, FontWeight::MEDIUM));
    click(cx, w_build / 2., 10.);
    assert_eq!(
        navigated.borrow().as_slice(),
        ["0:Build"],
        "clicking the first crumb must report its index and label"
    );
    click(cx, w_build + 22. + w_deploy / 2., 10.);
    assert_eq!(
        navigated.borrow().as_slice(),
        ["0:Build", "1:Deploy"],
        "clicking the second crumb must report its own index and label"
    );
}
