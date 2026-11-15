//! RangeCalendar — port of `@heroui/range-calendar` (v3).
//!
//! A month grid for selecting a start and end date. Selection is a two-step
//! anchor/extend interaction; the days between the anchor and the hovered day
//! preview as part of the range.

use std::sync::Arc;

use gpui::{
    div, prelude::*, px, App, ElementId, Entity, InteractiveElement, IntoElement, RenderOnce,
    Styled, Window,
};
use herogpui_core::element_id;
use herogpui_theme::ActiveTheme;

use crate::{
    a11y::{self, A11y as _},
    calendar::{add_days, days_from_civil, month_year_heading, Date},
    calendar_keys,
    calendar_view::{self, PageBehavior, SelectionAlignment, VisibleDuration},
    date_constraints::{DateConstraints, Weekday},
    date_picker::DateRangeState,
    icons, util,
};

type OnRangeChange = Arc<dyn Fn(&Date, &Date, &mut Window, &mut App) + 'static>;
type RangeDateUnavailable = Arc<dyn Fn(Date, Option<Date>) -> bool + 'static>;

/// HeroUI RangeCalendar.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct RangeCalendar {
    /// `value` — v3's controlled range, stored for the first render only.
    /// Each end is its own `Option` so a half-open controlled range keeps its
    /// null end, exactly as the builder spelled it.
    value: Option<(Option<Date>, Option<Date>)>,
    /// `defaultValue` — seeds the state on the first render only.
    default_value: Option<(Date, Date)>,
    id: ElementId,
    state: Entity<DateRangeState>,
    constraints: DateConstraints,
    range_date_unavailable: Option<RangeDateUnavailable>,
    is_disabled: bool,
    /// Retained range-picker exits keep their calendar painted while making
    /// descendants visual-only until the exit animation finishes.
    inert: bool,
    is_read_only: bool,
    /// Set by a picker: take the focus as the panel opens. See
    /// [`RangeCalendar::autofocus_grid`].
    autofocus_grid: bool,
    /// The calendar system the grid is drawn in, when the caller names one.
    /// `None` follows the operating system's locale, which is v3's default.
    calendar_system: Option<crate::calendar_system::CalendarSystem>,
    is_invalid: bool,
    focused_value: Option<Date>,
    duration: VisibleDuration,
    page_behavior: PageBehavior,
    selection_alignment: Option<SelectionAlignment>,
    /// `isYearPickerOpen` — `None` leaves the component holding the state,
    /// seeded from `defaultYearPickerOpen`.
    year_picker_open: Option<bool>,
    default_year_picker_open: bool,
    /// Whether the heading is the interactive `YearPickerTrigger` (v3 composes
    /// it as a part) rather than the plain `RangeCalendar.Heading` label.
    year_picker: bool,
    /// `RangeCalendar.YearPickerGrid.visibleYears` — `None` uses the v3
    /// min/max span or 20-year default.
    visible_years: Option<usize>,
    /// `RangeCalendar.YearPickerTriggerHeading.offset.months`.
    year_heading_offset_months: i32,
    on_year_picker_open_change: Option<Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    on_focus_change: Option<Arc<dyn Fn(&Date, &mut Window, &mut App) + 'static>>,
    /// `allowsNonContiguousRanges` — lets a range span unavailable dates.
    allows_non_contiguous_ranges: bool,
    /// `RangeCalendar.CellIndicator` — the dot under a marked day, the same part
    /// a [`Calendar`](crate::calendar::Calendar) draws.
    cell_indicator: Option<Box<dyn Fn(Date) -> bool + 'static>>,
    /// `RangeCalendar.Cell`'s render props: the closure replaces the day label
    /// and is handed the state v3 passes it, the two range ends included.
    cell: Option<Box<dyn Fn(&RangeCalendarCellState) -> gpui::AnyElement + 'static>>,
    on_change: Option<OnRangeChange>,
    /// The fill a hovered nav arrow takes, in place of `--default`.
    nav_hover_bg: Option<gpui::Hsla>,
    /// The fill a hovered year cell takes, in place of `--default`.
    year_hover_bg: Option<gpui::Hsla>,
    /// The fill a hovered plain (unselected, out-of-range) day takes, in
    /// place of `--default`. Today, range and selected cells keep their owns.
    day_hover_bg: Option<gpui::Hsla>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

/// What `RangeCalendar.Cell`'s render function is handed -- v3's render props
/// for the cell, one field each.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct RangeCalendarCellState {
    /// The date this cell draws.
    pub date: Date,
    /// `formattedDate` — the day label, as this port writes it.
    pub formatted_date: gpui::SharedString,
    /// `isSelected` — an end of the range or inside it.
    pub is_selected: bool,
    /// `isSelectionStart`
    pub is_selection_start: bool,
    /// `isSelectionEnd`
    pub is_selection_end: bool,
    /// `isUnavailable`
    pub is_unavailable: bool,
    /// `isOutsideMonth`
    pub is_outside_month: bool,
    /// Today, which v3 marks with `data-today`.
    pub is_today: bool,
    /// Outside the min/max range, or the calendar is disabled.
    pub is_disabled: bool,
}

impl RangeCalendar {
    /// The locale whose calendar system this grid is drawn in.
    ///
    /// The same override [`crate::Calendar::locale`] takes, and for the same
    /// reason: v3 wraps the calendar in an `I18nProvider` and gpui has no
    /// subtree context to put one in.
    pub fn locale(mut self, tag: impl AsRef<str>) -> Self {
        self.calendar_system = crate::calendar_system::CalendarSystem::for_locale(tag.as_ref());
        self
    }

    /// The system this grid measures in: the caller's, else the platform's.
    fn system(&self) -> &crate::calendar_system::CalendarSystem {
        self.calendar_system
            .as_ref()
            .unwrap_or_else(|| crate::calendar_system::system())
    }

    /// `focusedValue` — the date carrying the focus ring, independent of the
    /// selection.
    pub fn focused_value(mut self, date: Date) -> Self {
        self.focused_value = Some(date);
        self
    }

    /// `onFocusChange` — fires when a different date takes focus.
    pub fn on_focus_change(
        mut self,
        handler: impl Fn(&Date, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_focus_change = Some(Arc::new(handler));
        self
    }

    /// `value` — v3's controlled-range spelling, as a pure builder.
    ///
    /// The bound [`DateRangeState`] owns the range once the grid renders, so
    /// this seeds the state on the first render only, winning over
    /// [`RangeCalendar::default_value`] the way v3's controlled prop outranks
    /// the uncontrolled seed; calling `.value(..)` twice keeps the last call,
    /// like every other builder here. A later range is an imperative update
    /// rather than a builder: write the caller-owned state entity.
    pub fn value(mut self, start: Option<Date>, end: Option<Date>) -> Self {
        self.value = Some((start, end));
        self
    }

    /// Creates a range calendar bound to `state`.
    pub fn new(state: Entity<DateRangeState>) -> Self {
        Self {
            value: None,
            default_value: None,
            id: ElementId::named_usize("range-cal", state.entity_id().as_u64() as usize),
            state,
            constraints: DateConstraints::new().with_hero_calendar_bounds(),
            range_date_unavailable: None,
            calendar_system: None,
            is_disabled: false,
            inert: false,
            is_read_only: false,
            autofocus_grid: false,
            is_invalid: false,
            focused_value: None,
            duration: VisibleDuration::default(),
            page_behavior: PageBehavior::default(),
            selection_alignment: None,
            year_picker_open: None,
            default_year_picker_open: false,
            year_picker: false,
            visible_years: None,
            year_heading_offset_months: 0,
            on_year_picker_open_change: None,
            on_focus_change: None,
            allows_non_contiguous_ranges: false,
            cell_indicator: None,
            cell: None,
            on_change: None,
            day_hover_bg: None,
            nav_hover_bg: None,
            year_hover_bg: None,
            sx: None,
        }
    }

    /// `defaultValue` — the uncontrolled initial range.
    ///
    /// Written into the state on the first render only, so it seeds the
    /// component without fighting the user afterwards.
    pub fn default_value(mut self, value: (Date, Date)) -> Self {
        self.default_value = Some(value);
        self
    }

    /// `visibleDuration` — a month view, a run of weeks, or a rolling window
    /// of days.
    pub fn visible_duration(mut self, duration: VisibleDuration) -> Self {
        self.duration = duration;
        self
    }

    /// `pageBehavior` — whether navigation steps the whole visible range or a
    /// single month/week/day.
    pub fn page_behavior(mut self, behavior: PageBehavior) -> Self {
        self.page_behavior = behavior;
        self
    }

    /// `selectionAlignment` — where the range start sits inside the visible
    /// range, until the user navigates for themselves.
    pub fn selection_alignment(mut self, alignment: SelectionAlignment) -> Self {
        self.selection_alignment = Some(alignment);
        self
    }

    /// Whether the heading is the interactive year-picker trigger (v3's
    /// `RangeCalendar.YearPickerTrigger`) instead of a plain heading label.
    /// `is_year_picker_open` and `default_year_picker_open` imply it: a
    /// picker state with no trigger could never open.
    pub fn year_picker(mut self, v: bool) -> Self {
        self.year_picker = v;
        self
    }

    /// `isYearPickerOpen` — swaps the day grid for a year grid.
    pub fn is_year_picker_open(mut self, v: bool) -> Self {
        self.year_picker = true;
        self.year_picker_open = Some(v);
        self
    }

    /// `defaultYearPickerOpen` — the uncontrolled initial state.
    ///
    /// Only consulted when `isYearPickerOpen` is not supplied; the component
    /// then owns the state and the heading toggles it directly.
    pub fn default_year_picker_open(mut self, v: bool) -> Self {
        self.year_picker = true;
        self.default_year_picker_open = v;
        self
    }

    /// `RangeCalendar.YearPickerGrid.visibleYears` — the size of its sliding
    /// window.
    pub fn visible_years(mut self, count: usize) -> Self {
        self.visible_years = Some(count.max(1));
        self
    }

    /// `RangeCalendar.YearPickerTriggerHeading.offset` — shifts its displayed
    /// month.
    pub fn offset(mut self, months: i32) -> Self {
        self.year_heading_offset_months = months;
        self
    }

    /// `onYearPickerOpenChange` — reports the built-in heading trigger opening
    /// or closing the year grid.
    pub fn on_year_picker_open_change(
        mut self,
        f: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_year_picker_open_change = Some(Arc::new(f));
        self
    }

    /// Sets the earliest selectable date (`minValue`).
    pub fn min_value(mut self, date: Date) -> Self {
        self.constraints.min_value = Some(date);
        self
    }

    /// Sets the latest selectable date (`maxValue`).
    pub fn max_value(mut self, date: Date) -> Self {
        self.constraints.max_value = Some(date);
        self
    }

    /// `isDateUnavailable` — blocks a date relative to the active range anchor.
    /// The anchor is `None` until the first endpoint is selected and after the
    /// range is complete.
    pub fn is_date_unavailable(mut self, f: impl Fn(Date, Option<Date>) -> bool + 'static) -> Self {
        self.constraints.is_date_unavailable = None;
        self.range_date_unavailable = Some(Arc::new(f));
        self
    }

    /// `firstDayOfWeek`
    /// `RangeCalendar.Cell`'s render function — draw the day yourself.
    ///
    /// v3 hands it `{formattedDate, isSelected, isUnavailable, isOutsideMonth,
    /// isSelectionStart, isSelectionEnd}`; this port computes every one of those
    /// to draw the cell, so the closure is handed the same state.
    pub fn cell(
        mut self,
        render: impl Fn(&RangeCalendarCellState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.cell = Some(Box::new(render));
        self
    }

    /// `RangeCalendar.CellIndicator` — mark the days this returns `true` for.
    pub fn cell_indicator(mut self, f: impl Fn(Date) -> bool + 'static) -> Self {
        self.cell_indicator = Some(Box::new(f));
        self
    }

    /// Sets the first day of the week (`firstDayOfWeek`).
    pub fn first_day_of_week(mut self, day: Weekday) -> Self {
        self.constraints.first_day_of_week = day;
        self
    }

    /// `weeksInMonth`
    pub fn weeks_in_month(mut self, rows: usize) -> Self {
        self.constraints.weeks_in_month = Some(rows);
        self
    }

    /// Sets whether the calendar is invalid (`isInvalid`).
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// `allowsNonContiguousRanges` — permits a selection that spans dates the
    /// unavailable predicate rejects.
    pub fn allows_non_contiguous_ranges(mut self, v: bool) -> Self {
        self.allows_non_contiguous_ranges = v;
        self
    }

    /// All the date constraints at once.
    pub fn constraints(mut self, constraints: DateConstraints) -> Self {
        self.constraints = constraints.with_hero_calendar_bounds();
        self.range_date_unavailable = None;
        self
    }

    /// Sets whether the calendar is disabled (`isDisabled`).
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Makes the range calendar visual-only for a retained picker exit.
    /// Unlike `is_disabled`, this preserves normal painting and only removes
    /// focus, pointer and keyboard activation.
    pub(crate) fn inert(mut self, v: bool) -> Self {
        self.inert = v;
        self
    }

    /// Sets whether the calendar is read-only (`isReadOnly`).
    pub fn is_read_only(mut self, v: bool) -> Self {
        self.is_read_only = v;
        self
    }

    /// Takes the focus the first time the grid renders; see
    /// [`crate::calendar::Calendar::autofocus_grid`]. Crate-only for the same
    /// reason: a standalone calendar must not steal the focus.
    pub(crate) fn autofocus_grid(mut self, v: bool) -> Self {
        self.autofocus_grid = v;
        self
    }

    /// `onChange` — called with `(start, end)` once both endpoints complete a
    /// range. The first pick remains the internal anchor, matching React
    /// Stately's `useRangeCalendarState`.
    pub fn on_change(
        mut self,
        handler: impl Fn(&Date, &Date, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }

    /// The fill a hovered nav arrow takes, in place of `--default`.
    pub fn nav_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.nav_hover_bg = Some(color.into());
        self
    }

    /// The fill a hovered year cell takes, in place of `--default`.
    pub fn year_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.year_hover_bg = Some(color.into());
        self
    }

    /// The fill a hovered plain (unselected, out-of-range) day takes, in
    /// place of `--default`. Today, range and selected cells keep their own
    /// fills.
    pub fn day_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.day_hover_bg = Some(color.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the range calendar's root element after every value the
    /// component and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }
}

/// Resolves one range selection through the same constraint path for clicks
/// and keyboard activation. The effective constraints already include the
/// anchor-derived range bounds, so the requested endpoint only needs clamping
/// and an availability check here.
fn resolve_pick(
    start: Option<Date>,
    end: Option<Date>,
    requested: Date,
    constraints: &DateConstraints,
    range_date_unavailable: Option<&RangeDateUnavailable>,
) -> Option<(Date, Option<Date>)> {
    if start.is_none() || end.is_some() {
        return range_allows(constraints, range_date_unavailable, requested, None)
            .then_some((requested, None));
    }

    let anchor = start?;
    let target = constraints.constrain(requested);
    if range_is_unavailable(constraints, range_date_unavailable, target, Some(anchor)) {
        return None;
    }

    if days_from_civil(&target) < days_from_civil(&anchor) {
        Some((target, Some(anchor)))
    } else {
        Some((anchor, Some(target)))
    }
}

fn range_is_unavailable(
    constraints: &DateConstraints,
    range_date_unavailable: Option<&RangeDateUnavailable>,
    date: Date,
    anchor: Option<Date>,
) -> bool {
    match range_date_unavailable {
        Some(predicate) => predicate(date, anchor),
        None => constraints.is_unavailable(date),
    }
}

fn range_allows(
    constraints: &DateConstraints,
    range_date_unavailable: Option<&RangeDateUnavailable>,
    date: Date,
    anchor: Option<Date>,
) -> bool {
    !constraints.out_of_range(date)
        && !range_is_unavailable(constraints, range_date_unavailable, date, anchor)
}

/// Pinned React Stately searches through one visible duration on either side
/// of the active anchor, then probes the next sentinel day. The last available
/// neighbour becomes a temporary min/max bound for cells, focus and navigation.
fn unavailable_range_boundary(
    system: &crate::calendar_system::CalendarSystem,
    anchor: Date,
    direction: i32,
    duration: VisibleDuration,
    constraints: &DateConstraints,
    range_date_unavailable: Option<&RangeDateUnavailable>,
) -> Option<Date> {
    let limit = calendar_view::page_in(system, duration, PageBehavior::Visible, anchor, direction);
    let mut next = add_days(&anchor, i64::from(direction));
    let within_limit = |date: Date| {
        if direction < 0 {
            days_from_civil(&date) >= days_from_civil(&limit)
        } else {
            days_from_civil(&date) <= days_from_civil(&limit)
        }
    };

    while within_limit(next) {
        if range_is_unavailable(constraints, range_date_unavailable, next, Some(anchor)) {
            return Some(add_days(&next, -i64::from(direction)));
        }
        next = add_days(&next, i64::from(direction));
    }

    range_is_unavailable(constraints, range_date_unavailable, next, Some(anchor))
        .then(|| add_days(&next, -i64::from(direction)))
}

fn effective_range_constraints(
    system: &crate::calendar_system::CalendarSystem,
    anchor: Option<Date>,
    duration: VisibleDuration,
    constraints: &DateConstraints,
    range_date_unavailable: Option<&RangeDateUnavailable>,
    allows_non_contiguous_ranges: bool,
) -> DateConstraints {
    let mut effective = constraints.clone();
    let Some(anchor) = anchor else {
        return effective;
    };
    if allows_non_contiguous_ranges
        || (range_date_unavailable.is_none() && constraints.is_date_unavailable.is_none())
    {
        return effective;
    }

    if let Some(boundary) = unavailable_range_boundary(
        system,
        anchor,
        -1,
        duration,
        constraints,
        range_date_unavailable,
    ) {
        effective.min_value = Some(match effective.min_value {
            Some(min) if days_from_civil(&min) > days_from_civil(&boundary) => min,
            _ => boundary,
        });
    }
    if let Some(boundary) = unavailable_range_boundary(
        system,
        anchor,
        1,
        duration,
        constraints,
        range_date_unavailable,
    ) {
        effective.max_value = Some(match effective.max_value {
            Some(max) if days_from_civil(&max) < days_from_civil(&boundary) => max,
            _ => boundary,
        });
    }

    effective
}

/// Where pinned React Stately moves focus after the first keyboard endpoint:
/// prefer tomorrow, fall back to yesterday, and stay put if neither is valid.
fn keyboard_range_focus(
    anchor: Date,
    constraints: &DateConstraints,
    range_date_unavailable: Option<&RangeDateUnavailable>,
    allows_non_contiguous_ranges: bool,
) -> Option<Date> {
    [add_days(&anchor, 1), add_days(&anchor, -1)]
        .into_iter()
        .find(|date| {
            !constraints.out_of_range(*date)
                && (allows_non_contiguous_ranges
                    || !range_is_unavailable(
                        constraints,
                        range_date_unavailable,
                        *date,
                        Some(anchor),
                    ))
        })
}

/// The per-frame facts every cell needs, bundled so the helpers below take a
/// readable argument list.
struct Frame<'a> {
    start: Option<Date>,
    preview_end: Option<Date>,
    unavailable_anchor: Option<Date>,
    constraints: &'a DateConstraints,
    today: Date,
    cursor: &'a Entity<Option<Date>>,
    focus_preview: &'a Entity<bool>,
    selection_before_anchor: &'a Entity<Option<(Date, Date)>>,
    base: &'a str,
    /// The date wearing the focus ring: `focusedValue` when the caller controls
    /// it, otherwise wherever the arrow keys have walked to. `None` while the
    /// grid does not hold the keyboard.
    focused: Option<Date>,
    anchor: Date,
    visible_start: Option<Date>,
    cell_size: gpui::Pixels,
}

/// Where a cell sits in its row: the row's first and last columns drive the
/// pinned row-boundary rounding of the range track.
struct CellSlot {
    column: usize,
    columns: usize,
}

impl CellSlot {
    fn is_first(&self) -> bool {
        self.column == 0
    }

    fn is_last(&self) -> bool {
        self.column + 1 == self.columns
    }
}

impl RangeCalendar {
    /// One day cell. The range track lives on the outer cell and runs under
    /// the caps too; the inner button is the circle that takes the press.
    fn range_cell(
        &self,
        date: Date,
        outside_month: bool,
        frame: &Frame<'_>,
        key: String,
        slot: CellSlot,
        cx: &App,
    ) -> gpui::AnyElement {
        let colors = cx.colors();
        let accent = if self.is_invalid {
            colors.danger
        } else {
            colors.accent
        };
        let serial = days_from_civil(&date);
        let start_day = frame.start.map(|d| days_from_civil(&d));
        let end_day = frame.preview_end.map(|d| days_from_civil(&d));
        let unavailable = range_is_unavailable(
            frame.constraints,
            self.range_date_unavailable.as_ref(),
            date,
            frame.unavailable_anchor,
        );
        let disabled = outside_month
            || self.is_disabled
            || frame.constraints.out_of_range(date)
            || frame
                .visible_start
                .is_some_and(|start| days_from_civil(&date) < days_from_civil(&start));
        let eligible = !disabled && !unavailable;
        let selectable = eligible && !self.is_read_only && !self.inert;

        let is_start = start_day == Some(serial);
        let is_end = end_day == Some(serial);
        let in_range = eligible
            && match (start_day, end_day) {
                (Some(a), Some(b)) => serial > a.min(b) && serial < a.max(b),
                _ => false,
            };
        let is_selected = eligible && (is_start || is_end || in_range);
        let draw_start = is_start && !outside_month;
        let draw_end = is_end && !outside_month;
        let is_today = serial == days_from_civil(&frame.today);

        // The pinned anatomy splits the cell in two. The outer
        // `.range-calendar__cell` carries the range track -- the middle
        // segment's `rounded-none bg-accent-soft`, painted unscaled so the
        // run stays continuous across neighbours -- and the inner
        // `.range-calendar__cell-button`, the circle holding the day glyph,
        // is what takes the 0.9 press scale.
        let track_key = format!("{key}-track");
        let indicator_key = format!("{key}-indicator");
        let mut track = div()
            .relative()
            // The debug selector lets the headless tests read the outer
            // cell's laid-out bounds separately from the pressed button.
            .debug_selector(move || track_key)
            .flex()
            .items_center()
            .justify_center()
            .flex_1()
            .min_w_0()
            .h(frame.cell_size);
        if is_selected {
            // `[data-selected]:not([data-outside-month])` is
            // `rounded-none bg-accent-soft` -- on the caps too, whose solid
            // button paints over it. The track carries no text colour: the
            // inner button keeps the base `text-foreground` unless
            // `data-today` recolours it. Row ends round off with `lg` so a
            // run crossing a week boundary reads as one shape, and a cap
            // rounds its own side with `3xl`
            // (`rounded-ss-3xl rounded-es-3xl` / `rounded-se-3xl rounded-ee-3xl`).
            let cap_radius = util::control_radius(cx);
            let edge_radius = util::key_radius(cx);
            let left = if draw_start {
                cap_radius
            } else if slot.is_first() {
                edge_radius
            } else {
                px(0.)
            };
            let right = if draw_end {
                cap_radius
            } else if slot.is_last() {
                edge_radius
            } else {
                px(0.)
            };
            track = track.bg(accent.soft());
            track = track
                .rounded_tl(left)
                .rounded_bl(left)
                .rounded_tr(right)
                .rounded_br(right);
        }

        let mut cell = div()
            .id(ElementId::Name(key.clone().into()))
            // The debug selector lets the headless tests read the cell's
            // laid-out bounds.
            .debug_selector(move || key)
            .flex()
            .items_center()
            .justify_center()
            .size(frame.cell_size)
            .text_size(px(14.))
            .line_height(px(20.))
            .font_weight(gpui::FontWeight::MEDIUM)
            .child(match &self.cell {
                Some(render) => render(&RangeCalendarCellState {
                    date,
                    formatted_date: self.day_label(date).into(),
                    is_selected,
                    is_selection_start: is_start,
                    is_selection_end: is_end,
                    is_unavailable: unavailable,
                    is_outside_month: outside_month,
                    is_today,
                    is_disabled: disabled,
                }),
                None => self.day_label(date).into_any_element(),
            });

        if draw_start || draw_end {
            cell = cell
                .rounded_full()
                .bg(accent.color)
                .text_color(accent.foreground);
        } else if is_today {
            // `.range-calendar__cell[data-today]` fills the button with
            // `bg-accent-soft text-accent-soft-foreground`, today or inside
            // the range alike; only its hover (never on a selected cell)
            // deepens the same soft fill rather than `bg-default`.
            cell = cell
                .rounded_full()
                .bg(accent.soft())
                .text_color(accent.soft_foreground(colors.foreground));
            if selectable {
                cell = util::cursor_interactive(cell, cx);
                if !is_selected {
                    let hover_bg = accent.soft_hover();
                    cell = cell.hover(move |s| s.bg(hover_bg));
                }
            }
        } else if in_range {
            // The track fill lives on the outer cell, so the pressed inner
            // button stays transparent with the base foreground text and the
            // run never breaks.
        } else {
            cell = cell.rounded_full();
            if selectable {
                let hover_bg = self.day_hover_bg.unwrap_or(colors.default.color);
                cell = cell
                    .cursor(util::interactive_cursor(cx))
                    .hover(move |s| s.bg(hover_bg));
            }
        }

        // `.range-calendar__cell[data-pressed]` scales the inner button to
        // 0.9 and only the range caps recolour, to `bg-accent-hover` even
        // when the cap is also today; a pressed middle or today cell keeps
        // its soft fill, while a pressed plain day uses the default fill,
        // independently of `day_hover_bg`. The recolour merges with the press geometry
        // in one refinement -- a chained `.active` would overwrite it and
        // drop the scale.
        let cell = if selectable {
            let press_box = crate::anim::PressBox {
                height: frame.cell_size,
                padding_x: None,
                width: Some(frame.cell_size),
                min_width: None,
                text_size: px(14.),
                line_height: px(20.),
                gap: px(0.),
                radius: frame.cell_size / 2.,
                shrink_x: true,
                scale: crate::anim::PRESSED_SCALE_RANGE,
            };
            if draw_start || draw_end {
                crate::anim::pressed_with_background(cell, press_box, accent.hover(), cx)
            } else if is_today || in_range {
                crate::anim::pressed(cell, press_box, cx)
            } else {
                let pressed_bg = colors.default.color;
                crate::anim::pressed_with_background(cell, press_box, pressed_bg, cx)
            }
        } else {
            cell
        };

        // `.range-calendar__cell` takes `status-focused` -- a ring, not a border,
        // which would shrink the cell as the cursor arrived.
        // An overlay ring rather than a spread shadow: the shadow inherits the
        // element's own radius (a squarer arc than the `r + gap + 2` CSS draws)
        // and only paints with a blur. The asymmetric `rounded-ss-3xl` run caps
        // belong to the `track` underneath, not to this node -- the cell itself
        // is a uniform circle, or unrounded in mid-range, where the press slot
        // resolves the fallback `cell_size / 2` anyway -- so one radius says it
        // all. `cell` is that press slot once selectable: the resting footprint
        // holding the id, focus and corners while the skin takes the scale.
        let mut cell = util::with_focus_ring_overlay(
            cell,
            util::shows_focus_ring(
                !self.inert && !outside_month && frame.focused == Some(date),
                cx,
            ),
            true,
            frame.cell_size / 2.,
            Vec::new(),
            cx,
        );

        if disabled || unavailable {
            cell = cell
                .text_color(colors.muted)
                .cursor(gpui::CursorStyle::OperationNotAllowed)
                // The root owns the dim when the whole range calendar is
                // disabled, so cells do not compound that opacity.
                .when(!self.is_disabled, |day| day.opacity(cx.layout().disabled_opacity));
            if disabled && !outside_month {
                cell = cell.line_through();
            }
        }

        if selectable {
            // Tracking the hovered cell is what makes the half-open range
            // preview between the anchor and the cursor.
            let hover_state = self.state.clone();
            let hover_cursor = frame.cursor.clone();
            let hover_preview = frame.focus_preview.clone();
            let hover_focus = self.on_focus_change.clone();
            cell = cell.on_hover(move |over, window, cx| {
                let over = *over;
                let selecting = hover_state.update(cx, |s, cx| {
                    if over {
                        s.hovered = Some(date);
                    } else if s.hovered == Some(date) {
                        s.hovered = None;
                    }
                    cx.notify();
                    s.start.is_some() && s.end.is_none()
                });
                if over && selecting {
                    let focus_changed = hover_cursor.update(cx, |focused, cx| {
                        let changed = *focused != Some(date);
                        *focused = Some(date);
                        cx.notify();
                        changed
                    });
                    hover_preview.update(cx, |preview, _| *preview = true);
                    if focus_changed {
                        if let Some(cb) = &hover_focus {
                            cb(&date, window, cx);
                        }
                    }
                }
            });

            let state = self.state.clone();
            let on_change = self.on_change.clone();
            let on_focus = self.on_focus_change.clone();
            let constraints = frame.constraints.clone();
            let range_date_unavailable = self.range_date_unavailable.clone();
            let cursor = frame.cursor.clone();
            let focus_preview = frame.focus_preview.clone();
            let selection_before_anchor = frame.selection_before_anchor.clone();
            let anchor = frame.anchor;
            cell = cell.on_click(move |_, window, cx| {
                if let Some(cb) = &on_focus {
                    cb(&date, window, cx);
                }
                let (next, previous) = state.update(cx, |s, cx| {
                    let previous = s.start.zip(s.end);
                    let next = resolve_pick(
                        s.start,
                        s.end,
                        date,
                        &constraints,
                        range_date_unavailable.as_ref(),
                    );
                    if let Some((start, end)) = next {
                        s.start = Some(start);
                        s.end = end;
                        s.set_anchor(anchor);
                        cx.notify();
                    }
                    (next, previous)
                });
                cursor.update(cx, |focused, cx| {
                    *focused = Some(date);
                    cx.notify();
                });
                focus_preview.update(cx, |preview, _| *preview = false);
                selection_before_anchor.update(cx, |saved, _| {
                    if let Some((_, end)) = next {
                        *saved = if end.is_none() { previous } else { None };
                    }
                });
                if let (Some(cb), Some((start, Some(end)))) = (&on_change, next) {
                    cb(&start, &end, window, cx);
                }
            });
        }

        cell = cell
            .a11y_named(
                a11y::Role::Button,
                &a11y::Name::labelled(crate::calendar::day_cell_name(
                    format!("{} {}", date.day, month_year_heading(date.year, date.month)),
                    is_selected,
                    cx,
                )),
            )
            .a11y_selected(is_selected);

        let mut track = track.child(cell);

        // `.range-calendar__cell-indicator` is a `size-[3px] rounded-xs` dot
        // at `bottom-1`, centred in the cell, in the selected cell's
        // foreground when the day is chosen.
        if self.cell_indicator.as_ref().is_some_and(|f| f(date)) {
            let marker = if is_start || is_end || in_range {
                accent.foreground
            } else {
                colors.muted
            };
            track = track.child(
                div()
                    .absolute()
                    // The debug selector lets the headless tests read the
                    // dot's laid-out bounds.
                    .debug_selector(move || indicator_key)
                    .left((frame.cell_size - px(3.)) / 2.)
                    .bottom(px(4.))
                    .size(px(3.))
                    .rounded(px(2.))
                    .bg(marker),
            );
        }

        track
            .when(outside_month, |track| track.opacity(0.5))
            .into_any_element()
    }

    /// The seven column headers -- `.range-calendar__grid-header`.
    fn weekday_header(&self, cx: &App) -> gpui::Div {
        let muted = cx.colors().muted;
        let mut row = div().flex().flex_row().w_full();
        for (column, label) in self
            .constraints
            .first_day_of_week
            .header_row()
            .into_iter()
            .enumerate()
        {
            row = row.child(
                div()
                    .flex_1()
                    .debug_selector({
                        let key = format!("{:?}-weekday-{column}", self.id);
                        move || key
                    })
                    .pb(px(8.))
                    .text_center()
                    // `.range-calendar__header-cell` is `text-xs`.
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(muted)
                    .child(label),
            );
        }
        row
    }

    /// The heading over one visible month, named in the view calendar.
    fn month_heading_text(&self, year: i32, month: u32) -> String {
        let system = self.system();
        let (year, month) = system.add_months(year, month, self.year_heading_offset_months);
        let Some(date) = system.to_gregorian(year, month, 1) else {
            return String::new();
        };
        crate::calendar::date_heading_for_locale(system.locale(), date)
            .unwrap_or_else(|| month_year_heading(date.year, date.month))
    }

    /// The number a cell prints: the day of the month in the *view* calendar.
    fn day_label(&self, date: Date) -> String {
        self.system().from_gregorian(date).2.to_string()
    }

    /// One cell whose date the calendar system may not have; a triple it does
    /// not hold draws a blank rather than a wrong day.
    #[allow(clippy::too_many_arguments)]
    fn range_spill_cell(
        &self,
        date: Option<Date>,
        outside_month: bool,
        frame: &Frame<'_>,
        key: String,
        slot: CellSlot,
        cx: &App,
    ) -> gpui::AnyElement {
        match date {
            Some(date) => self.range_cell(date, outside_month, frame, key, slot, cx),
            None => div().size(frame.cell_size).into_any_element(),
        }
    }

    /// The 7-column grid for one month.
    fn month_grid(&self, y: i32, m: u32, frame: &Frame<'_>, cx: &App) -> gpui::AnyElement {
        let system = self.system();
        let lead = self.constraints.lead_cells_in(system, y, m);
        let total = system.days_in_month(y, m) as usize;
        let rows = self.constraints.rows_in(system, y, m);

        // `.range-calendar__grid-body` and `.range-calendar__grid-row` use
        // `display: contents` upstream. These flex rows preserve the seven
        // touching columns; the cells' `my-[2px]` becomes 4px between rows
        // and 2px at each body edge. The parent adds the first row's `mt-1`.
        let mut grid = div()
            .id(element_id::scoped(&self.id, format!("grid-{y}-{m}")))
            .flex()
            .flex_col()
            .gap(px(4.))
            .py(px(2.));
        for row_index in 0..rows {
            let mut row = div().flex().flex_row();
            for column in 0..7 {
                let index = row_index * 7 + column;
                let cell = if index < lead {
                    let (previous_year, previous_month) = system.add_months(y, m, -1);
                    let day = system.days_in_month(previous_year, previous_month) as usize - lead
                        + index
                        + 1;
                    self.range_spill_cell(
                        system.to_gregorian(previous_year, previous_month, day as u32),
                        true,
                        frame,
                        format!(
                            "{}-{y}-{m}-outside-{previous_year}-{previous_month}-day-{day}",
                            frame.base
                        ),
                        CellSlot { column, columns: 7 },
                        cx,
                    )
                } else {
                    let day = index - lead + 1;
                    if day <= total {
                        self.range_spill_cell(
                            system.to_gregorian(y, m, day as u32),
                            false,
                            frame,
                            format!("{}-{y}-{m}-day-{day}", frame.base),
                            CellSlot { column, columns: 7 },
                            cx,
                        )
                    } else {
                        let (next_year, next_month) = system.add_months(y, m, 1);
                        let next_day = day - total;
                        self.range_spill_cell(
                            system.to_gregorian(next_year, next_month, next_day as u32),
                            true,
                            frame,
                            format!(
                                "{}-{y}-{m}-outside-{next_year}-{next_month}-day-{next_day}",
                                frame.base
                            ),
                            CellSlot { column, columns: 7 },
                            cx,
                        )
                    }
                };
                row = row.child(cell);
            }
            grid = grid.child(row);
        }
        grid.a11y(a11y::Role::Grid).into_any_element()
    }

    /// The year grid shown while the year picker is open.
    fn year_grid(
        &self,
        view: calendar_view::YearGridView<'_>,
        year_focus: &gpui::FocusHandle,
        heading_focus: &gpui::FocusHandle,
        year_picker_own: Option<Entity<bool>>,
        window: &Window,
        cx: &App,
    ) -> gpui::AnyElement {
        let colors = cx.colors();
        let accent = if self.is_invalid {
            colors.danger
        } else {
            colors.accent
        };
        let active_year = view.active_year;
        // The debug selectors keep the component id's Debug form, which is what
        // the headless tests read; the element ids above stay structured.
        let base = format!("{:?}", view.base);
        let mut grid = div()
            .id(element_id::scoped(view.base, "year-viewport"))
            .debug_selector({
                let key = format!("{base}-year-viewport");
                move || key
            })
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&view.scroll.handle)
            .flex()
            .flex_col()
            .gap(px(4.));
        for chunk in view.years.chunks(3) {
            let mut row = div().flex().flex_shrink_0().gap(px(4.));
            for &year in chunk {
                let is_active = year == active_year;
                let mut cell = div()
                    .id(element_id::scoped(view.base, format!("y{year}")))
                    .debug_selector({
                        let key = format!("{base}-y{year}");
                        move || key
                    })
                    .when(!self.is_disabled && !self.inert && is_active, |cell| {
                        cell.track_focus(year_focus)
                    })
                    .flex_1()
                    .h(px(32.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .rounded(util::control_radius(cx));
                if is_active {
                    cell = cell.bg(accent.color).text_color(accent.foreground);
                } else if !self.is_disabled && !self.inert {
                    // `.calendar-year-picker__year-cell:hover` fills
                    // `bg-default text-default-foreground`.
                    let hover_bg = self.year_hover_bg.unwrap_or(colors.default.color);
                    let hover_fg = colors.default.foreground;
                    cell = cell
                        .text_color(colors.foreground)
                        .cursor(util::interactive_cursor(cx))
                        .hover(move |s| s.bg(hover_bg).text_color(hover_fg));
                }
                if !self.is_disabled && !self.inert {
                    let st = self.state.clone();
                    let on_open = self.on_year_picker_open_change.clone();
                    let on_focus = self.on_focus_change.clone();
                    let own = year_picker_own.clone();
                    let back_to_trigger = heading_focus.clone();
                    let system = self.system().clone();
                    cell = cell.on_click(move |_, window, cx| {
                        let next = st.update(cx, |s, cx| {
                            let anchor = s.anchor();
                            let next =
                                system.add_years(anchor, year - system.from_gregorian(anchor).0);
                            s.set_anchor(next);
                            cx.notify();
                            next
                        });
                        if year != active_year {
                            if let Some(cb) = &on_focus {
                                cb(&next, window, cx);
                            }
                        }
                        if let Some(held) = &own {
                            held.update(cx, |open, cx| {
                                *open = false;
                                cx.notify();
                            });
                        }
                        if let Some(cb) = &on_open {
                            cb(&false, window, cx);
                        }
                        window.focus(&back_to_trigger, cx);
                    });
                }
                if is_active {
                    cell = util::ring_overlay_if_focused(
                        cell,
                        year_focus,
                        false,
                        util::control_radius(cx),
                        Vec::new(),
                        window,
                        cx,
                    );
                }
                cell = cell
                    .a11y_named(a11y::Role::Button, &a11y::Name::labelled(year.to_string()))
                    .a11y_selected(is_active);
                row = row.child(cell.child(year.to_string()));
            }
            row = row.children((chunk.len()..3).map(|_| div().flex_1().px(px(10.))));
            grid = grid.child(row);
        }
        grid = grid.a11y(a11y::Role::Grid);
        div()
            .absolute()
            .inset_0()
            .p(px(4.))
            .child(grid)
            .when_some(view.reveal_row, |viewport, row| {
                let scroll = view.scroll.clone();
                viewport.on_children_prepainted(move |_, window, cx| {
                    scroll.last_target.set(Some((active_year, row)));
                    scroll.handle.scroll_to_item(row);
                    window.defer(cx, |window, _| window.refresh());
                })
            })
            .into_any_element()
    }
}

impl RenderOnce for RangeCalendar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `value` / `defaultValue` seed the state once, before anything reads
        // it. `value` is v3's controlled spelling, so it outranks the
        // uncontrolled seed; the state owns the range afterwards, and a write
        // to the caller-owned entity is the imperative update.
        if let Some((start, end)) = self.value {
            let state = self.state.clone();
            util::seed_once(
                window,
                cx,
                ElementId::named_usize(
                    "rangecalendar-default",
                    self.state.entity_id().as_u64() as usize,
                ),
                move |cx| {
                    state.update(cx, |s, cx| {
                        s.start = start;
                        s.end = end;
                        if let Some(start) = start {
                            s.view_year = start.year;
                            s.view_month = start.month;
                            s.view_day = start.day;
                        }
                        cx.notify();
                    });
                },
            );
        } else if let Some(value) = self.default_value {
            let state = self.state.clone();
            util::seed_once(
                window,
                cx,
                ElementId::named_usize(
                    "rangecalendar-default",
                    self.state.entity_id().as_u64() as usize,
                ),
                move |cx| {
                    state.update(cx, |s, cx| {
                        s.start = Some(value.0);
                        s.end = Some(value.1);
                        s.view_year = value.0.year;
                        s.view_month = value.0.month;
                        s.view_day = value.0.day;
                        cx.notify();
                    });
                },
            );
        }

        let base = format!("{:?}", self.id);

        // `isYearPickerOpen` wins; without it the component holds the flag and
        // the heading toggles it, which is what `defaultYearPickerOpen`
        // promises. This borrows `cx` mutably, so it precedes the tokens.
        let (year_picker_open, year_picker_own) = util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "yearpicker"),
            self.year_picker_open,
            self.default_year_picker_open,
        );

        // The grid is one tab stop with a cursor inside it, as the Calendar's is.
        // `use_keyed_state` takes `cx` mutably, so both precede the theme.
        let grid_focus = util::tab_stop_handle(element_id::scoped(&self.id, "focus"), window, cx);
        let prev_focus =
            util::tab_stop_handle(element_id::scoped(&self.id, "prev-focus"), window, cx);
        let next_focus =
            util::tab_stop_handle(element_id::scoped(&self.id, "next-focus"), window, cx);
        let year_focus =
            util::tab_stop_handle(element_id::scoped(&self.id, "year-focus"), window, cx);
        // Inside a picker the grid takes the focus as the panel opens, so the
        // arrows work without hunting for it with Tab.
        if self.autofocus_grid && !self.is_disabled && !self.inert && !year_picker_open {
            util::focus_once(
                window,
                cx,
                element_id::scoped(&self.id, "autofocus"),
                &grid_focus,
            );
        }
        let cursor = window.use_keyed_state(element_id::scoped(&self.id, "cursor"), cx, |_, _| {
            None::<Date>
        });
        let focus_preview =
            window.use_keyed_state(element_id::scoped(&self.id, "focus-preview"), cx, |_, _| {
                false
            });
        let selection_before_anchor = window.use_keyed_state(
            element_id::scoped(&self.id, "selection-before-anchor"),
            cx,
            |_, _| None::<(Date, Date)>,
        );
        let year_cursor =
            window.use_keyed_state(element_id::scoped(&self.id, "year-cursor"), cx, |_, _| {
                None::<i32>
            });
        let year_was_open =
            window.use_keyed_state(element_id::scoped(&self.id, "year-was-open"), cx, |_, _| {
                false
            });
        let year_trigger_index = window.use_keyed_state(
            element_id::scoped(&self.id, "year-trigger-index"),
            cx,
            |_, _| 0usize,
        );
        let cursor_at = *cursor.read(cx);
        let focus_preview_at = *focus_preview.read(cx);

        let (stored_anchor, selection_start, selection_end, hovered, navigated) = {
            let st = self.state.read(cx);
            (st.anchor(), st.start, st.end, st.hovered, st.user_navigated)
        };
        let effective_constraints = effective_range_constraints(
            self.system(),
            selection_start.filter(|_| selection_end.is_none()),
            self.duration,
            &self.constraints,
            self.range_date_unavailable.as_ref(),
            self.allows_non_contiguous_ranges,
        );
        let first_day = effective_constraints.first_day_of_week;
        let focused_value = self
            .focused_value
            .map(|date| effective_constraints.constrain(date));
        let active_preview = hovered.or(focus_preview_at.then_some(cursor_at).flatten());
        let (paint_start, preview_end) = match (selection_start, selection_end, active_preview) {
            (Some(start), None, Some(preview)) => resolve_pick(
                Some(start),
                None,
                preview,
                &effective_constraints,
                self.range_date_unavailable.as_ref(),
            )
            .map_or((Some(start), None), |(start, end)| (Some(start), end)),
            _ => (selection_start, selection_end),
        };
        // Pinned React Stately starts a long range at the first visible unit
        // when its end would fall beyond the default centered window. An
        // explicit `selectionAlignment` always wins.
        let selection_alignment =
            self.selection_alignment
                .unwrap_or_else(|| match (selection_start, selection_end) {
                    (Some(start), Some(end)) => {
                        let centered_anchor = calendar_view::aligned_anchor_in(
                            self.system(),
                            self.duration,
                            SelectionAlignment::Center,
                            first_day,
                            start,
                        );
                        let (_, centered_end) = calendar_view::visible_range(
                            self.system(),
                            self.duration,
                            first_day,
                            centered_anchor,
                        );
                        if days_from_civil(&end) > days_from_civil(&centered_end) {
                            SelectionAlignment::Start
                        } else {
                            SelectionAlignment::Center
                        }
                    }
                    _ => SelectionAlignment::Center,
                });
        // `selectionAlignment` frames the range around the selection start,
        // until the user drives navigation themselves.
        let anchor = match (navigated, selection_start) {
            (false, Some(sel)) => calendar_view::aligned_anchor_in(
                self.system(),
                self.duration,
                selection_alignment,
                first_day,
                sel,
            ),
            _ => stored_anchor,
        };
        let anchor = focused_value.map_or(anchor, |focused| {
            let (visible_start, visible_end) =
                calendar_view::visible_range(self.system(), self.duration, first_day, anchor);
            calendar_view::anchor_following_focus_in(
                self.system(),
                self.duration,
                first_day,
                anchor,
                visible_start,
                visible_end,
                focused,
            )
        });
        let initial_year = self
            .system()
            .from_gregorian(focused_value.unwrap_or(anchor))
            .0;
        let years = calendar_view::year_window(
            self.system(),
            initial_year,
            self.visible_years,
            effective_constraints.min_value,
            effective_constraints.max_value,
        );
        let first_year = years.first().copied().unwrap_or(initial_year);
        let last_year = years.last().copied().unwrap_or(initial_year);
        if year_picker_open && !*year_was_open.read(cx) && !self.is_disabled && !self.inert {
            year_cursor.update(cx, |year, _| *year = Some(initial_year));
            window.focus(&year_focus, cx);
        }
        let year_just_opened = year_picker_open && !*year_was_open.read(cx);
        year_was_open.update(cx, |was_open, _| *was_open = year_picker_open);
        let active_year = focused_value
            .map(|date| self.system().from_gregorian(date).0)
            .or(*year_cursor.read(cx))
            .unwrap_or(initial_year)
            .max(first_year)
            .min(last_year);
        let year_scroll_state =
            window.use_keyed_state(element_id::scoped(&self.id, "year-scroll"), cx, |_, _| {
                std::rc::Rc::new(calendar_view::YearGridScroll::default())
            });
        let year_scroll = year_scroll_state.read(cx).clone();
        let reveal_year_row =
            years
                .iter()
                .position(|year| *year == active_year)
                .and_then(|index| {
                    let row = index / 3;
                    (year_picker_open
                        && (year_just_opened
                            || year_scroll.last_target.get() != Some((active_year, row))))
                    .then_some(row)
                });

        // Taking the focus puts the ring where v3 would have focused -- the
        // range's start, or today.
        let ring_at = focused_value
            .or(cursor_at)
            .or(selection_start)
            .or_else(|| Some(Date::today()))
            .filter(|_| grid_focus.is_focused(window));
        let column_width = if matches!(self.duration, VisibleDuration::Months(n) if n > 1) {
            px(256.)
        } else {
            crate::calendar::CALENDAR_WIDTH
        };
        let frame = Frame {
            start: paint_start,
            preview_end,
            unavailable_anchor: selection_start.filter(|_| selection_end.is_none()),
            constraints: &effective_constraints,
            today: Date::today(),
            cursor: &cursor,
            focus_preview: &focus_preview,
            selection_before_anchor: &selection_before_anchor,
            base: &base,
            focused: ring_at,
            anchor,
            visible_start: matches!(self.duration, VisibleDuration::Days(_)).then_some(anchor),
            cell_size: column_width / 7.,
        };

        let months = calendar_view::month_headings_in(self.system(), self.duration, anchor);
        let linear = calendar_view::linear_cells(self.duration, first_day, anchor);
        let columns = months.len().max(1);
        let mut heading_focuses = Vec::with_capacity(columns);
        for index in 0..columns {
            heading_focuses.push(util::tab_stop_handle(
                element_id::scoped(&element_id::indexed(&self.id, "heading", index), "focus"),
                window,
                cx,
            ));
        }
        let active_heading_index = (*year_trigger_index.read(cx)).min(columns - 1);
        let active_heading_focus = heading_focuses[active_heading_index].clone();

        let colors = cx.colors().clone();
        let layout = cx.layout().clone();

        let nav_target = |dir: i32| {
            calendar_view::page_in(
                self.system(),
                self.duration,
                self.page_behavior,
                anchor,
                dir,
            )
        };
        let (visible_start, visible_end) =
            calendar_view::visible_range(self.system(), self.duration, first_day, anchor);
        // React Stately checks only the day immediately outside the visible
        // range against minValue/maxValue. A raw unavailable date does not
        // block paging, but an open contiguous range promotes the nearest
        // barrier to an effective bound above. Read-only still permits paging.
        let previous_disabled =
            self.is_disabled || effective_constraints.out_of_range(add_days(&visible_start, -1));
        let next_disabled =
            self.is_disabled || effective_constraints.out_of_range(add_days(&visible_end, 1));
        let state_for_nav = self.state.clone();
        let nav_btn = |icon: &'static str,
                       target: Date,
                       key: String,
                       focus: &gpui::FocusHandle,
                       disabled: bool,
                       inert: bool,
                       window: &mut Window,
                       cx: &mut App| {
            let state = state_for_nav.clone();
            // `.range-calendar__nav-button:hover` fills with `bg-default`.
            let hover_bg = self.nav_hover_bg.unwrap_or(colors.default.color);
            let debug_key = key.clone();
            let button_id = ElementId::Name(key.clone().into());
            let radius = util::small_radius(cx);
            let nav_name = crate::i18n::ui_string(
                if key.contains("-prev") {
                    crate::i18n::UiString::Previous
                } else {
                    crate::i18n::UiString::Next
                },
                cx,
            );
            // `.range-calendar__nav-button:active` scales the box to 0.95.
            let press_box = crate::anim::PressBox {
                height: px(24.),
                padding_x: None,
                width: Some(px(24.)),
                min_width: None,
                text_size: px(16.),
                line_height: px(0.),
                gap: px(0.),
                radius: util::small_radius(cx),
                shrink_x: true,
                scale: crate::anim::PRESSED_SCALE_DEEP,
            };
            // The icon joins the skin after the fade's fill — the fill is
            // appended and would paint over a child already present — and
            // before the press wrap, whose slot would inherit children added
            // after it.
            let nav_icon = || {
                gpui::svg()
                    // `.range-calendar__nav-button-icon` is `size-4`, painted
                    // `text-accent-soft-foreground` like its button.
                    .size(px(16.))
                    .path(icon)
                    .text_color(colors.accent.soft_foreground(colors.foreground))
            };
            let button = div()
                .id(button_id.clone())
                // The debug selector lets the headless tests read the
                // button's laid-out bounds.
                .debug_selector(move || debug_key)
                .when(!disabled && !inert, |b| b.track_focus(focus))
                .flex()
                .items_center()
                .justify_center()
                // `.range-calendar__nav-button` is `size-6 rounded-xl` -- one
                // radius step tighter than the single calendar's, which is
                // `rounded-2xl`.
                .size(px(24.))
                .rounded(util::small_radius(cx))
                .when(!disabled && !inert, |b| {
                    let pressed = b
                        .cursor(util::interactive_cursor(cx))
                        .on_click(move |_, _, cx| {
                            state.update(cx, |s, cx| {
                                s.set_anchor(target);
                                cx.notify();
                            });
                        });
                    let pressed = crate::anim::hover_fade_with_duration(
                        pressed,
                        element_id::scoped(&button_id, "hover-fade"),
                        (herogpui_core::with_alpha(colors.default.color, 0.0), hover_bg),
                        None,
                        None,
                        move |fill| fill.rounded(radius),
                        Some(100),
                        window,
                        cx,
                    );
                    let pressed = pressed.child(nav_icon());
                    crate::anim::pressed(pressed, press_box, cx)
                })
                .when(disabled || inert, |b| b.child(nav_icon()))
                .when(disabled, |b| b.opacity(layout.disabled_opacity))
                .when(year_picker_open, |b| b.invisible());
            let button = if inert {
                button
            } else {
                // The ring goes on the press slot `pressed` returned, which is
                // the node carrying `rounded(small_radius)` and the focus; the
                // skin inside only carries the 0.95 scale.
                util::ring_overlay_if_focused(button, focus, true, radius, Vec::new(), window, cx)
            };
            button.a11y_named(a11y::Role::Button, &a11y::Name::labelled(nav_name))
        };

        // A heading is a plain label only when the picker is controlled without
        // a change handler; otherwise it is the built-in year-picker trigger.
        let heading = |text: String,
                       key: String,
                       focus: &gpui::FocusHandle,
                       index: usize,
                       window: &mut Window,
                       cx: &mut App|
         -> gpui::AnyElement {
            let heading_name = text.clone();
            let label = div()
                .text_size(px(14.))
                .line_height(px(20.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(text);
            let indicator_id =
                element_id::scoped(&self.id, format!("year-picker-indicator-{index}"));
            // Without the composed `YearPickerTrigger` the heading is a plain
            // label, whatever state or handler the picker props carry.
            if !self.year_picker {
                return label.into_any_element();
            }
            match &self.on_year_picker_open_change {
                // No handler and no state of our own: a plain label.
                None if year_picker_own.is_none() => label.into_any_element(),
                None => {
                    let own = year_picker_own.clone();
                    let opener = year_trigger_index.clone();
                    let open = year_picker_open;
                    let trigger = div()
                        .id(ElementId::Name(key.into()))
                        .when(!self.is_disabled && !self.inert, |trigger| trigger.track_focus(focus))
                        .flex()
                        .items_center()
                        // `.calendar-year-picker__trigger` is `gap-1 rounded-lg`
                        // and hovers nothing: only focus and the open state
                        // recolour it.
                        .gap(px(4.))
                        .px(px(6.))
                        .py(px(2.))
                        .rounded(util::key_radius(cx))
                        .when(!self.is_disabled && !self.inert, |trigger| {
                            trigger
                                .cursor(util::interactive_cursor(cx))
                                .on_click(move |_, _, cx| {
                                    opener.update(cx, |value, _| *value = index);
                                    if let Some(held) = &own {
                                        held.update(cx, |value, cx| {
                                            *value = !open;
                                            cx.notify();
                                        });
                                    }
                                })
                        })
                        .when(self.is_disabled, |trigger| {
                            trigger.opacity(layout.disabled_opacity)
                        });
                    let trigger = if self.inert {
                        trigger
                    } else {
                        util::ring_overlay_if_focused(
                            trigger,
                            focus,
                            true,
                            util::key_radius(cx),
                            Vec::new(),
                            window,
                            cx,
                        )
                    };
                    trigger
                        .a11y_named(a11y::Role::Button, &a11y::Name::labelled(heading_name))
                        .a11y_expanded(open)
                        .child(label)
                        .child(crate::anim::rotating_indicator_with_angle_ease_out(
                            &indicator_id,
                            open,
                            gpui::svg()
                                .size(px(12.))
                                .path(icons::CHEVRON_DOWN)
                                .text_color(colors.muted),
                            crate::anim::YEAR_PICKER_INDICATOR_MS,
                            crate::anim::YEAR_PICKER_INDICATOR_ANGLE,
                            window,
                            cx,
                        ))
                        .into_any_element()
                }
                Some(cb) => {
                    let cb = cb.clone();
                    let open = year_picker_open;
                    let own = year_picker_own.clone();
                    let opener = year_trigger_index.clone();
                    let trigger = div()
                        .id(ElementId::Name(key.into()))
                        .when(!self.is_disabled && !self.inert, |trigger| trigger.track_focus(focus))
                        .flex()
                        .items_center()
                        // `.calendar-year-picker__trigger` is `gap-1 rounded-lg`
                        // and hovers nothing: only focus and the open state
                        // recolour it.
                        .gap(px(4.))
                        .px(px(6.))
                        .py(px(2.))
                        .rounded(util::key_radius(cx))
                        .when(!self.is_disabled && !self.inert, |trigger| {
                            trigger
                                .cursor(util::interactive_cursor(cx))
                                .on_click(move |_, window, cx| {
                                    opener.update(cx, |value, _| *value = index);
                                    // Uncontrolled: flip our own copy too, or
                                    // the callback would be the only opener.
                                    if let Some(held) = &own {
                                        held.update(cx, |value, cx| {
                                            *value = !open;
                                            cx.notify();
                                        });
                                    }
                                    cb(&!open, window, cx);
                                })
                        })
                        .when(self.is_disabled, |trigger| {
                            trigger.opacity(layout.disabled_opacity)
                        });
                    let trigger = if self.inert {
                        trigger
                    } else {
                        util::ring_overlay_if_focused(
                            trigger,
                            focus,
                            true,
                            util::key_radius(cx),
                            Vec::new(),
                            window,
                            cx,
                        )
                    };
                    trigger
                        .a11y_named(a11y::Role::Button, &a11y::Name::labelled(heading_name))
                        .a11y_expanded(open)
                        .child(label)
                        .child(crate::anim::rotating_indicator_with_angle_ease_out(
                            &indicator_id,
                            open,
                            gpui::svg()
                                .size(px(12.))
                                .path(icons::CHEVRON_DOWN)
                                .text_color(colors.muted),
                            crate::anim::YEAR_PICKER_INDICATOR_MS,
                            crate::anim::YEAR_PICKER_INDICATOR_ANGLE,
                            window,
                            cx,
                        ))
                        .into_any_element()
                }
            }
        };

        let mut root = div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .text_color(colors.surface.foreground)
            // readOnly blocks selection, not focus or navigation.
            .when(!self.is_disabled && !self.inert && !year_picker_open, |el| {
                el.track_focus(&grid_focus)
            });

        // The same keys the Calendar answers, and Enter picks: the first press
        // sets the range's start, the second its end, which is what `pick` does
        // for a click.
        if !self.is_disabled && !self.inert && !year_picker_open {
            let held = cursor.clone();
            let focus_preview = focus_preview.clone();
            let selection_before_anchor = selection_before_anchor.clone();
            let focus = grid_focus.clone();
            let prev_control = prev_focus.clone();
            let next_control = next_focus.clone();
            let heading_controls = heading_focuses.clone();
            let controlled_focus = focused_value;
            let from_start = focused_value
                .or(selection_start)
                .unwrap_or_else(Date::today);
            let state = self.state.clone();
            let on_change = self.on_change.clone();
            let on_focus = self.on_focus_change.clone();
            let constraints = effective_constraints.clone();
            let range_date_unavailable = self.range_date_unavailable.clone();
            let allows_non_contiguous_ranges = self.allows_non_contiguous_ranges;
            let read_only = self.is_read_only;
            let system = self.system().clone();
            let duration = self.duration;
            let page_behavior = self.page_behavior;
            let grid = calendar_keys::DayGrid {
                system: system.clone(),
                duration,
                page_behavior,
                first_day,
                anchor,
                visible_start,
                visible_end,
            };
            root = root.on_key_down(move |event, window, cx| {
                let header_focused = prev_control.is_focused(window)
                    || next_control.is_focused(window)
                    || heading_controls
                        .iter()
                        .any(|handle| handle.is_focused(window));
                if header_focused || !focus.contains_focused(window, cx) {
                    return;
                }
                let at = controlled_focus.or(*held.read(cx)).unwrap_or(from_start);
                let key = event.keystroke.key.as_str();
                let shift = event.keystroke.modifiers.shift;
                if key == "escape" {
                    let restore = *selection_before_anchor.read(cx);
                    let cancelled = state.update(cx, |s, cx| {
                        if s.start.is_some() && s.end.is_none() {
                            if let Some((start, end)) = restore {
                                s.start = Some(start);
                                s.end = Some(end);
                            } else {
                                s.start = None;
                            }
                            s.hovered = None;
                            cx.notify();
                            true
                        } else {
                            false
                        }
                    });
                    focus_preview.update(cx, |preview, _| *preview = false);
                    if cancelled {
                        selection_before_anchor.update(cx, |saved, _| *saved = None);
                    }
                    return;
                }
                if matches!(key, "enter" | "space") {
                    cx.stop_propagation();
                    if event.is_held || read_only {
                        return;
                    }
                    let (next, previous) = state.update(cx, |s, cx| {
                        let previous = s.start.zip(s.end);
                        let next = resolve_pick(
                            s.start,
                            s.end,
                            at,
                            &constraints,
                            range_date_unavailable.as_ref(),
                        );
                        if let Some((start, end)) = next {
                            s.start = Some(start);
                            s.end = end;
                            s.hovered = None;
                            s.set_anchor(anchor);
                            cx.notify();
                        }
                        (next, previous)
                    });
                    if let (Some(cb), Some((start, Some(end)))) = (&on_change, next) {
                        cb(&start, &end, window, cx);
                    }
                    if let Some((_, end)) = next {
                        focus_preview.update(cx, |preview, _| *preview = end.is_none());
                        selection_before_anchor.update(cx, |saved, _| {
                            *saved = if end.is_none() { previous } else { None };
                        });
                    }
                    if let Some(next_focus) = next.and_then(|(_, end)| {
                        end.is_none().then(|| {
                            keyboard_range_focus(
                                at,
                                &constraints,
                                range_date_unavailable.as_ref(),
                                allows_non_contiguous_ranges,
                            )
                        })?
                    }) {
                        if controlled_focus.is_none() {
                            held.update(cx, |focused, cx| {
                                *focused = Some(next_focus);
                                cx.notify();
                            });
                            state.update(cx, |s, cx| {
                                let next_anchor = calendar_view::anchor_following_focus_in(
                                    &system,
                                    duration,
                                    first_day,
                                    anchor,
                                    visible_start,
                                    visible_end,
                                    next_focus,
                                );
                                if next_anchor != anchor {
                                    s.set_anchor(next_anchor);
                                    cx.notify();
                                }
                            });
                        }
                        if let Some(cb) = &on_focus {
                            cb(&next_focus, window, cx);
                        }
                    }
                    return;
                }
                let Some(next) = grid.key_target(key, at, shift) else {
                    return;
                };
                let next = constraints.constrain(next);
                if controlled_focus.is_none() {
                    held.update(cx, |v, cx| {
                        *v = Some(next);
                        cx.notify();
                    });
                    focus_preview.update(cx, |preview, _| *preview = true);
                    // React Aria realigns a week/month window only after focus
                    // leaves it. Day views page the whole window directly.
                    state.update(cx, |s, cx| {
                        s.hovered = None;
                        let next_anchor = grid.anchor_after_key(key, next, shift);
                        if next_anchor != anchor {
                            s.set_anchor(next_anchor);
                            cx.notify();
                        }
                    });
                }
                if let Some(cb) = &on_focus {
                    cb(&next, window, cx);
                }
            });
        }

        if !self.is_disabled && !self.inert && year_picker_open {
            let held = year_cursor;
            let focus = year_focus.clone();
            let years_for_keys = years.clone();
            let own = year_picker_own.clone();
            let on_open = self.on_year_picker_open_change.clone();
            let on_focus = self.on_focus_change.clone();
            let system = self.system().clone();
            let controlled_year = focused_value.map(|date| system.from_gregorian(date).0);
            let back_to_trigger = active_heading_focus.clone();
            root = calendar_keys::on_year_picker_keys(
                root,
                calendar_keys::YearPickerKeys {
                    held,
                    focus,
                    years: years_for_keys,
                    own,
                    on_open,
                    on_focus,
                    system,
                    controlled_year,
                    back_to_trigger,
                    active_year,
                    anchor,
                },
            );
        }

        // `.range-calendar__grid-body > tr:first-child > td` has `mt-1`.
        // Day view moves that 4px onto the body itself; this header/body gap
        // owns it for both linear views. Month views use the column gap below.
        let mut body = div().flex().flex_col().gap(px(4.));
        if self.duration.is_month_view() {
            root = root.w(column_width * columns as f32 + px(32.) * (columns - 1) as f32);
            let mut row = div().flex().gap(px(32.));
            let mut headings = div().flex().gap(px(32.)).pb(px(16.));
            for (i, &(y, m)) in months.iter().enumerate() {
                let first = i == 0;
                let last = i + 1 == columns;
                // First-body-row margin, separate from each cell's 2px margin.
                let mut col = div().flex().flex_col().gap(px(4.)).w(column_width);
                // Only the outer columns carry nav buttons; the rest keep a
                // same-size spacer so every heading lines up.
                // The same box as a nav button, so every heading lines up.
                let spacer = || div().size(px(24.)).into_any_element();
                headings = headings.child(
                    div().w(column_width).child(
                        div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .w_full()
                        // `.range-calendar__header` is `px-0.5`.
                        .px(px(2.))
                        .child(if first {
                            nav_btn(
                                icons::CHEVRON_LEFT,
                                nav_target(-1),
                                format!("{base}-prev"),
                                &prev_focus,
                                previous_disabled,
                                self.inert,
                                window,
                                cx,
                            )
                                .into_any_element()
                        } else {
                            spacer()
                        })
                        .child(heading(
                            self.month_heading_text(y, m),
                            format!("{base}-heading{i}"),
                            &heading_focuses[i],
                            i,
                            window,
                            cx,
                        ))
                        .child(if last {
                            nav_btn(
                                icons::CHEVRON_RIGHT,
                                nav_target(1),
                                format!("{base}-next"),
                                &next_focus,
                                next_disabled,
                                self.inert,
                                window,
                                cx,
                            )
                                .into_any_element()
                        } else {
                            spacer()
                        }),
                    ),
                );
                col = col.child(self.weekday_header(cx));
                col = col.child(self.month_grid(y, m, &frame, cx));
                row = row.child(col);
            }
            root = root.child(headings);
            body = body.child(row);
        } else {
            root = root.w(crate::calendar::CALENDAR_WIDTH);
            root = root.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .w_full()
                    // `.range-calendar__header` is `px-0.5`.
                    .px(px(2.))
                    .pb(px(16.))
                    .child(nav_btn(
                        icons::CHEVRON_LEFT,
                        nav_target(-1),
                        format!("{base}-prev"),
                        &prev_focus,
                        previous_disabled,
                        self.inert,
                        window,
                        cx,
                    ))
                    .child(heading(
                        calendar_view::range_heading(&linear),
                        format!("{base}-heading"),
                        &heading_focuses[0],
                        0,
                        window,
                        cx,
                    ))
                    .child(nav_btn(
                        icons::CHEVRON_RIGHT,
                        nav_target(1),
                        format!("{base}-next"),
                        &next_focus,
                        next_disabled,
                        self.inert,
                        window,
                        cx,
                    )),
            );
            body = body.child(self.weekday_header(cx));
            // The linear `.range-calendar__grid-body` / `__grid-row` use the
            // same cell margins and seven columns as month_grid, including
            // empty slots at the boundaries of a rolling day view.
            let mut grid = div()
                .id(element_id::scoped(&self.id, "grid"))
                .flex()
                .flex_col()
                .gap(px(4.))
                .py(px(2.));
            for row in calendar_view::week_aligned_rows(visible_start, visible_end, first_day) {
                let mut line = div().flex().flex_row();
                for (column, date) in row.into_iter().enumerate() {
                    line = line.child(match date {
                        Some(date) => self.range_cell(
                            date,
                            false,
                            &frame,
                            format!("{base}-{}", date.format_iso()),
                            CellSlot { column, columns: 7 },
                            cx,
                        ),
                        None => div().size(px(36.)).into_any_element(),
                    });
                }
                grid = grid.child(line);
            }
            body = body.child(grid.a11y(a11y::Role::Grid));
        }

        let mut viewport = div()
            .relative()
            .child(body.when(year_picker_open, |body| body.invisible()));
        if year_picker_open {
            viewport = viewport.child(self.year_grid(
                calendar_view::YearGridView {
                    years: &years,
                    active_year,
                    base: &self.id,
                    scroll: &year_scroll,
                    reveal_row: reveal_year_row,
                },
                &year_focus,
                &active_heading_focus,
                year_picker_own.clone(),
                window,
                cx,
            ));
        }
        root = root.child(viewport);

        if self.is_disabled {
            root = root.opacity(layout.disabled_opacity);
        }

        root = util::apply_sx(root, &self.sx);
        let app_name = if let Some(&(year, month)) = months.first() {
            self.month_heading_text(year, month)
        } else {
            String::new()
        };
        root = root.a11y_named(a11y::Role::Application, &a11y::Name::labelled(app_name));
        root = util::record_focus_bounds(root, &grid_focus, window, cx);

        if columns > 1 {
            let heading = heading_focuses
                .iter()
                .position(|focus| focus.is_focused(window))
                .or_else(|| prev_focus.is_focused(window).then_some(0))
                .or_else(|| next_focus.is_focused(window).then_some(columns - 1));
            let reveal = heading
                .map(|index| {
                    let left = (column_width + px(32.)) * index as f32;
                    (
                        calendar_view::MonthScrollFocus::Heading(index),
                        left,
                        left + column_width,
                    )
                })
                .or_else(|| {
                    if year_picker_open {
                        years
                            .iter()
                            .position(|year| *year == active_year)
                            .map(|index| {
                                let width =
                                    column_width * columns as f32 + px(32.) * (columns - 1) as f32;
                                // The year grid has 4px padding and two 4px column gaps.
                                let cell = (width - px(16.)) / 3.;
                                let left = px(4.) + (cell + px(4.)) * (index % 3) as f32;
                                (
                                    calendar_view::MonthScrollFocus::Year(active_year),
                                    left,
                                    left + cell,
                                )
                            })
                    } else {
                        let date = focused_value.or(ring_at)?;
                        let (year, month, _) = self.system().from_gregorian(date);
                        let index = months.iter().position(|value| *value == (year, month))?;
                        let column = (crate::calendar::weekday_index(date) + 7
                            - first_day.monday_index())
                            % 7;
                        let left = (column_width + px(32.)) * index as f32
                            + frame.cell_size * column as f32;
                        Some((
                            calendar_view::MonthScrollFocus::Day(date),
                            left,
                            left + frame.cell_size,
                        ))
                    }
                });
            calendar_view::scrolling_months(
                root.flex_shrink_0().mx_auto().into_any_element(),
                &self.id,
                reveal,
                window,
                cx,
            )
        } else {
            root.into_any_element()
        }
    }
}

// The pinned today cell fills `bg-accent-soft` with
// `text-accent-soft-foreground` and hovers `bg-accent-soft-hover`; the old
// port invented an accent border, so the check is mechanical.
#[cfg(test)]
mod hover_tokens {
    #[test]
    fn the_today_cell_uses_the_accent_soft_tokens() {
        // Scan the implementation only; this test's own text names the
        // forbidden accessor.
        let source = include_str!("range_calendar.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.matches(".bg(accent.soft())").count() >= 2,
            "the today cell must fill `bg-accent-soft` like the range \
             interior (pinned `.range-calendar__cell[data-today]`)"
        );
        assert!(
            source.contains("let hover_bg = accent.soft_hover();"),
            "the today cell must hover `bg-accent-soft-hover` \
             (pinned `.range-calendar__cell[data-today]:hover`)"
        );
        assert!(
            !source.contains("cell.border_1()"),
            "the today cell must not invent an accent border"
        );
    }

    // gpui's `active` refinement overwrites the previous one, so chaining
    // `.active` after `anim::pressed` would drop the 0.9 press scale. The
    // pressed recolour must merge with the geometry in one refinement, which
    // is what `pressed_with_background` exists for.
    #[test]
    fn the_pressed_cell_merges_background_with_the_scale() {
        // Scan the implementation only; this test's own text names the
        // forbidden chaining.
        let source = include_str!("range_calendar.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            !source.contains(".active("),
            "a chained `.active` after `anim::pressed` replaces the pressed \
             scale (gpui overwrites the active refinement)"
        );
    }

    // Pinned pressed state: only the range caps recolour, to
    // `bg-accent-hover` even when the cap is also today; a pressed middle or
    // today cell keeps its `bg-accent-soft` fill, and a pressed plain day
    // shows the hover fill it already wears.
    #[test]
    fn the_pressed_caps_recolour_and_the_interior_keeps_its_fill() {
        let source = include_str!("range_calendar.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("pressed_with_background(cell, press_box, accent.hover(), cx)"),
            "a pressed range cap must fill `bg-accent-hover` \
             (pinned `[data-pressed] [data-selection-start|end]`)"
        );
        assert!(
            source.contains("} else if is_today || in_range {"),
            "a pressed middle or today cell must keep its `bg-accent-soft` \
             fill (the pinned pressed state recolours only the caps)"
        );
    }

    #[test]
    fn the_year_picker_trigger_hovers_nothing() {
        // Scan the implementation only; this test's own text names the
        // forbidden accessor.
        let source = include_str!("range_calendar.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            !source.contains("colors.default.soft_hover()"),
            "the year-picker trigger must not invent a hover background"
        );
    }

    #[test]
    fn the_year_picker_indicator_rotates_one_down_chevron() {
        let source = include_str!("range_calendar.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert_eq!(
            source
                .matches("crate::anim::rotating_indicator_with_angle_ease_out(")
                .count(),
            2,
            "both heading render paths must share the animated indicator"
        );
        assert_eq!(
            source
                .matches("crate::anim::YEAR_PICKER_INDICATOR_ANGLE")
                .count(),
            2,
            "the year picker must use the pinned 90-degree angle"
        );
        assert_eq!(
            source.matches(".path(icons::CHEVRON_DOWN)").count(),
            2,
            "open and closed states must reuse one down-chevron asset"
        );
        assert!(
            !source.contains("icons::CHEVRON_UP"),
            "the year picker must rotate one asset instead of swapping glyphs"
        );
    }

    #[test]
    fn the_nav_button_fades_its_hover_fill_without_replacing_press_scale() {
        let source = include_str!("range_calendar.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("crate::anim::hover_fade_with_duration(")
                && source.contains("Some(100)"),
            "the range navigation button must use HeroUI's 100ms hover fill"
        );
        assert!(
            source.contains("crate::anim::pressed(pressed, press_box, cx)"),
            "the navigation button must retain the separate 0.95 press skin"
        );
    }

    // Pinned anatomy: the selected track fill lives on the outer cell and
    // recolours nothing, so the interior `.range-calendar__cell-button`
    // keeps the base `text-foreground`; only `data-today` recolours its text,
    // today or inside the range alike.
    #[test]
    fn the_interior_text_stays_base_foreground_unless_today() {
        let source = include_str!("range_calendar.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("track = track.bg(accent.soft());"),
            "the range track fill must not set a text colour (pinned \
             `.range-calendar__cell-button` keeps `text-foreground`)"
        );
        let today = source
            .find("} else if is_today {")
            .expect("the today branch exists");
        let interior = source
            .find("} else if in_range {")
            .expect("the interior branch exists");
        assert!(
            today < interior,
            "the today branch must precede the interior branch, or a today \
             cell inside the range would fall through to the base foreground"
        );
    }

    // Pinned row-boundary rounding: a selected cell in the first column
    // rounds its left side `lg`, the last column its right side, and a cap
    // rounds its own side `3xl` on any column.
    #[test]
    fn the_track_rounds_row_boundaries_and_cap_sides() {
        let source = include_str!("range_calendar.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        for snippet in [
            "let cap_radius = util::control_radius(cx);",
            "let edge_radius = util::key_radius(cx);",
            "} else if slot.is_first() {",
            "} else if slot.is_last() {",
            ".rounded_tl(left)",
            ".rounded_bl(left)",
            ".rounded_tr(right)",
            ".rounded_br(right)",
        ] {
            assert!(
                source.contains(snippet),
                "the track rounding must implement the pinned row-boundary \
                 and cap variants; missing {snippet:?}"
            );
        }
    }
}

#[cfg(test)]
mod calendar_system_tests {
    use super::*;

    #[test]
    fn unavailable_scan_uses_the_selected_calendar_month() {
        let system =
            crate::calendar_system::CalendarSystem::for_locale("hi-IN-u-ca-indian").unwrap();
        let constraints = DateConstraints {
            is_date_unavailable: Some(Arc::new(|date| date == Date::new(2026, 2, 21))),
            ..Default::default()
        };
        let effective = effective_range_constraints(
            &system,
            Some(Date::new(2026, 1, 20)),
            VisibleDuration::Months(1),
            &constraints,
            None,
            false,
        );
        assert_eq!(effective.max_value, None);
    }
}

crate::util::impl_component_styled!(RangeCalendar);
