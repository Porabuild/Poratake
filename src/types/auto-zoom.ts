import type { CursorData, CursorEvent, MouseButton } from './cursor';
import type { ZoomSegment } from './zoom';
import { clamp } from '@/types/geometry';

const AUTO_ZOOM_ID_PREFIX = 'auto-zoom-';

type ActionKind = 'click' | 'drag' | 'scroll';

/** Zoom each kind of action deserves when the shot can afford it. */
const INTENT_ZOOM: Record<ActionKind, number> = {
  click: 2.4,
  drag: 1.6,
  scroll: 1.9,
};

/** How much an action counts when a cluster mixes several kinds. */
const ACTION_WEIGHT: Record<ActionKind, number> = {
  click: 1,
  drag: 0.8,
  scroll: 0.7,
};

/** Time held before the first action of a segment, per kind. */
const LEAD_IN: Record<ActionKind, number> = {
  click: 0.55,
  drag: 0.4,
  scroll: 0.35,
};

/** Time held after the last action of a segment, per kind. */
const HOLD_OUT: Record<ActionKind, number> = {
  click: 0.9,
  drag: 0.7,
  scroll: 0.6,
};

/** Zoom in/out duration written on the segment, per dominant kind. */
const TRANSITION: Record<ActionKind, number> = {
  click: 0.6,
  drag: 0.9,
  scroll: 0.8,
};

/** Below this a zoom is not worth the transition, so no segment is emitted. */
const MIN_AUTO_ZOOM_LEVEL = 1.4;
const ZOOM_STEP = 0.05;

/**
 * Viewport widths per second the camera may travel before a pan reads as a
 * whip. A viewport is `1 / zoomLevel` of the frame, so the screen-space budget
 * shrinks as the zoom grows - which is what keeps spread-out activity wide and
 * lets stationary activity punch in.
 */
const COMFORT_PAN_SPEED = 0.7;
/** Floor on the time a pan is given, so near-simultaneous actions stay finite. */
const MIN_PAN_TIME = 0.35;
/** Past this the camera cannot keep up at any useful zoom, so the shot is cut. */
const MAX_PAN_SPEED = COMFORT_PAN_SPEED / MIN_AUTO_ZOOM_LEVEL;

const CLUSTER_GAP = 1.5;
const MIN_SEGMENT_DURATION = 2.4;
const MIN_SEGMENT_GAP = 0.5;

/** A press that travels further than this, or is held longer, is a drag. */
const DRAG_TRAVEL = 0.02;
const LONG_PRESS = 0.5;
const SCROLL_BURST_GAP = 0.6;
/** A cluster tighter than this is framed statically instead of followed. */
const STATIC_EXTENT = 0.08;

interface Point {
  x: number;
  y: number;
}

interface Bounds {
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
}

interface Action {
  kind: ActionKind;
  startTime: number;
  endTime: number;
  bounds: Bounds;
}

interface Cluster {
  actions: Action[];
  startTime: number;
  endTime: number;
  bounds: Bounds;
  panSpeed: number;
}

interface TimeWindow {
  startTime: number;
  endTime: number;
}

/** A cluster together with the stretch of timeline it is shown over. */
interface Shot {
  cluster: Cluster;
  window: TimeWindow;
}

function roundTo(value: number, decimals: number): number {
  const factor = 10 ** decimals;
  return Math.round(value * factor) / factor;
}

function pointBounds(point: Point): Bounds {
  return { minX: point.x, maxX: point.x, minY: point.y, maxY: point.y };
}

function withPoint(bounds: Bounds, point: Point): Bounds {
  return {
    minX: Math.min(bounds.minX, point.x),
    maxX: Math.max(bounds.maxX, point.x),
    minY: Math.min(bounds.minY, point.y),
    maxY: Math.max(bounds.maxY, point.y),
  };
}

function unionBounds(a: Bounds, b: Bounds): Bounds {
  return {
    minX: Math.min(a.minX, b.minX),
    maxX: Math.max(a.maxX, b.maxX),
    minY: Math.min(a.minY, b.minY),
    maxY: Math.max(a.maxY, b.maxY),
  };
}

function extentOf(bounds: Bounds): number {
  return Math.max(bounds.maxX - bounds.minX, bounds.maxY - bounds.minY);
}

function centerOf(bounds: Bounds): Point {
  return {
    x: (bounds.minX + bounds.maxX) / 2,
    y: (bounds.minY + bounds.maxY) / 2,
  };
}

interface OpenPress {
  startTime: number;
  origin: Point;
  bounds: Bounds;
}

function toPointerAction(press: OpenPress, endTime: number): Action {
  const isDrag =
    extentOf(press.bounds) > DRAG_TRAVEL ||
    endTime - press.startTime > LONG_PRESS;

  if (isDrag) {
    return {
      kind: 'drag',
      startTime: press.startTime,
      endTime,
      bounds: press.bounds,
    };
  }

  return {
    kind: 'click',
    startTime: press.startTime,
    endTime,
    bounds: pointBounds(press.origin),
  };
}

function extractPointerActions(events: CursorEvent[]): Action[] {
  const actions: Action[] = [];
  const presses = new Map<MouseButton, OpenPress>();

  for (const event of events) {
    const button = event.button ?? 'left';

    if (event.type === 'move') {
      for (const press of presses.values()) {
        press.bounds = withPoint(press.bounds, event);
      }
      continue;
    }

    if (event.type === 'down') {
      const unreleased = presses.get(button);
      // A press with no matching release has an unknown duration, so it counts
      // as an instant click rather than an accidental long drag.
      if (unreleased)
        actions.push(toPointerAction(unreleased, unreleased.startTime));

      presses.set(button, {
        startTime: event.timestamp,
        origin: { x: event.x, y: event.y },
        bounds: pointBounds(event),
      });
      continue;
    }

    if (event.type !== 'up') continue;

    const press = presses.get(button);
    if (!press) continue;

    presses.delete(button);
    press.bounds = withPoint(press.bounds, event);
    actions.push(toPointerAction(press, event.timestamp));
  }

  for (const press of presses.values()) {
    actions.push(toPointerAction(press, press.startTime));
  }

  return actions;
}

function extractScrollActions(events: CursorEvent[]): Action[] {
  const actions: Action[] = [];

  for (const event of events) {
    if (event.type !== 'scroll') continue;

    const burst = actions[actions.length - 1];
    if (burst && event.timestamp - burst.endTime <= SCROLL_BURST_GAP) {
      burst.endTime = event.timestamp;
      burst.bounds = withPoint(burst.bounds, event);
      continue;
    }

    actions.push({
      kind: 'scroll',
      startTime: event.timestamp,
      endTime: event.timestamp,
      bounds: pointBounds(event),
    });
  }

  return actions;
}

function extractActions(events: CursorEvent[]): Action[] {
  return [
    ...extractPointerActions(events),
    ...extractScrollActions(events),
  ].toSorted((a, b) => a.startTime - b.startTime);
}

/** Screen fractions per second the camera must travel to link two actions. */
function panSpeedBetween(from: Action, to: Action): number {
  const origin = centerOf(from.bounds);
  const target = centerOf(to.bounds);
  const distance = Math.hypot(target.x - origin.x, target.y - origin.y);

  return distance / Math.max(to.startTime - from.endTime, MIN_PAN_TIME);
}

/** Screen fractions per second the camera must travel to follow one action. */
function panSpeedWithin(action: Action): number {
  return (
    extentOf(action.bounds) /
    Math.max(action.endTime - action.startTime, MIN_PAN_TIME)
  );
}

function clusterActions(actions: Action[]): Cluster[] {
  const clusters: Cluster[] = [];

  for (const action of actions) {
    const current = clusters[clusters.length - 1];
    const pan = current
      ? panSpeedBetween(current.actions[current.actions.length - 1], action)
      : Infinity;

    if (
      current &&
      action.startTime - current.endTime <= CLUSTER_GAP &&
      pan <= MAX_PAN_SPEED
    ) {
      current.actions.push(action);
      current.endTime = Math.max(current.endTime, action.endTime);
      current.bounds = unionBounds(current.bounds, action.bounds);
      current.panSpeed = Math.max(
        current.panSpeed,
        pan,
        panSpeedWithin(action)
      );
      continue;
    }

    clusters.push({
      actions: [action],
      startTime: action.startTime,
      endTime: action.endTime,
      bounds: action.bounds,
      panSpeed: panSpeedWithin(action),
    });
  }

  return clusters;
}

function mergeClusters(a: Cluster, b: Cluster): Cluster {
  return {
    actions: [...a.actions, ...b.actions],
    startTime: Math.min(a.startTime, b.startTime),
    endTime: Math.max(a.endTime, b.endTime),
    bounds: unionBounds(a.bounds, b.bounds),
    panSpeed: Math.max(
      a.panSpeed,
      b.panSpeed,
      panSpeedBetween(a.actions[a.actions.length - 1], b.actions[0])
    ),
  };
}

/**
 * The zoom the cluster asks for, capped by the zoom its camera motion allows.
 * Stationary clicks reach their full intent; a cluster the camera has to chase
 * is pulled back until the pan fits inside the comfort budget.
 */
function zoomLevelOf(cluster: Cluster): number {
  let weighted = 0;
  let total = 0;

  for (const action of cluster.actions) {
    const weight = ACTION_WEIGHT[action.kind];
    weighted += INTENT_ZOOM[action.kind] * weight;
    total += weight;
  }

  const intent = weighted / total;
  const panLimit =
    cluster.panSpeed > 0 ? COMFORT_PAN_SPEED / cluster.panSpeed : Infinity;

  return roundTo(
    Math.round(Math.min(intent, panLimit) / ZOOM_STEP) * ZOOM_STEP,
    2
  );
}

function dominantKind(cluster: Cluster): ActionKind {
  const totals = new Map<ActionKind, number>();
  let dominant = cluster.actions[0].kind;

  for (const action of cluster.actions) {
    const total = (totals.get(action.kind) ?? 0) + ACTION_WEIGHT[action.kind];
    totals.set(action.kind, total);
    if (total > (totals.get(dominant) ?? 0)) dominant = action.kind;
  }

  return dominant;
}

function windowOf(cluster: Cluster, totalDuration: number): TimeWindow {
  const actions = cluster.actions;
  let startTime = cluster.startTime - LEAD_IN[actions[0].kind];
  let endTime = cluster.endTime + HOLD_OUT[actions[actions.length - 1].kind];

  const shortfall = MIN_SEGMENT_DURATION - (endTime - startTime);
  if (shortfall > 0) {
    startTime -= shortfall / 2;
    endTime += shortfall / 2;
  }

  // Slide rather than shrink, so a segment at either end of the recording keeps
  // enough room for its transitions.
  const duration = Math.min(endTime - startTime, totalDuration);
  const start = clamp(startTime, 0, totalDuration - duration);

  return { startTime: start, endTime: start + duration };
}

function durationOf(window: TimeWindow): number {
  return window.endTime - window.startTime;
}

/**
 * Shots closer than `MIN_SEGMENT_GAP` never return to the full frame between
 * them. They become one shot when the wider framing is still worth zooming;
 * otherwise both give way at the boundary, because a shot that runs over the
 * next one's activity would hold on an area the user has already left. A shot
 * left too short to complete its transitions is dropped.
 */
function resolveConflicts(clusters: Cluster[], totalDuration: number): Shot[] {
  const resolved: Shot[] = [];

  for (const cluster of clusters) {
    const shot: Shot = { cluster, window: windowOf(cluster, totalDuration) };
    const previous = resolved[resolved.length - 1];

    if (
      !previous ||
      shot.window.startTime - previous.window.endTime >= MIN_SEGMENT_GAP
    ) {
      resolved.push(shot);
      continue;
    }

    const merged = mergeClusters(previous.cluster, cluster);
    if (zoomLevelOf(merged) >= MIN_AUTO_ZOOM_LEVEL) {
      resolved[resolved.length - 1] = {
        cluster: merged,
        window: windowOf(merged, totalDuration),
      };
      continue;
    }

    const boundary = (previous.cluster.endTime + cluster.startTime) / 2;
    previous.window.endTime = Math.min(
      previous.window.endTime,
      boundary - MIN_SEGMENT_GAP / 2
    );
    shot.window.startTime = Math.max(
      shot.window.startTime,
      boundary + MIN_SEGMENT_GAP / 2
    );

    if (durationOf(previous.window) < MIN_SEGMENT_DURATION) resolved.pop();
    if (durationOf(shot.window) >= MIN_SEGMENT_DURATION) resolved.push(shot);
  }

  return resolved;
}

function toZoomSegment(shot: Shot, index: number, stamp: number): ZoomSegment {
  const { cluster, window } = shot;
  const kind = dominantKind(cluster);

  const segment: ZoomSegment = {
    id: `${AUTO_ZOOM_ID_PREFIX}${index}-${stamp}`,
    startTime: roundTo(window.startTime, 2),
    endTime: roundTo(window.endTime, 2),
    zoomLevel: zoomLevelOf(cluster),
    transitionInDuration: TRANSITION[kind],
    transitionOutDuration: TRANSITION[kind],
  };

  // Activity confined to one spot is framed on that spot, so the shot holds
  // still instead of drifting with every cursor tremor.
  if (extentOf(cluster.bounds) <= STATIC_EXTENT) {
    const center = centerOf(cluster.bounds);
    segment.targetMode = 'manual';
    segment.focusPoint = {
      x: roundTo(center.x, 4),
      y: roundTo(center.y, 4),
    };
  }

  return segment;
}

export function generateAutoZoomSegments(
  cursorData: CursorData
): ZoomSegment[] {
  const { events, meta } = cursorData;

  if (events.length === 0 || meta.duration < MIN_SEGMENT_DURATION) {
    return [];
  }

  const sortedEvents = events.toSorted((a, b) => a.timestamp - b.timestamp);
  const worthwhile = clusterActions(extractActions(sortedEvents)).filter(
    cluster => zoomLevelOf(cluster) >= MIN_AUTO_ZOOM_LEVEL
  );

  const stamp = Date.now();
  return resolveConflicts(worthwhile, meta.duration).map((shot, index) =>
    toZoomSegment(shot, index, stamp)
  );
}

function isAutoZoomSegment(segment: ZoomSegment): boolean {
  return segment.id.startsWith(AUTO_ZOOM_ID_PREFIX);
}

export function mergeAutoZoomSegments(
  existing: ZoomSegment[],
  generated: ZoomSegment[]
): ZoomSegment[] {
  const manual = existing.filter(segment => !isAutoZoomSegment(segment));

  const additions = generated.filter(
    segment =>
      !manual.some(
        other =>
          segment.startTime < other.endTime && segment.endTime > other.startTime
      )
  );

  return [...manual, ...additions].toSorted(
    (a, b) => a.startTime - b.startTime
  );
}
