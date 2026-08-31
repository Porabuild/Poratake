use crate::history_store::{HistoryItem, HistoryItemType};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HistoryFilter {
    All,
    Screenshot,
    Video,
}

impl HistoryFilter {
    pub const ALL: [HistoryFilter; 3] = [Self::All, Self::Screenshot, Self::Video];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Screenshot => "Screenshots",
            Self::Video => "Videos",
        }
    }

    pub fn icon(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Screenshot => Some("camera"),
            Self::Video => Some("video"),
        }
    }

    pub fn empty_label(self) -> &'static str {
        match self {
            Self::Screenshot => "No screenshots found",
            Self::Video => "No videos found",
            Self::All => "No captures found",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Screenshot => "screenshot",
            Self::Video => "video",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "screenshot" => Self::Screenshot,
            "video" => Self::Video,
            _ => Self::All,
        }
    }

    fn matches(self, kind: HistoryItemType) -> bool {
        match self {
            Self::All => true,
            Self::Screenshot => kind == HistoryItemType::Screenshot,
            Self::Video => kind == HistoryItemType::Video,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HistorySortOrder {
    Newest,
    Oldest,
}

impl HistorySortOrder {
    pub fn toggled(self) -> Self {
        match self {
            Self::Newest => Self::Oldest,
            Self::Oldest => Self::Newest,
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Newest => "Newest first",
            Self::Oldest => "Oldest first",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Newest => "newest",
            Self::Oldest => "oldest",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "oldest" => Self::Oldest,
            _ => Self::Newest,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HistoryLayout {
    Grid,
    List,
}

pub const GRID_COLUMNS: usize = 2;

impl HistoryLayout {
    pub fn toggled(self) -> Self {
        match self {
            Self::Grid => Self::List,
            Self::List => Self::Grid,
        }
    }

    pub fn columns(self) -> usize {
        match self {
            Self::Grid => GRID_COLUMNS,
            Self::List => 1,
        }
    }

    pub fn toggle_icon(self) -> &'static str {
        match self {
            Self::Grid => "layout-list",
            Self::List => "layout-grid",
        }
    }

    pub fn toggle_tooltip(self) -> &'static str {
        match self {
            Self::Grid => "Switch to list",
            Self::List => "Switch to grid",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::List => "list",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "list" => Self::List,
            _ => Self::Grid,
        }
    }
}

pub fn visible_items(
    items: &[HistoryItem],
    filter: HistoryFilter,
    order: HistorySortOrder,
) -> Vec<HistoryItem> {
    let mut result: Vec<HistoryItem> = items
        .iter()
        .filter(|item| filter.matches(item.r#type))
        .cloned()
        .collect();
    if order == HistorySortOrder::Oldest {
        result.reverse();
    }
    result
}

pub fn format_relative_time(timestamp_ms: i64, now_ms: i64) -> String {
    let seconds = ((now_ms - timestamp_ms).max(0)) / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        return if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{days} days ago")
        };
    }
    if hours > 0 {
        return if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{hours} hours ago")
        };
    }
    if minutes > 0 {
        return if minutes == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{minutes} minutes ago")
        };
    }
    "Just now".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, kind: HistoryItemType) -> HistoryItem {
        HistoryItem {
            id: id.to_string(),
            timestamp: 0,
            original_path: format!("/tmp/{id}"),
            r#type: kind,
            editor_state: None,
            duration: None,
        }
    }

    #[test]
    fn filters_and_reverses_like_the_renderer() {
        let items = vec![
            item("a", HistoryItemType::Screenshot),
            item("b", HistoryItemType::Video),
            item("c", HistoryItemType::Screenshot),
        ];

        let all = visible_items(&items, HistoryFilter::All, HistorySortOrder::Newest);
        assert_eq!(all.len(), 3);

        let videos = visible_items(&items, HistoryFilter::Video, HistorySortOrder::Newest);
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].id, "b");

        let oldest = visible_items(&items, HistoryFilter::All, HistorySortOrder::Oldest);
        assert_eq!(oldest[0].id, "c");
    }

    #[test]
    fn formats_relative_time_like_the_renderer() {
        let minute = 60_000;
        let hour = 60 * minute;
        let day = 24 * hour;
        assert_eq!(format_relative_time(0, 0), "Just now");
        assert_eq!(format_relative_time(0, 30_000), "Just now");
        assert_eq!(format_relative_time(0, minute), "1 minute ago");
        assert_eq!(format_relative_time(0, 5 * minute), "5 minutes ago");
        assert_eq!(format_relative_time(0, hour), "1 hour ago");
        assert_eq!(format_relative_time(0, 3 * hour), "3 hours ago");
        assert_eq!(format_relative_time(0, day), "1 day ago");
        assert_eq!(format_relative_time(0, 2 * day), "2 days ago");
    }

    #[test]
    fn preference_strings_round_trip() {
        for filter in HistoryFilter::ALL {
            assert_eq!(HistoryFilter::parse(filter.as_str()), filter);
        }
        for order in [HistorySortOrder::Newest, HistorySortOrder::Oldest] {
            assert_eq!(HistorySortOrder::parse(order.as_str()), order);
        }
        for layout in [HistoryLayout::Grid, HistoryLayout::List] {
            assert_eq!(HistoryLayout::parse(layout.as_str()), layout);
        }
    }
}
