import { describe, expect, it } from 'vitest';
import { DEFAULT_ZOOM_SETTINGS, type ZoomSegment } from '@/types/zoom';
import type { CursorData } from '@/types/cursor';
import type { VideoSegment } from '@/types/video';
import { calculateZoomTransform } from '@/renderer/components/video-editor/composition/zoom-canvas-renderer';

const videoSegments: VideoSegment[] = [
  {
    id: 'segment-1',
    startTime: 0,
    endTime: 10,
    timelineStart: 0,
    speed: 1,
  },
];

const cursorData: CursorData = {
  events: [
    { timestamp: 0, x: 0.2, y: 0.5 },
    { timestamp: 1, x: 0.3, y: 0.5 },
    { timestamp: 2, x: 0.85, y: 0.5 },
    { timestamp: 3, x: 0.9, y: 0.5 },
  ],
};

const zoomSegment: ZoomSegment = {
  id: 'zoom-1',
  startTime: 0,
  endTime: 4,
  zoomLevel: 2,
};

describe('zoom composition logic', () => {
  it('keeps cursor follow viewport inside video bounds', () => {
    const transform = calculateZoomTransform(
      [zoomSegment],
      DEFAULT_ZOOM_SETTINGS,
      cursorData,
      videoSegments,
      2.5,
      1920,
      1080,
      { fps: 60 }
    );

    expect(transform.viewport?.x).toBeGreaterThanOrEqual(0);
    expect(transform.viewport?.x).toBeLessThanOrEqual(1 - 1 / transform.scale);
    expect(transform.viewport?.y).toBeGreaterThanOrEqual(0);
    expect(transform.viewport?.y).toBeLessThanOrEqual(1 - 1 / transform.scale);
  });
});
