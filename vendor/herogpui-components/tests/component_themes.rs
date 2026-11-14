//! Live `Theme.components` resolution: stock, defaults, recipes, and instance overrides.

use crate::harness;

use gpui::{prelude::*, px, TestAppContext, VisualTestContext};
use harness::open_host;
use herogpui_components::{
    Button, Input, InputState, Menu, MenuItem, PickerItem, Select, Slider, Switch,
};
use herogpui_theme::{
    set_theme, ButtonStyle, ComponentTheme, ComponentThemes, MenuStyle, SelectStyle, SliderStyle,
    SwitchStyle, TextFieldStyle, Theme,
};

fn themed(components: ComponentThemes) -> Theme {
    Theme::builder("app", Theme::light())
        .components(components)
        .build()
}

fn apply_theme(cx: &mut VisualTestContext, theme: Theme) {
    cx.update(|window, cx| {
        set_theme(theme, cx);
        window.refresh();
    });
}

fn restore_stock(cx: &mut VisualTestContext) {
    apply_theme(cx, Theme::light());
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.05
}

fn painted_corners(quad: &gpui::Quad, scale: f32) -> [f32; 4] {
    let corners = quad.corner_radii;
    [
        corners.top_left.0 / scale,
        corners.top_right.0 / scale,
        corners.bottom_right.0 / scale,
        corners.bottom_left.0 / scale,
    ]
}

fn md_inner_mark(cx: &mut VisualTestContext) -> [f32; 4] {
    let (quads, scale) = cx.update(|window, _| (window.painted_quads(), window.scale_factor()));
    let inner = quads
        .iter()
        .find(|quad| {
            close(quad.bounds.size.width.0 / scale, 24.)
                && close(quad.bounds.size.height.0 / scale, 16.)
        })
        .expect("Md inner mark must paint");
    painted_corners(inner, scale)
}

fn assert_corners(actual: [f32; 4], expected: [f32; 4], context: &str) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!(
            close(actual, expected),
            "{context}: painted corner {actual} must match {expected}"
        );
    }
}

fn slider_host<'a>(
    cx: &'a mut TestAppContext,
    recipe: Option<&'static str>,
) -> &'a mut VisualTestContext {
    open_host(cx, move || {
        let mut slider = Slider::new("theme-slider", 100.);
        if let Some(name) = recipe {
            slider = slider.recipe(name);
        }
        gpui::div().w(px(600.)).child(slider).into_any_element()
    })
}

#[gpui::test]
fn stock_slider_geometry_is_unchanged(cx: &mut TestAppContext) {
    let cx = slider_host(cx, None);
    let key_radius = cx.update(|_, cx| f32::from(herogpui_components::extend::key_radius(cx)));
    assert_corners(
        md_inner_mark(cx),
        [key_radius.min(8.); 4],
        "stock slider must keep the Md key-radius inner mark",
    );
}

#[gpui::test]
fn stock_menu_row_interval_is_unchanged(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        Menu::new(
            "stock-menu",
            vec![
                MenuItem::new("first", "First"),
                MenuItem::new("second", "Second"),
            ],
        )
        .animate_entry(false)
        .item_content(|key, _| {
            let key = key.clone();
            gpui::div()
                .h(px(20.))
                .w(px(40.))
                .debug_selector(move || format!("stock-{key}"))
                .into_any_element()
        })
        .into_any_element()
    });
    let first = cx.debug_bounds("stock-first").expect("first row");
    let second = cx.debug_bounds("stock-second").expect("second row");
    assert_eq!(second.origin.y - first.origin.y, px(38.));
}

#[gpui::test]
fn missing_recipe_is_a_noop(cx: &mut TestAppContext) {
    let cx = slider_host(cx, Some("missing"));
    let key_radius = cx.update(|_, cx| f32::from(herogpui_components::extend::key_radius(cx)));
    assert_corners(
        md_inner_mark(cx),
        [key_radius.min(8.); 4],
        "an unknown recipe must leave stock slider geometry",
    );
}

#[gpui::test]
fn slider_theme_defaults_recipes_and_instance_sx_follow_precedence(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(600.))
            .child(Slider::new("theme-default", 100.))
            .into_any_element()
    });
    let key_radius = cx.update(|_, cx| f32::from(herogpui_components::extend::key_radius(cx)));
    assert_corners(
        md_inner_mark(cx),
        [key_radius.min(8.); 4],
        "first frame must paint stock before set_theme",
    );

    apply_theme(
        cx,
        themed(
            ComponentThemes::default().slider(
                ComponentTheme::new(SliderStyle::default().radius(px(2.)))
                    .recipe("compact", SliderStyle::default().radius(px(3.))),
            ),
        ),
    );
    assert_corners(
        md_inner_mark(cx),
        [2.; 4],
        "theme defaults must reach the inner mark",
    );

    restore_stock(cx);
    assert_corners(
        md_inner_mark(cx),
        [key_radius.min(8.); 4],
        "switching back to the stock theme must restore stock corners",
    );
}

#[gpui::test]
fn slider_named_recipes_refine_defaults_and_ignore_missing_names(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("recipe-slider", 100.)
                    .recipe("compact")
                    .recipe("missing"),
            )
            .into_any_element()
    });
    apply_theme(
        cx,
        themed(
            ComponentThemes::default().slider(
                ComponentTheme::new(SliderStyle::default().radius(px(2.)))
                    .recipe("compact", SliderStyle::default().radius(px(3.))),
            ),
        ),
    );
    assert_corners(
        md_inner_mark(cx),
        [3.; 4],
        "named recipes must refine defaults; a missing name adds nothing",
    );
}

#[gpui::test]
fn slider_instance_sx_wins_named_corners_over_theme_radius(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(600.))
            .child(
                Slider::new("sx-slider", 100.)
                    .recipe("compact")
                    .sx(|el| el.rounded_tl(px(6.))),
            )
            .into_any_element()
    });
    apply_theme(
        cx,
        themed(
            ComponentThemes::default().slider(
                ComponentTheme::new(SliderStyle::default())
                    .recipe("compact", SliderStyle::default().radius(px(3.))),
            ),
        ),
    );
    assert_corners(
        md_inner_mark(cx),
        [6., 3., 3., 3.],
        "instance sx must win the named corner over the theme radius",
    );
}

#[gpui::test]
fn switch_theme_radius_reaches_the_track(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || Switch::new("theme-switch").into_any_element());
    apply_theme(
        cx,
        themed(
            ComponentThemes::default()
                .switch(ComponentTheme::new(SwitchStyle::default().radius(px(3.)))),
        ),
    );
    let (quads, scale) = cx.update(|window, _| (window.painted_quads(), window.scale_factor()));
    let track = quads
        .iter()
        .find(|quad| {
            close(quad.bounds.size.width.0 / scale, 40.)
                && close(quad.bounds.size.height.0 / scale, 20.)
        })
        .expect("Md switch track must paint");
    assert_corners(
        painted_corners(track, scale),
        [3.; 4],
        "switch theme radius must reach the track",
    );
}

#[gpui::test]
fn menu_theme_recipe_sets_row_interval(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        Menu::new(
            "theme-menu",
            vec![
                MenuItem::new("first", "First"),
                MenuItem::new("second", "Second"),
            ],
        )
        .recipe("compact")
        .animate_entry(false)
        .item_content(|key, _| {
            let key = key.clone();
            gpui::div()
                .h(px(20.))
                .w(px(40.))
                .debug_selector(move || format!("theme-{key}"))
                .into_any_element()
        })
        .into_any_element()
    });
    apply_theme(
        cx,
        themed(
            ComponentThemes::default().menu(
                ComponentTheme::new(MenuStyle::default().panel_gap(px(0.))).recipe(
                    "compact",
                    MenuStyle::default()
                        .row_height(px(28.))
                        .row_padding_y(px(2.))
                        .panel_padding(px(4.)),
                ),
            ),
        ),
    );
    let first = cx.debug_bounds("theme-first").expect("first row");
    let second = cx.debug_bounds("theme-second").expect("second row");
    assert_eq!(second.origin.y - first.origin.y, px(28.));
}

#[gpui::test]
fn menu_instance_panel_gap_wins_over_theme_defaults(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        Menu::new(
            "override-menu",
            vec![
                MenuItem::new("first", "First"),
                MenuItem::new("second", "Second"),
            ],
        )
        .animate_entry(false)
        .panel_gap(px(8.))
        .row_height(px(28.))
        .row_padding_y(px(2.))
        .panel_padding(px(4.))
        .item_content(|key, _| {
            let key = key.clone();
            gpui::div()
                .h(px(20.))
                .w(px(40.))
                .debug_selector(move || format!("override-{key}"))
                .into_any_element()
        })
        .into_any_element()
    });
    apply_theme(
        cx,
        themed(
            ComponentThemes::default()
                .menu(ComponentTheme::new(MenuStyle::default().panel_gap(px(0.)))),
        ),
    );
    let first = cx.debug_bounds("override-first").expect("first row");
    let second = cx.debug_bounds("override-second").expect("second row");
    assert_eq!(second.origin.y - first.origin.y, px(36.));
}

#[gpui::test]
fn button_theme_style_and_instance_sx_follow_precedence(cx: &mut TestAppContext) {
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                gpui::div()
                    .debug_selector(|| "stock-btn".into())
                    .child(Button::new("stock").label("Go")),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "recipe-btn".into())
                    .child(Button::new("recipe").label("Go").recipe("compact")),
            )
            .child(
                gpui::div().debug_selector(|| "sx-btn".into()).child(
                    Button::new("sx")
                        .label("Go")
                        .recipe("compact")
                        .sx(|el| el.h(px(40.))),
                ),
            )
            .into_any_element()
    });
    apply_theme(
        cx,
        themed(
            ComponentThemes::default().button(
                ComponentTheme::new(ButtonStyle::default())
                    .recipe("compact", ButtonStyle::default().style(|el| el.h(px(28.)))),
            ),
        ),
    );
    assert_eq!(cx.debug_bounds("stock-btn").unwrap().size.height, px(36.));
    assert_eq!(cx.debug_bounds("recipe-btn").unwrap().size.height, px(28.));
    assert_eq!(cx.debug_bounds("sx-btn").unwrap().size.height, px(40.));
}

#[gpui::test]
fn select_and_input_theme_heights_resolve_live(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let cx = open_host(cx, move || {
        gpui::div()
            .flex()
            .flex_col()
            .items_start()
            .child(Select::new(
                "theme-sel",
                vec![PickerItem::new("a", "Alpha")],
            ))
            .child(
                gpui::div()
                    .debug_selector(|| "theme-field".into())
                    .child(Input::new(state.clone()).recipe("compact")),
            )
            .into_any_element()
    });
    apply_theme(
        cx,
        themed(
            ComponentThemes::default()
                .select(ComponentTheme::new(SelectStyle::default().height(px(28.))))
                .text_field(
                    ComponentTheme::new(TextFieldStyle::default())
                        .recipe("compact", TextFieldStyle::default().height(px(28.))),
                ),
        ),
    );
    assert_eq!(
        cx.debug_bounds("select-trigger-Name(\"theme-sel\")")
            .unwrap()
            .size
            .height,
        px(28.)
    );
    assert_eq!(cx.debug_bounds("theme-field").unwrap().size.height, px(28.));

    restore_stock(cx);
    assert_eq!(
        cx.debug_bounds("select-trigger-Name(\"theme-sel\")")
            .unwrap()
            .size
            .height,
        px(36.)
    );
    assert_eq!(cx.debug_bounds("theme-field").unwrap().size.height, px(36.));
}
