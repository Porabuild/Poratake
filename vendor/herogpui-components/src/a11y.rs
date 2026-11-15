//! What each control tells assistive technology, stated once per control.
//!
//! HeroUI v3 owns almost none of this itself: every in-scope component
//! delegates to React Aria Components 1.20.0, which delegates to React Aria
//! 3.51.0's hooks, and the hooks are where the role and the aria attributes
//! are decided. So the contract this module carries is *their* contract, read
//! out of the pinned packages under `web/node_modules` — for example
//! `react-aria/dist/private/toggle/useToggle.js` is what says a checkbox's
//! `aria-required` appears only when the field is required, and
//! `.../progress/useProgressBar.js` is what says an indeterminate bar drops
//! `aria-valuenow` while keeping `aria-valuemin`/`aria-valuemax`.
//!
//! # Shape
//!
//! Three pieces, in the order a call site meets them:
//!
//! 1. [`Name`] — the accessible name and description. The web builds these by
//!    *reference* (`aria-labelledby`, `aria-describedby` pointing at the
//!    rendered `<Label>`, `<Description>` and `<FieldError>` nodes); gpui takes
//!    literal strings, so [`Name::field`] performs the join React Aria's
//!    `useField` performs with ids, once, for every field in the port.
//! 2. [`Range`] — the numeric range a slider, meter, progress bar or spin
//!    button reports. Clamping, the indeterminate case and the value text live
//!    here rather than in four components.
//! 3. [`A11y`] — the extension trait that writes 1 and 2 onto an element.
//!
//! # Why an extension trait, and why it is not on plain `div()`
//!
//! [`A11y`] extends [`StatefulInteractiveElement`], which gpui implements only
//! for elements that already carry an `.id(..)`. That is not an accident of
//! convenience: gpui derives an AccessKit `NodeId` by hashing the element's
//! `GlobalElementId`, so an element with no id produces no node at all and a
//! role set on it is silently dropped (`window/a11y.rs` logs
//! "focused element has no accessibility node"). Hanging the helpers off
//! `StatefulInteractiveElement` means a decorative wrapper cannot accidentally
//! acquire a contract, and a control that wants one has to have earned an id
//! first — which, per [`herogpui_core::element_id`], must be derived from the
//! caller's id rather than a constant, or two instances collide into one node.
//!
//! The role method is spelled [`A11y::a11y`] rather than gpui's own `role`
//! because `role` is already taken twice in this workspace —
//! `ThemeBuilder::role` and `ActiveTheme::role` are colour roles — and
//! `.shots/a11y_audit.py` has to be able to tell a parity claim from a colour
//! lookup by reading the source.
//!
//! # Deliberate omissions
//!
//! gpui-pre 0.3.3's `StatefulInteractiveElement` exposes 25 accessibility
//! builders and no more. React Aria sets these attributes that have no
//! counterpart there, so this port does not carry them:
//!
//! | Upstream attribute | Where React Aria sets it | Why it is omitted |
//! |---|---|---|
//! | `aria-disabled` | `useLink`, `useRadioGroup`, `useNumberField` group, pending `Button` | No gpui builder. A disabled control here leaves the tab order instead (`docs/agents/components.md`), which is the observable half. |
//! | `aria-invalid` | every field hook, when invalid | No gpui builder. The message text still reaches the node through [`Name::field`]. |
//! | `aria-required` | every field hook, when `isRequired` | No gpui builder. The visible `*` in [`crate::field::Label`] is the port's only marker. |
//! | `aria-readonly` | `useToggle`, `useRadioGroup`, `useSpinButton` | No gpui builder. |
//! | `aria-errormessage` | every field hook | No gpui builder, and React Aria itself notes it is unsupported by VoiceOver/NVDA and duplicates it into `aria-describedby`, which [`Name::field`] does carry. |
//! | `aria-labelledby` / `aria-describedby` | everywhere | gpui has no id-reference graph; the strings are inlined instead. |
//! | `aria-controls` | `useNumberField`'s stepper buttons, `useToggle`, `useDisclosure`, `useOverlayTrigger` | No gpui builder and no id graph to point at. A trigger can say it is expanded ([`A11y::a11y_expanded`]) but not what it expanded. |
//! | `aria-roledescription` | `useNumberField`'s input ("number field") | No gpui builder. |
//! | `aria-live="off"` | `useSlider`'s output | No gpui builder; gpui announces nothing live, so the suppression is moot. |
//! | `role="meter progressbar"` | `useMeter` | AccessKit roles are a single enum; the fallback half of upstream's pair exists only for browsers that do not implement `meter`. `accesskit::Role::Meter` is the half that is true. |
//! | `role="spinbutton"` | `useSpinButton` | `useNumberField` deletes it again (`role: null`) before it reaches the DOM, so the port must not add it. |
//! | `aria-modal="false"` | `useToast`'s `toastProps` | No gpui builder, and `accesskit::Role` is a single enum with no modality flag: `AlertDialog` is the role, and whether it traps focus is not something the node can say. `useDialog` deliberately sets no `aria-modal` at all (a WebKit bug), so a modal and a non-modal dialog are the same node upstream too. |
//! | `role="alert"` + `aria-atomic` | `useToast`'s `contentProps` | The point of that inner node is the live announcement. gpui exposes no live-region builder at all, so the port would be claiming an announcement it cannot make; the toast card's own `alertdialog` node carries the text instead. |
//! | `aria-haspopup` | `useOverlayTrigger`, `useMenuItem`'s submenu rows | No gpui builder. |
//! | `aria-hidden` | `useDisclosure`'s collapsed panel, `useToast`'s hidden content | No gpui builder. A collapsed panel leaves the element tree here instead, which is the stronger version of the same thing. |
//! | `role="presentation"` | `useMenuSection`'s heading | No gpui builder, and none is needed: an element with no role already produces no node (`window/a11y.rs`), which is what `presentation` asks for. |
//! | `role="separator"` | `useSeparator` | AccessKit 0.24 has no `Role::Separator`. `Role::Splitter` is a pane splitter, not a rule (it is what [`crate::resizable::ResizablePanelGroup`]'s handles report). [`crate::separator::Separator`] takes an optional `.id()` so a later AccessKit bump can claim the role; until then the row stays `PENDING` rather than lying. |
//! | `aria-multiselectable` | `useListBox`, `useGridList`, `useGrid` — `selectionMode === 'multiple' ? 'true' : undefined` | No gpui builder. Whether a collection takes more than one selection is not something an AccessKit node can say here; each row's [`A11y::a11y_selected`] still reports its own state. |
//! | `aria-sort` | `useTableColumnHeader` — `isSortedColumn ? sortDirection : 'none'` on a sortable column | No gpui builder. Upstream itself drops it on Android Talkback (`!isAndroid()`) and puts the sort order into `aria-describedby` instead, which is the half [`Name`] can carry. |
//! | `aria-autocomplete="list"` | `useComboBox`, `useAutocomplete` | No gpui builder. |
//! | `aria-live` / `aria-atomic` / `aria-relevant` | `useTagGroup`'s grid (`'aria-live': isFocusWithin ? 'polite' : 'off'`) | gpui exposes no live-region builder at all, the same reason the toast's inner `role="alert"` node is omitted. |
//! | `aria-colspan` | `useGridCell` | No gpui builder; this port's table has no spanning cells to describe either. |
//!
//! | `aria-colspan` | `useGridCell` | No gpui builder; this port's table has no spanning cells to describe either. |
//! | `aria-current="page"` | `Pagination`'s active page, `Breadcrumbs`' last crumb | No gpui builder on vanilla `gpui-pre`: AccessKit 0.24 defines the state but gpui publishes no setter and no node propagation for it. The active page keeps its pressed/disabled visual state and its name; only the current-page announcement is missing. Carried before as a local renderer fork (`docs/upstream/retired-patches/`), now an upstream PR item instead. |
//!
//! Every one of these is a "gpui has no equivalent" omission in the sense
//! `docs/agents/parity.md` requires: checked against the pinned gpui source,
//! not assumed.
//!
//! # The one omission that is not gpui's fault: a composed trigger
//!
//! React Aria puts the trigger half of an overlay contract —
//! `aria-expanded`, `aria-haspopup`, `aria-controls` — onto the *caller's own
//! button*, by injecting props through React context: RAC's `MenuTrigger`,
//! `DialogTrigger` and `Disclosure` all hand `buttonProps` down to whatever
//! `<Button>` the caller composed inside them. gpui has no equivalent: an
//! element is built by its owner and a parent cannot reach into a child
//! element it was handed as an `AnyElement`.
//!
//! So the rule in this port is *who owns the element*:
//!
//! * [`crate::accordion::Accordion`] builds its own trigger row, so it states
//!   `Role::Button` and [`A11y::a11y_expanded`] there.
//! * [`crate::disclosure::Disclosure`], [`crate::dropdown::Dropdown`],
//!   [`crate::popover::Popover`] and [`crate::tooltip::Tooltip`] take the
//!   trigger from the caller — a [`crate::button::Button`], or any element at
//!   all. Their triggers therefore report whatever that element reports and no
//!   expansion state. Wrapping the caller's element in a second node with a
//!   button role would report the trigger twice, which is worse than reporting
//!   it once without `aria-expanded`.
//!
//! Closing that gap is not an accessibility change: it needs an expanded-state
//! prop on `Button` that HeroUI v3 does not document (v3's `Button` has no
//! such prop either — RAC injects it), so it would be invented API.
//!
//! ## What the same rule costs the pickers
//!
//! Wave 3 met the sharper form of it. `useComboBox` does not decorate a
//! trigger button — it turns the **text input itself** into the widget:
//! `role: 'combobox'`, `aria-expanded`, `aria-controls`, `aria-autocomplete`
//! and `aria-activedescendant` all land on `inputProps`, which RAC's
//! `ComboBox` hands to whatever `<Input>` is composed inside it.
//!
//! In this port [`crate::combo_box::ComboBox`] and
//! [`crate::autocomplete::Autocomplete`] *construct* their text field, but
//! they construct it as a [`crate::input::Input`] value and call its `render`;
//! the role is decided inside `Input::render` from its `InputType`, and there
//! is no prop that overrides it. So the field keeps `Role::TextInput` /
//! `Role::SearchInput` — which is what it is — and the combobox half of the
//! contract is stated on the parts the picker does own:
//!
//! * the popup list, which is a real `role="listbox"` upstream too, and its
//!   rows, which are real `role="option"`s;
//! * the highlighted row, through [`A11y::a11y_active_descendant`] — gpui puts
//!   that relation on the descendant rather than the container, so it is the
//!   one piece of `useComboBox`'s input contract that survives not owning the
//!   input;
//! * [`crate::select::Select`]'s and `Autocomplete`'s trigger, which those
//!   components build themselves and which is a `<button>` upstream
//!   (`useSelect` derives it from `useMenuTrigger`), so it states
//!   `Role::Button` and [`A11y::a11y_expanded`].
//!
//! What is lost is `role="combobox"` on the field and its `aria-expanded`.
//! Both need either a role override on `Input` — invented API, since v3's
//! `Input` has no such prop; RAC injects it through `ComboBoxContext` — or a
//! parent that can modify an element it was handed, which gpui does not have.
//!
//! # Wave 4: status and content
//!
//! Most of v3's display surface — `Alert`, `Avatar`, `Badge`, `Card`, `Form`,
//! `Kbd`, `ScrollShadow`, `Skeleton`, `Surface`, `Typography` — imports no
//! RAC primitive and authors no `role`. A node there would be an invention.
//! [`crate::spinner::Spinner`] is the exception: `spinner/spinner.js`
//! hard-codes `role: "status"` / `aria-label="Loading"` on the root span,
//! which is why it is the one status node under contract. [`crate::separator::Separator`]
//! is a real AccessKit gap (RAC `Separator` + `useSeparator`, no
//! `Role::Separator` in 0.24), not an id-less constructor.
//!
//! # Wave 5: calendars and overlay-backed fields
//!
//! The calendar family and the colour/date fields that compose it. Each
//! states the role its React Aria hook reports:
//!
//! - [`crate::calendar::Calendar`] / [`crate::range_calendar::RangeCalendar`]:
//!   `Role::Application` on the root (`useCalendarBase`), `Role::Grid` on
//!   each month (`useCalendarGrid`), `Role::Button` plus `aria-selected` on
//!   each day (the pressable half of `useCalendarCell`'s `gridcell`+`button`
//!   pair — this port draws one circle), and `Role::Button` named
//!   "Previous"/"Next" on the nav.
//! - [`crate::date_picker::DateField`] / [`crate::time_field::TimeField`]:
//!   `Role::Group` on the box, `Role::TextInput` on each segment
//!   (`useDateSegment` rewrites the spinbutton into a textbox).
//! - [`crate::date_picker::DatePicker`] / `DateRangePicker`: `Role::Group` on
//!   the field and `Role::Button` plus `aria-expanded` on the trigger they
//!   build themselves.
//! - Colour: `Role::Group` on [`crate::color_picker::ColorArea`], `Role::Slider`
//!   on [`crate::color_picker::ColorSlider`], `Role::TextInput` on
//!   [`crate::color_picker::ColorField`], `Role::RadioGroup` /
//!   `Role::RadioButton` on [`crate::color_picker::ColorSwatchPicker`], and
//!   `Role::Button` plus `aria-expanded` / `Role::Dialog` on
//!   [`crate::color_picker::ColorPicker`], and `Role::Image` on a named
//!   [`crate::color_picker::ColorSwatch`].
//!
//! Groups that take no required id — [`crate::button_group::ButtonGroup`],
//! [`crate::input_group::InputGroup`], [`crate::progress::ProgressCircle`],
//! [`crate::field::Fieldset`], [`crate::toast::ToastViewport`] — report a
//! node only when the caller names them, the same shape as [`crate::toolbar::Toolbar`].

use gpui::{SharedString, StatefulInteractiveElement};

pub use gpui::accesskit::{AriaCurrent, Role, Toggled};

use crate::validation::Validity;

/// A control's accessible name and description.
///
/// React Aria's `useField` builds `aria-describedby` by concatenating the
/// description node's id and the field-error node's id
/// (`react-aria/dist/private/label/useField.js`); the resulting accessible
/// description is those two texts, in that order. gpui takes the text
/// directly, so [`Self::field`] performs the same concatenation on the strings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Name {
    label: Option<SharedString>,
    description: Option<SharedString>,
}

impl Name {
    /// No accessible name of its own — the node is named by its contents.
    pub fn none() -> Self {
        Self::default()
    }

    /// A control named by a literal string.
    pub fn labelled(label: impl Into<SharedString>) -> Self {
        Self {
            label: Some(label.into()),
            description: None,
        }
    }

    /// A control named by an optional string, unnamed when it is absent.
    pub fn maybe(label: Option<impl Into<SharedString>>) -> Self {
        Self {
            label: label.map(Into::into),
            description: None,
        }
    }

    /// The name and description a field's own anatomy already computed.
    ///
    /// `label` is the visible [`crate::field::Label`]'s text — React Aria
    /// points `aria-labelledby` at that element, so its text *is* the name.
    /// `description` is the [`crate::field::Description`]'s text, and the
    /// validation messages join it exactly when the field is invalid, which is
    /// exactly when React Aria's `FieldError` renders and contributes its id to
    /// `aria-describedby`.
    pub fn field(
        label: Option<&SharedString>,
        description: Option<&SharedString>,
        validity: &Validity,
    ) -> Self {
        let errors = if validity.is_invalid {
            validity.joined()
        } else {
            String::new()
        };
        let described = match (description.map(SharedString::as_ref), errors.as_str()) {
            (None, "") => None,
            (Some(d), "") => Some(SharedString::from(d.to_owned())),
            (None, e) => Some(SharedString::from(e.to_owned())),
            (Some(d), e) => Some(SharedString::from(format!("{d} {e}"))),
        };
        Self {
            label: label.cloned(),
            description: described,
        }
    }

    /// Replaces the description, for the controls that carry one without a
    /// validation story (a menu subtitle, a stepper hint).
    pub fn described(mut self, description: Option<impl Into<SharedString>>) -> Self {
        self.description = description.map(Into::into);
        self
    }

    /// Prefixes the name, the way `useNumberField` names its steppers
    /// "Increase `{label}`" rather than pointing at the field's label alone.
    pub fn prefixed(&self, prefix: &str) -> Self {
        Self {
            label: Some(match &self.label {
                Some(label) => SharedString::from(format!("{prefix} {label}")),
                None => SharedString::from(prefix.to_owned()),
            }),
            description: None,
        }
    }

    /// The accessible name, if any.
    pub fn label(&self) -> Option<&SharedString> {
        self.label.as_ref()
    }

    /// The accessible description, if any.
    pub fn description(&self) -> Option<&SharedString> {
        self.description.as_ref()
    }

    /// Whether this names anything at all. A control with no name is a control
    /// a screen reader announces by role only, which React Aria warns about in
    /// development (`useToggle`: "you must specify an aria-label").
    pub fn is_empty(&self) -> bool {
        self.label.is_none() && self.description.is_none()
    }
}

/// The numeric range a range-shaped control reports.
///
/// `useProgressBar` clamps the value into the range before reporting it, keeps
/// `aria-valuemin`/`aria-valuemax` unconditionally, and drops
/// `aria-valuenow`/`aria-valuetext` when the control is indeterminate. All four
/// range-shaped controls in this port inherit that from it — a progress bar and
/// circle directly, a meter through `useMeter`, a slider thumb through
/// `useSliderThumb`'s `<input type="range">` — so the rule is written here once.
#[derive(Clone, Debug, PartialEq)]
pub struct Range {
    min: f64,
    max: f64,
    value: Option<f64>,
    step: Option<f64>,
    text: Option<SharedString>,
}

impl Range {
    /// A determinate range. `value` is clamped into `[min, max]`, as
    /// `useProgressBar` clamps it; an inverted range reports `value` untouched
    /// rather than panicking in `f64::clamp`.
    pub fn new(min: f64, max: f64, value: f64) -> Self {
        let value = if min <= max {
            value.clamp(min, max)
        } else {
            value
        };
        Self {
            min,
            max,
            value: Some(value),
            step: None,
            text: None,
        }
    }

    /// An indeterminate range: the bounds still report, the value does not.
    pub fn indeterminate(min: f64, max: f64) -> Self {
        Self {
            min,
            max,
            value: None,
            step: None,
            text: None,
        }
    }

    /// `aria-valuetext` — the human-readable rendering of the value, which
    /// upstream fills with the formatted number (a percentage for a progress
    /// bar, `state.getThumbValueLabel(index)` for a slider thumb).
    pub fn text(mut self, text: Option<impl Into<SharedString>>) -> Self {
        self.text = text.map(Into::into);
        self
    }

    /// The `step` a spin button or slider thumb advances by.
    pub fn step(mut self, step: f64) -> Self {
        self.step = Some(step);
        self
    }

    /// The minimum value of the range.
    pub fn min(&self) -> f64 {
        self.min
    }

    /// The maximum value of the range.
    pub fn max(&self) -> f64 {
        self.max
    }

    /// The current value, if known.
    pub fn value(&self) -> Option<f64> {
        self.value
    }

    /// The keyboard step size, if any.
    pub fn step_size(&self) -> Option<f64> {
        self.step
    }

    /// The human-readable value text, if any.
    pub fn value_text(&self) -> Option<&SharedString> {
        self.text.as_ref()
    }
}

/// The plain text a string-typed label element carries, for the accessible
/// name.
///
/// GPUI derives no accessible name from text children, so a control whose
/// `label(..)` accepts `impl IntoElement` captures the text here when the
/// element is one of the plain string types — `&'static str`, `String` or
/// [`SharedString`]. Richer elements return `None`: their content is not
/// reducible to a name, and the control stays unnamed until the caller passes
/// a string label or an explicit `aria_label`. This is the derived-name half
/// of the name computation — an explicit `aria_label` still wins over it.
pub fn label_text<E: 'static>(el: &E) -> Option<SharedString> {
    let any = el as &dyn std::any::Any;
    if let Some(text) = any.downcast_ref::<&'static str>() {
        return Some(SharedString::from(*text));
    }
    if let Some(text) = any.downcast_ref::<String>() {
        return Some(SharedString::from(text.as_str()));
    }
    if let Some(text) = any.downcast_ref::<SharedString>() {
        return Some(text.clone());
    }
    None
}

/// Writes a control's parity contract onto the element that carries its id.
///
/// Implemented for every [`StatefulInteractiveElement`], which is to say for
/// every element that has an `.id(..)` and can therefore produce an AccessKit
/// node at all. See the module docs for why that bound is the point.
pub trait A11y: StatefulInteractiveElement + Sized {
    /// The control's role. This is the one call `.shots/a11y_audit.py` looks
    /// for, so a component that has a role upstream must spell it here.
    fn a11y(self, role: Role) -> Self {
        self.role(role)
    }

    /// The role together with the name, for the common case.
    fn a11y_named(self, role: Role, name: &Name) -> Self {
        self.a11y(role).a11y_name(name)
    }

    /// `aria-label` and `aria-describedby`'s resolved text.
    fn a11y_name(mut self, name: &Name) -> Self {
        if let Some(label) = name.label() {
            self = self.aria_label(label.clone());
        }
        if let Some(description) = name.description() {
            self = self.aria_description(description.clone());
        }
        self
    }

    /// `aria-checked` for a checkbox, switch or radio.
    ///
    /// `useCheckbox` sets the DOM `indeterminate` property, which is what makes
    /// a native checkbox report `aria-checked="mixed"`; mixed wins over the
    /// checked flag, exactly as it does in the browser.
    fn a11y_checked(self, checked: bool, indeterminate: bool) -> Self {
        self.aria_toggled(match (indeterminate, checked) {
            (true, _) => Toggled::Mixed,
            (false, true) => Toggled::True,
            (false, false) => Toggled::False,
        })
    }

    /// `aria-pressed` for a toggle button (`useToggleButton`).
    fn a11y_pressed(self, pressed: bool) -> Self {
        self.aria_toggled(if pressed {
            Toggled::True
        } else {
            Toggled::False
        })
    }

    /// `aria-valuemin` / `aria-valuemax` / `aria-valuenow` / `aria-valuetext`
    /// / `aria-valuestep`, with the indeterminate case handled by [`Range`].
    fn a11y_range(mut self, range: &Range) -> Self {
        self = self
            .aria_min_numeric_value(range.min())
            .aria_max_numeric_value(range.max());
        if let Some(value) = range.value() {
            self = self.aria_numeric_value(value);
            // `useProgressBar` drops `aria-valuetext` together with
            // `aria-valuenow`: an indeterminate control has no value to word.
            if let Some(text) = range.value_text() {
                self = self.aria_value(text.clone());
            }
        }
        if let Some(step) = range.step_size() {
            self = self.aria_numeric_value_step(step);
        }
        self
    }

    /// `aria-expanded` for a trigger that owns a collapsible region.
    ///
    /// `react-aria/dist/private/disclosure/useDisclosure.js` puts it on the
    /// disclosure's trigger button beside `aria-controls`;
    /// `.../overlays/useOverlayTrigger.js` and `.../menu/useMenuItem.js` put it
    /// on an overlay trigger and a submenu row. Only the flag ports: the
    /// `aria-controls` half of every one of those pairs needs an id graph gpui
    /// does not have (see the module's omission table), so the port states that
    /// the trigger is expanded without being able to say what it expanded.
    fn a11y_expanded(self, expanded: bool) -> Self {
        self.aria_expanded(expanded)
    }

    /// `aria-orientation`, translated from the port's own v3 prop enum so a
    /// call site never has to name two orientation types at once.
    fn a11y_orientation(self, orientation: herogpui_core::Orientation) -> Self {
        self.aria_orientation(match orientation {
            herogpui_core::Orientation::Horizontal => gpui::accesskit::Orientation::Horizontal,
            herogpui_core::Orientation::Vertical => gpui::accesskit::Orientation::Vertical,
        })
    }

    /// A text input's current text and its placeholder.
    fn a11y_text(mut self, value: &str, placeholder: Option<&SharedString>) -> Self {
        self = self.aria_value(SharedString::from(value.to_owned()));
        if let Some(placeholder) = placeholder {
            self = self.aria_placeholder(placeholder.clone());
        }
        self
    }

    /// `aria-selected` for a collection member.
    ///
    /// `react-aria/dist/private/listbox/useOption.mjs` writes it as
    /// `state.selectionManager.selectionMode !== 'none' ? isSelected :
    /// undefined`, and `.../grid/useGridRow.mjs` and
    /// `.../gridlist/useGridListItem.mjs` guard it the same way. The guard is
    /// the caller's, because only the caller knows the mode; what is written
    /// here is the flag itself.
    fn a11y_selected(self, selected: bool) -> Self {
        self.aria_selected(selected)
    }

    /// The current page/step/location value for navigation landmarks.
    ///
    /// Deliberately unwritten: vanilla `gpui-pre` publishes no `aria_current`
    /// builder and no AccessKit node propagation for it (see the omissions
    /// table at the top of this module). The method is kept as a no-op anchor
    /// so call sites and the upstream-PR re-application have one place to
    /// touch: when gpui gains the builder, restore the forwarding line and
    /// remove the `aria-current` row from the omissions table.
    fn a11y_current(self, _current: AriaCurrent) -> Self {
        self
    }

    /// `aria-posinset` / `aria-setsize`, from a **zero-based** index.
    ///
    /// Upstream sets this pair only under virtualization —
    /// `useOption.mjs`'s `if (isVirtualized) { optionProps['aria-posinset'] =
    /// index + 1; optionProps['aria-setsize'] = getItemCount(...) }` — because
    /// that is exactly when the rendered rows are a window onto a longer
    /// collection and the position can no longer be counted from the tree.
    /// The port's virtual list paths are the same situation, so the guard
    /// ports with the attribute. `index + 1` is applied here so no call site
    /// has to remember that ARIA counts from one.
    fn a11y_set_position(self, index: usize, size: usize) -> Self {
        self.aria_position_in_set(index + 1).aria_size_of_set(size)
    }

    /// `aria-level`, from a **zero-based** depth.
    ///
    /// `.../table/useTableRow.mjs` and `.../gridlist/useGridListItem.mjs`
    /// write `'aria-level': node.level + 1` on a tree row, so the same `+ 1`
    /// happens here.
    fn a11y_level(self, depth: usize) -> Self {
        self.aria_level(depth + 1)
    }

    /// `aria-rowcount` / `aria-colcount` on a grid.
    ///
    /// `react-aria/dist/private/grid/useGrid.mjs` sets both only
    /// `if (isVirtualized)`, and `.../table/useTable.mjs` then replaces the
    /// row count with `state.collection.size +
    /// state.collection.headerRows.length` — the header rows count, because
    /// they are rows of the grid. Both numbers are the *whole* collection's,
    /// not the rendered window's; a caller that cannot know the total must
    /// not call this.
    fn a11y_grid_size(self, rows: usize, columns: usize) -> Self {
        self.aria_row_count(rows).aria_column_count(columns)
    }

    /// `aria-rowindex` on a row, from a **zero-based** index.
    ///
    /// `.../grid/useGridRow.mjs`: `if (isVirtualized) rowProps['aria-rowindex']
    /// = node.index + 1; // aria-rowindex is 1 based`.
    fn a11y_row_index(self, index: usize) -> Self {
        self.aria_row_index(index + 1)
    }

    /// `aria-colindex` on a cell, from a **zero-based** index.
    ///
    /// `.../grid/useGridCell.mjs`: `'aria-colindex': node.colIndex != null ?
    /// node.colIndex + 1 : undefined`. Unlike the row index this is *not*
    /// gated on virtualization — a table collection always knows a cell's
    /// column.
    fn a11y_column_index(self, index: usize) -> Self {
        self.aria_column_index(index + 1)
    }

    /// The element assistive technology should treat as focused while an
    /// ancestor holds the real focus.
    ///
    /// This is the one place where gpui's shape is the *inverse* of the web's
    /// and the port is better off for it. `useComboBox.js` puts
    /// `aria-activedescendant` on the **input** — `'aria-activedescendant':
    /// focusedItem ? getItemId(state, focusedItem.key) : undefined` — pointing
    /// at a row id, which needs both an id graph and ownership of the input.
    /// gpui-pre 0.3.3's `aria_active_descendant` takes no argument and is set
    /// on the descendant itself (`elements/div.rs`: "Unlike the web's
    /// container-side `aria-activedescendant`, this is set on the descendant;
    /// GPUI honors it only when a focused ancestor is present in the tree, so
    /// it is safe to set unconditionally on the selected child"). A collection
    /// that owns its rows can therefore state the relation even when it does
    /// not own the element upstream would have written it on.
    fn a11y_active_descendant(self) -> Self {
        self.aria_active_descendant()
    }
}

impl<E: StatefulInteractiveElement> A11y for E {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::resolve;

    fn s(v: &str) -> SharedString {
        SharedString::from(v.to_owned())
    }

    #[test]
    fn a_clean_field_describes_itself_with_its_description_alone() {
        let name = Name::field(
            Some(&s("Email")),
            Some(&s("Work address")),
            &resolve(false, &[], None, None),
        );
        assert_eq!(name.label(), Some(&s("Email")));
        assert_eq!(name.description(), Some(&s("Work address")));
    }

    /// `useField` concatenates the description id and the error id, in that
    /// order, so the announced description is both texts in that order.
    #[test]
    fn an_invalid_field_appends_every_message_after_the_description() {
        let validity = resolve(false, &[s("Already taken")], Some(s("Too short")), None);
        let name = Name::field(Some(&s("Email")), Some(&s("Work address")), &validity);
        assert_eq!(
            name.description(),
            Some(&s("Work address Already taken Too short"))
        );

        // No description of its own: the messages are the whole description.
        let name = Name::field(Some(&s("Email")), None, &validity);
        assert_eq!(name.description(), Some(&s("Already taken Too short")));
    }

    /// `FieldError` renders only while the field is invalid, so a stale
    /// message must not reach the node once validity is restored.
    #[test]
    fn a_valid_field_announces_no_message() {
        let mut validity = resolve(false, &[s("Already taken")], None, None);
        validity.is_invalid = false;
        let name = Name::field(None, Some(&s("Work address")), &validity);
        assert_eq!(name.description(), Some(&s("Work address")));
    }

    #[test]
    fn an_unnamed_undescribed_field_is_empty() {
        let name = Name::field(None, None, &resolve(false, &[], None, None));
        assert!(name.is_empty());
        assert!(!Name::labelled("Close").is_empty());
        assert!(Name::none().is_empty());
        assert!(Name::maybe(None::<SharedString>).is_empty());
    }

    /// `useNumberField` names its steppers "Increase {label}", falling back to
    /// the bare verb when the field has no label at all.
    #[test]
    fn a_stepper_prefixes_the_fields_name() {
        let field = Name::labelled("Quantity");
        assert_eq!(
            field.prefixed("Increase").label(),
            Some(&s("Increase Quantity"))
        );
        assert_eq!(
            Name::none().prefixed("Increase").label(),
            Some(&s("Increase"))
        );
    }

    /// `useProgressBar` clamps before reporting, so a caller's out-of-range
    /// value never reaches the node.
    #[test]
    fn a_range_clamps_its_value_the_way_use_progress_bar_does() {
        assert_eq!(Range::new(0., 100., 150.).value(), Some(100.));
        assert_eq!(Range::new(0., 100., -5.).value(), Some(0.));
        assert_eq!(Range::new(0., 100., 42.).value(), Some(42.));
        // An inverted range would panic in `f64::clamp`; report it untouched.
        assert_eq!(Range::new(10., 0., 5.).value(), Some(5.));
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn an_indeterminate_range_keeps_its_bounds_and_drops_its_value() {
        let range = Range::indeterminate(0., 100.).text(Some("ignored"));
        assert_eq!(range.min(), 0.);
        assert_eq!(range.max(), 100.);
        assert_eq!(range.value(), None);
        // The text is still stored, but `a11y_range` writes it only beside a
        // value, so an indeterminate control never announces one.
        assert_eq!(range.value_text(), Some(&s("ignored")));
    }

    #[test]
    fn a_range_carries_its_step_and_value_text() {
        let range = Range::new(1., 10., 4.).step(0.5).text(Some("4 items"));
        assert_eq!(range.step_size(), Some(0.5));
        assert_eq!(range.value_text(), Some(&s("4 items")));
        assert_eq!(Range::new(1., 10., 4.).step_size(), None);
    }

    /// `label(..)` captures plain string text for the accessible name and
    /// leaves every other element unnamed.
    #[test]
    fn label_text_reads_the_three_string_types_only() {
        assert_eq!(label_text(&"Hello"), Some(s("Hello")));
        assert_eq!(label_text(&String::from("Owned")), Some(s("Owned")));
        assert_eq!(label_text(&s("Shared")), Some(s("Shared")));
        // A composed element and a non-string value are not reducible.
        assert_eq!(label_text(&gpui::div()), None);
        assert_eq!(label_text(&42), None);
    }
}
