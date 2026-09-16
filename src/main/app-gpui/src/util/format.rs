/// Port of `formatDuration` in the recording control bar: `M:SS`.
pub fn format_elapsed(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Port of `formatTime` in `video-editor/utils.ts`.
pub fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0);
    let minutes = (total / 60.0).floor() as i64;
    let secs = (total % 60.0).floor() as i64;
    format!("{minutes}:{secs:02}")
}

/// Port of `formatDuration` in `video-editor/utils.ts`.
pub fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        return format!("{}s", (seconds * 10.0).round() / 10.0);
    }
    let minutes = (seconds / 60.0).floor() as i64;
    let secs = (seconds % 60.0).round() as i64;
    format!("{minutes}m{secs}s")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_elapsed_time() {
        assert_eq!(format_elapsed(0), "0:00");
        assert_eq!(format_elapsed(9), "0:09");
        assert_eq!(format_elapsed(75), "1:15");
        assert_eq!(format_elapsed(3600), "60:00");
    }

    #[test]
    fn formats_times_like_the_renderer() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(9.4), "0:09");
        assert_eq!(format_time(61.0), "1:01");
        assert_eq!(format_time(600.0), "10:00");
    }

    #[test]
    fn formats_durations_like_the_renderer() {
        assert_eq!(format_duration(3.25), "3.3s");
        assert_eq!(format_duration(90.0), "1m30s");
    }
}
