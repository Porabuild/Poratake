//! Drawer — port of `@heroui/drawer`.
//!
//! Window-edge overlay panel, including when composed inside a clipped or
//! positioned container, like [`Modal`](crate::modal::Modal).

use gpui::{
    prelude::*, px, AnyElement, App, Bounds, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window,
};
use herogpui_theme::ActiveTheme;
use web_time::Instant;

use herogpui_core::{element_id, Backdrop};

use crate::a11y::{self, A11y as _};
use crate::modal::{DismissReason, OnClose, OnOpenChange};

/// Which edge the drawer is anchored to (`placement`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrawerPlacement {
    /// Anchored to the left edge.
    Left,
    /// Anchored to the right edge.
    Right,
    /// Anchored to the top edge.
    Top,
    /// Anchored to the bottom edge.
    #[default]
    Bottom,
}

/// The desktop side drawer width in v3's `drawer.css`.
const SIDE_EXTENT: gpui::Pixels = px(384.);

const DRAG_ACTIVATION: f32 = 8.;

const DRAG_DISMISS_FRACTION: f32 = 0.3;

const DRAG_VELOCITY: f32 = 0.5;

#[derive(Clone, Copy)]
struct DragState {
    start: f32,
    offset: f32,
    last: f32,
    last_at: Instant,
    velocity: f32,
    active: bool,
}

/// `Drawer.CloseTrigger` — the drawer's close slot, `absolute end-4 top-4`.
///
/// v3 spells visibility by composing or omitting the part: a composed
/// [`DrawerCloseTrigger`] renders in the slot and an omitted one leaves it
/// bare panel padding — there is no `hideCloseButton` and no automatic
/// stand-in. The part is v3's wired `CloseButton` (`slot="close"`), always
/// wired to the drawer's dismissal paths ([`Drawer::on_close`] plus
/// [`Drawer::on_open_change`], the same report Escape, the drag release and
/// the backdrop make) and closing regardless of `is_dismissible`, like v3's
/// composed trigger. Custom `children` only replace the button's glyph — the
/// press still runs the drawer's close action. With neither dismissal
/// callback on the drawer — or composed outside a [`Drawer`] — the part
/// draws nothing.
pub struct DrawerCloseTrigger {
    on_dismiss: Option<OnClose>,
    /// The id of the dialog this trigger was pulled out of; see
    /// [`crate::modal::CloseTriggerPart::wire`].
    owner: Option<gpui::ElementId>,
    /// This trigger's index within its dialog; see
    /// [`crate::modal::CloseTriggerPart::wire`].
    slot: usize,
    children: Vec<AnyElement>,
}

impl DrawerCloseTrigger {
    /// Creates a close trigger with no children.
    pub fn new() -> Self {
        Self {
            on_dismiss: None,
            owner: None,
            slot: 0,
            children: Vec::new(),
        }
    }
}

crate::modal::close_trigger_part!(DrawerCloseTrigger, "close-trigger");

/// `Drawer.Handle` — the optional drag-to-dismiss affordance.
///
/// v3 spells visibility by composing or omitting the part: a composed
/// [`DrawerHandle`] renders the `h-1 w-9 rounded-xs bg-separator` bar and,
/// while the drawer is `is_dismissible`, wires its press into the same drag
/// state the header and footer use; an omitted part leaves no bar, and a
/// composed one on a `is_dismissible(false)` drawer paints without the
/// gesture. The drawer pulls composed instances out of the body children —
/// so the scrolling surface never swallows them — and mounts them at the
/// sheet's content end for [`DrawerPlacement::Top`] (after the footer) and
/// before the header on every other placement. Composed outside a [`Drawer`]
/// the bar still paints but carries no drag wiring.
pub struct DrawerHandle {
    /// The owning drawer's placement, handed out during extraction; drives
    /// the Top/Bottom padding split and the drag axis.
    placement: Option<DrawerPlacement>,
    /// The owning drawer's keyed drag state, present only while the drawer
    /// is `is_dismissible`.
    drag: Option<gpui::Entity<Option<DragState>>>,
}

impl DrawerHandle {
    /// Creates a drag handle with no wiring; a [`Drawer`] supplies the
    /// placement and drag state when it pulls the part out of its children.
    pub fn new() -> Self {
        Self {
            placement: None,
            drag: None,
        }
    }

    /// Hands the part the drawer's placement and, while `is_dismissible`,
    /// its keyed drag state. Called by [`Drawer::render`] on every composed
    /// instance it extracts.
    fn wire(&mut self, placement: DrawerPlacement, drag: Option<gpui::Entity<Option<DragState>>>) {
        self.placement = Some(placement);
        self.drag = drag;
    }
}

impl Default for DrawerHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl Element for DrawerHandle {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        // `.drawer__handle` is `flex items-center justify-center pb-2` with
        // an `h-1 w-9 rounded-xs bg-separator` bar: the affordance that says
        // the sheet can be dragged shut. `--top` removes the padding, leaving
        // the bar flush with the moving edge.
        let placement = self.placement.unwrap_or_default();
        let colors = cx.colors();
        let mut bar = gpui::div().flex().items_center().justify_center().child(
            gpui::div()
                .h(px(4.))
                .w(px(36.))
                .rounded(crate::util::hairline_radius(cx))
                .bg(colors.separator),
        );
        bar = match placement {
            DrawerPlacement::Top => bar.pb(px(0.)),
            _ => bar.pb(px(8.)),
        };
        if let Some(drag) = self.drag.clone() {
            bar = bar.on_mouse_down(
                gpui::MouseButton::Left,
                move |ev: &gpui::MouseDownEvent, _window, cx| {
                    let at = drag_axis_position(placement, ev.position);
                    drag.update(cx, |v, cx| {
                        *v = Some(DragState {
                            start: at,
                            offset: 0.,
                            last: at,
                            last_at: Instant::now(),
                            velocity: 0.,
                            active: false,
                        });
                        cx.notify();
                    });
                },
            );
        }
        let mut inner = bar.into_any_element();
        let layout = inner.request_layout(window, cx);
        (layout, inner)
    }

    fn prepaint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<gpui::Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        state.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<gpui::Pixels>,
        state: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.paint(window, cx);
    }
}

impl IntoElement for DrawerHandle {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// HeroUI Drawer (controlled).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Drawer {
    /// Keys this dialog's own state; see [`Drawer::id`].
    id: gpui::ElementId,
    is_open: bool,
    placement: DrawerPlacement,
    is_dismissible: bool,
    backdrop: Backdrop,
    is_keyboard_dismiss_disabled: bool,
    on_open_change: Option<OnOpenChange>,
    title: Option<SharedString>,
    body: Vec<AnyElement>,
    footer: Vec<(AnyElement, bool)>,
    on_close: Option<OnClose>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Drawer {
    /// The element id this dialog's state is keyed by.
    ///
    /// Not a v3 prop: gpui needs an explicit id, and the exit phase, the focus
    /// handle and the drag offset are all keyed by it. Two dialogs on screen
    /// with the same key share all three -- which is what
    /// `HEROGPUI_OPEN_OVERLAYS=1` puts on screen.
    pub fn id(mut self, id: impl Into<gpui::ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Creates a closed drawer.
    pub fn new() -> Self {
        Self {
            id: gpui::ElementId::Name("drawer".into()),
            is_open: false,
            placement: DrawerPlacement::Bottom,
            is_dismissible: true,
            backdrop: Backdrop::Opaque,
            is_keyboard_dismiss_disabled: false,
            on_open_change: None,
            title: None,
            body: Vec::new(),
            footer: Vec::new(),
            on_close: None,
            sx: None,
        }
    }

    /// Sets whether the drawer is open (v3 `isOpen`).
    pub fn is_open(mut self, v: bool) -> Self {
        self.is_open = v;
        self
    }

    /// Sets the edge the drawer is anchored to (v3 `placement`).
    pub fn placement(mut self, p: DrawerPlacement) -> Self {
        self.placement = p;
        self
    }

    /// `isKeyboardDismissDisabled` — stops Escape from closing the drawer.
    pub fn is_keyboard_dismiss_disabled(mut self, v: bool) -> Self {
        self.is_keyboard_dismiss_disabled = v;
        self
    }

    /// `variant` on `Drawer.Backdrop` — the scrim style.
    pub fn backdrop(mut self, backdrop: Backdrop) -> Self {
        self.backdrop = backdrop;
        self
    }

    /// `onOpenChange` — fires with `false` on every dismissal path, alongside
    /// [`Drawer::on_close`].
    pub fn on_open_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_change = Some(std::sync::Arc::new(f));
        self
    }

    /// Sets whether the backdrop and dragging can dismiss the drawer.
    pub fn is_dismissible(mut self, v: bool) -> Self {
        self.is_dismissible = v;
        self
    }

    /// Sets the title text.
    pub fn title(mut self, t: impl Into<SharedString>) -> Self {
        self.title = Some(t.into());
        self
    }

    /// Appends an element to the footer.
    pub fn footer_child(mut self, el: impl IntoElement) -> Self {
        let mut element = el.into_any_element();
        let interactive = element
            .downcast_mut::<gpui::Stateful<gpui::Div>>()
            .is_some();
        self.footer.push((element, interactive));
        self
    }

    /// Enables the dismissal paths: the composed close trigger, Escape, the
    /// backdrop and a drag past the threshold all report through it,
    /// alongside [`Drawer::on_open_change`]. The [`DismissReason`] says which
    /// path fired.
    pub fn on_close(mut self, f: impl Fn(&DismissReason, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(std::sync::Arc::new(f));
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the drawer's root element — the full-window overlay the
    /// panel and the backdrop sit in — after every value the placement, the
    /// backdrop and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for Drawer {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for Drawer {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.body.extend(elements);
    }
}

impl RenderOnce for Drawer {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // v3 keeps a closing panel on screen for its `slide-out-to-*` run.
        let (phase, dismissal_token) = crate::util::overlay_scope_with_exit(
            window,
            cx,
            crate::modal::dialog_key(&self.id, "phase"),
            self.is_open,
            true,
            crate::anim::Motion::DRAWER_OUT.ms,
        );
        if phase == crate::util::OverlayPhase::Closed {
            // Every close path lands here, including a caller flipping
            // `is_open`, so the focus goes back from one place.
            crate::modal::release_dialog_focus(&self.id, window, cx);
            return gpui::div().into_any_element();
        }
        let exiting = phase == crate::util::OverlayPhase::Exiting;

        // Escape has to reach the overlay, and key events only travel to the
        // focused element and its ancestors. Claiming focus while nothing
        // inside holds it makes Escape work immediately; once a field inside
        // takes focus the event still bubbles up to here.
        let focus =
            window.use_keyed_state(crate::modal::dialog_key(&self.id, "focus"), cx, |_, cx| {
                cx.focus_handle()
            });
        let focus_handle = focus.read(cx).clone();
        crate::modal::claim_dialog_focus(&self.id, &focus_handle, window, cx);

        // A drag in progress: where it started along the dismissal axis, and how
        // far it has come. `use_keyed_state` takes `cx` mutably, so it precedes
        // the theme tokens.
        let drag =
            window.use_keyed_state(crate::modal::dialog_key(&self.id, "drag"), cx, |_, _| {
                None::<DragState>
            });
        let drag_now = *drag.read(cx);
        // How far the panel has been pulled toward its edge, which is what the
        // panel is offset by while the pointer is down.
        let drag_offset = drag_now.map_or(0.0, |state| state.offset.max(0.0));

        let panel_bounds = window.use_keyed_state(
            crate::modal::dialog_key(&self.id, "panel-bounds"),
            cx,
            |_, _| Bounds::default(),
        );
        let colors = cx.colors();

        // Every dismissal path reports through both callbacks.
        let dismiss: Option<OnClose> = match (self.on_close.clone(), self.on_open_change.clone()) {
            (None, None) => None,
            (close, open_change) => Some(crate::util::shared(
                move |reason: &DismissReason, window: &mut Window, cx: &mut App| {
                    if let Some(f) = &close {
                        f(reason, window, cx);
                    }
                    if let Some(f) = &open_change {
                        f(&false, window, cx);
                    }
                },
            )),
        };
        // The panel's outside-press dismissal; the drag release moves the
        // shared `dismiss` below, so this clone is where the two split.
        let press_out_dismiss = dismiss.clone();

        // v3 composes the close trigger as a child part. Pull every composed
        // `DrawerCloseTrigger` out of the body children and the footer row —
        // so neither slot swallows it — and hand each the drawer's dismissal
        // paths to wire the default `CloseButton` with, regardless of
        // `is_dismissible`.
        let mut footer_els: Vec<AnyElement> = std::mem::take(&mut self.footer)
            .into_iter()
            .map(|(child, _)| child)
            .collect();
        let mut close_triggers = crate::modal::take_close_triggers::<DrawerCloseTrigger>(
            &mut self.body,
            dismiss.clone(),
            &self.id,
            0,
        );
        close_triggers.extend(crate::modal::take_close_triggers::<DrawerCloseTrigger>(
            &mut footer_els,
            dismiss.clone(),
            &self.id,
            close_triggers.len(),
        ));
        // The composed triggers are out of the row; the interactivity mark is
        // recomputed for what remains.
        self.footer = footer_els
            .into_iter()
            .map(|mut child| {
                let interactive = child.downcast_mut::<gpui::Stateful<gpui::Div>>().is_some();
                (child, interactive)
            })
            .collect();

        // v3's `Drawer.Handle` is an optional composed part like the close
        // trigger: pull every composed instance out of the body children so
        // the scrolling surface never swallows it, and hand each this
        // drawer's placement plus — only while `is_dismissible` — its keyed
        // drag state. The bars mount at the sheet's content end for `Top`
        // (after the footer) and before the header on every other placement.
        let placement_for_handle = self.placement;
        let drag_for_handle = self.is_dismissible.then(|| drag.clone());
        let mut handles = Vec::new();
        self.body.retain_mut(|child| {
            if let Some(part) = child.downcast_mut::<DrawerHandle>() {
                part.wire(placement_for_handle, drag_for_handle.clone());
                handles.push(std::mem::replace(child, gpui::div().into_any_element()));
                false
            } else {
                true
            }
        });

        // `.drawer__header` is `flex flex-col gap-3` with no padding of its
        // own: the dialog's `p-6` is the inset, and the close trigger is
        // positioned against the dialog rather than sitting in the header.
        let mut header = gpui::div().flex().flex_col().gap(px(12.));
        if let Some(title) = &self.title {
            header = header.child(
                gpui::div()
                    .text_size(px(16.))
                    .line_height(px(24.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(title.to_string()),
            );
        }
        let has_header = self.title.is_some();

        if self.is_dismissible {
            let axis = self.placement;
            let held_down = drag.clone();
            header = header.on_mouse_down(
                gpui::MouseButton::Left,
                move |ev: &gpui::MouseDownEvent, _window, cx| {
                    let at = drag_axis_position(axis, ev.position);
                    held_down.update(cx, |v, cx| {
                        *v = Some(DragState {
                            start: at,
                            offset: 0.,
                            last: at,
                            last_at: Instant::now(),
                            velocity: 0.,
                            active: false,
                        });
                        cx.notify();
                    });
                },
            );
        }

        let has_body = !self.body.is_empty();
        let measured_bounds = panel_bounds.clone();
        let mut panel = gpui::div()
            // `drawer/drawer.js` is the modal's anatomy on an edge: RAC
            // `Modal`/`ModalOverlay` around a `Dialog`, so
            // `react-aria/.../dialog/useDialog.js` gives it the same
            // `role="dialog"` named by the composed `Heading`.
            .id(element_id::scoped(&self.id, "dialog"))
            .a11y_named(a11y::Role::Dialog, &a11y::Name::maybe(self.title.clone()))
            .relative()
            .flex()
            .flex_col()
            .p(px(24.))
            .when(self.placement == DrawerPlacement::Top, |panel| {
                panel.pb(px(8.))
            })
            .bg(colors.overlay.background)
            .text_color(colors.foreground)
            // Dark mode's `shadow-overlay` is the inset hairline alone.
            .when_some(cx.layout().overlay_hairline, |panel, hairline| {
                panel.border(cx.layout().border_width).border_color(hairline)
            })
            .shadow(cx.layout().overlay_shadow.clone())
            .when(self.placement == DrawerPlacement::Bottom, |panel| {
                panel
                    .rounded_tl(cx.layout().radius_2xl().min(px(32.)))
                    .rounded_tr(cx.layout().radius_2xl().min(px(32.)))
            })
            .when(self.placement == DrawerPlacement::Top, |panel| {
                panel
                    .rounded_bl(cx.layout().radius_2xl().min(px(32.)))
                    .rounded_br(cx.layout().radius_2xl().min(px(32.)))
            })
            .overflow_hidden()
            .child(
                gpui::canvas(
                    move |bounds, _, cx| {
                        measured_bounds.update(cx, |value, cx| {
                            if *value != bounds {
                                *value = bounds;
                                cx.notify();
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            // Composed `Drawer.Handle` parts sit before the header on every
            // placement except Top, whose handles mount after the footer.
            .children(if self.placement == DrawerPlacement::Top {
                Vec::new()
            } else {
                std::mem::take(&mut handles)
            })
            .when(has_header, |p| p.child(header))
            .when(has_body, |p| {
                p.child(
                    gpui::div()
                        .id(crate::modal::dialog_key(&self.id, "body-scroll"))
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.))
                        .mx(px(-3.))
                        .p(px(3.))
                        // `.drawer__header + .drawer__body` is `mt-2`.
                        .when(has_header, |b| b.mt(px(8.)))
                        .text_size(px(14.))
                        // `leading-[1.43]` on `text-sm`.
                        .line_height(px(20.))
                        .text_color(colors.muted)
                        // `.drawer__body` is the native scrolling surface;
                        // keeping the drag start listeners on its siblings
                        // leaves wheel and pointer scrolling conflict-free.
                        .overflow_y_scroll()
                        .children(self.body),
                )
            });

        // `.drawer__footer` has no border: the separator this used to draw is
        // not in v3's sheet, and `+ .drawer__footer` is `mt-5`.
        if !self.footer.is_empty() {
            // GPUI's `id().on_click(...)` divs are `Stateful<Div>` elements,
            // while a plain `Div` has no hitbox. Mark only that stateful div
            // shape as an interactive footer descendant; ordinary full-width
            // content remains on the drawer drag surface.
            let footer_content = gpui::div().flex().items_center().gap(px(8.)).children(
                self.footer.into_iter().map(|(child, interactive)| {
                    if interactive {
                        gpui::div()
                            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .child(child)
                            .into_any_element()
                    } else {
                        gpui::div().child(child).into_any_element()
                    }
                }),
            );
            let mut footer = gpui::div()
                .flex()
                .items_center()
                .justify_end()
                .when(has_header || has_body, |f| f.mt(px(20.)));
            if self.is_dismissible {
                let axis = self.placement;
                let held_down = drag.clone();
                footer = footer.on_mouse_down(
                    gpui::MouseButton::Left,
                    move |ev: &gpui::MouseDownEvent, window, cx| {
                        if window.default_prevented() {
                            return;
                        }
                        let at = drag_axis_position(axis, ev.position);
                        held_down.update(cx, |v, cx| {
                            *v = Some(DragState {
                                start: at,
                                offset: 0.,
                                last: at,
                                last_at: Instant::now(),
                                velocity: 0.,
                                active: false,
                            });
                            cx.notify();
                        });
                    },
                );
            }
            panel = panel.child(footer.child(footer_content));
        }

        // For Top the composed handles follow the footer — v3's `--top`
        // variant parks the bar at the sheet's far edge. Every other
        // placement already mounted them before the header.
        panel = panel.children(std::mem::take(&mut handles));

        // `.drawer__close-trigger` is `absolute end-4 top-4`. v3 renders a
        // close affordance only where the caller composes the part; an
        // omitted trigger leaves the spot bare panel padding.
        for trigger in close_triggers {
            panel = panel.child(
                gpui::div()
                    .absolute()
                    .top(px(16.))
                    .right(px(16.))
                    .child(trigger),
            );
        }

        // anchor to the requested edge, pulled out by however far the drag has
        // come: a div cannot be translated in this gpui, so the inset moves.
        let pulled = px(-drag_offset);
        let viewport = window.viewport_size();
        let side_extent = SIDE_EXTENT.min(viewport.width * 0.85);
        // `.drawer__content` is the positioned box the dialog slides in, and its
        // `--top`/`--bottom`/`--left`/`--right` variants are this match.
        let anchored = match self.placement {
            DrawerPlacement::Left => panel
                .w(side_extent)
                .max_w(viewport.width * 0.85)
                .h_full()
                .absolute()
                .left(pulled)
                .top(px(0.))
                .bottom(px(0.)),
            DrawerPlacement::Right => panel
                .w(side_extent)
                .max_w(viewport.width * 0.85)
                .h_full()
                .absolute()
                .right(pulled)
                .top(px(0.))
                .bottom(px(0.)),
            DrawerPlacement::Top => panel
                .max_h(viewport.height * 0.85)
                .w_full()
                .absolute()
                .top(pulled)
                .left(px(0.))
                .right(px(0.)),
            DrawerPlacement::Bottom => panel
                .max_h(viewport.height * 0.85)
                .w_full()
                .absolute()
                .bottom(pulled)
                .left(px(0.))
                .right(px(0.)),
        };

        // Backdrop dismissal lives on the **panel**, not on the backdrop: gpui
        // has no hitbox occlusion, so an `on_click` on the full-window
        // backdrop fired for a press on this sheet as well, and any pull at
        // all — even a sub-threshold one — reported a close. `on_mouse_down_out`
        // reads the panel's own bounds instead, so it only fires for a press
        // on the dimmed region around the sheet. `is_dismissible` gates it,
        // and the exit phase gets none: the drawer is already closing.
        let anchored = if self.is_dismissible && !exiting {
            if let Some(on_close) = press_out_dismiss {
                crate::util::dismiss_on_press_outside_with_token(
                    anchored,
                    dismissal_token.clone(),
                    move |window, cx| {
                        on_close(&DismissReason::Backdrop, window, cx);
                        crate::util::DismissResult::Handled
                    },
                )
            } else {
                anchored
            }
        } else {
            anchored
        };

        // v3 backdrop variants; gpui has no backdrop-filter, so `Blur` renders a
        // lighter scrim than `Opaque` to keep the layering readable.
        let backdrop_bg = match self.backdrop {
            Backdrop::Opaque => colors.backdrop,
            Backdrop::Blur => colors.backdrop.alpha(colors.backdrop.a * 0.6),
            Backdrop::Transparent => gpui::transparent_black(),
        };

        let keyboard_dismiss = if self.is_keyboard_dismiss_disabled {
            None
        } else {
            dismiss.clone()
        };

        // `Tab` cycles the drawer's own controls; see `util::trap_tab`.
        let mut overlay = crate::util::trap_tab(
            gpui::div().absolute().inset_0().track_focus(&focus_handle),
            &focus_handle,
        );
        if let Some(on_escape) = keyboard_dismiss {
            overlay = crate::util::dismiss_on_escape_with_token(
                overlay,
                dismissal_token,
                move |window, cx| {
                    on_escape(&DismissReason::Escape, window, cx);
                    crate::util::DismissResult::Handled
                },
            );
        }

        let measured_extent = match self.placement {
            DrawerPlacement::Left | DrawerPlacement::Right => {
                f32::from(panel_bounds.read(cx).size.width)
            }
            DrawerPlacement::Top | DrawerPlacement::Bottom => {
                f32::from(panel_bounds.read(cx).size.height)
            }
        };
        let fallback_extent = match self.placement {
            DrawerPlacement::Left | DrawerPlacement::Right => f32::from(side_extent),
            DrawerPlacement::Top | DrawerPlacement::Bottom => f32::from(viewport.height * 0.85),
        };
        let dismiss_extent = if measured_extent > 0. {
            measured_extent
        } else {
            fallback_extent
        };

        // GPUI's ordinary mouse-move listeners are bubble-phase and require
        // their hitbox to remain hovered. Registering the move/up listeners in
        // the canvas paint callback makes them capture-phase window listeners,
        // so a pull can continue past the moving edge and release outside it.
        if self.is_dismissible {
            let axis = self.placement;
            let held_move = drag.clone();
            let held_up = drag;
            let release = dismiss.clone();
            let global_drag = gpui::canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    let held_move = held_move.clone();
                    window.on_mouse_event(move |ev: &gpui::MouseMoveEvent, phase, _window, cx| {
                        if phase != gpui::DispatchPhase::Capture {
                            return;
                        }
                        let Some(state) = *held_move.read(cx) else {
                            return;
                        };
                        // A cancelled/lost pointer release must not leave a stale drag.
                        if ev.pressed_button != Some(gpui::MouseButton::Left) {
                            held_move.update(cx, |value, cx| {
                                *value = None;
                                cx.notify();
                            });
                            return;
                        }
                        let at = drag_axis_position(axis, ev.position);
                        let next = match axis {
                            DrawerPlacement::Left | DrawerPlacement::Top => state.start - at,
                            DrawerPlacement::Right | DrawerPlacement::Bottom => at - state.start,
                        }
                        .max(0.0);
                        let now = Instant::now();
                        let elapsed_ms = now.duration_since(state.last_at).as_secs_f32() * 1000.;
                        let movement = (at - state.last).abs();
                        let velocity = if next > state.offset && elapsed_ms > 0. {
                            movement / elapsed_ms
                        } else {
                            0.
                        };
                        let active = state.active || next >= DRAG_ACTIVATION;
                        let offset = if active { next } else { 0. };
                        if (offset - state.offset).abs() >= 1.
                            || active != state.active
                            || velocity > 0.
                        {
                            held_move.update(cx, |value, cx| {
                                *value = Some(DragState {
                                    start: state.start,
                                    offset,
                                    last: at,
                                    last_at: now,
                                    velocity,
                                    active,
                                });
                                cx.notify();
                            });
                        }
                    });

                    let held_up = held_up.clone();
                    window.on_mouse_event(move |ev: &gpui::MouseUpEvent, phase, window, cx| {
                        if phase != gpui::DispatchPhase::Capture
                            || ev.button != gpui::MouseButton::Left
                        {
                            return;
                        }
                        let Some(state) = *held_up.read(cx) else {
                            return;
                        };
                        held_up.update(cx, |value, cx| {
                            *value = None;
                            cx.notify();
                        });
                        if state.active
                            && (state.offset >= dismiss_extent * DRAG_DISMISS_FRACTION
                                || state.velocity > DRAG_VELOCITY)
                        {
                            if let Some(f) = &release {
                                f(&DismissReason::Drag, window, cx);
                            }
                        }
                    });
                },
            )
            .absolute()
            .inset_0();
            overlay = overlay.child(global_drag);
        }
        // `.drawer__backdrop` (its `--opaque`/`--blur`/`--transparent`
        // variants are the Backdrop enum) is a bare scrim — dimmed but
        // press-less, exactly like the modal's: the panel's `on_mouse_down_out`
        // dismisses on a press outside the sheet, and a scrim with its own
        // click listener would only double-report. v3 fades it in alongside
        // the panel.
        let scrim = gpui::div()
            .id(element_id::scoped(&self.id, "backdrop"))
            .absolute()
            .inset_0()
            .bg(backdrop_bg);
        overlay = overlay.child(if exiting {
            crate::anim::exiting(
                scrim,
                "drawer-backdrop-out",
                crate::anim::ZoomBox::default(),
                crate::anim::Motion::DRAWER_BACKDROP_OUT,
                cx,
            )
        } else {
            crate::anim::entering(
                scrim,
                "drawer-backdrop-anim",
                crate::anim::Motion::DRAWER_BACKDROP_IN,
                cx,
            )
        });
        // Drawers slide in from the edge they are anchored to.
        let edge = match self.placement {
            DrawerPlacement::Left => crate::anim::Edge::Left,
            DrawerPlacement::Right => crate::anim::Edge::Right,
            DrawerPlacement::Top => crate::anim::Edge::Top,
            DrawerPlacement::Bottom => crate::anim::Edge::Bottom,
        };
        // A pulled panel is being placed by the drag, so it must not also be
        // running its entry animation from the same edge.
        overlay = overlay.child(if exiting {
            crate::anim::exiting_to(
                anchored,
                "drawer-panel-out",
                edge,
                px(dismiss_extent),
                crate::anim::Motion::DRAWER_OUT,
                cx,
            )
        } else {
            crate::anim::entering_from(
                anchored,
                "drawer-panel",
                edge,
                px(dismiss_extent),
                crate::anim::Motion::DRAWER_IN,
                cx,
            )
        });

        overlay = crate::util::apply_sx(overlay, &self.sx);
        crate::util::window_overlay(overlay, window).into_any_element()
    }
}

/// Where a pointer sits along the axis a drawer is dismissed on.
fn drag_axis_position(placement: DrawerPlacement, at: gpui::Point<gpui::Pixels>) -> f32 {
    match placement {
        DrawerPlacement::Left | DrawerPlacement::Right => f32::from(at.x),
        DrawerPlacement::Top | DrawerPlacement::Bottom => f32::from(at.y),
    }
}

crate::util::impl_component_styled!(Drawer);
