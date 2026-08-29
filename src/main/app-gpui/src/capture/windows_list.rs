//! Port of `capture/window-selector/index.ts` — the daemon's window list, used
//! by the overlay's window-pick mode.

use serde::Deserialize;

use crate::daemon::DaemonHandle;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
pub struct Rect {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
}

impl Rect {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowListItem {
    pub window_id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub owner_name: String,
    #[serde(default)]
    pub owner_pid: i64,
    #[serde(default)]
    pub bounds: Rect,
}

impl WindowListItem {
    pub fn label(&self) -> String {
        if self.title.trim().is_empty() {
            self.owner_name.clone()
        } else {
            format!("{} \u{2014} {}", self.owner_name, self.title)
        }
    }
}

pub fn list(daemon: &DaemonHandle) -> Vec<WindowListItem> {
    if !daemon.is_running() && daemon.start().is_err() {
        return Vec::new();
    }
    let response = match daemon.call("window-selector", "list", None) {
        Ok(response) => response,
        Err(error) => {
            eprintln!("[window-selector] list failed: {error}");
            return Vec::new();
        }
    };
    serde_json::from_value::<Vec<WindowListItem>>(
        response.get("windows").cloned().unwrap_or_default(),
    )
    .unwrap_or_default()
}

pub fn hit_test(windows: &[WindowListItem], x: f64, y: f64) -> Option<&WindowListItem> {
    windows.iter().find(|window| window.bounds.contains(x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: i64, x: f64, y: f64, width: f64, height: f64) -> WindowListItem {
        WindowListItem {
            window_id: id,
            title: format!("Window {id}"),
            owner_name: "App".into(),
            owner_pid: 1,
            bounds: Rect {
                x,
                y,
                width,
                height,
            },
        }
    }

    #[test]
    fn picks_the_frontmost_window_under_the_cursor() {
        let windows = vec![
            window(1, 0.0, 0.0, 800.0, 600.0),
            window(2, 100.0, 100.0, 200.0, 200.0),
        ];
        assert_eq!(
            hit_test(&windows, 150.0, 150.0).map(|w| w.window_id),
            Some(1)
        );
        assert_eq!(
            hit_test(&windows, 700.0, 500.0).map(|w| w.window_id),
            Some(1)
        );
        assert!(hit_test(&windows, 900.0, 900.0).is_none());
    }

    #[test]
    fn labels_fall_back_to_the_owner_name() {
        let mut item = window(1, 0.0, 0.0, 10.0, 10.0);
        assert_eq!(item.label(), "App \u{2014} Window 1");
        item.title = "  ".into();
        assert_eq!(item.label(), "App");
    }

    #[test]
    fn parses_the_daemon_payload() {
        let payload = serde_json::json!([{
            "windowId": 42,
            "title": "Docs",
            "ownerName": "Browser",
            "ownerPid": 7,
            "bounds": { "x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0 }
        }]);
        let parsed: Vec<WindowListItem> = serde_json::from_value(payload).expect("parse");
        assert_eq!(parsed[0].window_id, 42);
        assert_eq!(parsed[0].bounds.width, 3.0);
    }
}
