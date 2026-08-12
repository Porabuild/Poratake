import { describe, it, expect } from 'vitest';
import {
  generateAutoZoomSegments,
  mergeAutoZoomSegments,
} from '@/types/auto-zoom';
import type { CursorData, CursorEvent } from '@/types/cursor';
import type { ZoomSegment } from '@/types/zoom';

function makeCursorData(
  events: CursorEvent[],
  duration: number = 30
): CursorData {
  return {
    recordingArea: { width: 1920, height: 1080 },
    events,
    meta: {
      startTime: '2026-01-22T18:00:00.000Z',
      duration,
      sampleRate: 32,
    },
  };
}

function click(
  timestamp: number,
  x: number,
  y: number,
  button: 'left' | 'right' = 'left'
): CursorEvent[] {
  return [
    { timestamp, x, y, type: 'down', button },
    { timestamp: timestamp + 0.08, x, y, type: 'up', button },
  ];
}

function scroll(timestamp: number, x: number, y: number): CursorEvent {
  return { timestamp, x, y, type: 'scroll', scrollDelta: { x: 0, y: -3 } };
}

describe('Auto Zoom Generation', () => {
  it('should return empty array for empty cursor data', () => {
    const segments = generateAutoZoomSegments(makeCursorData([], 10));
    expect(segments).toEqual([]);
  });

  it('should return empty array for short recordings', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData(click(1, 0.5, 0.5), 2)
    );
    expect(segments).toEqual([]);
  });

  it('should return empty array for no interactions', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData(
        [
          { timestamp: 0, x: 0.5, y: 0.5, type: 'move' },
          { timestamp: 5, x: 0.6, y: 0.6, type: 'move' },
          { timestamp: 10, x: 0.7, y: 0.7, type: 'move' },
        ],
        10
      )
    );
    expect(segments).toEqual([]);
  });

  it('should generate one zoom segment for continuous clicks', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData(
        [
          ...click(1, 0.5, 0.5),
          ...click(2, 0.5, 0.5),
          ...click(3, 0.5, 0.5),
          ...click(4, 0.5, 0.5),
        ],
        10
      )
    );

    expect(segments.length).toBe(1);
    expect(segments[0].zoomLevel).toBe(2.4);
  });

  it('should generate multiple segments when there are gaps with no clicks', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData(
        [
          ...click(1, 0.2, 0.2),
          ...click(2, 0.2, 0.2),
          ...click(10, 0.8, 0.8),
          ...click(11, 0.8, 0.8),
        ],
        15
      )
    );

    expect(segments.length).toBe(2);
    expect(segments[0].endTime).toBeLessThan(segments[1].startTime);
  });

  it('should collapse rapid duplicate clicks into one segment', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData(
        [
          ...click(1, 0.5, 0.5),
          ...click(1.01, 0.5, 0.5),
          ...click(1.02, 0.5, 0.5),
          ...click(2, 0.5, 0.5),
          ...click(2.01, 0.5, 0.5),
        ],
        10
      )
    );

    expect(segments.length).toBe(1);
  });

  it('should have valid segment structure', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData([...click(2, 0.5, 0.5), ...click(3, 0.5, 0.5)], 10)
    );

    expect(segments.length).toBeGreaterThan(0);
    for (const segment of segments) {
      expect(segment.id).toMatch(/^auto-zoom-\d+-\d+$/);
      expect(segment.startTime).toBeLessThan(segment.endTime);
      expect(segment.zoomLevel).toBeGreaterThanOrEqual(1.4);
      expect(segment.zoomLevel).toBeLessThanOrEqual(3);
    }
  });

  it('should enforce minimum segment duration', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData(click(5, 0.5, 0.5), 15)
    );

    expect(segments.length).toBe(1);
    expect(segments[0].endTime - segments[0].startTime).toBeGreaterThanOrEqual(
      2.4
    );
  });
});

describe('Auto Zoom Levels', () => {
  it('should zoom further on clicks than on drags', () => {
    const clicked = generateAutoZoomSegments(
      makeCursorData(click(5, 0.5, 0.5))
    );
    const dragged = generateAutoZoomSegments(
      makeCursorData([
        { timestamp: 5, x: 0.45, y: 0.45, type: 'down', button: 'left' },
        { timestamp: 5.3, x: 0.5, y: 0.5, type: 'move' },
        { timestamp: 5.6, x: 0.55, y: 0.55, type: 'move' },
        { timestamp: 5.9, x: 0.55, y: 0.55, type: 'up', button: 'left' },
      ])
    );

    expect(clicked[0].zoomLevel).toBe(2.4);
    expect(dragged[0].zoomLevel).toBe(1.6);
    expect(dragged[0].zoomLevel).toBeLessThan(clicked[0].zoomLevel);
  });

  it('should keep scrolls between drags and clicks', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData([scroll(10, 0.5, 0.5), scroll(10.1, 0.5, 0.5)])
    );

    expect(segments.length).toBe(1);
    expect(segments[0].zoomLevel).toBe(1.9);
  });

  it('should blend the zoom level when a cluster mixes actions', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData([
        ...click(5, 0.5, 0.5),
        { timestamp: 6, x: 0.5, y: 0.5, type: 'down', button: 'left' },
        { timestamp: 6.3, x: 0.53, y: 0.53, type: 'move' },
        { timestamp: 6.6, x: 0.53, y: 0.53, type: 'up', button: 'left' },
        ...click(7.5, 0.5, 0.5),
      ])
    );

    expect(segments.length).toBe(1);
    expect(segments[0].zoomLevel).toBeGreaterThan(1.6);
    expect(segments[0].zoomLevel).toBeLessThan(2.4);
  });

  it('should pull the zoom back when clicks are spread apart', () => {
    const near = generateAutoZoomSegments(
      makeCursorData([...click(5, 0.5, 0.5), ...click(6, 0.54, 0.54)])
    );
    const far = generateAutoZoomSegments(
      makeCursorData([...click(5, 0.4, 0.4), ...click(6, 0.65, 0.65)])
    );

    expect(near.length).toBe(1);
    expect(far.length).toBe(1);
    expect(near[0].zoomLevel).toBe(2.4);
    expect(far[0].zoomLevel).toBeLessThan(near[0].zoomLevel);
    expect(far[0].zoomLevel).toBeGreaterThanOrEqual(1.4);
  });

  it('should not zoom on a drag that sweeps across the screen', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData([
        { timestamp: 5, x: 0.05, y: 0.05, type: 'down', button: 'left' },
        { timestamp: 5.4, x: 0.5, y: 0.5, type: 'move' },
        { timestamp: 5.8, x: 0.95, y: 0.95, type: 'move' },
        { timestamp: 6, x: 0.95, y: 0.95, type: 'up', button: 'left' },
      ])
    );

    expect(segments).toEqual([]);
  });

  it('should not zoom when clicks whip across the screen', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData([...click(5, 0.05, 0.05), ...click(5.3, 0.95, 0.95)])
    );

    expect(segments).toEqual([]);
  });
});

describe('Auto Zoom Framing', () => {
  it('should frame a stationary cluster on its focus point', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData([...click(5, 0.3, 0.7), ...click(6, 0.32, 0.72)])
    );

    expect(segments[0].targetMode).toBe('manual');
    expect(segments[0].focusPoint).toEqual({ x: 0.31, y: 0.71 });
  });

  it('should follow the cursor when the cluster covers ground', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData([...click(5, 0.3, 0.3), ...click(6, 0.55, 0.55)])
    );

    expect(segments[0].targetMode).toBeUndefined();
    expect(segments[0].focusPoint).toBeUndefined();
  });

  it('should punch in faster on clicks than on drags', () => {
    const clicked = generateAutoZoomSegments(
      makeCursorData(click(5, 0.5, 0.5))
    );
    const dragged = generateAutoZoomSegments(
      makeCursorData([
        { timestamp: 5, x: 0.45, y: 0.45, type: 'down', button: 'left' },
        { timestamp: 5.6, x: 0.55, y: 0.55, type: 'move' },
        { timestamp: 5.9, x: 0.55, y: 0.55, type: 'up', button: 'left' },
      ])
    );

    expect(clicked[0].transitionInDuration).toBe(0.6);
    expect(clicked[0].transitionOutDuration).toBe(0.6);
    expect(dragged[0].transitionInDuration).toBe(0.9);
  });
});

describe('Auto Zoom Interactions', () => {
  it('should zoom on drags', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData([
        { timestamp: 5, x: 0.4, y: 0.4, type: 'down', button: 'left' },
        { timestamp: 6, x: 0.45, y: 0.45, type: 'move' },
        { timestamp: 7, x: 0.5, y: 0.5, type: 'move' },
        { timestamp: 8, x: 0.5, y: 0.5, type: 'up', button: 'left' },
      ])
    );

    expect(segments.length).toBe(1);
    expect(segments[0].startTime).toBeLessThanOrEqual(5);
    expect(segments[0].endTime).toBeGreaterThanOrEqual(8);
  });

  it('should zoom on scroll bursts', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData([
        scroll(10, 0.5, 0.5),
        scroll(10.1, 0.5, 0.5),
        scroll(10.2, 0.5, 0.5),
      ])
    );

    expect(segments.length).toBe(1);
    expect(segments[0].startTime).toBeLessThanOrEqual(10);
    expect(segments[0].endTime).toBeGreaterThanOrEqual(10.2);
  });

  it('should treat distant scroll bursts as separate segments', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData([scroll(2, 0.5, 0.5), scroll(20, 0.5, 0.5)])
    );

    expect(segments.length).toBe(2);
  });

  it('should zoom on right clicks', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData(click(5, 0.5, 0.5, 'right'))
    );

    expect(segments.length).toBe(1);
  });

  it('should keep zoom across small gaps between interactions', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData([
        ...click(1, 0.5, 0.5),
        ...click(2.2, 0.52, 0.52),
        ...click(3.4, 0.5, 0.51),
      ])
    );

    expect(segments.length).toBe(1);
    expect(segments[0].startTime).toBeLessThanOrEqual(1);
    expect(segments[0].endTime).toBeGreaterThanOrEqual(3.48);
  });

  it('should split zoom when interactions are far apart', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData([...click(1, 0.5, 0.5), ...click(10, 0.5, 0.5)])
    );

    expect(segments.length).toBe(2);
    expect(segments[0].endTime).toBeLessThan(segments[1].startTime);
  });

  it('should leave the full frame visible between segments', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData([
        ...click(1, 0.5, 0.5),
        ...click(6, 0.5, 0.5),
        ...click(11, 0.5, 0.5),
        ...click(20, 0.5, 0.5),
      ])
    );

    expect(segments.length).toBeGreaterThan(1);
    for (let i = 1; i < segments.length; i++) {
      expect(
        segments[i].startTime - segments[i - 1].endTime
      ).toBeGreaterThanOrEqual(0.5);
    }
  });

  it('should not cover the whole timeline with a single zoom', () => {
    const events: CursorEvent[] = [];
    for (let timestamp = 1; timestamp < 60; timestamp += 4) {
      events.push(...click(timestamp, 0.2 + (timestamp % 3) * 0.2, 0.5));
    }

    const segments = generateAutoZoomSegments(makeCursorData(events, 60));
    const covered = segments.reduce(
      (total, segment) => total + (segment.endTime - segment.startTime),
      0
    );

    expect(segments.length).toBeGreaterThan(3);
    expect(covered).toBeLessThan(60 * 0.8);
  });

  it('should keep segments inside the recording timeline', () => {
    const segments = generateAutoZoomSegments(
      makeCursorData(
        [
          ...click(0.2, 0.5, 0.5),
          ...click(3, 0.52, 0.52),
          ...click(9.8, 0.5, 0.5),
        ],
        10
      )
    );

    expect(segments.length).toBeGreaterThan(0);
    for (const segment of segments) {
      expect(segment.startTime).toBeGreaterThanOrEqual(0);
      expect(segment.endTime).toBeLessThanOrEqual(10);
    }
  });
});

describe('mergeAutoZoomSegments', () => {
  const manual: ZoomSegment = {
    id: 'manual-1',
    startTime: 10,
    endTime: 13,
    zoomLevel: 1.5,
  };
  const staleAuto: ZoomSegment = {
    id: 'auto-zoom-0-111',
    startTime: 1,
    endTime: 4,
    zoomLevel: 2,
  };

  it('should replace previous auto segments and keep manual ones', () => {
    const generated: ZoomSegment[] = [
      { id: 'auto-zoom-0-222', startTime: 1, endTime: 4, zoomLevel: 2 },
      { id: 'auto-zoom-1-222', startTime: 20, endTime: 23, zoomLevel: 2 },
    ];

    const merged = mergeAutoZoomSegments([manual, staleAuto], generated);

    expect(merged.map(segment => segment.id)).toEqual([
      'auto-zoom-0-222',
      'manual-1',
      'auto-zoom-1-222',
    ]);
  });

  it('should skip generated segments overlapping manual ones', () => {
    const generated: ZoomSegment[] = [
      { id: 'auto-zoom-0-222', startTime: 11, endTime: 14, zoomLevel: 2 },
      { id: 'auto-zoom-1-222', startTime: 20, endTime: 23, zoomLevel: 2 },
    ];

    const merged = mergeAutoZoomSegments([manual], generated);

    expect(merged.map(segment => segment.id)).toEqual([
      'manual-1',
      'auto-zoom-1-222',
    ]);
  });
});
