use std::time::{SystemTime, UNIX_EPOCH};

pub fn format_system_time(time: SystemTime) -> String {
    let elapsed = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_seconds = elapsed.as_secs() as i64;
    let days = total_seconds.div_euclid(86_400);
    let seconds = total_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_date(days);
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{:03}Z",
        elapsed.subsec_millis()
    )
}

fn civil_date(days_since_epoch: i64) -> (i64, i64, i64) {
    let shifted = days_since_epoch + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn formats_the_unix_epoch() {
        assert_eq!(format_system_time(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn formats_a_leap_day_with_milliseconds() {
        let time = UNIX_EPOCH + Duration::from_millis(1_709_209_845_123);
        assert_eq!(format_system_time(time), "2024-02-29T12:30:45.123Z");
    }

    #[test]
    fn formats_a_date_after_a_century_boundary() {
        let time = UNIX_EPOCH + Duration::from_secs(1_000_000_000);
        assert_eq!(format_system_time(time), "2001-09-09T01:46:40.000Z");
    }
}
