//! 1:1 port of `renderer/utils/area-selection.ts` — the geometry behind the
//! area overlay's editable selection: hit-testing the eight handles, the cursor
//! each one shows, resizing and moving under an optional aspect ratio, and
//! clamping everything to the display.

use gpui::CursorStyle;

/// `MIN_SELECTION_SIZE` — a freshly drawn box smaller than this is discarded
/// rather than committed.
pub const MIN_SELECTION_SIZE: f32 = 10.0;
/// `MIN_RESIZE_SIZE` — a resize can never collapse an edge past this.
pub const MIN_RESIZE_SIZE: f32 = 20.0;

const HANDLE_CORNER: f32 = 16.0;
const HANDLE_EDGE: f32 = 12.0;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl Handle {
    fn is_left(self) -> bool {
        matches!(self, Self::TopLeft | Self::Left | Self::BottomLeft)
    }

    fn is_right(self) -> bool {
        matches!(self, Self::TopRight | Self::Right | Self::BottomRight)
    }

    fn is_top(self) -> bool {
        matches!(self, Self::TopLeft | Self::Top | Self::TopRight)
    }

    fn is_bottom(self) -> bool {
        matches!(self, Self::BottomLeft | Self::Bottom | Self::BottomRight)
    }

    /// The `CURSORS` map in `area-selection.ts`.
    pub fn cursor(self) -> CursorStyle {
        match self {
            Self::TopLeft | Self::BottomRight => CursorStyle::ResizeUpLeftDownRight,
            Self::TopRight | Self::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
            Self::Top | Self::Bottom => CursorStyle::ResizeUpDown,
            Self::Left | Self::Right => CursorStyle::ResizeLeftRight,
        }
    }
}

/// What a press starts, mirroring the branch order of `startDrag` in
/// `use-area-selection.ts`: a handle resizes, the inside of the box moves, and
/// anywhere else begins a new box.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Gesture {
    Create,
    Move { offset: Point },
    Resize { handle: Handle },
}

/// The gesture a press at `point` begins against the current selection.
pub fn gesture_at(rect: Option<Rect>, point: Point) -> Gesture {
    let Some(rect) = rect else {
        return Gesture::Create;
    };
    if let Some(handle) = hit_test_handle(rect, point) {
        return Gesture::Resize { handle };
    }
    if contains_point(rect, point) {
        return Gesture::Move {
            offset: Point {
                x: point.x - rect.x,
                y: point.y - rect.y,
            },
        };
    }
    Gesture::Create
}

#[derive(Clone, Copy, Debug)]
struct Edges {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Rect {
    fn edges(self) -> Edges {
        Edges {
            left: self.x,
            top: self.y,
            right: self.x + self.width,
            bottom: self.y + self.height,
        }
    }
}

impl Edges {
    fn rect(self) -> Rect {
        Rect {
            x: self.left,
            y: self.top,
            width: self.right - self.left,
            height: self.bottom - self.top,
        }
    }

    fn contains(self, point: Point) -> bool {
        point.x >= self.left
            && point.x <= self.right
            && point.y >= self.top
            && point.y <= self.bottom
    }
}

pub fn clamp_point(point: Point, bounds: Size) -> Point {
    Point {
        x: point.x.clamp(0.0, bounds.width),
        y: point.y.clamp(0.0, bounds.height),
    }
}

pub fn normalize_rect(first: Point, second: Point) -> Rect {
    Edges {
        left: first.x.min(second.x),
        top: first.y.min(second.y),
        right: first.x.max(second.x),
        bottom: first.y.max(second.y),
    }
    .rect()
}

pub fn contains_point(rect: Rect, point: Point) -> bool {
    rect.edges().contains(point)
}

pub fn is_usable_selection(rect: Rect) -> bool {
    rect.width > MIN_SELECTION_SIZE && rect.height > MIN_SELECTION_SIZE
}

/// The handle under `point`, if any. Corners win over edges because they are
/// tested first, matching the order of the `tests` array in the renderer.
pub fn hit_test_handle(rect: Rect, point: Point) -> Option<Handle> {
    let edges = rect.edges();
    let center_x = (edges.left + edges.right) / 2.0;
    let center_y = (edges.top + edges.bottom) / 2.0;
    let span = HANDLE_CORNER * 2.0;

    let tests: [(Handle, Edges); 8] = [
        (
            Handle::TopLeft,
            Edges {
                left: edges.left - HANDLE_EDGE,
                top: edges.top - HANDLE_EDGE,
                right: edges.left + HANDLE_CORNER,
                bottom: edges.top + HANDLE_CORNER,
            },
        ),
        (
            Handle::TopRight,
            Edges {
                left: edges.right - HANDLE_CORNER,
                top: edges.top - HANDLE_EDGE,
                right: edges.right + HANDLE_EDGE,
                bottom: edges.top + HANDLE_CORNER,
            },
        ),
        (
            Handle::BottomRight,
            Edges {
                left: edges.right - HANDLE_CORNER,
                top: edges.bottom - HANDLE_CORNER,
                right: edges.right + HANDLE_EDGE,
                bottom: edges.bottom + HANDLE_EDGE,
            },
        ),
        (
            Handle::BottomLeft,
            Edges {
                left: edges.left - HANDLE_EDGE,
                top: edges.bottom - HANDLE_CORNER,
                right: edges.left + HANDLE_CORNER,
                bottom: edges.bottom + HANDLE_EDGE,
            },
        ),
        (
            Handle::Top,
            Edges {
                left: center_x - span,
                top: edges.top - HANDLE_EDGE,
                right: center_x + span,
                bottom: edges.top + HANDLE_EDGE,
            },
        ),
        (
            Handle::Right,
            Edges {
                left: edges.right - HANDLE_EDGE,
                top: center_y - span,
                right: edges.right + HANDLE_EDGE,
                bottom: center_y + span,
            },
        ),
        (
            Handle::Bottom,
            Edges {
                left: center_x - span,
                top: edges.bottom - HANDLE_EDGE,
                right: center_x + span,
                bottom: edges.bottom + HANDLE_EDGE,
            },
        ),
        (
            Handle::Left,
            Edges {
                left: edges.left - HANDLE_EDGE,
                top: center_y - span,
                right: edges.left + HANDLE_EDGE,
                bottom: center_y + span,
            },
        ),
    ];

    tests
        .into_iter()
        .find(|(_, box_)| box_.contains(point))
        .map(|(handle, _)| handle)
}

/// `cursorFor`: crosshair with no selection, a resize cursor over a handle,
/// `move` inside the box, crosshair outside it.
pub fn cursor_for(rect: Option<Rect>, point: Point) -> CursorStyle {
    let Some(rect) = rect else {
        return CursorStyle::Crosshair;
    };
    if let Some(handle) = hit_test_handle(rect, point) {
        return handle.cursor();
    }
    if contains_point(rect, point) {
        CursorStyle::ClosedHand
    } else {
        CursorStyle::Crosshair
    }
}

pub fn adjust_rect_to_ratio(rect: Rect, ratio: f32, handle: Option<Handle>) -> Rect {
    if ratio <= 0.0 || rect.width <= 0.0 || rect.height <= 0.0 {
        return rect;
    }
    let edges = rect.edges();

    if rect.width / rect.height > ratio {
        let width = (rect.height * ratio).round();
        return match handle {
            None => {
                let center = (edges.left + edges.right) / 2.0;
                let left = (center - width / 2.0).round();
                Edges {
                    left,
                    right: left + width,
                    ..edges
                }
                .rect()
            }
            Some(handle) if handle.is_left() => Edges {
                left: edges.right - width,
                ..edges
            }
            .rect(),
            Some(_) => Edges {
                right: edges.left + width,
                ..edges
            }
            .rect(),
        };
    }

    let height = (rect.width / ratio).round();
    match handle {
        None => {
            let center = (edges.top + edges.bottom) / 2.0;
            let top = (center - height / 2.0).round();
            Edges {
                top,
                bottom: top + height,
                ..edges
            }
            .rect()
        }
        Some(handle) if handle.is_top() => Edges {
            top: edges.bottom - height,
            ..edges
        }
        .rect(),
        Some(_) => Edges {
            bottom: edges.top + height,
            ..edges
        }
        .rect(),
    }
}

pub fn fit_rect(rect: Rect, bounds: Size) -> Rect {
    let width = rect.width.clamp(1.0, bounds.width.max(1.0));
    let height = rect.height.clamp(1.0, bounds.height.max(1.0));
    Rect {
        width,
        height,
        x: rect.x.clamp(0.0, (bounds.width - width).max(0.0)),
        y: rect.y.clamp(0.0, (bounds.height - height).max(0.0)),
    }
}

pub fn resize_rect(rect: Rect, point: Point, handle: Handle, ratio: Option<f32>) -> Rect {
    let edges = rect.edges();
    let mut resized = edges;

    if handle.is_left() {
        resized.left = point.x.min(edges.right - MIN_RESIZE_SIZE);
    }
    if handle.is_right() {
        resized.right = point.x.max(edges.left + MIN_RESIZE_SIZE);
    }
    if handle.is_top() {
        resized.top = point.y.min(edges.bottom - MIN_RESIZE_SIZE);
    }
    if handle.is_bottom() {
        resized.bottom = point.y.max(edges.top + MIN_RESIZE_SIZE);
    }

    let next = resized.rect();
    match ratio {
        Some(ratio) => adjust_rect_to_ratio(next, ratio, Some(handle)),
        None => next,
    }
}

pub fn move_rect(rect: Rect, point: Point, offset: Point, bounds: Size) -> Rect {
    Rect {
        x: if rect.width >= bounds.width {
            0.0
        } else {
            (point.x - offset.x).clamp(0.0, bounds.width - rect.width)
        },
        y: if rect.height >= bounds.height {
            0.0
        } else {
            (point.y - offset.y).clamp(0.0, bounds.height - rect.height)
        },
        ..rect
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOX: Rect = Rect {
        x: 100.0,
        y: 100.0,
        width: 200.0,
        height: 100.0,
    };
    const BOUNDS: Size = Size {
        width: 1000.0,
        height: 800.0,
    };

    fn point(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    #[test]
    fn normalizes_a_drag_in_any_direction() {
        let forward = normalize_rect(point(10.0, 20.0), point(50.0, 70.0));
        let backward = normalize_rect(point(50.0, 70.0), point(10.0, 20.0));
        assert_eq!(forward, backward);
        assert_eq!(
            forward,
            Rect {
                x: 10.0,
                y: 20.0,
                width: 40.0,
                height: 50.0
            }
        );
    }

    #[test]
    fn discards_a_selection_under_the_minimum() {
        assert!(!is_usable_selection(Rect {
            x: 0.0,
            y: 0.0,
            width: MIN_SELECTION_SIZE,
            height: 100.0
        }));
        assert!(is_usable_selection(Rect {
            x: 0.0,
            y: 0.0,
            width: MIN_SELECTION_SIZE + 1.0,
            height: MIN_SELECTION_SIZE + 1.0
        }));
    }

    #[test]
    fn hit_tests_every_handle_and_prefers_corners() {
        let edges = BOX.edges();
        assert_eq!(
            hit_test_handle(BOX, point(edges.left, edges.top)),
            Some(Handle::TopLeft)
        );
        assert_eq!(
            hit_test_handle(BOX, point(edges.right, edges.top)),
            Some(Handle::TopRight)
        );
        assert_eq!(
            hit_test_handle(BOX, point(edges.right, edges.bottom)),
            Some(Handle::BottomRight)
        );
        assert_eq!(
            hit_test_handle(BOX, point(edges.left, edges.bottom)),
            Some(Handle::BottomLeft)
        );
        let mid_x = (edges.left + edges.right) / 2.0;
        let mid_y = (edges.top + edges.bottom) / 2.0;
        assert_eq!(
            hit_test_handle(BOX, point(mid_x, edges.top)),
            Some(Handle::Top)
        );
        assert_eq!(
            hit_test_handle(BOX, point(mid_x, edges.bottom)),
            Some(Handle::Bottom)
        );
        assert_eq!(
            hit_test_handle(BOX, point(edges.left, mid_y)),
            Some(Handle::Left)
        );
        assert_eq!(
            hit_test_handle(BOX, point(edges.right, mid_y)),
            Some(Handle::Right)
        );
        // The middle of the box is not a handle.
        assert_eq!(hit_test_handle(BOX, point(mid_x, mid_y)), None);
    }

    #[test]
    fn cursor_follows_the_pointer() {
        assert_eq!(cursor_for(None, point(0.0, 0.0)), CursorStyle::Crosshair);
        assert_eq!(
            cursor_for(Some(BOX), point(BOX.x, BOX.y)),
            CursorStyle::ResizeUpLeftDownRight
        );
        assert_eq!(
            cursor_for(Some(BOX), point(BOX.x + BOX.width, BOX.y)),
            CursorStyle::ResizeUpRightDownLeft
        );
        assert_eq!(
            cursor_for(Some(BOX), point(BOX.x + BOX.width / 2.0, BOX.y)),
            CursorStyle::ResizeUpDown
        );
        assert_eq!(
            cursor_for(Some(BOX), point(BOX.x, BOX.y + BOX.height / 2.0)),
            CursorStyle::ResizeLeftRight
        );
        assert_eq!(
            cursor_for(Some(BOX), point(BOX.x + 60.0, BOX.y + 50.0)),
            CursorStyle::ClosedHand
        );
        assert_eq!(
            cursor_for(Some(BOX), point(BOX.x - 100.0, BOX.y)),
            CursorStyle::Crosshair
        );
    }

    #[test]
    fn resizing_respects_the_minimum_and_the_dragged_edge() {
        // Pulling the left edge past the right one stops at MIN_RESIZE_SIZE.
        let squashed = resize_rect(BOX, point(999.0, 150.0), Handle::Left, None);
        assert_eq!(squashed.width, MIN_RESIZE_SIZE);
        assert_eq!(squashed.x + squashed.width, BOX.x + BOX.width);
        // The opposite edge stays put while the dragged one moves.
        let widened = resize_rect(BOX, point(400.0, 150.0), Handle::Right, None);
        assert_eq!(widened.x, BOX.x);
        assert_eq!(widened.width, 300.0);
        // A vertical handle leaves the horizontal edges alone.
        let taller = resize_rect(BOX, point(0.0, 400.0), Handle::Bottom, None);
        assert_eq!(taller.x, BOX.x);
        assert_eq!(taller.width, BOX.width);
        assert_eq!(taller.height, 300.0);
    }

    #[test]
    fn resizing_under_a_ratio_pins_the_dragged_edge() {
        let square = resize_rect(BOX, point(400.0, 150.0), Handle::Right, Some(1.0));
        assert_eq!(square.width, square.height);
        // Dragging the right edge keeps the left one anchored.
        assert_eq!(square.x, BOX.x);
        let from_left = resize_rect(BOX, point(50.0, 150.0), Handle::Left, Some(1.0));
        assert_eq!(from_left.width, from_left.height);
        assert_eq!(from_left.x + from_left.width, BOX.x + BOX.width);
    }

    #[test]
    fn ratio_without_a_handle_grows_from_the_centre() {
        let wide = Rect {
            x: 100.0,
            y: 100.0,
            width: 400.0,
            height: 100.0,
        };
        let adjusted = adjust_rect_to_ratio(wide, 1.0, None);
        assert_eq!(adjusted.width, 100.0);
        assert_eq!(adjusted.height, 100.0);
        // Centres are preserved.
        assert_eq!(adjusted.x + adjusted.width / 2.0, wide.x + wide.width / 2.0);
    }

    #[test]
    fn moving_keeps_the_box_on_screen() {
        let offset = point(10.0, 10.0);
        let moved = move_rect(BOX, point(0.0, 0.0), offset, BOUNDS);
        assert_eq!(moved.x, 0.0);
        assert_eq!(moved.y, 0.0);
        assert_eq!(moved.width, BOX.width);
        let pushed = move_rect(BOX, point(9999.0, 9999.0), offset, BOUNDS);
        assert_eq!(pushed.x, BOUNDS.width - BOX.width);
        assert_eq!(pushed.y, BOUNDS.height - BOX.height);
        // A box wider than the screen pins to the origin.
        let oversized = Rect {
            x: 0.0,
            y: 0.0,
            width: BOUNDS.width + 100.0,
            height: 10.0,
        };
        assert_eq!(
            move_rect(oversized, point(500.0, 0.0), offset, BOUNDS).x,
            0.0
        );
    }

    #[test]
    fn fitting_clamps_size_and_origin() {
        let outside = Rect {
            x: 990.0,
            y: 790.0,
            width: 200.0,
            height: 200.0,
        };
        let fitted = fit_rect(outside, BOUNDS);
        assert_eq!(fitted.width, 200.0);
        assert_eq!(fitted.x, BOUNDS.width - 200.0);
        assert_eq!(fitted.y, BOUNDS.height - 200.0);
        let huge = fit_rect(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 5000.0,
                height: 5000.0,
            },
            BOUNDS,
        );
        assert_eq!(huge.width, BOUNDS.width);
        assert_eq!(huge.height, BOUNDS.height);
    }

    #[test]
    fn a_press_starts_the_gesture_the_renderer_would() {
        // No box yet: any press draws a new one.
        assert_eq!(gesture_at(None, point(0.0, 0.0)), Gesture::Create);
        // On a handle: resize that edge.
        assert_eq!(
            gesture_at(Some(BOX), point(BOX.x, BOX.y)),
            Gesture::Resize {
                handle: Handle::TopLeft
            }
        );
        assert_eq!(
            gesture_at(
                Some(BOX),
                point(BOX.x + BOX.width, BOX.y + BOX.height / 2.0)
            ),
            Gesture::Resize {
                handle: Handle::Right
            }
        );
        // Inside the box: move it, holding the grab offset.
        assert_eq!(
            gesture_at(Some(BOX), point(BOX.x + 60.0, BOX.y + 50.0)),
            Gesture::Move {
                offset: point(60.0, 50.0)
            }
        );
        // Outside it: start over.
        assert_eq!(
            gesture_at(Some(BOX), point(BOX.x - 100.0, BOX.y - 100.0)),
            Gesture::Create
        );
    }

    /// A press begins a gesture, dragging applies it, and the result stays on
    /// screen — the loop the overlay runs on every pointer stream.
    #[test]
    fn a_move_gesture_round_trips_through_the_grab_offset() {
        let grab = point(BOX.x + 30.0, BOX.y + 40.0);
        let Gesture::Move { offset } = gesture_at(Some(BOX), grab) else {
            panic!("expected a move");
        };
        // Dragging back to the same point leaves the box where it was.
        assert_eq!(move_rect(BOX, grab, offset, BOUNDS), BOX);
        // Dragging by a delta moves the box by that delta.
        let moved = move_rect(BOX, point(grab.x + 25.0, grab.y - 15.0), offset, BOUNDS);
        assert_eq!(moved.x, BOX.x + 25.0);
        assert_eq!(moved.y, BOX.y - 15.0);
    }

    #[test]
    fn clamping_keeps_the_pointer_inside_the_display() {
        assert_eq!(clamp_point(point(-5.0, -5.0), BOUNDS), point(0.0, 0.0));
        assert_eq!(
            clamp_point(point(5000.0, 5000.0), BOUNDS),
            point(BOUNDS.width, BOUNDS.height)
        );
    }
}
