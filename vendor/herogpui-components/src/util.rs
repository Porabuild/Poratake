//! Shared render helpers for HeroGPUI components.

use gpui::{App, BorrowAppContext, Div, Hsla, ParentElement, Pixels, Refineable, Styled};
use herogpui_core::{element_id, FieldVariant};
use herogpui_theme::ActiveTheme;

// Browser hosts register the bundled mono family before opening a window.
pub(crate) const MONO_FONT: &str = if cfg!(target_family = "wasm") {
    "JetBrains Mono"
} else {
    "Consolas"
};

/// The one height every v3 form field has: `.date-input-group` and
/// `.color-input-group` are `h-9`, and `.input`'s `py-2` plus its line box comes
/// to the same 36px. v3 removed `size` from the field
/// components (Input, Select, ComboBox, DateField, ...), keeping it only on the
/// nineteen where a scale is documented, so a field's metrics are constants
/// rather than a [`herogpui_core::Size`] lookup.
pub const FIELD_HEIGHT: Pixels = gpui::px(36.);
/// Type size inside a form field.
pub const FIELD_TEXT: Pixels = gpui::px(14.);
/// Glyph size for an icon inside a form field.
pub const FIELD_ICON: Pixels = gpui::px(16.);

/// HeroUI's numeric outputs use `tabular-nums` so changing a value does not
/// move the label row. GPUI exposes the equivalent OpenType feature directly;
/// keep the construction here so every numeric component uses the same
/// feature tag and no component falls back to proportional figures.
pub(crate) fn tabular_font_features() -> gpui::FontFeatures {
    gpui::FontFeatures(std::sync::Arc::new(vec![("tnum".to_owned(), 1)]))
}

/// Optional box geometry and chrome overrides shared by the field family.
///
/// `Input` keeps its own copies — its grouped padding rules predate this
/// struct — and `DateField` keeps its embedded bare flags; the rest of the
/// family stores one of these and resolves the values in render. All defaults
/// reproduce the stock field box exactly.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FieldBox {
    pub(crate) height: Option<Pixels>,
    pub(crate) padding_x: Option<Pixels>,
    /// Vertical padding, in place of the family's unpadded `min-h` box.
    ///
    /// Only meaningful where the box's content can grow past one line — v3's
    /// select trigger carries `py-2` for exactly that reason, and the rest of
    /// the family holds a single-line editor whose height `height` already
    /// names. `None` reproduces the stock unpadded box everywhere.
    pub(crate) padding_y: Option<Pixels>,
    pub(crate) is_bare: bool,
    pub(crate) is_bare_is_set: bool,
    /// Whether the focused field should paint its focus ring. `None` keeps the
    /// stock treatment; `Some(false)` hides only that visual indicator while
    /// retaining focus, editing and validation feedback.
    pub(crate) focus_ring: Option<bool>,
}

impl FieldBox {
    /// The explicit single-line height, or the stock [`FIELD_HEIGHT`].
    pub(crate) fn resolved_height(&self) -> Pixels {
        self.height.unwrap_or(FIELD_HEIGHT)
    }

    /// The explicit horizontal padding, or v3's `px-3`.
    pub(crate) fn resolved_padding_x(&self) -> Pixels {
        self.padding_x.unwrap_or(gpui::px(12.))
    }
}

// v3 does not have one "control" radius: each component names its own step, and
// they span the whole scale. `design_audit.py` diffs these against the real
// stylesheets, so the mapping here is checked rather than asserted.

/// Applies the theme's interactive cursor (`--cursor-interactive`) to `el`.
///
/// The token replacement for GPUI's `cursor_pointer()`: v3 puts
/// `cursor: pointer` on every clickable control, and a theme overrides all of
/// them at once through [`LayoutTheme::cursor_interactive`].
///
/// Sites that need the value inside a closure GPUI does not hand a `cx` — a
/// `when` or `hover` body — read [`interactive_cursor`] first and call
/// `Styled::cursor` with the copy.
///
/// [`LayoutTheme::cursor_interactive`]: herogpui_theme::LayoutTheme::cursor_interactive
pub fn cursor_interactive<T: Styled>(el: T, cx: &App) -> T {
    el.cursor(interactive_cursor(cx))
}

/// The theme's interactive cursor, for a site that must capture it by value.
pub fn interactive_cursor(cx: &App) -> gpui::CursorStyle {
    cx.layout().cursor_interactive
}

/// `rounded-3xl` — buttons, toggle buttons and avatars.
pub fn control_radius(cx: &App) -> Pixels {
    let layout = cx.layout();
    layout.capped(layout.radius_3xl())
}

/// `rounded-2xl` — chips, menu and list rows, the colour area.
pub fn soft_radius(cx: &App) -> Pixels {
    let layout = cx.layout();
    layout.capped(layout.radius_2xl())
}

/// `rounded-xl` — close buttons, tags, links, tooltips.
pub fn small_radius(cx: &App) -> Pixels {
    let layout = cx.layout();
    layout.capped(layout.radius_xl())
}

/// `rounded-md` — the checkbox control (`.checkbox__control` is
/// `size-4 rounded-md`).
pub fn mark_radius(cx: &App) -> Pixels {
    let layout = cx.layout();
    layout.capped(layout.radius_md())
}

/// `rounded-lg` — the keyboard key and the radio control, which v3 draws as a
/// rounded square rather than a circle.
pub fn key_radius(cx: &App) -> Pixels {
    let layout = cx.layout();
    layout.capped(layout.radius_lg())
}

/// `rounded-sm` — separators and skeletons, which are nearly square.
pub fn hairline_radius(cx: &App) -> Pixels {
    let layout = cx.layout();
    layout.capped(layout.radius_sm())
}

/// `rounded-xs` — the small ProgressBar track.
pub fn micro_radius(cx: &App) -> Pixels {
    let layout = cx.layout();
    layout.capped(layout.radius_xs())
}

/// Corner radius of a form field — `--field-radius`.
pub fn field_radius(cx: &App) -> Pixels {
    let layout = cx.layout();
    layout.capped(layout.field_radius)
}

/// `min(32px, --radius-3xl)` — cards, the table, and every floating panel
/// (modal, popover, toast, alert, dropdown). Surface is not on the list:
/// upstream `.surface` declares no radius.
pub fn container_radius(cx: &App) -> Pixels {
    let layout = cx.layout();
    layout.capped(layout.radius_3xl())
}

/// HeroUI v3.2.7 list popovers nest the row's 2xl radius plus its 6px inset.
pub fn picker_radius(cx: &App) -> Pixels {
    (cx.layout().radius_2xl() + gpui::px(6.)).min(gpui::px(32.))
}

/// Corner radius for a fill painted *inside* a rounded `overflow_hidden` clip
/// parent on vanilla GPUI.
///
/// Upstream clips `overflow_hidden()` to the parent's rectangle even when the
/// parent is rounded, so a square child fill or image squares off the
/// parent's corners (the retired renderer fork in
/// `docs/upstream/retired-patches/` fixed this for every component at once).
/// Until that lands upstream, each clipped fill carries the same radius as
/// its clip parent: the fill's own rounded corners hide the square bleed.
/// Subtract the border width when the parent draws one, the way a CSS
/// `border-radius` shrinks inward; clamps at zero so a hairline radius never
/// goes negative.
///
/// Where a whole stack of fills shares one clip (the color area, the color
/// slider track, checkerboards), each layer repeats the radius: GPUI's `Svg`
/// element is not an alternative here, it renders through an alpha mask
/// (`Window::paint_svg` → `render_alpha_mask`), so a multicolor gradient SVG
/// can only ever paint a monochrome silhouette.
pub fn inner_fill_radius(outer: Pixels, border: Pixels) -> Pixels {
    (outer - border).max(gpui::px(0.))
}

/// Shift+wheel horizontal scroll for mouse wheels on the web platform.
///
/// A mouse wheel reports only `deltaY`; every desktop platform reads
/// shift+wheel as the horizontal axis, and GPUI's native platforms deliver it
/// that way. The web platform forwards the raw axes instead, so a
/// horizontally scrollable region — the `Tabs` strip, a wide `Table` — cannot
/// be wheel-scrolled there at all, with no fallback. Until the platform fix
/// lands upstream (see `docs/upstream/retired-patches/`), horizontal
/// scrollers call this from `on_scroll`: when shift is held and the
/// event carries no horizontal component, the vertical delta drives the
/// handle's x offset instead, matching the platform sign convention (offsets
/// go negative as content scrolls down/right).
///
/// Web-only by construction (`cfg(target_family = "wasm")`): native platforms
/// already translate, so their events keep flowing through `track_scroll`
/// untouched. A `Lines` delta (Firefox mouse wheels) converts at 16px/line.
pub fn shift_wheel_scroll_x(
    handle: &gpui::ScrollHandle,
    event: &gpui::ScrollEvent,
    window: &mut gpui::Window,
) {
    #[cfg(target_family = "wasm")]
    {
        if !event.modifiers.shift {
            return;
        }
        let vertical = match event.delta {
            gpui::ScrollDelta::Pixels(delta) => {
                if delta.x != gpui::px(0.) {
                    return;
                }
                delta.y
            }
            gpui::ScrollDelta::Lines(delta) => {
                if delta.x != 0.0 {
                    return;
                }
                gpui::px(delta.y * 16.0)
            }
        };
        if vertical == gpui::px(0.) {
            return;
        }
        let at = handle.offset();
        handle.set_offset(gpui::point(at.x + vertical, at.y));
        window.refresh();
    }
    #[cfg(not(target_family = "wasm"))]
    {
        let _ = (handle, event, window);
    }
}

/// Applies the v3 field chrome: background, radius, border and — for
/// `primary` only — the `--field-shadow`.
///
/// The chrome paints the caller's resolved radius, so a per-component
/// `radius` override survives it: pass `Some` with the value the override
/// resolved to, or `None` to keep the shared `field_radius` helper.
///
/// Generic over [`Styled`] so a field that needed an `.id()` first (and is
/// therefore a `Stateful<Div>`) can share it. Six components used to hand-roll
/// this, and every one of them filled the `secondary` variant with
/// `surface_secondary` instead of `--default`.
pub fn apply_field_chrome<T: Styled>(
    el: T,
    variant: FieldVariant,
    is_invalid: bool,
    is_focused: bool,
    radius_override: Option<Pixels>,
    cx: &App,
) -> T {
    apply_field_chrome_with_focus_ring(
        el,
        variant,
        is_invalid,
        is_focused,
        true,
        radius_override,
        cx,
    )
}

/// Applies the v3 field chrome with an explicit focus-ring switch.
///
/// `show_focus_ring` changes only the focused visual treatment. The element
/// remains focusable and editable, and an invalid field still reports its
/// danger state through the one-pixel border when the focus ring is disabled.
pub fn apply_field_chrome_with_focus_ring<T: Styled>(
    el: T,
    variant: FieldVariant,
    is_invalid: bool,
    is_focused: bool,
    show_focus_ring: bool,
    radius_override: Option<Pixels>,
    cx: &App,
) -> T {
    let colors = cx.colors();
    let layout = cx.layout();

    let radius = radius_override.unwrap_or_else(|| field_radius(cx));
    let mut el = el.rounded(radius).bg(match variant {
        FieldVariant::Primary => colors.field.background,
        // `.input--secondary` sets `--input-bg: var(--default)` and drops the
        // shadow. This used to use `surface_secondary`, which is a different
        // token (oklch 95.24% vs 94% in light mode) and a shade too light.
        FieldVariant::Secondary => colors.default.color,
    });

    // `--field-border-width` is 0, so a field's states are *rings*, not borders:
    // `status-focused-field` is `ring-2 ring-focus` with no offset, and
    // `status-invalid-field` is a 1px danger outline that becomes a 2px danger
    // ring once the field takes focus. Both ride on the field's own shadow,
    // because `shadow()` replaces the list rather than adding to it.
    let mut shadows = if variant == FieldVariant::Primary {
        layout.field_shadow.clone()
    } else {
        Vec::new()
    };

    if is_invalid {
        if is_focused && show_focus_ring {
            shadows.push(gpui::BoxShadow {
                color: colors.danger.color,
                offset: gpui::point(gpui::px(0.), gpui::px(0.)),
                blur_radius: gpui::px(1.),
                spread_radius: gpui::px(2.),
                inset: false,
            });
        } else {
            el = el
                .border(layout.border_width.max(gpui::px(1.)))
                .border_color(colors.danger.color);
        }
    } else if is_focused && show_focus_ring {
        shadows.extend(focus_ring_shadows(false, cx));
    } else if layout.field_border_width > gpui::px(0.) {
        el = el
            .border(layout.field_border_width)
            .border_color(colors.field.border);
    }

    if shadows.is_empty() {
        el
    } else {
        el.shadow(shadows)
    }
}

/// [`apply_field_chrome_with_focus_ring`] without the state ring, for a shell
/// whose ring is painted by a non-clipping wrapper around it.
///
/// A field that clips its children -- `Input` while multi-line, and the
/// `NumberField` and `DateField` groups, whose shells are `overflow-hidden`
/// around segments and steppers -- cannot host the overlay ring itself: the
/// ring hangs outside the box and the clip is the first thing to cut it.
/// Those shells keep the fill, border and shadow here and hand the ring to
/// their wrapper through [`field_ring_color`] and [`with_field_ring_overlay`].
/// The focused (and focused-invalid) states paint no border at all, exactly as
/// they do when the ring is a child: the ring replaces the chrome border.
pub fn apply_field_chrome_ringless<T: Styled>(
    el: T,
    variant: FieldVariant,
    is_invalid: bool,
    is_focused: bool,
    show_focus_ring: bool,
    radius_override: Option<Pixels>,
    cx: &App,
) -> T {
    let colors = cx.colors();
    let layout = cx.layout();

    let radius = radius_override.unwrap_or_else(|| field_radius(cx));
    let mut el = el.rounded(radius).bg(match variant {
        FieldVariant::Primary => colors.field.background,
        FieldVariant::Secondary => colors.default.color,
    });

    let shadows = if variant == FieldVariant::Primary {
        layout.field_shadow.clone()
    } else {
        Vec::new()
    };

    // The ring, wherever it is painted, replaces the chrome border; the
    // else-if chain the shadow and overlay spellings share is the same one.
    if field_ring_color(is_invalid, is_focused, show_focus_ring, cx).is_none() {
        if is_invalid {
            el = el
                .border(layout.border_width.max(gpui::px(1.)))
                .border_color(colors.danger.color);
        } else if layout.field_border_width > gpui::px(0.) {
            el = el
                .border(layout.field_border_width)
                .border_color(colors.field.border);
        }
    }

    if shadows.is_empty() {
        el
    } else {
        el.shadow(shadows)
    }
}

/// The colour of the state ring a field shell shows, or `None` for a state
/// that shows none.
///
/// `status-focused-field` is `ring-2 ring-focus`; `status-invalid-field`'s
/// focused form is the same geometry in `danger`. One place resolves which,
/// so a shell that paints its ring on a wrapper cannot drift from one that
/// paints it as its own child.
pub fn field_ring_color(
    is_invalid: bool,
    is_focused: bool,
    show_focus_ring: bool,
    cx: &App,
) -> Option<Hsla> {
    if !(is_focused && show_focus_ring) {
        return None;
    }
    Some(if is_invalid {
        cx.colors().danger.color
    } else {
        cx.colors().focus
    })
}

/// Hangs the ring [`field_ring_color`] resolved on an element, as an overlay
/// child drawn at `radius`.
pub fn with_field_ring_overlay<T: ParentElement>(
    el: T,
    ring: Option<Hsla>,
    radius: Pixels,
    cx: &App,
) -> T {
    match ring {
        Some(color) => el.child(ring_overlay_in(radius, false, color, cx)),
        None => el,
    }
}

/// A non-clipping carrier for a field shell that clips, so its state ring has
/// somewhere to hang.
///
/// The shells that need one -- the multi-line `Input`, the `NumberField` and
/// `DateField` groups -- are `overflow-hidden` around text, segments or
/// steppers, and an overlay ring drawn in the margin is the first thing such a
/// clip cuts. The carrier takes the shell's place in the tree and the shell
/// becomes its only child, so the ring hangs outside the shell's box but
/// inside nothing. It carries no id, no listeners and no chrome: hit-testing,
/// the focus path and the shell's own paint are unchanged.
///
/// Column flow, because that is what the shell sat in: a flex column stretches
/// its children across its width, so the shell keeps the width it had as a
/// direct child of the field's label-to-error column, and the carrier's height
/// is the shell's.
pub fn field_ring_carrier(
    shell: impl gpui::IntoElement,
    ring: Option<Hsla>,
    radius: Pixels,
    cx: &App,
) -> Div {
    with_field_ring_overlay(
        gpui::div().relative().flex().flex_col().child(shell),
        ring,
        radius,
        cx,
    )
}

/// [`field_ring_carrier`] for a ring the caller already built — the chrome
/// ramp's clip-escaping overlay, whose opacity its own tween drives — hung
/// in the same non-clipping position after the shell.
pub fn field_ring_carrier_overlay(
    shell: impl gpui::IntoElement,
    overlay: impl gpui::IntoElement,
) -> Div {
    gpui::div()
        .relative()
        .flex()
        .flex_col()
        .child(shell)
        .child(overlay)
}

/// [`apply_field_chrome_with_focus_ring`] with the ring as an overlay child.
///
/// Same chrome, except that the focused ring -- and the focused *invalid*
/// ring, which is the same geometry in `danger` -- is painted by
/// `ring_overlay_in` rather than by a blurred spread shadow. For a field
/// shell that does not clip its children; the ones that do put the same
/// overlay on a non-clipping wrapper instead and take
/// [`apply_field_chrome_ringless`] themselves.
pub fn apply_field_chrome_overlay<T: Styled + ParentElement>(
    el: T,
    variant: FieldVariant,
    is_invalid: bool,
    is_focused: bool,
    show_focus_ring: bool,
    radius_override: Option<Pixels>,
    cx: &App,
) -> T {
    let radius = radius_override.unwrap_or_else(|| field_radius(cx));
    let ring = field_ring_color(is_invalid, is_focused, show_focus_ring, cx);
    let el = apply_field_chrome_ringless(
        el,
        variant,
        is_invalid,
        is_focused,
        show_focus_ring,
        Some(radius),
        cx,
    );
    with_field_ring_overlay(el, ring, radius, cx)
}

/// Paints a filled 16-segment disc — the round cap and join completion both
/// canvas-stroked marks need (`ProgressCircle`'s arc ends, the checkbox's
/// live stroke ends and elbow), since gpui's public stroke builder has
/// neither a round cap nor a round join to pick.
pub(crate) fn paint_disc(
    center: gpui::Point<Pixels>,
    radius: Pixels,
    color: Hsla,
    window: &mut gpui::Window,
) {
    let mut builder = gpui::PathBuilder::fill();
    for step in 0..16 {
        let angle = std::f32::consts::TAU * step as f32 / 16.;
        let point = gpui::point(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        );
        if step == 0 {
            builder.move_to(point);
        } else {
            builder.line_to(point);
        }
    }
    builder.close();
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// Lifts a floating panel above the rest of the page.
///
/// gpui paints in tree order, so an `absolute` panel is still overdrawn by any
/// later sibling — a `Select` list opened near the top of a page would be
/// painted over by the sections below it. `deferred` keeps the panel in the
/// layout tree but paints it after all of its ancestors, which is what every
/// floating surface needs.
pub fn floating(el: impl gpui::IntoElement) -> gpui::Deferred {
    gpui::deferred(el)
}

pub(crate) fn window_overlay(el: impl gpui::IntoElement, window: &gpui::Window) -> gpui::Deferred {
    use gpui::InteractiveElement;

    let viewport = window.viewport_size();
    floating(
        gpui::anchored()
            .position(gpui::point(gpui::px(0.), gpui::px(0.)))
            .child(
                gpui::div()
                    .relative()
                    .occlude()
                    .w(viewport.width)
                    .h(viewport.height)
                    .flex_shrink_0()
                    .child(el),
            ),
    )
}

/// The result of an explicit overlay dismissal attempt.
///
/// Only `Handled` consumes the event. A declined outside press continues to
/// the control under the pointer, matching React Aria's `useOverlay`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DismissResult {
    /// The dismissal was handled and the event is consumed.
    Handled,
    /// The dismissal was declined and the event continues to the control underneath.
    Declined,
}

#[derive(Default)]
struct OverlayStack {
    entries: Vec<gpui::WeakEntity<OverlayRegistration>>,
    next_order: u64,
}

impl gpui::Global for OverlayStack {}

#[derive(Clone)]
struct OverlayRegistration {
    window_id: gpui::WindowId,
    order: u64,
    phase: OverlayPhase,
    keep_exiting: bool,
    exit_generation: u64,
    escape_capture: Option<std::sync::Arc<dyn Fn(&mut gpui::Window, &mut App) -> DismissResult>>,
}

/// A direct handle to one registration returned by [`overlay_scope`].
///
/// The token is intentionally not inferred from an element or a render-local
/// variable. It becomes inert when its registration is unmounted.
#[derive(Clone)]
pub struct OverlayToken {
    registration: gpui::WeakEntity<OverlayRegistration>,
    window_id: gpui::WindowId,
}

fn ensure_overlay_stack(cx: &mut App) {
    if cx.try_global::<OverlayStack>().is_none() {
        cx.set_global(OverlayStack::default());
    }
}

fn prune_overlay_stack(stack: &mut OverlayStack, cx: &App) {
    stack.entries.retain(|entry| {
        let Some(entry) = entry.upgrade() else {
            return false;
        };
        entry.read(cx).phase == OverlayPhase::Open
    });
}

fn sync_overlay_stack(registration: &gpui::Entity<OverlayRegistration>, cx: &mut App) {
    ensure_overlay_stack(cx);
    let weak = registration.downgrade();
    let active = registration.read(cx).phase == OverlayPhase::Open;
    cx.update_global::<OverlayStack, _>(|stack, cx| {
        prune_overlay_stack(stack, cx);
        if active && !stack.entries.iter().any(|entry| entry == &weak) {
            stack.entries.push(weak);
        }
    });
}

fn is_topmost(token: &OverlayToken, cx: &mut App) -> bool {
    ensure_overlay_stack(cx);
    cx.update_global::<OverlayStack, _>(|stack, cx| {
        prune_overlay_stack(stack, cx);
        let Some(registration) = token.registration.upgrade() else {
            return false;
        };
        let state = registration.read(cx);
        if state.window_id != token.window_id || state.phase != OverlayPhase::Open {
            return false;
        }
        stack
            .entries
            .iter()
            .filter_map(|entry| entry.upgrade())
            .filter(|entry| entry.read(cx).window_id == token.window_id)
            .max_by_key(|entry| entry.read(cx).order)
            .is_some_and(|entry| entry == registration)
    })
}

/// Gives an overlay a document-level Escape handler.
///
/// React Aria uses this for Tooltip because focus may sit anywhere else in the
/// document while hover keeps the tip open. [`app_focus_root`] invokes the
/// handler during capture, before a focused descendant can answer the key.
/// The newest open captured handler wins even when a non-capturing overlay was
/// registered later, matching the document listener's precedence.
pub fn capture_escape(
    token: &OverlayToken,
    handler: impl Fn(&mut gpui::Window, &mut App) -> DismissResult + 'static,
    cx: &mut App,
) {
    if let Some(registration) = token.registration.upgrade() {
        let handler = shared(handler);
        registration.update(cx, |state, _| state.escape_capture = Some(handler));
    }
}

fn dismiss_captured_escape(window: &mut gpui::Window, cx: &mut App) -> bool {
    let window_id = window.window_handle().window_id();
    ensure_overlay_stack(cx);
    let handler = cx.update_global::<OverlayStack, _>(|stack, cx| {
        prune_overlay_stack(stack, cx);
        stack
            .entries
            .iter()
            .filter_map(|entry| entry.upgrade())
            .filter(|entry| entry.read(cx).window_id == window_id)
            .filter_map(|entry| {
                let state = entry.read(cx);
                state
                    .escape_capture
                    .clone()
                    .map(|handler| (state.order, handler))
            })
            .max_by_key(|(order, _)| *order)
            .map(|(_, handler)| handler)
    });
    handler.is_some_and(|handler| handler(window, cx) == DismissResult::Handled)
}

/// Closes a floating panel on Escape and on a press outside it.
///
/// No prop table asks for this: React Aria gives every popover-like surface
/// `useOverlay`, so v3 only documents dismissal where it is *configurable*
/// (`isDismissable` on a dialog backdrop). A panel that closes only through its
/// own trigger is the difference between a port that looks right and one that
/// works -- an open menu followed the page as it scrolled and stayed open
/// forever.
///
/// Attach this to the panel itself, not to a wrapper: `on_mouse_down_out` reads
/// the element's own bounds, and the wrapper an absolute panel sits in has none,
/// which would make every press inside the panel count as outside. Escape is a
/// key event, so it needs the focus to be inside the panel -- pair it with
/// [`panel_focus`] where nothing else there is focused.
pub fn dismiss_on_escape_with_token<E: gpui::InteractiveElement>(
    el: E,
    token: OverlayToken,
    close: impl Fn(&mut gpui::Window, &mut App) -> DismissResult + 'static,
) -> E {
    el.on_key_down(move |event: &gpui::KeyDownEvent, window, cx| {
        if event.keystroke.key == "escape"
            && is_topmost(&token, cx)
            && close(window, cx) == DismissResult::Handled
        {
            cx.stop_propagation();
        }
    })
}

/// The outside-press half of overlay dismissal, for a surface whose Escape is
/// already part of a keyboard it owns -- a select and a combo box read Escape in
/// the same handler that reads the arrows, and binding it twice would close
/// twice.
pub fn dismiss_on_press_outside_with_token<E: gpui::InteractiveElement>(
    el: E,
    token: OverlayToken,
    close: impl Fn(&mut gpui::Window, &mut App) -> DismissResult + 'static,
) -> E {
    dismiss_on_press_outside_with_token_event(el, token, move |_, window, cx| close(window, cx))
}

/// The pass-through form of [`dismiss_on_press_outside_with_token`]: the
/// surface still dismisses on an outside press, but the press is *not*
/// consumed, so it also reaches the control under the pointer.
///
/// That is React Aria's `isNonModal` popover contract — no underlay div, so
/// `useInteractOutside` lets the same press activate whatever it lands on.
/// HeroUI's ComboBox is the non-modal surface (`ComboBox` passes
/// `isNonModal: true` to its `Popover`); Select, MenuTrigger and the dialog
/// popovers render the full-window underlay instead and keep the consuming
/// form above.
pub fn dismiss_on_press_outside_passthrough_with_token<E: gpui::InteractiveElement>(
    el: E,
    token: OverlayToken,
    close: impl Fn(&mut gpui::Window, &mut App) -> DismissResult + 'static,
) -> E {
    el.on_mouse_down_out(move |_, window, cx| {
        if is_topmost(&token, cx) {
            close(window, cx);
        }
    })
}

/// The event-aware form of [`dismiss_on_press_outside_with_token`].
///
/// Compound surfaces such as a menu and its deferred submenu use the pointer
/// position to treat the union of both panels as inside, while the token still
/// prevents a lower overlay from answering the same press.
pub fn dismiss_on_press_outside_with_token_event<E: gpui::InteractiveElement>(
    el: E,
    token: OverlayToken,
    close: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut App) -> DismissResult + 'static,
) -> E {
    el.on_mouse_down_out(move |event, window, cx| {
        if is_topmost(&token, cx) && close(event, window, cx) == DismissResult::Handled {
            cx.stop_propagation();
        }
    })
}

/// A focus handle for a floating panel, focused as the panel opens.
///
/// A panel nothing focuses never sees a key, so this is what makes Escape and
/// the arrows work at all. It is not a tab stop: the panel is transient, and Tab
/// inside it should reach the controls it contains.
///
/// The open transition saves the previously focused handle and claims the
/// panel. The close transition restores that handle only when focus is still
/// inside the panel, so an intentional move elsewhere wins. Both transitions
/// are remembered in keyed state derived from `base`; a per-render flag would
/// forget ownership on the repaint caused by the opening press.
#[derive(Clone, Debug, Default)]
struct PanelFocusState {
    was_open: bool,
    restore: Option<gpui::WeakFocusHandle>,
}

/// A stable handle for the control that opens a [`panel_focus`] scope.
///
/// Track this on the trigger wrapper. It is deliberately not a tab stop: the
/// trigger's own control owns that position, while this handle gives the panel
/// a stable restoration target even when the trigger is a rebuilt child.
pub fn panel_restore_focus(
    window: &mut gpui::Window,
    cx: &mut App,
    base: &gpui::ElementId,
) -> gpui::FocusHandle {
    window
        .use_keyed_state(
            element_id::scoped(base, "panel-restore-focus"),
            cx,
            |_, cx| cx.focus_handle(),
        )
        .read(cx)
        .clone()
}

pub fn panel_focus(
    window: &mut gpui::Window,
    cx: &mut App,
    base: &gpui::ElementId,
    open: bool,
) -> gpui::FocusHandle {
    let held = window.use_keyed_state(element_id::scoped(base, "panel-focus"), cx, |_, cx| {
        cx.focus_handle()
    });
    let handle = held.read(cx).clone();
    let state =
        window.use_keyed_state(element_id::scoped(base, "panel-focus-state"), cx, |_, _| {
            PanelFocusState::default()
        });
    let current = state.read(cx).clone();

    if open && !current.was_open {
        let trigger = panel_restore_focus(window, cx, base);
        // `focused` is the actual control that opened the panel. The wrapper
        // is only a fallback for a programmatic open with no focused control;
        // restoring it when it merely contains the Button loses the Button's
        // own focus-visible state and keyboard activation.
        let restore = window
            .focused(cx)
            .filter(|focused| focused.tab_stop)
            .map(|focused| focused.downgrade())
            .or_else(|| Some(trigger.downgrade()));
        window.focus(&handle, cx);
        state.update(cx, |state, _| {
            state.was_open = true;
            state.restore = restore;
        });
    } else if !open && current.was_open {
        if handle.contains_focused(window, cx) {
            if let Some(restore) = current.restore.and_then(|handle| handle.upgrade()) {
                window.focus(&restore, cx);
            }
        }
        state.update(cx, |state, _| {
            state.was_open = false;
            state.restore = None;
        });
    }
    handle
}

/// Returns a stable, non-tab-stop scope that closes an open popover when focus
/// leaves its trigger-plus-panel subtree. Track it on their common root.
///
/// Active windows use `on_focus_out`. GPUI blanks that event's focus paths for
/// inactive windows, including its headless test platform, so a shared
/// render-time edge detects moves to an outside tab stop there. Both paths
/// consume `seen_inside`, preventing duplicate closes.
#[derive(Clone, Copy, Debug, Default)]
struct CloseOnBlurState {
    /// Whether the scope held the focus as of the last observed frame.
    seen_inside: bool,
}

#[derive(Clone)]
pub struct FocusLeave {
    handle: gpui::FocusHandle,
    subscription: gpui::Entity<Option<gpui::Subscription>>,
    state: gpui::Entity<CloseOnBlurState>,
}

impl FocusLeave {
    pub fn focus_handle(&self) -> gpui::FocusHandle {
        self.handle.clone()
    }

    /// Marks a departure as already handled by the event that caused it.
    pub fn consume(&self, cx: &mut App) {
        self.state.update(cx, |state, _| state.seen_inside = false);
        self.subscription.update(cx, |slot, _| *slot = None);
    }
}

pub fn close_on_blur(
    window: &mut gpui::Window,
    cx: &mut App,
    base: &gpui::ElementId,
    open: bool,
    close: impl Fn(&mut gpui::Window, &mut App) + 'static,
) -> gpui::FocusHandle {
    on_focus_leave(window, cx, base, open, close).focus_handle()
}

/// Observes focus leaving a stable subtree while `active`.
pub fn on_focus_leave(
    window: &mut gpui::Window,
    cx: &mut App,
    base: &gpui::ElementId,
    active: bool,
    leave: impl Fn(&mut gpui::Window, &mut App) + 'static,
) -> FocusLeave {
    let held_scope = window.use_keyed_state(
        element_id::scoped(base, "close-on-blur-scope"),
        cx,
        |_, cx| cx.focus_handle().tab_stop(false),
    );
    let scope = held_scope.read(cx).clone();
    // Storing the subscription is arming; dropping it is disarming. The
    // `Option` slot flips either way without subscribing twice.
    let subscription = window.use_keyed_state(
        element_id::scoped(base, "close-on-blur-subscription"),
        cx,
        |_, _| None::<gpui::Subscription>,
    );
    let state = window.use_keyed_state(
        element_id::scoped(base, "close-on-blur-state"),
        cx,
        |_, _| CloseOnBlurState::default(),
    );
    let armed = subscription.read(cx).is_some();
    // Both observation legs hand the same closer out; gpui runs single-threaded,
    // so an `Rc` shares it without asking the closure to be `Clone`.
    let leave = std::rc::Rc::new(leave);

    // The frame-end half (real, focused windows): the guard reads the shared
    // edge so a render that got there first leaves this nothing to do, and
    // firing also drops the subscription -- a transition owns its close once.
    if active && !armed {
        // The listener is owned by `subscription`; weak captures avoid a cycle
        // that would otherwise retain an unmounted open component forever.
        let disarmer = subscription.downgrade();
        let edge = state.downgrade();
        let leave = std::rc::Rc::clone(&leave);
        let listener = window.on_focus_out(&scope, cx, move |_, window, cx| {
            let Some(edge) = edge.upgrade() else {
                return;
            };
            let due = edge.read(cx).seen_inside;
            if let Some(disarmer) = disarmer.upgrade() {
                disarmer.update(cx, |slot, _| *slot = None);
            }
            if !due {
                return;
            }
            edge.update(cx, |state, _| state.seen_inside = false);
            leave(window, cx);
        });
        subscription.update(cx, |slot, _| *slot = Some(listener));
    }

    // The render half (everywhere else): whatever the observer APIs do, a
    // focus move always invalidates the window, so a move to another tab stop
    // can be observed on the next frame. Requiring a tab stop avoids treating
    // the app root's non-interactive recovery handle as a user departure.
    // Never-before-seen counts as absent, not departed: the first frames of a
    // freshly opened surface hold no focus yet.
    if active {
        if scope.contains_focused(window, cx) {
            state.update(cx, |state, _| state.seen_inside = true);
        } else if state.read(cx).seen_inside
            && window.focused(cx).is_some_and(|focused| focused.tab_stop)
        {
            state.update(cx, |state, _| state.seen_inside = false);
            subscription.update(cx, |slot, _| *slot = None);
            leave(window, cx);
        }
    } else {
        if state.read(cx).seen_inside {
            state.update(cx, |state, _| state.seen_inside = false);
        }
        if armed {
            subscription.update(cx, |slot, _| *slot = None);
        }
    }

    FocusLeave {
        handle: scope,
        subscription,
        state,
    }
}

/// Wraps a callback for sharing between closures.
///
/// gpui callbacks take `&mut App` and therefore never leave the main thread;
/// `Arc` is used only because `Box<dyn Fn>` is not `Clone`. clippy's
/// `arc_with_non_send_sync` check is about cross-thread sharing, which cannot
/// happen here.
#[allow(clippy::arc_with_non_send_sync)]
pub fn shared<F: 'static>(f: F) -> std::sync::Arc<F> {
    std::sync::Arc::new(f)
}

/// Gives `handle` focus the first time this element renders, and never again.
///
/// This is `autoFocus`. The "first time" has to be remembered somewhere, so a
/// one-shot flag lives in element state keyed by `key`: without it the field
/// would steal focus back on every frame and the user could never leave it.
pub fn focus_once(
    window: &mut gpui::Window,
    cx: &mut App,
    key: impl Into<gpui::ElementId>,
    handle: &gpui::FocusHandle,
) {
    let done = window.use_keyed_state(key.into(), cx, |_, _| false);
    if !*done.read(cx) {
        window.focus(handle, cx);
        done.update(cx, |d, _| *d = true);
    }
}

/// Which phase an overlay is in, so `[data-exiting]` has something to render.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverlayPhase {
    /// Not rendered at all.
    #[default]
    Closed,
    /// Rendered, and animating in.
    Open,
    /// `isOpen` has gone false, but the panel is still on screen for its exit.
    Exiting,
}

/// What `overlay_phase` remembers between renders.
#[derive(Clone, Copy, Debug, Default)]
struct PhaseState {
    was_open: bool,
    exiting: bool,
    exit_generation: u64,
}

/// Registers one overlay in the app-global, window-scoped dismissal stack.
///
/// The returned token is the only handle accepted by the explicit dismissal
/// helpers. An overlay receives a new order only when it transitions from
/// `Closed` or `Exiting` to `Open`; repainting an open overlay is stable.
pub fn overlay_scope(
    window: &mut gpui::Window,
    cx: &mut App,
    key: impl Into<gpui::ElementId>,
    is_open: bool,
    keep_exiting: bool,
) -> (OverlayPhase, OverlayToken) {
    overlay_scope_with_exit(
        window,
        cx,
        key,
        is_open,
        keep_exiting,
        crate::anim::EXITING_MS,
    )
}

/// Registers an overlay whose exit lifetime differs from the shared 100ms.
pub fn overlay_scope_with_exit(
    window: &mut gpui::Window,
    cx: &mut App,
    key: impl Into<gpui::ElementId>,
    is_open: bool,
    keep_exiting: bool,
    exit_ms: u64,
) -> (OverlayPhase, OverlayToken) {
    let key = key.into();
    let window_id = window.window_handle().window_id();
    let registration = window.use_keyed_state(key, cx, |_, _| OverlayRegistration {
        window_id,
        order: 0,
        phase: OverlayPhase::Closed,
        keep_exiting,
        exit_generation: 0,
        escape_capture: None,
    });
    let current = registration.read(cx).clone();

    let phase = if is_open {
        if current.phase != OverlayPhase::Open {
            ensure_overlay_stack(cx);
            let order = cx.update_global::<OverlayStack, _>(|stack, _| {
                stack.next_order = stack.next_order.saturating_add(1);
                stack.next_order
            });
            registration.update(cx, |state, _| {
                state.order = order;
                state.phase = OverlayPhase::Open;
                state.keep_exiting = keep_exiting;
            });
        }
        OverlayPhase::Open
    } else if current.phase == OverlayPhase::Open && keep_exiting {
        let exit_generation = current.exit_generation.saturating_add(1);
        registration.update(cx, |state, _| {
            state.phase = OverlayPhase::Exiting;
            state.keep_exiting = keep_exiting;
            state.exit_generation = exit_generation;
        });
        let held = registration.downgrade();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(exit_ms))
                .await;
            cx.update(|cx| {
                let _ = held.update(cx, |state, cx| {
                    if state.phase == OverlayPhase::Exiting
                        && state.exit_generation == exit_generation
                    {
                        state.phase = OverlayPhase::Closed;
                        cx.notify();
                    }
                });
            });
        })
        .detach();
        OverlayPhase::Exiting
    } else if current.phase == OverlayPhase::Open {
        registration.update(cx, |state, _| {
            state.phase = OverlayPhase::Closed;
            state.keep_exiting = keep_exiting;
        });
        OverlayPhase::Closed
    } else if current.phase == OverlayPhase::Exiting {
        OverlayPhase::Exiting
    } else {
        if current.keep_exiting != keep_exiting {
            registration.update(cx, |state, _| state.keep_exiting = keep_exiting);
        }
        OverlayPhase::Closed
    };

    sync_overlay_stack(&registration, cx);
    (
        phase,
        OverlayToken {
            registration: registration.downgrade(),
            window_id,
        },
    )
}

/// Compose a pressable part's action without bypassing its disabled/focus semantics.
pub(crate) fn chain_press(
    handler: &mut Option<std::sync::Arc<dyn Fn(&gpui::ClickEvent, &mut gpui::Window, &mut App)>>,
    next: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut App) + 'static,
) {
    let previous = handler.take();
    *handler = Some(shared(
        move |event: &gpui::ClickEvent, window: &mut gpui::Window, cx: &mut App| {
            if let Some(previous) = &previous {
                previous(event, window, cx);
            }
            next(event, window, cx);
        },
    ));
}

/// Resolves `isOpen` into a phase that includes v3's `[data-exiting]`.
///
/// Legacy non-stack helper for components not yet migrated. New overlays must
/// use [`overlay_scope`] and pass its token to the explicit dismissal helpers.
///
/// A `RenderOnce` component drops out of the tree the moment `isOpen` goes
/// false, which leaves an exit animation nothing to play. This keeps the panel
/// alive for [`crate::anim::EXITING_MS`] afterwards: the flip to closed starts a
/// timer, and until it fires the phase is `Exiting`.
///
/// Callers render nothing on `Closed`, [`crate::anim::entering_zoom`] on `Open`
/// and [`crate::anim::exiting`] on `Exiting`.
pub fn overlay_phase(
    window: &mut gpui::Window,
    cx: &mut App,
    key: impl Into<gpui::ElementId>,
    is_open: bool,
) -> OverlayPhase {
    retained_phase(window, cx, key, is_open, true, crate::anim::EXITING_MS)
}

/// Resolves a collapsible content panel into an open, exiting, or closed
/// phase. Unlike [`overlay_phase`], this lets a component use the source
/// component's own transition duration and skip the retained exit entirely
/// under reduced motion. It does not register an overlay dismissal token.
pub fn panel_phase(
    window: &mut gpui::Window,
    cx: &mut App,
    key: impl Into<gpui::ElementId>,
    is_open: bool,
    keep_exiting: bool,
    exit_ms: u64,
) -> OverlayPhase {
    retained_phase(window, cx, key, is_open, keep_exiting, exit_ms)
}

fn retained_phase(
    window: &mut gpui::Window,
    cx: &mut App,
    key: impl Into<gpui::ElementId>,
    is_open: bool,
    keep_exiting: bool,
    exit_ms: u64,
) -> OverlayPhase {
    let key = key.into();
    let held = window.use_keyed_state(key, cx, |_, _| PhaseState::default());
    let current = *held.read(cx);

    if is_open {
        if !current.was_open {
            held.update(cx, |s, _| {
                s.was_open = true;
                s.exiting = false;
            });
        }
        OverlayPhase::Open
    } else if current.was_open && keep_exiting {
        // Just closed: hold the panel for its exit, then drop it.
        let exit_generation = current.exit_generation.saturating_add(1);
        held.update(cx, |s, _| {
            s.was_open = false;
            s.exiting = true;
            s.exit_generation = exit_generation;
        });
        let held = held.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(exit_ms))
                .await;
            cx.update(|cx| {
                held.update(cx, |s, cx| {
                    if s.exiting && s.exit_generation == exit_generation {
                        s.exiting = false;
                        cx.notify();
                    }
                });
            });
        })
        .detach();
        OverlayPhase::Exiting
    } else if current.was_open {
        held.update(cx, |s, _| {
            s.was_open = false;
            s.exiting = false;
        });
        OverlayPhase::Closed
    } else if current.exiting {
        OverlayPhase::Exiting
    } else {
        OverlayPhase::Closed
    }
}

/// Runs `apply` on the first render only.
///
/// This is how a `default*` prop seeds a caller-owned state entity: the entity
/// outlives any one render, so writing the default unconditionally would fight
/// the user on every frame. Keyed on `key`, so two components of the same kind
/// seed independently.
pub fn seed_once(
    window: &mut gpui::Window,
    cx: &mut App,
    key: impl Into<gpui::ElementId>,
    apply: impl FnOnce(&mut App),
) {
    let done = window.use_keyed_state(key.into(), cx, |_, _| false);
    if !*done.read(cx) {
        done.update(cx, |d, _| *d = true);
        apply(cx);
    }
}

/// Resolves a controlled prop against an uncontrolled default.
///
/// This is v3's `value` / `defaultValue` pair. When the caller supplies the
/// controlled value it owns the state and the setter is a no-op passthrough;
/// when it does not, the component keeps the value itself in element state,
/// seeded once from `default`.
///
/// Returns the value to render and, in the uncontrolled case, the entity to
/// write the next value into.
pub fn controlled<T>(
    window: &mut gpui::Window,
    cx: &mut App,
    key: impl Into<gpui::ElementId>,
    controlled: Option<T>,
    default: T,
) -> (T, Option<gpui::Entity<T>>)
where
    T: Clone + 'static,
{
    match controlled {
        // The caller drives it; nothing to remember.
        Some(v) => (v, None),
        None => {
            let held = window.use_keyed_state(key.into(), cx, move |_, _| default);
            let current = held.read(cx).clone();
            (current, Some(held))
        }
    }
}

// ---------------------------------------------------------------------------
// Focus rings (`status-focused`)
// ---------------------------------------------------------------------------

/// Whether the last input this app saw was a keyboard-control key.
///
/// HeroUI / React Aria treat every non-modifier key, including Escape, as
/// focus-visible. Closing a pointer-opened menu then rings its trigger. This
/// port only turns the modality on for keys that actually switch to keyboard
/// controls; Escape and modifier-only keys leave it alone, and any pointer
/// down turns it off.
struct FocusVisible(bool);
impl gpui::Global for FocusVisible {}

#[derive(Default)]
struct ActiveKeyboardPresses(Vec<(gpui::WindowId, String, gpui::WeakEntity<(bool, bool)>)>);
impl gpui::Global for ActiveKeyboardPresses {}

/// `[data-focus-visible]` — whether a focus ring should be showing.
///
/// A browser rings a control focused by the keyboard and not one focused by a
/// click; React Aria says the same thing with `data-focus-visible`, and 41 of
/// v3's stylesheets style that state. gpui reports *that* an element has focus
/// but not how the focus arrived, so the app root records which kind of input
/// was last seen and every ring in the tree reads it.
/// The shared focus portion of v3 field render props.
///
/// Fields draw their focus chrome from these three values, so content closures
/// receive them rather than re-deriving focus. Components with additional
/// render props embed the same values in their component-specific state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct FieldFocus {
    /// `isFocused` — this control holds the keyboard.
    pub is_focused: bool,
    /// `isFocusWithin` — it or something inside it does.
    pub is_focus_within: bool,
    /// `isFocusVisible` — focused, and the last input was a key.
    pub is_focus_visible: bool,
}

/// v3's *value* render props, as one value.
///
/// `Select.Value`, `Autocomplete.Value` and `ComboBox.Value` all hand their
/// children a function and pass in `{defaultChildren, isPlaceholder,
/// selectedItems, selectedText}`. `defaultChildren` is what the slot would have
/// drawn, so a caller can wrap it instead of rebuilding it -- which is what v3's
/// own examples do (`if (isPlaceholder) return defaultChildren`).
#[non_exhaustive]
pub struct SelectionValue<'a> {
    /// `selectedItems` — the chosen items' text. The order is the component's
    /// selection order: Select walks the collection, while ComboBox and
    /// Autocomplete follow their selection set's insertion order, the way
    /// pinned react-stately 3.49.0's `Set` iterates.
    pub selected_items: &'a [gpui::SharedString],
    /// Where those items sit in the collection, for a caller keyed by index,
    /// in the same order as `selected_items`.
    pub selected_indices: &'a [usize],
    /// The chosen items' keys, in selection insertion order, when the
    /// component's collection is keyed and those keys are distinct from the
    /// labels (`Autocomplete`, `ComboBox`). `Select` is index-keyed and
    /// carries no distinct key here. `selected_items` and `selected_indices`
    /// only contain entries for keys that currently resolve to collection
    /// items, so async-loaded or missing keys can make their lengths differ
    /// from `selected_keys`.
    pub selected_keys: Option<&'a [gpui::SharedString]>,
    /// `selectedText` — the same items joined. Select approximates v3's en-US
    /// list formatter; the other two use plain comma-space in this port.
    pub selected_text: &'a str,
    /// `isPlaceholder` — nothing is chosen, so the placeholder shows.
    pub is_placeholder: bool,
    /// `defaultChildren` — the element this slot would have drawn.
    pub default_children: gpui::AnyElement,
}

/// v3's interactive render props, as one value.
///
/// Every pressable control in v3 hands its children a function and passes these
/// in: `{isHovered, isPressed, isFocused, isFocusVisible, isSelected,
/// isDisabled}`; Button additionally supplies `isPending`. This port draws
/// each of those states itself, and a component
/// that also takes a content closure hands the same values over rather than
/// leaving a caller to re-derive them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct InteractiveState {
    /// `isHovered` — the pointer is over the control. Known one frame late: gpui
    /// reports a hover to a *handler*, not to the render that draws it.
    pub is_hovered: bool,
    /// `isPressed` — the pointer or activation key is down, likewise one frame late.
    pub is_pressed: bool,
    /// `isFocused`
    pub is_focused: bool,
    /// `isFocusVisible` — focused *and* the last input was a key.
    pub is_focus_visible: bool,
    /// `isSelected` — for the controls where selection is a state.
    pub is_selected: bool,
    /// `isDisabled`
    pub is_disabled: bool,
    /// `isPending` — Button is waiting for an operation while remaining focusable.
    pub is_pending: bool,
    /// `isIndeterminate` — a multi-selection row where some but not all of the
    /// group's keys are chosen.
    pub is_indeterminate: bool,
}

/// Where a control keeps the hover and press it will report next frame.
pub type Interaction = gpui::Entity<(bool, bool)>;

fn has_active_keyboard_press(slot: &Interaction, window: &gpui::Window, cx: &App) -> bool {
    let weak = slot.downgrade();
    let window_id = window.window_handle().window_id();
    cx.try_global::<ActiveKeyboardPresses>()
        .is_some_and(|pressed| {
            pressed.0.iter().any(|(active_window, _, interaction)| {
                *active_window == window_id && interaction == &weak
            })
        })
}

pub(crate) fn begin_keyboard_press(
    slot: &Interaction,
    event: &gpui::KeyDownEvent,
    window: &gpui::Window,
    cx: &mut App,
) {
    if event.is_held || !matches!(event.keystroke.key.as_str(), "enter" | "space") {
        return;
    }
    let began = slot.update(cx, |state, cx| {
        if state.1 {
            false
        } else {
            state.1 = true;
            cx.notify();
            true
        }
    });
    if began {
        if cx.try_global::<ActiveKeyboardPresses>().is_none() {
            cx.set_global(ActiveKeyboardPresses::default());
        }
        let active = (
            window.window_handle().window_id(),
            event.keystroke.key.clone(),
            slot.downgrade(),
        );
        cx.update_global::<ActiveKeyboardPresses, _>(|pressed, _| {
            pressed
                .0
                .retain(|(_, _, interaction)| interaction.upgrade().is_some());
            pressed.0.push(active);
        });
    }
}

/// The keyed `(hovered, pressed)` slot for one control.
///
/// gpui tells a *handler* about a hover and a press; a render can only read what
/// the last frame recorded, which is why this is a piece of state rather than a
/// question asked during layout.
pub fn interaction(id: gpui::ElementId, window: &mut gpui::Window, cx: &mut App) -> Interaction {
    window.use_keyed_state(id, cx, |_, _| (false, false))
}

/// Wires the hover and press handlers that keep an [`Interaction`] current.
pub fn track_interaction<T>(el: T, slot: &Interaction) -> T
where
    T: gpui::StatefulInteractiveElement + ParentElement,
{
    track_interaction_on_mouse_down(el, slot, |_, _| {})
}

/// Wires [`track_interaction`] and performs component-specific focus work in
/// the same mouse-down handler that records the press.
pub(crate) fn track_interaction_on_mouse_down<T>(
    el: T,
    slot: &Interaction,
    on_mouse_down: impl Fn(&mut gpui::Window, &mut App) + 'static,
) -> T
where
    T: gpui::StatefulInteractiveElement + ParentElement,
{
    let hover = slot.clone();
    let down = slot.clone();
    let up = slot.clone();
    let key_down = slot.clone();
    let key_up = slot.clone();
    let outside_up = slot.clone();
    // This listener is deliberately unconditional, not armed while the slot is
    // pressed: `Window::on_mouse_event` is `debug_assert_paint`-only, so a
    // press-armed registration could not exist until the frame *after* the
    // mouse down, and a release dispatched before that frame completes -- a
    // fast click, or a press dragged outside -- would be missed and leave the
    // slot stuck pressed. gpui 0.2.2 has no pointer capture; the per-frame
    // re-registration is what makes any outside release observable at all.
    let release = gpui::canvas(
        |bounds, _, _| bounds,
        move |_, _, window, _| {
            window.on_mouse_event(move |event: &gpui::MouseUpEvent, phase, window, cx| {
                if phase == gpui::DispatchPhase::Capture
                    && event.button == gpui::MouseButton::Left
                    && !has_active_keyboard_press(&outside_up, window, cx)
                {
                    outside_up.update(cx, |state, cx| {
                        if state.1 {
                            state.1 = false;
                            cx.notify();
                        }
                    });
                }
            });
        },
    )
    .absolute()
    .inset_0();
    el.on_hover(move |over, _, cx| {
        let over = *over;
        hover.update(cx, |state, cx| {
            if state.0 != over {
                state.0 = over;
                cx.notify();
            }
        });
    })
    .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
        down.update(cx, |state, cx| {
            if !state.1 {
                state.1 = true;
                cx.notify();
            }
        });
        on_mouse_down(window, cx);
    })
    .on_mouse_up(gpui::MouseButton::Left, move |_, window, cx| {
        if !has_active_keyboard_press(&up, window, cx) {
            up.update(cx, |state, cx| {
                if state.1 {
                    state.1 = false;
                    cx.notify();
                }
            });
        }
    })
    .on_key_down(move |event, window, cx| {
        begin_keyboard_press(&key_down, event, window, cx);
    })
    .on_key_up(move |event, _, cx| {
        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
            key_up.update(cx, |state, cx| {
                if state.1 {
                    state.1 = false;
                    cx.notify();
                }
            });
        }
    })
    .child(release)
}

/// Whether focus rings should currently be shown, i.e. the last interaction was keyboard-driven.
pub fn focus_visible(cx: &App) -> bool {
    cx.try_global::<FocusVisible>().is_some_and(|v| v.0)
}

/// Sets whether focus rings should be shown, refreshing windows when the value changes.
pub fn set_focus_visible(visible: bool, cx: &mut App) {
    if focus_visible(cx) != visible {
        cx.set_global(FocusVisible(visible));
        cx.refresh_windows();
    }
}

/// Whether `target` should paint the keyboard focus ring.
pub(crate) fn shows_focus_ring(target: bool, cx: &App) -> bool {
    target && focus_visible(cx)
}

/// Keys that switch the app into keyboard focus-ring modality.
///
/// Escape dismisses overlays; it is not a switch to keyboard controls.
/// Modifier-only keys also do not count. Any other key does, including Tab,
/// arrows, Enter/Space, typeahead and editing keys.
pub(crate) fn key_enables_focus_visible(key: &str) -> bool {
    !matches!(
        key,
        "escape"
            | "shift"
            | "control"
            | "ctrl"
            | "alt"
            | "option"
            | "meta"
            | "command"
            | "win"
            | "windows"
            | "fn"
            | "function"
    )
}

/// The window-level focus root kept by [`app_focus_root`].
struct FocusRoot {
    handle: gpui::FocusHandle,
    /// The focused element the root last saw painted inside it. Focus that
    /// stays on it after it has left the tree is focus on nothing.
    painted_focus: Option<gpui::WeakFocusHandle>,
}

/// Records keyboard-versus-pointer input, and moves the focus on Tab.
///
/// Put this on the app's root element once. Three things have to be true for a
/// focus ring to work at all, and this is where they are arranged:
///
/// - **The root holds the focus when nothing else does.** gpui delivers a key
///   event to the focused element and then up through its ancestors; with
///   nothing focused there is no chain, so the very first Tab would go nowhere.
///   The root claims the focus only when the window has no focused handle, or
///   when the focused element was painted inside the root and has since left
///   the tree. A handle focused before it first paints — a view focusing its
///   own handle while its window opens, or a panel focusing a field it has
///   not drawn yet — keeps the focus.
/// - **Tab moves the focus.** In a browser the platform does this. Here the app
///   asks for it, and gpui walks the tab stops in tree order.
/// - **The kind of input is recorded**, because a ring shows only after a
///   keyboard-control key, never after a pointer press or Escape. Both halves
///   run in the capture phase, before the event reaches whatever it landed
///   on — a descendant that stops propagation cannot hide the modality.
pub fn app_focus_root<T>(el: T, window: &mut gpui::Window, cx: &mut App) -> T
where
    T: gpui::InteractiveElement,
{
    let state = window.use_keyed_state(
        gpui::ElementId::Name("herogpui-focus-root".into()),
        cx,
        |_, cx| FocusRoot {
            handle: cx.focus_handle(),
            painted_focus: None,
        },
    );
    let root = state.read(cx).handle.clone();
    let claim = match window.focused(cx) {
        None => true,
        Some(focused) if root.contains_focused(window, cx) => {
            state.update(cx, |state, _| {
                state.painted_focus = Some(focused.downgrade());
            });
            false
        }
        Some(focused) => state
            .read(cx)
            .painted_focus
            .as_ref()
            .is_some_and(|painted| *painted == focused),
    };
    if claim {
        window.focus(&root, cx);
    }
    el.track_focus(&root)
        .capture_any_mouse_down(|_, _, cx| set_focus_visible(false, cx))
        .capture_key_down(|event, window, cx| {
            // Keyboard modality is recorded in the capture phase, the way
            // React Aria's document-level keydown listener does it: a
            // descendant that stops propagation on this key still leaves the
            // modality recorded, so its focus move shows the ring.
            if key_enables_focus_visible(&event.keystroke.key) {
                set_focus_visible(true, cx);
            }
            if event.keystroke.key == "escape" && dismiss_captured_escape(window, cx) {
                cx.stop_propagation();
            }
        })
        .on_key_down(|event, window, cx| {
            if event.keystroke.key == "tab" {
                if event.keystroke.modifiers.shift {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
                cx.stop_propagation();
            }
        })
        .capture_key_up(|event, window, cx| {
            if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                || cx.try_global::<ActiveKeyboardPresses>().is_none()
            {
                return;
            }
            let window_id = window.window_handle().window_id();
            let key = event.keystroke.key.as_str();
            let interactions = cx.update_global::<ActiveKeyboardPresses, _>(|pressed, _| {
                let mut released = Vec::new();
                pressed
                    .0
                    .retain(|(active_window, active_key, interaction)| {
                        if *active_window == window_id && active_key == key {
                            released.push(interaction.clone());
                            false
                        } else {
                            interaction.upgrade().is_some()
                        }
                    });
                released
            });
            for interaction in interactions {
                if let Some(interaction) = interaction.upgrade() {
                    interaction.update(cx, |state, cx| {
                        if state.1 {
                            state.1 = false;
                            cx.notify();
                        }
                    });
                }
            }
        })
}

/// A focus handle the Tab key can reach, kept in the window's keyed state.
///
/// gpui registers a tab stop from the **handle's** own `tab_stop` flag; the
/// element's `tab_index` builder only configures a handle the element creates
/// for itself, which a component that has to read its own focus state cannot
/// use. Marking the handle is what makes `window.focus_next(cx)` see it.
pub fn tab_stop_handle(
    id: gpui::ElementId,
    window: &mut gpui::Window,
    cx: &mut App,
) -> gpui::FocusHandle {
    window
        .use_keyed_state(id, cx, |_, cx| cx.focus_handle().tab_stop(true))
        .read(cx)
        .clone()
}

/// v3's `ring-2`: the focus ring's own thickness, shared by the shadow and
/// the overlay spellings of it.
const RING_WIDTH: Pixels = gpui::px(2.);

/// The shadows that draw v3's focus ring.
///
/// `status-focused` is `ring-2 ring-focus` over a `ring-offset-2` in the
/// background colour: two rings, the inner one separating the accent from the
/// control. A ring costs no layout here because it is a shadow -- a border would
/// move the content inside it -- and they are painted largest first, since a
/// later shadow paints over an earlier one and that overlap is what carves the
/// gap.
///
/// **The blur cannot be zero.** gpui's shadow shader is a Gaussian integral: it
/// samples over `3 * blur_radius`, so a blur of zero integrates over nothing and
/// paints a completely transparent shadow -- which is why the first version of
/// this drew no ring at all. One pixel is the smallest blur that draws, and it
/// softens the ring's outer edge by about a pixel: the closest this gpui gets to
/// a crisp `ring-2`.
///
/// [`focus_ring_overlay`] is the default spelling now; this one remains for the
/// cases its scalar bands cannot draw, listed there.
pub fn focus_ring_shadows(offset: bool, cx: &App) -> Vec<gpui::BoxShadow> {
    let colors = cx.colors();
    let layout = cx.layout();
    let ring = RING_WIDTH;
    let blur = gpui::px(1.);
    let gap = if offset {
        layout.ring_offset_width
    } else {
        gpui::px(0.)
    };
    let mut shadows = vec![gpui::BoxShadow {
        color: colors.focus,
        offset: gpui::point(gpui::px(0.), gpui::px(0.)),
        blur_radius: blur,
        spread_radius: gap + ring,
        inset: false,
    }];
    if gap > gpui::px(0.) {
        shadows.push(gpui::BoxShadow {
            color: colors.background,
            offset: gpui::point(gpui::px(0.), gpui::px(0.)),
            blur_radius: blur,
            spread_radius: gap,
            inset: false,
        });
    }
    shadows
}

/// The `status-focused` ring as geometry rather than shadow.
///
/// [`focus_ring_shadows`] is a faithful *offset* of the ring, but vanilla gpui
/// gets two things wrong about it that no shadow parameter can fix:
///
/// 1. A spread shadow dilates the element's box while keeping the element's
///    corner *radius*, so the ring's outer corner stays as tight as the
///    element's and reads squarer than CSS, where the outer radius of a ring is
///    `r + gap + ring`.
/// 2. The shadow shader is a Gaussian integral over `3 * blur_radius`, so a
///    blur of zero paints nothing at all and the ring has to carry a one-pixel
///    blur. Tailwind's `ring-2` is a crisp `0 0 0 2px`.
///
/// Borders have neither problem: gpui paints them through the same signed
/// distance field as a background, crisply antialiased, and with the radius the
/// element asks for. So the ring is painted as an absolutely positioned,
/// *bordered* child instead. Taffy lays an absolute child out against its
/// parent's box without the parent needing `.relative()`, and a negative
/// `inset` pushes the child outside that box, which is what puts the ring in the
/// margin where a shadow would have been. Concentric by construction: the outer
/// div's border sits between radius `radius + gap + ring` and `radius + gap`,
/// and the gap div's between `radius + gap` and `radius`.
///
/// The overlay takes neither pointer events nor focus: it has no id, no
/// listeners, no `occlude()`, and no mouse cursor, which is exactly the set
/// `Interactivity::should_insert_hitbox` checks, so it inserts no hitbox and
/// cannot shadow a sibling's hover.
///
/// A ring drawn outside the box is the first thing an `overflow_hidden` parent
/// cuts off, so an element that clips does not host the overlay itself: it
/// becomes the only child of a non-clipping carrier (see
/// [`field_ring_carrier`], and the `Switch` track's) and the overlay hangs
/// there instead. What is left on [`focus_ring_shadows`] is the shape it
/// cannot draw: a control whose four corners do not resolve to one radius --
/// a grouped `Button` or `ToggleButton` `Start`/`End` member, or any member an
/// `sx` refinement makes asymmetric -- since the overlay's bands are built
/// from a scalar, plus the one ring that is interpolated frame by frame
/// (`anim::field_chrome_ramp`'s tracked ring) — but only its in-flow form:
/// a clip-escaping (`ring_escapes_clip`) shell cannot host the ring as a
/// child at all, deferred or not, so the ramp hands the caller this same
/// hollow overlay geometry — opacity-tweened — to hang on the non-clipping
/// carrier instead, where a filled spread shadow would have covered the
/// shell's interior.
/// A ring whose opacity tweens --
/// the colour picker's slider thumb, swatch items, and trigger -- still uses
/// this overlay: the caller animates `.opacity(..)` on the returned child
/// instead of shadow alpha.
pub fn focus_ring_overlay(radius: Pixels, offset: bool, cx: &App) -> Div {
    ring_overlay_in(radius, offset, cx.colors().focus, cx)
}

/// [`focus_ring_overlay`] in an explicit colour, for the field family's
/// `status-invalid-field` ring, which is the same geometry in `danger`.
pub(crate) fn ring_overlay_in(radius: Pixels, offset: bool, color: Hsla, cx: &App) -> Div {
    let gap = if offset {
        cx.layout().ring_offset_width
    } else {
        gpui::px(0.)
    };
    let outer = ring_overlay_band(radius, gap, color);
    if gap > gpui::px(0.) {
        outer.child(ring_overlay_gap(radius, gap, cx.colors().background))
    } else {
        outer
    }
}

/// How far the ring's blur reaches past its outer edge, in logical pixels.
const RING_BLUR_REACH: Pixels = gpui::px(3.);
/// The Gaussian's standard deviation, in logical pixels, tuned against the
/// fork build's ring profile read one device pixel at a time at 2x.
const RING_BLUR_SIGMA: f32 = 0.7;
/// How far the stroke extends *under* the mask, so the blur of its inner
/// edge happens where the mask hides it and the ring meets the gap at full
/// strength, as the fork's did.
const RING_UNDERLAP: f32 = 1.5;

fn ring_overlay_radius(width: f32, height: f32, radius: f32) -> f32 {
    let inset = f32::from(RING_WIDTH + RING_BLUR_REACH);
    // SVG clamps rx and ry independently. Resolve the shared circular radius
    // first, like CSS/GPUI, so a pill never acquires elliptical ring corners.
    radius
        .min((width - 2. * inset).max(0.) / 2.)
        .min((height - 2. * inset).max(0.) / 2.)
}

/// The accent band: v3's `ring-2`, `gap` outside the control.
///
/// v3's ring is a crisp `0 0 0 2px` box shadow; the retired fork drew it with
/// a one-pixel blur (the shader's minimum, see [`focus_ring_shadows`]) and
/// that soft profile is the look this port keeps. Vanilla gpui cannot draw
/// it as a shadow with the right corners (a spread shadow keeps the element's
/// radius), and bordered bands stipple along the arc, so the band is an SVG:
/// a `stroke-width: 2` rounded rectangle under `feGaussianBlur`, masked to
/// the outside of the gap so the inner edge stays crisp against the control,
/// rasterised by resvg at device resolution and tinted through gpui's
/// monochrome sprite path (`Window::paint_svg`, the same alpha-mask route
/// the checkerboard cells take). The document is built at paint time from
/// the canvas bounds, so the ring fits any control size; the cache key
/// carries size and radius, so distinct geometries never share a raster.
fn ring_overlay_band(radius: Pixels, gap: Pixels, color: Hsla) -> Div {
    let reach = RING_WIDTH + RING_BLUR_REACH;
    let corner = radius + gap;
    let canvas = gpui::canvas(
        |_, _, _| (),
        move |bounds, _, window, cx| {
            let w = f32::from(bounds.size.width);
            let h = f32::from(bounds.size.height);
            if w <= 0. || h <= 0. {
                return;
            }
            let ring = f32::from(RING_WIDTH);
            let inner = f32::from(reach);
            let rg = ring_overlay_radius(w, h, f32::from(corner));
            let stroke_w = ring + RING_UNDERLAP;
            let stroke_at = f32::from(RING_BLUR_REACH) + stroke_w / 2.;
            let svg = format!(
                concat!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.2}\" height=\"{h:.2}\" ",
                    "viewBox=\"0 0 {w:.2} {h:.2}\"><defs>",
                    "<filter id=\"b\" x=\"-50%\" y=\"-50%\" width=\"200%\" height=\"200%\">",
                    "<feGaussianBlur stdDeviation=\"{sigma:.3}\"/></filter>",
                    "<mask id=\"m\"><rect width=\"{w:.2}\" height=\"{h:.2}\" fill=\"#fff\"/>",
                    "<rect x=\"{inner:.2}\" y=\"{inner:.2}\" width=\"{iw:.2}\" height=\"{ih:.2}\" rx=\"{rg:.2}\" fill=\"#000\"/>",
                    "</mask></defs><g mask=\"url(#m)\">",
                    "<rect x=\"{sx:.2}\" y=\"{sx:.2}\" width=\"{sw:.2}\" height=\"{sh:.2}\" rx=\"{srx:.2}\" ",
                    "fill=\"none\" stroke=\"#000\" stroke-width=\"{ring:.2}\" filter=\"url(#b)\"/></g></svg>"
                ),
                w = w,
                h = h,
                sigma = RING_BLUR_SIGMA,
                inner = inner,
                iw = (w - 2. * inner).max(0.),
                ih = (h - 2. * inner).max(0.),
                rg = rg,
                sx = stroke_at,
                sw = (w - 2. * stroke_at).max(0.),
                sh = (h - 2. * stroke_at).max(0.),
                srx = (rg + ring - stroke_w / 2.).max(0.),
                ring = stroke_w,
            );
            let key: gpui::SharedString =
                format!("herogpui://focus-ring/{w:.1}x{h:.1}/r{rg:.1}").into();
            let _ = window.paint_svg(
                bounds,
                key,
                Some(svg.as_bytes()),
                gpui::TransformationMatrix::unit(),
                color,
                cx,
            );
        },
    )
    .absolute()
    .inset(-reach);
    gpui::div()
        .absolute()
        .inset(-gap)
        .rounded(corner)
        .child(canvas)
}

/// The `ring-offset` band, in the background colour, which is what separates
/// the accent from the control: it fills the carrier's box, `gap` wide from
/// the control's edge outward.
fn ring_overlay_gap(radius: Pixels, gap: Pixels, background: Hsla) -> Div {
    gpui::div()
        .absolute()
        .inset(gpui::px(0.))
        .rounded(radius + gap)
        .border(gap)
        .border_color(background)
}

/// [`with_focus_ring`] for an element that can host the ring as a child.
///
/// `base` is still applied as the element's shadow list whether or not it is
/// focused, because `shadow()` replaces rather than adds; only the *ring* moves
/// from the shadow list into an overlay child. The overlay is appended last so
/// it paints above the element's own content.
pub fn with_focus_ring_overlay<T: Styled + ParentElement>(
    el: T,
    focused: bool,
    offset: bool,
    radius: Pixels,
    base: Vec<gpui::BoxShadow>,
    cx: &App,
) -> T {
    let el = if base.is_empty() { el } else { el.shadow(base) };
    if !focused {
        return el;
    }
    el.child(focus_ring_overlay(radius, offset, cx))
}

/// [`ring_if_focused`] painted as an overlay child instead of a shadow.
pub fn ring_overlay_if_focused<T: Styled + ParentElement>(
    el: T,
    handle: &gpui::FocusHandle,
    offset: bool,
    radius: Pixels,
    base: Vec<gpui::BoxShadow>,
    window: &gpui::Window,
    cx: &App,
) -> T {
    let focused = shows_focus_ring(handle.is_focused(window), cx);
    with_focus_ring_overlay(el, focused, offset, radius, base, cx)
}

/// Keeps Tab inside `scope`, which is v3's `Tab` cycles elements.
///
/// gpui's tab order is the window's: a tab group only *orders* its children, so
/// Tab walks straight out of a dialog and into the page behind it. There is no
/// way to enumerate one subtree's stops either, so the trap is done by moving
/// and checking: step, and if the focus left the scope, come back in from the
/// far end. Reversing means walking forward until the focus leaves and stepping
/// back once, which is bounded so a dialog with no stops of its own cannot spin.
///
/// The step has to be ours rather than the app root's, so this stops
/// propagation: `util::app_focus_root` binds Tab to `focus_next` on a listener
/// higher in the tree, and both firing would move twice.
pub fn trap_tab<T: gpui::InteractiveElement>(el: T, scope: &gpui::FocusHandle) -> T {
    let scope = scope.clone();
    el.on_key_down(move |event: &gpui::KeyDownEvent, window, cx| {
        if event.keystroke.key != "tab" {
            return;
        }
        // Stopping propagation also skips the root's `set_focus_visible`, and a
        // trapped Tab that moved the focus without turning the ring on looks
        // like it did nothing at all.
        cx.stop_propagation();
        set_focus_visible(true, cx);
        let back = event.keystroke.modifiers.shift;
        if back {
            window.focus_prev(cx);
        } else {
            window.focus_next(cx);
        }
        if scope.contains_focused(window, cx) {
            return;
        }
        // Out of the scope: re-enter from the other side.
        window.focus(&scope, cx);
        window.focus_next(cx);
        if !back {
            return;
        }
        // Backwards: walk to the last stop inside, then stop one short.
        for _ in 0..256 {
            window.focus_next(cx);
            if !scope.contains_focused(window, cx) {
                window.focus_prev(cx);
                return;
            }
        }
    })
}

/// v3's *inset* focus ring, as an overlay to hang inside the focused element.
///
/// A table is the exception to `status-focused`: `.table__cell` and
/// `.table__column` are `shadow-[inset_0_0_0_2px_var(--focus)]` with
/// `rounded-lg`, and a focused row draws the same ring split across its cells so
/// it reads as one continuous outline *inside* the row. An outset ring cannot
/// work there -- the next cell is flush against it, so a ring drawn outside is
/// either clipped or, on a transparent cell, bleeds through and fills it (a
/// focused column header came out solid accent).
///
/// gpui has no inset shadow and a border would move the content, so the ring is
/// an absolutely positioned child: it paints over the element and costs no
/// layout. The parent needs `.relative()`.
pub fn inset_focus_ring(cx: &App) -> Div {
    let colors = cx.colors();
    gpui::div()
        .absolute()
        .inset_0()
        .border_2()
        .border_color(colors.focus)
        .rounded(key_radius(cx))
}

/// Applies the focus ring on top of whatever the element already casts.
///
/// `base` is the element's own shadow list, because `shadow()` replaces rather
/// than adds: a focused field that dropped its `field_shadow` would flatten as
/// it took focus.
pub fn with_focus_ring<T: Styled>(
    el: T,
    focused: bool,
    offset: bool,
    base: Vec<gpui::BoxShadow>,
    cx: &App,
) -> T {
    if !focused {
        return if base.is_empty() { el } else { el.shadow(base) };
    }
    let mut all = base;
    all.extend(focus_ring_shadows(offset, cx));
    el.shadow(all)
}

/// The ring a control shows when it holds a keyboard focus.
///
/// The two conditions v3's selector has: the element is focused, *and* the focus
/// came from the keyboard.
pub fn ring_if_focused<T: Styled>(
    el: T,
    handle: &gpui::FocusHandle,
    offset: bool,
    base: Vec<gpui::BoxShadow>,
    window: &gpui::Window,
    cx: &App,
) -> T {
    let focused = shows_focus_ring(handle.is_focused(window), cx);
    with_focus_ring(el, focused, offset, base, cx)
}

// The `sx` slot: this port's answer to React's `sx`. One slot per component
// where the caller restyles the component's root element with GPUI's own
// styling methods. The closure styles a scratch `Div`; only the refinement it
// leaves behind is kept, and each component merges it over its root style at
// the very end of render, so an override wins over every token-driven value
// the component set.

/// Captures the styling a caller's `sx` closure leaves on a scratch `Div`.
///
/// Children, listeners and ids the closure adds to the scratch are dropped on
/// purpose: the slot restyles the component's own root, it does not substitute
/// a new one.
pub fn capture_sx(style: impl FnOnce(Div) -> Div) -> Box<gpui::StyleRefinement> {
    Box::new(style(gpui::div()).style().clone())
}

/// Adds a closure's root refinement to one already set through GPUI's
/// `Styled` methods (or an earlier `sx` call). Builder order stays meaningful
/// for fields both calls set; unrelated fields remain in place.
pub(crate) fn refine_sx(
    sx: &mut Option<Box<gpui::StyleRefinement>>,
    style: impl FnOnce(Div) -> Div,
) {
    let next = capture_sx(style);
    if let Some(current) = sx {
        current.as_mut().refine(&next);
    } else {
        *sx = Some(next);
    }
}

pub(crate) fn split_press_style(
    style: &gpui::StyleRefinement,
) -> (gpui::StyleRefinement, gpui::StyleRefinement) {
    let mut skin = style.clone();
    let mut slot = gpui::StyleRefinement::default();
    macro_rules! move_field {
        ($($field:ident),+ $(,)?) => { $(slot.$field = std::mem::take(&mut skin.$field);)+ };
    }
    move_field!(
        position,
        inset,
        margin,
        align_self,
        flex_basis,
        flex_grow,
        flex_shrink,
        grid_location,
        opacity
    );
    slot.size = skin.size.clone();
    slot.min_size = skin.min_size.clone();
    slot.max_size = skin.max_size.clone();
    slot.aspect_ratio = skin.aspect_ratio;
    slot.display = skin.display;
    slot.visibility = skin.visibility;
    slot.mouse_cursor = skin.mouse_cursor;
    slot.overflow = skin.overflow.clone();
    if matches!(
        slot.size.width,
        Some(gpui::Length::Definite(gpui::DefiniteLength::Fraction(_)))
    ) {
        skin.size.width = Some(gpui::relative(1.).into());
    }
    if matches!(
        slot.size.height,
        Some(gpui::Length::Definite(gpui::DefiniteLength::Fraction(_)))
    ) {
        skin.size.height = Some(gpui::relative(1.).into());
    }
    (skin, slot)
}

/// The `Styled` storage is the same root-only refinement that `sx` uses.
/// Invoke this beside each component type with an `sx` field so GPUI's full
/// style vocabulary can be used without forwarding styles into child parts.
macro_rules! impl_component_styled {
    ($($component:ty),+ $(,)?) => {
        $(
            impl gpui::Styled for $component {
                fn style(&mut self) -> &mut gpui::StyleRefinement {
                    self.sx
                        .get_or_insert_with(|| Box::new(gpui::StyleRefinement::default()))
                        .as_mut()
                }
            }
        )+
    };
}
pub(crate) use impl_component_styled;

/// The last laid-out bounds of a focused HeroGPUI control in each window.
/// GPUI exposes the focus handle and prepaint bounds separately, so controls
/// record the pair for keyboard ContextMenu placement. Weak handles prevent
/// the registry from keeping retired controls alive.
#[derive(Clone, Default)]
struct FocusedBounds(
    std::rc::Rc<
        std::cell::RefCell<
            std::collections::HashMap<
                gpui::WindowId,
                (gpui::WeakFocusHandle, gpui::Bounds<Pixels>),
            >,
        >,
    >,
);

impl gpui::Global for FocusedBounds {}

/// Append a layout-free probe to the focus-owning element, after its content.
/// A stale prepaint cannot anchor a later focus: lookup checks handle identity.
pub(crate) fn record_focus_bounds<T: ParentElement>(
    el: T,
    handle: &gpui::FocusHandle,
    window: &gpui::Window,
    cx: &mut App,
) -> T {
    if !handle.is_focused(window) {
        return el;
    }
    let registry = if let Some(registry) = cx.try_global::<FocusedBounds>() {
        registry.clone()
    } else {
        let registry = FocusedBounds::default();
        cx.set_global(registry.clone());
        registry
    };
    let weak = handle.downgrade();
    el.child(
        gpui::canvas(
            move |bounds, window, _| {
                registry
                    .0
                    .borrow_mut()
                    .insert(window.window_handle().window_id(), (weak, bounds));
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0(),
    )
}

/// Bounds recorded in the last prepaint for this window's current focus.
pub(crate) fn focused_element_bounds(
    window: &gpui::Window,
    cx: &App,
) -> Option<gpui::Bounds<Pixels>> {
    let focused = window.focused(cx)?;
    let registry = cx.try_global::<FocusedBounds>()?;
    let bounds = registry.0.borrow();
    let (handle, bounds) = bounds.get(&window.window_handle().window_id())?;
    (*handle == focused).then_some(*bounds)
}

/// Merges a captured `sx` refinement over a root element's own style.
///
/// Call this after every value the component derived from its variant and the
/// active theme: `refine` replaces exactly the fields the closure set and
/// leaves the rest of the component's styling alone.
pub fn apply_sx<T: Styled>(el: T, sx: &Option<Box<gpui::StyleRefinement>>) -> T {
    let Some(sx) = sx else { return el };
    let mut el = el;
    el.style().refine(sx);
    el
}

/// The solid colour an `sx` override painted as the root background, if any.
///
/// Components that draw state-driven fills of their own — Button's hover fade
/// is one — read this so the override holds across states, not only at rest.
pub fn sx_background(sx: &Option<Box<gpui::StyleRefinement>>) -> Option<Hsla> {
    match sx.as_ref()?.background {
        Some(gpui::Fill::Color(background)) => background.as_solid(),
        _ => None,
    }
}

/// The solid border colour an `sx` override set on the root, if any.
///
/// Components that paint their border as a child overlay (so an edge control
/// can paint above it) use this to preserve the same root-level customization
/// that [`apply_sx`] would otherwise provide directly.
pub fn sx_border_color(sx: &Option<Box<gpui::StyleRefinement>>) -> Option<Hsla> {
    sx.as_ref()?.border_color
}

/// The definite pixel size an `sx` override set on the root, axis by axis.
///
/// Fractions and rems resolve against the parent and the rem size, which a
/// component's own geometry cannot know; those stay with the plain refine in
/// [`apply_sx`].
pub fn sx_pixel_size(sx: &Option<Box<gpui::StyleRefinement>>) -> gpui::Size<Option<Pixels>> {
    fn definite(length: gpui::Length) -> Option<Pixels> {
        match length {
            gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                gpui::AbsoluteLength::Pixels(pixels),
            )) => Some(pixels),
            _ => None,
        }
    }
    let Some(sx) = sx else {
        return gpui::Size {
            width: None,
            height: None,
        };
    };
    gpui::Size {
        width: sx.size.width.and_then(definite),
        height: sx.size.height.and_then(definite),
    }
}

/// The pair [`crate::anim::hover_fade`] eases between, resolving a component's
/// resting pair against the two caller-owned overrides.
///
/// Precedence, in one place because the three cases are easy to conflate:
///
/// - `hover_bg` set: the fade runs from the resting background — the `sx`
///   background if there is one, the component's resting colour otherwise —
///   to the named hover colour.
/// - only an `sx` background: both endpoints are that colour, so the fade
///   paints the override rather than easing the component colour back over it.
/// - neither: the component's own pair, untouched.
///
/// Pure so the precedence is table-testable without a window.
pub(crate) fn fade_endpoints(
    variant: Option<(Hsla, Hsla)>,
    sx_background: Option<Hsla>,
    hover_bg: Option<Hsla>,
) -> Option<(Hsla, Hsla)> {
    let resting = sx_background.or_else(|| variant.map(|(idle, _)| idle));
    match (hover_bg, resting) {
        (Some(hover), Some(resting)) => Some((resting, hover)),
        _ => variant.map(|colors| sx_background.map_or(colors, |color| (color, color))),
    }
}

/// The leading v3 pairs with a Tailwind text step: 12/16, 14/20 and 16/24.
///
/// `None` for a size outside the table, so an override keeps the component's
/// own leading rather than guessing.
pub(crate) fn leading_for(text_size: Pixels) -> Option<Pixels> {
    let size = f32::from(text_size);
    if (size - 12.0).abs() < f32::EPSILON {
        Some(gpui::px(16.))
    } else if (size - 14.0).abs() < f32::EPSILON {
        Some(gpui::px(20.))
    } else if (size - 16.0).abs() < f32::EPSILON {
        Some(gpui::px(24.))
    } else {
        None
    }
}

/// Refines `el`'s corners with the explicit `sx` corners, leaving each corner
/// with the component's own radius when the override did not name it.
///
/// Child painted parts (a slider's track/fill/knob, a switch's track/thumb)
/// call this after their own radius so an `sx` corner wins per corner.
/// Fill corners the caller did not name, so a theme radius reaches every
/// painted part while an explicit `sx` corner still wins per corner.
pub(crate) fn fill_unspecified_corners(
    mut corners: gpui::Corners<Option<Pixels>>,
    radius: Option<Pixels>,
) -> gpui::Corners<Option<Pixels>> {
    let Some(radius) = radius else {
        return corners;
    };
    if corners.top_left.is_none() {
        corners.top_left = Some(radius);
    }
    if corners.top_right.is_none() {
        corners.top_right = Some(radius);
    }
    if corners.bottom_right.is_none() {
        corners.bottom_right = Some(radius);
    }
    if corners.bottom_left.is_none() {
        corners.bottom_left = Some(radius);
    }
    corners
}

pub(crate) fn round_sx_corners<T: Styled>(el: T, corners: &gpui::Corners<Option<Pixels>>) -> T {
    let mut el = el;
    if let Some(pixels) = corners.top_left {
        el = el.rounded_tl(pixels);
    }
    if let Some(pixels) = corners.top_right {
        el = el.rounded_tr(pixels);
    }
    if let Some(pixels) = corners.bottom_right {
        el = el.rounded_br(pixels);
    }
    if let Some(pixels) = corners.bottom_left {
        el = el.rounded_bl(pixels);
    }
    el
}

/// The definite pixel padding an `sx` override set on the root, edge by edge.
///
/// Only pixels extract: a rem resolves against the root font size and a
/// fraction against the parent's size, neither of which a component's own
/// geometry can know. An unsupported edge reads `None` here while the plain
/// [`apply_sx`] refinement still carries the real value to the root; child
/// geometry simply cannot reconcile it yet.
// No component reconciles child geometry against an `sx` padding yet; the
// extraction is kept (and unit-tested) as the documented contract for the
// first one that does, beside `sx_radius` and `sx_border_color`.
#[allow(dead_code)]
pub fn sx_padding(sx: &Option<Box<gpui::StyleRefinement>>) -> gpui::Edges<Option<Pixels>> {
    fn definite(length: gpui::DefiniteLength) -> Option<Pixels> {
        match length {
            gpui::DefiniteLength::Absolute(gpui::AbsoluteLength::Pixels(pixels)) => Some(pixels),
            _ => None,
        }
    }
    let Some(sx) = sx else {
        return gpui::Edges::all(None);
    };
    gpui::Edges {
        top: sx.padding.top.and_then(definite),
        right: sx.padding.right.and_then(definite),
        bottom: sx.padding.bottom.and_then(definite),
        left: sx.padding.left.and_then(definite),
    }
}

/// The definite pixel corner radii an `sx` override set on the root.
///
/// `corner_radii` holds [`gpui::AbsoluteLength`]s, so the unsupported case is
/// a rem rather than a percentage; that corner reads `None` while the others
/// keep their values and [`apply_sx`] still refines the root.
pub fn sx_radius(sx: &Option<Box<gpui::StyleRefinement>>) -> gpui::Corners<Option<Pixels>> {
    fn absolute(length: gpui::AbsoluteLength) -> Option<Pixels> {
        match length {
            gpui::AbsoluteLength::Pixels(pixels) => Some(pixels),
            gpui::AbsoluteLength::Rems(_) => None,
        }
    }
    let Some(sx) = sx else {
        return gpui::Corners::default();
    };
    gpui::Corners {
        top_left: sx.corner_radii.top_left.and_then(absolute),
        top_right: sx.corner_radii.top_right.and_then(absolute),
        bottom_right: sx.corner_radii.bottom_right.and_then(absolute),
        bottom_left: sx.corner_radii.bottom_left.and_then(absolute),
    }
}

#[cfg(test)]
mod sx_extraction_tests {
    use super::*;
    use gpui::{px, relative, rems, Div, Styled};

    fn captured(style: impl FnOnce(Div) -> Div) -> Option<Box<gpui::StyleRefinement>> {
        Some(capture_sx(style))
    }

    #[test]
    #[allow(clippy::float_cmp)] // the fallback is the exact literal, not near it
    fn field_box_defaults_are_the_stock_field_metrics() {
        let field = FieldBox::default();
        assert_eq!(field.resolved_height(), FIELD_HEIGHT);
        assert_eq!(f32::from(field.resolved_padding_x()), 12.);
    }

    #[test]
    fn no_override_extracts_nothing() {
        assert_eq!(sx_padding(&None), gpui::Edges::all(None));
        assert_eq!(sx_radius(&None), gpui::Corners::default());
        assert_eq!(sx_border_color(&None), None);
    }

    #[test]
    fn border_color_override_is_preserved_for_child_chrome() {
        let color = gpui::hsla(0.58, 0.7, 0.4, 1.0);
        let sx = captured(|d| d.border_color(color));
        assert_eq!(sx_border_color(&sx), Some(color));
    }

    #[test]
    fn uniform_pixel_padding_extracts_on_every_edge() {
        let sx = captured(|d| d.p(px(8.)));
        assert_eq!(sx_padding(&sx), gpui::Edges::all(Some(px(8.))));
    }

    #[test]
    fn per_edge_padding_preserves_each_edge() {
        let sx = captured(|d| d.pt(px(1.)).pr(px(2.)).pb(px(3.)).pl(px(4.)));
        assert_eq!(
            sx_padding(&sx),
            gpui::Edges {
                top: Some(px(1.)),
                right: Some(px(2.)),
                bottom: Some(px(3.)),
                left: Some(px(4.)),
            }
        );
    }

    #[test]
    fn zero_padding_is_a_real_override() {
        let sx = captured(|d| d.p(px(0.)));
        assert_eq!(sx_padding(&sx), gpui::Edges::all(Some(px(0.))));
    }

    #[test]
    fn rems_and_fractions_stay_unsupported_edge_by_edge() {
        let sx = captured(|d| d.pt(px(2.)).pb(rems(1.)).pl(relative(0.5)));
        assert_eq!(
            sx_padding(&sx),
            gpui::Edges {
                top: Some(px(2.)),
                right: None,
                bottom: None,
                left: None,
            }
        );
        // The override itself stays on the refinement for `apply_sx`; only the
        // extracted geometry drops it.
        let raw = sx.as_ref().unwrap();
        assert!(matches!(
            raw.padding.bottom,
            Some(gpui::DefiniteLength::Absolute(gpui::AbsoluteLength::Rems(
                _
            )))
        ));
        assert!(matches!(
            raw.padding.left,
            Some(gpui::DefiniteLength::Fraction(_))
        ));
    }

    #[test]
    fn uniform_and_per_corner_radius_extract() {
        let sx = captured(|d| d.rounded(px(6.)));
        assert_eq!(
            sx_radius(&sx),
            gpui::Corners {
                top_left: Some(px(6.)),
                top_right: Some(px(6.)),
                bottom_right: Some(px(6.)),
                bottom_left: Some(px(6.)),
            }
        );

        let sx = captured(|d| d.rounded_tl(px(1.)).rounded_br(px(3.)));
        assert_eq!(
            sx_radius(&sx),
            gpui::Corners {
                top_left: Some(px(1.)),
                top_right: None,
                bottom_right: Some(px(3.)),
                bottom_left: None,
            }
        );
    }

    #[test]
    fn leading_pairs_follow_the_v3_steps() {
        assert_eq!(leading_for(px(12.)), Some(px(16.)));
        assert_eq!(leading_for(px(14.)), Some(px(20.)));
        assert_eq!(leading_for(px(16.)), Some(px(24.)));
        assert_eq!(leading_for(px(13.)), None, "an unpairable size stays unset");
    }

    #[test]
    fn explicit_sx_corners_refine_each_corner_individually() {
        let sx = captured(|d| d.rounded_tl(px(2.)).rounded_br(px(6.)));
        let corners = sx_radius(&sx);
        let mut el = round_sx_corners(gpui::div().rounded(px(4.)), &corners);
        let radii = el.style().corner_radii.clone();
        assert_eq!(
            radii.top_left,
            Some(gpui::AbsoluteLength::Pixels(px(2.))),
            "the explicit top-left corner must win"
        );
        assert_eq!(
            radii.bottom_right,
            Some(gpui::AbsoluteLength::Pixels(px(6.))),
            "the explicit bottom-right corner must win"
        );
        assert_eq!(
            radii.top_right,
            Some(gpui::AbsoluteLength::Pixels(px(4.))),
            "an unnamed corner keeps the component's own radius"
        );
        assert_eq!(
            radii.bottom_left,
            Some(gpui::AbsoluteLength::Pixels(px(4.))),
            "an unnamed corner keeps the component's own radius"
        );
    }

    #[test]
    fn rem_radius_stays_unsupported_for_that_corner() {
        let sx = captured(|d| d.rounded_tl(rems(1.)).rounded_br(px(3.)));
        assert_eq!(
            sx_radius(&sx),
            gpui::Corners {
                top_left: None,
                top_right: None,
                bottom_right: Some(px(3.)),
                bottom_left: None,
            }
        );
    }
}

#[cfg(test)]
mod overlay_stack_tests {
    use super::*;
    use gpui::AppContext;

    fn registration(
        cx: &mut gpui::TestAppContext,
        window_id: gpui::WindowId,
        order: u64,
    ) -> gpui::Entity<OverlayRegistration> {
        cx.new(|_| OverlayRegistration {
            window_id,
            order,
            phase: OverlayPhase::Open,
            keep_exiting: false,
            exit_generation: 0,
            escape_capture: None,
        })
    }

    #[gpui::test]
    fn sibling_order_is_stable_when_open_registrations_repaint(cx: &mut gpui::TestAppContext) {
        let outer = registration(cx, gpui::WindowId::from(1), 1);
        let inner = registration(cx, gpui::WindowId::from(1), 2);
        cx.update(|cx| {
            sync_overlay_stack(&outer, cx);
            sync_overlay_stack(&inner, cx);
            sync_overlay_stack(&outer, cx);
            sync_overlay_stack(&inner, cx);
            let stack = cx.global::<OverlayStack>();
            assert_eq!(stack.entries.len(), 2);
            assert_eq!(stack.entries[0].upgrade().unwrap().read(cx).order, 1);
            assert_eq!(stack.entries[1].upgrade().unwrap().read(cx).order, 2);
        });
    }

    #[gpui::test]
    fn topmost_registration_is_window_scoped(cx: &mut gpui::TestAppContext) {
        let first = registration(cx, gpui::WindowId::from(1), 1);
        let second = registration(cx, gpui::WindowId::from(2), 2);
        let first_token = OverlayToken {
            registration: first.downgrade(),
            window_id: gpui::WindowId::from(1),
        };
        let second_token = OverlayToken {
            registration: second.downgrade(),
            window_id: gpui::WindowId::from(2),
        };
        cx.update(|cx| {
            cx.set_global(OverlayStack {
                entries: vec![first.downgrade(), second.downgrade()],
                next_order: 2,
            });
            assert!(is_topmost(&first_token, cx));
            assert!(is_topmost(&second_token, cx));
        });
    }

    #[gpui::test]
    fn dead_registration_is_pruned_from_the_stack(cx: &mut gpui::TestAppContext) {
        let registration = registration(cx, gpui::WindowId::from(1), 1);
        let weak = registration.downgrade();
        cx.update(|cx| {
            cx.set_global(OverlayStack {
                entries: vec![weak.clone()],
                next_order: 1,
            });
        });
        drop(registration);
        cx.update(|cx| {
            cx.update_global::<OverlayStack, _>(|stack, cx| {
                prune_overlay_stack(stack, cx);
                assert!(stack.entries.is_empty());
            });
        });
    }
}

#[cfg(test)]
mod focus_ring_overlay_tests {
    use super::*;
    use gpui::{px, AbsoluteLength, Length, Styled};

    fn inset(el: &mut Div) -> Length {
        el.style().inset.top.unwrap()
    }

    fn radius(el: &mut Div) -> AbsoluteLength {
        el.style().corner_radii.top_left.unwrap()
    }

    fn border(el: &mut Div) -> AbsoluteLength {
        el.style().border_widths.top.unwrap()
    }

    #[test]
    fn pill_rings_use_one_radius_for_both_svg_axes() {
        // A 200x36 control, with a 2px gap and 5px SVG reach on each edge.
        // Its gap outline is a circle of radius 18+2, on either orientation.
        for (width, height) in [(214., 50.), (50., 214.), (50., 50.)] {
            assert!((ring_overlay_radius(width, height, 26.) - 20.).abs() < f32::EPSILON);
        }
        assert!((ring_overlay_radius(214., 50., 10.) - 10.).abs() < f32::EPSILON);
    }

    /// Without the offset the carrier is the control's own box: the blurred
    /// band canvas inside it reaches `2 + blur` outward from that edge.
    #[test]
    fn unoffset_ring_carrier_is_the_control_box() {
        let mut carrier = ring_overlay_band(px(8.), px(0.), gpui::red());
        assert_eq!(inset(&mut carrier), Length::Definite(px(0.).into()));
        assert_eq!(radius(&mut carrier), AbsoluteLength::Pixels(px(8.)));
        assert!(carrier.style().border_widths.top.is_none());
        assert!(
            carrier.style().box_shadow.is_none(),
            "the band is an svg, not a shadow"
        );
    }

    /// With the offset the carrier moves out by the gap, and the gap band
    /// fills it as a `gap`-wide border in the background colour, bridging the
    /// element's radius `r` to the band's inner radius `r + gap`.
    #[test]
    fn offset_ring_pushes_the_carrier_out_and_paints_the_gap() {
        let gap = px(2.);
        let mut carrier = ring_overlay_band(px(8.), gap, gpui::red());
        assert_eq!(inset(&mut carrier), Length::Definite(px(-2.).into()));
        assert_eq!(radius(&mut carrier), AbsoluteLength::Pixels(px(10.)));

        let mut inner = ring_overlay_gap(px(8.), gap, gpui::blue());
        assert_eq!(inset(&mut inner), Length::Definite(px(0.).into()));
        assert_eq!(radius(&mut inner), AbsoluteLength::Pixels(px(10.)));
        assert_eq!(border(&mut inner), AbsoluteLength::Pixels(gap));
    }

    /// The gap the offset ring leaves is the theme's `ring_offset_width`, not a
    /// literal, and the unoffset ring leaves none.
    #[gpui::test]
    fn the_offset_gap_comes_from_the_layout_theme(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            herogpui_theme::ThemeProvider::init(cx);
            let gap = cx.layout().ring_offset_width;
            let mut ring = focus_ring_overlay(px(8.), true, cx);
            assert_eq!(inset(&mut ring), Length::Definite((-gap).into()));
            assert_eq!(radius(&mut ring), AbsoluteLength::Pixels(px(8.) + gap));

            let mut flat = focus_ring_overlay(px(8.), false, cx);
            assert_eq!(inset(&mut flat), Length::Definite(px(0.).into()));
            assert_eq!(radius(&mut flat), AbsoluteLength::Pixels(px(8.)));
        });
    }
}
