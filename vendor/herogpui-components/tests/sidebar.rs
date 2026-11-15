//! `Sidebar` (HeroGPUI extension): pressing and keyboard-activating items,
//! the roving cursor over items and collapsible group headers (skipping
//! disabled items and closed groups), group expansion by press and
//! Right/Left, typeahead, and the collapse-to-icons toggle and icon mode.

use crate::harness;

use std::collections::HashSet;

use gpui::{prelude::*, px, Modifiers, SharedString, TestAppContext, VisualTestContext};
use harness::{events, open_host, press, still, Events};
use herogpui_components::{IconName, Sidebar, SidebarGroup, SidebarItem};

#[derive(Clone, Copy, Default)]
struct Config {
    collapsed: bool,
}

fn groups() -> Vec<SidebarGroup> {
    vec![
        SidebarGroup::new("main").items(vec![
            SidebarItem::new("inbox", "Inbox")
                .icon(IconName::Inbox)
                .badge("3"),
            SidebarItem::new("drafts", "Drafts").icon(IconName::File),
            SidebarItem::new("spam", "Spam")
                .icon(IconName::Trash)
                .is_disabled(true),
        ]),
        SidebarGroup::new("projects")
            .label("Projects")
            .is_collapsible(true)
            .items(vec![
                SidebarItem::new("web", "Website").icon(IconName::Globe),
                SidebarItem::new("app", "Mobile app").icon(IconName::House),
            ]),
        SidebarGroup::new("more")
            .label("More")
            .items(vec![
                SidebarItem::new("settings", "Settings").icon(IconName::Settings)
            ]),
    ]
}

fn sorted(keys: &HashSet<SharedString>) -> String {
    let mut keys: Vec<&str> = keys.iter().map(|k| k.as_ref()).collect();
    keys.sort_unstable();
    keys.join(",")
}

fn host(cx: &mut TestAppContext, config: Config) -> (Events, &mut VisualTestContext) {
    still();
    let seen = events();
    let s = seen.clone();
    let cx = open_host(cx, move || {
        let (sel, col, exp) = (s.clone(), s.clone(), s.clone());
        gpui::div()
            .h(px(700.))
            .flex()
            .child(
                Sidebar::new("nav", groups())
                    .default_active_key("inbox")
                    .default_collapsed(config.collapsed)
                    .on_select(move |key, _, _| sel.borrow_mut().push(format!("select:{key}")))
                    .on_collapsed_change(move |c, _, _| {
                        col.borrow_mut().push(format!("collapsed:{c}"));
                    })
                    .on_expanded_change(move |keys, _, _| {
                        exp.borrow_mut().push(format!("expanded:{}", sorted(keys)));
                    }),
            )
            .into_any_element()
    });
    (seen, cx)
}

fn frame(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

fn bounds(cx: &mut VisualTestContext, name: &str) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.debug_bounds(Box::leak(name.to_owned().into_boxed_str()))
}

fn click(cx: &mut VisualTestContext, name: &str) {
    let at = bounds(cx, name)
        .unwrap_or_else(|| panic!("{name} laid out"))
        .center();
    cx.simulate_click(at, Modifiers::none());
    frame(cx);
}

fn keys(cx: &mut VisualTestContext, list: &str) {
    for key in list.split_whitespace() {
        press(cx, key);
        frame(cx);
    }
}

fn log(seen: &Events) -> Vec<String> {
    seen.borrow().clone()
}

#[gpui::test]
fn pressing_an_item_selects_it_and_a_disabled_one_does_nothing(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    click(cx, "nav-item-spam");
    assert!(log(&seen).is_empty(), "a disabled item does not select");
    click(cx, "nav-item-drafts");
    assert_eq!(log(&seen), ["select:drafts"]);
}

#[gpui::test]
fn the_keyboard_enters_on_the_active_item_and_skips_disabled_items(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    // Tab lands on the sidebar's single stop, at the active item (Inbox).
    keys(cx, "tab down");
    // Drafts; Down again skips the disabled Spam to the Projects header.
    keys(cx, "enter");
    assert_eq!(log(&seen), ["select:drafts"]);
    keys(cx, "down enter");
    assert_eq!(
        log(&seen),
        ["select:drafts", "expanded:"],
        "Enter on the Projects header collapses it"
    );
    // With Projects closed, Down goes straight to Settings.
    keys(cx, "down enter");
    assert_eq!(
        log(&seen),
        ["select:drafts", "expanded:", "select:settings"]
    );
    // Home returns to Inbox.
    keys(cx, "home enter");
    assert_eq!(log(&seen).last().unwrap(), "select:inbox");
}

#[gpui::test]
fn right_and_left_expand_and_collapse_from_a_header(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    assert!(
        bounds(cx, "nav-item-web").is_some(),
        "groups start expanded"
    );
    keys(cx, "tab down down left");
    assert_eq!(log(&seen), ["expanded:"]);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(400));
    frame(cx);
    assert!(
        bounds(cx, "nav-item-web").is_none(),
        "the closed group's items leave"
    );
    keys(cx, "right");
    assert_eq!(log(&seen), ["expanded:", "expanded:projects"]);
    frame(cx);
    assert!(bounds(cx, "nav-item-web").is_some());
    // Typeahead finds "Mobile app".
    keys(cx, "m enter");
    assert_eq!(log(&seen).last().unwrap(), "select:app");
}

#[gpui::test]
fn the_header_press_toggles_its_group(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    click(cx, "nav-heading-projects");
    assert_eq!(log(&seen), ["expanded:"]);
    click(cx, "nav-heading-projects");
    assert_eq!(log(&seen), ["expanded:", "expanded:projects"]);
}

#[gpui::test]
fn the_toggle_collapses_to_icons(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config::default());
    frame(cx);
    let wide = bounds(cx, "nav-sidebar").unwrap().size.width;
    assert_eq!(wide, px(256.));
    click(cx, "nav-toggle");
    assert_eq!(log(&seen), ["collapsed:true"]);
    let narrow = bounds(cx, "nav-sidebar").unwrap().size.width;
    assert_eq!(narrow, px(56.));
    assert!(
        bounds(cx, "nav-heading-projects").is_none(),
        "icon mode hides the group headings"
    );
    // Every item is still reachable as an icon.
    assert!(bounds(cx, "nav-item-settings").is_some());
    click(cx, "nav-toggle");
    assert_eq!(log(&seen), ["collapsed:true", "collapsed:false"]);
}

#[gpui::test]
fn icon_mode_walks_the_items_only_and_draws_icon_rows(cx: &mut TestAppContext) {
    let (seen, cx) = host(cx, Config { collapsed: true });
    frame(cx);
    // In icon mode there are no header stops: Down from Inbox is Drafts, then
    // (Spam is disabled) Website.
    keys(cx, "tab down down");
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(50));
    frame(cx);
    keys(cx, "enter");
    assert_eq!(log(&seen), ["select:web"]);
    let row = bounds(cx, "nav-item-web").unwrap();
    assert!(row.size.width <= px(40.), "an icon-only item");
}
