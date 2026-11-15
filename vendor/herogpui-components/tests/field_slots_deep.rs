//! Focused slot tests for `field.rs` — the composition parts HeroUI v3's
//! field components assemble: `Label`, `Description`, `ErrorMessage`,
//! `FieldError`, `Fieldset.Legend`, `Fieldset.Group` and `Fieldset.Actions`.
//!
//! The upstream contract is pinned v3.2.4 (`label.css`, `description.css`,
//! `error-message.css`, `field-error.css`, `fieldset.css` and the fieldset
//! component source):
//!
//! - `.field-error` is `h-0 px-1 … opacity-0 data-visible:h-auto
//!   data-visible:opacity-100`: a FieldError renders a line only when the
//!   field is invalid *and* a message is present; otherwise it collapses to
//!   zero height. `.error-message` and `.description` have no such gate —
//!   they render their 16px line whenever composed.
//! - `.label` associates with its field (`htmlFor`): clicking it focuses the
//!   field; the `status-disabled` label does not.
//! - `.label--required` adds the `*` mark (`after:ms-0.5 after:text-danger`).
//! - `.fieldset` is `flex flex-col gap-6 shrink grow basis-0`; its legend is
//!   a 24px line, `.fieldset__actions` is `gap-2 pt-1`, and
//!   `.fieldset__field-group` is `w-full`.
//!
//! Geometry is derived from the components' own constants, not guessed:
//! `Description`/`ErrorMessage`/`FieldError` are 16px lines (text-xs,
//! leading-4), `FieldsetLegend` is a 24px line, a bare `Button` is
//! `Size::Md::control_height` = 36px, `Label` is a 20px line (text-sm,
//! leading-5) and a bare `Input` is `util::FIELD_HEIGHT` = 36px.
//!
//! ```text
//! cargo test -p herogpui-components --test field_slots_deep
//! ```

use crate::harness;

use gpui::{prelude::*, px, Focusable, TestAppContext};
use herogpui_components::{
    Button, Description, ErrorMessage, FieldError, FieldGroup, Fieldset, FieldsetActions,
    FieldsetLegend, Input, InputState, Label,
};

use harness::{click, open_host};

/// A probe the geometry assertions can measure: a fixed-size div registered
/// under a stable debug selector.
fn probe(name: &'static str, w: f32, h: f32) -> gpui::AnyElement {
    gpui::div()
        .w(px(w))
        .h(px(h))
        .debug_selector(move || name.to_owned())
        .into_any_element()
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> gpui::Bounds<gpui::Pixels> {
    cx.debug_bounds(name).unwrap_or_else(|| {
        panic!(
            "the `{name}` probe must paint; a missing probe means the slot \
             above it never rendered"
        )
    })
}

// ---------------------------------------------------------------------------
// FieldError visibility gating
// ---------------------------------------------------------------------------

#[gpui::test]
fn field_error_renders_only_when_invalid_with_a_message(cx: &mut TestAppContext) {
    // Three FieldErrors in one column: the valid-with-message one occupies
    // the only 16px line; the invalid-without-message and the
    // message-without-invalid ones must collapse to zero height (upstream
    // `h-0 … opacity-0`), so the probe sits at exactly y 16. Any extra
    // rendered line would push it below that.
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .child(FieldError::new().message("Enter a valid email."))
            .child(FieldError::new().is_invalid(true))
            .child(FieldError::new().message("Hidden").is_invalid(false))
            .child(probe("fe-gate-probe", 20., 10.))
            .into_any_element()
    });

    let probe_bounds = bounds(cx, "fe-gate-probe");
    assert_eq!(
        probe_bounds.origin.y,
        px(16.),
        "exactly one FieldError line (the invalid one with a message) must \
         render; the invalid-without-message and message-without-invalid \
         variants must collapse to zero height"
    );
}

#[gpui::test]
fn a_long_description_wraps_inside_its_field(cx: &mut TestAppContext) {
    // `.description` is `text-wrap wrap-break-word`, and `checkbox.css` and
    // `radio.css` spell their copy of it `w-full min-w-0` on top of that: copy
    // longer than the field wraps onto another line rather than running out
    // through the field's edge. gpui sizes a text child to its content unless
    // its box is the parent's width, so a Description that did not take that
    // width overflowed in plain sight.
    //
    // The probe sits after a 200px-wide column holding one long description.
    // Wrapped, the description is more than one 16px line tall, so the probe is
    // pushed past 16; unwrapped it stays on one line and the probe sits at 16
    // with the copy spilling sideways.
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .w(px(200.))
            .child(Description::new(
                "One email every Monday morning with the week ahead, the week                  behind, and everything still open.",
            ))
            .child(probe("desc-wrap-probe", 20., 10.))
            .into_any_element()
    });

    let probe_bounds = bounds(cx, "desc-wrap-probe");
    assert!(
        probe_bounds.origin.y > px(16.),
        "a description longer than its 200px field must wrap onto further          16px lines; the probe sat at y {:?}, which is the single-line height          a description that overflows sideways leaves behind",
        probe_bounds.origin.y,
    );
}

#[gpui::test]
fn error_message_and_description_always_render_their_lines(cx: &mut TestAppContext) {
    // Unlike FieldError, `ErrorMessage` and `Description` carry no visibility
    // gate upstream: each is a 16px text-xs line whenever composed, in
    // composition order (order is part of anatomy).
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .child(ErrorMessage::new("Server rejected the address."))
            .child(Description::new("We never share your address."))
            .child(probe("em-order-probe", 20., 10.))
            .into_any_element()
    });

    let probe_bounds = bounds(cx, "em-order-probe");
    assert_eq!(
        probe_bounds.origin.y,
        px(32.),
        "ErrorMessage then Description must each draw their 16px line, in \
         the order they were composed"
    );
}

// ---------------------------------------------------------------------------
// Label state
// ---------------------------------------------------------------------------

#[gpui::test]
fn label_click_focuses_the_field_it_names(cx: &mut TestAppContext) {
    // `htmlFor` in this port is the field's focus handle: clicking the label
    // must focus the field, and typing must then reach it.
    let state = cx.new(|cx| InputState::new(cx));
    let handle = cx.update(|cx| state.read(cx).focus_handle(cx));
    let handle_for_label = handle.clone();
    let state_for_view = state.clone();
    let cx = open_host(cx, move || {
        let state = state_for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .child(Label::new("Email").label_for("email-label", handle_for_label.clone()))
            .child(Input::new(state))
            .into_any_element()
    });

    // Label 0..20, input 20..56: the click lands on the label, not the field.
    click(cx, 10., 10.);
    let focused = cx.update(|window, cx| window.focused(cx).is_some_and(|h| h == handle));
    assert!(focused, "clicking the label must focus the field it names");

    cx.simulate_input("ada");
    let value = cx.update(|_, cx| state.read(cx).value().to_owned());
    assert_eq!(
        value, "ada",
        "typing after the label click must reach the focused field"
    );
}

#[gpui::test]
fn disabled_label_click_never_focuses_the_field_it_names(cx: &mut TestAppContext) {
    // A `status-disabled` label renders no click target (the port gates the
    // whole `label_for` behaviour off), matching upstream's
    // `pointer-events: none`: the click falls through to dead space, which
    // blurs the field in a browser and in gpui alike. The label must
    // certainly never focus the field it names.
    let state = cx.new(|cx| InputState::new(cx));
    let handle = cx.update(|cx| state.read(cx).focus_handle(cx));
    let handle_for_label = handle.clone();
    let state_for_view = state;
    let cx = open_host(cx, move || {
        let state = state_for_view.clone();
        gpui::div()
            .flex()
            .flex_col()
            .child(Input::new(state))
            .child(
                Label::new("Email")
                    .is_disabled(true)
                    .label_for("disabled-label", handle_for_label.clone()),
            )
            .into_any_element()
    });

    // Input 0..36, disabled label 36..56.
    click(cx, 10., 18.);
    click(cx, 10., 46.);
    let focused = cx.update(|window, cx| window.focused(cx).is_some_and(|h| h == handle));
    assert!(
        !focused,
        "clicking a disabled label must not focus the field it names"
    );
}

#[gpui::test]
fn required_label_draws_the_asterisk_mark(cx: &mut TestAppContext) {
    // `.label--required` adds the `*` after the text (`after:ms-0.5`), so the
    // required row's trailing probe must sit further right than the plain
    // row's — the mark is a real 2px-gap + glyph, not a no-op.
    let cx = open_host(cx, || {
        gpui::div()
            .flex()
            .flex_col()
            .child(
                gpui::div()
                    .flex()
                    .flex_row()
                    .child(Label::new("Email"))
                    .child(probe("plain-label-probe", 20., 10.)),
            )
            .child(
                gpui::div()
                    .flex()
                    .flex_row()
                    .child(Label::new("Email").is_required(true))
                    .child(probe("required-label-probe", 20., 10.)),
            )
            .into_any_element()
    });

    let plain = bounds(cx, "plain-label-probe");
    let required = bounds(cx, "required-label-probe");
    assert!(
        required.origin.x > plain.origin.x,
        "the required label must be wider by the asterisk mark: plain ends \
         at x {:?}, required at x {:?}",
        plain.origin.x,
        required.origin.x
    );
}

// ---------------------------------------------------------------------------
// Fieldset structure: order, spacing, width
// ---------------------------------------------------------------------------

#[gpui::test]
fn fieldset_slots_compose_in_order_with_pinned_spacing(cx: &mut TestAppContext) {
    // The docs' anatomy — Legend, Description, Group, Actions — measured
    // through wrapper probes so each slot's band is visible without altering
    // the Fieldset's own gap. Pinned v3 numbers: gap-6 (24) between
    // children, a 24px legend line, a 16px description, and `pt-1` (4) above
    // the actions row, whose `gap-2` (8) separates its children.
    let cx = open_host(cx, || {
        Fieldset::new()
            .child(
                gpui::div()
                    .debug_selector(move || "fs-legend".to_owned())
                    .child(FieldsetLegend::new("Profile")),
            )
            .child(
                gpui::div()
                    .debug_selector(move || "fs-description".to_owned())
                    .child(Description::new("Update your profile.")),
            )
            .child(
                gpui::div()
                    .debug_selector(move || "fs-group".to_owned())
                    .child(FieldGroup::new().child(probe("fs-group-field", 20., 20.))),
            )
            .child(
                gpui::div()
                    .debug_selector(move || "fs-actions".to_owned())
                    .child(
                        FieldsetActions::new()
                            .child(
                                gpui::div()
                                    .debug_selector(move || "fs-action-btn".to_owned())
                                    .child(Button::new("fs-btn").label("Save")),
                            )
                            .child(probe("fs-action-probe", 20., 10.)),
                    ),
            )
            .into_any_element()
    });

    let legend = bounds(cx, "fs-legend");
    assert_eq!(legend.origin.y, px(0.), "the legend leads the composition");
    assert_eq!(
        legend.size.height,
        px(24.),
        "the legend is a 24px text-base line"
    );

    let description = bounds(cx, "fs-description");
    assert_eq!(
        description.origin.y,
        px(48.),
        "the description follows the legend across the 24px gap"
    );
    assert_eq!(
        description.size.height,
        px(16.),
        "the description is a 16px text-xs line"
    );

    let group = bounds(cx, "fs-group");
    assert_eq!(
        group.origin.y,
        px(88.),
        "the group follows the description across the 24px gap"
    );
    assert_eq!(group.size.height, px(20.), "the group hugs its field");

    let actions = bounds(cx, "fs-actions");
    assert_eq!(
        actions.origin.y,
        px(132.),
        "the actions row follows the group across the 24px gap"
    );

    let button = bounds(cx, "fs-action-btn");
    assert_eq!(
        button.origin.y,
        actions.origin.y + px(4.),
        "`.fieldset__actions` is `pt-1`: its content starts 4px below the row"
    );
    assert_eq!(
        button.size.height,
        px(36.),
        "the md button keeps its 36px control height inside the actions row"
    );

    let trailing = bounds(cx, "fs-action-probe");
    assert_eq!(
        trailing.origin.x,
        button.origin.x + button.size.width + px(8.),
        "`.fieldset__actions` is `gap-2`: 8px between its children"
    );
}

#[gpui::test]
fn fieldset_fills_a_constrained_flex_row_after_a_fixed_sibling(cx: &mut TestAppContext) {
    // `.fieldset` carries `shrink grow basis-0`: next to a fixed 100px
    // sibling in a 500px row it must take the remaining 400px, and its
    // `w-full` FieldGroup must hand that width down. A content-hugging
    // fieldset would leave the inner probe far short of 400px.
    let cx = open_host(cx, || {
        gpui::div()
            .w(px(500.))
            .flex()
            .flex_row()
            .child(probe("fs-fixed-sibling", 100., 20.))
            .child(
                Fieldset::new().child(
                    FieldGroup::new().child(
                        gpui::div()
                            .w_full()
                            .h(px(20.))
                            .debug_selector(move || "fs-inner-probe".to_owned()),
                    ),
                ),
            )
            .into_any_element()
    });

    let inner = bounds(cx, "fs-inner-probe");
    assert_eq!(
        inner.origin.x,
        px(100.),
        "the fieldset must start right after its fixed sibling"
    );
    assert_eq!(
        inner.size.width,
        px(400.),
        "`shrink grow basis-0` must stretch the fieldset across the \
         remaining 400px, and `w-full` FieldGroup must pass it through"
    );
}

#[gpui::test]
fn fieldset_gap_builder_overrides_the_24_default(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        Fieldset::new()
            .gap(px(12.))
            .child(probe("fs-gap-first", 20., 20.))
            .child(probe("fs-gap-second", 20., 20.))
            .into_any_element()
    });

    let second = bounds(cx, "fs-gap-second");
    assert_eq!(
        second.origin.y,
        px(32.),
        "a custom gap must replace the pinned 24px, not stack with it"
    );
}

#[gpui::test]
fn field_group_gap_builder_overrides_the_16_default(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        FieldGroup::new()
            .gap(px(10.))
            .child(probe("fg-gap-first", 20., 20.))
            .child(probe("fg-gap-second", 20., 20.))
            .into_any_element()
    });

    let second = bounds(cx, "fg-gap-second");
    assert_eq!(
        second.origin.y,
        px(30.),
        "a custom FieldGroup gap must replace the pinned 16px space-y-4"
    );
}

#[gpui::test]
fn fieldset_actions_gap_builder_overrides_the_8_default(cx: &mut TestAppContext) {
    let cx = open_host(cx, || {
        FieldsetActions::new()
            .gap(px(24.))
            .child(probe("fa-gap-first", 20., 10.))
            .child(probe("fa-gap-second", 20., 10.))
            .into_any_element()
    });

    let second = bounds(cx, "fa-gap-second");
    assert_eq!(
        second.origin.x,
        px(44.),
        "a custom actions gap must replace the pinned 8px gap-2"
    );
}
