//! The calendar system a month grid is drawn in.
//!
//! v3 does not take the calendar as a prop. It reads it from the locale, which
//! `I18nProvider` supplies: `locale="hi-IN-u-ca-indian"` draws the Indian
//! calendar, and its "International Calendars" examples are exactly that. This
//! port has no provider, so it reads the same `-u-ca-` extension from the
//! locale the operating system reports -- the same preference chain the month
//! names and the first weekday already follow.
//!
//! [`Date`] stays proleptic-Gregorian throughout. It is the value a caller
//! selects, submits and reads back, and v3 keeps that ISO too; only the *grid*
//! moves. So every cell converts to a Gregorian [`Date`] the moment it is
//! built, and constraints, selection and callbacks never see another system.
//!
//! When the resolved system is Gregorian -- the default, and every locale that
//! names no calendar -- each function short-circuits to the plain arithmetic in
//! [`crate::calendar`]. That is what keeps the common path free of ICU and its
//! behaviour bit-for-bit unchanged.

use std::sync::OnceLock;

use icu_calendar::{types, AnyCalendar, AnyCalendarKind, Date as IcuDate, Gregorian, Ref};
use icu_locale_core::Locale as IcuLocale;

use crate::calendar::{days_in_month, first_weekday_pub, Date};

/// A resolved calendar system, and the ICU data to compute in it.
#[derive(Clone)]
pub struct CalendarSystem {
    kind: AnyCalendarKind,
    calendar: AnyCalendar,
    /// The tag this system was resolved from, so a heading can be formatted in
    /// the same locale that chose the calendar.
    locale: String,
}

impl CalendarSystem {
    /// The system named by one locale tag, or `None` if the tag is unparseable.
    pub(crate) fn for_locale(tag: &str) -> Option<Self> {
        let locale = tag.parse::<IcuLocale>().ok()?;
        let kind = AnyCalendarKind::try_new((&locale).into()).unwrap_or(AnyCalendarKind::Gregorian);
        Some(Self {
            kind,
            calendar: AnyCalendar::new(kind),
            locale: tag.to_owned(),
        })
    }

    /// The locale tag this system came from.
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// Whether this is the plain Gregorian calendar, where every conversion is
    /// the identity and the arithmetic in [`crate::calendar`] already applies.
    pub fn is_gregorian(&self) -> bool {
        self.kind == AnyCalendarKind::Gregorian
    }

    fn icu(&self, date: Date) -> Option<IcuDate<Ref<'_, AnyCalendar>>> {
        let month = u8::try_from(date.month).ok()?;
        let day = u8::try_from(date.day).ok()?;
        Some(
            IcuDate::try_new_gregorian(date.year, month, day)
                .ok()?
                .to_calendar(Ref(&self.calendar)),
        )
    }

    /// The (year, month, day) one Gregorian date has in this system.
    pub fn from_gregorian(&self, date: Date) -> (i32, u32, u32) {
        if self.is_gregorian() {
            return (date.year, date.month, date.day);
        }
        self.icu(date)
            .map_or((date.year, date.month, date.day), |d| {
                (
                    d.year().extended_year(),
                    u32::from(d.month().ordinal),
                    u32::from(d.day_of_month().0),
                )
            })
    }

    /// The Gregorian date one (year, month, day) in this system names.
    ///
    /// `None` when the triple does not exist -- a 31st in a 30-day month, or a
    /// month this system's year does not have.
    pub fn to_gregorian(&self, year: i32, month: u32, day: u32) -> Option<Date> {
        if self.is_gregorian() {
            return ((1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month))
                .then(|| Date::new(year, month, day));
        }
        let date = self.date_in(year, month, day)?.to_calendar(Gregorian);
        Some(Date::new(
            date.year().extended_year(),
            u32::from(date.month().ordinal),
            u32::from(date.day_of_month().0),
        ))
    }

    /// Days in one month of this system.
    pub fn days_in_month(&self, year: i32, month: u32) -> u32 {
        if self.is_gregorian() {
            return days_in_month(year, month);
        }
        self.date_in(year, month, 1).map_or_else(
            || days_in_month(year, month),
            |d| u32::from(d.days_in_month()),
        )
    }

    /// Months in one year of this system. Lunisolar years gain a leap month.
    pub fn months_in_year(&self, year: i32) -> u32 {
        if self.is_gregorian() {
            return 12;
        }
        self.date_in(year, 1, 1)
            .map_or(12, |d| u32::from(d.months_in_year()))
    }

    /// The weekday the 1st of this month falls on, Monday as 0 -- the same
    /// convention as [`first_weekday_pub`].
    pub fn first_weekday(&self, year: i32, month: u32) -> usize {
        if self.is_gregorian() {
            return first_weekday_pub(year, month);
        }
        self.date_in(year, month, 1).map_or_else(
            || first_weekday_pub(year, month),
            |d| (d.weekday() as usize + 6) % 7,
        )
    }

    /// The (year, month) `delta` months away in this system.
    ///
    /// Years do not all hold twelve months -- a lunisolar year gains a leap
    /// month -- so this steps one month at a time rather than dividing, and
    /// asks the system how long each year it crosses actually is.
    pub fn add_months(&self, year: i32, month: u32, delta: i32) -> (i32, u32) {
        if self.is_gregorian() {
            return crate::calendar::add_months(year, month, delta);
        }
        let (mut year, mut month) = (year, month);
        for _ in 0..delta.abs() {
            if delta > 0 {
                if month >= self.months_in_year(year) {
                    year += 1;
                    month = 1;
                } else {
                    month += 1;
                }
            } else if month <= 1 {
                year -= 1;
                month = self.months_in_year(year);
            } else {
                month -= 1;
            }
        }
        (year, month)
    }

    /// Move a Gregorian date by years in this calendar, preserving its month code.
    pub(crate) fn add_years(&self, date: Date, years: i32) -> Date {
        if self.is_gregorian() {
            let year = date.year + years;
            return Date::new(
                year,
                date.month,
                date.day.min(days_in_month(year, date.month)),
            );
        }
        let Some(mut value) = self.icu(date) else {
            return date;
        };
        if value
            .try_add_with_options(
                types::DateDuration {
                    years: years.unsigned_abs(),
                    is_negative: years < 0,
                    ..Default::default()
                },
                Default::default(),
            )
            .is_err()
        {
            return date;
        }
        let value = value.to_calendar(Gregorian);
        Date::new(
            value.year().extended_year(),
            u32::from(value.month().ordinal),
            u32::from(value.day_of_month().0),
        )
    }

    fn date_in(&self, year: i32, month: u32, day: u32) -> Option<IcuDate<Ref<'_, AnyCalendar>>> {
        let mut fields = types::DateFields::default();
        fields.extended_year = Some(year);
        fields.ordinal_month = Some(u8::try_from(month).ok()?);
        fields.day = Some(u8::try_from(day).ok()?);
        IcuDate::try_from_fields(fields, Default::default(), Ref(&self.calendar)).ok()
    }
}

/// The system the running locale names, resolved once.
///
/// Gregorian unless a locale in the platform's date preference chain carries a
/// `-u-ca-` extension naming another one.
pub fn system() -> &'static CalendarSystem {
    static SYSTEM: OnceLock<CalendarSystem> = OnceLock::new();
    SYSTEM.get_or_init(|| {
        crate::date_constraints::system_locale_tags()
            .iter()
            .find_map(|tag| CalendarSystem::for_locale(tag))
            .unwrap_or_else(|| {
                CalendarSystem::for_locale("en-US").expect("en-US is a valid locale")
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_steps_preserve_calendar_month_codes_and_constrain_leap_days() {
        let gregorian = CalendarSystem::for_locale("en-US").unwrap();
        assert_eq!(
            gregorian.add_years(Date::new(2024, 2, 29), 1),
            Date::new(2025, 2, 28)
        );
        let hebrew = CalendarSystem::for_locale("en-US-u-ca-hebrew").unwrap();
        assert_eq!(
            hebrew.add_years(Date::new(2024, 3, 25), 1),
            Date::new(2025, 3, 15)
        );
        assert_eq!(
            hebrew.add_years(Date::new(2025, 3, 15), -1),
            Date::new(2024, 3, 25)
        );
    }

    #[test]
    fn leap_month_ordinals_round_trip_without_becoming_month_codes() {
        let system = CalendarSystem::for_locale("en-US-u-ca-hebrew").unwrap();
        for date in [
            Date::new(2024, 2, 20),
            Date::new(2024, 3, 25),
            Date::new(2024, 4, 25),
            Date::new(2024, 9, 25),
        ] {
            let (year, month, day) = system.from_gregorian(date);
            assert_eq!(system.to_gregorian(year, month, day), Some(date));
        }
        assert_eq!(system.days_in_month(5784, 6), 30);
        assert_eq!(system.days_in_month(5784, 7), 29);
    }

    fn indian() -> CalendarSystem {
        CalendarSystem::for_locale("hi-IN-u-ca-indian").unwrap()
    }

    #[test]
    fn a_locale_without_an_extension_is_gregorian() {
        assert!(CalendarSystem::for_locale("en-US").unwrap().is_gregorian());
        assert!(CalendarSystem::for_locale("de-DE").unwrap().is_gregorian());
        assert!(!indian().is_gregorian());
        assert!(!CalendarSystem::for_locale("th-TH-u-ca-buddhist")
            .unwrap()
            .is_gregorian());
    }

    #[test]
    fn the_gregorian_path_matches_the_plain_arithmetic() {
        let system = CalendarSystem::for_locale("en-US").unwrap();
        for (year, month) in [(2026, 1), (2024, 2), (2025, 2), (2026, 4), (2026, 12)] {
            assert_eq!(
                system.days_in_month(year, month),
                days_in_month(year, month)
            );
            assert_eq!(
                system.first_weekday(year, month),
                first_weekday_pub(year, month)
            );
            assert_eq!(system.months_in_year(year), 12);
        }
        let date = Date::new(2026, 1, 15);
        assert_eq!(system.from_gregorian(date), (2026, 1, 15));
        assert_eq!(system.to_gregorian(2026, 1, 15), Some(date));
    }

    #[test]
    fn a_non_gregorian_system_reports_its_own_year_and_month() {
        // 15 January 2026 is 25 Pausha 1947 in the Indian national calendar,
        // whose tenth month runs 30 days.
        let system = indian();
        assert_eq!(
            system.from_gregorian(Date::new(2026, 1, 15)),
            (1947, 10, 25)
        );
        assert_eq!(system.days_in_month(1947, 10), 30);
        assert_eq!(system.months_in_year(1947), 12);
    }

    #[test]
    fn conversion_round_trips_through_the_other_system() {
        let system = indian();
        for date in [
            Date::new(2026, 1, 15),
            Date::new(2026, 3, 22),
            Date::new(2024, 2, 29),
            Date::new(1999, 12, 31),
        ] {
            let (year, month, day) = system.from_gregorian(date);
            assert_eq!(
                system.to_gregorian(year, month, day),
                Some(date),
                "{date:?} must survive the trip through the Indian calendar"
            );
        }
    }

    #[test]
    fn an_impossible_day_reports_nothing_rather_than_clamping() {
        let gregorian = CalendarSystem::for_locale("en-US").unwrap();
        assert_eq!(
            gregorian.to_gregorian(2025, 2, 29),
            None,
            "2025 is not a leap year"
        );
        assert_eq!(
            gregorian.to_gregorian(2026, 13, 1),
            None,
            "there is no 13th month"
        );
        assert_eq!(
            gregorian.to_gregorian(2026, 4, 31),
            None,
            "April has 30 days"
        );
        // The Indian calendar's tenth month has 30 days, so the 31st is not a
        // date and must not silently become the 1st of the next one.
        assert_eq!(indian().to_gregorian(1947, 10, 31), None);
    }

    #[test]
    fn stepping_months_wraps_each_system_own_year() {
        let gregorian = CalendarSystem::for_locale("en-US").unwrap();
        assert_eq!(gregorian.add_months(2026, 12, 1), (2027, 1));
        assert_eq!(gregorian.add_months(2026, 1, -1), (2025, 12));
        assert_eq!(gregorian.add_months(2026, 8, 5), (2027, 1));

        // Stepping past the end of an Indian year must land in the next one,
        // and stepping back out of it must land on that year's last month.
        let system = indian();
        let months = system.months_in_year(1947);
        assert_eq!(system.add_months(1947, months, 1), (1948, 1));
        assert_eq!(system.add_months(1948, 1, -1), (1947, months));
    }

    #[test]
    fn stepping_a_month_moves_a_real_day_by_a_real_month() {
        // The step is only meaningful if the month it lands on exists: the 1st
        // of the next month must be a date, and a later one than this month's.
        let system = indian();
        let (year, month) = system.add_months(1947, 10, 1);
        let this = system.to_gregorian(1947, 10, 1).unwrap();
        let next = system.to_gregorian(year, month, 1).unwrap();
        assert!(
            crate::calendar::days_from_civil(&next) > crate::calendar::days_from_civil(&this),
            "{next:?} must follow {this:?}"
        );
    }

    #[test]
    fn the_first_weekday_is_monday_indexed_in_both_systems() {
        // 1 Pausha 1947 and its Gregorian equivalent are the same actual day,
        // so they must report the same weekday whichever system asks.
        let system = indian();
        let first = system.to_gregorian(1947, 10, 1).unwrap();
        assert_eq!(
            system.first_weekday(1947, 10),
            crate::calendar::weekday_index(first)
        );
    }
}
