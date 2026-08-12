import { describe, expect, it } from 'vitest';
import {
  isCameraVisibleAt,
  mapVideoRangesToCameraSegments,
  type CameraSegment,
} from '@/types/camera';
import { splitTrackSegments } from '@/renderer/components/video-editor/timeline-split';

function makeCamera(overrides: Partial<CameraSegment> = {}): CameraSegment {
  return {
    id: 'camera-1',
    startTime: 0,
    endTime: 5,
    ...overrides,
  };
}

describe('mapVideoRangesToCameraSegments', () => {
  it('maps ranges identically when there are no video segments', () => {
    const result = mapVideoRangesToCameraSegments(
      [{ start: 2, end: 5 }],
      [],
      10
    );

    expect(result).toHaveLength(1);
    expect(result[0].startTime).toBe(2);
    expect(result[0].endTime).toBe(5);
  });

  it('covers the full duration when no ranges are recorded', () => {
    const result = mapVideoRangesToCameraSegments(null, [], 10);

    expect(result).toHaveLength(1);
    expect(result[0].startTime).toBe(0);
    expect(result[0].endTime).toBe(10);
  });

  it('clips ranges to the trimmed video segment on the timeline', () => {
    const segments = [{ originalStart: 2, originalEnd: 8 }];

    const full = mapVideoRangesToCameraSegments(null, segments, 6);
    expect(full).toHaveLength(1);
    expect(full[0].startTime).toBe(0);
    expect(full[0].endTime).toBe(6);

    const inner = mapVideoRangesToCameraSegments(
      [{ start: 4, end: 6 }],
      segments,
      6
    );
    expect(inner).toHaveLength(1);
    expect(inner[0].startTime).toBe(2);
    expect(inner[0].endTime).toBe(4);

    const trimmedAway = mapVideoRangesToCameraSegments(
      [{ start: 0, end: 2 }],
      segments,
      6
    );
    expect(trimmedAway).toHaveLength(0);
  });

  it('scales ranges by segment speed', () => {
    const segments = [{ originalStart: 0, originalEnd: 10, speed: 2 }];

    const result = mapVideoRangesToCameraSegments(
      [{ start: 4, end: 8 }],
      segments,
      5
    );

    expect(result).toHaveLength(1);
    expect(result[0].startTime).toBe(2);
    expect(result[0].endTime).toBe(4);
  });

  it('produces one clip per video segment that intersects a range', () => {
    const segments = [
      { originalStart: 5, originalEnd: 10 },
      { originalStart: 0, originalEnd: 5 },
    ];

    const result = mapVideoRangesToCameraSegments(
      [{ start: 0, end: 10 }],
      segments,
      10
    );
    expect(result).toHaveLength(2);
    expect(result[0]).toMatchObject({ startTime: 0, endTime: 5 });
    expect(result[1]).toMatchObject({ startTime: 5, endTime: 10 });

    const partial = mapVideoRangesToCameraSegments(
      [{ start: 7, end: 10 }],
      segments,
      10
    );
    expect(partial).toHaveLength(1);
    expect(partial[0]).toMatchObject({ startTime: 2, endTime: 5 });
  });
});

describe('isCameraVisibleAt', () => {
  const segments = [makeCamera({ startTime: 1, endTime: 3 })];

  it('returns true without segments', () => {
    expect(isCameraVisibleAt(null, 0)).toBe(true);
  });

  it('is inclusive at the start and exclusive at the end', () => {
    expect(isCameraVisibleAt(segments, 0.5)).toBe(false);
    expect(isCameraVisibleAt(segments, 1)).toBe(true);
    expect(isCameraVisibleAt(segments, 2.9)).toBe(true);
    expect(isCameraVisibleAt(segments, 3)).toBe(false);
  });
});

describe('splitTrackSegments with camera segments', () => {
  it('splits the camera segment containing the cut time', () => {
    const segments = [
      makeCamera(),
      makeCamera({ id: 'camera-2', startTime: 6, endTime: 9 }),
    ];

    const result = splitTrackSegments(segments, 2);

    expect(result).toHaveLength(3);
    expect(result[0]).toMatchObject({
      id: 'camera-1',
      startTime: 0,
      endTime: 2,
    });
    expect(result[1].startTime).toBe(2);
    expect(result[1].endTime).toBe(5);
    expect(result[1].id).not.toBe('camera-1');
    expect(result[2].id).toBe('camera-2');
  });

  it('returns the same array when no segment contains the cut time', () => {
    const segments = [makeCamera()];
    expect(splitTrackSegments(segments, 5.5)).toBe(segments);
  });
});
