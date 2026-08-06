import type {
  CursorEvent,
  CursorType,
  InterpolatedCursorPosition,
} from '@/types/cursor';

export function findSurroundingEvents(
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

export function getMouseState(events: CursorEvent[], timestamp: number) {
  let isDown = false;
  let button: CursorEvent['button'];
  let cursorType: CursorType = 'arrow';

  const clickTimes: number[] = [];
  const mergeThreshold = 0.3;

  for (const event of events) {
    if (event.timestamp > timestamp) break;

    if (event.cursor) {
      cursorType = event.cursor;
    }

    if (event.type === 'down') {
      isDown = true;
      clickTimes.push(event.timestamp);
      button = event.button;
    } else if (event.type === 'up') {
      isDown = false;
      button = undefined;
    }
  }

  if (clickTimes.length === 0) {
    return { isDown, clickProgress: 0, button, cursorType };
  }

  let sequenceStart = clickTimes[clickTimes.length - 1];
  for (let i = clickTimes.length - 2; i >= 0; i--) {
    if (sequenceStart - clickTimes[i] < mergeThreshold) {
      sequenceStart = clickTimes[i];
    } else {
      break;
    }
  }

  const animationDuration = 0.35;
  const elapsed = timestamp - sequenceStart;
  const clickProgress =
    elapsed > animationDuration ? 0 : 1 - elapsed / animationDuration;

  return { isDown, clickProgress, button, cursorType };
}

export function applySmoothing(
  events: CursorEvent[],
  baseIndex: number,
  baseX: number,
  baseY: number,
  timestamp: number,
  smoothing: number
): { x: number; y: number } {
  const windowMs = 0.05 + smoothing * 0.1;
  const windowStart = timestamp - windowMs;
  const windowEnd = timestamp + windowMs * 0.5;
  const sigma = windowMs * 0.5;

  let weightedX = 0,
    weightedY = 0,
    totalWeight = 0;

  for (
    let i = Math.max(0, baseIndex - 20);
    i < events.length && i <= baseIndex + 10;
    i++
  ) {
    const e = events[i];
    if (e.timestamp < windowStart) continue;
    if (e.timestamp > windowEnd) break;

    const dist = Math.abs(e.timestamp - timestamp);
    const weight = Math.exp(-(dist * dist) / (2 * sigma * sigma));
    weightedX += e.x * weight;
    weightedY += e.y * weight;
    totalWeight += weight;
  }

  if (totalWeight === 0) return { x: baseX, y: baseY };

  const smoothedX = weightedX / totalWeight;
  const smoothedY = weightedY / totalWeight;
  return {
    x: baseX + (smoothedX - baseX) * smoothing,
    y: baseY + (smoothedY - baseY) * smoothing,
  };
}

export function interpolateCursorPosition(
  events: CursorEvent[],
  timestamp: number,
  smoothing = 0
): InterpolatedCursorPosition | null {
  if (events.length === 0) return null;

  const indices = findSurroundingEvents(events, timestamp);
  if (!indices) return null;

  const before = events[indices.before];
  const after = events[indices.after];

  let x: number, y: number;
  if (indices.before === indices.after) {
    x = before.x;
    y = before.y;
  } else {
    const t =
      (timestamp - before.timestamp) / (after.timestamp - before.timestamp);
    x = before.x + (after.x - before.x) * t;
    y = before.y + (after.y - before.y) * t;
  }

  if (smoothing > 0) {
    const smoothed = applySmoothing(
      events,
      indices.before,
      x,
      y,
      timestamp,
      smoothing
    );
    x = smoothed.x;
    y = smoothed.y;
  }

  const mouseState = getMouseState(events, timestamp);
  return { x, y, isClicking: mouseState.isDown, ...mouseState };
}

export function calculateClickBounceScale(progress: number): number {
  if (progress <= 0) return 1;

  const minScale = 0.7;
  const bellCurve = Math.sin(Math.PI * progress);
  const eased = bellCurve * bellCurve * (3 - 2 * bellCurve);

  return 1 - (1 - minScale) * eased;
}

export function calculateIdleOpacity(
  events: CursorEvent[],
  videoTime: number,
  timeout: number
): number {
  const fadeDuration = 0.5;
  const movementThreshold = 0.002;
  let lastMoveTime = 0;

  for (const event of events) {
    if (event.timestamp > videoTime) break;
    if (event.type === 'move' || event.type === 'down' || event.type === 'up') {
      lastMoveTime = event.timestamp;
    }
  }

  let hasMoved = false;
  for (let i = 1; i < events.length; i++) {
    if (events[i].timestamp > videoTime) break;
    if (events[i].timestamp < videoTime - timeout - fadeDuration) continue;

    const dx = events[i].x - events[i - 1].x;
    const dy = events[i].y - events[i - 1].y;
    if (Math.sqrt(dx * dx + dy * dy) > movementThreshold) {
      lastMoveTime = events[i].timestamp;
      hasMoved = true;
    }
  }

  if (!hasMoved && videoTime > timeout) return 0;

  const idleDuration = videoTime - lastMoveTime;
  if (idleDuration <= timeout) return 1;

  const fadeProgress = Math.min((idleDuration - timeout) / fadeDuration, 1);
  return 1 - fadeProgress;
}
