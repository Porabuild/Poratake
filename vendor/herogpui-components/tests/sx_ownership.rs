//! The part-scoped `sx` ownership inventory.
//!
//! An `sx` override is resolved per painted part and per property, not once
//! for a whole component: a tab-list background is not a selected-tab
//! background, and a slider track color is not its value fill. Each entry
//! below names the source function that owns one painted part or derived
//! metric and the extractor that part is ruled against — `util::sx_background`,
//! `util::sx_padding` or `util::sx_radius`.
//!
//! Every entry is decided; there are no pending conversions left:
//!
//! - **wired** — the part paints over the surface the root `sx` refines (or
//!   derives from it), so it must read the extractor or it would cover the
//!   override. The function owning the marker has to call the reader.
//! - **independent** — the root `sx` refines a wrapper the part does not
//!   paint over: the part is a control fill or field chrome of its own, or it
//!   rests transparent over the root so the override already shows through.
//!   Forwarding the wrapper's background into it is what principle 2 of
//!   `docs/customisation-roadmap.md` forbids, and each part's state colour
//!   has its own named seam instead. The function owning the marker must
//!   *not* call the reader, so a later forward is a deliberate decision that
//!   flips the entry rather than an accident.
//!
//! Either way the marker must still exist, so deleting or renaming a consumer
//! fails the test rather than silently shrinking the inventory. Scoping is to
//! the enclosing function, so a sibling's extractor call never satisfies an
//! entry (see the fixtures at the bottom).

use crate::source_scan;

use source_scan::{component_src, scope_contains};

enum Ruling {
    Wired,
    /// Why the root `sx` does not reach this part, and the seam that does.
    Independent(&'static str),
}

struct Part {
    file: &'static str,
    part: &'static str,
    marker: &'static str,
    reader: &'static str,
    ruling: Ruling,
}

impl Part {
    const fn wired(
        file: &'static str,
        part: &'static str,
        marker: &'static str,
        reader: &'static str,
    ) -> Self {
        Self {
            file,
            part,
            marker,
            reader,
            ruling: Ruling::Wired,
        }
    }

    const fn independent(
        file: &'static str,
        part: &'static str,
        marker: &'static str,
        reader: &'static str,
        reason: &'static str,
    ) -> Self {
        Self {
            file,
            part,
            marker,
            reader,
            ruling: Ruling::Independent(reason),
        }
    }
}

/// The root is a wrapper (the control row or field column); the part is the
/// control's own state fill, painted over nothing the root owns.
const OWN_CONTROL: &str = "the root sx refines the wrapper column/row; this part is the \
     control's own state fill, recoloured through its named hover seam";
/// The root is the field column; the part is the field chrome the `sx` docs
/// say the override does not reach.
const FIELD_CHROME: &str = "the root sx refines the column the field chrome sits in, not the \
     chrome (see the component's `sx` docs); the chrome's hover endpoint has its own seam";
/// The part rests transparent over the root surface.
const RESTS_TRANSPARENT: &str = "the part rests transparent over the root surface, so a root \
     sx background already shows through; the hover wash is a state endpoint with its own seam";
/// The part lives in a detached popover panel, not on the root.
const DETACHED_PANEL: &str = "the rows paint in the detached popover panel, which the root sx \
     never refines; the row hover has its own seam";

const INVENTORY: &[Part] = &[
    // Wired: Button freezes both hover-fade endpoints on the resolved resting
    // background, and the `sx` background is the resting value.
    Part::wired(
        "button.rs",
        "fill and hover-fade endpoints",
        "crate::anim::hover_fade_with_press(",
        "util::sx_background",
    ),
    Part::wired(
        "toggle_button.rs",
        "fill and hover-fade endpoints",
        "crate::anim::hover_fade_with_press(",
        "util::sx_background",
    ),
    Part::wired(
        "close_button.rs",
        "fill and hover-fade endpoints",
        "crate::anim::hover_fade(",
        "util::sx_background",
    ),
    // Wired: the thumb paints the bar's only fill, so the `sx` background is
    // its resting colour and the hover endpoint is resolved against it; the
    // track box reads the cross-axis `sx` size, and the thumb's corners read
    // the `sx` radii over the derived default.
    Part::wired(
        "scrollbar.rs",
        "thumb resting and hover fill endpoints",
        "crate::util::fade_endpoints(",
        "util::sx_background",
    ),
    Part::wired(
        "scrollbar.rs",
        "track box thickness",
        "let thickness = if horizontal {",
        "util::sx_pixel_size",
    ),
    Part::wired(
        "scrollbar.rs",
        "thumb corner radii",
        "crate::util::fill_unspecified_corners(",
        "util::sx_radius",
    ),
    // Wired: the unselected-tab hover wash fades from the resolved tray fill,
    // so a root `sx` background — refining the tray last — is part of the
    // resting value the overlay colour is derived from, the same resting
    // value `scrollbar.rs` resolves for its thumb.
    Part::wired(
        "tabs.rs",
        "unselected-tab hover wash base",
        "let tab_overlay = tray.alpha(",
        "util::sx_background",
    ),
    // Wired: a Surface accordion's root is the card, and a closed trigger's
    // fade paints its resting endpoint on the trigger itself; that endpoint
    // is the resolved card fill, so an `sx` card colour is not covered by the
    // stock surface under every header. The hover endpoint stays the state
    // wash (`Accordion::hover_bg` or `bg-default`).
    Part::wired(
        "accordion.rs",
        "surface trigger resting fade endpoint",
        "crate::anim::hover_fade_with_duration(",
        "util::sx_background",
    ),
    // Independent: each decided against forwarding; see the reason.
    Part::independent(
        "switch.rs",
        "track hover fill",
        "track_motion_frame.render(",
        "util::sx_background",
        OWN_CONTROL,
    ),
    Part::independent(
        "checkbox.rs",
        "control fill fade",
        "\"fill-fade\"",
        "util::sx_background",
        OWN_CONTROL,
    ),
    Part::independent(
        "select.rs",
        "trigger hover fill",
        // The trigger's hover endpoint moved into the fade's `hover_border`
        // argument when `anim` took ownership of the element's gpui hover
        // style; the call is the consumer now.
        "crate::anim::hover_fade_with_duration_and_easing_suppressed(",
        "util::sx_background",
        FIELD_CHROME,
    ),
    Part::independent(
        "list_box.rs",
        "row hover fill",
        ".hover(move |s| s.bg(hover_bg))",
        "util::sx_background",
        RESTS_TRANSPARENT,
    ),
    Part::independent(
        "pagination.rs",
        "control hover fill",
        "crate::anim::hover_fade(",
        "util::sx_background",
        RESTS_TRANSPARENT,
    ),
    Part::independent(
        "calendar.rs",
        "day hover fill",
        ".hover(move |s| s.bg(hover_bg))",
        "util::sx_background",
        RESTS_TRANSPARENT,
    ),
    Part::independent(
        "range_calendar.rs",
        "day hover fill",
        ".hover(move |s| s.bg(hover_bg))",
        "util::sx_background",
        RESTS_TRANSPARENT,
    ),
    Part::independent(
        "combo_box.rs",
        "row hover fill",
        ".hover(move |s| s.bg(hover_bg))",
        "util::sx_background",
        DETACHED_PANEL,
    ),
    Part::independent(
        "autocomplete.rs",
        "clear-button hover fill",
        ".hover(move |st| st.bg(hover_bg))",
        "util::sx_background",
        FIELD_CHROME,
    ),
    Part::independent(
        "dropdown.rs",
        "row hover fill",
        "row = row.hover(move |s| s.bg(row_hover_bg));",
        "util::sx_background",
        DETACHED_PANEL,
    ),
    Part::independent(
        "tag_group.rs",
        "tag hover fill",
        "hover_fade_with_duration_and_easing(",
        "util::sx_background",
        OWN_CONTROL,
    ),
    Part::independent(
        "time_field.rs",
        "stepper hover fill",
        ".hover(move |s| s.bg(hover_bg))",
        "util::sx_background",
        FIELD_CHROME,
    ),
    Part::independent(
        "input_otp.rs",
        "slot hover fill",
        "let hover_bg = self.slot_hover_bg.unwrap_or(match self.variant {",
        "util::sx_background",
        OWN_CONTROL,
    ),
    Part::independent(
        "input_group.rs",
        "group hover fill",
        // Same move as select.rs: the border endpoint is now the fade's
        // `hover_border` argument rather than a `.hover(..)` on the group.
        "crate::anim::hover_fade_with_duration_and_easing(",
        "util::sx_background",
        FIELD_CHROME,
    ),
    Part::independent(
        "number_field.rs",
        "group hover fill",
        "crate::anim::field_chrome_ramp(",
        "util::sx_background",
        FIELD_CHROME,
    ),
    Part::independent(
        "input.rs",
        "clear-button hover fill",
        ".hover(move |s| s.bg(clear_hover_bg))",
        "util::sx_background",
        RESTS_TRANSPARENT,
    ),
    Part::independent(
        "date_picker/range.rs",
        "trigger hover fill",
        ".hover(move |s| s.bg(hover_bg))",
        "util::sx_background",
        FIELD_CHROME,
    ),
];

/// The failure for one entry against `source`, or `None` when it holds.
fn check(part: &Part, source: &str) -> Option<String> {
    if !source.contains(part.marker) {
        return Some(format!(
            "{} [{}]: marker {:?} is gone — the consumer was removed or renamed; \
             update or delete this inventory entry",
            part.file, part.part, part.marker
        ));
    }
    match (
        &part.ruling,
        scope_contains(source, part.marker, part.reader),
    ) {
        (Ruling::Wired, Err(err)) => Some(format!("{} [{}]: {err}", part.file, part.part)),
        (Ruling::Independent(reason), Ok(())) => Some(format!(
            "{} [{}]: ruled independent ({reason}) but its function now reads {}; \
             flip the entry to `Part::wired` with the new contract, or drop the forward",
            part.file, part.part, part.reader
        )),
        _ => None,
    }
}

#[test]
fn every_inventoried_part_owns_its_extractor_or_is_ruled_independent() {
    let failures: Vec<String> = INVENTORY
        .iter()
        .filter_map(|part| check(part, &component_src(part.file)))
        .collect();
    assert!(
        failures.is_empty(),
        "sx ownership wiring failed:\n{}",
        failures.join("\n")
    );
}

/// Both rulings can fail: a wired part whose function dropped the reader, and
/// an independent part whose function started forwarding the root override.
#[test]
fn each_ruling_fails_on_the_wrong_scope() {
    let forwards = "fn owner() {\n    let bg = util::sx_background(&sx);\n    paint(bg);\n}\n";
    let plain = "fn owner() {\n    paint(stock);\n}\n";
    let wired = Part::wired("fixture.rs", "part", "paint(", "util::sx_background");
    let independent = Part::independent(
        "fixture.rs",
        "part",
        "paint(",
        "util::sx_background",
        "reason",
    );
    assert!(check(&wired, forwards).is_none());
    assert!(
        check(&wired, plain).is_some(),
        "a wired part must read the extractor"
    );
    assert!(check(&independent, plain).is_none());
    assert!(
        check(&independent, forwards).is_some(),
        "an independent part must not start forwarding the root override"
    );
}

/// A sibling's extractor call must not satisfy a consumer's entry.
#[test]
fn the_scanner_rejects_a_sibling_extractor_call() {
    let source = "\
fn owner() {
    paint();
}

fn sibling() {
    let bg = util::sx_background(&sx);
    paint_sibling(bg);
}
";
    assert!(
        scope_contains(source, "paint();", "util::sx_background").is_err(),
        "the sibling call must not satisfy the owner"
    );
    assert!(
        scope_contains(
            source,
            "let bg = util::sx_background",
            "util::sx_background"
        )
        .is_ok(),
        "the owning call must satisfy its own entry"
    );
}

/// A removed consumer must fail loudly instead of passing on an empty scope.
#[test]
fn the_scanner_requires_the_marker_to_exist() {
    let source = "fn owner() {\n    let bg = util::sx_background(&sx);\n    paint(bg);\n}\n";
    assert!(
        scope_contains(source, "removed_consumer", "util::sx_background").is_err(),
        "a missing marker is a failure, not an empty pass"
    );
}

/// The scope is the enclosing function, including a marker that is itself the
/// function signature.
#[test]
fn the_scanner_scopes_to_the_enclosing_function() {
    let good = "fn owner() {\n    let bg = util::sx_background(&sx);\n    paint(bg);\n}\n";
    assert!(scope_contains(good, "paint(bg)", "util::sx_background").is_ok());

    let signature = "fn owner() {\n    util::sx_background(&sx);\n}\n";
    assert!(scope_contains(signature, "fn owner()", "util::sx_background").is_ok());
}
