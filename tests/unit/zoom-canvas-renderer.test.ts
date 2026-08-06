import { describe, expect, it } from 'vitest';
import { calculateZoomTransform } from '../../src/renderer/components/video-editor/composition/zoom-canvas-renderer';
import type { CursorData } from '../../src/types/cursor';
import type { VideoSegment } from '../../src/types/video';
import type { ZoomSegment, ZoomSettings } from '../../src/types/zoom';

const VIDEO_WIDTH = 1920;
const VIDEO_HEIGHT = 1080;

const videoSegments: VideoSegment[] = [
  {
    id: 'seg-1',
    startTime: 0,
    endTime: 10,
    timelineStart: 0,
    speed: 1,
  },
];

const zoomSegments: ZoomSegment[] = [
  {
    id: 'zoom-1',
    startTime: 0,
    endTime: 4,
    zoomLevel: 2,
  },
];

const zoomSettings: ZoomSettings = {
  transitionInDuration: 1,
  transitionOutDuration: 1,
  easing: 'ease-in-out',
};

const centeredCursorData: CursorData = {
  recordingArea: { width: VIDEO_WIDTH, height: VIDEO_HEIGHT },
  events: [
    { timestamp: 0, x: 0.5, y: 0.5, type: 'move' },
    { timestamp: 2, x: 0.5, y: 0.5, type: 'move' },
    { timestamp: 4, x: 0.5, y: 0.5, type: 'move' },
  ],
  meta: {
    startTime: '2026-02-08T00:00:00.000Z',
    duration: 4,
    sampleRate: 30,
  },
};

describe('calculateZoomTransform', () => {
  it('matches centered cursor behavior during transition-in without cursor data', () => {
    const timelineTime = 0.5;

    const withoutCursor = calculateZoomTransform(
      zoomSegments,
      zoomSettings,
      null,
      videoSegments,
      timelineTime,
      VIDEO_WIDTH,
      VIDEO_HEIGHT,
      { fps: 60 }
    );

    const withCenteredCursor = calculateZoomTransform(
      zoomSegments,
      zoomSettings,
      centeredCursorData,
      videoSegments,
      timelineTime,
      VIDEO_WIDTH,
      VIDEO_HEIGHT,
      { fps: 60 }
    );

    expect(withoutCursor.scale).toBeCloseTo(withCenteredCursor.scale, 6);
    expect(withoutCursor.translateX).toBeCloseTo(
      withCenteredCursor.translateX,
      6
    );
    expect(withoutCursor.translateY).toBeCloseTo(
      withCenteredCursor.translateY,
      6
    );
  });

  it('matches centered cursor behavior during steady zoom without cursor data', () => {
    const timelineTime = 2;

    const withoutCursor = calculateZoomTransform(
      zoomSegments,
      zoomSettings,
      null,
      videoSegments,
      timelineTime,
      VIDEO_WIDTH,
      VIDEO_HEIGHT,
      { fps: 60 }
    );

    const withCenteredCursor = calculateZoomTransform(
      zoomSegments,
      zoomSettings,
      centeredCursorData,
      videoSegments,
      timelineTime,
      VIDEO_WIDTH,
      VIDEO_HEIGHT,
      { fps: 60 }
    );

    expect(withoutCursor.scale).toBeCloseTo(withCenteredCursor.scale, 6);
    expect(withoutCursor.translateX).toBeCloseTo(
      withCenteredCursor.translateX,
      6
    );
    expect(withoutCursor.translateY).toBeCloseTo(
      withCenteredCursor.translateY,
      6
    );
  });
});
