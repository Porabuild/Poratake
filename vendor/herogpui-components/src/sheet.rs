//! Sheet — HeroUI v3.2.7's edge overlay with partial snap positions.
//!
//! Native pointer gestures replace React Aria's DOM scroll-snap primitive.
//! Snap lengths use pixels or a fraction of the panel; arbitrary CSS lengths
//! and the perspective transform of a covered parent are not available here.

use std::{cell::Cell, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    prelude::*, px, AnyElement, App, Bounds, ElementId, IntoElement, RenderOnce, SharedString,
    Window,
};
use herogpui_core::{element_id, Backdrop};
use herogpui_theme::ActiveTheme;
use web_time::Instant;

use crate::{
    a11y::{self, A11y as _},
    button::Button,
    close_button::CloseButton,
    drawer::DrawerPlacement,
    modal::{DismissReason, OnClose, OnOpenChange},
    util,
};

/// A partial resting position. Full expansion is always implicit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SheetSnapPoint {
    /// Visible pixels, independent of the panel size.
    Pixels(f32),
    /// Fraction of the full panel extent (`0.5` is half).
    Fraction(f32),
}

impl From<f32> for SheetSnapPoint {
    fn from(value: f32) -> Self {
        Self::Pixels(value)
    }
}

/// State passed to [`Sheet::content`].
#[derive(Clone, Copy, Debug)]
pub struct SheetState {
    /// Whether the sheet is fully expanded and its body can scroll.
    pub is_expanded: bool,
    /// Visible extent along the placement axis in logical pixels.
    pub position: f32,
}

const SHEET_RELEASE_MS: u64 = 500;

type Content = Arc<dyn Fn(SheetState) -> AnyElement>;
type AnimationEnd = Arc<dyn Fn(&bool, &mut Window, &mut App)>;

#[derive(Clone, Copy)]
struct Drag {
    start: f32,
    rest: f32,
    last: f32,
    last_at: Instant,
    velocity: f32,
}

#[derive(Clone)]
struct Position {
    rest: f32,
    points: Vec<f32>,
    drag: Option<Drag>,
    was_open: bool,
    generation: usize,
    from: f32,
    started_at: Instant,
    painted: Rc<Cell<f32>>,
}

/// A modal sheet. Compose a trigger for uncontrolled state, or drive `is_open`.
#[must_use = "a component does nothing until it is rendered"]
#[derive(IntoElement)]
pub struct Sheet {
    id: ElementId,
    is_open: Option<bool>,
    default_open: bool,
    placement: DrawerPlacement,
    is_dismissible: bool,
    is_detached: bool,
    backdrop: Backdrop,
    snap_points: Vec<SheetSnapPoint>,
    title: Option<SharedString>,
    aria_label: Option<SharedString>,
    role: a11y::Role,
    trigger: Option<Button>,
    close_trigger: Option<CloseButton>,
    handle: Option<AnyElement>,
    body: Vec<AnyElement>,
    footer: Vec<AnyElement>,
    close_children: Vec<Button>,
    nested: Vec<Sheet>,
    is_nested: bool,
    content: Option<Content>,
    on_open_change: Option<OnOpenChange>,
    on_close: Option<OnClose>,
    on_animation_end: Option<AnimationEnd>,
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Sheet {
    /// Creates a closed sheet with an instance-specific state key.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            is_open: None,
            default_open: false,
            placement: DrawerPlacement::Bottom,
            is_dismissible: true,
            is_detached: false,
            backdrop: Backdrop::Opaque,
            snap_points: Vec::new(),
            title: None,
            aria_label: None,
            role: a11y::Role::Dialog,
            trigger: None,
            close_trigger: None,
            handle: None,
            body: Vec::new(),
            footer: Vec::new(),
            close_children: Vec::new(),
            nested: Vec::new(),
            is_nested: false,
            content: None,
            on_open_change: None,
            on_close: None,
            on_animation_end: None,
            sx: None,
        }
    }

    /// Controlled visibility. Requests are reported through `on_open_change`.
    pub fn is_open(mut self, open: bool) -> Self {
        self.is_open = Some(open);
        self
    }
    /// Initial uncontrolled visibility.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }
    /// Physical edge; horizontal placement is independent of locale direction.
    pub fn placement(mut self, placement: DrawerPlacement) -> Self {
        self.placement = placement;
        self
    }
    /// Gate outside presses, Escape and swipe dismissal. Close controls still work.
    pub fn is_dismissible(mut self, enabled: bool) -> Self {
        self.is_dismissible = enabled;
        self
    }
    /// Add an 8px margin and round all panel corners.
    pub fn is_detached(mut self, detached: bool) -> Self {
        self.is_detached = detached;
        self
    }
    /// Backdrop appearance. Blur uses the native overlay's lighter scrim fallback.
    pub fn backdrop(mut self, backdrop: Backdrop) -> Self {
        self.backdrop = backdrop;
        self
    }
    /// Partial resting positions; numbers mean pixels. A trailing pixel `1`
    /// or fraction `1.0` is ignored, matching upstream's legacy full-size sentinel.
    pub fn snap_points(
        mut self,
        points: impl IntoIterator<Item = impl Into<SheetSnapPoint>>,
    ) -> Self {
        self.snap_points = points.into_iter().map(Into::into).collect();
        self
    }
    /// Dialog heading.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }
    /// Accessible name when no visible heading is supplied.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }
    /// Dialog role (`Dialog` or `AlertDialog`).
    pub fn role(mut self, role: a11y::Role) -> Self {
        self.role = if role == a11y::Role::AlertDialog {
            role
        } else {
            a11y::Role::Dialog
        };
        self
    }
    /// One pressable trigger. Its own press action runs before opening.
    pub fn trigger(mut self, trigger: Button) -> Self {
        self.trigger = Some(trigger);
        self
    }
    /// Explicit close control; its existing on_press action is preserved.
    pub fn close_trigger(mut self, trigger: CloseButton) -> Self {
        self.close_trigger = Some(trigger);
        self
    }
    /// Custom decorative grip. The enclosing handle remains draggable.
    pub fn handle(mut self, handle: impl IntoElement) -> Self {
        self.handle = Some(handle.into_any_element());
        self
    }
    /// Append a footer action.
    pub fn footer_child(mut self, child: impl IntoElement) -> Self {
        self.footer.push(child.into_any_element());
        self
    }
    /// Append a footer action that closes the sheet after its own press handler.
    pub fn close_child(mut self, child: Button) -> Self {
        self.close_children.push(child);
        self
    }
    /// Compose a nested sheet. Only the outermost sheet paints a page scrim.
    pub fn nested_child(mut self, mut sheet: Sheet) -> Self {
        sheet.is_nested = true;
        self.nested.push(sheet);
        self
    }
    /// Content render function, receiving the current expansion state.
    pub fn content(mut self, content: impl Fn(SheetState) -> AnyElement + 'static) -> Self {
        self.content = Some(Arc::new(content));
        self
    }
    /// Every requested open state change.
    pub fn on_open_change(
        mut self,
        callback: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Arc::new(callback));
        self
    }
    /// Called when closing is requested, alongside `on_open_change(false)`.
    pub fn on_close(
        mut self,
        callback: impl Fn(&DismissReason, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Arc::new(callback));
        self
    }
    /// Called after an uninterrupted enter or exit transition finishes.
    pub fn on_animation_end(
        mut self,
        callback: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_animation_end = Some(Arc::new(callback));
        self
    }
    /// Caller-owned panel styling, applied after the stock sheet styles.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }
}

impl ParentElement for Sheet {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.body.extend(elements);
    }
}

fn axis(placement: DrawerPlacement, point: gpui::Point<gpui::Pixels>) -> f32 {
    match placement {
        DrawerPlacement::Top | DrawerPlacement::Bottom => point.y.into(),
        _ => point.x.into(),
    }
}

fn direction(placement: DrawerPlacement) -> f32 {
    match placement {
        DrawerPlacement::Bottom | DrawerPlacement::Right => -1.,
        _ => 1.,
    }
}

fn snap_value(point: &SheetSnapPoint, extent: f32) -> f32 {
    match point {
        SheetSnapPoint::Pixels(value) => *value,
        SheetSnapPoint::Fraction(value) => value * extent,
    }
}

fn snaps(points: &[SheetSnapPoint], extent: f32) -> Vec<f32> {
    let mut result = Vec::new();
    for point in points {
        let value = snap_value(point, extent);
        if value.is_finite() && value > 0. && value < extent && !result.contains(&value) {
            result.push(value);
        }
    }
    result.push(extent);
    result
}

fn release_target(points: &[f32], current: f32, velocity: f32, dismissible: bool) -> f32 {
    let first = points.iter().copied().fold(f32::INFINITY, f32::min);
    if dismissible && current < first && (current < first * 0.75 || velocity < -0.4) {
        return 0.;
    }
    if velocity.abs() > 0.4 {
        let next = points.iter().copied().filter(|point| {
            if velocity > 0. {
                *point > current + 0.5
            } else {
                *point < current - 0.5
            }
        });
        if let Some(next) = if velocity > 0. {
            next.min_by(f32::total_cmp)
        } else {
            next.max_by(f32::total_cmp)
        } {
            return next;
        }
    }
    points
        .iter()
        .copied()
        .min_by(|a, b| (a - current).abs().total_cmp(&(b - current).abs()))
        .unwrap_or(current)
}

impl RenderOnce for Sheet {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (open, own) = util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "open"),
            self.is_open,
            self.default_open,
        );
        // Normalize the legacy final full-size sentinel before any layout decision.
        if matches!(
            self.snap_points.last(),
            Some(SheetSnapPoint::Pixels(1.) | SheetSnapPoint::Fraction(1.))
        ) {
            self.snap_points.pop();
        }
        self.snap_points.retain(|point| match point {
            SheetSnapPoint::Pixels(value) => value.is_finite() && *value > 0.,
            SheetSnapPoint::Fraction(value) => value.is_finite() && *value > 0.,
        });
        let change = self.on_open_change.clone();
        let close = self.on_close.clone();
        let held = own.clone();
        let dismiss: OnClose = util::shared(move |reason, window, cx| {
            if let Some(held) = &held {
                held.update(cx, |open, cx| {
                    *open = false;
                    cx.notify();
                });
            }
            if let Some(close) = &close {
                close(reason, window, cx);
            }
            if let Some(change) = &change {
                change(&false, window, cx);
            }
        });
        let mut root = gpui::div();
        if let Some(trigger) = self.trigger.take() {
            let change = self.on_open_change.clone();
            root = root.child(trigger.then_press(move |_, window, cx| {
                if let Some(own) = &own {
                    own.update(cx, |open, cx| {
                        *open = true;
                        cx.notify();
                    });
                }
                if let Some(change) = &change {
                    change(&true, window, cx);
                }
            }));
        }
        let reduce = ActiveTheme::reduce_motion(cx);
        let (phase, token) = util::overlay_scope_with_exit(
            window,
            cx,
            element_id::scoped(&self.id, "phase"),
            open,
            !reduce,
            SHEET_RELEASE_MS,
        );
        let viewport = window.viewport_size();
        let vertical = matches!(
            self.placement,
            DrawerPlacement::Top | DrawerPlacement::Bottom
        );
        let available = f32::from(if vertical {
            viewport.height
        } else {
            viewport.width
        });
        let measured =
            window.use_keyed_state(element_id::scoped(&self.id, "bounds"), cx, |_, _| {
                Bounds::<gpui::Pixels>::default()
            });
        let measured_extent = f32::from(if vertical {
            measured.read(cx).size.height
        } else {
            measured.read(cx).size.width
        });
        let full = if !self.snap_points.is_empty() {
            available - if self.is_detached { 16. } else { 0. }
        } else if vertical {
            if measured_extent > 0. {
                measured_extent.min(available - 32.)
            } else {
                available - 32.
            }
        } else {
            384_f32.min(available * 0.96)
        };
        let full = full.max(0.);
        let points = snaps(&self.snap_points, full);
        let initial = self
            .snap_points
            .first()
            .map_or(full, |point| snap_value(point, full).min(full));
        // The transition clock is the background executor's, matching the
        // completion timer below (and gpui's own `Animation` since 0.3.11):
        // one clock, so the painted interpolation and `on_animation_end`
        // cannot disagree, and tests step both deterministically.
        let now = cx.background_executor().now();
        let state = window.use_keyed_state(element_id::scoped(&self.id, "position"), cx, |_, _| {
            Position {
                rest: initial,
                points: points.clone(),
                drag: None,
                was_open: false,
                generation: 0,
                from: initial,
                started_at: now,
                painted: Rc::new(Cell::new(initial)),
            }
        });
        if open && !self.snap_points.is_empty() && state.read(cx).points != points {
            state.update(cx, |state, _| {
                let old_full = state.points.last().copied().unwrap_or(full);
                let rest = state.drag.take().map_or(state.rest, |drag| drag.rest);
                let index = state
                    .points
                    .iter()
                    .position(|point| (*point - rest).abs() < 0.5);
                let next = if (rest - old_full).abs() < 0.5 {
                    full
                } else {
                    index
                        .and_then(|index| points.get(index).copied())
                        .unwrap_or(initial)
                };
                state.rest = next;
                state.from = next;
                state.painted.set(next);
                state.started_at = now;
                state.generation = state.generation.wrapping_add(1);
                state.points = points.clone();
            });
        }
        let changed = state.read(cx).was_open != open;
        if changed {
            state.update(cx, |state, _| {
                state.was_open = open;
                state.generation = state.generation.wrapping_add(1);
                state.started_at = now;
                state.drag = None;
                if open {
                    state.rest = initial;
                    state.from = 0.;
                    state.painted.set(0.);
                } else {
                    state.from = state.painted.get();
                    state.rest = 0.;
                }
            });
            if let Some(callback) = self.on_animation_end {
                let generation = state.read(cx).generation;
                let held = state.downgrade();
                window
                    .spawn(cx, async move |cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(if reduce {
                                0
                            } else {
                                SHEET_RELEASE_MS
                            }))
                            .await;
                        let _ = cx.update(|window, cx| {
                            if held
                                .upgrade()
                                .is_some_and(|state| state.read(cx).generation == generation)
                            {
                                callback(&open, window, cx);
                            }
                        });
                    })
                    .detach();
            }
        }
        if !open {
            crate::modal::release_dialog_focus(&self.id, window, cx);
        }
        if phase == util::OverlayPhase::Closed {
            return root.into_any_element();
        }
        // Vanilla GPUI cannot paint arbitrary children without registering their
        // input/focus handlers. Keep the closing surface at its measured size,
        // but remove caller content as soon as the sheet closes.
        if !open {
            self.body.clear();
            self.footer.clear();
            self.close_children.clear();
            self.close_trigger = None;
            self.handle = None;
            self.content = None;
        }
        let focus = window
            .use_keyed_state(element_id::scoped(&self.id, "focus"), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();
        if open {
            crate::modal::claim_dialog_focus(&self.id, &focus, window, cx);
        }
        let current = state.read(cx).clone();
        let target = if self.snap_points.is_empty() && open && current.drag.is_none() {
            full
        } else {
            current.rest.min(full)
        };
        // Interpolate the inset without changing the descendant element-id path.
        let elapsed = (cx.background_executor().now() - current.started_at).as_secs_f32() * 1000.
            / SHEET_RELEASE_MS as f32;
        let visible = if current.drag.is_none() && !reduce && elapsed < 1. {
            let next =
                current.from + (target - current.from) * crate::anim::Curve::OutFluid.at(elapsed);
            window.request_animation_frame();
            next
        } else {
            target
        };
        current.painted.set(visible);
        let expanded = visible >= full - 0.5 && current.drag.is_none();
        let colors = cx.colors();
        let radius = cx.layout().radius_2xl();
        // `.sheet__handle` and `.sheet__handle-bar`: decorative grip and pointer surface.
        let mut handle = gpui::div()
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .py(px(12.))
            .debug_selector(|| "sheet-handle".into())
            .child(self.handle.take().unwrap_or_else(|| {
                gpui::div()
                    .h(px(4.8))
                    .w(px(32.))
                    .rounded_full()
                    .bg(colors.default.color)
                    .into_any_element()
            }));
        if open {
            let held = state.clone();
            let placement = self.placement;
            handle = handle.on_mouse_down(gpui::MouseButton::Left, move |event, _, cx| {
                let at = axis(placement, event.position);
                held.update(cx, |state, cx| {
                    state.rest = state.painted.get();
                    state.generation = state.generation.wrapping_add(1);
                    state.drag = Some(Drag {
                        start: at,
                        rest: state.rest,
                        last: at,
                        last_at: Instant::now(),
                        velocity: 0.,
                    });
                    cx.notify();
                });
            });
        }
        let mut handle = Some(handle);
        // `.sheet__dialog` and placement variants round the edge facing the page.
        // `.sheet__content` / --bottom / --top / --left / --right own the physical edge.
        // Detached content rounds every corner; nesting keeps the ancestor scrim.
        let measured_for_panel = measured;
        let mut panel = gpui::div()
            .id(element_id::scoped(&self.id, "dialog"))
            .a11y_named(
                self.role,
                &a11y::Name::maybe(self.aria_label.or(self.title.clone())),
            )
            .relative()
            .flex()
            .flex_col()
            .min_h(px(0.))
            .bg(colors.overlay.background)
            .text_color(colors.overlay.foreground)
            // Dark mode's `shadow-overlay` is the inset hairline alone.
            .when_some(cx.layout().overlay_hairline, |panel, hairline| {
                panel.border(cx.layout().border_width).border_color(hairline)
            })
            .shadow(cx.layout().overlay_shadow.clone())
            .debug_selector(|| "sheet-dialog".into())
            .when(self.is_detached, |panel| panel.rounded(radius))
            .when(
                !self.is_detached && self.placement == DrawerPlacement::Bottom,
                |panel| panel.rounded_tl(radius).rounded_tr(radius),
            )
            .when(
                !self.is_detached && self.placement == DrawerPlacement::Top,
                |panel| panel.rounded_bl(radius).rounded_br(radius),
            )
            .when(
                !self.is_detached && self.placement == DrawerPlacement::Left,
                |panel| panel.rounded_tr(radius).rounded_br(radius),
            )
            .when(
                !self.is_detached && self.placement == DrawerPlacement::Right,
                |panel| panel.rounded_tl(radius).rounded_bl(radius),
            )
            .overflow_hidden()
            .child(
                gpui::canvas(
                    move |bounds, _, cx| {
                        if measured_for_panel.read(cx).size != bounds.size {
                            measured_for_panel.update(cx, |value, cx| {
                                *value = bounds;
                                cx.notify();
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        if self.placement != DrawerPlacement::Top {
            panel = panel.children(handle.take());
        }
        // `.sheet__header` and `.sheet__heading`: padded 18px semibold title.
        if let Some(title) = self.title {
            panel = panel.child(
                gpui::div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .px(px(20.))
                    .pt(px(16.))
                    .pb(px(8.))
                    .text_size(px(18.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(title),
            );
        }
        if let Some(content) = self.content {
            self.body.push(content(SheetState {
                is_expanded: expanded,
                position: visible,
            }));
        }
        // `.sheet__body` scrolls only at the full detent.
        if open {
            self.body
                .extend(self.nested.into_iter().map(IntoElement::into_any_element));
        }
        let mut body = gpui::div()
            .id(element_id::scoped(&self.id, "body"))
            .flex_1()
            .min_h(px(0.))
            .px(px(20.))
            .py(px(8.))
            .children(self.body);
        body = if expanded {
            body.overflow_y_scroll()
        } else {
            body.overflow_hidden()
        };
        panel = panel.child(body);
        // `.sheet__footer`: end-aligned actions with an 8px gap.
        let has_footer = !self.footer.is_empty() || !self.close_children.is_empty();
        let mut footer = gpui::div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_end()
            .gap(px(8.))
            .px(px(20.))
            .pt(px(8.))
            .pb(px(16.))
            .children(self.footer);
        for child in self.close_children {
            let dismiss = dismiss.clone();
            footer = footer.child(child.then_press(move |_, window, cx| {
                dismiss(&DismissReason::CloseButton, window, cx);
            }));
        }
        if has_footer {
            panel = panel.child(footer);
        }
        panel = panel.children(handle);
        // `.sheet__close-trigger` is fixed above the body scroll clip.
        if let Some(trigger) = self.close_trigger {
            let dismiss = dismiss.clone();
            panel = panel.child(gpui::div().absolute().top(px(12.)).right(px(12.)).child(
                trigger.then_press(move |_, window, cx| {
                    dismiss(&DismissReason::CloseButton, window, cx);
                }),
            ));
        }
        let inset = if self.is_detached { 8. } else { 0. };
        let placement = self.placement;
        let offset = visible - full + inset;
        panel = if vertical {
            panel
                .w(viewport.width - px(inset * 2.))
                .max_h(viewport.height - px(if self.is_detached { 16. } else { 32. }))
                .when(!self.snap_points.is_empty() || !open, |panel| {
                    panel.h(px(full)).max_h(px(full))
                })
        } else {
            panel.w(px(full)).h(viewport.height - px(inset * 2.))
        };
        panel = panel.absolute();
        panel = match placement {
            DrawerPlacement::Bottom => panel.bottom(px(offset)).left(px(inset)),
            DrawerPlacement::Top => panel.top(px(offset)).left(px(inset)),
            DrawerPlacement::Left => panel.left(px(offset)).top(px(inset)),
            DrawerPlacement::Right => panel.right(px(offset)).top(px(inset)),
        };
        panel = util::apply_sx(panel, &self.sx).occlude();
        if open && self.is_dismissible {
            let dismiss = dismiss.clone();
            panel = util::dismiss_on_press_outside_with_token(
                panel,
                token.clone(),
                move |window, cx| {
                    dismiss(&DismissReason::Backdrop, window, cx);
                    util::DismissResult::Handled
                },
            );
        }
        let panel = panel.into_any_element();
        // `.sheet__overlay` covers the viewport and contains modal focus only while open.
        let mut overlay = gpui::div().absolute().inset_0();
        if open {
            overlay = util::trap_tab(overlay.track_focus(&focus), &focus);
            if self.is_dismissible {
                let close = dismiss.clone();
                overlay = util::dismiss_on_escape_with_token(overlay, token, move |window, cx| {
                    close(&DismissReason::Escape, window, cx);
                    util::DismissResult::Handled
                });
            }
            let held = state.clone();
            let close = dismiss.clone();
            let targets = points.clone();
            let can_dismiss = self.is_dismissible;
            overlay = overlay.child(
                gpui::canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        let held_move = held.clone();
                        window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                            if phase != gpui::DispatchPhase::Capture {
                                return;
                            }
                            let Some(mut drag) = held_move.read(cx).drag else {
                                return;
                            };
                            if event.pressed_button != Some(gpui::MouseButton::Left) {
                                held_move.update(cx, |state, cx| {
                                    state.from = state.rest;
                                    state.rest = drag.rest;
                                    state.started_at = cx.background_executor().now();
                                    state.generation = state.generation.wrapping_add(1);
                                    state.drag = None;
                                    cx.notify();
                                });
                                return;
                            }
                            let at = axis(placement, event.position);
                            let now = Instant::now();
                            let elapsed = now.duration_since(drag.last_at).as_secs_f32() * 1000.;
                            drag.velocity = if elapsed > 0. {
                                direction(placement) * (at - drag.last) / elapsed
                            } else {
                                0.
                            };
                            drag.last = at;
                            drag.last_at = now;
                            held_move.update(cx, |state, cx| {
                                state.rest = (drag.rest + direction(placement) * (at - drag.start))
                                    .clamp(0., full);
                                state.drag = Some(drag);
                                cx.notify();
                            });
                        });
                        let held_up = held.clone();
                        let targets = targets.clone();
                        let close = close.clone();
                        window.on_mouse_event(
                            move |event: &gpui::MouseUpEvent, phase, window, cx| {
                                if phase != gpui::DispatchPhase::Capture
                                    || event.button != gpui::MouseButton::Left
                                {
                                    return;
                                }
                                let Some(drag) = held_up.read(cx).drag else {
                                    return;
                                };
                                let target = release_target(
                                    &targets,
                                    held_up.read(cx).rest,
                                    drag.velocity,
                                    can_dismiss && held_up.read(cx).rest < drag.rest,
                                );
                                held_up.update(cx, |state, cx| {
                                    state.from = state.rest;
                                    state.started_at = cx.background_executor().now();
                                    state.rest = if target == 0. {
                                        targets.iter().copied().fold(full, f32::min)
                                    } else {
                                        target
                                    };
                                    state.drag = None;
                                    state.generation = state.generation.wrapping_add(1);
                                    cx.notify();
                                });
                                if target == 0. {
                                    close(&DismissReason::Drag, window, cx);
                                }
                            },
                        );
                    },
                )
                .absolute()
                .inset_0(),
            );
        }
        // `.sheet__backdrop`, --opaque / --blur / --transparent; only the outer sheet dims.
        let backdrop_color = if self.is_nested {
            gpui::transparent_black()
        } else {
            let fade_start = if points.len() > 1 {
                points[..points.len() - 1]
                    .iter()
                    .copied()
                    .fold(0_f32, f32::max)
            } else {
                0.
            };
            let fade = ((visible - fade_start) / (full - fade_start).max(1.)).clamp(0., 1.);
            match self.backdrop {
                Backdrop::Opaque => colors.backdrop.alpha(colors.backdrop.a * fade),
                Backdrop::Blur => colors.backdrop.alpha(colors.backdrop.a * 0.6 * fade),
                Backdrop::Transparent => gpui::transparent_black(),
            }
        };
        let scrim = gpui::div().absolute().inset_0().bg(backdrop_color);
        overlay = overlay.child(scrim).child(panel);
        root = root.child(util::window_overlay(overlay, window));
        root.into_any_element()
    }
}

crate::util::impl_component_styled!(Sheet);
