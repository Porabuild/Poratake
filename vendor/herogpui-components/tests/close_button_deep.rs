//! CloseButton geometry and press against pinned HeroUI v3.2.4
//! `packages/styles/components/close-button.css`.
//!
//! The stylesheet's interactive contract is:
//! - `.close-button--default:active, &[data-pressed="true"]` is
//!   `transform: scale(0.93)` about `origin-center`, not an opacity dim.
//! - `.close-button svg` is `size-4 shrink-0 self-center -mx-0.5 my-0.5`.
//! - hover is `bg-default-hover` only; focus-visible is `status-focused`;
//!   disabled is `status-disabled` (dim, no pointer, no press).
//!
//! gpui has no div-level transform, so the press is the same centred inset
//! `anim::pressed` uses: the 24px box shrinks to `24 * 0.93` and the leftover
//! margin keeps neighbours from reflowing. The shrink ramps over the
//! stylesheet's `transform 250ms var(--ease-out-quart)`; reduced motion keeps
//! the endpoint geometry but arrives on the first frame. Pointer / keyboard
//! activation and render-prop state stay in `buttons.rs` and
//! `value_props.rs`.

use crate::harness;
use crate::source_scan;

use gpui::{
    point, prelude::*, px, Bounds, ElementId, Modifiers, MouseButton, Pixels, TestAppContext,
    VisualTestContext,
};
use harness::{events, open_host, press};
use herogpui_components::CloseButton;

/// Pinned `.close-button--default:active { transform: scale(0.93) }`.
const PRESS_SCALE: f32 = 0.93;
/// `.close-button` is `h-6 w-6`.
const BOX: f32 = 24.;
/// `.close-button svg` is `size-4`.
const ICON: f32 = 16.;
/// Tailwind `0.5` at a 16px root: `-mx-0.5` / `my-0.5`.
const SVG_MARGIN: f32 = 2.;

fn flush_frame(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
}

/// How many animation-frame callbacks are pending: one per live animation.
/// The press ramp registers one while it runs and stops when it settles, the
/// way `checkbox_motion.rs` observes motion.
fn pending_frames(cx: &mut VisualTestContext) -> usize {
    cx.update(|window, cx| window.simulate_next_frame(cx))
}

/// Drains frames until every in-flight animation has settled, failing if one
/// never does (a re-arming animation would keep registering callbacks).
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..100 {
        if pending_frames(cx) == 0 {
            return;
        }
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(10));
    }
    panic!("close button animations never settled");
}

fn probe(name: String) -> &'static str {
    Box::leak(name.into_boxed_str())
}

fn root_probe(id: &'static str) -> &'static str {
    probe(format!("{:?}", ElementId::from(id)))
}

fn icon_probe(id: &'static str) -> &'static str {
    probe(format!("{:?}-icon", ElementId::from(id)))
}

fn near(a: impl Into<Pixels>, b: f32) -> bool {
    (f32::from(a.into()) - b).abs() < 0.5
}

fn bounds(cx: &mut VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} must paint"))
}

fn centre(b: Bounds<Pixels>) -> gpui::Point<Pixels> {
    point(
        b.origin.x + b.size.width / 2.,
        b.origin.y + b.size.height / 2.,
    )
}

// ---------------------------------------------------------------------------
// Pinned source
// ---------------------------------------------------------------------------

/// Remaining source-text check. `.close-button svg` is `-mx-0.5 my-0.5` (2px
/// at the 16px root): symmetric margins on a centred 16px child cancel, and
/// the test platform draws no svg, so the margins leave no trace in layout
/// or paint. (The press scale, its ramp and the disabled path are measured
/// in the press-geometry tests below.)
#[test]
fn default_svg_uses_the_pinned_margins() {
    let code: String = source_scan::component_src("close_button.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("the implementation section is always present")
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        code.contains("mx(px(-2.))") && code.contains("my(px(2.))"),
        "the default CloseButton glyph must take the pinned -mx-0.5 my-0.5 \
         margins, representable as px(-2)/px(2)"
    );
}

/// The press recolours nothing and dims nothing: a held close button paints
/// the same fills at full opacity (the previous port dimmed to 70%).
#[gpui::test]
fn press_scales_without_dimming(cx: &mut TestAppContext) {
    harness::still();
    let cx = open_host(cx, || {
        gpui::div()
            .p(px(24.))
            .child(CloseButton::new("cb-dim"))
            .into_any_element()
    });
    let resting = harness::painted(cx).solids();
    let at = point(px(36.), px(36.));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    let pressed = harness::painted(cx);
    assert!(
        !pressed.quads.is_empty()
            && pressed
                .quads
                .iter()
                .filter_map(|q| q.background.as_solid())
                .all(|c| c.a < 1e-3 || (c.a - 1.).abs() < 1e-3),
        "a pressed close button must not dim, painted {:?} (resting {resting:?})",
        pressed.solids()
    );
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
}

// ---------------------------------------------------------------------------
// Press geometry
// ---------------------------------------------------------------------------

/// A pointer press must ramp the 24px box down to `24 * 0.93` about its
/// centre — starting from the painted frame, animating over the pinned 250ms,
/// settling at the pinned endpoint — and spring back the same way, while the
/// completed click still reports `on_press`.
#[gpui::test]
fn press_scales_the_box_to_0_93(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        CloseButton::new("cb")
            .on_press(move |_, _, _| recorded.borrow_mut().push("press".into()))
            .into_any_element()
    });

    let root = root_probe("cb");
    flush_frame(cx);
    let at_rest = bounds(cx, root);
    assert!(
        near(at_rest.size.width, BOX) && near(at_rest.size.height, BOX),
        "a resting CloseButton is a 24px square, got {at_rest:?}"
    );

    let at = centre(at_rest);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    flush_frame(cx);
    // The hover fade eases its own fill here, so pending frames are expected;
    // what hover must never do is move the box.
    let hovered = bounds(cx, root);
    assert!(
        near(hovered.size.width, BOX) && near(hovered.size.height, BOX),
        "hover must not change the box, got {hovered:?}"
    );

    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    let starting = bounds(cx, root);
    assert!(
        near(starting.size.width, BOX),
        "the ramp starts from the painted frame, not the endpoint, got {starting:?}"
    );
    assert!(
        pending_frames(cx) > 0,
        "a press must start the transform ramp, not snap"
    );

    settle(cx);
    let pressed = bounds(cx, root);
    let scaled = BOX * PRESS_SCALE;
    assert!(
        near(pressed.size.width, scaled) && near(pressed.size.height, scaled),
        "a pressed CloseButton must scale to 0.93 ({scaled}px), got {pressed:?}"
    );
    let inset = (BOX - scaled) / 2.;
    assert!(
        near(pressed.origin.x, inset) && near(pressed.origin.y, inset),
        "the scale is about the centre, so the leftover is margin, got {pressed:?}"
    );

    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(
        pending_frames(cx) > 0,
        "the release must ramp back rather than snap"
    );
    settle(cx);
    let released = bounds(cx, root);
    assert!(
        near(released.size.width, BOX) && near(released.size.height, BOX),
        "the box must spring back after the release, got {released:?}"
    );
    assert_eq!(
        presses.borrow().as_slice(),
        ["press"],
        "the completed click must still report on_press"
    );
}

/// `motion-reduce:transition-none` still applies the CSS transform, so the
/// pressed box must be at its final geometry on the first reduced-motion frame.
/// Activation must survive that instant state change.
#[gpui::test]
fn reduced_motion_keeps_the_press_scale_instantly(cx: &mut TestAppContext) {
    harness::still();
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        CloseButton::new("cb-still")
            .on_press(move |_, _, _| recorded.borrow_mut().push("press".into()))
            .into_any_element()
    });

    let root = root_probe("cb-still");
    flush_frame(cx);
    let at = centre(bounds(cx, root));
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    let pressed = bounds(cx, root);
    let scaled = BOX * PRESS_SCALE;
    assert!(
        near(pressed.size.width, scaled) && near(pressed.size.height, scaled),
        "reduced motion must apply the press scale immediately ({scaled}px), got {pressed:?}"
    );
    let inset = (BOX - scaled) / 2.;
    assert!(
        near(pressed.origin.x, inset) && near(pressed.origin.y, inset),
        "reduced-motion press must stay centered, got {pressed:?}"
    );
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert_eq!(
        presses.borrow().as_slice(),
        ["press"],
        "reduced motion must not swallow the press"
    );
}

/// A disabled CloseButton is `status-disabled`: no hover fill, no press
/// scale, no activation.
#[gpui::test]
fn disabled_press_does_not_scale_or_fire(cx: &mut TestAppContext) {
    let presses = events();
    let recorded = presses.clone();
    let cx = open_host(cx, move || {
        let recorded = recorded.clone();
        CloseButton::new("cb-off")
            .is_disabled(true)
            .on_press(move |_, _, _| recorded.borrow_mut().push("press".into()))
            .into_any_element()
    });

    let root = root_probe("cb-off");
    flush_frame(cx);
    let at = centre(bounds(cx, root));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    let pressed = bounds(cx, root);
    assert!(
        near(pressed.size.width, BOX) && near(pressed.size.height, BOX),
        "a disabled CloseButton must not scale, got {pressed:?}"
    );
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    flush_frame(cx);
    assert!(
        presses.borrow().is_empty(),
        "a disabled CloseButton must not report on_press"
    );
}

/// Keyboard focus draws `status-focused` as a ring; it must not change the
/// 24px box. Tab still lands on the enabled button.
#[gpui::test]
fn focus_visible_keeps_the_24px_box(cx: &mut TestAppContext) {
    let cx = open_host(cx, || CloseButton::new("cb-focus").into_any_element());
    let root = root_probe("cb-focus");
    flush_frame(cx);
    press(cx, "tab");
    flush_frame(cx);
    let focused = bounds(cx, root);
    assert!(
        near(focused.size.width, BOX) && near(focused.size.height, BOX),
        "focus-visible must not change the box, got {focused:?}"
    );
}

// ---------------------------------------------------------------------------
// Default glyph placement
// ---------------------------------------------------------------------------

/// `.close-button` is `h-6 w-6 p-1` around a `size-4` glyph. Symmetric
/// `-mx-0.5 my-0.5` on a centred 16px child in a 16px content box cancel,
/// so the glyph still fills the padding box at (4, 4). The margins are
/// still applied (source scan above); this pins the observable placement.
#[gpui::test]
fn default_glyph_fills_the_padded_content_box(cx: &mut TestAppContext) {
    let cx = open_host(cx, || CloseButton::new("cb-icon").into_any_element());
    let icon = icon_probe("cb-icon");
    flush_frame(cx);
    let glyph = bounds(cx, icon);
    assert!(
        near(glyph.size.width, ICON) && near(glyph.size.height, ICON),
        "the default glyph is size-4 (16px), got {glyph:?}"
    );
    assert!(
        near(glyph.origin.x, 4.) && near(glyph.origin.y, 4.),
        "items-center + justify-center keep a 16px glyph at the p-1 inset, \
         got {glyph:?}"
    );
    let _ = SVG_MARGIN;
}

/// `hover_bg` and the `sx` background feed the shared endpoint resolver: the
/// fade rests on the sx background (or `--default`) and eases to the named
/// hover colour. Resolver cases live with `util::fade_endpoints`; this reads
/// both endpoints off the painted scene — hovered, and after the pointer
/// leaves, when the settled fade layer paints the resting endpoint.
#[gpui::test]
fn close_button_reads_the_hover_override_and_the_sx_background(cx: &mut TestAppContext) {
    let (rest, over): (gpui::Hsla, gpui::Hsla) =
        (gpui::rgb(0x224466).into(), gpui::rgb(0x88aa22).into());
    let cx = open_host(cx, move || {
        gpui::div()
            .p(px(24.))
            .child(
                CloseButton::new("cb-hover")
                    .sx(move |s| s.bg(rest))
                    .hover_bg(over),
            )
            .into_any_element()
    });
    let resting = harness::painted(cx).solids();
    assert!(
        harness::has_color(&resting, rest) && !harness::has_color(&resting, over),
        "the resting endpoint must be the sx background, painted {resting:?}"
    );
    cx.simulate_mouse_move(point(px(36.), px(36.)), None, Modifiers::none());
    harness::wait_ms(cx, 300);
    let hovered = harness::painted(cx).solids();
    assert!(
        harness::has_color(&hovered, over),
        "the hover endpoint must be the named override, painted {hovered:?}"
    );
    cx.simulate_mouse_move(point(px(600.), px(600.)), None, Modifiers::none());
    harness::wait_ms(cx, 300);
    let left = harness::painted(cx).solids();
    let stock = cx.update(|_, cx| {
        use herogpui_theme::ActiveTheme;
        cx.colors().default.color
    });
    assert!(
        harness::has_color(&left, rest) && !harness::has_color(&left, stock),
        "leaving must ease back to the sx background, not `--default`, \
         painted {left:?}"
    );
}
