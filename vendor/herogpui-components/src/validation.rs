//! The shared `validate` / `validationErrors` model.
//!
//! v3's `validate` is a function the *component* runs: it receives the current
//! value and returns either nothing or a message to display. Treating it as
//! "the caller validates and passes `isInvalid`" loses that — the component
//! never runs anything and the prop does not exist. This module is the shape
//! every field uses instead, so the eight components that document `validate`
//! behave identically.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use gpui::{App, ElementId, Entity, SharedString, Window};

use crate::form::{CommitField, ValidationBehavior};

/// Mints record identity: every *constructed or rebuilt* [`ValidationErrors`]
/// is a new server response, while a `clone` keeps the revision it was copied
/// from. Structural equality cannot carry this — a re-sent response can be
/// content-equal to the old one and must still re-arm.
static NEXT_RECORD_REVISION: AtomicU64 = AtomicU64::new(1);

fn next_record_revision() -> u64 {
    NEXT_RECORD_REVISION.fetch_add(1, Ordering::Relaxed)
}

/// HeroUI v3's `ValidationErrors` — `Record<string, string | string[]>`, the
/// server-side errors [`Form`](crate::form::Form) maps by field name.
///
/// The record is a *name-keyed mapping*, not a message list: the form routes
/// each entry into the named field's own error slot, never into a form-level
/// stack. Names the fields do not register display nowhere and block nothing.
///
/// Identity is part of the contract. Delivery is keyed to [`Self::revision`],
/// not to equality:
///
/// - a `clone` keeps its revision, so re-rendering with the same record never
///   resurrects a message the user edited away;
/// - any freshly built record — [`new`](Self::new), [`default`](Default::default)
///   or [`set`](Self::set) — mints a new revision and re-arms every named
///   field, even when the content equals the old record (a server that
///   re-sends the same error is a new response, not silence).
///
/// The caller half of that contract: *retain one record for as long as a
/// response is current* — in app state, a `thread_local`, wherever the frame
/// rebuilds from — and hand the form a [`Clone`] of it every frame.
/// Constructing a fresh record per frame (`ValidationErrors::new()`, or
/// re-running the builder chain in `render`) is a brand-new server response
/// every frame and re-arms every named field every frame, however equal the
/// content looks. This is exactly React Stately's reference identity: what
/// re-arms is a *new record*, never a re-sent one.
///
/// [`PartialEq`] compares content only, so records can be compared while the
/// identity stays a separate, explicit question.
#[derive(Clone, Debug)]
pub struct ValidationErrors {
    revision: u64,
    entries: BTreeMap<SharedString, Vec<SharedString>>,
}

impl ValidationErrors {
    /// A new, empty record: a fresh server response.
    pub fn new() -> Self {
        Self {
            revision: next_record_revision(),
            entries: BTreeMap::new(),
        }
    }

    /// The record's identity. Two records with equal content but different
    /// revisions are two different responses; a clone shares the revision.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Whether no name carries a message.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The messages recorded for `name` — `record[name]`, `None` when absent.
    pub fn get(&self, name: &str) -> Option<&[SharedString]> {
        self.entries.get(name).map(Vec::as_slice)
    }

    /// Iterates `(name, messages)` in deterministic name order.
    pub fn iter(&self) -> impl Iterator<Item = (&SharedString, &[SharedString])> {
        self.entries
            .iter()
            .map(|(name, messages)| (name, &**messages))
    }

    /// The names carrying at least one message, in deterministic name order.
    pub fn names(&self) -> impl Iterator<Item = &SharedString> {
        self.entries.keys()
    }

    /// `record[name] = message` — the single-message spelling.
    ///
    /// Rebuilding a record is a new response: the revision is minted, so a
    /// form handed the result re-arms its fields.
    pub fn set(mut self, name: impl Into<SharedString>, message: impl Into<SharedString>) -> Self {
        self.entries.insert(name.into(), vec![message.into()]);
        self.revision = next_record_revision();
        self
    }

    /// `record[name] = [m0, m1, ...]` — the multiple-messages spelling, shown
    /// in the order given.
    pub fn set_many(
        mut self,
        name: impl Into<SharedString>,
        messages: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        let messages: Vec<SharedString> = messages.into_iter().map(Into::into).collect();
        self.entries.insert(name.into(), messages);
        self.revision = next_record_revision();
        self
    }
}

impl Default for ValidationErrors {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for ValidationErrors {
    /// Content equality. Identity is [`Self::revision`] and deliberately
    /// excluded: two content-equal records can be two different responses.
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl Eq for ValidationErrors {}

/// `validate` — returns `None` when the value is acceptable, or the message to
/// show when it is not.
///
/// v3 types this `(value) => ValidationError | true | null | undefined`, where
/// a returned error marks the field invalid; `None` here is that `null`.
pub type Validator<T> = Arc<dyn Fn(&T) -> Option<SharedString> + 'static>;
// `T` may be unsized (`str`); a type alias does not enforce bounds, so none
// is written here.

/// The messages a field should display, and whether it is invalid.
///
/// Order matches v3: an explicit `isInvalid` always wins, then server-supplied
/// `validationErrors`, then whatever `validate` returns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Validity {
    /// Whether the value is invalid.
    pub is_invalid: bool,
    /// The validation messages, if any.
    pub messages: Vec<SharedString>,
}

impl Validity {
    /// The first message, if any.
    ///
    /// The single-line text family — `Input`, `NumberField`, `InputOTP` —
    /// no longer renders this: their error slots show every message through
    /// [`Self::joined`], the way React Aria's `FieldError` default renders
    /// them. The compound controls (Switch, Checkbox, the date and time
    /// fields, the colour pickers) still show the one message this returns.
    pub fn first(&self) -> Option<SharedString> {
        self.messages.first().cloned()
    }

    /// Every message joined the way React Aria's `FieldError` default renders
    /// them: one string, messages space-joined in upstream order.
    pub fn joined(&self) -> String {
        self.messages
            .iter()
            .map(|message| &message[..])
            .collect::<Vec<&str>>()
            .join(" ")
    }
}

/// Resolves a field's validity from the three sources v3 defines.
///
/// * `is_invalid` — the controlled flag; forces invalid even with no message.
/// * `errors` — `validationErrors`, e.g. from a server round-trip.
/// * `validate_result` — whatever the `validate` function returned.
/// * `own_message` — the component's own `errorMessage` prop.
pub fn resolve(
    is_invalid: bool,
    errors: &[SharedString],
    validate_result: Option<SharedString>,
    own_message: Option<SharedString>,
) -> Validity {
    let mut messages: Vec<SharedString> = Vec::new();
    messages.extend(errors.iter().cloned());
    if let Some(m) = validate_result {
        messages.push(m);
    }
    // The component's own message is the fallback, not an addition: showing it
    // alongside a specific validation failure would duplicate the reason.
    if messages.is_empty() {
        if let Some(m) = own_message {
            messages.push(m);
        }
    }
    Validity {
        is_invalid: is_invalid || !messages.is_empty(),
        messages,
    }
}

/// The commit-gated validity snapshot a field paints under
/// [`ValidationBehavior::Native`] — React Stately's `nextValidation`
/// (rewritten every render) and `currentValidity` (the last commit's copy).
///
/// `pending` is the gated half of the live resolution — `validate` and the
/// built-in constraint/parse text, with no controlled or server sources —
/// rewritten by [`Self::sync`] each render. `displayed` is what a commit last
/// copied out of it. A [`Self::queue_commit`] defers the copy to the next
/// render, so the snapshot sees the value the triggering event produced;
/// [`Self::commit`] and [`Self::reset`] act immediately for the blur and form
/// paths that run outside a render.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ValidationDisplay {
    pending: Validity,
    displayed: Validity,
    commit_queued: bool,
}

impl ValidationDisplay {
    /// Render: record this frame's commit-gated validity; a queued commit
    /// lands now, against the value the triggering event produced.
    pub(crate) fn sync(&mut self, pending: Validity) {
        self.pending = pending;
        if self.commit_queued {
            self.commit_queued = false;
            self.displayed = self.pending.clone();
        }
    }

    /// Event handlers (a toggle, a step, a calendar pick, Enter): commit on
    /// the next render.
    pub(crate) fn queue_commit(&mut self) {
        self.commit_queued = true;
    }

    /// Commit immediately against the last rendered pending (a blur in
    /// render, a form submit). Returns whether `displayed` changed.
    pub(crate) fn commit(&mut self) -> bool {
        self.commit_queued = false;
        if self.displayed == self.pending {
            return false;
        }
        self.displayed = self.pending.clone();
        true
    }

    /// `resetValidation`: displayed back to valid, a queued commit cancelled.
    /// Returns whether anything changed — the snapshot or a pending queue.
    pub(crate) fn reset(&mut self) -> bool {
        let was_queued = self.commit_queued;
        self.commit_queued = false;
        if self.displayed == Validity::default() && !was_queued {
            return false;
        }
        self.displayed = Validity::default();
        true
    }

    /// The commit-gated validity as last synced.
    pub(crate) fn pending(&self) -> &Validity {
        &self.pending
    }

    /// The displayed snapshot — what `Native` paints.
    pub(crate) fn displayed(&self) -> &Validity {
        &self.displayed
    }
}

/// What a field paints and exposes — the chrome, the error text, the
/// accessibility invalid flag, a render-prop `is_invalid`, the `is_invalid`
/// handed to an inner field.
///
/// `Allow` (= React Aria's `aria` behavior) shows every source live,
/// `validate` included, with no commit at all. `Native` keeps the controlled
/// flag, the server messages and `errorMessage` live, but the
/// validate/constraint half is the committed `displayed` snapshot — edits
/// after a commit leave it untouched until the next commit.
pub(crate) fn display_validity(
    behavior: ValidationBehavior,
    live: &Validity,
    is_invalid: bool,
    server_errors: &[SharedString],
    displayed: &Validity,
    error_message: Option<SharedString>,
) -> Validity {
    if behavior == ValidationBehavior::Allow {
        return live.clone();
    }
    let mut display = resolve(is_invalid, server_errors, None, None);
    display.messages.extend(displayed.messages.iter().cloned());
    display.is_invalid |= displayed.is_invalid;
    if display.messages.is_empty() && display.is_invalid {
        if let Some(message) = error_message {
            display.messages.push(message);
        }
    }
    display
}

/// Change-on-blur tracking for commit-gated validation. Call every render
/// with whether the control holds focus (`focus`-within for compound
/// controls) and its current value; returns the value it held when focus
/// arrived, on the one render where focus has just left — `None` otherwise.
///
/// The keyed state under `id` is `Some(value_at_arrival)` exactly while the
/// control is focused, so presence doubles as the previous frame's focus.
pub(crate) fn left_focus<V: Clone + PartialEq + 'static>(
    id: ElementId,
    focus_within: bool,
    value: &V,
    window: &mut Window,
    cx: &mut App,
) -> Option<V> {
    let held = window.use_keyed_state(id, cx, |_, _| None::<V>);
    if focus_within {
        if held.read(cx).is_none() {
            // Focus gained: remember the value — the blur below commits only
            // when it differs, as the native `change` event does.
            let arrival = value.clone();
            held.update(cx, |slot, _| *slot = Some(arrival));
        }
        return None;
    }
    let arrival = held.read(cx).clone();
    if arrival.is_some() {
        held.update(cx, |slot, _| *slot = None);
    }
    arrival
}

/// Queue a commit on a component-owned [`ValidationDisplay`] and repaint: the
/// commit lands on the next render's `sync`, the way a toggle or step commits
/// upstream's single validation state.
pub(crate) fn queue_commit(display: &Entity<ValidationDisplay>, cx: &mut App) {
    display.update(cx, |d, cx| {
        d.queue_commit();
        cx.notify();
    });
}

/// The `(commit, reset)` closures a [`LiveFormFieldState`](crate::form::LiveFormFieldState)
/// stores so a `Form`'s submit/reset can reach the snapshot. Each notifies
/// only when the displayed half actually changed.
pub(crate) fn form_hooks(display: &Entity<ValidationDisplay>) -> (CommitField, CommitField) {
    let commit_display = display.clone();
    let commit: CommitField = Arc::new(move |cx: &mut App| {
        commit_display.update(cx, |d, cx| {
            if d.commit() {
                cx.notify();
            }
        });
    });
    let reset_display = display.clone();
    let reset: CommitField = Arc::new(move |cx: &mut App| {
        reset_display.update(cx, |d, cx| {
            if d.reset() {
                cx.notify();
            }
        });
    });
    (commit, reset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> SharedString {
        SharedString::from(v.to_owned())
    }

    #[test]
    fn clean_value_is_valid() {
        let v = resolve(false, &[], None, None);
        assert!(!v.is_invalid);
        assert!(v.messages.is_empty());
        assert_eq!(v.first(), None);
    }

    #[test]
    fn is_invalid_alone_marks_the_field() {
        // No message, but the caller says it is wrong.
        let v = resolve(true, &[], None, None);
        assert!(v.is_invalid);
        assert!(v.messages.is_empty());
    }

    #[test]
    fn a_validate_failure_marks_and_reports() {
        let v = resolve(false, &[], Some(s("Too short")), None);
        assert!(v.is_invalid);
        assert_eq!(v.first(), Some(s("Too short")));
    }

    #[test]
    fn server_errors_come_first() {
        let v = resolve(false, &[s("Already taken")], Some(s("Too short")), None);
        assert_eq!(v.messages, vec![s("Already taken"), s("Too short")]);
        assert_eq!(v.first(), Some(s("Already taken")));
    }

    #[test]
    fn own_message_is_a_fallback_not_an_addition() {
        // With a specific failure, the generic message would duplicate it.
        let v = resolve(
            false,
            &[],
            Some(s("Too short")),
            Some(s("Check this field")),
        );
        assert_eq!(v.messages, vec![s("Too short")]);
        // With nothing specific, it is what the field shows.
        let v = resolve(false, &[], None, Some(s("Check this field")));
        assert_eq!(v.messages, vec![s("Check this field")]);
        assert!(v.is_invalid);
    }

    #[test]
    fn joined_renders_every_message_in_upstream_order() {
        // React Aria's FieldError default is `validationErrors.join(' ')`:
        // all messages, one string, upstream order.
        let v = resolve(
            false,
            &[s("Already registered"), s("Check the server")],
            Some(s("Too short")),
            None,
        );
        assert_eq!(v.joined(), "Already registered Check the server Too short");
        assert_eq!(v.first(), Some(s("Already registered")));
        assert_eq!(resolve(false, &[], None, None).joined(), "");
    }

    #[test]
    fn a_record_maps_names_to_one_or_many_messages() {
        let record = ValidationErrors::new()
            .set("email", "Already registered")
            .set_many("roles", ["Role A", "Role B"]);
        assert_eq!(record.get("email"), Some(&[s("Already registered")][..]));
        assert_eq!(record.get("roles"), Some(&[s("Role A"), s("Role B")][..]));
        assert_eq!(record.get("absent"), None);
        assert!(!record.is_empty());
        // Deterministic name order, whatever the insertion order was.
        assert_eq!(
            record.names().cloned().collect::<Vec<_>>(),
            vec![s("email"), s("roles")]
        );
        assert_eq!(record.iter().count(), 2);
        assert!(ValidationErrors::new().is_empty());
    }

    #[test]
    fn a_clone_keeps_record_identity_while_a_new_record_mints() {
        let record = ValidationErrors::new().set("email", "Taken");
        let clone = record.clone();
        assert_eq!(clone.revision(), record.revision(), "clones share identity");
        assert_eq!(clone, record, "equality is content only");

        // The same content, rebuilt: a new response, not the old one.
        let rebuilt = ValidationErrors::new().set("email", "Taken");
        assert_eq!(rebuilt, record, "content equality holds");
        assert_ne!(
            rebuilt.revision(),
            record.revision(),
            "a genuinely new record must carry a new revision"
        );
        assert_ne!(
            ValidationErrors::default().revision(),
            ValidationErrors::default().revision(),
            "even two defaults are distinct responses"
        );

        // Rebuilding through a builder mints too: `default().set(..)` is a
        // fresh record, never a revision-0 orphan that would dedup against
        // another default-built record.
        let built = ValidationErrors::default().set("email", "Taken");
        assert_ne!(built.revision(), 0);
        assert_ne!(built.revision(), ValidationErrors::default().revision());
    }

    fn invalid() -> Validity {
        Validity {
            is_invalid: true,
            messages: vec![s("bad")],
        }
    }

    #[test]
    fn a_queued_commit_lands_on_the_next_sync() {
        let mut display = ValidationDisplay::default();
        display.sync(invalid());
        // Nothing displayed until a commit.
        assert_eq!(display.displayed(), &Validity::default());
        display.queue_commit();
        // Still nothing until the next render syncs the pending snapshot.
        assert_eq!(display.displayed(), &Validity::default());
        display.sync(invalid());
        assert_eq!(display.displayed(), &invalid());
    }

    #[test]
    fn commit_and_reset_report_whether_displayed_changed() {
        let mut display = ValidationDisplay::default();
        display.sync(invalid());
        // pending -> displayed is a change.
        assert!(display.commit());
        assert_eq!(display.displayed(), &invalid());
        // A second commit of the same pending is a no-op.
        assert!(!display.commit());
        // Reset clears the snapshot and reports the change; a second reset
        // over an already-clean display does not.
        assert!(display.reset());
        assert_eq!(display.displayed(), &Validity::default());
        assert!(!display.reset());
    }

    #[test]
    fn reset_cancels_a_queued_commit() {
        let mut display = ValidationDisplay::default();
        display.sync(invalid());
        display.queue_commit();
        display.reset();
        // The queued commit must not resurrect the snapshot on the next sync.
        display.sync(invalid());
        assert_eq!(display.displayed(), &Validity::default());
    }
}
