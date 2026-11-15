//! The 0.10.0 capability hooks, each added for a measured divergence from a
//! HeroUI v3 reference that no existing builder, theme or `sx` could reach.
//!
//! Every test here pins both halves of the contract: the stock geometry or
//! behaviour that existing callers still get, and the opted-in one.

use crate::harness;

use gpui::{point, prelude::*, px, Modifiers, Pixels, TestAppContext, VisualTestContext};
use harness::{click, open_host, still};
use herogpui_components::{Button, Color, Menu, MenuItem, PickerItem, Select, Tooltip, Variant};
use herogpui_theme::{
    set_theme, ActiveTheme, ComponentTheme, ComponentThemes, MenuStyle, SelectStyle, Theme,
};

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

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

/// The painted separator band: the only quad filled with `colors.separator`
/// inside a menu whose rows and panel use unrelated fills.
fn separator_band(cx: &mut VisualTestContext) -> (f32, f32) {
    let (quads, scale, separator) = cx.update(|window, cx| {
        (
            window.painted_quads(),
            window.scale_factor(),
            cx.colors().separator,
        )
    });
    let band = quads
        .iter()
        .find(|quad| quad.background.as_solid() == Some(separator))
        .expect("the separator must paint a band in the separator colour");
    (
        band.bounds.size.width.0 / scale,
        band.bounds.size.height.0 / scale,
    )
}

fn separator_menu(id: &'static str) -> impl Fn() -> gpui::AnyElement {
    move || {
        Menu::new(
            id,
            vec![
                MenuItem::new("first", "First"),
                MenuItem::Separator,
                MenuItem::new("second", "Second"),
            ],
        )
        .animate_entry(false)
        // A fixed panel width makes the inset arithmetic exact rather than
        // dependent on the widest label's shaped width.
        .panel_min_width(px(200.))
        .panel_max_width(px(200.))
        .panel_padding(px(0.))
        .into_any_element()
    }
}

// -- Ask 1: menu separator geometry ---------------------------------------

/// `menu.css:8-11` and `dropdown.css:127-130`:
/// `[data-slot="separator"] { @apply ms-[3%] w-[94%] }`, the same rule
/// `list_box.rs` already ports. The panel used to draw the band full bleed.
#[gpui::test]
fn a_stock_menu_separator_takes_v3s_proportional_inset(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, separator_menu("stock-separator"));
    let (width, height) = separator_band(cx);
    let border = cx.update(|_, cx| cx.layout().border_width);
    assert!(
        (width - 188.).abs() < f32::EPSILON,
        "94% of a 200pt panel, centred by a 3% margin"
    );
    assert!(
        (height - f32::from(border)).abs() < 0.05,
        "the stock rule is the theme hairline, got {height}"
    );
}

#[gpui::test]
fn a_theme_can_inset_and_thicken_the_menu_separator(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, separator_menu("themed-separator"));
    apply_theme(
        cx,
        themed(
            ComponentThemes::default().menu(ComponentTheme::new(
                // AppKit's own menu separator: 15pt in on each side, 1pt thick.
                MenuStyle::default()
                    .separator_inset(px(15.))
                    .separator_thickness(px(1.)),
            )),
        ),
    );
    let (width, height) = separator_band(cx);
    assert!(
        (width - 170.).abs() < f32::EPSILON,
        "an absolute inset replaces the proportional one: 15pt off each side of a 200pt panel"
    );
    assert!((height - 1.).abs() < f32::EPSILON);
}

#[gpui::test]
fn a_menu_instance_can_inset_the_separator_without_a_theme(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        Menu::new(
            "instance-separator",
            vec![
                MenuItem::new("first", "First"),
                MenuItem::Separator,
                MenuItem::new("second", "Second"),
            ],
        )
        .animate_entry(false)
        .panel_min_width(px(200.))
        .panel_max_width(px(200.))
        .panel_padding(px(0.))
        .separator_inset(px(20.))
        .separator_thickness(px(2.))
        .into_any_element()
    });
    let (width, height) = separator_band(cx);
    assert!((width - 160.).abs() < f32::EPSILON);
    assert!((height - 2.).abs() < f32::EPSILON);
}

// -- Ask 2: select trigger vertical padding --------------------------------

fn trigger_height(cx: &mut VisualTestContext, name: &'static str) -> f32 {
    let height = cx
        .debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} must render"))
        .size
        .height;
    f32::from(height)
}

/// A label long enough to wrap inside a narrow trigger, so the trigger's
/// content is two lines tall rather than one.
const WRAPPING: &str = "Side by side with the preview pane";

fn wrapping_select(id: &'static str, padding_y: Option<Pixels>) -> Select {
    let mut select = Select::new(id, vec![PickerItem::new("wide", WRAPPING)])
        .value(Some("wide".into()))
        .full_width(true);
    if let Some(p) = padding_y {
        select = select.padding_y(p);
    }
    select
}

#[gpui::test]
fn select_padding_y_leaves_a_single_line_trigger_at_36(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(320.))
            .child(gpui::div().debug_selector(|| "stock".into()).child(
                Select::new("stock", vec![PickerItem::new("a", "A")]).value(Some("a".into())),
            ))
            .child(
                gpui::div().debug_selector(|| "padded".into()).child(
                    Select::new("padded", vec![PickerItem::new("a", "A")])
                        .value(Some("a".into()))
                        .padding_y(px(8.)),
                ),
            )
            .into_any_element()
    });
    assert!(
        (trigger_height(cx, "stock") - 36.).abs() < f32::EPSILON,
        "the stock single-line trigger is min-h-9"
    );
    assert!(
        (trigger_height(cx, "padded") - 36.).abs() < f32::EPSILON,
        "py-2 over a 20px line advance still resolves to the 36px minimum"
    );
}

#[gpui::test]
fn select_padding_y_grows_a_wrapped_trigger(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(140.))
            .child(
                gpui::div()
                    .debug_selector(|| "stock".into())
                    .child(wrapping_select("stock-wrap", None)),
            )
            .child(
                gpui::div()
                    .debug_selector(|| "padded".into())
                    .child(wrapping_select("padded-wrap", Some(px(8.)))),
            )
            .into_any_element()
    });
    let stock = trigger_height(cx, "stock");
    let padded = trigger_height(cx, "padded");
    assert!(
        stock > 36.,
        "the fixture must actually wrap; got {stock} for the stock trigger"
    );
    assert!(
        (padded - stock - 16.).abs() < f32::EPSILON,
        "padding_y adds its 8px to each edge of the wrapped trigger"
    );
}

#[gpui::test]
fn select_padding_y_can_come_from_the_theme(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(140.))
            .child(
                gpui::div()
                    .debug_selector(|| "themed".into())
                    .child(wrapping_select("themed-wrap", None)),
            )
            .into_any_element()
    });
    let stock = trigger_height(cx, "themed");
    apply_theme(
        cx,
        themed(ComponentThemes::default().select(ComponentTheme::new(
            SelectStyle::default().padding_y(px(8.)),
        ))),
    );
    assert!((trigger_height(cx, "themed") - stock - 16.).abs() < f32::EPSILON);
}

#[gpui::test]
fn a_select_instance_padding_y_wins_over_the_theme(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        gpui::div()
            .w(px(140.))
            .child(
                gpui::div()
                    .debug_selector(|| "instance".into())
                    .child(wrapping_select("instance-wrap", Some(px(4.)))),
            )
            .into_any_element()
    });
    let themed_out = {
        apply_theme(
            cx,
            themed(ComponentThemes::default().select(ComponentTheme::new(
                SelectStyle::default().padding_y(px(20.)),
            ))),
        );
        trigger_height(cx, "instance")
    };
    apply_theme(cx, Theme::light());
    let stock = trigger_height(cx, "instance");
    assert!(
        (themed_out - stock).abs() < f32::EPSILON,
        "the instance's 4px must survive a theme asking for 20px"
    );
}

// -- Ask 4: per-row leading content in Select ------------------------------

#[gpui::test]
fn select_item_leading_draws_one_element_per_open_row(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        Select::new(
            "swatches",
            vec![PickerItem::new("red", "Red"), PickerItem::new("sky", "Sky")],
        )
        .is_open(true)
        .item_leading(|key, _| {
            let key = key.clone();
            Some(
                gpui::div()
                    .size(px(12.))
                    .debug_selector(move || format!("swatch-{key}"))
                    .into_any_element(),
            )
        })
        .into_any_element()
    });
    let red = cx.debug_bounds("swatch-red").expect("a swatch per row");
    let sky = cx.debug_bounds("swatch-sky").expect("a swatch per row");
    assert!((f32::from(red.size.width) - 12.).abs() < f32::EPSILON);
    assert!(
        sky.origin.y > red.origin.y,
        "each row draws its own leading element"
    );
}

#[gpui::test]
fn select_item_leading_may_skip_a_row(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        Select::new(
            "partial",
            vec![PickerItem::new("red", "Red"), PickerItem::new("none", "—")],
        )
        .is_open(true)
        .item_leading(|key, _| {
            (key.as_ref() == "red").then(|| {
                gpui::div()
                    .size(px(12.))
                    .debug_selector(|| "swatch-red".into())
                    .into_any_element()
            })
        })
        .into_any_element()
    });
    assert!(cx.debug_bounds("swatch-red").is_some());
}

// -- Ask 5: rich tooltip bodies --------------------------------------------

#[gpui::test]
fn a_tooltip_body_replaces_the_measured_single_line(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        Tooltip::new("Tokens: name, kind")
            .id("rich")
            .delay(0)
            .body(|_, _| {
                gpui::div()
                    .w(px(180.))
                    .h(px(60.))
                    .debug_selector(|| "rich-body".into())
                    .into_any_element()
            })
            .child(gpui::div().w(px(120.)).h(px(36.)))
            // Inset from the origin, where the test platform's pointer starts:
            // a trigger under it is hovered on the very first frame.
            .map(|tip| gpui::div().p(px(50.)).child(tip))
            .into_any_element()
    });
    assert!(
        cx.debug_bounds("rich-body").is_none(),
        "a closed tooltip must not build its body"
    );
    // The trigger is a 120x36 box inset by 50, so its centre is (110, 68).
    cx.simulate_mouse_move(point(px(110.), px(68.)), None, Modifiers::none());
    flush_frame(cx);
    let body = cx
        .debug_bounds("rich-body")
        .expect("the tip must draw the caller's body");
    assert!(
        (f32::from(body.size.height) - 60.).abs() < f32::EPSILON,
        "the element keeps its own intrinsic height, unmeasured"
    );
    assert!(
        (f32::from(body.size.width) - 180.).abs() < f32::EPSILON,
        "and its own width, under the same 320px cap the string path uses"
    );
}

// -- Ask 3: interactive menu rows ------------------------------------------

/// A menu whose second row hosts a 40x20 probe. `dismissed` records whether the
/// menu's dismissal ran, which is what closes it in a real Dropdown.
fn interactive_menu(
    interactive: bool,
    dismissed: harness::Events,
) -> impl Fn() -> gpui::AnyElement {
    move || {
        let dismissed = dismissed.clone();
        Menu::new(
            "hosting-menu",
            vec![
                MenuItem::new("plain", "Plain"),
                MenuItem::new("hosted", "Hosted").is_interactive(interactive),
            ],
        )
        .animate_entry(false)
        .on_dismiss(move |_, _, _| dismissed.borrow_mut().push("dismiss".to_owned()))
        .item_content(|key, _| {
            let key = key.clone();
            gpui::div()
                .h(px(20.))
                .w(px(40.))
                .debug_selector(move || format!("row-{key}"))
                .into_any_element()
        })
        .into_any_element()
    }
}

#[gpui::test]
fn a_stock_menu_row_dismisses_the_menu_when_pressed(cx: &mut TestAppContext) {
    still();
    let dismissed = harness::events();
    let cx = open_host(cx, interactive_menu(false, dismissed.clone()));
    let row = cx.debug_bounds("row-hosted").expect("the hosted row");
    click(cx, f32::from(row.center().x), f32::from(row.center().y));
    assert_eq!(
        dismissed.borrow().len(),
        1,
        "the stock row is one button: pressing it closes the menu"
    );
}

#[gpui::test]
fn an_interactive_menu_row_leaves_its_content_the_press(cx: &mut TestAppContext) {
    still();
    let dismissed = harness::events();
    let cx = open_host(cx, interactive_menu(true, dismissed.clone()));
    let row = cx.debug_bounds("row-hosted").expect("the hosted row");
    click(cx, f32::from(row.center().x), f32::from(row.center().y));
    assert!(
        dismissed.borrow().is_empty(),
        "an interactive row must not report a pick or dismiss the menu"
    );
    let plain = cx.debug_bounds("row-plain").expect("the plain row");
    click(cx, f32::from(plain.center().x), f32::from(plain.center().y));
    assert_eq!(
        dismissed.borrow().len(),
        1,
        "its neighbours are unaffected and still dismiss"
    );
}

#[gpui::test]
fn an_interactive_menu_row_ignores_enter(cx: &mut TestAppContext) {
    still();
    let dismissed = harness::events();
    let cx = open_host(cx, interactive_menu(true, dismissed.clone()));
    // Down seats the cursor on the first row, a second Down on the hosted one.
    harness::press(cx, "down down enter");
    assert!(
        dismissed.borrow().is_empty(),
        "Enter belongs to the hosted control, not to the row"
    );
}

#[gpui::test]
fn a_select_hosted_in_an_interactive_row_opens_without_closing_the_menu(cx: &mut TestAppContext) {
    still();
    let dismissed = harness::events();
    let for_menu = dismissed.clone();
    let cx = open_host(cx, move || {
        let dismissed = for_menu.clone();
        Menu::new(
            "hosting-select",
            vec![MenuItem::new("layout", "Layout").is_interactive(true)],
        )
        .animate_entry(false)
        .on_dismiss(move |_, _, _| dismissed.borrow_mut().push("dismiss".to_owned()))
        .item_content(|_, _| {
            gpui::div()
                .debug_selector(|| "hosted-select".into())
                .child(
                    Select::new(
                        "inline",
                        vec![
                            PickerItem::new("side", "Side by side"),
                            PickerItem::new("stack", "Stacked"),
                        ],
                    )
                    .default_value(Some("side".into()))
                    // The row's own popup: an inline secondary control, not a
                    // submenu flyout.
                    .item_leading(|_, _| {
                        Some(
                            gpui::div()
                                .size(px(8.))
                                .debug_selector(|| "inline-row".into())
                                .into_any_element(),
                        )
                    }),
                )
                .into_any_element()
        })
        .into_any_element()
    });
    let trigger = cx
        .debug_bounds("hosted-select")
        .expect("the hosted select must render inside the row");
    assert!(
        cx.debug_bounds("inline-row").is_none(),
        "the hosted select starts closed"
    );
    click(
        cx,
        f32::from(trigger.center().x),
        f32::from(trigger.center().y),
    );
    assert!(
        dismissed.borrow().is_empty(),
        "pressing the hosted control must not dismiss the menu around it"
    );
    assert!(
        cx.debug_bounds("inline-row").is_some(),
        "and the press must reach the control, opening its own list"
    );
}

#[gpui::test]
fn a_select_trigger_does_not_activate_the_element_around_it(cx: &mut TestAppContext) {
    still();
    let outer = harness::events();
    let for_view = outer.clone();
    let cx = open_host(cx, move || {
        let outer = for_view.clone();
        gpui::div()
            .id("pressable-row")
            // Sized to the trigger, so the row's centre is the trigger's: an
            // auto-width block would stretch past it and the press would land
            // on the row alone.
            .w(px(200.))
            .debug_selector(|| "pressable-row".into())
            .on_click(move |_, _, _| outer.borrow_mut().push("outer".to_owned()))
            .child(
                Select::new("embedded", vec![PickerItem::new("a", "A")])
                    .default_value(Some("a".into()))
                    .full_width(true)
                    .item_leading(|_, _| {
                        Some(
                            gpui::div()
                                .size(px(8.))
                                .debug_selector(|| "embedded-row".into())
                                .into_any_element(),
                        )
                    }),
            )
            .into_any_element()
    });
    let row = cx
        .debug_bounds("pressable-row")
        .expect("the row must render");
    click(cx, f32::from(row.center().x), f32::from(row.center().y));
    assert!(
        cx.debug_bounds("embedded-row").is_some(),
        "the fixture must actually press the trigger"
    );
    assert!(
        outer.borrow().is_empty(),
        "a press the select trigger consumed must not also fire the enclosing handler"
    );
}

/// The option rows inside the open popup must consume their pick the way the
/// trigger consumes its press: a select hosted in a clickable ancestor -- an
/// `is_interactive` menu row, a pressable card -- closes its list without the
/// enclosing handler hearing any of it. Opening is uncontrolled
/// (`default_open`) so the pick closes the list itself, exactly as it would
/// over a real host.
#[gpui::test]
fn a_select_option_pick_does_not_activate_the_element_around_it(cx: &mut TestAppContext) {
    still();
    let outer = harness::events();
    let picks = harness::events();
    let opens = harness::events();
    let for_view = (outer.clone(), picks.clone(), opens.clone());
    let cx = open_host(cx, move || {
        let (outer, picks, opens) = for_view.clone();
        gpui::div()
            .id("pickable-row")
            .w(px(200.))
            // Taller than the trigger, so the strip below it is the ancestor's
            // own press target: the liveness probe at the end (run after the
            // closed panel stops covering it) proves the recorder hears a
            // click the select does not consume.
            .h(px(60.))
            .debug_selector(|| "pickable-row".into())
            .on_click(move |_, _, _| outer.borrow_mut().push("outer".to_owned()))
            .child(
                Select::new(
                    "embedded",
                    vec![PickerItem::new("a", "A"), PickerItem::new("b", "B")],
                )
                .default_value(Some("a".into()))
                .default_open(true)
                .full_width(true)
                .on_open_change(move |open, _, _| {
                    opens.borrow_mut().push(format!("open:{open}"));
                })
                .on_selection_change(move |value, _, _| {
                    picks.borrow_mut().push(format!(
                        "pick:{}",
                        value.as_ref().map_or("", |key| key.as_ref())
                    ));
                }),
            )
            .into_any_element()
    });
    // The fixture is live: a press on the ancestor's own strip reaches it.
    let option = cx
        .debug_bounds("select-list-Name(\"embedded\")-opt-1")
        .expect("the open list must render its second option");
    click(
        cx,
        f32::from(option.center().x),
        f32::from(option.center().y),
    );
    assert_eq!(
        picks.borrow().as_slice(),
        ["pick:b"],
        "the click must reach the option row itself"
    );
    assert!(
        outer.borrow().is_empty(),
        "a pick the option row consumed must not also fire the enclosing handler"
    );
    assert_eq!(
        opens.borrow().as_slice(),
        ["open:false"],
        "and the list must close through its normal pick path"
    );
    // Prove the closed surface where its row used to be, past the exit run:
    // no second pick answers there, and the enclosure still never fires.
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(150));
    flush_frame(cx);
    cx.run_until_parked();
    click(
        cx,
        f32::from(option.center().x),
        f32::from(option.center().y),
    );
    assert_eq!(
        picks.borrow().as_slice(),
        ["pick:b"],
        "the popup must be gone, not still open under the click"
    );
    assert!(outer.borrow().is_empty());
    // Finally the liveness probe, now that the closed panel no longer covers
    // the ancestor's own strip: the recorder does hear a click the select
    // leaves unconsumed, so the silences above are the row consuming its pick.
    click(cx, 100., 48.);
    assert_eq!(outer.borrow().as_slice(), ["outer"]);
}

// -- Contract pins from the adversarial review ------------------------------

/// `MenuItem::is_interactive` is documented as ignored on a row that opens a
/// submenu (`dropdown.rs`: a submenu trigger already skips the row click, and
/// `row_is_interactive` is `is_interactive && !has_submenu`). The flag must be
/// *silently* dropped: the row behaves exactly like an unflagged submenu
/// trigger -- it opens its child panel, and picking a submenu row dismisses
/// the menu once, as a plain pick does.
#[gpui::test]
fn is_interactive_is_dropped_on_a_submenu_trigger(cx: &mut TestAppContext) {
    still();
    let dismissed = harness::events();
    let for_menu = dismissed.clone();
    let cx = open_host(cx, move || {
        let dismissed = for_menu.clone();
        Menu::new(
            "flagged-branch",
            vec![MenuItem::new("branch", "Branch")
                .is_interactive(true)
                .submenu(vec![MenuItem::new("leaf", "Leaf")])],
        )
        .animate_entry(false)
        .on_dismiss(move |_, _, _| dismissed.borrow_mut().push("dismiss".to_owned()))
        .item_content(|key, _| {
            let key = key.clone();
            gpui::div()
                .h(px(20.))
                .w(px(40.))
                .debug_selector(move || format!("row-{key}"))
                .into_any_element()
        })
        .into_any_element()
    });
    let row = cx
        .debug_bounds("row-branch")
        .expect("the flagged trigger row");
    click(cx, f32::from(row.center().x), f32::from(row.center().y));
    flush_frame(cx);
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("dropdown-submenu").is_some(),
        "the flag must not change the row: it still opens its submenu"
    );
    assert!(
        dismissed.borrow().is_empty(),
        "opening the submenu must not pick or dismiss anything"
    );
    let leaf = cx.debug_bounds("row-leaf").expect("the submenu's own row");
    click(cx, f32::from(leaf.center().x), f32::from(leaf.center().y));
    assert_eq!(
        dismissed.borrow().len(),
        1,
        "and a submenu pick dismisses exactly once, as a plain row does"
    );
}

/// The instance separator settings a menu copies onto its submenu
/// (`dropdown.rs` forwards `separator_inset`/`separator_thickness` with the
/// rest of the presentation) must reach the child panel's own band. The parent
/// carries no separator, so the one painted is the submenu's.
#[gpui::test]
fn a_submenus_separator_inherits_the_instance_inset_and_thickness(cx: &mut TestAppContext) {
    still();
    let cx = open_host(cx, move || {
        Menu::new(
            "inheriting-menu",
            vec![MenuItem::new("branch", "Branch").submenu(vec![
                MenuItem::new("leaf", "Leaf"),
                MenuItem::Separator,
                MenuItem::new("leaf-two", "Leaf two"),
            ])],
        )
        .animate_entry(false)
        // A fixed panel width -- propagated onto the submenu with the rest of
        // the presentation -- keeps the inset arithmetic exact rather than
        // dependent on the widest label's shaped width.
        .panel_min_width(px(200.))
        .panel_max_width(px(200.))
        .panel_padding(px(0.))
        .separator_inset(px(20.))
        .separator_thickness(px(2.))
        .item_content(|key, _| {
            let key = key.clone();
            gpui::div()
                .h(px(20.))
                .w(px(40.))
                .debug_selector(move || format!("row-{key}"))
                .into_any_element()
        })
        .into_any_element()
    });
    let row = cx
        .debug_bounds("row-branch")
        .expect("the submenu trigger row");
    click(cx, f32::from(row.center().x), f32::from(row.center().y));
    flush_frame(cx);
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("dropdown-submenu").is_some(),
        "the submenu must be open before its band is measured"
    );
    let (width, height) = separator_band(cx);
    assert!(
        (width - 160.).abs() < f32::EPSILON,
        "the submenu's band inherits the 20pt inset on each side of its 200pt panel"
    );
    assert!(
        (height - 2.).abs() < f32::EPSILON,
        "and the instance's 2pt thickness"
    );
}

/// A theme-level `role_hover("accent", ..)` names the `*-hover` shade outright,
/// and `Variant::Primary` resolves its hover endpoint through
/// `colors.accent.hover()` -- so the named colour is what the button actually
/// paints on hover, not just what the resolver returns. Under reduced motion
/// the hover fill swaps in one frame with no fade, so the painted quad is the
/// endpoint itself.
#[gpui::test]
fn a_theme_role_hover_reaches_the_fill_a_primary_button_hovers_to(cx: &mut TestAppContext) {
    still();
    // A green no derivation of the stock palette can produce, so a match can
    // only come from the named override.
    let named = gpui::rgb(0x00ff8c).into();
    let cx = open_host(cx, move || {
        // `Button` carries no debug selector of its own; the unpadded wrapper's
        // origin is the button's, so a point just inside it is on the control.
        gpui::div()
            .debug_selector(|| "themed-primary-wrap".into())
            .child(
                Button::new("themed-primary")
                    .label("Go")
                    .variant(Variant::Primary),
            )
            .into_any_element()
    });
    let wrap = cx
        .debug_bounds("themed-primary-wrap")
        .expect("the button must render");
    let on_the_button = point(wrap.origin.x + px(10.), wrap.origin.y + px(10.));
    assert!(
        !paints_solid(cx, named),
        "the override must not be on screen before it is named"
    );
    apply_theme(
        cx,
        Theme::builder("app", Theme::light())
            .role_hover(Color::Accent, named)
            .build(),
    );
    cx.simulate_mouse_move(on_the_button, None, Modifiers::none());
    flush_frame(cx);
    flush_frame(cx);
    assert!(
        paints_solid(cx, named),
        "the hovered primary button must paint the named accent hover outright"
    );
}

/// Whether any painted quad is filled with exactly `color`.
fn paints_solid(cx: &mut VisualTestContext, color: gpui::Hsla) -> bool {
    cx.update(|window, _| {
        window
            .painted_quads()
            .iter()
            .any(|quad| quad.background.as_solid() == Some(color))
    })
}
