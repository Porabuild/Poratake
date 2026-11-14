//! Tag line boxes must match v3.2.4 even outside the gallery's 14/20 text root.

use crate::harness;

use gpui::{prelude::*, px, TestAppContext};
use herogpui_components::{SelectionMode, Size, Tag, TagGroup, TagVariant};

const SELECTORS: [&str; 3] = ["tag-sm", "tag-md", "tag-lg"];

/// HeroUI 3.2.5 adds a transparent 24px touch target around the 12px
/// remove glyph. It must not change the chip's size or select its body.
#[gpui::test]
fn remove_target_extends_without_changing_tag_layout(cx: &mut TestAppContext) {
    for size in Size::ALL {
        for variant in TagVariant::ALL {
            for custom in [false, true] {
                let removed = harness::events();
                let selected = harness::events();
                let for_remove = removed.clone();
                let for_select = selected.clone();
                let leading = if size == Size::Lg { 20. } else { 16. };
                let padding = match size {
                    Size::Sm => (8., 2.),
                    Size::Md => (8., 4.),
                    Size::Lg => (10., 6.),
                };
                let cx = harness::open_host(cx, move || {
                    let tag = Tag::new("one", "Tag");
                    let tag = if custom {
                        tag.remove_content(|| gpui::div().size(px(12.)).into_any_element())
                    } else {
                        tag
                    };
                    let removed = for_remove.clone();
                    let selected = for_select.clone();
                    gpui::div()
                        .p(px(12.))
                        .flex()
                        .flex_col()
                        .items_start()
                        .child(
                            gpui::div().debug_selector(|| "target-root".into()).child(
                                TagGroup::new("target", vec![tag])
                                    .size(size)
                                    .variant(variant)
                                    .selection_mode(SelectionMode::Single)
                                    .tag_content(move |_, _| {
                                        gpui::div().w(px(40.)).h(px(leading)).into_any_element()
                                    })
                                    .on_remove(move |keys, _, _| {
                                        removed
                                            .borrow_mut()
                                            .push(keys.iter().next().unwrap().to_string());
                                    })
                                    .on_selection_change(move |_, _, _| {
                                        selected.borrow_mut().push("selected".into());
                                    }),
                            ),
                        )
                        .into_any_element()
                });
                cx.run_until_parked();
                let bounds = cx.debug_bounds("target-root").unwrap();
                assert_eq!(bounds.size.width, px(padding.0 * 2. + 40. + 4. + 12.));
                assert_eq!(bounds.size.height, px(padding.1 * 2. + leading));
                // x padding + 40px label + 4px gap + 6px half-glyph.
                let center_x = 12. + padding.0 + 40. + 4. + 6.;
                let center_y = 12. + padding.1 + leading / 2.;
                for (dx, dy) in [(-11., 0.), (11., 0.), (0., -11.), (0., 11.)] {
                    harness::click(cx, center_x + dx, center_y + dy);
                }
                assert_eq!(
                    removed.borrow().as_slice(),
                    ["one", "one", "one", "one"],
                    "{size:?}/{variant:?}, custom={custom}"
                );
                assert!(
                    selected.borrow().is_empty(),
                    "expanded removal must not select the tag"
                );
            }
        }
    }
}

#[gpui::test]
fn expanded_remove_targets_respect_disabled_tags_and_neighbors(cx: &mut TestAppContext) {
    for disabled_mode in 0..3 {
        let events = harness::events();
        let for_view = events.clone();
        let cx = harness::open_host(cx, move || {
            let removed = for_view.clone();
            let selected = for_view.clone();
            gpui::div()
                .p(px(12.))
                .child(
                    TagGroup::new(
                        "neighbors",
                        vec![
                            Tag::new("one", "One").is_disabled(disabled_mode == 0),
                            Tag::new("two", "Two"),
                        ],
                    )
                    .size(Size::Sm)
                    .is_disabled(disabled_mode == 2)
                    .disabled_keys(if disabled_mode == 1 {
                        vec!["one".into()]
                    } else {
                        vec![]
                    })
                    .selection_mode(SelectionMode::Single)
                    .tag_content(|_, _| gpui::div().w(px(40.)).h(px(16.)).into_any_element())
                    .on_remove(move |keys, _, _| {
                        removed
                            .borrow_mut()
                            .push(format!("remove:{}", keys.iter().next().unwrap()));
                    })
                    .on_selection_change(move |keys, _, _| {
                        selected
                            .borrow_mut()
                            .push(format!("select:{}", keys.iter().next().unwrap()));
                    }),
                )
                .into_any_element()
        });
        // First target centre (70,22); second tag begins at x=90.
        for (x, y) in [(70., 22.), (81., 22.), (70., 11.), (70., 33.)] {
            harness::click(cx, x, y);
        }
        assert!(events.borrow().is_empty(), "disabled mode {disabled_mode}");
        harness::click(cx, 100., 22.);
        harness::click(cx, 159., 22.);
        if disabled_mode == 2 {
            assert!(events.borrow().is_empty());
        } else {
            assert_eq!(events.borrow().as_slice(), ["select:two", "remove:two"]);
        }
    }
}

#[gpui::test]
fn tag_sizes_do_not_inherit_host_line_height(cx: &mut TestAppContext) {
    for leading in [None, Some(48.)] {
        let cx = harness::open_host(cx, move || {
            let mut root = gpui::div().flex().flex_col().items_start();
            if let Some(leading) = leading {
                root = root.text_size(px(32.)).line_height(px(leading));
            }
            root.children(Size::ALL.into_iter().enumerate().map(|(index, size)| {
                gpui::div()
                    .debug_selector(move || SELECTORS[index].to_owned())
                    .child(
                        TagGroup::new(format!("tag-{index}"), vec![Tag::new("one", "Tag")])
                            .size(size),
                    )
            }))
            .into_any_element()
        });
        cx.run_until_parked();
        for (index, height) in [20., 24., 32.].into_iter().enumerate() {
            let bounds = cx.debug_bounds(SELECTORS[index]).unwrap();
            assert!(
                (f32::from(bounds.size.height) - height).abs() < 0.5,
                "size {index}, host leading {leading:?}: expected {height}px, got {bounds:?}"
            );
        }
    }
}

#[gpui::test]
fn tag_label_description_and_empty_state_own_their_line_boxes(cx: &mut TestAppContext) {
    let cx = harness::open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .text_size(px(32.))
            .line_height(px(48.))
            .child(
                gpui::div()
                    .debug_selector(|| "tag-labelled".to_owned())
                    .child(
                        TagGroup::new("labelled", vec![Tag::new("one", "Tag")])
                            .label("Label")
                            .description("Description"),
                    ),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "tag-empty".to_owned())
                    .child(TagGroup::new("empty", vec![]).label("Label")),
            )
            .into_any_element()
    });
    cx.run_until_parked();
    // Label 20 + gaps 8 + medium tag 24 + description 16 with p-1 = 76.
    let labelled = cx.debug_bounds("tag-labelled").unwrap();
    assert_eq!(labelled.size.height, px(76.));
    // Label 20 + gap 4 + empty-state 20 with p-2 = 60.
    let empty = cx.debug_bounds("tag-empty").unwrap();
    assert_eq!(empty.size.height, px(60.));
}
