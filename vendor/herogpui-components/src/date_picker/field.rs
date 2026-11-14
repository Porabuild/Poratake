//! DateField.

use super::*;

// DateField (segmented)
// ---------------------------------------------------------------------------

/// One editable part of a [`DateField`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateSegment {
    /// The month segment.
    Month,
    /// The day segment.
    Day,
    /// The year segment.
    Year,
}

impl DateSegment {
    /// All date segments in canonical month/day/year order.
    pub const ALL: [DateSegment; 3] = [DateSegment::Month, DateSegment::Day, DateSegment::Year];

    /// The lowercase segment name (`month`, `day` or `year`).
    pub fn label(self) -> &'static str {
        match self {
            DateSegment::Month => "month",
            DateSegment::Day => "day",
            DateSegment::Year => "year",
        }
    }

    /// The segment's accessible name in the active locale (`year`, `month`,
    /// `day` in en-US; React Aria's datepicker dictionary).
    pub(crate) fn a11y_label(self, cx: &App) -> SharedString {
        use crate::i18n::{ui_string, UiString};
        ui_string(
            match self {
                DateSegment::Month => UiString::Month,
                DateSegment::Day => UiString::Day,
                DateSegment::Year => UiString::Year,
            },
            cx,
        )
    }

    /// The placeholder this segment shows with no value, sized like its digits.
    pub(super) fn hint(self) -> &'static str {
        match self {
            DateSegment::Month => "mm",
            DateSegment::Day => "dd",
            DateSegment::Year => "yyyy",
        }
    }

    /// How many digits this segment holds — the point at which typing moves on.
    pub(super) fn digits(self) -> usize {
        match self {
            DateSegment::Year => 4,
            _ => 2,
        }
    }

    pub(super) fn page_step(self) -> i32 {
        match self {
            DateSegment::Year => 5,
            DateSegment::Month => 2,
            DateSegment::Day => 7,
        }
    }

    /// `date` with this segment set to `value`, clamped to what the calendar
    /// allows (February 31st becomes the 28th or 29th).
    pub(super) fn with_value(self, date: Date, value: u32) -> Date {
        match self {
            DateSegment::Year => {
                let year = value as i32;
                let day = date
                    .day
                    .min(crate::calendar::days_in_month(year, date.month));
                Date::new(year, date.month, day)
            }
            DateSegment::Month => {
                let month = value.clamp(1, 12);
                let day = date
                    .day
                    .min(crate::calendar::days_in_month(date.year, month));
                Date::new(date.year, month, day)
            }
            DateSegment::Day => Date::new(
                date.year,
                date.month,
                value.clamp(1, crate::calendar::days_in_month(date.year, date.month)),
            ),
        }
    }

    /// `date` with this segment moved by `delta`, keeping the result a real
    /// calendar date (31 January + 1 month is the end of February, not the 31st).
    pub(super) fn bump(self, date: Date, delta: i32) -> Date {
        match self {
            DateSegment::Year => {
                let year = cycle_value(date.year, delta, 1, 9999);
                let day = date
                    .day
                    .min(crate::calendar::days_in_month(year, date.month));
                Date::new(year, date.month, day)
            }
            DateSegment::Month => {
                let month = cycle_value(date.month as i32, delta, 1, 12) as u32;
                let day = date
                    .day
                    .min(crate::calendar::days_in_month(date.year, month));
                Date::new(date.year, month, day)
            }
            DateSegment::Day => {
                let days = crate::calendar::days_in_month(date.year, date.month);
                let day = cycle_value(date.day as i32, delta, 1, days as i32) as u32;
                Date::new(date.year, date.month, day)
            }
        }
    }

    pub(super) fn bound(self, date: Date, maximum: bool) -> Date {
        match (self, maximum) {
            (DateSegment::Year, false) => {
                let day = date.day.min(crate::calendar::days_in_month(1, date.month));
                Date::new(1, date.month, day)
            }
            (DateSegment::Year, true) => {
                let day = date
                    .day
                    .min(crate::calendar::days_in_month(9999, date.month));
                Date::new(9999, date.month, day)
            }
            (DateSegment::Month, false) => {
                let day = date.day.min(crate::calendar::days_in_month(date.year, 1));
                Date::new(date.year, 1, day)
            }
            (DateSegment::Month, true) => {
                let day = date.day.min(crate::calendar::days_in_month(date.year, 12));
                Date::new(date.year, 12, day)
            }
            (DateSegment::Day, false) => Date::new(date.year, date.month, 1),
            (DateSegment::Day, true) => Date::new(
                date.year,
                date.month,
                crate::calendar::days_in_month(date.year, date.month),
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RegionalDateFormat {
    order: [DateSegment; 3],
    literals: [String; 4],
    month_has_leading_zero: bool,
    day_has_leading_zero: bool,
}

impl RegionalDateFormat {
    pub(super) fn for_locale(locale: &str) -> Option<Self> {
        use icu_datetime::{
            fieldsets,
            input::Date as IcuDate,
            options::YearStyle,
            provider::{
                fields::{FieldLength, FieldSymbol},
                pattern::{reference, runtime, PatternItem},
            },
            DateTimeFormatter,
        };
        use icu_locale_core::Locale as IcuLocale;

        let locale = locale.parse::<IcuLocale>().ok()?;
        let formatter = DateTimeFormatter::try_new(
            locale.into(),
            fieldsets::YMD::short().with_year_style(YearStyle::Full),
        )
        .ok()?;
        let formatted = formatter.format(&IcuDate::try_new_iso(2000, 1, 1).ok()?);
        let pattern: runtime::Pattern<'_> = formatted.pattern().into();
        let mut order = Vec::with_capacity(3);
        let mut literals = Vec::with_capacity(4);
        let mut literal = String::new();
        let mut month_has_leading_zero = None;
        let mut day_has_leading_zero = None;
        for item in reference::Pattern::from(&pattern).into_items() {
            let field = match item {
                PatternItem::Literal(ch) => {
                    literal.push(ch);
                    continue;
                }
                PatternItem::Field(field) => field,
            };
            let segment = match field.symbol {
                FieldSymbol::Month(_) => {
                    month_has_leading_zero = Some(field.length == FieldLength::Two);
                    DateSegment::Month
                }
                FieldSymbol::Day(_) => {
                    day_has_leading_zero = Some(field.length == FieldLength::Two);
                    DateSegment::Day
                }
                FieldSymbol::Year(_) => DateSegment::Year,
                _ => return None,
            };
            if order.contains(&segment) {
                return None;
            }
            literals.push(std::mem::take(&mut literal));
            order.push(segment);
        }
        literals.push(literal);
        Some(Self {
            order: order.try_into().ok()?,
            literals: literals.try_into().ok()?,
            month_has_leading_zero: month_has_leading_zero?,
            day_has_leading_zero: day_has_leading_zero?,
        })
    }

    pub(super) fn for_preferences(tags: &[String]) -> Option<Self> {
        tags.iter().find_map(|tag| Self::for_locale(tag))
    }
}

pub(super) fn system_date_format() -> &'static RegionalDateFormat {
    static SYSTEM_DATE_FORMAT: OnceLock<RegionalDateFormat> = OnceLock::new();
    SYSTEM_DATE_FORMAT.get_or_init(|| {
        RegionalDateFormat::for_preferences(&crate::date_constraints::system_locale_tags())
            .unwrap_or(RegionalDateFormat {
                order: DateSegment::ALL,
                literals: [String::new(), "/".to_owned(), "/".to_owned(), String::new()],
                month_has_leading_zero: false,
                day_has_leading_zero: false,
            })
    })
}

pub(super) fn cycle_value(value: i32, delta: i32, min: i32, max: i32) -> i32 {
    (value - min + delta).rem_euclid(max - min + 1) + min
}

/// `granularity` — the smallest unit a date field shows.
///
/// v3 defaults a date to `day`; anything smaller adds the time segments, which
/// is why its own example switches `defaultValue` from `parseDate` to
/// `parseZonedDateTime` when the granularity drops below a day.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Granularity {
    /// Day precision, with no time segments.
    #[default]
    Day,
    /// Hour precision.
    Hour,
    /// Minute precision.
    Minute,
    /// Second precision.
    Second,
}

impl Granularity {
    /// Every granularity, from coarsest to finest.
    pub const ALL: [Granularity; 4] = [
        Granularity::Day,
        Granularity::Hour,
        Granularity::Minute,
        Granularity::Second,
    ];

    /// The human-readable name of this granularity.
    pub fn label(self) -> &'static str {
        match self {
            Granularity::Day => "Day",
            Granularity::Hour => "Hour",
            Granularity::Minute => "Minute",
            Granularity::Second => "Second",
        }
    }

    /// The time granularity this asks for, or `None` for a plain date.
    pub(super) fn time(self) -> Option<crate::time_field::TimeGranularity> {
        use crate::time_field::TimeGranularity as T;
        match self {
            Granularity::Day => None,
            Granularity::Hour => Some(T::Hour),
            Granularity::Minute => Some(T::Minute),
            Granularity::Second => Some(T::Second),
        }
    }
}

/// One editable slot of a date field: a date part, or -- below `day`
/// granularity -- a time part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldSegment {
    /// A date segment (month, day or year).
    Date(DateSegment),
    /// A time segment, present below `day` granularity.
    Time(crate::time_field::TimeSegment),
}

#[derive(Clone)]
pub(super) struct DateFieldDisplay {
    date: Option<Date>,
    time: Option<crate::time_field::Time>,
    cleared: Vec<FieldSegment>,
    committed: String,
}

impl DateFieldDisplay {
    pub(super) fn new(committed: &str, segments: &[FieldSegment]) -> Self {
        let (date, time) = parse_value(committed);
        let mut this = Self {
            date,
            time,
            cleared: Vec::new(),
            committed: committed.to_owned(),
        };
        this.sync_visible_segments(segments);
        this
    }

    pub(super) fn sync(&mut self, committed: &str, segments: &[FieldSegment]) {
        if self.committed != committed {
            let (date, time) = parse_value(committed);
            self.date = date;
            self.time = time;
            self.cleared.clear();
            self.committed = committed.to_owned();
        }
        self.sync_visible_segments(segments);
    }

    pub(super) fn sync_visible_segments(&mut self, segments: &[FieldSegment]) {
        self.cleared.retain(|segment| segments.contains(segment));
        for segment in segments {
            let missing = match segment {
                FieldSegment::Date(_) => self.date.is_none(),
                FieldSegment::Time(_) => self.time.is_none(),
            };
            if missing && !self.cleared.contains(segment) {
                self.cleared.push(*segment);
            }
        }
    }

    pub(super) fn edit(
        &mut self,
        focused: FieldSegment,
        date: Date,
        time: Option<crate::time_field::Time>,
        segments: &[FieldSegment],
        granularity: Granularity,
    ) -> Option<(Date, Option<crate::time_field::Time>, String)> {
        self.date = Some(date);
        self.time = time;
        self.cleared.retain(|segment| *segment != focused);
        if segments
            .iter()
            .any(|segment| self.cleared.contains(segment))
        {
            return None;
        }

        let committed = format_value(date, time, granularity);
        self.committed.clone_from(&committed);
        Some((date, time, committed))
    }

    pub(super) fn clear(&mut self, focused: FieldSegment, segments: &[FieldSegment]) -> bool {
        if !self.cleared.contains(&focused) {
            self.cleared.push(focused);
        }
        if !segments
            .iter()
            .all(|segment| self.cleared.contains(segment))
        {
            return false;
        }

        self.date = None;
        self.time = None;
        self.committed.clear();
        true
    }
}

pub(super) type FieldSegmentRender =
    std::sync::Arc<dyn Fn(FieldSegment, SharedString) -> gpui::AnyElement + 'static>;

/// State supplied to v3's DateField children render function.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct DateFieldRenderState {
    /// Whether the field is disabled.
    pub is_disabled: bool,
    /// Whether controlled, server, custom or constraint validation is invalid.
    pub is_invalid: bool,
    /// Whether segments can be focused but not edited.
    pub is_read_only: bool,
    /// Whether the field is required.
    pub is_required: bool,
    /// Whether the field's input owns focus.
    pub is_focused: bool,
    /// Whether focus is inside the field.
    pub is_focus_within: bool,
    /// Whether keyboard-visible focus chrome should be shown.
    pub is_focus_visible: bool,
}

/// v3's DateField: three editable segments (month / day / year), with the ISO
/// text kept in the bound `InputState` so the form and `onChange` still see a
/// plain date string.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct DateField {
    /// See [`DateField::content`].
    content: Option<std::sync::Arc<dyn Fn(DateFieldRenderState) -> gpui::AnyElement + 'static>>,
    /// `segment` — v3's render prop for one editable date or time segment,
    /// handed which segment it is and the text the field would show.
    segment: Option<FieldSegmentRender>,
    /// `validationBehavior` — written into the text state on render.
    validation_behavior: Option<crate::form::ValidationBehavior>,
    /// `defaultValue` — seeds the text state on the first render only.
    default_value: Option<Date>,
    /// `value` — v3's controlled date, stored for the first render only and
    /// seeded into the text state as ISO. The outer `Option` distinguishes an
    /// unset builder from `value(null)`, the explicitly controlled empty date.
    value: Option<Option<Date>>,
    full_width: bool,
    is_required: bool,
    /// `validate` — run by the component, not the caller.
    validate: Option<crate::validation::Validator<Option<Date>>>,
    /// `validationErrors` — messages from a server round-trip.
    validation_errors: Vec<SharedString>,
    is_invalid: bool,
    variant: herogpui_core::FieldVariant,
    constraints: DateConstraints,
    placeholder_value: Option<Date>,
    /// `granularity` — `day`, or a time unit, in which case the field grows the
    /// segments for it and the bound state holds an ISO date-and-time.
    granularity: Granularity,
    /// `hourCycle` — 12- or 24-hour, for the hour segment `granularity` adds.
    hour_cycle: crate::time_field::HourCycle,
    /// `DateField.Prefix` — content before the segments, drawn in the
    /// placeholder colour and inert (`pointer-events-none`).
    prefix: Option<gpui::AnyElement>,
    /// `DateField.Suffix` — content after the segments.
    suffix: Option<gpui::AnyElement>,
    state: Entity<crate::input::InputState>,
    label: Option<SharedString>,
    /// `Description` — composed inside the field in v3's own example.
    description: Option<SharedString>,
    /// `name` — the name this field submits under.
    name: Option<SharedString>,
    /// `autoFocus` — take focus on the first render.
    auto_focus: bool,
    /// `shouldForceLeadingZeros` — force month, day and hour to two digits
    /// instead of using the system regional format.
    should_force_leading_zeros: bool,
    is_disabled: bool,
    is_read_only: bool,
    on_change: Option<OnChange>,
    embedded: bool,
    bare: bool,
    /// Public chrome-only bare mode: keeps the box geometry, drops the paint.
    is_bare: bool,
    /// Whether focused state paints the visual focus ring.
    focus_ring: bool,
    /// The family the segments are drawn with; unset keeps the mono token.
    font_family: Option<SharedString>,
    /// The corner radius, in place of the owning `field_radius` helper.
    radius: Option<Pixels>,
    /// Explicit single-line box height; `None` is the 36px stock box.
    height: Option<Pixels>,
    /// Explicit horizontal box padding; `None` is `px-3`.
    padding_x: Option<Pixels>,
    on_picker_open: Option<std::sync::Arc<dyn Fn(&mut Window, &mut App) + 'static>>,
    report_invalid_changes: bool,
    /// The locale whose date order, separators and padding the segments use.
    /// Absent means the OS regional format (`system_date_format`).
    locale: Option<SharedString>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl DateField {
    /// `fullWidth`
    pub fn full_width(mut self, v: bool) -> Self {
        self.full_width = v;
        self
    }

    /// `isRequired`
    /// `Description` — help text under the field.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// `name` — the name this field submits under.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// The `Form` field this control submits, when it has a `name`.
    pub fn form_field(&self) -> Option<crate::form::FormField> {
        let name = self.name.clone()?;
        let form_state = date_field_form_state(self.state.entity_id().as_u64());
        form_state.borrow_mut().is_successful = !self.is_disabled;
        if let Some(default) = self.default_value {
            install_date_field_restore(
                &form_state,
                self.state.clone(),
                default.format_iso().into(),
            );
        }
        let mut field =
            crate::form::FormField::live(name, form_state).is_required(self.is_required);
        if let Some(behavior) = self.validation_behavior {
            field = field.validation_behavior(behavior);
        }
        Some(field)
    }

    /// `autoFocus` — take focus on the first render.
    pub fn auto_focus(mut self, v: bool) -> Self {
        self.auto_focus = v;
        self
    }

    /// `shouldForceLeadingZeros` — force month, day and hour to two digits.
    /// Without this flag those segments follow the system regional format;
    /// minute and second segments are always two digits.
    pub fn should_force_leading_zeros(mut self, v: bool) -> Self {
        self.should_force_leading_zeros = v;
        self
    }

    /// `isDisabled` — greys the field out and stops it answering keys.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// `isReadOnly` — shows the value but refuses edits.
    pub fn is_read_only(mut self, v: bool) -> Self {
        self.is_read_only = v;
        self
    }

    /// Sets whether the field is required (`isRequired`).
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// `placeholderValue` — the date the empty field formats its hint from.
    pub fn placeholder_value(mut self, date: Date) -> Self {
        self.placeholder_value = Some(date);
        self
    }

    /// `granularity` — the smallest unit the field shows.
    ///
    /// Below `day` the field grows the time segments and the bound state holds
    /// an ISO date-and-time (`2025-02-03T08:45`), which is the value a form
    /// submits; `on_change` still reports the date part.
    pub fn granularity(mut self, granularity: Granularity) -> Self {
        self.granularity = granularity;
        self
    }

    /// `hourCycle` — whether the hour segment `granularity` adds is 12- or
    /// 24-hour.
    pub fn hour_cycle(mut self, cycle: crate::time_field::HourCycle) -> Self {
        self.hour_cycle = cycle;
        self
    }

    /// `DateField.Prefix` — content before the segments.
    pub fn prefix(mut self, el: impl IntoElement) -> Self {
        self.prefix = Some(el.into_any_element());
        self
    }

    /// `DateField.Suffix` — content after the segments.
    pub fn suffix(mut self, el: impl IntoElement) -> Self {
        self.suffix = Some(el.into_any_element());
        self
    }

    /// `validate` — returns the message to show, or `None` when the date is fine.
    ///
    /// The component runs it and surfaces the result.
    pub fn validate(mut self, f: impl Fn(&Option<Date>) -> Option<SharedString> + 'static) -> Self {
        self.validate = Some(std::sync::Arc::new(f));
        self
    }

    /// `validationErrors` — messages produced elsewhere, shown ahead of
    /// whatever `validate` returns.
    pub fn validation_errors(
        mut self,
        errors: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.validation_errors = errors.into_iter().map(Into::into).collect();
        self
    }

    /// `isInvalid` — forces the danger treatment regardless of the text.
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// `variant` — `Secondary` drops the field shadow.
    pub fn variant(mut self, variant: herogpui_core::FieldVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Replaces the 36px box height. Only the single-line box changes: the
    /// segments keep their 14px type and 20px line and stay centred.
    pub fn height(mut self, h: impl Into<Pixels>) -> Self {
        self.height = Some(h.into());
        self
    }

    /// Replaces the box's `px-3` horizontal padding.
    pub fn padding_x(mut self, p: impl Into<Pixels>) -> Self {
        self.padding_x = Some(p.into());
        self
    }

    /// Renders the box with no background, border, field shadow or focus ring,
    /// for a caller painting around it. The field keeps its box geometry and
    /// stays editable and focusable.
    pub fn is_bare(mut self, v: bool) -> Self {
        self.is_bare = v;
        self
    }

    /// Shows or hides only the segmented field's visual focus ring. The field
    /// remains focusable and editable when set to `false`.
    pub fn focus_ring(mut self, v: bool) -> Self {
        self.focus_ring = v;
        self
    }

    /// The family the segments are drawn with; unset keeps the mono token.
    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// The corner radius, in place of the owning `field_radius` helper. Not a
    /// v3 prop; the removed v2 `radius` prop is prohibited and this is a
    /// per-component repository extension.
    ///
    /// The box and shared field chrome use the same resolved radius. A bare
    /// field retains it without painting chrome.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// `value` — v3's controlled-date spelling, as a pure builder.
    ///
    /// The bound [`crate::InputState`] owns the ISO text once the field
    /// renders, so this seeds the state on the first render only, winning
    /// over [`DateField::default_value`] the way v3's controlled prop
    /// outranks the uncontrolled seed; calling `.value(..)` twice keeps the
    /// last call, like every other builder here. `None` is v3's `null` — an
    /// explicitly controlled empty date, seeded as empty text, which still
    /// outranks `default_value`. A later date is an imperative update rather
    /// than a builder: `state.update(cx, |s, _| s.set_value(..))`.
    pub fn value(mut self, date: Option<Date>) -> Self {
        self.value = Some(date);
        self
    }

    /// `minValue` — the earliest date the field accepts.
    pub fn min_value(mut self, date: Date) -> Self {
        self.constraints.min_value = Some(date);
        self
    }

    /// `maxValue` — the latest date the field accepts.
    pub fn max_value(mut self, date: Date) -> Self {
        self.constraints.max_value = Some(date);
        self
    }

    /// `isDateUnavailable` — rejects individual dates inside the range.
    pub fn is_date_unavailable(mut self, f: impl Fn(Date) -> bool + 'static) -> Self {
        self.constraints.is_date_unavailable = Some(std::sync::Arc::new(f));
        self
    }

    /// All the date constraints at once, for callers that already hold a set.
    pub fn constraints(mut self, constraints: DateConstraints) -> Self {
        self.constraints = constraints;
        self
    }

    /// v3's field `children`-as-a-function, handed the complete
    /// [`DateFieldRenderState`].
    pub fn content(
        mut self,
        render: impl Fn(DateFieldRenderState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.content = Some(std::sync::Arc::new(render));
        self
    }

    /// Creates a date field backed by the input `state`.
    pub fn new(state: Entity<crate::input::InputState>) -> Self {
        Self {
            content: None,
            segment: None,
            granularity: Granularity::Day,
            hour_cycle: crate::time_field::HourCycle::default(),
            validation_behavior: None,
            default_value: None,
            value: None,
            full_width: false,
            is_required: false,
            validate: None,
            validation_errors: Vec::new(),
            is_invalid: false,
            variant: herogpui_core::FieldVariant::Primary,
            constraints: DateConstraints::new(),
            placeholder_value: None,
            prefix: None,
            suffix: None,
            state,
            label: None,
            description: None,
            name: None,
            auto_focus: false,
            should_force_leading_zeros: false,
            is_disabled: false,
            is_read_only: false,
            on_change: None,
            embedded: false,
            bare: false,
            is_bare: false,
            focus_ring: true,
            font_family: None,
            radius: None,
            height: None,
            padding_x: None,
            on_picker_open: None,
            report_invalid_changes: false,
            locale: None,
            sx: None,
        }
    }

    /// The locale whose date order, separators and padding the segments use.
    ///
    /// v3 sets this through `<I18nProvider locale=...>`; gpui has no subtree
    /// context, so the tag is a builder on the field the provider would have
    /// wrapped. Absent, the OS regional format wins.
    pub fn locale(mut self, tag: impl Into<SharedString>) -> Self {
        self.locale = Some(tag.into());
        self
    }

    pub(super) fn embedded(mut self, bare: bool) -> Self {
        self.embedded = true;
        self.bare = bare;
        self
    }

    pub(super) fn on_picker_open(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_picker_open = Some(std::sync::Arc::new(f));
        self
    }

    pub(super) fn report_invalid_changes(mut self) -> Self {
        self.report_invalid_changes = true;
        self
    }

    /// `segment` — replaces the contents of each editable segment.
    ///
    /// The closure receives which [`FieldSegment`] it is drawing and the text
    /// the field would have shown, including time segments below day
    /// granularity.
    pub fn segment(
        mut self,
        render: impl Fn(FieldSegment, SharedString) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.segment = Some(std::sync::Arc::new(render));
        self
    }

    /// `validationBehavior` — see [`crate::input::Input::validation_behavior`].
    pub fn validation_behavior(mut self, behavior: crate::form::ValidationBehavior) -> Self {
        self.validation_behavior = Some(behavior);
        self
    }

    /// `defaultValue` — the uncontrolled initial date.
    ///
    /// Written into the state on the first render only, so it seeds the
    /// component without fighting the user afterwards.
    pub fn default_value(mut self, value: Date) -> Self {
        self.default_value = Some(value);
        self
    }

    /// Sets the label shown above the field.
    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = Some(l.into());
        self
    }

    /// Sets the handler called when the date changes (`onChange`).
    pub fn on_change(mut self, f: impl Fn(&Option<Date>, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(std::sync::Arc::new(f));
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the field's root element after every value the variant and
    /// the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

pub(super) fn parse_iso(text: &str) -> Option<Date> {
    let parts: Vec<&str> = text.trim().split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y: i32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    let d: u32 = parts[2].parse().ok()?;
    if !(1..=12).contains(&m) || d == 0 || d > crate::calendar::days_in_month(y, m) {
        return None;
    }
    Some(Date::new(y, m, d))
}

/// A date field's value: the date, and the time when `granularity` asks for
/// one. `T` or a space separates them, as ISO 8601 allows both.
pub(super) fn parse_value(text: &str) -> (Option<Date>, Option<crate::time_field::Time>) {
    let text = text.trim();
    let (date, time) = match text.split_once(['T', ' ']) {
        Some((d, t)) => (d, Some(t)),
        None => (text, None),
    };
    (parse_iso(date), time.and_then(parse_time))
}

/// `HH`, `HH:MM` or `HH:MM:SS`. A missing part is zero, which is what makes an
/// hour-granularity value round-trip.
pub(super) fn parse_time(text: &str) -> Option<crate::time_field::Time> {
    let mut parts = text.trim().split(':');
    let hour: u32 = parts.next()?.parse().ok()?;
    let minute: u32 = match parts.next() {
        Some(m) => m.parse().ok()?,
        None => 0,
    };
    let second: u32 = match parts.next() {
        Some(sec) => sec.parse().ok()?,
        None => 0,
    };
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(crate::time_field::Time::new(hour, minute).with_second(second))
}

/// The text the state holds, which is also what a form submits: an ISO date,
/// widened by exactly as much time as the granularity shows.
pub(super) fn format_value(
    date: Date,
    time: Option<crate::time_field::Time>,
    granularity: Granularity,
) -> String {
    let t = time.unwrap_or_default();
    match granularity {
        Granularity::Day => date.format_iso(),
        Granularity::Hour => format!("{}T{:02}", date.format_iso(), t.hour),
        Granularity::Minute => format!("{}T{:02}:{:02}", date.format_iso(), t.hour, t.minute),
        Granularity::Second => format!(
            "{}T{:02}:{:02}:{:02}",
            date.format_iso(),
            t.hour,
            t.minute,
            t.second
        ),
    }
}

impl RenderOnce for DateField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let entity_id = self.state.entity_id().as_u64();
        // Every keyed slot below hangs off this one base, so no two of them can
        // flatten into a shared key. The segments keep their own base, whose
        // name has always differed from this one.
        let base_id = gpui::ElementId::named_usize("datefield", entity_id as usize);
        let segment_base_id = gpui::ElementId::named_usize("date", entity_id as usize);
        let form_state = registered_date_field_form_state(entity_id);
        if let Some(form_state) = form_state.as_ref() {
            let mut state = form_state.borrow_mut();
            state.value =
                crate::form::FormValue::Text(self.state.read(cx).value().to_owned().into());
            state.is_successful = !self.is_disabled;
            state.focus = Some(self.state.read(cx).focus_handle.clone());
            if let Some(default) = self.default_value {
                drop(state);
                install_date_field_restore(
                    form_state,
                    self.state.clone(),
                    default.format_iso().into(),
                );
            }
        }
        // `validationBehavior` travels with the name, on the text state.
        if let Some(behavior) = self.validation_behavior {
            if self.state.read(cx).validation_behavior() != behavior {
                self.state
                    .update(cx, |s, _| s.set_validation_behavior(behavior));
            }
        }
        // `value` / `defaultValue` seed the state once, before anything reads
        // it. `value` is v3's controlled spelling, so it outranks the
        // uncontrolled seed; the state owns the text afterwards, and
        // `InputState::set_value` is the imperative update.
        if let Some(date) = self.value {
            let text = date.map(|d| d.format_iso()).unwrap_or_default();
            let state = self.state.clone();
            crate::util::seed_once(
                window,
                cx,
                gpui::ElementId::named_usize(
                    "datefield-default",
                    self.state.entity_id().as_u64() as usize,
                ),
                move |cx| {
                    state.update(cx, |s, cx| {
                        s.set_value(text);
                        cx.notify();
                    });
                },
            );
        } else if let Some(value) = self.default_value {
            let state = self.state.clone();
            crate::util::seed_once(
                window,
                cx,
                gpui::ElementId::named_usize(
                    "datefield-default",
                    self.state.entity_id().as_u64() as usize,
                ),
                move |cx| {
                    state.update(cx, |s, cx| {
                        s.set_value(value.format_iso());
                        cx.notify();
                    });
                },
            );
        }

        let regional_owned = self
            .locale
            .as_deref()
            .and_then(RegionalDateFormat::for_locale);
        let regional_date = regional_owned
            .as_ref()
            .unwrap_or_else(|| system_date_format());

        // Which segment the arrows and typing act on. `use_keyed_state` takes
        // `cx` mutably, so this precedes the theme tokens.
        let first_date_segment = regional_date.order[0];
        let focused_seg =
            window.use_keyed_state(element_id::scoped(&base_id, "seg"), cx, move |_, _| {
                FieldSegment::Date(first_date_segment)
            });
        let mut focused = *focused_seg.read(cx);
        // Digits typed into the focused segment but not yet complete, so `1` in
        // the month segment can still become `12`. Cleared whenever focus moves.
        let typing = window.use_keyed_state(element_id::scoped(&base_id, "typing"), cx, |_, _| {
            String::new()
        });

        let colors = cx.colors().clone();
        let navigable = !self.is_disabled;

        let text = self.state.read(cx).value().to_owned();
        let (parsed, _) = parse_value(&text);
        let non_empty = !text.trim().is_empty();

        let twelve_hour = self.hour_cycle == crate::time_field::HourCycle::H12;
        let regional_time = self.granularity.time().map(|granularity| {
            crate::time_field::regional_time_pattern(granularity, self.hour_cycle)
        });
        let mut segments: Vec<FieldSegment> = regional_date
            .order
            .iter()
            .copied()
            .map(FieldSegment::Date)
            .collect();
        if let Some(regional_time) = regional_time.as_ref() {
            segments.extend(regional_time.order.iter().copied().map(FieldSegment::Time));
        }
        // A narrower granularity can leave the caret on a slot that is gone.
        if !segments.contains(&focused) {
            focused = segments[0];
        }
        let granularity = self.granularity;
        let display =
            window.use_keyed_state(element_id::scoped(&base_id, "display"), cx, |_, _| {
                DateFieldDisplay::new(&text, &segments)
            });
        display.update(cx, |display, _| display.sync(&text, &segments));
        let (display_date, display_time, cleared) = {
            let display = display.read(cx);
            (display.date, display.time, display.cleared.clone())
        };

        // The three ways a date can be wrong are reported separately, as the
        // calendar grids distinguish them too.
        let rejection = if !non_empty {
            None
        } else if parsed.is_none() {
            Some("Enter a valid date.".to_owned())
        } else {
            let date = parsed.expect("checked above");
            if self.constraints.out_of_range(date) {
                Some(
                    match (self.constraints.min_value, self.constraints.max_value) {
                        (Some(min), Some(max)) => format!(
                            "Pick a date between {} and {}.",
                            min.format_iso(),
                            max.format_iso()
                        ),
                        (Some(min), None) => format!("Pick {} or later.", min.format_iso()),
                        (None, Some(max)) => format!("Pick {} or earlier.", max.format_iso()),
                        (None, None) => "Date is out of range.".to_owned(),
                    },
                )
            } else if self.constraints.is_unavailable(date) {
                Some("That date is unavailable.".to_owned())
            } else {
                None
            }
        };

        // v3 order: the controlled flag, then server errors, then `validate`,
        // then whichever constraint the date breaks.
        let validate_result = self.validate.as_ref().and_then(|f| f(&parsed));
        let constraint_msg: Option<SharedString> = rejection.map(Into::into);
        let validity = crate::validation::resolve(
            self.is_invalid,
            &self.validation_errors,
            validate_result.clone(),
            constraint_msg.clone(),
        );
        // React Stately's commit-gated display: `validate` and the built-in
        // constraint text are the pending half, synced into the bound
        // `InputState`'s snapshot every render; focus leaving the field after
        // a change commits it. `display_validity` composes what paints — the
        // controlled flag and server messages stay live, the gated half is
        // the snapshot.
        let validity_state = self.state.clone();
        let pending = crate::validation::resolve(false, &[], validate_result, constraint_msg);
        validity_state.update(cx, |state, _| state.set_pending(pending));

        let focus_handle = self.state.read(cx).focus_handle.clone();
        // Blur-within commits only when the parsed date changed since focus
        // entered; moving between segments keeps focus within, so it never
        // commits. Runs before `display_validity` reads `displayed`, so the
        // committing frame already paints the fresh snapshot.
        if let Some(arrival) = crate::validation::left_focus(
            element_id::scoped(&base_id, "left-focus"),
            focus_handle.contains_focused(window, cx),
            &parsed,
            window,
            cx,
        ) {
            if arrival != parsed {
                self.state.update(cx, |state, _| state.commit_display());
            }
        }

        let displayed = validity_state.read(cx).displayed().clone();
        let shown = crate::validation::display_validity(
            validity_state.read(cx).validation_behavior(),
            &validity,
            self.is_invalid,
            &self.validation_errors,
            &displayed,
            None,
        );
        // Painted and exposed state follows the snapshot; the form's blocking
        // mirror below stays on the live `validity`.
        let is_invalid = shown.is_invalid;
        if validity_state.read(cx).validity() != &validity {
            validity_state.update(cx, |state, _| state.set_validity(validity.clone()));
        }
        if let Some(form_state) = form_state.as_ref() {
            let mut state = form_state.borrow_mut();
            state.value = crate::form::FormValue::Text(text.clone().into());
            state.is_invalid = validity.is_invalid;
            state.is_successful = !self.is_disabled;
            state.focus = Some(self.state.read(cx).focus_handle.clone());
            let commit_state = self.state.clone();
            let reset_state = self.state.clone();
            state.commit_validation = Some(std::sync::Arc::new(move |cx: &mut App| {
                commit_state.update(cx, |s, cx| {
                    if s.commit_display() {
                        cx.notify();
                    }
                });
            }));
            state.reset_validation = Some(std::sync::Arc::new(move |cx: &mut App| {
                reset_state.update(cx, |s, cx| {
                    if s.reset_display() {
                        cx.notify();
                    }
                });
            }));
        }

        if self.auto_focus {
            crate::util::focus_once(
                window,
                cx,
                element_id::scoped(&base_id, "autofocus"),
                &focus_handle,
            );
        }
        if let Some(render) = self.content.clone() {
            let focused = focus_handle.is_focused(window);
            return render(DateFieldRenderState {
                is_disabled: self.is_disabled,
                is_invalid,
                is_read_only: self.is_read_only,
                is_required: self.is_required,
                is_focused: focused,
                is_focus_within: focus_handle.contains_focused(window, cx),
                is_focus_visible: focused && crate::util::focus_visible(cx),
            })
            .into_any_element();
        }

        let pad_month = self.should_force_leading_zeros || regional_date.month_has_leading_zero;
        let pad_day = self.should_force_leading_zeros || regional_date.day_has_leading_zero;
        let pad_hour = self.should_force_leading_zeros
            || regional_time
                .as_ref()
                .is_some_and(|format| format.hour_has_leading_zero);
        let pad_minute = regional_time
            .as_ref()
            .is_none_or(|format| format.minute_has_leading_zero);
        let pad_second = regional_time
            .as_ref()
            .is_none_or(|format| format.second_has_leading_zero);
        let zero_based_twelve_hour = regional_time
            .as_ref()
            .is_some_and(|format| format.hour_zero_based);
        let am = regional_time
            .as_ref()
            .map_or_else(|| "AM".to_owned(), |format| format.am.clone());
        let pm = regional_time
            .as_ref()
            .map_or_else(|| "PM".to_owned(), |format| format.pm.clone());
        let segment_text = move |segment: FieldSegment| -> String {
            use crate::time_field::TimeSegment as T;
            match segment {
                FieldSegment::Date(segment) => {
                    if cleared.contains(&FieldSegment::Date(segment)) {
                        return segment.hint().to_owned();
                    }
                    let Some(d) = display_date else {
                        return segment.hint().to_owned();
                    };
                    match segment {
                        DateSegment::Month if pad_month => format!("{:02}", d.month),
                        DateSegment::Day if pad_day => format!("{:02}", d.day),
                        DateSegment::Month => d.month.to_string(),
                        DateSegment::Day => d.day.to_string(),
                        DateSegment::Year => format!("{:04}", d.year),
                    }
                }
                FieldSegment::Time(segment) => {
                    if cleared.contains(&FieldSegment::Time(segment)) {
                        return if segment == T::Meridiem {
                            am.clone()
                        } else {
                            "--".to_owned()
                        };
                    }
                    let Some(t) = display_time else {
                        return "--".to_owned();
                    };
                    match segment {
                        T::Hour if twelve_hour && pad_hour => {
                            let hour = if zero_based_twelve_hour {
                                t.hour % 12
                            } else {
                                t.twelve_hour().0
                            };
                            format!("{hour:02}")
                        }
                        T::Hour if twelve_hour => {
                            if zero_based_twelve_hour {
                                (t.hour % 12).to_string()
                            } else {
                                t.twelve_hour().0.to_string()
                            }
                        }
                        T::Hour if pad_hour => format!("{:02}", t.hour),
                        T::Hour => t.hour.to_string(),
                        T::Minute if pad_minute => format!("{:02}", t.minute),
                        T::Minute => t.minute.to_string(),
                        T::Second if pad_second => format!("{:02}", t.second),
                        T::Second => t.second.to_string(),
                        T::Meridiem => {
                            if t.hour < 12 {
                                am.clone()
                            } else {
                                pm.clone()
                            }
                        }
                    }
                }
            }
        };

        // An empty field seeds from `placeholderValue`, the way v3 does, so the
        // first arrow press lands on a sensible date instead of jumping a step
        // from nothing.
        let seed = self.placeholder_value.unwrap_or_else(Date::today);
        // The box and shared field chrome use the same resolved radius.
        let radius = self.radius.unwrap_or_else(|| crate::util::field_radius(cx));
        // `useDateField` is `role: 'group'` on the field box. The hidden
        // native input is `role: 'presentation'` and has no counterpart
        // element here.
        let mut group = gpui::div()
            .id(base_id.clone())
            .a11y_named(
                a11y::Role::Group,
                &a11y::Name::field(self.label.as_ref(), self.description.as_ref(), &shown),
            )
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.))
            .text_size(crate::util::FIELD_TEXT)
            .line_height(px(20.))
            .font_family(crate::util::MONO_FONT)
            .when_some(self.font_family.clone(), |group, family| {
                group.font_family(family)
            })
            .text_color(colors.field.foreground)
            .when(!self.bare, |el| {
                // `.date-input-group` is `h-9 items-center overflow-hidden`
                // with the segments inside it.
                el.px(self.padding_x.unwrap_or(px(12.)))
                    .h(self.height.unwrap_or(crate::util::FIELD_HEIGHT))
                    .overflow_hidden()
                    .rounded(radius)
            })
            .when(self.bare, |el| el.flex_1().min_w_0());

        // v3 drives a date field from the keyboard: the arrows step the focused
        // segment and walk between segments, and digits type into it. Without
        // this the steppers were the only way to change a value at all.
        if navigable {
            let state = self.state.clone();
            let on_change = self.on_change.clone();
            let constraints = self.constraints.clone();
            let display = display.clone();
            let held = focused_seg.clone();
            let buffer = typing;
            let fh = focus_handle.clone();
            let slots = segments.clone();
            let is_read_only = self.is_read_only;
            let on_picker_open = self.on_picker_open.clone();
            let report_invalid_changes = self.report_invalid_changes;
            group = group
                .track_focus(&focus_handle)
                .key_context("DateField")
                .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                    window.focus(&fh, cx);
                })
                .on_key_down(move |event, window, cx| {
                    let key = event.keystroke.key.as_str();
                    if ((event.keystroke.modifiers.alt && matches!(key, "down" | "up"))
                        || key == "space")
                        && on_picker_open.is_some()
                        && !is_read_only
                    {
                        if let Some(open) = &on_picker_open {
                            open(window, cx);
                        }
                        return;
                    }
                    let commit = |focused: FieldSegment,
                                  date: Date,
                                  time: Option<crate::time_field::Time>,
                                  window: &mut Window,
                                  cx: &mut App| {
                        let complete = display.update(cx, |display, cx| {
                            let complete = display.edit(focused, date, time, &slots, granularity);
                            cx.notify();
                            complete
                        });
                        if let Some((date, _, committed)) = complete {
                            state.update(cx, |s, cx| {
                                s.set_value(committed);
                                cx.notify();
                            });
                            if let Some(cb) = &on_change {
                                let reported = if report_invalid_changes {
                                    Some(date)
                                } else {
                                    Some(date).filter(|d| constraints.allows(*d))
                                };
                                cb(&reported, window, cx);
                            }
                        }
                    };
                    let (display_date, display_time) = {
                        let display = display.read(cx);
                        (display.date, display.time)
                    };
                    let seed_time = display_time.unwrap_or_default();
                    match key {
                        "left" | "right" => {
                            let delta = if key == "right" { 1 } else { -1 };
                            buffer.update(cx, |b, _| b.clear());
                            let here = slots.iter().position(|s| *s == focused).unwrap_or(0) as i32;
                            let next = (here + delta).clamp(0, slots.len() as i32 - 1) as usize;
                            let next = slots[next];
                            held.update(cx, |seg, cx| {
                                *seg = next;
                                cx.notify();
                            });
                        }
                        _ if is_read_only => {}
                        "up" | "down" | "pageup" | "pagedown" => {
                            let direction = if matches!(key, "up" | "pageup") {
                                1
                            } else {
                                -1
                            };
                            let delta = match focused {
                                FieldSegment::Date(segment)
                                    if matches!(key, "pageup" | "pagedown") =>
                                {
                                    direction * segment.page_step()
                                }
                                _ => direction,
                            };
                            let base = display_date.unwrap_or(seed);
                            buffer.update(cx, |b, _| b.clear());
                            // The first press on an empty field takes the seed
                            // itself rather than stepping past it.
                            match focused {
                                FieldSegment::Date(segment) => {
                                    let next = match display_date {
                                        Some(_) => segment.bump(base, delta),
                                        None => base,
                                    };
                                    commit(focused, next, display_time, window, cx);
                                }
                                FieldSegment::Time(segment) => {
                                    let next = match display_time {
                                        Some(t) => t.bump(segment, delta),
                                        None => seed_time,
                                    };
                                    commit(focused, base, Some(next), window, cx);
                                }
                            }
                        }
                        "home" | "end" => {
                            let maximum = key == "end";
                            let base = display_date.unwrap_or(seed);
                            buffer.update(cx, |b, _| b.clear());
                            match focused {
                                FieldSegment::Date(segment) => commit(
                                    focused,
                                    segment.bound(base, maximum),
                                    display_time,
                                    window,
                                    cx,
                                ),
                                FieldSegment::Time(segment) => {
                                    use crate::time_field::TimeSegment as T;
                                    let next = match segment {
                                        T::Meridiem => {
                                            let hour =
                                                seed_time.hour % 12 + if maximum { 12 } else { 0 };
                                            crate::time_field::Time::new(hour, seed_time.minute)
                                                .with_second(seed_time.second)
                                        }
                                        _ => segment.with_value(
                                            seed_time,
                                            if maximum { u32::MAX } else { 0 },
                                            twelve_hour,
                                            zero_based_twelve_hour,
                                        ),
                                    };
                                    commit(focused, base, Some(next), window, cx);
                                }
                            }
                        }
                        // The meridiem answers its own letters, as v3's does.
                        "a" | "p"
                            if focused
                                == FieldSegment::Time(crate::time_field::TimeSegment::Meridiem) =>
                        {
                            let hour = seed_time.hour % 12 + if key == "p" { 12 } else { 0 };
                            let next = crate::time_field::Time::new(hour, seed_time.minute)
                                .with_second(seed_time.second);
                            commit(
                                focused,
                                display_date.unwrap_or(seed),
                                Some(next),
                                window,
                                cx,
                            );
                        }
                        "backspace" | "delete" => {
                            buffer.update(cx, |b, _| b.clear());
                            let emptied = display.update(cx, |display, cx| {
                                let emptied = display.clear(focused, &slots);
                                cx.notify();
                                emptied
                            });
                            if emptied {
                                state.update(cx, |s, cx| {
                                    s.set_value(String::new());
                                    cx.notify();
                                });
                                if let Some(cb) = &on_change {
                                    cb(&None, window, cx);
                                }
                            }
                        }
                        digit if digit.len() == 1 && digit.chars().all(|c| c.is_ascii_digit()) => {
                            let digits = match focused {
                                FieldSegment::Date(segment) => segment.digits(),
                                FieldSegment::Time(segment) => segment.digits(),
                            };
                            if digits == 0 {
                                return;
                            }
                            let text = buffer.update(cx, |b, _| {
                                if b.len() >= digits {
                                    b.clear();
                                }
                                b.push_str(digit);
                                b.clone()
                            });
                            let Ok(value) = text.parse::<u32>() else {
                                return;
                            };
                            match focused {
                                FieldSegment::Date(segment) => {
                                    let base = display_date.unwrap_or(seed);
                                    commit(
                                        focused,
                                        segment.with_value(base, value),
                                        display_time,
                                        window,
                                        cx,
                                    );
                                }
                                FieldSegment::Time(segment) => commit(
                                    focused,
                                    display_date.unwrap_or(seed),
                                    Some(segment.with_value(
                                        seed_time,
                                        value,
                                        twelve_hour,
                                        zero_based_twelve_hour,
                                    )),
                                    window,
                                    cx,
                                ),
                            }
                            // A full segment hands the caret on, which is what
                            // makes `12252025` type a whole date.
                            if text.len() >= digits {
                                buffer.update(cx, |b, _| b.clear());
                                let here =
                                    slots.iter().position(|s| *s == focused).unwrap_or(0) as i32;
                                let next = (here + 1).clamp(0, slots.len() as i32 - 1) as usize;
                                let next = slots[next];
                                held.update(cx, |seg, cx| {
                                    *seg = next;
                                    cx.notify();
                                });
                            }
                        }
                        _ => {}
                    }
                });
        }

        // `.date-input-group` is `overflow-hidden` around its segments, so a
        // ring drawn in its margin would be cut: the group takes the chrome
        // without the ring and `util::field_ring_carrier` below hangs the ring
        // outside the clip.
        let mut carrier_ring = None;
        if !self.bare && !self.is_bare {
            carrier_ring = Some(crate::util::field_ring_color(
                is_invalid,
                focus_handle.is_focused(window),
                self.focus_ring,
                cx,
            ));
            group = crate::util::apply_field_chrome_ringless(
                group,
                self.variant,
                is_invalid,
                focus_handle.is_focused(window),
                self.focus_ring,
                Some(radius),
                cx,
            );

            let focused = focus_handle.is_focused(window);
            if !self.embedded && !self.is_disabled && !is_invalid && !focused {
                let idle_bg = match self.variant {
                    herogpui_core::FieldVariant::Primary => colors.field.background,
                    herogpui_core::FieldVariant::Secondary => colors.default.color,
                };
                let hover_bg = match self.variant {
                    herogpui_core::FieldVariant::Primary => colors.field.hover(),
                    herogpui_core::FieldVariant::Secondary => colors.default.hover(),
                };
                let hover_border = colors.field.border_hover();
                group = crate::anim::hover_fade_with_duration_and_easing(
                    group,
                    element_id::scoped(&base_id, "hover-fade"),
                    (idle_bg, hover_bg),
                    None,
                    Some(hover_border),
                    move |fill| fill.rounded(radius),
                    Some(150),
                    crate::anim::HoverFadeEasing::EaseSmooth,
                    window,
                    cx,
                );
            }
        }
        if self.full_width {
            group = group.w_full();
        }

        // `.date-input-group__prefix` is `ms-3 me-0`; the shell's own `px-3`
        // already provides that inset, so the slot only needs to sit inline and
        // inherit the placeholder colour.
        if let Some(prefix) = self.prefix {
            group = group.child(
                gpui::div()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .mr(px(4.))
                    .text_color(colors.field.placeholder)
                    .child(prefix),
            );
        }

        for (index, segment) in segments.iter().copied().enumerate() {
            let separator = match (index, segment) {
                (index, FieldSegment::Date(_)) => Some(regional_date.literals[index].clone()),
                (index, FieldSegment::Time(_)) => {
                    let time_index = index - regional_date.order.len();
                    regional_time.as_ref().and_then(|format| {
                        format.literals.get(time_index).map(|literal| {
                            if time_index == 0 {
                                format!(", {literal}")
                            } else {
                                literal.clone()
                            }
                        })
                    })
                }
            };
            if let Some(separator) = separator.filter(|separator| !separator.is_empty()) {
                group = group.child(gpui::div().text_color(colors.muted).child(separator));
            }

            let mut seg = gpui::div()
                .id(element_id::indexed(&segment_base_id, "seg", index))
                // `.date-input-group__segment` is `rounded-md px-0.5`.
                .px(px(2.))
                .py(px(1.))
                .rounded(cx.layout().radius_md())
                // `segment` is v3's render prop on `DateField.Segment`: the
                // closure is handed which segment it is drawing.
                .child(match &self.segment {
                    Some(render) => render(segment, segment_text(segment).into()),
                    _ => segment_text(segment).into_any_element(),
                });

            if parsed.is_none() {
                seg = seg.text_color(colors.muted);
            }
            // `.date-input-group__segment:focus` only: the remembered segment
            // is where focus lands next, not a focused segment while the
            // field itself does not hold focus.
            if focused == segment && focus_handle.is_focused(window) {
                seg = seg
                    .bg(colors.accent.soft())
                    .text_color(colors.accent.soft_foreground(colors.foreground));
            }

            if navigable {
                let held = focused_seg.clone();
                seg = seg
                    .cursor(crate::util::interactive_cursor(cx))
                    .on_click(move |_, _, cx| {
                        held.update(cx, |s, cx| {
                            *s = segment;
                            cx.notify();
                        });
                    });
            }

            // `useDateSegment` is `role: 'spinbutton'` then rewritten to
            // `role: 'textbox'` (`useDateSegment.mjs`). Named by the
            // segment type, the same string `displayNames.of` gives.
            let seg_text = segment_text(segment);
            seg = seg
                .a11y_named(
                    a11y::Role::TextInput,
                    &a11y::Name::labelled(match segment {
                        FieldSegment::Date(segment) => segment.a11y_label(cx),
                        FieldSegment::Time(segment) => segment.a11y_label(cx),
                    }),
                )
                .a11y_text(&seg_text, None);

            group = group.child(seg);
            if index == regional_date.order.len() - 1 && !regional_date.literals[3].is_empty() {
                group = group.child(
                    gpui::div()
                        .text_color(colors.muted)
                        .child(regional_date.literals[3].clone()),
                );
            }
        }
        if let Some(literal) = regional_time
            .as_ref()
            .and_then(|format| format.literals.last())
            .filter(|literal| !literal.is_empty())
        {
            group = group.child(gpui::div().text_color(colors.muted).child(literal.clone()));
        }

        if let Some(suffix) = self.suffix {
            // `.date-input-group__suffix` is `shrink-0 me-3`: pinned to the
            // field's end, not trailing the last segment. `ml_auto` pushes it
            // across the free space and the `pl` keeps the same minimum gap
            // the old `ml(4)` plus the row's `gap(2)` gave (the group's `px`
            // already supplies v3's `me-3` inset from the border).
            group = group.child(
                gpui::div()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .ml_auto()
                    .pl(px(4.))
                    .text_color(colors.field.placeholder)
                    .child(suffix),
            );
        }
        if navigable {
            group = crate::util::record_focus_bounds(group, &focus_handle, window, cx);
        }

        if self.embedded {
            if self.is_disabled {
                group = group.opacity(cx.layout().disabled_opacity);
            }
            let group = crate::util::apply_sx(group, &self.sx);
            return match carrier_ring {
                Some(ring) => {
                    crate::util::field_ring_carrier(group, ring, radius, cx).into_any_element()
                }
                None => group.into_any_element(),
            };
        }

        let row: gpui::AnyElement = match carrier_ring {
            Some(ring) => {
                crate::util::field_ring_carrier(group, ring, radius, cx).into_any_element()
            }
            None => group.into_any_element(),
        };

        // -- label / description / error wrapper ------------------------------
        let mut el = gpui::div().flex().flex_col().gap(px(4.));
        if !self.full_width {
            el = el.max_w(px(320.));
        } else {
            el = el.w_full();
        }
        if let Some(label) = self.label.clone() {
            el = el.child(
                crate::field::Label::new(label)
                    .is_required(self.is_required)
                    .is_invalid(is_invalid)
                    .is_disabled(self.is_disabled),
            );
        }
        el = el.child(row);
        // v3's `date-field.tsx` renders only the caller's `children` here —
        // no format hint appears unless the caller supplies a description.
        match shown.first() {
            Some(message) => el = el.child(crate::field::ErrorMessage::new(message)),
            None => {
                if let Some(description) = self.description.clone() {
                    el = el.child(crate::field::Description::new(description));
                }
            }
        }
        if self.is_disabled {
            el = el.opacity(cx.layout().disabled_opacity);
        }
        crate::util::apply_sx(el, &self.sx).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    #[test]
    fn enabled_date_field_hover_uses_the_pinned_smooth_fill_transition() {
        let source = include_str!("field.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("hover_fade_with_duration_and_easing(")
                && source.contains("Some(150)")
                && source.contains("HoverFadeEasing::EaseSmooth")
        );
        assert!(source.contains("!self.embedded && !self.is_disabled && !is_invalid && !focused"));
    }
    use crate::time_field::{HourCycle, Time, TimeSegment};

    #[test]
    fn a_day_value_is_a_plain_iso_date() {
        let date = Date::new(2025, 2, 3);
        assert_eq!(format_value(date, None, Granularity::Day), "2025-02-03");
        assert_eq!(parse_value("2025-02-03"), (Some(date), None));
    }

    #[test]
    fn date_order_literals_and_padding_follow_locale_patterns() {
        let us = RegionalDateFormat::for_locale("en-US").unwrap();
        assert_eq!(us.order, DateSegment::ALL);
        assert_eq!(us.literals, ["", "/", "/", ""]);
        assert!(!us.month_has_leading_zero);
        assert!(!us.day_has_leading_zero);

        let gb = RegionalDateFormat::for_locale("en-GB").unwrap();
        assert_eq!(
            gb.order,
            [DateSegment::Day, DateSegment::Month, DateSegment::Year]
        );
        assert_eq!(gb.literals, ["", "/", "/", ""]);
        assert!(gb.month_has_leading_zero);
        assert!(gb.day_has_leading_zero);

        let german = RegionalDateFormat::for_locale("de-DE").unwrap();
        assert_eq!(
            german.order,
            [DateSegment::Day, DateSegment::Month, DateSegment::Year]
        );
        assert_eq!(german.literals, ["", ".", ".", ""]);

        let japanese = RegionalDateFormat::for_locale("ja-JP").unwrap();
        assert_eq!(
            japanese.order,
            [DateSegment::Year, DateSegment::Month, DateSegment::Day]
        );
        assert_eq!(japanese.literals, ["", "/", "/", ""]);
        assert_eq!(RegionalDateFormat::for_locale("not_a_locale"), None);
    }

    #[test]
    fn date_padding_prefers_the_system_time_category() {
        // The time category comes first in the system chain (`system_locale`).
        let tags = ["en-GB".to_owned(), "en-US".to_owned()];
        assert_eq!(
            RegionalDateFormat::for_preferences(&tags),
            RegionalDateFormat::for_locale("en-GB")
        );
    }

    #[gpui::test]
    fn date_field_hour_cycle_follows_the_system_time_locale(cx: &mut gpui::TestAppContext) {
        const CHILD: &str = "HEROGPUI_DATE_FIELD_TIME_LOCALE_TEST";
        if std::env::var_os(CHILD).is_none() {
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "date_picker::tests::date_field_hour_cycle_follows_the_system_time_locale",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .env_remove("LC_ALL")
                .env("LC_TIME", "en_US.UTF-8")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "12-hour DateField locale child failed:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            return;
        }

        assert_eq!(HourCycle::default(), HourCycle::H12);
        let state = cx.new(|cx| crate::input::InputState::new(cx));
        let field = DateField::new(state.clone()).granularity(Granularity::Minute);
        assert_eq!(field.hour_cycle, HourCycle::H12);
        assert_eq!(
            TimeSegment::order(
                field.granularity.time().unwrap(),
                field.hour_cycle == HourCycle::H12,
            ),
            [
                TimeSegment::Hour,
                TimeSegment::Minute,
                TimeSegment::Meridiem,
            ]
        );

        let explicit = DateField::new(state)
            .granularity(Granularity::Minute)
            .hour_cycle(HourCycle::H24);
        assert_eq!(explicit.hour_cycle, HourCycle::H24);
        assert_eq!(
            TimeSegment::order(
                explicit.granularity.time().unwrap(),
                explicit.hour_cycle == HourCycle::H12,
            ),
            [TimeSegment::Hour, TimeSegment::Minute]
        );
    }

    #[test]
    fn a_time_value_round_trips_at_every_granularity() {
        let date = Date::new(2025, 2, 3);
        let time = Time::new(8, 45).with_second(9);
        for (granularity, text) in [
            (Granularity::Hour, "2025-02-03T08"),
            (Granularity::Minute, "2025-02-03T08:45"),
            (Granularity::Second, "2025-02-03T08:45:09"),
        ] {
            assert_eq!(format_value(date, Some(time), granularity), text);
            let (d, t) = parse_value(text);
            assert_eq!(d, Some(date));
            // Only as much of the time as the granularity wrote comes back.
            let t = t.expect("a time");
            assert_eq!(t.hour, 8);
            assert_eq!(
                (t.minute, t.second),
                match granularity {
                    Granularity::Hour => (0, 0),
                    Granularity::Minute => (45, 0),
                    _ => (45, 9),
                }
            );
        }
    }

    #[test]
    fn a_space_separates_as_well_as_a_t() {
        assert_eq!(
            parse_value("2025-02-03 08:45"),
            parse_value("2025-02-03T08:45")
        );
    }

    #[test]
    fn an_impossible_time_is_no_time() {
        assert_eq!(parse_value("2025-02-03T24:00").1, None);
        assert_eq!(parse_value("2025-02-03T08:60").1, None);
        assert_eq!(parse_value("2025-02-03Tzz").1, None);
    }

    #[test]
    fn granularity_picks_the_time_slots() {
        let slots = |g: Granularity, twelve: bool| {
            g.time()
                .map(|t| TimeSegment::order(t, twelve))
                .unwrap_or_default()
        };
        assert!(slots(Granularity::Day, false).is_empty());
        assert_eq!(slots(Granularity::Hour, false), vec![TimeSegment::Hour]);
        assert_eq!(
            slots(Granularity::Second, false),
            vec![TimeSegment::Hour, TimeSegment::Minute, TimeSegment::Second]
        );
        // A 12-hour clock adds the meridiem, wherever the granularity stops.
        assert_eq!(
            slots(Granularity::Hour, true),
            vec![TimeSegment::Hour, TimeSegment::Meridiem]
        );
    }
}

crate::util::impl_component_styled!(DateField);
