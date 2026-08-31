//! Port of `capture/window-selector/index.ts` — the daemon's window list, used
//! by the overlay's window-pick mode.

use crate::daemon::DaemonHandle;

#[cfg(test)]
pub(crate) use poratake_daemon_common::geometry::WindowBounds as Rect;
pub use poratake_daemon_common::geometry::WindowInfo as WindowListItem;

pub fn list(daemon: &DaemonHandle) -> Vec<WindowListItem> {
    match daemon.window_selector().list() {
        Ok(windows) => windows,
        Err(error) => {
            eprintln!("[window-selector] list failed: {error}");
            Vec::new()
        }
    }
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
