import type { CursorData, CursorEvent } from '@/types/cursor';
import type { ZoomSegment } from '@/types/zoom';

const AUTO_ZOOM_LEVEL = 2;

interface ClickEvent {
  timestamp: number;
  x: number;
  y: number;
}

const MIN_SEGMENT_DURATION = 3;
const SEGMENT_PADDING_BEFORE = 0.5;
const SEGMENT_PADDING_AFTER = 1;
const NO_CLICK_GAP_THRESHOLD = 3;
const CLICK_DEDUP_THRESHOLD = 0.1;

function extractClickEvents(events: CursorEvent[]): ClickEvent[] {
  const clicks: ClickEvent[] = [];
  let lastClickTime = -1;

  for (const event of events) {
    if (event.type === 'down' && event.button === 'left') {
      if (
        lastClickTime < 0 ||
        event.timestamp - lastClickTime > CLICK_DEDUP_THRESHOLD
      ) {
        clicks.push({
          timestamp: event.timestamp,
          x: event.x,
          y: event.y,
        });
        lastClickTime = event.timestamp;
      }
    }
  }

  return clicks;
}

interface ClickSequence {
  startTime: number;
  endTime: number;
}

function findClickSequences(clicks: ClickEvent[]): ClickSequence[] {
  if (clicks.length === 0) return [];

  const sequences: ClickSequence[] = [];
  let sequenceStart = clicks[0].timestamp;
  let lastClickTime = clicks[0].timestamp;

  for (let i = 1; i < clicks.length; i++) {
    const gap = clicks[i].timestamp - lastClickTime;

    if (gap > NO_CLICK_GAP_THRESHOLD) {
      sequences.push({
        startTime: sequenceStart,
        endTime: lastClickTime,
      });
      sequenceStart = clicks[i].timestamp;
    }

    lastClickTime = clicks[i].timestamp;
  }

  sequences.push({
    startTime: sequenceStart,
    endTime: lastClickTime,
  });

  return sequences;
}

function sequenceToZoomSegment(
  sequence: ClickSequence,
  totalDuration: number,
  index: number
): ZoomSegment {
  let startTime = Math.max(0, sequence.startTime - SEGMENT_PADDING_BEFORE);
  let endTime = Math.min(
    totalDuration,
    sequence.endTime + SEGMENT_PADDING_AFTER
  );

  const duration = endTime - startTime;
  if (duration < MIN_SEGMENT_DURATION) {
    const padding = (MIN_SEGMENT_DURATION - duration) / 2;
    startTime = Math.max(0, startTime - padding);
    endTime = Math.min(totalDuration, endTime + padding);
  }

  return {
    id: `auto-zoom-${index}-${Date.now()}`,
    startTime: Math.round(startTime * 100) / 100,
    endTime: Math.round(endTime * 100) / 100,
    zoomLevel: AUTO_ZOOM_LEVEL,
  };
}

export function generateAutoZoomSegments(
  cursorData: CursorData
): ZoomSegment[] {
  const { events, meta } = cursorData;
  const totalDuration = meta.duration;

  if (events.length === 0 || totalDuration < MIN_SEGMENT_DURATION) {
    return [];
  }

  const clicks = extractClickEvents(events);

  if (clicks.length === 0) {
    return [];
  }

  const sequences = findClickSequences(clicks);

  return sequences.map((sequence, index) =>
    sequenceToZoomSegment(sequence, totalDuration, index)
  );
}
