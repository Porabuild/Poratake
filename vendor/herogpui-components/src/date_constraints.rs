//! The date constraints v3 puts on every date component.
//!
//! `Calendar`, `RangeCalendar`, `DateField`, `DatePicker` and
//! `DateRangePicker` all accept `minValue`, `maxValue`, `isDateUnavailable`,
//! `firstDayOfWeek` and `weeksInMonth`. Modelling them once keeps the five
//! components from drifting apart.

use std::sync::{Arc, OnceLock};

use icu_calendar::{types::Weekday as IcuWeekday, week::WeekInformation};
use icu_locale_core::Locale as IcuLocale;

use crate::calendar::{days_from_civil, days_in_month, first_weekday_pub, Date};

/// The locale tags the system prefers for dates and times, most preferred
/// first.
///
/// This is the platform's regional-format chain, not its interface language
/// (`crate::system_locale`): a reader can run an English interface and still
/// expect German dates. Callers try each tag in turn because CLDR may know a
/// region the platform reports but not the exact tag spelling.
pub(crate) fn system_locale_tags() -> Vec<String> {
    crate::system_locale::time_locale_tags()
}

/// The seven weekday labels CLDR gives for one locale, Monday first.
pub(crate) fn weekday_labels_for_locale(locale: &str) -> Option<[String; 7]> {
    use icu_datetime::{fieldsets, DateTimeFormatter};

    let locale = locale.parse::<IcuLocale>().ok()?;
    let formatter = DateTimeFormatter::try_new(locale.into(), fieldsets::E::short()).ok()?;
    let ordered = [
        IcuWeekday::Monday,
        IcuWeekday::Tuesday,
        IcuWeekday::Wednesday,
        IcuWeekday::Thursday,
        IcuWeekday::Friday,
        IcuWeekday::Saturday,
        IcuWeekday::Sunday,
    ];
    let labels: Vec<String> = ordered
        .iter()
        .map(|day| formatter.format(day).to_string())
        .collect();
    labels.try_into().ok()
}

/// The running locale's weekday labels, Monday first, resolved once.
fn system_weekday_labels() -> &'static [String; 7] {
    static SYSTEM_WEEKDAY_LABELS: OnceLock<[String; 7]> = OnceLock::new();
    SYSTEM_WEEKDAY_LABELS.get_or_init(|| {
        system_locale_tags()
            .iter()
            .find_map(|tag| weekday_labels_for_locale(tag))
            .unwrap_or_else(|| Weekday::ALL.map(|day| day.fallback_short_label().to_owned()))
    })
}

/// The first column of a month grid (`firstDayOfWeek`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weekday {
    /// Sunday.
    Sun,
    /// Monday.
    Mon,
    /// Tuesday.
    Tue,
    /// Wednesday.
    Wed,
    /// Thursday.
    Thu,
    /// Friday.
    Fri,
    /// Saturday.
    Sat,
}

impl Default for Weekday {
    fn default() -> Self {
        static SYSTEM_FIRST_DAY: OnceLock<Weekday> = OnceLock::new();
        *SYSTEM_FIRST_DAY
            .get_or_init(|| Self::for_preferences(&system_locale_tags()).unwrap_or(Self::Sun))
    }
}

impl Weekday {
    /// In Monday-first order, matching [`first_weekday_pub`].
    pub const ALL: [Weekday; 7] = [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
        Weekday::Sat,
        Weekday::Sun,
    ];

    fn for_locale(locale: &str) -> Option<Self> {
        let locale = locale.parse::<IcuLocale>().ok()?;
        let first = WeekInformation::try_new((&locale).into())
            .ok()?
            .first_weekday;
        Some(match first {
            IcuWeekday::Sunday => Self::Sun,
            IcuWeekday::Monday => Self::Mon,
            IcuWeekday::Tuesday => Self::Tue,
            IcuWeekday::Wednesday => Self::Wed,
            IcuWeekday::Thursday => Self::Thu,
            IcuWeekday::Friday => Self::Fri,
            IcuWeekday::Saturday => Self::Sat,
        })
    }

    fn for_preferences(tags: &[String]) -> Option<Self> {
        tags.iter().find_map(|tag| Self::for_locale(tag))
    }

    /// Index with Monday as 0.
    pub fn monday_index(self) -> usize {
        match self {
            Weekday::Mon => 0,
            Weekday::Tue => 1,
            Weekday::Wed => 2,
            Weekday::Thu => 3,
            Weekday::Fri => 4,
            Weekday::Sat => 5,
            Weekday::Sun => 6,
        }
    }

    /// The English two-letter label, used when CLDR has nothing to say for the
    /// running locale.
    fn fallback_short_label(self) -> &'static str {
        match self {
            Weekday::Sun => "Su",
            Weekday::Mon => "Mo",
            Weekday::Tue => "Tu",
            Weekday::Wed => "We",
            Weekday::Thu => "Th",
            Weekday::Fri => "Fr",
            Weekday::Sat => "Sa",
        }
    }

    /// The running locale's column label for this day.
    ///
    /// v3 heads its grid through `Intl.DateTimeFormat`, so a German reader sees
    /// `Mo Di Mi`, not `Mo Tu We`.
    pub fn short_label(self) -> &'static str {
        &system_weekday_labels()[self.monday_index()]
    }

    /// The seven column headers, starting from this day.
    pub fn header_row(self) -> [&'static str; 7] {
        let start = self.monday_index();
        let mut out = [""; 7];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = Weekday::ALL[(start + i) % 7].short_label();
        }
        out
    }
}

/// Predicate marking individual dates unavailable (`isDateUnavailable`).
pub type DateUnavailable = Arc<dyn Fn(Date) -> bool + 'static>;

/// Shared date constraints.
#[derive(Clone, Default)]
pub struct DateConstraints {
    /// Earliest selectable date (`minValue`).
    pub min_value: Option<Date>,
    /// Latest selectable date (`maxValue`).
    pub max_value: Option<Date>,
    /// Predicate marking dates as unavailable (`isDateUnavailable`).
    pub is_date_unavailable: Option<DateUnavailable>,
    /// First column of the month grid (`firstDayOfWeek`).
    pub first_day_of_week: Weekday,
    /// `weeksInMonth` — forces the grid to this many rows.
    pub weeks_in_month: Option<usize>,
}

impl DateConstraints {
    /// Creates constraints with no bounds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Fill the bounds HeroUI's Calendar and RangeCalendar roots provide when
    /// callers omit them. Explicit sides remain authoritative.
    pub(crate) fn with_hero_calendar_bounds(mut self) -> Self {
        self.min_value.get_or_insert(Date::new(1900, 1, 1));
        self.max_value.get_or_insert(Date::new(2099, 12, 31));
        self
    }

    /// Whether `date` may be selected.
    pub fn allows(&self, date: Date) -> bool {
        if self.out_of_range(date) {
            return false;
        }
        match &self.is_date_unavailable {
            Some(f) => !f(date),
            None => true,
        }
    }

    /// Whether `date` falls outside `[min_value, max_value]`, ignoring the
    /// unavailable predicate. Out-of-range days are muted while unavailable
    /// days are struck through, so callers need to tell them apart.
    pub fn out_of_range(&self, date: Date) -> bool {
        let day = days_from_civil(&date);
        if let Some(min) = self.min_value {
            if day < days_from_civil(&min) {
                return true;
            }
        }
        if let Some(max) = self.max_value {
            if day > days_from_civil(&max) {
                return true;
            }
        }
        false
    }

    /// Clamp a date to the inclusive min/max range. React Stately applies
    /// this to controlled focus values before it derives the visible range.
    pub fn constrain(&self, date: Date) -> Date {
        if let Some(min) = self.min_value {
            if days_from_civil(&date) < days_from_civil(&min) {
                return min;
            }
        }
        if let Some(max) = self.max_value {
            if days_from_civil(&date) > days_from_civil(&max) {
                return max;
            }
        }
        date
    }

    /// Whether `date` is blocked only by the unavailable predicate.
    pub fn is_unavailable(&self, date: Date) -> bool {
        match &self.is_date_unavailable {
            Some(f) => f(date),
            None => false,
        }
    }

    /// Blank leading cells before the 1st, honouring `first_day_of_week`.
    pub fn lead_cells(&self, year: i32, month: u32) -> usize {
        let first = first_weekday_pub(year, month);
        (first + 7 - self.first_day_of_week.monday_index()) % 7
    }

    /// Blank leading cells before the 1st of a month in `system`.
    ///
    /// `(year, month)` are that system's own, not Gregorian ones.
    pub fn lead_cells_in(
        &self,
        system: &crate::calendar_system::CalendarSystem,
        year: i32,
        month: u32,
    ) -> usize {
        let first = system.first_weekday(year, month);
        (first + 7 - self.first_day_of_week.monday_index()) % 7
    }

    /// Rows the grid should render for this month.
    pub fn rows(&self, year: i32, month: u32) -> usize {
        if let Some(rows) = self.weeks_in_month.filter(|rows| *rows > 0) {
            return rows;
        }
        let cells = self.lead_cells(year, month) + days_in_month(year, month) as usize;
        cells.div_ceil(7)
    }

    /// Rows the grid should render for a month in `system`.
    pub fn rows_in(
        &self,
        system: &crate::calendar_system::CalendarSystem,
        year: i32,
        month: u32,
    ) -> usize {
        if let Some(rows) = self.weeks_in_month.filter(|rows| *rows > 0) {
            return rows;
        }
        let cells =
            self.lead_cells_in(system, year, month) + system.days_in_month(year, month) as usize;
        cells.div_ceil(7)
    }
}

impl std::fmt::Debug for DateConstraints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DateConstraints")
            .field("min_value", &self.min_value)
            .field("max_value", &self.max_value)
            .field(
                "is_date_unavailable",
                &self.is_date_unavailable.as_ref().map(|_| "<fn>"),
            )
            .field("first_day_of_week", &self.first_day_of_week)
            .field("weeks_in_month", &self.weeks_in_month)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> Date {
        Date::new(y, m, day)
    }

    #[test]
    fn unconstrained_allows_everything() {
        let c = DateConstraints::new();
        assert!(c.allows(d(1900, 1, 1)));
        assert!(c.allows(d(2099, 12, 31)));
    }

    #[test]
    fn min_and_max_are_inclusive() {
        let c = DateConstraints {
            min_value: Some(d(2026, 3, 10)),
            max_value: Some(d(2026, 3, 20)),
            ..Default::default()
        };
        assert!(!c.allows(d(2026, 3, 9)));
        assert!(c.allows(d(2026, 3, 10)));
        assert!(c.allows(d(2026, 3, 20)));
        assert!(!c.allows(d(2026, 3, 21)));
        assert_eq!(c.constrain(d(2026, 3, 1)), d(2026, 3, 10));
        assert_eq!(c.constrain(d(2026, 3, 15)), d(2026, 3, 15));
        assert_eq!(c.constrain(d(2026, 3, 31)), d(2026, 3, 20));
    }

    #[test]
    fn hero_calendar_bounds_fill_only_missing_sides() {
        let defaults = DateConstraints::new().with_hero_calendar_bounds();
        assert_eq!(defaults.min_value, Some(d(1900, 1, 1)));
        assert_eq!(defaults.max_value, Some(d(2099, 12, 31)));

        let explicit = DateConstraints {
            min_value: Some(d(2020, 2, 3)),
            max_value: Some(d(2030, 4, 5)),
            ..DateConstraints::new()
        }
        .with_hero_calendar_bounds();
        assert_eq!(explicit.min_value, Some(d(2020, 2, 3)));
        assert_eq!(explicit.max_value, Some(d(2030, 4, 5)));

        let minimum_only = DateConstraints {
            min_value: Some(d(2020, 2, 3)),
            ..DateConstraints::new()
        }
        .with_hero_calendar_bounds();
        assert_eq!(minimum_only.min_value, Some(d(2020, 2, 3)));
        assert_eq!(minimum_only.max_value, Some(d(2099, 12, 31)));

        let maximum_only = DateConstraints {
            max_value: Some(d(2030, 4, 5)),
            ..DateConstraints::new()
        }
        .with_hero_calendar_bounds();
        assert_eq!(maximum_only.min_value, Some(d(1900, 1, 1)));
        assert_eq!(maximum_only.max_value, Some(d(2030, 4, 5)));
    }

    #[test]
    fn range_and_unavailable_are_distinguishable() {
        let c = DateConstraints {
            min_value: Some(d(2026, 3, 10)),
            is_date_unavailable: Some(Arc::new(|date: Date| date.day == 15)),
            ..Default::default()
        };
        // Blocked by the predicate, not the range.
        assert!(!c.allows(d(2026, 3, 15)));
        assert!(!c.out_of_range(d(2026, 3, 15)));
        assert!(c.is_unavailable(d(2026, 3, 15)));
        // Blocked by the range, not the predicate.
        assert!(c.out_of_range(d(2026, 3, 1)));
        assert!(!c.is_unavailable(d(2026, 3, 1)));
    }

    #[test]
    fn first_day_of_week_shifts_the_lead() {
        // 2026-03-01 is a Sunday: Monday-start needs six blanks, Sunday none.
        let sunday = DateConstraints {
            first_day_of_week: Weekday::Sun,
            ..Default::default()
        };
        assert_eq!(sunday.lead_cells(2026, 3), 0);

        let monday = DateConstraints {
            first_day_of_week: Weekday::Mon,
            ..Default::default()
        };
        assert_eq!(monday.lead_cells(2026, 3), 6);
    }

    #[test]
    fn first_day_of_week_follows_locale_week_data() {
        assert_eq!(Weekday::for_locale("en-US"), Some(Weekday::Sun));
        assert_eq!(Weekday::for_locale("de-DE"), Some(Weekday::Mon));
        assert_eq!(Weekday::for_locale("en-US-u-fw-wed"), Some(Weekday::Wed));
        assert_eq!(Weekday::for_locale("not_a_locale"), None);
    }

    #[test]
    fn first_day_of_week_prefers_the_system_time_category() {
        // The system chain puts the time category first (LC_TIME, the macOS
        // Region, the Windows regional format); see `system_locale`.
        let tags = ["de-DE".to_owned(), "en-US".to_owned()];
        assert_eq!(Weekday::for_preferences(&tags), Some(Weekday::Mon));
        let unknown_first = ["zz-ZZ-x-bogus".to_owned(), "de-DE".to_owned()];
        assert_eq!(Weekday::for_preferences(&unknown_first), Some(Weekday::Mon));
    }

    #[test]
    fn weekday_labels_follow_the_locale() {
        let english = weekday_labels_for_locale("en-US").unwrap();
        assert_eq!(english[0], "Mon");
        let german = weekday_labels_for_locale("de-DE").unwrap();
        assert_eq!(german[0], "Mo");
        assert_eq!(german[1], "Di", "German Tuesday is Di, not Tu");
        assert!(weekday_labels_for_locale("not a locale").is_none());
    }

    #[test]
    fn header_row_starts_on_the_configured_day() {
        // The labels themselves are the running locale's, so this pins the
        // rotation rather than the spelling: whatever Monday is called, a
        // Monday-first row starts with it and a Sunday-first row ends with it.
        let monday = Weekday::Mon.short_label();
        let sunday = Weekday::Sun.short_label();
        assert_eq!(Weekday::Mon.header_row()[0], monday);
        assert_eq!(Weekday::Mon.header_row()[6], sunday);
        assert_eq!(Weekday::Sun.header_row()[0], sunday);
        assert_eq!(Weekday::Sun.header_row()[1], monday);
        assert_eq!(Weekday::Sat.header_row()[1], sunday);
        assert_eq!(Weekday::Sat.header_row()[2], monday);
        for start in Weekday::ALL {
            let row = start.header_row();
            let unique: std::collections::HashSet<_> = row.iter().collect();
            assert_eq!(unique.len(), 7, "every day appears once in {row:?}");
        }
    }

    #[test]
    fn the_system_aware_grid_matches_the_plain_one_for_gregorian() {
        let gregorian = crate::calendar_system::CalendarSystem::for_locale("en-US").unwrap();
        let c = DateConstraints::new();
        for (y, m) in [(2026, 1), (2026, 2), (2024, 2), (2026, 8), (2027, 5)] {
            assert_eq!(c.lead_cells_in(&gregorian, y, m), c.lead_cells(y, m));
            assert_eq!(c.rows_in(&gregorian, y, m), c.rows(y, m));
        }
    }

    #[test]
    fn a_non_gregorian_grid_covers_its_own_month() {
        let system =
            crate::calendar_system::CalendarSystem::for_locale("hi-IN-u-ca-indian").unwrap();
        let c = DateConstraints::new();
        for month in 1..=12 {
            let cells =
                c.lead_cells_in(&system, 1947, month) + system.days_in_month(1947, month) as usize;
            assert!(
                c.rows_in(&system, 1947, month) * 7 >= cells,
                "month {month} needs {cells} cells"
            );
        }
    }

    #[test]
    fn rows_cover_every_day() {
        let c = DateConstraints::new();
        for (y, m) in [(2026, 2), (2026, 3), (2026, 8), (2024, 2), (2027, 5)] {
            let rows = c.rows(y, m);
            let needed = c.lead_cells(y, m) + days_in_month(y, m) as usize;
            assert!(
                rows * 7 >= needed,
                "{y}-{m}: {rows} rows for {needed} cells"
            );
        }
    }

    #[test]
    fn weeks_in_month_overrides_without_clamping() {
        let forced = DateConstraints {
            weeks_in_month: Some(6),
            ..Default::default()
        };
        assert_eq!(forced.rows(2026, 2), 6);

        let extended = DateConstraints {
            weeks_in_month: Some(7),
            ..Default::default()
        };
        assert_eq!(extended.rows(2026, 2), 7);

        let locale_default = DateConstraints {
            weeks_in_month: Some(0),
            first_day_of_week: Weekday::Mon,
            ..Default::default()
        };
        assert_eq!(locale_default.rows(2026, 2), 5);
    }
}
