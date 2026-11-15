//! Per-component `radius` builders under the narrow parity exception: the
//! default stays the owning `util` helper, and the builder replaces it.
//!
//! Proved on the painted scene, the way `radius_painted.rs` proves the
//! components that need no open surface: this file reaches the overlay
//! panels (opened through their controlled/default-open builders or real
//! pointer input), the field-family boxes (which paint through the shared
//! field chrome), and the composites whose inner parts must keep their own
//! shape while the root takes the override.

use crate::harness;

use herogpui_theme::ActiveTheme;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    point, prelude::*, px, AnyElement, App, Modifiers, MouseButton, Pixels, TestAppContext,
    VisualTestContext,
};
use harness::{open_host, painted, press, settle, still, wait_ms, Painted};
use herogpui_components::extend::{
    container_radius, control_radius, field_radius, key_radius, small_radius, soft_radius,
};
use herogpui_components::{
    clear_toasts, Accordion, AccordionItem, AccordionVariant, AlertDialog, Autocomplete, Badge,
    BadgeAnchor, Button, Checkbox, CloseButton, ColorField, ComboBox, DateField, Dropdown, Input,
    InputGroup, InputOTP, InputState, Link, MenuItem, Modal, OtpState, PickerColor, PickerItem,
    Popover, RadioGroup, RadioOption, Select, Size, Table, Tag, TagGroup, TimeField, TimeState,
    Toast, ToastViewport, ToggleButton, Tooltip, Typography,
};

/// An override no helper or token produces, and small enough that no box
/// under test clamps it to half its side.
const OVERRIDE: f32 = 1.75;

type Helper = fn(&App) -> Pixels;
type Build = Rc<dyn Fn(Option<Pixels>) -> AnyElement>;

/// One component whose default paints `helper` and whose `.radius(..)`
/// replaces it. `drive` runs after the window opens (a hover, a Tab) and
/// `ring` says the observed box is the offset focus ring's gap band, whose
/// corners sit `ring_offset_width` outside the control's own.
struct Case {
    name: &'static str,
    helper: Helper,
    ring: bool,
    build: Build,
    drive: fn(&mut VisualTestContext),
    setup: Option<fn(Option<Pixels>, &mut App)>,
}

fn case(name: &'static str, helper: Helper, build: Build) -> Case {
    Case {
        name,
        helper,
        ring: false,
        build,
        drive: settle,
        setup: None,
    }
}

fn with<T>(el: T, radius: Option<Pixels>, set: impl FnOnce(T, Pixels) -> T) -> T {
    match radius {
        Some(r) => set(el, r),
        None => el,
    }
}

fn items() -> Vec<PickerItem> {
    vec![PickerItem::new("a", "Alpha"), PickerItem::new("b", "Beta")]
}

fn host(build: Build, radius: Option<Pixels>) -> impl Fn() -> AnyElement {
    move || {
        gpui::div()
            .p(px(24.))
            .w(px(480.))
            .h(px(600.))
            .child(build(radius))
            .into_any_element()
    }
}

/// The painted scene of one render, plus the value the default must paint.
fn paint(cx: &mut TestAppContext, case: &Case, radius: Option<Pixels>) -> (Painted, f32) {
    still();
    if let Some(setup) = case.setup {
        cx.update(|cx| {
            clear_toasts(cx);
            setup(radius, cx);
        });
    }
    let vcx = open_host(cx, host(case.build.clone(), radius));
    (case.drive)(vcx);
    let scene = painted(vcx);
    let helper = case.helper;
    let ring = case.ring;
    let value = vcx.update(|_, cx| {
        f32::from(helper(cx))
            + if ring {
                f32::from(ActiveTheme::layout(cx).ring_offset_width)
            } else {
                0.
            }
    });
    (scene, value)
}

fn hover_trigger(cx: &mut VisualTestContext) {
    settle(cx);
    cx.simulate_mouse_move(point(px(60.), px(40.)), None, Modifiers::none());
    settle(cx);
    cx.executor().advance_clock(Duration::from_millis(50));
    settle(cx);
}

fn tab_in(cx: &mut VisualTestContext) {
    settle(cx);
    press(cx, "tab");
    settle(cx);
}

fn cases(cx: &mut TestAppContext) -> Vec<Case> {
    let input = cx.new(|cx| InputState::new(cx));
    let grouped = cx.new(|cx| InputState::new(cx));
    let date = cx.new(|cx| InputState::new(cx));
    let combo = cx.new(|cx| InputState::new(cx));
    let auto = cx.new(|cx| InputState::new(cx));
    let time = cx.new(|cx| TimeState::new(cx));
    let otp = cx.new(|cx| OtpState::with_length(cx, 4));
    vec![
        Case {
            drive: hover_trigger,
            ..case(
                "Tooltip",
                small_radius,
                Rc::new(|r| {
                    with(
                        Tooltip::new("Saved")
                            .id("tt")
                            .delay(0)
                            .child(gpui::div().w(px(120.)).h(px(36.))),
                        r,
                        Tooltip::radius,
                    )
                    .into_any_element()
                }),
            )
        },
        case(
            "Modal",
            container_radius,
            Rc::new(|r| {
                with(
                    Modal::new().id("md").is_open(true).child("Body"),
                    r,
                    Modal::radius,
                )
                .into_any_element()
            }),
        ),
        Case {
            setup: Some(|r, cx| {
                with(
                    Toast::new("Saved").timeout(Duration::ZERO),
                    r,
                    Toast::radius,
                )
                .push(None, cx);
            }),
            ..case(
                "Toast",
                container_radius,
                Rc::new(|_| ToastViewport::new().into_any_element()),
            )
        },
        case(
            "Popover",
            container_radius,
            Rc::new(|r| {
                with(
                    Popover::new(gpui::div().w(px(40.)).h(px(36.)))
                        .id("pop")
                        .is_open(true)
                        .child("Body"),
                    r,
                    Popover::radius,
                )
                .into_any_element()
            }),
        ),
        case(
            "Dropdown",
            |cx| (cx.layout().radius_2xl() + px(6.)).min(px(32.)),
            Rc::new(|r| {
                with(
                    Dropdown::new(
                        "dd",
                        Button::new("dd-t").label("Open"),
                        vec![MenuItem::new("a", "Alpha")],
                        true,
                    ),
                    r,
                    Dropdown::radius,
                )
                .into_any_element()
            }),
        ),
        case(
            "Input",
            field_radius,
            Rc::new(move |r| with(Input::new(input.clone()), r, Input::radius).into_any_element()),
        ),
        case(
            "InputOTP",
            field_radius,
            Rc::new(move |r| {
                with(InputOTP::new(otp.clone()), r, InputOTP::radius).into_any_element()
            }),
        ),
        case(
            "TimeField",
            field_radius,
            Rc::new(move |r| {
                with(TimeField::new(time.clone()), r, TimeField::radius).into_any_element()
            }),
        ),
        case(
            "DateField",
            field_radius,
            Rc::new(move |r| {
                with(DateField::new(date.clone()), r, DateField::radius).into_any_element()
            }),
        ),
        case(
            "ColorField",
            field_radius,
            Rc::new(|r| {
                with(
                    ColorField::new("cf", PickerColor::from_hex("#FF0000")),
                    r,
                    ColorField::radius,
                )
                .into_any_element()
            }),
        ),
        case(
            "Select (panel)",
            |cx| (cx.layout().radius_2xl() + px(6.)).min(px(32.)),
            Rc::new(|r| {
                with(
                    Select::new("sel", items()).default_open(true),
                    r,
                    Select::radius,
                )
                .into_any_element()
            }),
        ),
        case(
            "ComboBox (panel)",
            |cx| (cx.layout().radius_2xl() + px(6.)).min(px(32.)),
            Rc::new(move |r| {
                with(
                    ComboBox::new(combo.clone(), items()).default_open(true),
                    r,
                    ComboBox::radius,
                )
                .into_any_element()
            }),
        ),
        case(
            "Autocomplete (panel)",
            |cx| (cx.layout().radius_2xl() + px(6.)).min(px(32.)),
            Rc::new(move |r| {
                with(
                    Autocomplete::new(auto.clone(), items()).default_open(true),
                    r,
                    Autocomplete::radius,
                )
                .into_any_element()
            }),
        ),
        case(
            "InputGroup",
            field_radius,
            Rc::new(move |r| {
                with(
                    InputGroup::new().input(Input::new(grouped.clone())),
                    r,
                    InputGroup::radius,
                )
                .into_any_element()
            }),
        ),
        case(
            "AlertDialog",
            container_radius,
            Rc::new(|r| {
                with(
                    AlertDialog::new("Delete?").id("ad").is_open(true),
                    r,
                    AlertDialog::radius,
                )
                .into_any_element()
            }),
        ),
        Case {
            ring: true,
            drive: tab_in,
            ..case(
                "Link (focus ring)",
                small_radius,
                Rc::new(|r| {
                    with(
                        Link::new("lnk").label("Docs").href("#/docs"),
                        r,
                        Link::radius,
                    )
                    .into_any_element()
                }),
            )
        },
    ]
}

#[gpui::test]
fn radius_builders_override_their_helper_defaults(cx: &mut TestAppContext) {
    let mut checked = 0;
    for case in cases(cx) {
        let name = case.name;
        let ring = if case.ring { 2. } else { 0. };
        let (scene, value) = paint(cx, &case, None);
        let helper_boxes = scene.rounded(value).len();
        assert!(
            scene.rounded_exact(OVERRIDE + ring).is_empty() && helper_boxes > 0,
            "{name}: the default must paint the helper's {value}px, not the override\n{}",
            scene.describe()
        );
        checked += 1;

        let (scene, _) = paint(cx, &case, Some(px(OVERRIDE)));
        let remaining = scene.rounded(value).len();
        assert!(
            !scene.rounded_exact(OVERRIDE + ring).is_empty() && remaining < helper_boxes,
            "{name}: `.radius({OVERRIDE})` must replace the painted {value}px \
             ({helper_boxes} helper boxes before, {remaining} after)"
        );
        checked += 1;
    }
    assert_eq!(checked, 32, "all 16 cases ran both of their checks");
}

/// Select, ComboBox and Autocomplete round their detached *panel*; the
/// trigger box stays the shared field chrome's, so a closed picker with an
/// override still paints `field_radius` and nothing at the override.
#[gpui::test]
fn detached_panel_overrides_do_not_reach_the_trigger_box(cx: &mut TestAppContext) {
    let combo = cx.new(|cx| InputState::new(cx));
    let triggers: Vec<(&str, Build)> = vec![
        (
            "Select",
            Rc::new(|_| {
                Select::new("sel-c", items())
                    .radius(px(OVERRIDE))
                    .into_any_element()
            }),
        ),
        (
            "ComboBox",
            Rc::new(move |_| {
                ComboBox::new(combo.clone(), items())
                    .radius(px(OVERRIDE))
                    .into_any_element()
            }),
        ),
    ];
    for (name, build) in triggers {
        still();
        let vcx = open_host(cx, host(build, None));
        let scene = painted(vcx);
        let field = vcx.update(|_, cx| f32::from(field_radius(cx)));
        assert!(
            scene.rounded_exact(OVERRIDE).is_empty() && !scene.rounded(field).is_empty(),
            "{name}: the closed trigger must keep the field chrome's {field}px\n{}",
            scene.describe()
        );
    }
}

/// `Autocomplete::radius` documents the same split — "the trigger is a field
/// box of its own, painted by the shared field chrome — `--field-radius`,
/// not this value". Until 0.13 the render resolved the trigger radius from
/// the override; the 0.13 test conversion found it (the source-text check it
/// replaced looked only for the literal `Some(radius),`).
#[gpui::test]
fn autocomplete_panel_override_does_not_reach_the_trigger_box(cx: &mut TestAppContext) {
    let auto = cx.new(|cx| InputState::new(cx));
    still();
    let vcx = open_host(cx, move || {
        Autocomplete::new(auto.clone(), items())
            .radius(px(OVERRIDE))
            .into_any_element()
    });
    let scene = painted(vcx);
    let field = vcx.update(|_, cx| f32::from(field_radius(cx)));
    assert!(
        scene.rounded_exact(OVERRIDE).is_empty() && !scene.rounded(field).is_empty(),
        "Autocomplete: the closed trigger must keep the field chrome's {field}px\n{}",
        scene.describe()
    );
}

/// `Table::radius` replaces the wrapper shell's helper value, which paints
/// only where the shell's corner shows: the `Primary` tray paints its own
/// pinned radius over both, and in `Secondary` the last row's fill rounds
/// its bottom corners to the shell's. Hover that row and read them.
#[gpui::test]
fn table_override_reaches_the_shell_corner_the_rows_round_to(cx: &mut TestAppContext) {
    use herogpui_components::TableVariant;
    for radius in [None, Some(px(OVERRIDE))] {
        still();
        let vcx = open_host(cx, move || {
            gpui::div()
                .p(px(24.))
                .w(px(480.))
                .child(with(
                    Table::new(vec!["Name".into()])
                        .id("tbl")
                        .variant(TableVariant::Secondary)
                        .row(vec![gpui::div().child("Ada").into_any_element()]),
                    radius,
                    Table::radius,
                ))
                .into_any_element()
        });
        settle(vcx);
        let before = painted(vcx);
        vcx.simulate_mouse_move(point(px(240.), px(80.)), None, Modifiers::none());
        let scene = painted(vcx);
        let helper = vcx.update(|_, cx| f32::from(container_radius(cx)));
        let fills = scene
            .quads
            .iter()
            .filter(|q| q.background.as_solid().is_some_and(|c| c.a > 0.))
            .filter(|q| {
                !before
                    .quads
                    .iter()
                    .any(|b| b.bounds == q.bounds && b.background == q.background)
            })
            .collect::<Vec<_>>();
        assert!(
            !fills.is_empty(),
            "hovering the row must paint its fill\n{}",
            scene.describe()
        );
        let want = match radius {
            Some(r) => f32::from(r),
            None => helper,
        };
        let bottom_round = fills.iter().any(|q| {
            let b = scene.bounds(q);
            let want = want.min(f32::from(b.size.height) / 2.);
            let [tl, tr, br, bl] = scene.corners(q);
            tl < 0.05 && tr < 0.05 && (br - want).abs() < 0.05 && (bl - want).abs() < 0.05
        });
        assert!(
            bottom_round,
            "radius {radius:?}: the last row's fill must round its bottom corners \
             to the shell's {want}px\n{}",
            scene.describe()
        );
    }
}

/// `ColorField`'s editable path renders the inner `Input`'s own box, so the
/// override rides along with the field box.
#[gpui::test]
fn color_field_forwards_the_override_to_its_editable_input(cx: &mut TestAppContext) {
    for radius in [None, Some(px(OVERRIDE))] {
        let state = cx.new(|cx| InputState::new(cx));
        still();
        let vcx = open_host(cx, move || {
            let field =
                ColorField::new("cf-edit", PickerColor::from_hex("#00FF00")).state(state.clone());
            with(field, radius, ColorField::radius).into_any_element()
        });
        let scene = painted(vcx);
        let field = vcx.update(|_, cx| f32::from(field_radius(cx)));
        match radius {
            None => assert!(
                !scene.rounded(field).is_empty() && scene.rounded_exact(OVERRIDE).is_empty(),
                "the editable ColorField must paint the field chrome's {field}px"
            ),
            Some(_) => assert!(
                !scene.rounded_exact(OVERRIDE).is_empty(),
                "the editable ColorField must forward the override to its input box"
            ),
        }
    }
}

/// An `InputGroup` child is `rounded-none`: the group owns the one outline,
/// so a fill the caller gives the inner field shows square corners inside
/// the group's rounded shell rather than a second set of corners.
#[gpui::test]
fn grouped_input_uses_the_group_shell_radius_only(cx: &mut TestAppContext) {
    let state = cx.new(|cx| InputState::new(cx));
    let fill = gpui::rgb(0x336699).into();
    still();
    let vcx = open_host(cx, move || {
        InputGroup::new()
            .radius(px(5.))
            .input(Input::new(state.clone()).sx(move |s| s.bg(fill)))
            .into_any_element()
    });
    let scene = painted(vcx);
    let inner = scene.filled(fill);
    assert!(!inner.is_empty(), "the inner field's fill must paint");
    for quad in inner {
        assert!(
            scene.corners(quad).iter().all(|c| c.abs() < 0.05),
            "the grouped input must not round its own box, got {:?}",
            scene.corners(quad)
        );
    }
    assert!(
        !scene.rounded(5.).is_empty(),
        "the group shell must paint the group's radius"
    );
}

/// The dialog's panel is painted by its resting chain and, with motion on,
/// re-rounded every frame by the entry zoom; the settled zoom must land on
/// the resolved override, not on the helper.
#[gpui::test]
fn alert_dialog_entry_zoom_lands_on_the_resolved_radius(cx: &mut TestAppContext) {
    for radius in [None, Some(px(OVERRIDE))] {
        let vcx = open_host(cx, move || {
            gpui::div()
                .p(px(24.))
                .child(with(
                    AlertDialog::new("Delete?").id("ad-zoom").is_open(true),
                    radius,
                    AlertDialog::radius,
                ))
                .into_any_element()
        });
        wait_ms(vcx, 500);
        let scene = painted(vcx);
        let helper = vcx.update(|_, cx| f32::from(container_radius(cx)));
        match radius {
            None => assert!(
                !scene.rounded(helper).is_empty() && scene.rounded_exact(OVERRIDE).is_empty(),
                "the settled dialog must paint the container helper's {helper}px"
            ),
            Some(_) => assert!(
                !scene.rounded_exact(OVERRIDE).is_empty(),
                "the settled entry zoom must paint the override, not the helper"
            ),
        }
    }
}

/// Only the `Surface` variant paints a card, so the override is inert on the
/// flush default variant. (Its separators are 1px rules: any radius clamps
/// to half a pixel there, so which radius they read is not a visible fact.)
#[gpui::test]
fn accordion_override_is_inert_on_the_flush_default_variant(cx: &mut TestAppContext) {
    for variant in [AccordionVariant::Default, AccordionVariant::Surface] {
        still();
        let vcx = open_host(cx, move || {
            Accordion::new(vec![
                AccordionItem::new("a", "One"),
                AccordionItem::new("b", "Two"),
            ])
            .variant(variant)
            .radius(px(OVERRIDE))
            .into_any_element()
        });
        let scene = painted(vcx);
        let painted_override = !scene.rounded_exact(OVERRIDE).is_empty();
        assert_eq!(
            painted_override,
            variant == AccordionVariant::Surface,
            "{variant:?}: only the Surface card takes the override\n{}",
            scene.describe()
        );
    }
}

/// Holds the left button down at `at` and settles, without releasing.
fn hold(cx: &mut VisualTestContext, at: gpui::Point<Pixels>) {
    cx.simulate_mouse_move(at, None, Modifiers::none());
    settle(cx);
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    settle(cx);
}

/// The close button's box is its whole shape, so its pressed skin scales the
/// resolved radius — the override's, when one is set.
#[gpui::test]
fn close_button_press_scale_follows_the_override(cx: &mut TestAppContext) {
    for radius in [None, Some(px(10.))] {
        still();
        let vcx = open_host(cx, move || {
            gpui::div()
                .p(px(24.))
                .child(with(CloseButton::new("cb"), radius, CloseButton::radius))
                .into_any_element()
        });
        settle(vcx);
        let resting = vcx
            .debug_bounds("Name(\"cb\")")
            .expect("the close button must be laid out");
        hold(vcx, resting.center());
        let scene = painted(vcx);
        let value = match radius {
            Some(r) => f32::from(r),
            None => vcx.update(|_, cx| f32::from(small_radius(cx))),
        };
        let pressed = value * 0.93;
        assert!(
            !scene.rounded(pressed).is_empty(),
            "radius {radius:?}: the pressed skin must paint {value} x 0.93 = {pressed}"
        );
        vcx.simulate_mouse_up(resting.center(), MouseButton::Left, Modifiers::none());
    }
}

/// `is_round` is documented shape semantics: the override replaces the
/// helper's value on the square control only, and the round one stays a
/// circle.
#[gpui::test]
fn checkbox_override_never_overrides_the_round_circle(cx: &mut TestAppContext) {
    for round in [false, true] {
        still();
        let vcx = open_host(cx, move || {
            gpui::div()
                .p(px(24.))
                .child(Checkbox::new("cb").is_round(round).radius(px(OVERRIDE)))
                .into_any_element()
        });
        let scene = painted(vcx);
        let circles = scene
            .quads
            .iter()
            .filter(|q| {
                let b = scene.bounds(q);
                let side = f32::from(b.size.width);
                (side - f32::from(b.size.height)).abs() < 0.5
                    && side >= 12.
                    && scene.is_uniform(q, side / 2.)
            })
            .count();
        if round {
            assert!(
                scene.rounded_exact(OVERRIDE).is_empty() && circles > 0,
                "a round checkbox must stay a circle under an override"
            );
        } else {
            assert!(
                !scene.rounded_exact(OVERRIDE).is_empty(),
                "a square checkbox must take the override"
            );
        }
    }
}

/// A pressed radio control scales the resolved radius, so it does not snap
/// back to the helper; the selected dot inside is an inner part and keeps
/// `key_radius`.
#[gpui::test]
fn radio_control_press_follows_the_override_and_the_dot_does_not(cx: &mut TestAppContext) {
    const R: f32 = 3.;
    // Motion on: under reduced motion the press refinement is skipped.
    let vcx = open_host(cx, || {
        gpui::div()
            .p(px(24.))
            .child(
                RadioGroup::new("rg", vec![RadioOption::new("One")])
                    .default_value("One")
                    .radius(px(R)),
            )
            .into_any_element()
    });
    let resting = painted(vcx);
    let control = resting
        .rounded_exact(R)
        .first()
        .map(|q| resting.bounds(q))
        .expect("the selected control must paint the override");
    hold(vcx, control.center());
    wait_ms(vcx, 400);
    let scene = painted(vcx);
    assert!(
        !scene.rounded_exact(R * 0.95).is_empty(),
        "the pressed control must scale the override (x0.95), not the helper\n{}",
        scene.describe()
    );
    let key = vcx.update(|_, cx| f32::from(key_radius(cx)));
    let dot = scene
        .quads
        .iter()
        .filter(|q| contains(control, scene.bounds(q)))
        .filter(|q| f32::from(scene.bounds(q).size.width) <= 8.5)
        .collect::<Vec<_>>();
    assert!(
        !dot.is_empty(),
        "the selected dot must paint inside the control"
    );
    for q in dot {
        assert!(
            scene.is_uniform(q, key) && !scene.is_uniform(q, R),
            "the dot keeps key_radius ({key}px), not the control's override"
        );
    }
}

fn contains(outer: gpui::Bounds<Pixels>, inner: gpui::Bounds<Pixels>) -> bool {
    harness::contains(outer, inner)
}

/// The `Code` chip is the only kind that paints a box, so the override is
/// inert on every other kind — a heading's `radius` would otherwise lie.
#[gpui::test]
fn typography_override_sits_inside_the_code_branch(cx: &mut TestAppContext) {
    for code in [false, true] {
        still();
        let vcx = open_host(cx, move || {
            let t = if code {
                Typography::code("let x")
            } else {
                Typography::heading(2, "Title")
            };
            t.radius(px(OVERRIDE)).into_any_element()
        });
        let scene = painted(vcx);
        assert_eq!(
            !scene.rounded_exact(OVERRIDE).is_empty(),
            code,
            "code={code}: only the `Code` chip rounds"
        );
    }
}

fn badge(size: Size, radius: Option<Pixels>) -> AnyElement {
    gpui::div()
        .flex()
        .items_start()
        .p(px(40.))
        .child(
            BadgeAnchor::new()
                .child(gpui::div().w(px(64.)).h(px(64.)))
                .child(with(
                    Badge::new().size(size).child("3"),
                    radius,
                    Badge::radius,
                )),
        )
        .into_any_element()
}

/// Both size-stepped boxes keep their step as the fallback — each size paints
/// its own helper — and the override replaces the step's value; the tag's
/// remove button is an inner part that stays a circle under an override.
#[gpui::test]
fn badge_and_tag_steps_stay_the_fallback_and_inner_parts_keep_their_own(cx: &mut TestAppContext) {
    let steps: [(Size, Helper, Helper); 3] = [
        (Size::Sm, small_radius, small_radius),
        (Size::Md, control_radius, small_radius),
        (Size::Lg, soft_radius, soft_radius),
    ];
    for (size, badge_helper, tag_helper) in steps {
        for radius in [None, Some(px(OVERRIDE))] {
            still();
            let vcx = open_host(cx, move || badge(size, radius));
            let scene = painted(vcx);
            let step = vcx.update(|_, cx| f32::from(badge_helper(cx)));
            match radius {
                None => assert!(
                    !scene.rounded(step).is_empty() && scene.rounded_exact(OVERRIDE).is_empty(),
                    "Badge {size:?}: the step's {step}px must be the fallback"
                ),
                Some(_) => assert!(
                    !scene.rounded_exact(OVERRIDE).is_empty(),
                    "Badge {size:?}: the override must replace the step"
                ),
            }

            still();
            let vcx = open_host(cx, move || {
                gpui::div()
                    .p(px(24.))
                    .child(with(
                        TagGroup::new("tg", vec![Tag::new("a", "Alpha")]).size(size),
                        radius,
                        TagGroup::radius,
                    ))
                    .into_any_element()
            });
            let scene = painted(vcx);
            let step = vcx.update(|_, cx| f32::from(tag_helper(cx)));
            match radius {
                None => assert!(
                    !scene.rounded(step).is_empty() && scene.rounded_exact(OVERRIDE).is_empty(),
                    "Tag {size:?}: the step's {step}px must be the fallback"
                ),
                Some(_) => assert!(
                    !scene.rounded_exact(OVERRIDE).is_empty(),
                    "Tag {size:?}: the override must replace the step"
                ),
            }
        }
    }

    // The remove button's hover disc is `rounded-full` whatever the tag's
    // radius: hover it and the 12px disc paints as a circle.
    still();
    let vcx = open_host(cx, || {
        gpui::div()
            .p(px(24.))
            .child(
                TagGroup::new("tg-rm", vec![Tag::new("a", "Alpha")])
                    .radius(px(OVERRIDE))
                    .on_remove(|_, _, _| {}),
            )
            .into_any_element()
    });
    let resting = painted(vcx);
    let chip = resting
        .rounded_exact(OVERRIDE)
        .first()
        .map(|q| resting.bounds(q))
        .expect("the tag chip must paint the override");
    // The remove button trails the label inside the chip's end padding.
    let target = point(
        chip.origin.x + chip.size.width - px(14.),
        chip.origin.y + chip.size.height / 2.,
    );
    vcx.simulate_mouse_move(target, None, Modifiers::none());
    let scene = painted(vcx);
    let discs = scene
        .quads
        .iter()
        .filter(|q| {
            let b = scene.bounds(q);
            (f32::from(b.size.width) - 12.).abs() < 0.5
                && (f32::from(b.size.height) - 12.).abs() < 0.5
        })
        .collect::<Vec<_>>();
    assert!(
        !discs.is_empty(),
        "hovering the remove button must paint its disc"
    );
    for q in discs {
        assert!(
            scene.is_uniform(q, 6.),
            "the remove disc stays a circle under the tag's override"
        );
    }
}

/// The animated hover fill consumes the pixel corners an `sx` refinement
/// names — once it is mounted the skin underneath stays transparent, so the
/// fill is the only painted layer — and the pressed state keeps them: an
/// `sx` top-left corner survives resting, hover and press on all three
/// buttons.
#[gpui::test]
fn button_shapes_reconcile_sx_on_skin_fill_and_press(cx: &mut TestAppContext) {
    const TL: f32 = 7.;
    type Shape = (&'static str, fn() -> AnyElement);
    let shapes: [Shape; 3] = [
        ("Button", || {
            Button::new("b")
                .label("Save")
                .sx(|s| s.rounded_tl(px(TL)))
                .into_any_element()
        }),
        ("ToggleButton", || {
            ToggleButton::new("t")
                .label("Bold")
                .sx(|s| s.rounded_tl(px(TL)))
                .into_any_element()
        }),
        ("CloseButton", || {
            CloseButton::new("c")
                .sx(|s| s.rounded_tl(px(TL)))
                .into_any_element()
        }),
    ];
    for (name, build) in shapes {
        // Motion on: the hover fill only exists while a fade is mounted.
        let vcx = open_host(cx, move || {
            gpui::div().p(px(24.)).child(build()).into_any_element()
        });
        let resting = painted(vcx);
        let skin = resting
            .quads
            .iter()
            .find(|q| (resting.corners(q)[0] - TL).abs() < 0.05)
            .map_or_else(
                || panic!("{name}: the resting skin must take the sx corner"),
                |q| resting.bounds(q),
            );

        vcx.simulate_mouse_move(skin.center(), None, Modifiers::none());
        wait_ms(vcx, 300);
        let hovered = painted(vcx);
        let layers = hovered
            .quads
            .iter()
            .filter(|q| {
                let b = hovered.bounds(q);
                (f32::from(b.size.width) - f32::from(skin.size.width)).abs() < 0.5
                    && (f32::from(b.size.height) - f32::from(skin.size.height)).abs() < 0.5
                    && q.background.as_solid().is_some_and(|c| c.a > 0.)
            })
            .collect::<Vec<_>>();
        assert!(
            !layers.is_empty(),
            "{name}: hovering must paint the fade fill, got {}",
            layers.len()
        );
        for q in layers {
            assert!(
                (hovered.corners(q)[0] - TL).abs() < 0.05,
                "{name}: every hover layer must keep the sx corner, got {:?}",
                hovered.corners(q)
            );
        }

        vcx.simulate_mouse_down(skin.center(), MouseButton::Left, Modifiers::none());
        wait_ms(vcx, 400);
        let pressed = painted(vcx);
        // The visible pressed layer is the fade fill — the element
        // underneath stays transparent once the fill is mounted — so the
        // painted corner is the fill's own `sx` corner, the same shape
        // CloseButton's transform-only press has always painted.
        let pressed_fill = pressed
            .quads
            .iter()
            .rfind(|q| q.background.as_solid().is_some_and(|c| c.a > 0.))
            .expect("the pressed fade fill must be painting");
        assert!(
            (pressed.corners(pressed_fill)[0] - TL).abs() < 0.05,
            "{name}: the pressed fill must keep the sx corner {TL}, got {:?}",
            pressed.corners(pressed_fill)
        );
        vcx.simulate_mouse_up(skin.center(), MouseButton::Left, Modifiers::none());
        settle(vcx);
    }
}
