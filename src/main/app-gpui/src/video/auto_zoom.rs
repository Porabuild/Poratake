//! Port of `types/auto-zoom.ts` — turns a recording's pointer activity into
//! zoom segments, capping each shot's zoom by how fast the camera would have
//! to pan to follow it.

use std::collections::HashMap;

use crate::video::sidecars::{CursorData, CursorEvent};
use crate::windows::video_editor::model::{FocusPoint, ZoomSegment};

const AUTO_ZOOM_ID_PREFIX: &str = "auto-zoom-";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ActionKind {
    Click,
    Drag,
    Scroll,
}

/// Zoom each kind of action deserves when the shot can afford it.
fn intent_zoom(kind: ActionKind) -> f64 {
    match kind {
        ActionKind::Click => 2.4,
        ActionKind::Drag => 1.6,
        ActionKind::Scroll => 1.9,
    }
}

/// How much an action counts when a cluster mixes several kinds.
fn action_weight(kind: ActionKind) -> f64 {
    match kind {
        ActionKind::Click => 1.0,
        ActionKind::Drag => 0.8,
        ActionKind::Scroll => 0.7,
    }
}

/// Time held before the first action of a segment, per kind.
fn lead_in(kind: ActionKind) -> f64 {
    match kind {
        ActionKind::Click => 0.55,
        ActionKind::Drag => 0.4,
        ActionKind::Scroll => 0.35,
    }
}

/// Time held after the last action of a segment, per kind.
fn hold_out(kind: ActionKind) -> f64 {
    match kind {
        ActionKind::Click => 0.9,
        ActionKind::Drag => 0.7,
        ActionKind::Scroll => 0.6,
    }
}

/// Zoom in/out duration written on the segment, per dominant kind.
fn transition(kind: ActionKind) -> f64 {
    match kind {
        ActionKind::Click => 0.6,
        ActionKind::Drag => 0.9,
        ActionKind::Scroll => 0.8,
    }
}

/// Below this a zoom is not worth the transition, so no segment is emitted.
const MIN_AUTO_ZOOM_LEVEL: f64 = 1.4;
const ZOOM_STEP: f64 = 0.05;

/// Viewport widths per second the camera may travel before a pan reads as a
/// whip.
const COMFORT_PAN_SPEED: f64 = 0.7;
/// Floor on the time a pan is given, so near-simultaneous actions stay finite.
const MIN_PAN_TIME: f64 = 0.35;
/// Past this the camera cannot keep up at any useful zoom, so the shot is cut.
const MAX_PAN_SPEED: f64 = COMFORT_PAN_SPEED / MIN_AUTO_ZOOM_LEVEL;

const CLUSTER_GAP: f64 = 1.5;
const MIN_SEGMENT_DURATION: f64 = 2.4;
const MIN_SEGMENT_GAP: f64 = 0.5;

/// A press that travels further than this, or is held longer, is a drag.
const DRAG_TRAVEL: f64 = 0.02;
const LONG_PRESS: f64 = 0.5;
const SCROLL_BURST_GAP: f64 = 0.6;
/// A cluster tighter than this is framed statically instead of followed.
const STATIC_EXTENT: f64 = 0.08;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Bounds {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl Bounds {
    fn point(x: f64, y: f64) -> Self {
        Self {
            min_x: x,
            max_x: x,
            min_y: y,
            max_y: y,
        }
    }

    fn with_point(self, x: f64, y: f64) -> Self {
        Self {
            min_x: self.min_x.min(x),
            max_x: self.max_x.max(x),
            min_y: self.min_y.min(y),
            max_y: self.max_y.max(y),
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            max_x: self.max_x.max(other.max_x),
            min_y: self.min_y.min(other.min_y),
            max_y: self.max_y.max(other.max_y),
        }
    }

    fn extent(self) -> f64 {
        (self.max_x - self.min_x).max(self.max_y - self.min_y)
    }

    fn center(self) -> (f64, f64) {
        (
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct Action {
    kind: ActionKind,
    start_time: f64,
    end_time: f64,
    bounds: Bounds,
}

#[derive(Clone, Debug)]
struct Cluster {
    actions: Vec<Action>,
    start_time: f64,
    end_time: f64,
    bounds: Bounds,
    pan_speed: f64,
}

#[derive(Clone, Copy, Debug)]
struct TimeWindow {
    start_time: f64,
    end_time: f64,
}

impl TimeWindow {
    fn duration(self) -> f64 {
        self.end_time - self.start_time
    }
}

#[derive(Clone, Debug)]
struct Shot {
    cluster: Cluster,
    window: TimeWindow,
}

fn round_to(value: f64, decimals: i32) -> f64 {
    let factor = 10f64.powi(decimals);
    (value * factor).round() / factor
}

#[derive(Clone, Copy, Debug)]
struct OpenPress {
    start_time: f64,
    origin: (f64, f64),
    bounds: Bounds,
}

fn to_pointer_action(press: OpenPress, end_time: f64) -> Action {
    let is_drag = press.bounds.extent() > DRAG_TRAVEL || end_time - press.start_time > LONG_PRESS;
    if is_drag {
        return Action {
            kind: ActionKind::Drag,
            start_time: press.start_time,
            end_time,
            bounds: press.bounds,
        };
    }
    Action {
        kind: ActionKind::Click,
        start_time: press.start_time,
        end_time,
        bounds: Bounds::point(press.origin.0, press.origin.1),
    }
}

fn extract_pointer_actions(events: &[CursorEvent]) -> Vec<Action> {
    let mut actions: Vec<Action> = Vec::new();
    // Insertion order matters: an unreleased press is flushed in the order it
    // was opened, which is what the renderer's `Map` iteration gives.
    let mut order: Vec<String> = Vec::new();
    let mut presses: HashMap<String, OpenPress> = HashMap::new();

    for event in events {
        let button = event.button.clone().unwrap_or_else(|| "left".to_string());

        if event.kind == "move" {
            for press in presses.values_mut() {
                press.bounds = press.bounds.with_point(event.x, event.y);
            }
            continue;
        }

        if event.kind == "down" {
            // A press with no matching release has an unknown duration, so it
            // counts as an instant click rather than an accidental long drag.
            if let Some(unreleased) = presses.remove(&button) {
                actions.push(to_pointer_action(unreleased, unreleased.start_time));
                order.retain(|value| value != &button);
            }
            presses.insert(
                button.clone(),
                OpenPress {
                    start_time: event.timestamp,
                    origin: (event.x, event.y),
                    bounds: Bounds::point(event.x, event.y),
                },
            );
            order.push(button);
            continue;
        }

        if event.kind != "up" {
            continue;
        }
        let Some(mut press) = presses.remove(&button) else {
            continue;
        };
        order.retain(|value| value != &button);
        press.bounds = press.bounds.with_point(event.x, event.y);
        actions.push(to_pointer_action(press, event.timestamp));
    }

    for button in order {
        if let Some(press) = presses.get(&button) {
            actions.push(to_pointer_action(*press, press.start_time));
        }
    }
    actions
}

fn extract_scroll_actions(events: &[CursorEvent]) -> Vec<Action> {
    let mut actions: Vec<Action> = Vec::new();
    for event in events {
        if event.kind != "scroll" {
            continue;
        }
        if let Some(burst) = actions.last_mut() {
            if event.timestamp - burst.end_time <= SCROLL_BURST_GAP {
                burst.end_time = event.timestamp;
                burst.bounds = burst.bounds.with_point(event.x, event.y);
                continue;
            }
        }
        actions.push(Action {
            kind: ActionKind::Scroll,
            start_time: event.timestamp,
            end_time: event.timestamp,
            bounds: Bounds::point(event.x, event.y),
        });
    }
    actions
}

fn extract_actions(events: &[CursorEvent]) -> Vec<Action> {
    let mut actions = extract_pointer_actions(events);
    actions.extend(extract_scroll_actions(events));
    actions.sort_by(|a, b| a.start_time.total_cmp(&b.start_time));
    actions
}

/// Screen fractions per second the camera must travel to link two actions.
fn pan_speed_between(from: &Action, to: &Action) -> f64 {
    let origin = from.bounds.center();
    let target = to.bounds.center();
    let distance = (target.0 - origin.0).hypot(target.1 - origin.1);
    distance / (to.start_time - from.end_time).max(MIN_PAN_TIME)
}

/// Screen fractions per second the camera must travel to follow one action.
fn pan_speed_within(action: &Action) -> f64 {
    action.bounds.extent() / (action.end_time - action.start_time).max(MIN_PAN_TIME)
}

fn cluster_actions(actions: &[Action]) -> Vec<Cluster> {
    let mut clusters: Vec<Cluster> = Vec::new();
    for action in actions {
        let pan = clusters
            .last()
            .and_then(|cluster| cluster.actions.last())
            .map(|last| pan_speed_between(last, action));

        if let (Some(current), Some(pan)) = (clusters.last_mut(), pan) {
            if action.start_time - current.end_time <= CLUSTER_GAP && pan <= MAX_PAN_SPEED {
                current.actions.push(*action);
                current.end_time = current.end_time.max(action.end_time);
                current.bounds = current.bounds.union(action.bounds);
                current.pan_speed = current.pan_speed.max(pan).max(pan_speed_within(action));
                continue;
            }
        }

        clusters.push(Cluster {
            actions: vec![*action],
            start_time: action.start_time,
            end_time: action.end_time,
            bounds: action.bounds,
            pan_speed: pan_speed_within(action),
        });
    }
    clusters
}

fn merge_clusters(a: &Cluster, b: &Cluster) -> Cluster {
    let link = match (a.actions.last(), b.actions.first()) {
        (Some(from), Some(to)) => pan_speed_between(from, to),
        _ => 0.0,
    };
    let mut actions = a.actions.clone();
    actions.extend(b.actions.iter().copied());
    Cluster {
        actions,
        start_time: a.start_time.min(b.start_time),
        end_time: a.end_time.max(b.end_time),
        bounds: a.bounds.union(b.bounds),
        pan_speed: a.pan_speed.max(b.pan_speed).max(link),
    }
}

/// The zoom the cluster asks for, capped by the zoom its camera motion allows.
fn zoom_level_of(cluster: &Cluster) -> f64 {
    let mut weighted = 0.0;
    let mut total = 0.0;
    for action in &cluster.actions {
        let weight = action_weight(action.kind);
        weighted += intent_zoom(action.kind) * weight;
        total += weight;
    }
    if total == 0.0 {
        return 0.0;
    }
    let intent = weighted / total;
    let pan_limit = if cluster.pan_speed > 0.0 {
        COMFORT_PAN_SPEED / cluster.pan_speed
    } else {
        f64::INFINITY
    };
    round_to((intent.min(pan_limit) / ZOOM_STEP).round() * ZOOM_STEP, 2)
}

fn dominant_kind(cluster: &Cluster) -> ActionKind {
    let mut totals: HashMap<ActionKind, f64> = HashMap::new();
    let mut dominant = cluster
        .actions
        .first()
        .map(|action| action.kind)
        .unwrap_or(ActionKind::Click);
    for action in &cluster.actions {
        let total = totals.get(&action.kind).copied().unwrap_or(0.0) + action_weight(action.kind);
        totals.insert(action.kind, total);
        if total > totals.get(&dominant).copied().unwrap_or(0.0) {
            dominant = action.kind;
        }
    }
    dominant
}

fn window_of(cluster: &Cluster, total_duration: f64) -> TimeWindow {
    let first = cluster
        .actions
        .first()
        .map(|action| action.kind)
        .unwrap_or(ActionKind::Click);
    let last = cluster
        .actions
        .last()
        .map(|action| action.kind)
        .unwrap_or(ActionKind::Click);

    let mut start_time = cluster.start_time - lead_in(first);
    let mut end_time = cluster.end_time + hold_out(last);

    let shortfall = MIN_SEGMENT_DURATION - (end_time - start_time);
    if shortfall > 0.0 {
        start_time -= shortfall / 2.0;
        end_time += shortfall / 2.0;
    }

    // Slide rather than shrink, so a segment at either end of the recording
    // keeps enough room for its transitions.
    let duration = (end_time - start_time).min(total_duration);
    let start = start_time.clamp(0.0, (total_duration - duration).max(0.0));
    TimeWindow {
        start_time: start,
        end_time: start + duration,
    }
}

/// Port of `resolveConflicts`.
fn resolve_conflicts(clusters: &[Cluster], total_duration: f64) -> Vec<Shot> {
    let mut resolved: Vec<Shot> = Vec::new();

    for cluster in clusters {
        let mut shot = Shot {
            cluster: cluster.clone(),
            window: window_of(cluster, total_duration),
        };

        let overlaps = resolved.last().is_some_and(|previous| {
            shot.window.start_time - previous.window.end_time < MIN_SEGMENT_GAP
        });
        if !overlaps {
            resolved.push(shot);
            continue;
        }

        let previous = resolved
            .last()
            .cloned()
            .expect("overlaps implies a previous shot");
        let merged = merge_clusters(&previous.cluster, cluster);
        if zoom_level_of(&merged) >= MIN_AUTO_ZOOM_LEVEL {
            let window = window_of(&merged, total_duration);
            *resolved.last_mut().expect("previous shot") = Shot {
                cluster: merged,
                window,
            };
            continue;
        }

        // Neither shot may run over the other's activity, so both give way at
        // the midpoint between them.
        let boundary = (previous.cluster.end_time + cluster.start_time) / 2.0;
        let previous_end = previous
            .window
            .end_time
            .min(boundary - MIN_SEGMENT_GAP / 2.0);
        resolved.last_mut().expect("previous shot").window.end_time = previous_end;
        shot.window.start_time = shot.window.start_time.max(boundary + MIN_SEGMENT_GAP / 2.0);

        if resolved
            .last()
            .is_some_and(|shot| shot.window.duration() < MIN_SEGMENT_DURATION)
        {
            resolved.pop();
        }
        if shot.window.duration() >= MIN_SEGMENT_DURATION {
            resolved.push(shot);
        }
    }

    resolved
}

fn to_zoom_segment(shot: &Shot, index: usize, stamp: i64) -> ZoomSegment {
    let kind = dominant_kind(&shot.cluster);
    let mut segment = ZoomSegment {
        id: format!("{AUTO_ZOOM_ID_PREFIX}{index}-{stamp}"),
        start_time: round_to(shot.window.start_time, 2),
        end_time: round_to(shot.window.end_time, 2),
        zoom_level: zoom_level_of(&shot.cluster),
        transition_in_duration: Some(transition(kind)),
        transition_out_duration: Some(transition(kind)),
        target_mode: None,
        focus_point: None,
    };

    // Activity confined to one spot is framed on that spot, so the shot holds
    // still instead of drifting with every cursor tremor.
    if shot.cluster.bounds.extent() <= STATIC_EXTENT {
        let (x, y) = shot.cluster.bounds.center();
        segment.target_mode = Some("manual".to_string());
        segment.focus_point = Some(FocusPoint {
            x: round_to(x, 4),
            y: round_to(y, 4),
        });
    }
    segment
}

/// `generateAutoZoomSegments`. `stamp` disambiguates ids across runs; the
/// renderer uses `Date.now()`.
pub fn generate(cursor_data: &CursorData, stamp: i64) -> Vec<ZoomSegment> {
    if cursor_data.events.is_empty() || cursor_data.meta.duration < MIN_SEGMENT_DURATION {
        return Vec::new();
    }

    let mut events = cursor_data.events.clone();
    events.sort_by(|a, b| a.timestamp.total_cmp(&b.timestamp));

    let worthwhile: Vec<Cluster> = cluster_actions(&extract_actions(&events))
        .into_iter()
        .filter(|cluster| zoom_level_of(cluster) >= MIN_AUTO_ZOOM_LEVEL)
        .collect();

    resolve_conflicts(&worthwhile, cursor_data.meta.duration)
        .iter()
        .enumerate()
        .map(|(index, shot)| to_zoom_segment(shot, index, stamp))
        .collect()
}

fn is_auto_zoom(segment: &ZoomSegment) -> bool {
    segment.id.starts_with(AUTO_ZOOM_ID_PREFIX)
}

/// `mergeAutoZoomSegments` — regenerating replaces previous auto zooms and
/// never overlaps a hand-placed one.
pub fn merge(existing: &[ZoomSegment], generated: &[ZoomSegment]) -> Vec<ZoomSegment> {
    let manual: Vec<ZoomSegment> = existing
        .iter()
        .filter(|segment| !is_auto_zoom(segment))
        .cloned()
        .collect();

    let mut result = manual.clone();
    result.extend(
        generated
            .iter()
            .filter(|segment| {
                !manual.iter().any(|other| {
                    segment.start_time < other.end_time && segment.end_time > other.start_time
                })
            })
            .cloned(),
    );
    result.sort_by(|a, b| a.start_time.total_cmp(&b.start_time));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video::sidecars::RecordingMeta;

    fn event(timestamp: f64, x: f64, y: f64, kind: &str) -> CursorEvent {
        CursorEvent {
            timestamp,
            x,
            y,
            kind: kind.to_string(),
            ..CursorEvent::default()
        }
    }

    fn data(events: Vec<CursorEvent>, duration: f64) -> CursorData {
        CursorData {
            events,
            meta: RecordingMeta {
                duration,
                ..RecordingMeta::default()
            },
            ..CursorData::default()
        }
    }

    #[test]
    fn a_short_recording_produces_nothing() {
        let events = vec![event(0.0, 0.5, 0.5, "down"), event(0.1, 0.5, 0.5, "up")];
        assert!(generate(&data(events, 1.0), 0).is_empty());
        assert!(generate(&data(Vec::new(), 30.0), 0).is_empty());
    }

    #[test]
    fn a_stationary_click_becomes_a_manual_shot() {
        let events = vec![
            event(5.0, 0.42, 0.31, "down"),
            event(5.05, 0.42, 0.31, "up"),
        ];
        let segments = generate(&data(events, 30.0), 7);
        assert_eq!(segments.len(), 1);
        let segment = &segments[0];
        assert!(segment.id.starts_with("auto-zoom-"));
        assert_eq!(segment.target_mode.as_deref(), Some("manual"));
        assert_eq!(segment.focus_point, Some(FocusPoint { x: 0.42, y: 0.31 }));
        assert!(segment.zoom_level >= MIN_AUTO_ZOOM_LEVEL);
        assert!(segment.end_time - segment.start_time >= MIN_SEGMENT_DURATION - 1e-9);
        assert_eq!(segment.transition_in_duration, Some(0.6));
    }

    #[test]
    fn a_fast_pan_across_the_screen_is_not_worth_zooming() {
        let events = vec![
            event(2.0, 0.05, 0.05, "down"),
            event(2.02, 0.05, 0.05, "up"),
            event(2.2, 0.95, 0.95, "down"),
            event(2.22, 0.95, 0.95, "up"),
        ];
        let segments = generate(&data(events, 30.0), 0);
        // Either the pair is dropped outright or it is framed wide enough to
        // cover both, never zoomed in tight on one of them.
        for segment in &segments {
            assert!(segment.zoom_level <= 2.0, "{}", segment.zoom_level);
        }
    }

    #[test]
    fn a_long_press_reads_as_a_drag() {
        let events = vec![event(3.0, 0.5, 0.5, "down"), event(3.9, 0.5, 0.5, "up")];
        let segments = generate(&data(events, 30.0), 0);
        assert_eq!(segments.len(), 1);
        // Drags get the drag transition, not the click one.
        assert_eq!(segments[0].transition_in_duration, Some(0.9));
    }

    #[test]
    fn scroll_bursts_collapse_into_one_shot() {
        let events: Vec<CursorEvent> = (0..10)
            .map(|index| event(4.0 + index as f64 * 0.1, 0.5, 0.5, "scroll"))
            .collect();
        let segments = generate(&data(events, 30.0), 0);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].transition_in_duration, Some(0.8));
    }

    #[test]
    fn merging_keeps_manual_segments_and_drops_stale_auto_ones() {
        let manual = ZoomSegment {
            id: "manual-1".into(),
            start_time: 10.0,
            end_time: 12.0,
            zoom_level: 2.0,
            ..ZoomSegment::default()
        };
        let stale = ZoomSegment {
            id: "auto-zoom-0-1".into(),
            start_time: 0.0,
            end_time: 3.0,
            zoom_level: 2.0,
            ..ZoomSegment::default()
        };
        let generated = vec![
            ZoomSegment {
                id: "auto-zoom-0-2".into(),
                start_time: 4.0,
                end_time: 7.0,
                zoom_level: 2.0,
                ..ZoomSegment::default()
            },
            // Overlaps the manual segment, so it is skipped.
            ZoomSegment {
                id: "auto-zoom-1-2".into(),
                start_time: 11.0,
                end_time: 13.0,
                zoom_level: 2.0,
                ..ZoomSegment::default()
            },
        ];

        let merged = merge(&[manual.clone(), stale], &generated);
        let ids: Vec<&str> = merged.iter().map(|segment| segment.id.as_str()).collect();
        assert_eq!(ids, vec!["auto-zoom-0-2", "manual-1"]);
    }

    #[test]
    fn generated_segments_never_overlap_each_other() {
        let events: Vec<CursorEvent> = (0..12)
            .flat_map(|index| {
                let time = 2.0 + index as f64 * 2.0;
                [
                    event(time, 0.5, 0.5, "down"),
                    event(time + 0.03, 0.5, 0.5, "up"),
                ]
            })
            .collect();
        let segments = generate(&data(events, 60.0), 0);
        for pair in segments.windows(2) {
            assert!(
                pair[1].start_time >= pair[0].end_time,
                "{:?} overlaps {:?}",
                pair[0],
                pair[1]
            );
        }
    }
}
