//! The v3 calendar view model: `visibleDuration`, `pageBehavior` and
//! `selectionAlignment`.
//!
//! v3 lets a calendar show a month grid, several month grids side by side, a
//! run of week rows, or a rolling window of days. All three views share one
//! anchor date. Date calculations are pure; the shared year-grid render state
//! also lives here.

use herogpui_core::element_id;

use crate::calendar::{add_days, bump_month, days_from_civil, days_in_month, Date};
use crate::date_constraints::Weekday;

/// `visibleDuration` — how much time one calendar shows at once.
///
/// React spells this `{months?: n, weeks?: n, days?: n}`; an enum makes the
/// three mutually exclusive forms unrepresentable in combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisibleDuration {
    /// `{months: n}` — n month grids side by side.
    Months(usize),
    /// `{weeks: n}` — one grid of n week rows.
    Weeks(usize),
    /// `{days: n}` — a rolling window of n consecutive days.
    /// Its displayed week rows may also contain disabled leading dates.
    Days(usize),
}

impl Default for VisibleDuration {
    fn default() -> Self {
        VisibleDuration::Months(1)
    }
}

impl VisibleDuration {
    /// The unit count, never zero — a calendar always shows something.
    pub fn count(self) -> usize {
        match self {
            VisibleDuration::Months(n) | VisibleDuration::Weeks(n) | VisibleDuration::Days(n) => {
                n.max(1)
            }
        }
    }

    /// Month view keeps the classic 7-column grid with lead/trail cells; the
    /// week and day views are a flat run of dates instead.
    pub fn is_month_view(self) -> bool {
        matches!(self, VisibleDuration::Months(_))
    }
}

/// `pageBehavior` — whether navigation advances the whole visible range or a
/// single unit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PageBehavior {
    /// `'visible'` — step by the visible duration.
    #[default]
    Visible,
    /// `'single'` — step one month/week/day at a time.
    Single,
}

/// `selectionAlignment` — where the selection sits inside the visible range on
/// first render.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionAlignment {
    /// The selection sits at the start of the visible range.
    Start,
    #[default]
    /// The selection sits at the centre of the visible range.
    Center,
    /// The selection sits at the end of the visible range.
    End,
}

impl SelectionAlignment {
    /// How many units before the selection the range starts, for a range of
    /// `count` units.
    pub fn lead_units(self, count: usize) -> usize {
        let count = count.max(1);
        match self {
            SelectionAlignment::Start => 0,
            // A 2-up range centres by leading with one unit, matching v3.
            SelectionAlignment::Center => (count - 1) / 2,
            SelectionAlignment::End => count - 1,
        }
    }
}

/// The first day of the week containing `date`, honouring `firstDayOfWeek`.
pub fn week_start(date: Date, first_day: Weekday) -> Date {
    // `first_weekday_pub` is Monday-indexed, so shift into the caller's frame
    // before taking the remainder.
    let dow = crate::calendar::weekday_index(date);
    let offset = (dow + 7 - first_day.monday_index()) % 7;
    add_days(&date, -(offset as i64))
}

/// Clamps `day` into `(year, month)` — paging from the 31st into a short month
/// must not produce an invalid date.
fn clamp_day(year: i32, month: u32, day: u32) -> Date {
    Date::new(year, month, day.min(days_in_month(year, month)))
}

/// The anchor after paging by `dir` (-1 back, +1 forward).
pub fn page(duration: VisibleDuration, behavior: PageBehavior, anchor: Date, dir: i32) -> Date {
    page_in(
        crate::calendar_system::system(),
        duration,
        behavior,
        anchor,
        dir,
    )
}

/// The anchor one page away, stepping months in `system`'s own calendar.
///
/// A month view pages by a month the *reader* would recognise, so an Indian
/// grid moves Pausha to Magha rather than by a Gregorian month that would drift
/// against it. The anchor stays Gregorian either way.
pub fn page_in(
    system: &crate::calendar_system::CalendarSystem,
    duration: VisibleDuration,
    behavior: PageBehavior,
    anchor: Date,
    dir: i32,
) -> Date {
    let step = match behavior {
        PageBehavior::Visible => duration.count(),
        PageBehavior::Single => 1,
    };
    match duration {
        VisibleDuration::Months(_) => {
            if system.is_gregorian() {
                let (mut y, mut m) = (anchor.year, anchor.month);
                for _ in 0..step {
                    let (ny, nm) = bump_month(y, m, dir);
                    y = ny;
                    m = nm;
                }
                return clamp_day(y, m, anchor.day);
            }
            let (year, month, day) = system.from_gregorian(anchor);
            let (year, month) = system.add_months(year, month, dir.signum() * step as i32);
            // The same day of the month where that month has one, else its
            // last -- the non-Gregorian half of `clamp_day`.
            let day = day.min(system.days_in_month(year, month)).max(1);
            system.to_gregorian(year, month, day).unwrap_or(anchor)
        }
        VisibleDuration::Weeks(_) => add_days(&anchor, dir as i64 * step as i64 * 7),
        VisibleDuration::Days(_) => add_days(&anchor, dir as i64 * step as i64),
    }
}

/// The focused section reached by PageUp/PageDown in pinned React Aria.
///
/// Month and week views move one displayed unit regardless of `pageBehavior`;
/// Shift moves their next larger unit. Day views delegate to page navigation,
/// so they honor `pageBehavior` and ignore Shift.
pub fn focus_section(
    duration: VisibleDuration,
    behavior: PageBehavior,
    anchor: Date,
    dir: i32,
    larger: bool,
) -> Date {
    focus_section_in(
        crate::calendar_system::system(),
        duration,
        behavior,
        anchor,
        dir,
        larger,
    )
}

/// Uses the explicitly selected calendar system.
pub(crate) fn focus_section_in(
    system: &crate::calendar_system::CalendarSystem,
    duration: VisibleDuration,
    behavior: PageBehavior,
    anchor: Date,
    dir: i32,
    larger: bool,
) -> Date {
    match (duration, larger) {
        (VisibleDuration::Days(_), _) => page_in(system, duration, behavior, anchor, dir),
        (VisibleDuration::Weeks(_), false) => page_in(
            system,
            VisibleDuration::Weeks(1),
            PageBehavior::Single,
            anchor,
            dir,
        ),
        (VisibleDuration::Weeks(_), true) => page_in(
            system,
            VisibleDuration::Months(1),
            PageBehavior::Single,
            anchor,
            dir,
        ),
        (VisibleDuration::Months(_), false) => page_in(
            system,
            VisibleDuration::Months(1),
            PageBehavior::Single,
            anchor,
            dir,
        ),
        (VisibleDuration::Months(_), true) => system.add_years(anchor, dir),
    }
}

/// The date reached by Home in pinned React Stately's calendar grid.
pub fn section_start(duration: VisibleDuration, visible_start: Date, focused: Date) -> Date {
    section_start_in(
        crate::calendar_system::system(),
        duration,
        visible_start,
        focused,
    )
}

/// Uses the explicitly selected calendar system.
pub(crate) fn section_start_in(
    system: &crate::calendar_system::CalendarSystem,
    duration: VisibleDuration,
    visible_start: Date,
    focused: Date,
) -> Date {
    match duration {
        VisibleDuration::Days(_) => visible_start,
        // React Stately deliberately uses the locale week here rather than
        // the grid's firstDayOfWeek override.
        VisibleDuration::Weeks(_) => week_start(focused, Weekday::default()),
        VisibleDuration::Months(_) => {
            let (year, month, _) = system.from_gregorian(focused);
            system.to_gregorian(year, month, 1).unwrap_or(focused)
        }
    }
}

/// The date reached by End in pinned React Stately's calendar grid.
pub fn section_end(duration: VisibleDuration, visible_end: Date, focused: Date) -> Date {
    section_end_in(
        crate::calendar_system::system(),
        duration,
        visible_end,
        focused,
    )
}

/// Uses the explicitly selected calendar system.
pub(crate) fn section_end_in(
    system: &crate::calendar_system::CalendarSystem,
    duration: VisibleDuration,
    visible_end: Date,
    focused: Date,
) -> Date {
    match duration {
        VisibleDuration::Days(_) => visible_end,
        VisibleDuration::Weeks(_) => add_days(&week_start(focused, Weekday::default()), 6),
        VisibleDuration::Months(_) => {
            let (year, month, _) = system.from_gregorian(focused);
            system
                .to_gregorian(year, month, system.days_in_month(year, month))
                .unwrap_or(focused)
        }
    }
}

/// Realign a visible window after keyboard focus crosses either edge.
pub fn anchor_following_focus(
    duration: VisibleDuration,
    first_day: Weekday,
    anchor: Date,
    visible_start: Date,
    visible_end: Date,
    focused: Date,
) -> Date {
    anchor_following_focus_in(
        crate::calendar_system::system(),
        duration,
        first_day,
        anchor,
        visible_start,
        visible_end,
        focused,
    )
}

/// Uses the explicitly selected calendar system.
pub(crate) fn anchor_following_focus_in(
    system: &crate::calendar_system::CalendarSystem,
    duration: VisibleDuration,
    first_day: Weekday,
    anchor: Date,
    visible_start: Date,
    visible_end: Date,
    focused: Date,
) -> Date {
    if days_from_civil(&focused) < days_from_civil(&visible_start) {
        aligned_anchor_in(
            system,
            duration,
            SelectionAlignment::End,
            first_day,
            focused,
        )
    } else if days_from_civil(&focused) > days_from_civil(&visible_end) {
        aligned_anchor_in(
            system,
            duration,
            SelectionAlignment::Start,
            first_day,
            focused,
        )
    } else {
        anchor
    }
}

/// The anchor that puts `selection` at the requested position in the range.
///
/// v3 applies this on initial render only, so callers seed state with it
/// rather than recomputing every frame.
pub fn aligned_anchor(
    duration: VisibleDuration,
    alignment: SelectionAlignment,
    first_day: Weekday,
    selection: Date,
) -> Date {
    aligned_anchor_in(
        crate::calendar_system::system(),
        duration,
        alignment,
        first_day,
        selection,
    )
}

/// Uses the explicitly selected calendar system.
pub(crate) fn aligned_anchor_in(
    system: &crate::calendar_system::CalendarSystem,
    duration: VisibleDuration,
    alignment: SelectionAlignment,
    first_day: Weekday,
    selection: Date,
) -> Date {
    let lead = alignment.lead_units(duration.count());
    match duration {
        VisibleDuration::Months(_) => {
            let (year, month, day) = system.from_gregorian(selection);
            let (year, month) = system.add_months(year, month, -(lead as i32));
            system
                .to_gregorian(year, month, day.min(system.days_in_month(year, month)))
                .unwrap_or(selection)
        }
        VisibleDuration::Weeks(_) => {
            add_days(&week_start(selection, first_day), -(lead as i64) * 7)
        }
        VisibleDuration::Days(_) => add_days(&selection, -(lead as i64)),
    }
}

/// The `(year, month)` heading of each month grid in a month view.
pub fn month_headings(duration: VisibleDuration, anchor: Date) -> Vec<(i32, u32)> {
    month_headings_in(crate::calendar_system::system(), duration, anchor)
}

/// The visible months, in `system`'s own year and month numbering.
///
/// The anchor stays Gregorian -- it is the caller's state -- and is converted
/// here, so a grid drawn in another calendar never leaks that system into
/// `CalendarState`.
pub fn month_headings_in(
    system: &crate::calendar_system::CalendarSystem,
    duration: VisibleDuration,
    anchor: Date,
) -> Vec<(i32, u32)> {
    if !duration.is_month_view() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(duration.count());
    let (mut y, mut m, _) = system.from_gregorian(anchor);
    for _ in 0..duration.count() {
        out.push((y, m));
        let (ny, nm) = system.add_months(y, m, 1);
        y = ny;
        m = nm;
    }
    out
}

/// The consecutive dates in a week or day view's logical range; empty for a month view,
/// which builds its cells from lead offsets instead.
pub fn linear_cells(duration: VisibleDuration, first_day: Weekday, anchor: Date) -> Vec<Date> {
    let n = duration.count();
    match duration {
        VisibleDuration::Months(_) => Vec::new(),
        VisibleDuration::Weeks(_) => {
            let start = week_start(anchor, first_day);
            (0..n as i64 * 7).map(|i| add_days(&start, i)).collect()
        }
        VisibleDuration::Days(_) => (0..n as i64).map(|i| add_days(&anchor, i)).collect(),
    }
}

/// Day views retain the whole weekday grid. Leading dates are rendered disabled
/// by the caller; trailing slots after the visible end stay empty.
pub(crate) fn week_aligned_rows(
    start: Date,
    end: Date,
    first_day: Weekday,
) -> Vec<[Option<Date>; 7]> {
    let first = week_start(start, first_day);
    let rows = (days_from_civil(&end) - days_from_civil(&first)) / 7 + 1;
    (0..rows)
        .map(|row| {
            std::array::from_fn(|column| {
                let date = add_days(&first, row * 7 + column as i64);
                (days_from_civil(&date) <= days_from_civil(&end)).then_some(date)
            })
        })
        .collect()
}

/// The first and last dates in the visible range, excluding month-grid spill
/// cells. React Stately tests the immediately adjacent days to decide whether
/// the previous and next controls are disabled.
pub(crate) fn visible_range(
    system: &crate::calendar_system::CalendarSystem,
    duration: VisibleDuration,
    first_day: Weekday,
    anchor: Date,
) -> (Date, Date) {
    match duration {
        VisibleDuration::Months(_) => {
            let months = month_headings_in(system, duration, anchor);
            let (start_year, start_month) = months[0];
            let (end_year, end_month) = months[months.len() - 1];
            (
                system
                    .to_gregorian(start_year, start_month, 1)
                    .unwrap_or(anchor),
                system
                    .to_gregorian(
                        end_year,
                        end_month,
                        system.days_in_month(end_year, end_month),
                    )
                    .unwrap_or(anchor),
            )
        }
        VisibleDuration::Weeks(_) | VisibleDuration::Days(_) => {
            let cells = linear_cells(duration, first_day, anchor);
            (cells[0], cells[cells.len() - 1])
        }
    }
}

/// The heading for a week or day view, e.g. `Aug 3 – Aug 9, 2026`.
pub fn range_heading(cells: &[Date]) -> String {
    match (cells.first(), cells.last()) {
        (Some(a), Some(b)) if a == b => {
            format!(
                "{} {}, {}",
                crate::calendar::month_abbr(a.month),
                a.day,
                a.year
            )
        }
        (Some(a), Some(b)) if a.year == b.year => format!(
            "{} {} \u{2013} {} {}, {}",
            crate::calendar::month_abbr(a.month),
            a.day,
            crate::calendar::month_abbr(b.month),
            b.day,
            b.year
        ),
        (Some(a), Some(b)) => format!(
            "{} {}, {} \u{2013} {} {}, {}",
            crate::calendar::month_abbr(a.month),
            a.day,
            a.year,
            crate::calendar::month_abbr(b.month),
            b.day,
            b.year
        ),
        _ => String::new(),
    }
}

/// The sliding year-picker window around `view_year`.
///
/// HeroUI defaults to 20 visible years, except when both date bounds are set:
/// then it shows their full inclusive year span. An explicit `visibleYears`
/// wins, and the window stays inside either bound.
pub(crate) fn year_window(
    system: &crate::calendar_system::CalendarSystem,
    view_year: i32,
    visible_years: Option<usize>,
    min_value: Option<Date>,
    max_value: Option<Date>,
) -> Vec<i32> {
    let min_year = min_value.map(|date| system.from_gregorian(date).0);
    let max_year = max_value.map(|date| system.from_gregorian(date).0);
    let available = min_year.zip(max_year).map(|(min, max)| {
        let span = i64::from(max) - i64::from(min) + 1;
        usize::try_from(span.max(1)).unwrap_or(usize::MAX)
    });
    let requested = visible_years.or(available).unwrap_or(20).max(1);
    let count = requested.min(available.unwrap_or(requested));
    let count_i64 = i64::try_from(count).unwrap_or(i64::MAX);
    let mut start = i64::from(view_year) - count_i64 / 2;

    if let Some(min) = min_year {
        start = start.max(i64::from(min));
    }
    if let Some(max) = max_year {
        start = start.min(i64::from(max) - count_i64 + 1);
    }
    if let Some(min) = min_year {
        start = start.max(i64::from(min));
    }

    (0..count)
        .filter_map(|offset| {
            let year = start + i64::try_from(offset).ok()?;
            i32::try_from(year).ok()
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MonthScrollFocus {
    Day(Date),
    Year(i32),
    Heading(usize),
}

#[derive(Default)]
struct MonthGridScroll {
    handle: gpui::ScrollHandle,
    last_target:
        std::cell::Cell<Option<(MonthScrollFocus, gpui::Pixels, gpui::Pixels, gpui::Pixels)>>,
}

pub(crate) fn scrolling_months(
    content: gpui::AnyElement,
    base: &gpui::ElementId,
    reveal: Option<(MonthScrollFocus, gpui::Pixels, gpui::Pixels)>,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) -> gpui::AnyElement {
    use gpui::{prelude::*, InteractiveElement, StatefulInteractiveElement};
    let state = window.use_keyed_state(
        element_id::scoped(base, "months-scroll-state"),
        cx,
        |_, _| std::rc::Rc::new(MonthGridScroll::default()),
    );
    let scroll = state.read(cx).clone();
    let handle = scroll.handle.clone();
    gpui::div()
        .on_children_prepainted(move |_, window, cx| {
            let width = scroll.handle.bounds().size.width;
            let target = reveal.map(|(focus, left, right)| (focus, left, right, width));
            if scroll.last_target.replace(target) == target {
                return;
            }
            if let Some((_, left, right, _)) = target {
                let offset = scroll.handle.offset();
                let x = if left < -offset.x {
                    -left
                } else if right > width - offset.x {
                    width - right
                } else {
                    offset.x
                };
                if x != offset.x {
                    scroll.handle.set_offset(gpui::point(x, offset.y));
                    window.defer(cx, |window, _| window.refresh());
                }
            }
        })
        .id(element_id::scoped(base, "months-scroll"))
        .w_full()
        .min_w_0()
        .overflow_x_scroll()
        .track_scroll(&handle)
        .flex()
        .child(content)
        .into_any_element()
}

#[derive(Default)]
pub(crate) struct YearGridScroll {
    pub(crate) handle: gpui::ScrollHandle,
    pub(crate) last_target: std::cell::Cell<Option<(i32, usize)>>,
}

/// The shared render inputs for Calendar and RangeCalendar's year grid.
pub(crate) struct YearGridView<'a> {
    pub(crate) years: &'a [i32],
    pub(crate) active_year: i32,
    pub(crate) base: &'a gpui::ElementId,
    pub(crate) scroll: &'a std::rc::Rc<YearGridScroll>,
    pub(crate) reveal_row: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_rows_keep_leading_dates_and_pad_the_visible_end() {
        let rows = week_aligned_rows(d(2026, 9, 2), d(2026, 9, 4), Weekday::Mon);
        assert_eq!(
            rows,
            vec![[
                Some(d(2026, 8, 31)),
                Some(d(2026, 9, 1)),
                Some(d(2026, 9, 2)),
                Some(d(2026, 9, 3)),
                Some(d(2026, 9, 4)),
                None,
                None
            ]]
        );
        let rows = week_aligned_rows(d(2026, 9, 2), d(2026, 9, 9), Weekday::Sun);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], Some(d(2026, 8, 30)));
        assert_eq!(
            rows[1],
            [
                Some(d(2026, 9, 6)),
                Some(d(2026, 9, 7)),
                Some(d(2026, 9, 8)),
                Some(d(2026, 9, 9)),
                None,
                None,
                None
            ]
        );
    }

    #[test]
    fn explicit_calendar_bounds_and_year_pages_keep_gregorian_values() {
        let indian =
            crate::calendar_system::CalendarSystem::for_locale("hi-IN-u-ca-indian").unwrap();
        assert_eq!(
            visible_range(
                &indian,
                VisibleDuration::Months(1),
                Weekday::Mon,
                d(2026, 1, 15)
            ),
            (d(2025, 12, 22), d(2026, 1, 20))
        );
        assert_eq!(
            year_window(
                &indian,
                1947,
                None,
                Some(d(2025, 3, 22)),
                Some(d(2027, 3, 21))
            ),
            vec![1947, 1948]
        );
        let hebrew =
            crate::calendar_system::CalendarSystem::for_locale("en-US-u-ca-hebrew").unwrap();
        assert_eq!(
            focus_section_in(
                &hebrew,
                VisibleDuration::Months(1),
                PageBehavior::Single,
                d(2024, 3, 25),
                1,
                true
            ),
            d(2025, 3, 15)
        );
    }

    fn d(y: i32, m: u32, day: u32) -> Date {
        Date::new(y, m, day)
    }

    #[test]
    fn visible_duration_never_zero() {
        assert_eq!(VisibleDuration::Months(0).count(), 1);
        assert_eq!(VisibleDuration::Weeks(0).count(), 1);
    }

    #[test]
    fn month_view_pages_by_visible_range() {
        let two = VisibleDuration::Months(2);
        // 'visible' advances the whole range; 'single' moves one month.
        assert_eq!(
            page(two, PageBehavior::Visible, d(2026, 8, 1), 1),
            d(2026, 10, 1)
        );
        assert_eq!(
            page(two, PageBehavior::Single, d(2026, 8, 1), 1),
            d(2026, 9, 1)
        );
    }

    #[test]
    fn month_paging_clamps_into_short_months() {
        // Jan 31 -> Feb must not yield Feb 31.
        let one = VisibleDuration::Months(1);
        assert_eq!(
            page(one, PageBehavior::Visible, d(2026, 1, 31), 1),
            d(2026, 2, 28)
        );
        assert_eq!(
            page(one, PageBehavior::Visible, d(2024, 1, 31), 1),
            d(2024, 2, 29)
        );
    }

    #[test]
    fn month_paging_crosses_the_year() {
        let one = VisibleDuration::Months(1);
        assert_eq!(
            page(one, PageBehavior::Visible, d(2026, 12, 5), 1),
            d(2027, 1, 5)
        );
        assert_eq!(
            page(one, PageBehavior::Visible, d(2026, 1, 5), -1),
            d(2025, 12, 5)
        );
    }

    #[test]
    fn week_and_day_views_page_in_days() {
        let w = VisibleDuration::Weeks(2);
        assert_eq!(
            page(w, PageBehavior::Visible, d(2026, 8, 3), 1),
            d(2026, 8, 17)
        );
        assert_eq!(
            page(w, PageBehavior::Single, d(2026, 8, 3), 1),
            d(2026, 8, 10)
        );
        let day = VisibleDuration::Days(3);
        assert_eq!(
            page(day, PageBehavior::Visible, d(2026, 8, 3), 1),
            d(2026, 8, 6)
        );
        assert_eq!(
            page(day, PageBehavior::Single, d(2026, 8, 3), -1),
            d(2026, 8, 2)
        );
    }

    #[test]
    fn focused_sections_follow_the_displayed_unit() {
        let at = d(2026, 8, 15);
        assert_eq!(
            focus_section(
                VisibleDuration::Weeks(2),
                PageBehavior::Visible,
                at,
                1,
                false
            ),
            d(2026, 8, 22)
        );
        assert_eq!(
            focus_section(
                VisibleDuration::Weeks(2),
                PageBehavior::Visible,
                at,
                1,
                true
            ),
            d(2026, 9, 15)
        );
        assert_eq!(
            focus_section(VisibleDuration::Days(3), PageBehavior::Visible, at, 1, true),
            d(2026, 8, 18)
        );
        assert_eq!(
            focus_section(
                VisibleDuration::Days(3),
                PageBehavior::Single,
                at,
                -1,
                false
            ),
            d(2026, 8, 14)
        );
    }

    #[test]
    fn week_start_honours_first_day_of_week() {
        // 2026-08-22 is a Saturday.
        assert_eq!(week_start(d(2026, 8, 22), Weekday::Mon), d(2026, 8, 17));
        assert_eq!(week_start(d(2026, 8, 22), Weekday::Sun), d(2026, 8, 16));
        assert_eq!(week_start(d(2026, 8, 22), Weekday::Sat), d(2026, 8, 22));
    }

    #[test]
    fn alignment_places_the_selection() {
        let two = VisibleDuration::Months(2);
        // React Aria's alignCenter halves the duration then backs off again on
        // an even count, so a 2-up range leads by nothing and Center == Start.
        assert_eq!(
            aligned_anchor(
                two,
                SelectionAlignment::Center,
                Weekday::Mon,
                d(2026, 8, 10)
            ),
            d(2026, 8, 10)
        );
        assert_eq!(
            aligned_anchor(two, SelectionAlignment::Start, Weekday::Mon, d(2026, 8, 10)),
            d(2026, 8, 10)
        );
        // A 3-up range does centre exactly.
        assert_eq!(
            aligned_anchor(
                VisibleDuration::Months(3),
                SelectionAlignment::Center,
                Weekday::Mon,
                d(2026, 8, 10)
            ),
            d(2026, 7, 10)
        );
        assert_eq!(
            aligned_anchor(two, SelectionAlignment::End, Weekday::Mon, d(2026, 8, 10)),
            d(2026, 7, 10)
        );
        let three = VisibleDuration::Months(3);
        assert_eq!(
            aligned_anchor(three, SelectionAlignment::End, Weekday::Mon, d(2026, 8, 10)),
            d(2026, 6, 10)
        );
    }

    #[test]
    fn paging_a_non_gregorian_grid_moves_one_of_its_own_months() {
        let system =
            crate::calendar_system::CalendarSystem::for_locale("hi-IN-u-ca-indian").unwrap();
        // 15 January 2026 is 25 Pausha 1947. One page forward must land in the
        // *next Indian* month, not one Gregorian month later.
        let anchor = d(2026, 1, 15);
        let next = page_in(
            &system,
            VisibleDuration::Months(1),
            PageBehavior::Single,
            anchor,
            1,
        );
        assert_eq!(system.from_gregorian(next).0, 1947);
        assert_eq!(
            system.from_gregorian(next).1,
            11,
            "paging forward from Pausha must reach Magha"
        );
        // ...and back again returns to the month it came from.
        let back = page_in(
            &system,
            VisibleDuration::Months(1),
            PageBehavior::Single,
            next,
            -1,
        );
        assert_eq!(system.from_gregorian(back).1, 10);
    }

    #[test]
    fn month_headings_walk_forward_from_the_anchor() {
        assert_eq!(
            month_headings(VisibleDuration::Months(3), d(2026, 11, 1)),
            vec![(2026, 11), (2026, 12), (2027, 1)]
        );
        assert!(month_headings(VisibleDuration::Weeks(2), d(2026, 8, 1)).is_empty());
    }

    #[test]
    fn linear_cells_are_contiguous() {
        let cells = linear_cells(VisibleDuration::Weeks(2), Weekday::Mon, d(2026, 8, 22));
        assert_eq!(cells.len(), 14);
        assert_eq!(cells[0], d(2026, 8, 17));
        assert_eq!(cells[13], d(2026, 8, 30));
        let days = linear_cells(VisibleDuration::Days(3), Weekday::Mon, d(2026, 8, 30));
        assert_eq!(days, vec![d(2026, 8, 30), d(2026, 8, 31), d(2026, 9, 1)]);
    }

    #[test]
    fn visible_range_excludes_month_spill_and_spans_linear_views() {
        assert_eq!(
            visible_range(
                crate::calendar_system::system(),
                VisibleDuration::Months(2),
                Weekday::Mon,
                d(2026, 8, 15)
            ),
            (d(2026, 8, 1), d(2026, 9, 30))
        );
        assert_eq!(
            visible_range(
                crate::calendar_system::system(),
                VisibleDuration::Weeks(1),
                Weekday::Mon,
                d(2026, 8, 22)
            ),
            (d(2026, 8, 17), d(2026, 8, 23))
        );
        assert_eq!(
            visible_range(
                crate::calendar_system::system(),
                VisibleDuration::Days(3),
                Weekday::Mon,
                d(2026, 8, 30)
            ),
            (d(2026, 8, 30), d(2026, 9, 1))
        );
    }

    #[test]
    fn range_heading_collapses_and_spans() {
        let one = linear_cells(VisibleDuration::Days(1), Weekday::Mon, d(2026, 8, 3));
        assert_eq!(range_heading(&one), "Aug 3, 2026");
        let week = linear_cells(VisibleDuration::Weeks(1), Weekday::Mon, d(2026, 8, 5));
        assert_eq!(range_heading(&week), "Aug 3 \u{2013} Aug 9, 2026");
        let span = linear_cells(VisibleDuration::Days(3), Weekday::Mon, d(2026, 12, 31));
        assert_eq!(span, vec![d(2026, 12, 31), d(2027, 1, 1), d(2027, 1, 2)]);
        assert_eq!(range_heading(&span), "Dec 31, 2026 \u{2013} Jan 2, 2027");
    }

    #[test]
    fn year_window_centers_and_clamps_to_bounds() {
        let years = year_window(crate::calendar_system::system(), 2026, None, None, None);
        assert_eq!(years.len(), 20);
        assert_eq!(years[10], 2026);

        assert_eq!(
            year_window(
                crate::calendar_system::system(),
                2026,
                None,
                Some(d(2024, 6, 1)),
                Some(d(2028, 6, 1))
            ),
            vec![2024, 2025, 2026, 2027, 2028]
        );
        assert_eq!(
            year_window(
                crate::calendar_system::system(),
                2026,
                Some(3),
                Some(d(2024, 6, 1)),
                Some(d(2028, 6, 1))
            ),
            vec![2025, 2026, 2027]
        );
        assert_eq!(
            year_window(
                crate::calendar_system::system(),
                2024,
                Some(3),
                Some(d(2024, 6, 1)),
                None
            ),
            vec![2024, 2025, 2026]
        );
        assert_eq!(
            year_window(
                crate::calendar_system::system(),
                2028,
                Some(3),
                None,
                Some(d(2028, 6, 1))
            ),
            vec![2026, 2027, 2028]
        );
    }

    /// React Aria's `offset={months: n}` adds the duration before formatting.
    /// Each calendar now applies that offset in its own view system, so this
    /// pins the pair the components compose: step, then name the month.
    #[test]
    fn month_heading_offset_crosses_the_year() {
        let system = crate::calendar_system::CalendarSystem::for_locale("en-US").unwrap();
        let heading = |year: i32, month: u32, offset: i32| {
            let (year, month) = system.add_months(year, month, offset);
            crate::calendar::month_heading_for_locale(system.locale(), year, month).unwrap()
        };
        assert_eq!(heading(2026, 8, 0), "August 2026");
        assert_eq!(heading(2026, 12, 2), "February 2027");
        assert_eq!(heading(2026, 1, -2), "November 2025");
    }
}
