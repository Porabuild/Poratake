import type { ZoomSegment, ZoomSettings } from '@/types/zoom';
import type { CursorData, CursorEvent } from '@/types/cursor';
import type { VideoSegment } from '@/types/video';
import { BOUNDING_RATIO } from '@/types/zoom';
import { mapTimelineToVideoTime } from './types';

export interface ZoomState {
  scale: number;
  isZooming: boolean;
  segment: ZoomSegment | null;
  transitionProgress: number;
  effectiveTransitionIn: number;
  effectiveTransitionOut: number;
  isTransitioningIn: boolean;
  isTransitioningOut: boolean;
  zoomOutProgress: number;
}

export interface ViewportTransform {
  scale: number;
  translateX: number;
  translateY: number;
}

export interface Position {
  x: number;
  y: number;
}

function applyEasing(t: number): number {
  return t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

export function getZoomState(
  timelineTime: number,
  zoomSegments: ZoomSegment[],
  zoomSettings: ZoomSettings
): ZoomState {
  const { transitionInDuration, transitionOutDuration } = zoomSettings;

  for (const segment of zoomSegments) {
    const { startTime, endTime, zoomLevel } = segment;
    const segmentDuration = endTime - startTime;
    const segmentTransitionIn =
      segment.transitionInDuration ?? transitionInDuration;
    const segmentTransitionOut =
      segment.transitionOutDuration ?? transitionOutDuration;

    const effectiveTransitionIn = Math.min(
      segmentTransitionIn,
      segmentDuration / 2
    );
    const effectiveTransitionOut = Math.min(
      segmentTransitionOut,
      segmentDuration / 2
    );

    if (timelineTime >= startTime && timelineTime <= endTime) {
      const timeIntoSegment = timelineTime - startTime;
      const timeFromEnd = endTime - timelineTime;

      if (timeIntoSegment < effectiveTransitionIn) {
        const progress = timeIntoSegment / effectiveTransitionIn;
        const easedProgress = applyEasing(progress);
        const scale = 1 + (zoomLevel - 1) * easedProgress;
        return {
          scale,
          isZooming: true,
          segment,
          transitionProgress: progress,
          effectiveTransitionIn,
          effectiveTransitionOut,
          isTransitioningIn: true,
          isTransitioningOut: false,
          zoomOutProgress: 0,
        };
      }

      if (timeFromEnd < effectiveTransitionOut) {
        const progress = timeFromEnd / effectiveTransitionOut;
        const easedProgress = applyEasing(progress);
        const scale = 1 + (zoomLevel - 1) * easedProgress;
        const zoomOutProgress = 1 - progress;
        return {
          scale,
          isZooming: true,
          segment,
          transitionProgress: 1,
          effectiveTransitionIn,
          effectiveTransitionOut,
          isTransitioningIn: false,
          isTransitioningOut: true,
          zoomOutProgress: applyEasing(zoomOutProgress),
        };
      }

      return {
        scale: zoomLevel,
        isZooming: true,
        segment,
        transitionProgress: 1,
        effectiveTransitionIn,
        effectiveTransitionOut,
        isTransitioningIn: false,
        isTransitioningOut: false,
        zoomOutProgress: 0,
      };
    }
  }

  return {
    scale: 1,
    isZooming: false,
    segment: null,
    transitionProgress: 0,
    effectiveTransitionIn: 0,
    effectiveTransitionOut: 0,
    isTransitioningIn: false,
    isTransitioningOut: false,
    zoomOutProgress: 0,
  };
}

function findSurroundingEvents(
  events: CursorEvent[],
  timestamp: number
): { before: number; after: number } | null {
  let before = -1;
  let after = -1;

  for (let i = 0; i < events.length; i++) {
    if (events[i].timestamp <= timestamp) before = i;
    if (events[i].timestamp >= timestamp && after === -1) {
      after = i;
      break;
    }
  }

  if (before === -1 && after === -1) return null;
  return {
    before: before === -1 ? after : before,
    after: after === -1 ? before : after,
  };
}

function interpolateCursorPosition(
  events: CursorEvent[],
  timestamp: number
): Position | null {
  if (events.length === 0) return null;

  const indices = findSurroundingEvents(events, timestamp);
  if (!indices) return null;

  const before = events[indices.before];
  const after = events[indices.after];

  if (indices.before === indices.after) {
    return { x: before.x, y: before.y };
  }

  const t =
    (timestamp - before.timestamp) / (after.timestamp - before.timestamp);
  return {
    x: before.x + (after.x - before.x) * t,
    y: before.y + (after.y - before.y) * t,
  };
}

export function getCursorAtTime(
  cursorData: CursorData,
  videoSegments: VideoSegment[],
  timelineTime: number
): Position | null {
  const videoTime = mapTimelineToVideoTime(timelineTime, videoSegments);
  if (videoTime === null) return null;
  return interpolateCursorPosition(cursorData.events, videoTime);
}

export function collectCursorPositionsDuringSegment(
  cursorData: CursorData,
  videoSegments: VideoSegment[],
  segment: ZoomSegment,
  sampleRate: number = 30
): Position[] {
  const positions: Position[] = [];
  const duration = segment.endTime - segment.startTime;
  const sampleInterval = 1 / sampleRate;

  for (let t = 0; t <= duration; t += sampleInterval) {
    const timelineTime = segment.startTime + t;
    const cursor = getCursorAtTime(cursorData, videoSegments, timelineTime);
    if (cursor) {
      positions.push(cursor);
    }
  }

  return positions;
}

export function calculateOptimalCenter(
  cursorData: CursorData,
  videoSegments: VideoSegment[],
  segment: ZoomSegment,
  viewportSize: number,
  transitionInDuration: number = 0
): Position {
  const positions = collectCursorPositionsDuringSegment(
    cursorData,
    videoSegments,
    segment
  );

  if (positions.length === 0) {
    return { x: 0.5, y: 0.5 };
  }

  let minX = Infinity;
  let maxX = -Infinity;
  let minY = Infinity;
  let maxY = -Infinity;

  for (const pos of positions) {
    minX = Math.min(minX, pos.x);
    maxX = Math.max(maxX, pos.x);
    minY = Math.min(minY, pos.y);
    maxY = Math.max(maxY, pos.y);
  }

  const boundingSize = viewportSize * BOUNDING_RATIO;
  const cursorRangeX = maxX - minX;
  const cursorRangeY = maxY - minY;

  let centerX: number;
  let centerY: number;

  if (cursorRangeX <= boundingSize) {
    centerX = (minX + maxX) / 2;
  } else {
    const transitionEndTime = segment.startTime + transitionInDuration;
    const cursorAtTransitionEnd = getCursorAtTime(
      cursorData,
      videoSegments,
      transitionEndTime
    );
    centerX = cursorAtTransitionEnd?.x ?? positions[0].x;
  }

  if (cursorRangeY <= boundingSize) {
    centerY = (minY + maxY) / 2;
  } else {
    const transitionEndTime = segment.startTime + transitionInDuration;
    const cursorAtTransitionEnd = getCursorAtTime(
      cursorData,
      videoSegments,
      transitionEndTime
    );
    centerY = cursorAtTransitionEnd?.y ?? positions[0].y;
  }

  const maxViewport = 1 - viewportSize;
  const viewportX = clamp(centerX - viewportSize / 2, 0, maxViewport);
  const viewportY = clamp(centerY - viewportSize / 2, 0, maxViewport);

  return { x: viewportX, y: viewportY };
}

export function isCursorInBoundingArea(
  cursorPos: Position,
  viewportPos: Position,
  viewportSize: number,
  boundingRatio: number = BOUNDING_RATIO
): boolean {
  const boundingSize = viewportSize * boundingRatio;
  const margin = (viewportSize - boundingSize) / 2;

  const boundingLeft = viewportPos.x + margin;
  const boundingRight = viewportPos.x + viewportSize - margin;
  const boundingTop = viewportPos.y + margin;
  const boundingBottom = viewportPos.y + viewportSize - margin;

  return (
    cursorPos.x >= boundingLeft &&
    cursorPos.x <= boundingRight &&
    cursorPos.y >= boundingTop &&
    cursorPos.y <= boundingBottom
  );
}

export function calculateFollowTarget(
  cursorPos: Position,
  currentViewport: Position,
  viewportSize: number,
  boundingRatio: number = BOUNDING_RATIO
): Position {
  const boundingSize = viewportSize * boundingRatio;
  const margin = (viewportSize - boundingSize) / 2;
  const maxViewport = 1 - viewportSize;

  const boundingLeft = currentViewport.x + margin;
  const boundingRight = currentViewport.x + viewportSize - margin;
  const boundingTop = currentViewport.y + margin;
  const boundingBottom = currentViewport.y + viewportSize - margin;

  let targetX = currentViewport.x;
  let targetY = currentViewport.y;

  if (cursorPos.x < boundingLeft) {
    targetX = cursorPos.x - margin;
  } else if (cursorPos.x > boundingRight) {
    targetX = cursorPos.x - viewportSize + margin;
  }

  if (cursorPos.y < boundingTop) {
    targetY = cursorPos.y - margin;
  } else if (cursorPos.y > boundingBottom) {
    targetY = cursorPos.y - viewportSize + margin;
  }

  return {
    x: clamp(targetX, 0, maxViewport),
    y: clamp(targetY, 0, maxViewport),
  };
}

export interface SimulateViewportOptions {
  cursorData: CursorData;
  videoSegments: VideoSegment[];
  segment: ZoomSegment;
  currentTime: number;
  viewportSize: number;
  fps: number;
  transitionInDuration?: number;
  optimalCenter?: Position;
  followSmoothness?: number;
  lookAhead?: number;
}

interface ViewportKeyframe {
  time: number;
  x: number;
  y: number;
}

const viewportKeyframeCache = new Map<string, ViewportKeyframe[]>();

export function clearViewportKeyframeCache(): void {
  viewportKeyframeCache.clear();
}

const KEYFRAME_INTERVAL = 0.1;
const EDGE_MARGIN = 0.05;

const DEFAULT_FOLLOW_SMOOTHNESS = 0.3;
const DEFAULT_LOOK_AHEAD = 0.12;
const MAX_SPEED = 2.0;

interface SmoothState {
  posX: number;
  posY: number;
  velX: number;
  velY: number;
}

function smoothDamp(
  current: number,
  target: number,
  velocity: number,
  smoothTime: number,
  dt: number,
  maxSpeed: number = MAX_SPEED
): { value: number; velocity: number } {
  const omega = 2 / smoothTime;
  const x = omega * dt;
  const exp = 1 / (1 + x + 0.48 * x * x + 0.235 * x * x * x);

  let delta = current - target;
  const maxDelta = maxSpeed * smoothTime;
  delta = clamp(delta, -maxDelta, maxDelta);

  const temp = (velocity + omega * delta) * dt;
  let newVelocity = (velocity - omega * temp) * exp;
  let newValue = target + (delta + temp) * exp;

  if (target - current > 0 === newValue > target) {
    newValue = target;
    newVelocity = (newValue - current) / dt;
  }

  return { value: newValue, velocity: newVelocity };
}

function smoothDampPosition(
  current: SmoothState,
  targetX: number,
  targetY: number,
  smoothTime: number,
  dt: number
): SmoothState {
  const resultX = smoothDamp(
    current.posX,
    targetX,
    current.velX,
    smoothTime,
    dt
  );
  const resultY = smoothDamp(
    current.posY,
    targetY,
    current.velY,
    smoothTime,
    dt
  );

  return {
    posX: resultX.value,
    posY: resultY.value,
    velX: resultX.velocity,
    velY: resultY.velocity,
  };
}

function isCursorNearEdge(
  cursor: Position,
  viewportPos: Position,
  viewportSize: number
): boolean {
  const margin = viewportSize * EDGE_MARGIN;
  return (
    cursor.x < viewportPos.x + margin ||
    cursor.x > viewportPos.x + viewportSize - margin ||
    cursor.y < viewportPos.y + margin ||
    cursor.y > viewportPos.y + viewportSize - margin
  );
}

function isCursorOutsideViewport(
  cursor: Position,
  viewportPos: Position,
  viewportSize: number
): boolean {
  return (
    cursor.x < viewportPos.x ||
    cursor.x > viewportPos.x + viewportSize ||
    cursor.y < viewportPos.y ||
    cursor.y > viewportPos.y + viewportSize
  );
}

function generateViewportKeyframes(
  cursorData: CursorData,
  videoSegments: VideoSegment[],
  segment: ZoomSegment,
  viewportSize: number,
  startViewport: Position,
  followSmoothness: number,
  lookAhead: number
): ViewportKeyframe[] {
  const keyframes: ViewportKeyframe[] = [];
  const maxViewport = 1 - viewportSize;
  const duration = segment.endTime - segment.startTime;

  let state: SmoothState = {
    posX: clamp(startViewport.x, 0, maxViewport),
    posY: clamp(startViewport.y, 0, maxViewport),
    velX: 0,
    velY: 0,
  };

  keyframes.push({ time: segment.startTime, x: state.posX, y: state.posY });

  const steps = Math.ceil(duration / KEYFRAME_INTERVAL);
  for (let i = 1; i <= steps; i++) {
    const time = segment.startTime + i * KEYFRAME_INTERVAL;
    if (time > segment.endTime) break;

    const cursor = getCursorAtTime(
      cursorData,
      videoSegments,
      Math.min(time + lookAhead, segment.endTime)
    );
    if (!cursor) {
      keyframes.push({
        time,
        x: state.posX,
        y: state.posY,
      });
      continue;
    }

    const currentViewport = { x: state.posX, y: state.posY };
    const isOutside = isCursorOutsideViewport(
      cursor,
      currentViewport,
      viewportSize
    );
    const isNearEdge = isCursorNearEdge(cursor, currentViewport, viewportSize);
    const inBounds = isCursorInBoundingArea(
      cursor,
      currentViewport,
      viewportSize
    );

    if (isOutside || isNearEdge || !inBounds) {
      const target = calculateFollowTarget(
        cursor,
        currentViewport,
        viewportSize
      );

      let smoothTime = followSmoothness;
      if (isOutside) {
        smoothTime = followSmoothness * 0.27;
      } else if (isNearEdge) {
        smoothTime = followSmoothness * 0.5;
      }

      state = smoothDampPosition(
        state,
        target.x,
        target.y,
        Math.max(0.04, smoothTime),
        KEYFRAME_INTERVAL
      );
    } else {
      state.velX *= 0.8;
      state.velY *= 0.8;
    }

    keyframes.push({
      time,
      x: clamp(state.posX, 0, maxViewport),
      y: clamp(state.posY, 0, maxViewport),
    });
  }

  return keyframes;
}

function getViewportKeyframes(
  segmentId: string,
  cursorData: CursorData,
  videoSegments: VideoSegment[],
  segment: ZoomSegment,
  viewportSize: number,
  startViewport: Position,
  followSmoothness: number,
  lookAhead: number
): ViewportKeyframe[] {
  const cacheKey = `${segmentId}-${viewportSize.toFixed(4)}-${followSmoothness.toFixed(2)}-${lookAhead.toFixed(2)}`;

  const cached = viewportKeyframeCache.get(cacheKey);
  if (cached) {
    return cached;
  }

  const keyframes = generateViewportKeyframes(
    cursorData,
    videoSegments,
    segment,
    viewportSize,
    startViewport,
    followSmoothness,
    lookAhead
  );

  viewportKeyframeCache.set(cacheKey, keyframes);
  return keyframes;
}

function catmullRom(
  p0: number,
  p1: number,
  p2: number,
  p3: number,
  t: number
): number {
  const t2 = t * t;
  const t3 = t2 * t;
  return (
    0.5 *
    (2 * p1 +
      (-p0 + p2) * t +
      (2 * p0 - 5 * p1 + 4 * p2 - p3) * t2 +
      (-p0 + 3 * p1 - 3 * p2 + p3) * t3)
  );
}

function interpolateKeyframes(
  keyframes: ViewportKeyframe[],
  time: number
): Position {
  if (keyframes.length === 0) {
    return { x: 0.5, y: 0.5 };
  }

  if (time <= keyframes[0].time) {
    return { x: keyframes[0].x, y: keyframes[0].y };
  }

  if (time >= keyframes[keyframes.length - 1].time) {
    const last = keyframes[keyframes.length - 1];
    return { x: last.x, y: last.y };
  }

  let low = 0;
  let high = keyframes.length - 1;
  while (low < high - 1) {
    const mid = Math.floor((low + high) / 2);
    if (keyframes[mid].time <= time) {
      low = mid;
    } else {
      high = mid;
    }
  }

  const t =
    (time - keyframes[low].time) / (keyframes[high].time - keyframes[low].time);

  const i0 = Math.max(0, low - 1);
  const i1 = low;
  const i2 = high;
  const i3 = Math.min(keyframes.length - 1, high + 1);

  const p0 = keyframes[i0];
  const p1 = keyframes[i1];
  const p2 = keyframes[i2];
  const p3 = keyframes[i3];

  return {
    x: catmullRom(p0.x, p1.x, p2.x, p3.x, t),
    y: catmullRom(p0.y, p1.y, p2.y, p3.y, t),
  };
}

export function simulateViewport(options: SimulateViewportOptions): Position {
  const {
    cursorData,
    videoSegments,
    segment,
    currentTime,
    viewportSize,
    transitionInDuration = 0,
    optimalCenter,
    followSmoothness = DEFAULT_FOLLOW_SMOOTHNESS,
    lookAhead = DEFAULT_LOOK_AHEAD,
  } = options;

  const maxViewport = 1 - viewportSize;

  const startViewport =
    optimalCenter ??
    calculateOptimalCenter(
      cursorData,
      videoSegments,
      segment,
      viewportSize,
      transitionInDuration
    );

  const keyframes = getViewportKeyframes(
    segment.id,
    cursorData,
    videoSegments,
    segment,
    viewportSize,
    startViewport,
    followSmoothness,
    lookAhead
  );

  const interpolated = interpolateKeyframes(keyframes, currentTime);

  return {
    x: clamp(interpolated.x, 0, maxViewport),
    y: clamp(interpolated.y, 0, maxViewport),
  };
}
