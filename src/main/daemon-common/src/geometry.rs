use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CaptureRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl CaptureRect {
    pub fn has_positive_size(self) -> bool {
        self.width > 0 && self.height > 0
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DisplayOrigin {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayCaptureContext {
    #[serde(flatten)]
    pub rect: CaptureRect,
    #[serde(default = "default_scale_factor")]
    pub scale_factor: f64,
    #[serde(default)]
    pub display_origin_x: i32,
    #[serde(default)]
    pub display_origin_y: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_id: Option<u32>,
}

impl DisplayCaptureContext {
    pub fn new(
        rect: CaptureRect,
        scale_factor: f64,
        display_origin: DisplayOrigin,
        display_id: Option<u32>,
    ) -> Self {
        Self {
            rect,
            scale_factor,
            display_origin_x: display_origin.x,
            display_origin_y: display_origin.y,
            display_id,
        }
    }

    pub fn display_origin(self) -> DisplayOrigin {
        DisplayOrigin {
            x: self.display_origin_x,
            y: self.display_origin_y,
        }
    }

    pub fn validate(self) -> Result<(), String> {
        if !self.rect.has_positive_size()
            || self.rect.x.checked_add(self.rect.width).is_none()
            || self.rect.y.checked_add(self.rect.height).is_none()
        {
            return Err("capture requires valid x, y, width, height".into());
        }
        if !self.scale_factor.is_finite() || !(0.25..=8.0).contains(&self.scale_factor) {
            return Err("scaleFactor must be between 0.25 and 8".into());
        }
        Ok(())
    }
}

fn default_scale_factor() -> f64 {
    1.0
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub rect: CaptureRect,
    pub primary: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct WindowBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl WindowBounds {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub window_id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub owner_name: String,
    #[serde(default)]
    pub owner_pid: i64,
    #[serde(default)]
    pub bounds: WindowBounds,
}

impl WindowInfo {
    pub fn label(&self) -> String {
        if self.title.trim().is_empty() {
            return self.owner_name.clone();
        }
        format!("{} \u{2014} {}", self.owner_name, self.title)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureAreaRequest {
    #[serde(flatten)]
    pub capture: DisplayCaptureContext,
    pub path: PathBuf,
    #[serde(default)]
    pub cached: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureWindowRequest {
    pub window_id: i64,
    pub path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_area_serializes_the_existing_wire_shape() {
        let request = CaptureAreaRequest {
            capture: DisplayCaptureContext::new(
                CaptureRect {
                    x: 1,
                    y: 2,
                    width: 3,
                    height: 4,
                },
                2.0,
                DisplayOrigin { x: 10, y: 20 },
                Some(7),
            ),
            path: PathBuf::from("capture.png"),
            cached: true,
            window_id: Some(42),
        };

        let value = serde_json::to_value(request).expect("serialize request");

        assert!(value.get("capture").is_none());
        assert_eq!(value["displayOriginX"], 10);
        assert_eq!(value["scaleFactor"], 2.0);
        assert_eq!(value["displayId"], 7);
        assert_eq!(value["windowId"], 42);
        let round_trip: CaptureAreaRequest =
            serde_json::from_value(value).expect("deserialize request");
        assert_eq!(round_trip.window_id, Some(42));
    }

    #[test]
    fn capture_window_round_trips_the_existing_wire_shape() {
        let request = CaptureWindowRequest {
            window_id: 42,
            path: PathBuf::from("capture.png"),
        };
        let value = serde_json::to_value(&request).expect("serialize request");
        let round_trip: CaptureWindowRequest =
            serde_json::from_value(value.clone()).expect("deserialize request");

        assert_eq!(value["windowId"], 42);
        assert_eq!(value["path"], "capture.png");
        assert_eq!(round_trip, request);
    }

    #[test]
    fn display_capture_defaults_preserve_legacy_requests() {
        let capture: DisplayCaptureContext = serde_json::from_value(serde_json::json!({
            "x": -100,
            "y": 20,
            "width": 300,
            "height": 200
        }))
        .expect("legacy display capture");

        assert_eq!(capture.scale_factor, 1.0);
        assert_eq!(capture.display_origin(), DisplayOrigin::default());
        assert_eq!(capture.display_id, None);
    }

    #[test]
    fn display_capture_keeps_mixed_display_values_together() {
        let capture = DisplayCaptureContext::new(
            CaptureRect {
                x: -2_560,
                y: 180,
                width: 1_280,
                height: 720,
            },
            1.5,
            DisplayOrigin { x: -2_560, y: 180 },
            Some(73),
        );

        let round_trip: DisplayCaptureContext = serde_json::from_value(
            serde_json::to_value(capture).expect("serialize mixed display capture"),
        )
        .expect("deserialize mixed display capture");

        assert_eq!(round_trip, capture);
    }

    #[test]
    fn window_info_uses_the_shared_camel_case_wire_shape() {
        let window = WindowInfo {
            window_id: 42,
            title: "Document".into(),
            owner_name: "Editor".into(),
            owner_pid: 7,
            bounds: WindowBounds {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            },
        };

        let value = serde_json::to_value(window).expect("serialize window");

        assert_eq!(value["windowId"], 42);
        assert_eq!(value["ownerName"], "Editor");
    }
}
