//! AvatarGroup behavior against the pinned v3.2.6 contract
//! (`packages/react/tests/components/avatar-group/avatar-group.test.tsx`).
//!
//! Upstream asserts `data-slot` / BEM class names; the port's debug
//! selectors carry the same modifiers (`avatar-group--clip`,
//! `avatar--sm`, `avatar__fallback--accent`, ...). Every host here is static,
//! so `debug_bounds` presence is a reliable "rendered" signal.

use crate::harness;

use gpui::{prelude::*, px, TestAppContext, VisualTestContext};
use harness::{open_host, probe};
use herogpui_components::{
    Avatar, AvatarGroup, AvatarGroupCount, AvatarGroupOverlap, AvatarVariant,
};
use herogpui_core::{Color, Size};

fn letter(name: &'static str) -> Avatar {
    Avatar::new(name).fallback(probe(name))
}

/// `debug_bounds` takes a `&'static str`; the selectors here are built per
/// test, so leak the handful a test binary asks for.
fn key(selector: &str) -> &'static str {
    Box::leak(selector.to_owned().into_boxed_str())
}

fn has(cx: &mut VisualTestContext, selector: &str) -> bool {
    cx.debug_bounds(key(selector)).is_some()
}

#[gpui::test]
fn exposes_the_slot_and_default_clip_modifier(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        AvatarGroup::new("g").child(letter("A")).into_any_element()
    });
    assert!(has(cx, "avatar-group[g].avatar-group--clip"));
    assert!(has(cx, "A"));
}

#[gpui::test]
fn ring_overlap_replaces_the_clip_modifier(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        AvatarGroup::new("g")
            .overlap(AvatarGroupOverlap::Ring)
            .child(letter("A"))
            .into_any_element()
    });
    assert!(has(cx, "avatar-group[g].avatar-group--ring"));
    assert!(!has(cx, "avatar-group[g].avatar-group--clip"));
}

#[gpui::test]
fn without_max_every_avatar_renders_and_there_is_no_count(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        AvatarGroup::new("g")
            .children(["A", "B", "C", "D", "E", "F"].map(letter))
            .into_any_element()
    });
    for name in ["A", "B", "C", "D", "E", "F"] {
        assert!(has(cx, name), "{name} must render");
    }
    assert!(!has(cx, "avatar-group-count[g-count]"));
}

#[gpui::test]
fn max_truncates_and_appends_the_automatic_count(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        AvatarGroup::new("g")
            .max(2)
            .children(["A", "B", "C", "D"].map(letter))
            .into_any_element()
    });
    assert!(has(cx, "A") && has(cx, "B"));
    assert!(
        !has(cx, "C") && !has(cx, "D"),
        "avatars past max are dropped"
    );
    assert!(
        has(cx, "avatar-group-count[g-count]"),
        "the +2 count renders"
    );
}

#[gpui::test]
fn an_explicit_count_renders_without_max(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        AvatarGroup::new("g")
            .size(Size::Sm)
            .children(["A", "B", "C"].map(letter))
            .count(AvatarGroupCount::new("explicit").child("+9"))
            .into_any_element()
    });
    assert!(has(cx, "A") && has(cx, "B") && has(cx, "C"));
    assert!(has(cx, "avatar-group-count[explicit]"));
    assert!(!has(cx, "avatar-group-count[g-count]"));
    assert!(
        has(cx, "avatar[explicit]--sm--default"),
        "the count inherits the group size"
    );
}

#[gpui::test]
fn an_explicit_count_suppresses_the_automatic_count(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        AvatarGroup::new("g")
            .max(2)
            .children(["A", "B", "C", "D"].map(letter))
            .count(AvatarGroupCount::new("explicit").child("+99"))
            .into_any_element()
    });
    assert!(has(cx, "A") && has(cx, "B"));
    assert!(!has(cx, "C") && !has(cx, "D"));
    assert!(has(cx, "avatar-group-count[explicit]"));
    assert!(!has(cx, "avatar-group-count[g-count]"), "no auto +2");
}

#[gpui::test]
fn grid_keeps_the_overlap_modifier_but_spaces_instead_of_stacking(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                AvatarGroup::new("grid")
                    .is_grid(true)
                    .children(["A", "B"].map(letter)),
            )
            .child(AvatarGroup::new("stack").children(["C", "D"].map(letter)))
            .into_any_element()
    });
    assert!(has(
        cx,
        "avatar-group[grid].avatar-group--grid.avatar-group--clip"
    ));
    let x = |cx: &mut VisualTestContext, name: &str| {
        cx.debug_bounds(key(&format!("avatar[{name}]--md--default")))
            .unwrap_or_else(|| panic!("{name} must render"))
            .origin
            .x
    };
    // `gap-3`: 40px avatar + 12px gap; stacked: 40px - 8px overlap.
    assert_eq!(x(cx, "B") - x(cx, "A"), px(52.));
    assert_eq!(x(cx, "D") - x(cx, "C"), px(32.));
}

#[gpui::test]
fn size_flows_to_direct_children_and_a_direct_prop_wins(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        AvatarGroup::new("g")
            .size(Size::Sm)
            .child(letter("A"))
            .child(letter("B").size(Size::Lg))
            .into_any_element()
    });
    assert!(has(cx, "avatar[A]--sm--default"));
    assert!(has(cx, "avatar[B]--lg--default"));
}

#[gpui::test]
fn color_flows_to_child_fallbacks(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        AvatarGroup::new("g")
            .color(Color::Accent)
            .child(letter("A"))
            .child(letter("B").color(Color::Danger))
            .into_any_element()
    });
    assert!(has(cx, "avatar__fallback[A]--accent"));
    assert!(has(cx, "avatar__fallback[B]--danger"));
}

#[gpui::test]
fn soft_variant_flows_to_children(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        AvatarGroup::new("g")
            .variant(AvatarVariant::Soft)
            .child(letter("A"))
            .into_any_element()
    });
    assert!(has(cx, "avatar[A]--md--soft"));
}

#[gpui::test]
fn a_nested_avatar_does_not_inherit(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        AvatarGroup::new("g")
            .size(Size::Sm)
            .variant(AvatarVariant::Soft)
            .color(Color::Accent)
            .child_element(gpui::div().child(letter("N")))
            .into_any_element()
    });
    assert!(has(cx, "avatar[N]--md--default"));
    assert!(has(cx, "avatar__fallback[N]--default"));
}
