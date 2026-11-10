//! AlertDialog — port of `@heroui/alert-dialog` (v3).
//!
//! A modal for critical confirmations. Unlike [`Modal`](crate::modal::Modal) it
//! is not dismissible by clicking the backdrop, it always announces a title and
//! description, and it renders a confirm/cancel action pair unless the caller
//! composes their own `AlertDialog.Footer` children.

use std::sync::Arc;

use gpui::{
    div, prelude::*, px, AnyElement, App, ClickEvent, IntoElement, ParentElement, Pixels,
    RenderOnce, SharedString, Styled, Window,
};
use herogpui_core::{element_id, Backdrop, Color, Size, Variant};
use herogpui_theme::ActiveTheme;

use crate::{
    a11y::{self, A11y as _},
    button::Button,
    icons,
    modal::{ModalPlacement, OnOpenChange},
    util,
};

/// AlertDialog width preset (`size`) — `xs | sm | md | lg | cover`.
///
/// There is no `full` here: a critical confirmation should never fill the
/// viewport edge to edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlertDialogSize {
    /// Extra small width.
    Xs,
    /// Small width.
    Sm,
    /// Medium width.
    #[default]
    Md,
    /// Large width.
    Lg,
    /// Nearly fills the viewport, keeping a margin.
    Cover,
}

impl AlertDialogSize {
    /// Every size, in display order.
    pub const ALL: [AlertDialogSize; 5] = [
        AlertDialogSize::Xs,
        AlertDialogSize::Sm,
        AlertDialogSize::Md,
        AlertDialogSize::Lg,
        AlertDialogSize::Cover,
    ];

    /// `max-w-xs` … `max-w-lg` from `.alert-dialog__dialog--*`, which is
    /// Tailwind's scale: 20rem, 24rem, 28rem, 32rem. `Cover` is `w-full`.
    fn max_width(self) -> Option<Pixels> {
        match self {
            AlertDialogSize::Xs => Some(px(320.)),
            AlertDialogSize::Sm => Some(px(384.)),
            AlertDialogSize::Md => Some(px(448.)),
            AlertDialogSize::Lg => Some(px(512.)),
            AlertDialogSize::Cover => None,
        }
    }

    /// The display name of this size.
    pub fn label(self) -> &'static str {
        match self {
            AlertDialogSize::Xs => "Xs",
            AlertDialogSize::Sm => "Sm",
            AlertDialogSize::Md => "Md",
            AlertDialogSize::Lg => "Lg",
            AlertDialogSize::Cover => "Cover",
        }
    }
}

type OnAction = Arc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// `AlertDialog.CloseTrigger` — the dialog's close slot, `absolute end-4
/// top-4`.
///
/// v3 spells visibility by composing or omitting the part: a composed
/// [`AlertDialogCloseTrigger`] renders in the slot and an omitted one leaves
/// it bare panel padding — there is no `hideCloseButton` and no automatic
/// stand-in. The part is v3's wired `CloseButton` (`slot="close"`), always
/// wired to `onOpenChange(false)` alone — never to [`AlertDialog::on_cancel`]
/// — and closing regardless of `is_dismissible` or
/// `is_keyboard_dismiss_disabled`, like v3's composed trigger. Custom
/// `children` only replace the button's glyph — the press still runs the
/// dialog's close action. Without an `on_open_change` — or composed outside
/// an [`AlertDialog`] — the part draws nothing.
pub struct AlertDialogCloseTrigger {
    on_dismiss: Option<crate::modal::OnClose>,
    /// The id of the dialog this trigger was pulled out of; see
    /// [`crate::modal::CloseTriggerPart::wire`].
    owner: Option<gpui::ElementId>,
    /// This trigger's index within its dialog; see
    /// [`crate::modal::CloseTriggerPart::wire`].
    slot: usize,
    children: Vec<AnyElement>,
}

impl AlertDialogCloseTrigger {
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

crate::modal::close_trigger_part!(AlertDialogCloseTrigger, "close-trigger");

/// `.alert-dialog__icon--{status}`: the disc's background, the glyph colour
/// and the glyph. `--default` uses the plain `bg-default text-foreground`
/// pair with the info glyph, not a soft mix; the status roles use their soft
/// background and `RoleColor::soft_foreground(..)`, upstream's role/foreground
/// `color-mix` token. The glyphs follow upstream's icon map:
/// info for `default` and `accent`, then success, warning and danger.
fn icon_presentation(
    status: Color,
    colors: &herogpui_theme::ThemeColors,
) -> (gpui::Hsla, gpui::Hsla, &'static str) {
    match status {
        Color::Default => (colors.default.color, colors.foreground, icons::INFO_CIRCLE),
        Color::Accent => (
            colors.accent.soft(),
            colors.accent.soft_foreground(colors.foreground),
            icons::INFO_CIRCLE,
        ),
        Color::Success => (
            colors.success.soft(),
            colors.success.soft_foreground(colors.foreground),
            icons::CHECK_CIRCLE,
        ),
        Color::Warning => (
            colors.warning.soft(),
            colors.warning.soft_foreground(colors.foreground),
            icons::WARNING_TRIANGLE,
        ),
        Color::Danger => (
            colors.danger.soft(),
            colors.danger.soft_foreground(colors.foreground),
            icons::CIRCLE_EXCLAMATION,
        ),
    }
}

/// HeroUI AlertDialog (controlled).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct AlertDialog {
    /// Keys this dialog's own state; see [`AlertDialog::id`].
    id: gpui::ElementId,
    is_open: bool,
    title: SharedString,
    description: Option<SharedString>,
    size: AlertDialogSize,
    backdrop: Backdrop,
    placement: ModalPlacement,
    /// `status` on `AlertDialog.Icon`. `None` renders no icon, matching a v3
    /// dialog that does not compose one.
    status: Option<Color>,
    is_dismissible: bool,
    is_keyboard_dismiss_disabled: bool,
    on_open_change: Option<OnOpenChange>,
    confirm_label: SharedString,
    cancel_label: SharedString,
    /// Composed `AlertDialog.Footer` children. Any composed child retires the
    /// built-in cancel/confirm pair: v3's footer is caller-composed, which is
    /// where a danger or pending confirm is spelled.
    footer: Vec<AnyElement>,
    children: Vec<AnyElement>,
    on_confirm: Option<OnAction>,
    on_cancel: Option<OnAction>,
    /// The panel's corner radius, in place of the owning `container_radius`
    /// helper. The panel's entry zoom interpolates the same value.
    radius: Option<Pixels>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl AlertDialog {
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

    /// Creates a closed alert dialog with the given title.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            id: gpui::ElementId::Name("alert-dialog".into()),
            is_open: false,
            title: title.into(),
            description: None,
            size: AlertDialogSize::default(),
            backdrop: Backdrop::Opaque,
            placement: ModalPlacement::default(),
            status: None,
            // v3 defaults an alert dialog to non-dismissible: the user has to
            // pick one of the two actions.
            is_dismissible: false,
            // v3 defaults this to true for an alert dialog.
            is_keyboard_dismiss_disabled: true,
            on_open_change: None,
            confirm_label: "Confirm".into(),
            cancel_label: "Cancel".into(),
            footer: Vec::new(),
            children: Vec::new(),
            on_confirm: None,
            on_cancel: None,
            radius: None,
            sx: None,
        }
    }

    /// Sets whether the dialog is open (v3 `isOpen`).
    pub fn is_open(mut self, v: bool) -> Self {
        self.is_open = v;
        self
    }

    /// Sets the description text.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Sets the width preset (v3 `size`).
    pub fn size(mut self, size: AlertDialogSize) -> Self {
        self.size = size;
        self
    }

    /// `placement` on `AlertDialog.Container`.
    pub fn placement(mut self, placement: ModalPlacement) -> Self {
        self.placement = placement;
        self
    }

    /// `status` on `AlertDialog.Icon` — shows the status glyph in that colour.
    pub fn status(mut self, status: Color) -> Self {
        self.status = Some(status);
        self
    }

    /// `isKeyboardDismissDisabled` — Escape is disabled by default here, so
    /// pass `false` to allow it.
    pub fn is_keyboard_dismiss_disabled(mut self, v: bool) -> Self {
        self.is_keyboard_dismiss_disabled = v;
        self
    }

    /// `isDismissable` — allows dismissal by clicking the backdrop. The close
    /// slot is not the backdrop: like v3's composed `AlertDialog.CloseTrigger`,
    /// the composed part renders and closes regardless of this flag, reporting
    /// through [`AlertDialog::on_open_change`] alone.
    pub fn is_dismissible(mut self, v: bool) -> Self {
        self.is_dismissible = v;
        self
    }

    /// `onOpenChange` — fires with `false` when the dialog is dismissed.
    pub fn on_open_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_change = Some(Arc::new(f));
        self
    }

    /// Sets the backdrop style.
    pub fn backdrop(mut self, backdrop: Backdrop) -> Self {
        self.backdrop = backdrop;
        self
    }

    /// The panel's corner radius, in place of the owning `container_radius`
    /// helper. The panel's entry zoom interpolates the same value, so the
    /// animation never parts company with the painted shape; the status icon's
    /// `rounded-3xl` tile inside is an inner part and keeps its own. Not a v3
    /// prop; the removed v2 `radius` prop is prohibited and this is a
    /// per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the dialog's root element after every value the size, the
    /// placement and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }

    /// Sets the label of the confirm button.
    pub fn confirm_label(mut self, text: impl Into<SharedString>) -> Self {
        self.confirm_label = text.into();
        self
    }

    /// Sets the label of the cancel button.
    pub fn cancel_label(mut self, text: impl Into<SharedString>) -> Self {
        self.cancel_label = text.into();
        self
    }

    /// Composes `AlertDialog.Footer` — the caller-owned action row.
    ///
    /// v3 has no built-in confirm/cancel pair: the footer buttons are
    /// composed, which is where a danger or pending confirm is spelled
    /// (`variant="danger"`, `is_pending` on the composed `Button`). Any
    /// composed child retires the built-in pair and owns its own close
    /// reporting; repeated calls append in order.
    pub fn footer_child(mut self, el: impl IntoElement) -> Self {
        self.footer.push(el.into_any_element());
        self
    }

    /// The confirm action on the built-in footer pair. The pair renders only
    /// when the caller composes no footer children: a composed
    /// `AlertDialog.Footer` retires it whole and owns its own close
    /// reporting, so `on_confirm` does nothing there — see
    /// [`AlertDialog::footer_child`].
    pub fn on_confirm(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_confirm = Some(Arc::new(handler));
        self
    }

    /// The cancel action on the built-in footer pair. The pair renders only
    /// when the caller composes no footer children, and a close trigger is
    /// never a cancel: `on_cancel` fires from the built-in cancel button
    /// alone — see [`AlertDialog::footer_child`].
    pub fn on_cancel(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_cancel = Some(Arc::new(handler));
        self
    }
}

impl ParentElement for AlertDialog {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for AlertDialog {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // v3 keeps a closing panel on screen for its `[data-exiting]` run.
        let (phase, dismissal_token) = util::overlay_scope(
            window,
            cx,
            crate::modal::dialog_key(&self.id, "phase"),
            self.is_open,
            true,
        );
        if phase == util::OverlayPhase::Closed {
            // Every close path lands here, including a caller flipping
            // `is_open`, so the focus goes back from one place.
            crate::modal::release_dialog_focus(&self.id, window, cx);
            return div().into_any_element();
        }
        let exiting = phase == util::OverlayPhase::Exiting;

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

        let colors = cx.colors();
        let layout = cx.layout();

        // Every close slot reports through `onOpenChange(false)` alone — never
        // through `onCancel`. v3's `slot="close"` buttons carry RAC's
        // `state.close()` as the slot's own `onPress`, and a consumer handler
        // chains *after* it: `AlertDialog.CloseTrigger` (the composed part in
        // the close slot), the `ModalOverlay`'s outside-press dismissal and
        // its Escape dismissal are all plain closes with no action of their
        // own. `isDismissable` only gates the scrim.
        let close_action: Option<crate::modal::OnClose> =
            self.on_open_change
                .clone()
                .map(|open_change| -> crate::modal::OnClose {
                    util::shared(
                        move |_: &crate::modal::DismissReason,
                              window: &mut Window,
                              cx: &mut App| {
                            open_change(&false, window, cx);
                        },
                    )
                });

        // v3 composes the close trigger as a child part. Pull every composed
        // `AlertDialogCloseTrigger` out of the dialog's children and the
        // footer row — so neither slot swallows it — and hand each the close
        // path above to wire the default `CloseButton` with.
        let composed_footer = !self.footer.is_empty();
        let mut close_triggers = crate::modal::take_close_triggers::<AlertDialogCloseTrigger>(
            &mut self.children,
            close_action.clone(),
            &self.id,
            0,
        );
        close_triggers.extend(
            crate::modal::take_close_triggers::<AlertDialogCloseTrigger>(
                &mut self.footer,
                close_action.clone(),
                &self.id,
                close_triggers.len(),
            ),
        );

        // `.alert-dialog__container` is `p-4 sm:p-10`, so its content box —
        // the box v3's `max-h-full` dialog resolves against — is the viewport
        // less 80px. Absolute pixels because gpui resolves a percentage
        // max height against the parent *content box*, which is exactly the
        // budget this number names.
        let panel_max_h = window.viewport_size().height - px(80.);
        // The dialog's own `p-6` takes 48 of that before the body sees any.
        let body_max_h = panel_max_h - px(48.);

        // The panel's corner, resolved once: the zoom below has to interpolate
        // the same value the panel paints.
        let radius = self.radius.unwrap_or_else(|| util::container_radius(cx));

        // `.alert-dialog__dialog` has no gap: the spacing between the header,
        // the body and the footer comes from v3's `+` rules (mt-2, mt-5), so
        // each part carries its own top margin instead.
        let mut panel = div()
            // `alert-dialog/alert-dialog.js` is the one dialog in v3 that
            // names its own role: it passes `role: "alertdialog"` to the RAC
            // `Dialog`, and `react-aria/.../dialog/useDialog.js` passes that
            // through. That role is also what makes the body the dialog's
            // description upstream — `useDialog` points `aria-describedby` at
            // the content id only `when role === 'alertdialog'` — so the
            // description text joins the name here rather than being a node.
            .id(element_id::scoped(&self.id, "dialog"))
            .a11y_named(
                a11y::Role::AlertDialog,
                &a11y::Name::labelled(self.title.clone()).described(self.description.clone()),
            )
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .when_some(self.size.max_width(), |e, w| e.max_w(w))
            .max_h(panel_max_h)
            // `.alert-dialog__dialog` is `overflow-clip`: a long body scrolls
            // inside the body slot below instead of pushing the footer out of
            // the container. gpui 0.2.2 has no `clip` overflow; `hidden` is
            // its clip equivalent here.
            .overflow_hidden()
            // `--cover` is `h-full min-h-full w-full`: the panel fills the
            // container's content box outright instead of hugging its content.
            .when(self.size == AlertDialogSize::Cover, |e| {
                e.h(panel_max_h).min_h(panel_max_h)
            })
            .p(px(24.))
            .rounded(radius)
            .bg(colors.overlay.background)
            // v3 gives a floating panel no border; dark mode's inset hairline is
            // what separates it from the page.
            .when_some(layout.overlay_hairline, |el, hairline| {
                el.border(layout.border_width).border_color(hairline)
            })
            .text_color(colors.overlay.foreground)
            .when(!layout.overlay_shadow.is_empty(), |e| {
                e.shadow(layout.overlay_shadow.clone())
            })
            .child({
                // `.alert-dialog__header` is `flex flex-col gap-3`, and the icon
                // is a *child* of it: `size-10 rounded-3xl` above the heading,
                // not a disc floating in the corner.
                let mut header = div().flex().flex_col().gap(px(12.));
                if let Some(status) = self.status {
                    let (bg, fg, glyph) = icon_presentation(status, colors);
                    header = header.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_shrink_0()
                            .size(px(40.))
                            .rounded(util::control_radius(cx))
                            .bg(bg)
                            .child(
                                gpui::svg()
                                    .size(px(20.))
                                    .path(glyph)
                                    // svg() never inherits text colour.
                                    .text_color(fg),
                            ),
                    );
                }
                header.child(
                    div()
                        .text_size(px(16.))
                        .line_height(px(24.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(self.title.to_string()),
                )
            });

        // `.alert-dialog__body` is one scrolling slot that holds the
        // description and any composed children: `min-h-0 flex-1 scrollbar`,
        // `text-sm leading-[1.43] text-muted`, `mt-2` after the header, and
        // the `-m-[3px] my-0 p-[3px]` that lets the text run 3px under the
        // browser's scrollbar. gpui reserves gutter space only through
        // `scrollbar_width` and paints none on a plain div, so the
        // compensation translates to the margins and padding alone. The
        // budget is the panel cap minus the dialog's `p-6`; a scroll
        // container in an auto-height flex column measures as zero (the
        // modal learned this the hard way), so this is a *max* height and
        // the header and the footer claim their own space first.
        let has_description = self.description.is_some();
        if has_description || !self.children.is_empty() {
            let mut body = div()
                .id(element_id::scoped(&self.id, "body"))
                .mt(px(8.))
                .mx(px(-3.))
                .p(px(3.))
                .w_full()
                .min_h_0()
                .max_h(body_max_h)
                .overflow_y_scroll()
                // v3 spells the body `flex-1`: inside a `--cover` dialog's
                // fixed height it stretches and pins the footer to the bottom
                // edge. In a content-sized panel that spelling measures the
                // scroller as zero (the modal's lesson), so it only applies
                // when the panel has a definite height.
                .when(self.size == AlertDialogSize::Cover, |e| e.flex_1())
                .text_size(px(14.))
                .line_height(px(20.))
                .text_color(colors.muted);
            if let Some(description) = &self.description {
                body = body.child(description.to_string());
            }
            if !self.children.is_empty() {
                body = body.child(
                    div()
                        .when(has_description, |e| e.mt(px(8.)))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .children(self.children),
                );
            }
            panel = panel.child(body);
        }

        // Cancel first, confirm last — the destructive action is furthest from
        // the reading position. `composed_footer` was measured before the
        // trigger pull: a footer that held only a close trigger still retires
        // the built-in pair, but the pull also leaves nothing to render in
        // the row, and an empty `mt-5` row draws nothing but a phantom 20px
        // gap — so the row is spelled only when it has content.
        let mut actions = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .gap(px(8.))
            // `+ .alert-dialog__footer` is `mt-5`.
            .mt(px(20.));
        if !composed_footer {
            // The built-in footer pair stands in for v3's composed
            // `Button slot="close"` compositions, and is retired whole the
            // moment the caller composes their own footer. RAC chains the
            // slot's own `onPress` — `state.close()`, the owner's
            // `onOpenChange(false)` — *before* a consumer `onPress`, so the
            // close is reported first and the action callback runs after.
            // The dialog is controlled: the owner, not the button, decides
            // the next render.
            let cancel_action = match (self.on_open_change.clone(), self.on_cancel.clone()) {
                (None, None) => None,
                (open_change, action) => Some(util::shared(
                    move |ev: &ClickEvent, window: &mut Window, cx: &mut App| {
                        if let Some(f) = &open_change {
                            f(&false, window, cx);
                        }
                        if let Some(f) = &action {
                            f(ev, window, cx);
                        }
                    },
                )),
            };
            let confirm_action = match (self.on_open_change.clone(), self.on_confirm.clone()) {
                (None, None) => None,
                (open_change, action) => Some(util::shared(
                    move |ev: &ClickEvent, window: &mut Window, cx: &mut App| {
                        if let Some(f) = &open_change {
                            f(&false, window, cx);
                        }
                        if let Some(f) = &action {
                            f(ev, window, cx);
                        }
                    },
                )),
            };

            let mut cancel = Button::new("alert-dialog-cancel")
                .label(self.cancel_label.clone())
                .variant(Variant::Tertiary)
                .size(Size::Md);
            if let Some(action) = cancel_action {
                cancel = cancel.on_press(move |ev, window, cx| action(ev, window, cx));
            }

            let mut confirm = Button::new("alert-dialog-confirm")
                .label(self.confirm_label.clone())
                .variant(Variant::Primary)
                .size(Size::Md);
            if let Some(action) = confirm_action {
                confirm = confirm.on_press(move |ev, window, cx| action(ev, window, cx));
            }

            actions = actions.child(cancel).child(confirm);
            panel = panel.child(actions);
        } else if !self.footer.is_empty() {
            panel = panel.child(actions.children(self.footer));
        }

        let backdrop_bg = match self.backdrop {
            Backdrop::Opaque => colors.backdrop,
            Backdrop::Blur => colors.backdrop.alpha(colors.backdrop.a * 0.6),
            Backdrop::Transparent => gpui::transparent_black(),
        };

        // `.alert-dialog__close-trigger` is `absolute end-4 top-4`. v3 renders
        // a close affordance only where the caller composes the part; an
        // omitted trigger leaves the spot bare panel padding.
        for trigger in close_triggers {
            panel = panel.child(div().absolute().top(px(16.)).right(px(16.)).child(trigger));
        }

        // Backdrop dismissal lives on the **panel**, exactly as in the modal
        // and the drawer: gpui has no hitbox occlusion, so a click on the
        // full-window backdrop would fire for a press on this panel too.
        // `on_mouse_down_out` reads the panel's own bounds instead, so it
        // only fires for a press on the dimmed region around the panel.
        // `is_dismissible` is the whole gate — the close slot above is not —
        // and the exit phase gets none: the dialog is already closing.
        let dismiss: Option<crate::modal::OnClose> = if self.is_dismissible {
            close_action.clone()
        } else {
            None
        };
        let panel = match (dismiss, exiting) {
            (Some(on_dismiss), false) => util::dismiss_on_press_outside_with_token(
                panel,
                dismissal_token.clone(),
                move |window, cx| {
                    on_dismiss(&crate::modal::DismissReason::Backdrop, window, cx);
                    util::DismissResult::Handled
                },
            ),
            _ => panel,
        };

        // Escape is the `ModalOverlay`'s own dismissal, so it is a plain
        // close too: `onOpenChange(false)`, never `onCancel`.
        let keyboard_dismiss: Option<crate::modal::OnClose> = if self.is_keyboard_dismiss_disabled {
            None
        } else {
            close_action.clone()
        };

        // `.alert-dialog__backdrop`, whose variants are the `Backdrop` enum.
        // A bare scrim, like the modal's and the drawer's: the panel's
        // `on_mouse_down_out` owns backdrop dismissal, and a second listener
        // here would only double-report.
        let backdrop = div()
            .id(element_id::scoped(&self.id, "backdrop"))
            .absolute()
            .inset_0()
            .bg(backdrop_bg);
        // v3 fades the backdrop in alongside the panel, and out with it.
        let backdrop = if exiting {
            crate::anim::exiting(
                backdrop,
                "alert-dialog-backdrop-out",
                crate::anim::ZoomBox::default(),
                crate::anim::Motion::BACKDROP_OUT,
                cx,
            )
        } else {
            crate::anim::entering(
                backdrop,
                "alert-dialog-backdrop-anim",
                crate::anim::Motion::BACKDROP_IN,
                cx,
            )
        };

        // `Tab` cycles the dialog's own controls; see `util::trap_tab`.
        let mut overlay = util::trap_tab(
            div()
                .id(element_id::scoped(&self.id, "root"))
                .absolute()
                .inset_0()
                .flex()
                // `.alert-dialog__container` is `p-4 sm:p-10`.
                .p(px(40.))
                .track_focus(&focus_handle),
            &focus_handle,
        )
        .when(
            matches!(
                self.placement,
                ModalPlacement::Center | ModalPlacement::Auto
            ),
            |e| e.items_center().justify_center(),
        )
        .when(self.placement == ModalPlacement::Top, |e| {
            e.items_start().justify_center()
        })
        .when(self.placement == ModalPlacement::Bottom, |e| {
            e.items_end().justify_center()
        })
        .child(backdrop)
        .child({
            let mut zoom = crate::anim::ZoomBox::panel(px(24.), radius).padding_x(px(24.));
            // The zoom scales a known box; `Cover` has no width of its own and
            // the `ZoomBox` carries no height, so there is nothing to hand it —
            // its enter/exit zoom rides the padding, radius and fade alone.
            if let Some(w) = self.size.max_width() {
                zoom = zoom.sized(w);
            }
            let (slide_x, slide_y) = crate::modal::placement_entry_offset(self.placement);
            zoom.slide_x = (slide_x != 0.0).then(|| px(slide_x));
            zoom.slide_y = (slide_y != 0.0).then(|| px(slide_y));
            if exiting {
                crate::anim::exiting(
                    panel,
                    "alert-dialog-panel-out",
                    zoom,
                    crate::anim::Motion::PANEL_OUT,
                    cx,
                )
            } else {
                crate::anim::entering_zoom(
                    panel,
                    "alert-dialog-panel",
                    zoom,
                    crate::anim::Motion::PANEL_IN,
                    cx,
                )
            }
        });
        if let Some(on_escape) = keyboard_dismiss {
            overlay =
                util::dismiss_on_escape_with_token(overlay, dismissal_token, move |window, cx| {
                    on_escape(&crate::modal::DismissReason::Escape, window, cx);
                    util::DismissResult::Handled
                });
        }
        overlay = util::apply_sx(overlay, &self.sx);
        util::window_overlay(overlay, window).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Hsla` carries no `PartialEq`; compare channel-wise.
    fn assert_same_color(a: gpui::Hsla, b: gpui::Hsla) {
        assert!((a.h - b.h).abs() < 1e-4, "{a:?} != {b:?}");
        assert!((a.s - b.s).abs() < 1e-4, "{a:?} != {b:?}");
        assert!((a.l - b.l).abs() < 1e-4, "{a:?} != {b:?}");
        assert!((a.a - b.a).abs() < 1e-4, "{a:?} != {b:?}");
    }

    #[test]
    fn icon_presentation_maps_every_status() {
        let colors = herogpui_theme::ThemeColors::light();
        // One row per status, with upstream's icon map: `AlertDialog.Icon`
        // picks info for `default` and `accent`, then the success, warning
        // and danger glyphs.
        let expected = [
            (Color::Default, icons::INFO_CIRCLE),
            (Color::Accent, icons::INFO_CIRCLE),
            (Color::Success, icons::CHECK_CIRCLE),
            (Color::Warning, icons::WARNING_TRIANGLE),
            (Color::Danger, icons::CIRCLE_EXCLAMATION),
        ];
        for (status, glyph) in expected {
            let (bg, fg, actual) = icon_presentation(status, &colors);
            assert_eq!(actual, glyph, "{status:?} must use the upstream glyph");
            let role = match status {
                Color::Default => {
                    // `.alert-dialog__icon--default` is `bg-default
                    // text-foreground` with the info glyph — not
                    // `--default-soft`, not a role colour.
                    assert_same_color(bg, colors.default.color);
                    assert_same_color(fg, colors.foreground);
                    continue;
                }
                Color::Accent => &colors.accent,
                Color::Success => &colors.success,
                Color::Warning => &colors.warning,
                Color::Danger => &colors.danger,
            };
            assert_same_color(bg, role.soft());
            assert_same_color(fg, role.soft_foreground(colors.foreground));
        }
    }
}

crate::util::impl_component_styled!(AlertDialog);
