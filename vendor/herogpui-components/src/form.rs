//! Form — port of `@heroui/form` (v3).
//!
//! v3's `Form` is a `<form>`: `name` on each field is what makes a submission
//! carry that field's value, and `onSubmit` receives the collected `FormData`.
//! There is no DOM here, and gpui gives a child no way to find its ancestor, so
//! the form is *told* which fields it owns — `Form::field(..)` — instead of
//! discovering them. Everything downstream of that is the same: names, a
//! collected submission, reset, and an invalid path that runs instead of submit.
//!
//! Submission has two doors that share one implementation
//! (`Form::run_submission`): the caller-wired submit button
//! (`Form::submit_handler`, standing in for `<button type="submit">`) and the
//! native form's implicit submission — Enter pressed in a focused field that
//! semantically participates, which the form root's key handler answers. Only
//! the fields that participate carry the Enter reader; a TextArea's Enter is a
//! newline, and a focused submit button submits through its own click.
//!
//! The second door is a GPUI substitute for the browser's implicit submission,
//! not a port of it. A browser picks the *default submitter* — the first
//! submit button in tree order — and skips implicit submission entirely when a
//! form has no submit button and more than one field blocking validation; both
//! rules read the form's children, which are opaque elements here. A browser
//! also lets a field's own keydown handler cancel the keystroke; the controls
//! that own Enter here stop propagation to the same effect. So Enter in a
//! participating field always runs this form's one submission, whoever else
//! could have been the submitter.
//!
//! A blocked Enter-origin submission defers the focus move to the key's
//! release. gpui activates whichever element the frame drawn for the release
//! shows as focused, so focusing a Select trigger or a Switch mid-keystroke
//! would let the release click it — opening or toggling the very control the
//! repair was meant to reach. The form latches the blocked keystroke in keyed
//! state, disarms the release (`prevent_default` gates gpui's activation
//! listener), and moves the focus once the dispatch is over.
//!
//! Read-only controls are the third gate, after disabled and inert: they stay
//! successful (they submit their value) and focusable, but constraint
//! validation bars them — neither a missing value nor a stored error on a
//! read-only field can block a submission, as a native form has it.
//!
//! `validationErrors` is HeroUI v3's `ValidationErrors` record —
//! `Record<string, string | string[]>` — a *name-keyed* mapping of server
//! errors, not a form-level message stack. The form routes it: every named
//! field's own state receives its messages and displays them in its own
//! error slot, exactly as React Aria routes server errors into the field a
//! `<FieldError />` composes. The upstream row is "displayed immediately and
//! cleared when user modifies the field", and both halves live with the
//! field: an edit suppresses only that field's messages, reset hides them
//! all, and each field keeps its own delivery receipt — the record revision
//! stored beside its messages (see `SetServerErrors`) — so a clone of a
//! delivered record re-arms nothing, wherever the field sits or however the
//! registrations churn, while a freshly built record — same content or not —
//! re-arms every named field it mentions. Blocking follows the same
//! routing: a native submit is blocked by a field whose routed messages are
//! present (unless the field is disabled or read-only, which display without
//! blocking), `validationBehavior: "aria"` displays without blocking, and a
//! name nothing displays is a name that never blocks — a control with no
//! error-display path (the live-registered selects, switches, checkboxes and
//! pickers) receives no routed messages at all, so a record can never block a
//! submission invisibly.
//!
//! What is deliberately absent is the HTTP half — `action`, `method`, `encType`
//! and `target`. There is no browser to navigate.

use std::{cell::RefCell, rc::Rc, sync::Arc};

use gpui::{
    px, AnyElement, App, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    KeyUpEvent, ParentElement, RenderOnce, SharedString, Styled, Window,
};

use crate::{input::InputState, number_field::NumberState, validation::ValidationErrors};

/// One field's value in a submission.
#[derive(Clone, Debug, PartialEq)]
pub enum FormValue {
    /// A text value.
    Text(SharedString),
    /// A numeric value.
    Number(f64),
    /// A boolean flag.
    Flag(bool),
    /// A multi-selection: `CheckboxGroup`, a multiple `Select`, `TagGroup`.
    Keys(Vec<SharedString>),
}

impl FormValue {
    /// The value as a string, the way an HTML form would send it.
    pub fn as_text(&self) -> SharedString {
        match self {
            FormValue::Text(t) => t.clone(),
            FormValue::Number(n) => SharedString::from(n.to_string()),
            // An unchecked box sends nothing; "on" is the HTML default value.
            FormValue::Flag(true) => SharedString::from("on"),
            FormValue::Flag(false) => SharedString::from(""),
            FormValue::Keys(k) => SharedString::from(
                k.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        }
    }

    /// Whether an HTML form would treat this as filled in — what `isRequired`
    /// checks against.
    pub fn is_empty(&self) -> bool {
        match self {
            FormValue::Text(t) => t.trim().is_empty(),
            FormValue::Number(_) => false,
            FormValue::Flag(v) => !v,
            FormValue::Keys(k) => k.is_empty(),
        }
    }
}

/// A submission: every named field the form was given, in registration order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FormData {
    entries: Vec<(SharedString, FormValue)>,
}

impl FormData {
    /// The value of the field with the given name, if present.
    pub fn get(&self, name: &str) -> Option<&FormValue> {
        self.entries.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    /// The named field as text, which is how a form field usually reads.
    pub fn text(&self, name: &str) -> Option<SharedString> {
        self.get(name).map(FormValue::as_text)
    }

    /// All values submitted under `name`, matching the browser `FormData`
    /// `getAll` operation. Multi-selection fields contribute one value per
    /// selected key.
    pub fn get_all(&self, name: &str) -> Vec<SharedString> {
        self.entries
            .iter()
            .filter(|(entry_name, _)| entry_name == name)
            .flat_map(|(_, value)| match value {
                FormValue::Keys(keys) => keys.clone(),
                value => vec![value.as_text()],
            })
            .collect()
    }

    /// Iterates over the field names and values.
    pub fn iter(&self) -> impl Iterator<Item = (&SharedString, &FormValue)> {
        self.entries.iter().map(|(n, v)| (n, v))
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The names whose value is missing but required.
    pub fn missing_required(&self, required: &[SharedString]) -> Vec<SharedString> {
        required
            .iter()
            .filter(|name| self.get(name).is_none_or(FormValue::is_empty))
            .cloned()
            .collect()
    }
}

type Read = Arc<dyn Fn(&App) -> FormValue + 'static>;
type Restore = Arc<dyn Fn(&mut Window, &mut App) + 'static>;
type ReadName = Arc<dyn Fn(&App) -> Option<SharedString> + 'static>;
type ReadBehavior = Arc<dyn Fn(&App) -> ValidationBehavior + 'static>;
type ReadSuccessful = Arc<dyn Fn(&App) -> bool + 'static>;
/// Reads the field's stored validity — whether its own validation is in error,
/// which blocks a native submission like a missing required value.
type ReadInvalid = Arc<dyn Fn(&App) -> bool + 'static>;
/// Reads the server messages currently routed to this field by the form's
/// `validationErrors` record — present from the moment the record arrives,
/// empty once the user edited the field or reset cleared them.
type ReadServerErrors = Arc<dyn Fn(&App) -> Vec<SharedString> + 'static>;
/// Writes the routed server messages *with the revision of the record they
/// came from*, in one update. Idempotent per field: the revision is stored
/// beside the messages as the field's own delivery receipt, so re-running
/// with a record the field has already received — a clone re-rendered on a
/// later frame, or after this field moved or was replaced within the
/// registration — is a no-op, and only a genuinely new revision re-arms.
/// Present only on fields with an error-display path of their own: a control
/// that cannot show a routed message must never be blocked by one.
type SetServerErrors = Arc<dyn Fn(&mut App, Vec<SharedString>, u64) + 'static>;
/// Clears the routed server messages *without rewinding the delivery
/// receipt* — the reset half. The record that delivered already named the
/// field, so its clones must stay hidden; only a genuinely new revision
/// re-arms.
type ClearServerErrors = Arc<dyn Fn(&mut App) + 'static>;
/// Moves the focus to this field, which a blocked submit uses for v3's
/// "the first invalid field will be focused".
type FocusField = Arc<dyn Fn(&mut Window, &mut App) + 'static>;
/// Whether a focused press of Enter submits the form from this field — the
/// reader half of a native form's implicit submission. Only the controls a
/// browser submits a form from carry one: the single-line text family (a
/// `<input type=number>` among them) and, as pinned v3 builds it, the OTP
/// row's single text input. A multi-line field and every non-text compound
/// control (switch, select, checkbox group) never do.
type SubmitsOnEnter = Arc<dyn Fn(&Window, &App) -> bool + 'static>;
/// Whether the rendered control is read-only. A read-only field stays
/// successful and focusable, but HTML constraint validation bars it: neither
/// required emptiness nor stored invalidity may block a submission. Read from
/// the mirror the component's render writes, so the answer is the rendered
/// state and not a stale builder flag.
type ReadReadOnly = Arc<dyn Fn(&App) -> bool + 'static>;

/// Value, validity and focus owned by a rendered control without an entity.
pub(crate) struct LiveFormFieldState {
    pub(crate) value: FormValue,
    pub(crate) is_invalid: bool,
    pub(crate) is_successful: bool,
    pub(crate) focus: Option<FocusHandle>,
    pub(crate) restore: Option<Restore>,
    /// Commits the control's commit-gated validation snapshot for display —
    /// a form submission reaches every field at once. Written by the
    /// control's render each frame; `None` on fields with no gated validity.
    pub(crate) commit_validation: Option<CommitField>,
    /// Clears that snapshot for reset — React Stately's `resetValidation`.
    /// `None` wherever [`Self::commit_validation`] is.
    pub(crate) reset_validation: Option<CommitField>,
}

/// A named field a [`Form`] reads on submit.
///
/// The `name` may come from the field itself: a component whose value lives in
/// an entity (`Input`, `TextArea`, `NumberField`) writes its `name` prop into
/// that entity, so [`FormField::text`] and [`FormField::number`] can pick it up
/// and the call site does not repeat it.
#[must_use = "builder methods return a new value; pass the field to its form"]
#[derive(Clone)]
pub struct FormField {
    name: Option<SharedString>,
    /// Reads the `name` prop out of the field's own state entity.
    name_of: Option<ReadName>,
    read: Read,
    restore: Option<Restore>,
    is_required: bool,
    /// `validationBehavior` on the field: `Allow` shows its message without
    /// blocking submission.
    validation_behavior: ValidationBehavior,
    /// Reads `validationBehavior` off the field's own state entity.
    behavior_of: Option<ReadBehavior>,
    /// Whether this field is a successful native form control. Disabled
    /// checkbox inputs are neither submitted nor validated.
    successful_of: Option<ReadSuccessful>,
    /// Reads the field's stored validity — whether the field considers itself
    /// in error, written by `Input::render`.
    invalid_of: Option<ReadInvalid>,
    /// Reads the server messages routed to this field by the form's
    /// `validationErrors` record — what a native submit consults beside the
    /// stored validity.
    server_errors_of: Option<ReadServerErrors>,
    /// Writes (or clears) the routed server messages. `None` on every field
    /// without an error-display path, so a routed name can neither display
    /// nor block there.
    set_server_errors: Option<SetServerErrors>,
    /// Clears the routed server messages for reset, leaving the field's
    /// delivery receipt alone. `None` wherever [`Self::set_server_errors`]
    /// is: a field that cannot display routed messages has nothing to hide.
    clear_server_errors: Option<ClearServerErrors>,
    /// Commits the field's derived validation for display — a blur does it
    /// per-field inside the control's own render; a submission attempt does
    /// it for every registered field at once. `None` on fields with no
    /// commit-gated validity.
    commit_of: Option<CommitField>,
    /// Clears the displayed validation snapshot for reset — React Stately's
    /// `resetValidation`: the display returns to valid and the gated half
    /// waits for the next commit again. `None` wherever [`Self::commit_of`]
    /// is.
    reset_validation_of: Option<CommitField>,
    /// Focuses the field — the invalid-path step, when it has a reachable
    /// handle.
    focus: Option<FocusField>,
    /// Whether a focused Enter submits from this field. `None` on every
    /// non-text or compound field, which never submits implicitly.
    submits_on_enter_of: Option<SubmitsOnEnter>,
    /// Whether the rendered control is read-only — still successful and
    /// focusable, but barred from constraint validation.
    read_only_of: Option<ReadReadOnly>,
    /// The state entity carrying this field's value, when one exists. That
    /// entity outlives the frames, so it is the stable identity the form's
    /// blocked-keystroke latch is keyed by — the same trick
    /// `Input`'s `defaultValue` seed uses.
    state_id: Option<gpui::EntityId>,
    /// Whether a keyed selection with nothing chosen still submits one empty
    /// value. Pinned React Aria Components 1.20.0's key-mode serialization
    /// maps an empty `selectedKeys` to one hidden input with value `""`, so
    /// the name appears in FormData either way; group-backed fields (a
    /// checkbox group, a multiple `Select`) omit themselves instead, like the
    /// HTML controls they stand in for.
    empty_keys_submits_empty: bool,
}

impl FormField {
    /// A single-line text field, read from its [`InputState`].
    ///
    /// This is the registration for an [`Input`](crate::Input) or
    /// [`SearchField`](crate::SearchField) — the controls a native form
    /// implicitly submits from, so the field carries the Enter reader. A
    /// multi-line field rendered from the same state must register with
    /// [`FormField::text_area`].
    pub fn text(state: Entity<InputState>) -> Self {
        let read_state = state.clone();
        let name_state = state.clone();
        let behavior_state = state.clone();
        let successful_state = state.clone();
        let invalid_state = state.clone();
        let enter_state = state.clone();
        let read_only_state = state.clone();
        let routed_read_state = state.clone();
        let routed_set_state = state.clone();
        let routed_clear_state = state.clone();
        let commit_state = state.clone();
        let reset_validation_state = state.clone();
        let state_id = state.entity_id();
        let focus_state = state;
        Self {
            name: None,
            name_of: Some(Arc::new(move |cx: &App| name_state.read(cx).name())),
            behavior_of: Some(Arc::new(move |cx: &App| {
                behavior_state.read(cx).validation_behavior()
            })),
            successful_of: Some(Arc::new(move |cx: &App| {
                successful_state.read(cx).is_successful()
            })),
            invalid_of: Some(Arc::new(move |cx: &App| {
                invalid_state.read(cx).validity().is_invalid
            })),
            server_errors_of: Some(Arc::new(move |cx: &App| {
                routed_read_state.read(cx).routed_errors().to_vec()
            })),
            set_server_errors: Some(Arc::new(
                move |cx: &mut App, messages: Vec<SharedString>, revision: u64| {
                    // The receipt is the field's own: a revision this state
                    // has already delivered is a clone of a delivered record,
                    // and re-delivering it — however many frames re-render it
                    // or wherever this field now sits in the registration —
                    // would resurrect a message the user edited away. Receipt
                    // and messages move in one update, so no frame can see
                    // one without the other.
                    if routed_set_state.read(cx).routed_revision() == revision {
                        return;
                    }
                    routed_set_state.update(cx, |s, cx| {
                        s.set_routed(messages, revision);
                        cx.notify();
                    });
                },
            )),
            clear_server_errors: Some(Arc::new(move |cx: &mut App| {
                // Reset hides the messages but never rewinds the receipt.
                if !routed_clear_state.read(cx).routed_errors().is_empty() {
                    routed_clear_state.update(cx, |s, cx| {
                        s.clear_routed_errors();
                        cx.notify();
                    });
                }
            })),
            read: Arc::new(move |cx: &App| {
                FormValue::Text(SharedString::from(read_state.read(cx).value().to_owned()))
            }),
            restore: None,
            is_required: false,
            validation_behavior: ValidationBehavior::Native,
            focus: Some(Arc::new(move |window, cx| {
                let fh = focus_state.read(cx).focus_handle.clone();
                window.focus(&fh, cx);
            })),
            submits_on_enter_of: Some(Arc::new(move |window, cx| {
                enter_state.read(cx).focus_handle.is_focused(window)
            })),
            read_only_of: Some(Arc::new(move |cx: &App| {
                read_only_state.read(cx).is_read_only()
            })),
            state_id: Some(state_id),
            commit_of: Some(Arc::new(move |cx: &mut App| {
                commit_state.update(cx, |s, cx| {
                    if s.commit_display() {
                        cx.notify();
                    }
                });
            })),
            reset_validation_of: Some(Arc::new(move |cx: &mut App| {
                reset_validation_state.update(cx, |s, cx| {
                    if s.reset_display() {
                        cx.notify();
                    }
                });
            })),
            empty_keys_submits_empty: false,
        }
    }

    /// A multi-line text field, read from its [`InputState`] — the state a
    /// [`TextArea`](crate::TextArea) renders.
    ///
    /// Identical to [`FormField::text`] except that Enter never submits the
    /// form: a native form does not implicitly submit from a `<textarea>`,
    /// whose Enter is a newline. The multiline flag lives on the
    /// [`TextArea`](crate::TextArea) builder, not on the shared state, so
    /// the registration is where the control kind is named.
    pub fn text_area(state: Entity<InputState>) -> Self {
        let field = Self::text(state);
        Self {
            submits_on_enter_of: None,
            ..field
        }
    }

    /// A live field whose rendered control is a single-line text input.
    ///
    /// The keyed control's own value, restore and focus live in the shared
    /// [`LiveFormFieldState`] it syncs each frame. Everything else — the
    /// implicit-Enter reader, the read-only mirror, the resolved validity
    /// (`validate` included), `validationBehavior`, the routed server errors
    /// and the latch identity — stays on the [`InputState`] the control
    /// renders, because that state's mirrors are what the control's own
    /// render writes; duplicating the readers onto the shared state would
    /// give the form two answers that can drift apart.
    pub(crate) fn live_text(
        name: impl Into<SharedString>,
        state: Rc<RefCell<LiveFormFieldState>>,
        input: Entity<InputState>,
    ) -> Self {
        let live = Self::live(name, state);
        let text = Self::text(input);
        Self {
            name: live.name,
            read: live.read,
            restore: live.restore,
            focus: live.focus,
            empty_keys_submits_empty: true,
            ..text
        }
    }

    /// A numeric field, read from its [`NumberState`].
    ///
    /// The field's validity is the one `NumberField::render` resolved —
    /// controlled flags, then server errors, then `validate` — mirrored onto
    /// the inner [`InputState`] exactly as `name` is, so the submission reads
    /// the rendered state rather than a builder snapshot. Read-only travels
    /// the same way: `NumberField` forwards the flag to the inner input,
    /// whose render mirrors it here.
    pub fn number(state: Entity<NumberState>) -> Self {
        let read_state = state.clone();
        let name_state = state.clone();
        let behavior_state = state.clone();
        let successful_state = state.clone();
        let invalid_state = state.clone();
        let enter_state = state.clone();
        let read_only_state = state.clone();
        let routed_read_state = state.clone();
        let routed_set_state = state.clone();
        let routed_clear_state = state.clone();
        let commit_state = state.clone();
        let reset_state = state.clone();
        let state_id = state.entity_id();
        let focus_state = state;
        Self {
            name: None,
            name_of: Some(Arc::new(move |cx: &App| {
                let st = name_state.read(cx);
                st.input.read(cx).name()
            })),
            behavior_of: Some(Arc::new(move |cx: &App| {
                behavior_state.read(cx).input.read(cx).validation_behavior()
            })),
            successful_of: Some(Arc::new(move |cx: &App| {
                successful_state.read(cx).input.read(cx).is_successful()
            })),
            invalid_of: Some(Arc::new(move |cx: &App| {
                invalid_state.read(cx).input.read(cx).validity().is_invalid
            })),
            // The routed channel lives on the inner `InputState`, the same
            // state that displays the messages — and the same state that
            // carries the delivery receipt.
            server_errors_of: Some(Arc::new(move |cx: &App| {
                routed_read_state
                    .read(cx)
                    .input
                    .read(cx)
                    .routed_errors()
                    .to_vec()
            })),
            set_server_errors: Some(Arc::new(
                move |cx: &mut App, messages: Vec<SharedString>, revision: u64| {
                    let inner = routed_set_state.read(cx).input.clone();
                    // Same per-field receipt as the text field, one level
                    // down: a revision the inner state has already delivered
                    // is a clone, never a re-arm.
                    if inner.read(cx).routed_revision() == revision {
                        return;
                    }
                    inner.update(cx, |s, cx| {
                        s.set_routed(messages, revision);
                        cx.notify();
                    });
                },
            )),
            clear_server_errors: Some(Arc::new(move |cx: &mut App| {
                let inner = routed_clear_state.read(cx).input.clone();
                if !inner.read(cx).routed_errors().is_empty() {
                    inner.update(cx, |s, cx| {
                        s.clear_routed_errors();
                        cx.notify();
                    });
                }
            })),
            read: Arc::new(move |cx: &App| FormValue::Number(read_state.read(cx).value())),
            restore: None,
            is_required: false,
            validation_behavior: ValidationBehavior::Native,
            focus: Some(Arc::new(move |window, cx| {
                let fh = focus_state.read(cx).input.read(cx).focus_handle.clone();
                window.focus(&fh, cx);
            })),
            // `<input type=number>` is a single-line text control: a native
            // form submits from it.
            submits_on_enter_of: Some(Arc::new(move |window, cx| {
                enter_state
                    .read(cx)
                    .input
                    .read(cx)
                    .focus_handle
                    .is_focused(window)
            })),
            read_only_of: Some(Arc::new(move |cx: &App| {
                read_only_state.read(cx).input.read(cx).is_read_only()
            })),
            state_id: Some(state_id),
            commit_of: Some(Arc::new(move |cx: &mut App| {
                commit_state.update(cx, |s, cx| {
                    if s.commit_validation() {
                        cx.notify();
                    }
                });
            })),
            reset_validation_of: Some(Arc::new(move |cx: &mut App| {
                reset_state.update(cx, |s, cx| {
                    if s.reset_validation() {
                        cx.notify();
                    }
                });
            })),
            empty_keys_submits_empty: false,
        }
    }

    /// An OTP field, read from its [`crate::input_otp::OtpState`].
    ///
    /// Pinned v3 builds `InputOTP` on a single text input, and the row's
    /// cells share one focus handle, so a focused Enter here is the same
    /// implicit submission it is in any single-line field. The OTP answers
    /// no Enter of its own — its handler fills cells and walks the caret —
    /// so the keystroke bubbles to the form.
    pub fn code(name: impl Into<SharedString>, state: Entity<crate::input_otp::OtpState>) -> Self {
        let read_state = state.clone();
        let successful_state = state.clone();
        let invalid_state = state.clone();
        let enter_state = state.clone();
        let routed_read_state = state.clone();
        let routed_set_state = state.clone();
        let routed_clear_state = state.clone();
        let state_id = state.entity_id();
        let focus_state = state;
        Self {
            name: Some(name.into()),
            name_of: None,
            read: Arc::new(move |cx: &App| {
                FormValue::Text(SharedString::from(read_state.read(cx).code()))
            }),
            restore: None,
            is_required: false,
            validation_behavior: ValidationBehavior::Native,
            behavior_of: None,
            successful_of: Some(Arc::new(move |cx: &App| {
                successful_state.read(cx).is_successful()
            })),
            invalid_of: Some(Arc::new(move |cx: &App| {
                invalid_state.read(cx).validity().is_invalid
            })),
            server_errors_of: Some(Arc::new(move |cx: &App| {
                routed_read_state.read(cx).routed_errors().to_vec()
            })),
            set_server_errors: Some(Arc::new(
                move |cx: &mut App, messages: Vec<SharedString>, revision: u64| {
                    // Same per-field receipt as the text field.
                    if routed_set_state.read(cx).routed_revision() == revision {
                        return;
                    }
                    routed_set_state.update(cx, |s, cx| {
                        s.set_routed(messages, revision);
                        cx.notify();
                    });
                },
            )),
            clear_server_errors: Some(Arc::new(move |cx: &mut App| {
                if !routed_clear_state.read(cx).routed_errors().is_empty() {
                    routed_clear_state.update(cx, |s, cx| {
                        s.clear_routed_errors();
                        cx.notify();
                    });
                }
            })),
            focus: Some(Arc::new(move |window, cx| {
                let fh = focus_state.read(cx).focus_handle.clone();
                window.focus(&fh, cx);
            })),
            // The whole row is one text input to the form: Enter while any
            // cell holds the focus submits, exactly as it does from a
            // single-line field.
            submits_on_enter_of: Some(Arc::new(move |window, cx| {
                enter_state.read(cx).focus_handle.is_focused(window)
            })),
            // The OTP row has no read-only prop, so there is nothing to bar.
            read_only_of: None,
            state_id: Some(state_id),
            commit_of: None,
            reset_validation_of: None,
            empty_keys_submits_empty: false,
        }
    }

    /// A plain text value the caller holds — a formatted date, a colour hex,
    /// an OTP code.
    pub fn text_value(name: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        let value = value.into();
        Self {
            name: Some(name.into()),
            name_of: None,
            read: Arc::new(move |_| FormValue::Text(value.clone())),
            restore: None,
            is_required: false,
            validation_behavior: ValidationBehavior::Native,
            behavior_of: None,
            successful_of: None,
            invalid_of: None,
            // A caller-held value has no state to route messages into — and
            // nothing to display them with.
            server_errors_of: None,
            set_server_errors: None,
            clear_server_errors: None,
            focus: None,
            // A caller-held value has no rendered control to be read-only.
            read_only_of: None,
            submits_on_enter_of: None,
            state_id: None,
            commit_of: None,
            reset_validation_of: None,
            empty_keys_submits_empty: false,
        }
    }

    /// A live field owned by a single-threaded rendered control.
    pub(crate) fn live(
        name: impl Into<SharedString>,
        state: Rc<RefCell<LiveFormFieldState>>,
    ) -> Self {
        let read_state = state.clone();
        let invalid_state = state.clone();
        let successful_state = state.clone();
        let restore_state = state.clone();
        let commit_validation_state = state.clone();
        let reset_validation_state = state.clone();
        let focus_state = state;
        Self {
            name: Some(name.into()),
            name_of: None,
            read: Arc::new(move |_| read_state.borrow().value.clone()),
            restore: Some(Arc::new(move |window, cx| {
                let restore = restore_state.borrow().restore.clone();
                if let Some(restore) = restore {
                    restore(window, cx);
                }
            })),
            // The control writes its commit/reset closures each render; a
            // submission or reset reaches them through the shared state.
            commit_of: Some(Arc::new(move |cx: &mut App| {
                let commit = commit_validation_state.borrow().commit_validation.clone();
                if let Some(commit) = commit {
                    commit(cx);
                }
            })),
            reset_validation_of: Some(Arc::new(move |cx: &mut App| {
                let reset = reset_validation_state.borrow().reset_validation.clone();
                if let Some(reset) = reset {
                    reset(cx);
                }
            })),
            is_required: false,
            validation_behavior: ValidationBehavior::Native,
            behavior_of: None,
            successful_of: Some(Arc::new(move |_| successful_state.borrow().is_successful)),
            invalid_of: Some(Arc::new(move |_| invalid_state.borrow().is_invalid)),
            // A live field belongs to a rendered non-text control — a switch,
            // a select, a checkbox — none of which has an error-display path
            // the form could route into. No channel: a name landing here
            // neither displays nor blocks.
            server_errors_of: None,
            set_server_errors: None,
            clear_server_errors: None,
            focus: Some(Arc::new(move |window, cx| {
                let focus = focus_state.borrow().focus.clone();
                if let Some(focus) = focus {
                    window.focus(&focus, cx);
                }
            })),
            // A live field belongs to a rendered non-text control — a switch,
            // a select, a checkbox — none of which submits implicitly, and
            // whose read-only state is not mirrored on this shared state.
            read_only_of: None,
            submits_on_enter_of: None,
            state_id: None,
            empty_keys_submits_empty: false,
        }
    }

    /// A plain number the caller holds — a slider or colour channel.
    pub fn number_value(name: impl Into<SharedString>, value: f64) -> Self {
        Self {
            name: Some(name.into()),
            name_of: None,
            read: Arc::new(move |_| FormValue::Number(value)),
            restore: None,
            is_required: false,
            validation_behavior: ValidationBehavior::Native,
            behavior_of: None,
            successful_of: None,
            invalid_of: None,
            server_errors_of: None,
            set_server_errors: None,
            clear_server_errors: None,
            focus: None,
            read_only_of: None,
            submits_on_enter_of: None,
            state_id: None,
            commit_of: None,
            reset_validation_of: None,
            empty_keys_submits_empty: false,
        }
    }

    /// A checkbox or switch, whose value the caller holds.
    pub fn flag(name: impl Into<SharedString>, value: bool) -> Self {
        Self {
            name: Some(name.into()),
            name_of: None,
            read: Arc::new(move |_| FormValue::Flag(value)),
            restore: None,
            is_required: false,
            validation_behavior: ValidationBehavior::Native,
            behavior_of: None,
            successful_of: None,
            invalid_of: None,
            server_errors_of: None,
            set_server_errors: None,
            clear_server_errors: None,
            focus: None,
            read_only_of: None,
            submits_on_enter_of: None,
            state_id: None,
            commit_of: None,
            reset_validation_of: None,
            empty_keys_submits_empty: false,
        }
    }

    /// A selection, whose value the caller holds.
    pub fn keys(
        name: impl Into<SharedString>,
        values: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        let values: Vec<SharedString> = values.into_iter().map(Into::into).collect();
        Self {
            name: Some(name.into()),
            name_of: None,
            read: Arc::new(move |_| FormValue::Keys(values.clone())),
            restore: None,
            is_required: false,
            validation_behavior: ValidationBehavior::Native,
            behavior_of: None,
            successful_of: None,
            invalid_of: None,
            server_errors_of: None,
            set_server_errors: None,
            clear_server_errors: None,
            focus: None,
            read_only_of: None,
            submits_on_enter_of: None,
            state_id: None,
            commit_of: None,
            reset_validation_of: None,
            empty_keys_submits_empty: false,
        }
    }

    /// Overrides the name, for a field that carries none of its own.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// The value a reset restores. Without one, a reset only reports itself.
    pub fn default_text(
        mut self,
        state: Entity<InputState>,
        value: impl Into<SharedString>,
    ) -> Self {
        let value = value.into();
        self.restore = Some(Arc::new(move |_, cx: &mut App| {
            state.update(cx, |s, cx| {
                s.set_value(value.to_string());
                s.reset_display();
                cx.notify();
            });
        }));
        self
    }

    /// The number a reset restores.
    pub fn default_number(mut self, state: Entity<NumberState>, value: f64) -> Self {
        self.restore = Some(Arc::new(move |_, cx: &mut App| {
            state.update(cx, |s, cx| {
                s.set_value(value, cx);
                s.reset_validation();
                cx.notify();
            });
        }));
        self
    }

    /// Marks the field required, which is what `onInvalid` reports on.
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// `validationBehavior` — `Allow` shows the field's message without
    /// blocking submission.
    pub fn validation_behavior(mut self, behavior: ValidationBehavior) -> Self {
        self.validation_behavior = behavior;
        self
    }

    /// Whether this field's invalidity blocks submission.
    pub fn blocks_submission(&self, cx: &App) -> bool {
        let behavior = self
            .behavior_of
            .as_ref()
            .map_or(self.validation_behavior, |f| f(cx));
        behavior == ValidationBehavior::Native
    }

    /// The name this field submits under: an explicit [`FormField::name`],
    /// otherwise the `name` prop the component wrote into its own state.
    pub fn field_name(&self, cx: &App) -> Option<SharedString> {
        self.name
            .clone()
            .or_else(|| self.name_of.as_ref().and_then(|f| f(cx)))
    }

    /// Whether Enter pressed right now submits the form from this field —
    /// the field holds the focus and is a single-line text control, the
    /// only thing a native form implicitly submits from.
    fn submits_on_enter(&self, window: &Window, cx: &App) -> bool {
        self.submits_on_enter_of
            .as_ref()
            .is_some_and(|f| f(window, cx))
    }

    /// Whether the rendered control is read-only: still successful and
    /// focusable, but barred from constraint validation — a native form
    /// neither blocks on a read-only field's emptiness nor on its stored
    /// errors, while its value still submits.
    fn is_read_only(&self, cx: &App) -> bool {
        self.read_only_of.as_ref().is_some_and(|f| f(cx))
    }

    /// Whether the field's stored validity is in error — the render-side of
    /// the validation a native submit consults.
    fn is_invalid(&self, cx: &App) -> bool {
        self.invalid_of.as_ref().is_some_and(|f| f(cx))
    }

    /// A submission attempt commits every field's derived validation — v3
    /// shows all field errors on a blocked submit, not only the blurred ones.
    fn commit(&self, cx: &mut App) {
        if let Some(commit) = &self.commit_of {
            commit(cx);
        }
    }

    /// The server messages currently routed to this field by the form's
    /// `validationErrors` record — present from the moment the record
    /// arrives, gone once the user edited the field or a reset hid them.
    fn server_errors(&self, cx: &App) -> Vec<SharedString> {
        self.server_errors_of
            .as_ref()
            .map_or_else(Vec::new, |f| f(cx))
    }

    /// Whether this field carries routed server messages.
    fn has_server_errors(&self, cx: &App) -> bool {
        !self.server_errors(cx).is_empty()
    }

    /// Delivers this field's messages from the record — the routing half of
    /// `Form::validation_errors`. A name the record does not mention clears
    /// the field's routed messages; a field with no display channel is never
    /// written, so a name landing there can never block invisibly. The write
    /// is receipted on the field's own state (see [`SetServerErrors`]), so
    /// calling this every frame — or after this field moved or was replaced
    /// within the registration — delivers nothing until a genuinely new
    /// record arrives.
    fn deliver_server_errors(&self, record: &ValidationErrors, cx: &mut App) {
        let Some(set) = self.set_server_errors.as_ref() else {
            return;
        };
        // An unnamed field — its control has not rendered yet, so its name
        // reader still answers None — receives nothing and stamps no receipt.
        // A receipt taken before the name exists would spend this record's
        // revision on an empty delivery, and the error would never reach the
        // field once its control later renders and publishes its name.
        let Some(name) = self.field_name(cx) else {
            return;
        };
        let messages = record
            .get(&name)
            .map(<[SharedString]>::to_vec)
            .unwrap_or_default();
        // A record with nothing for this field, over a slot already empty, is
        // an unobservable delivery: a freshly built empty record mints a new
        // revision every frame, and stamping it would rewrite every field's
        // receipt — and notify — for nothing. An occupied slot fails this
        // guard and clears below, so an emptied record still hides what it
        // used to name; a non-empty entry always reaches the receipted write.
        if messages.is_empty() && self.server_errors(cx).is_empty() {
            return;
        }
        set(cx, messages, record.revision());
    }

    fn is_successful(&self, cx: &App) -> bool {
        self.successful_of.as_ref().is_none_or(|f| f(cx))
    }
}

type OnSubmit = Arc<dyn Fn(&FormData, &mut Window, &mut App) + 'static>;
type OnReset = Arc<dyn Fn(&mut Window, &mut App) + 'static>;
pub(crate) type CommitField = Arc<dyn Fn(&mut App)>;

/// How invalid children are handled (`validationBehavior`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValidationBehavior {
    /// A failed field blocks submission and `onInvalid` runs instead.
    #[default]
    Native,
    /// Submission proceeds; the messages are shown but not enforced.
    Allow,
}

/// HeroUI Form: a vertical field stack that collects a named submission.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Form {
    /// `validationErrors` — the [`ValidationErrors`] record the form routes
    /// into its named fields' own error slots.
    validation_errors: ValidationErrors,
    validation_behavior: ValidationBehavior,
    fields: Vec<FormField>,
    on_submit: Option<OnSubmit>,
    on_reset: Option<OnReset>,
    on_invalid: Option<OnSubmit>,
    children: Vec<AnyElement>,
}

impl Form {
    /// Creates an empty form.
    pub fn new() -> Self {
        Self {
            validation_errors: ValidationErrors::new(),
            validation_behavior: ValidationBehavior::default(),
            fields: Vec::new(),
            on_submit: None,
            on_reset: None,
            on_invalid: None,
            children: Vec::new(),
        }
    }

    /// `validationErrors` — HeroUI v3's `ValidationErrors` record:
    /// server-side errors mapped by field name
    /// (`Record<string, string | string[]>`).
    ///
    /// The form routes the record; the fields display it. Each entry lands in
    /// the named field's own state and error slot — there is no form-level
    /// message stack. Delivery is receipted per field: each field's own state
    /// stores the record's [`ValidationErrors::revision`] beside its routed
    /// messages, so re-rendering with a clone keeps the per-field state (an
    /// edit stays suppressed, and reordering or replacing the registrations
    /// re-arms nothing), while any freshly built record re-arms every named
    /// field it mentions, content-equal or not. Names no registered field
    /// displays neither block nor render.
    ///
    /// Identity is the caller's to preserve: keep one record for as long as
    /// the response is current and pass a [`Clone`] of it each frame; building
    /// a fresh record per frame — `ValidationErrors::new()` inline, or a
    /// re-run builder chain — is a new server response every frame and re-arms
    /// every named field it mentions, even with identical content. This is
    /// React Stately's reference identity, unchanged.
    pub fn validation_errors(mut self, errors: ValidationErrors) -> Self {
        self.validation_errors = errors;
        self
    }

    /// `validationBehavior` — whether a failed field blocks submission.
    pub fn validation_behavior(mut self, behavior: ValidationBehavior) -> Self {
        self.validation_behavior = behavior;
        self
    }

    /// Registers a named field, so its value appears in the submission.
    pub fn field(mut self, field: FormField) -> Self {
        self.fields.push(field);
        self
    }

    /// `onSubmit` — receives the collected submission.
    pub fn on_submit(mut self, f: impl Fn(&FormData, &mut Window, &mut App) + 'static) -> Self {
        self.on_submit = Some(Arc::new(f));
        self
    }

    /// `onReset` — fires after the registered fields are restored.
    pub fn on_reset(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_reset = Some(Arc::new(f));
        self
    }

    /// `onInvalid` — runs instead of `onSubmit` when validation blocks it.
    ///
    /// Blocked means a required field with no value, or a field whose own
    /// resolved validity is in error (`validate`, `isInvalid`, the field's
    /// `validationErrors`, an HTML5 attribute violation, or a server message
    /// routed to it by the form's `validationErrors` record) — and only under
    /// [`ValidationBehavior::Native`]; `Allow` submits regardless, as v3's
    /// does. Disabled and read-only fields never block.
    pub fn on_invalid(mut self, f: impl Fn(&FormData, &mut Window, &mut App) + 'static) -> Self {
        self.on_invalid = Some(Arc::new(f));
        self
    }

    /// Collects the registered fields into a submission.
    pub fn data(&self, cx: &App) -> FormData {
        Self::collect_data(&self.fields, cx)
    }

    /// Collects `fields` into a submission — the body of [`Form::data`],
    /// shared with the submission path.
    fn collect_data(fields: &[FormField], cx: &App) -> FormData {
        let mut entries = Vec::with_capacity(fields.len());
        for field in fields {
            if !field.is_successful(cx) {
                continue;
            }
            // An unnamed field is not submitted, exactly as in HTML.
            if let Some(name) = field.field_name(cx) {
                let value = (field.read)(cx);
                // Unchecked checkbox inputs and checkbox groups with no
                // selected inputs are absent from native FormData — but a
                // keyed field carrying the pinned key-mode serialization
                // still submits one empty value, the hidden `value=""`
                // input RAC's `values.length === 0` branch renders.
                let value = match (&value, field.empty_keys_submits_empty) {
                    (FormValue::Keys(keys), true) if keys.is_empty() => {
                        FormValue::Text(SharedString::from(""))
                    }
                    _ => value,
                };
                let omitted = match &value {
                    FormValue::Flag(false) => true,
                    FormValue::Keys(keys) => keys.is_empty(),
                    _ => false,
                };
                if !omitted {
                    entries.push((name, value));
                }
            }
        }
        FormData { entries }
    }

    /// The names of the required fields whose *own* value is missing and
    /// whose error would block: enabled, native, not read-only. Each field is
    /// asked about its own live value — two fields registered under one name
    /// are validated independently, never collapsed through the submission
    /// record's first-wins `get`.
    fn missing_required_fields(fields: &[FormField], cx: &App) -> Vec<SharedString> {
        fields
            .iter()
            .filter(|f| {
                f.is_successful(cx)
                    && f.is_required
                    && !f.is_read_only(cx)
                    && f.blocks_submission(cx)
                    && (f.read)(cx).is_empty()
            })
            .filter_map(|f| f.field_name(cx))
            .collect()
    }

    /// The one submission implementation: collect the named fields, decide
    /// whether validation blocks, and route to `on_submit` or `on_invalid`.
    /// Both doors into a submission — the caller-wired submit button
    /// ([`Form::submit_handler`]) and the form root's implicit Enter key
    /// handler — run this, so a submission is validated, focused and
    /// reported identically however it arrived.
    ///
    /// `defer_focus` is the Enter-origin latch. On the button path it is
    /// `None` and a blocked submit focuses the first invalid field inline,
    /// exactly as a click handler may. On the Enter path it is the keyed
    /// latch: focusing mid-keystroke would leave the newly focused control
    /// holding the focus when the key is *released*, and gpui activates a
    /// focused element's click listeners on release — a blocked Enter in a
    /// text field would open the Select it moved the focus to, or flip the
    /// Switch. So the latch is set instead, the release handler disarms the
    /// release and moves the focus once the dispatch is over.
    fn run_submission(
        fields: &[FormField],
        behavior: ValidationBehavior,
        on_submit: Option<&OnSubmit>,
        on_invalid: Option<&OnSubmit>,
        defer_focus: Option<&Entity<bool>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        for field in fields {
            field.commit(cx);
        }
        let data = Self::collect_data(fields, cx);
        let missing = Self::missing_required_fields(fields, cx);
        // A field error blocks a native submit however it arose: a required
        // field with no value, a field whose stored validity says it is in
        // error (`validate`, `isInvalid`, the field's `validationErrors`, or
        // an HTML5 attribute violation), or a field the form's
        // `validationErrors` record routed a server message to. Read-only
        // fields are barred from constraint validation on all counts, and a
        // disabled field is not successful, so neither can block.
        let own_invalid: Vec<SharedString> = fields
            .iter()
            .filter(|f| {
                f.is_successful(cx)
                    && f.blocks_submission(cx)
                    && !f.is_read_only(cx)
                    && (f.is_invalid(cx) || f.has_server_errors(cx))
            })
            .filter_map(|f| f.field_name(cx))
            .collect();
        let blocked = behavior == ValidationBehavior::Native
            && (!missing.is_empty() || !own_invalid.is_empty());
        if blocked {
            // v3, verbatim: "By default, the first invalid field will be
            // focused." A blocked submit moves the focus to the first
            // registered field whose error keeps the form from submitting —
            // the same union the blocked condition above computes. From the
            // button path this is an ordinary click handler; from the Enter
            // path the move is deferred past the keystroke (see
            // `defer_focus`). The tests drive both paths and assert the
            // field holds the focus after.
            let focus = Self::first_invalid_focus(fields, cx);
            match (defer_focus, focus) {
                (Some(latch), Some(_)) => latch.update(cx, |pending, _| *pending = true),
                (_, focus) => {
                    if let Some(focus) = focus {
                        focus(window, cx);
                    }
                }
            }
            if let Some(f) = on_invalid {
                f(&data, window, cx);
            }
        } else if let Some(f) = on_submit {
            f(&data, window, cx);
        }
    }

    /// The focus callback of the first registered field whose error blocks
    /// the submission — the same union [`Self::run_submission`] tests for
    /// the blocked decision, so the focus lands on the field the report is
    /// about. Read-only fields cannot nominate themselves, and a field with
    /// no reachable handle contributes none. An unnamed field cannot either:
    /// the blocking lists collect names (each `filter_map`s `field_name`),
    /// so an unnamed required or invalid field never blocks and must never
    /// take the focus from the named blocker behind it. Each field is asked
    /// about its own live value, so duplicate names never collapse into the
    /// record's first-wins `get`.
    fn first_invalid_focus(fields: &[FormField], cx: &App) -> Option<FocusField> {
        fields
            .iter()
            .find(|field| {
                field.field_name(cx).is_some()
                    && !field.is_read_only(cx)
                    && field.is_successful(cx)
                    && field.blocks_submission(cx)
                    && (field.is_invalid(cx)
                        || field.has_server_errors(cx)
                        || (field.is_required && (field.read)(cx).is_empty()))
            })
            .and_then(|field| field.focus.clone())
    }

    /// The handler a submit button calls: collects the submission, then routes
    /// it to `onSubmit` or `onInvalid`.
    ///
    /// gpui gives a child no way to reach its form, so the caller wires this to
    /// the button in place of `<button type="submit">`. The form root's Enter
    /// key handler runs the same implementation, so a submission is decided
    /// identically however it arrived.
    #[allow(clippy::arc_with_non_send_sync)] // see `util::shared`
    pub fn submit_handler(&self) -> Arc<dyn Fn(&mut Window, &mut App) + 'static> {
        let fields = self.fields.clone();
        let on_submit = self.on_submit.clone();
        let on_invalid = self.on_invalid.clone();
        let behavior = self.validation_behavior;
        Arc::new(move |window: &mut Window, cx: &mut App| {
            // Button origin: no keystroke is in flight, so the focus move
            // needs no deferral.
            Self::run_submission(
                &fields,
                behavior,
                on_submit.as_ref(),
                on_invalid.as_ref(),
                None,
                window,
                cx,
            );
        })
    }

    /// The handler a reset button calls: restores every field that declared a
    /// default, hides the routed server errors, then fires `onReset`.
    ///
    /// Hiding is a clear, not a rewind: each field keeps its delivery receipt
    /// (see `SetServerErrors`), so a re-render passing a *clone* keeps the
    /// messages hidden, and only a genuinely new record re-arms.
    #[allow(clippy::arc_with_non_send_sync)] // see `util::shared`
    pub fn reset_handler(&self) -> Arc<dyn Fn(&mut Window, &mut App) + 'static> {
        let restores: Vec<Restore> = self
            .fields
            .iter()
            .filter_map(|f| f.restore.clone())
            .collect();
        let clear_server_errors: Vec<ClearServerErrors> = self
            .fields
            .iter()
            .filter_map(|f| f.clear_server_errors.clone())
            .collect();
        let reset_validation: Vec<CommitField> = self
            .fields
            .iter()
            .filter_map(|f| f.reset_validation_of.clone())
            .collect();
        let on_reset = self.on_reset.clone();
        Arc::new(move |window: &mut Window, cx: &mut App| {
            for restore in &restores {
                restore(window, cx);
            }
            for clear in &clear_server_errors {
                clear(cx);
            }
            for reset in &reset_validation {
                reset(cx);
            }
            if let Some(f) = &on_reset {
                f(window, cx);
            }
        })
    }
}

impl Default for Form {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for Form {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Form {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            validation_errors: record,
            validation_behavior: behavior,
            fields,
            on_submit,
            on_invalid,
            children,
            ..
        } = self;
        let mut el = gpui::div().flex().flex_col().gap(px(16.)).w_full();
        // The `validationErrors` record routes; it does not render here.
        // Each field carries its own delivery receipt — the record revision
        // stored beside its routed messages (see [`SetServerErrors`]) — so
        // a re-render passing a *clone* redelivers nothing (a message the
        // user edited away stays suppressed), and replacing or reordering
        // this form's field registrations re-arms nothing either: the
        // receipt travels with the field, not with the form. A genuinely
        // new record — its content equal or not — re-arms every named field
        // it mentions.
        // The blocked-Enter latch: "the keystroke whose release must not
        // click anything". Keyed window state, because the release can be
        // dispatched against a frame drawn after the press — closures from
        // different frames — and keyed by the first participating field's
        // state entity, the one stable identity a form without a DOM id has
        // (only entity-backed fields — text, number, OTP — can submit on
        // Enter, so a form without one never arms the latch).
        let latch = fields.iter().find_map(|f| f.state_id).map(|id| {
            window.use_keyed_state(
                gpui::ElementId::named_usize("form-enter-latch", id.as_u64() as usize),
                cx,
                |_, _| false,
            )
        });
        // A native `<form>` also submits when Enter is pressed in a
        // single-line text control — the implicit submission React Aria's
        // Form inherits, and the door this port must wire itself because the
        // submit button is caller-wired. The handler never infers from an
        // arbitrary bubbling Enter: it fires only while a *registered field
        // that participates* — [`FormField::text`], [`FormField::number`] or
        // [`FormField::code`] — holds the focus. A TextArea's Enter is a
        // newline; a focused submit Button submits through its own click,
        // which gpui fires on key-up, so firing here too would submit twice;
        // and the non-text compound controls never submit implicitly.
        let down_fields = fields.clone();
        let down_latch = latch.clone();
        el = el.on_key_down(move |ev: &KeyDownEvent, window, cx| {
            // A latch still set when a press arrives is from a keystroke
            // whose release never happened here — always stale, because a
            // release follows its own press through this same dispatch
            // path. Clear it before anything else arms a fresh one.
            if let Some(latch) = &down_latch {
                latch.update(cx, |pending, _| *pending = false);
            }
            // Mirror the single-line field's own Enter branch: the plain key
            // (shift allowed — the rendered control submits on shift+enter
            // too), never a chord.
            let mods = &ev.keystroke.modifiers;
            if ev.keystroke.key.as_str() == "enter"
                && !(mods.control || mods.alt || mods.platform)
                && down_fields.iter().any(|f| f.submits_on_enter(window, cx))
            {
                Self::run_submission(
                    &down_fields,
                    behavior,
                    on_submit.as_ref(),
                    on_invalid.as_ref(),
                    down_latch.as_ref(),
                    window,
                    cx,
                );
            }
        });
        // The release half of the same door: a blocked Enter set the latch
        // instead of moving the focus. Consume it on the plain release,
        // disarm gpui's key-up activation (`prevent_default` gates the
        // listener that fires a focused element's click), and move the focus
        // to the first invalid field only after the keystroke has fully
        // dispatched — the newly focused Select or Switch never holds the
        // focus during a key event, so the release cannot click it.
        let up_fields = fields.clone();
        let up_latch = latch;
        el = el.capture_key_up(move |ev: &KeyUpEvent, window, cx| {
            let Some(latch) = &up_latch else {
                return;
            };
            let mods = &ev.keystroke.modifiers;
            if ev.keystroke.key.as_str() != "enter"
                || mods.control
                || mods.alt
                || mods.platform
                || !*latch.read(cx)
            {
                return;
            }
            latch.update(cx, |pending, _| *pending = false);
            window.prevent_default();
            if let Some(focus) = Self::first_invalid_focus(&up_fields, cx) {
                window.defer(cx, move |window, cx| focus(window, cx));
            }
        });
        // The delivery itself runs as the last child of the stack: a
        // zero-size canvas appended *after* the caller's fields, whose
        // prepaint callback therefore fires after every field has rendered
        // and written its `name` into its own state — the same tree-order
        // trick that lets a field remember its text origin, or a table arm
        // its load-more sentinel. Delivering from `Form::render` instead
        // would resolve every `name_of` reader to None: the form renders
        // before its fields do. The canvas runs every frame; the per-field
        // receipt makes each write idempotent, so a settled frame writes
        // nothing.
        el = el.children(children);
        if fields.iter().any(|f| f.set_server_errors.is_some()) {
            let delivery_fields = fields;
            el = el.child(
                gpui::canvas(
                    move |_, _, cx| {
                        for field in &delivery_fields {
                            field.deliver_server_errors(&record, cx);
                        }
                    },
                    |_, _, _, _| {},
                )
                .size_0(),
            );
        }
        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(entries: &[(&str, FormValue)]) -> FormData {
        FormData {
            entries: entries
                .iter()
                .map(|(n, v)| (SharedString::from(n.to_string()), v.clone()))
                .collect(),
        }
    }

    #[test]
    fn a_submission_reads_by_name() {
        let d = data(&[
            ("email", FormValue::Text("a@b.c".into())),
            ("age", FormValue::Number(41.0)),
        ]);
        assert_eq!(d.text("email").unwrap(), SharedString::from("a@b.c"));
        assert_eq!(d.text("age").unwrap(), SharedString::from("41"));
        assert!(d.get("missing").is_none());
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn flags_and_keys_serialise_the_html_way() {
        assert_eq!(FormValue::Flag(true).as_text(), SharedString::from("on"));
        assert_eq!(FormValue::Flag(false).as_text(), SharedString::from(""));
        let keys = FormValue::Keys(vec!["a".into(), "b".into()]);
        assert_eq!(keys.as_text(), SharedString::from("a,b"));
    }

    #[test]
    fn emptiness_follows_the_control() {
        assert!(FormValue::Text("   ".into()).is_empty());
        assert!(!FormValue::Text("x".into()).is_empty());
        // An unchecked box submits nothing, so it counts as empty.
        assert!(FormValue::Flag(false).is_empty());
        assert!(!FormValue::Flag(true).is_empty());
        assert!(FormValue::Keys(vec![]).is_empty());
        // Zero is a value.
        assert!(!FormValue::Number(0.0).is_empty());
    }

    #[test]
    fn missing_required_reports_absent_and_blank_alike() {
        let d = data(&[
            ("name", FormValue::Text("".into())),
            ("tos", FormValue::Flag(true)),
        ]);
        let required: Vec<SharedString> =
            vec!["name".into(), "tos".into(), "never-registered".into()];
        assert_eq!(
            d.missing_required(&required),
            vec![
                SharedString::from("name"),
                SharedString::from("never-registered")
            ]
        );
    }
}
