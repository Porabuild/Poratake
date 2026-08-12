import { describe, expect, it, vi } from 'vitest';
import { renderCamera } from '../../src/renderer/components/video-editor/composition/camera-canvas-renderer';
import {
  DEFAULT_CAMERA_STYLE,
  isCameraVisibleAt,
  type CameraSegment,
} from '../../src/types/camera';
import type { VideoSegment } from '../../src/types/video';

const segments: VideoSegment[] = [
  {
    id: 'segment-1',
    startTime: 0,
    endTime: 10,
    timelineStart: 0,
    timelineEnd: 10,
  },
];

if (!('HTMLVideoElement' in globalThis)) {
  Object.defineProperty(globalThis, 'HTMLVideoElement', { value: class {} });
}

function createMockContext() {
  return {
    save: vi.fn(),
    restore: vi.fn(),
    beginPath: vi.fn(),
    rect: vi.fn(),
    roundRect: vi.fn(),
    clip: vi.fn(),
    fill: vi.fn(),
    translate: vi.fn(),
    scale: vi.fn(),
    drawImage: vi.fn(),
  } as unknown as CanvasRenderingContext2D;
}

function makeSegment(
  id: string,
  startTime: number,
  endTime: number
): CameraSegment {
  return { id, startTime, endTime };
}

function renderAt(
  timelineTime: number,
  cameraVisibleRanges: CameraSegment[] | null
) {
  const ctx = createMockContext();
  renderCamera(
    ctx,
    timelineTime,
    { width: 1280, height: 720 } as unknown as OffscreenCanvas,
    {
      cameraStyle: DEFAULT_CAMERA_STYLE,
      cameraVisibleRanges,
      segments,
      videoWidth: 1920,
      videoHeight: 1080,
      offsetX: 0,
      offsetY: 0,
    }
  );
  return ctx;
}

describe('isCameraVisibleAt', () => {
  it('treats a missing range list as always visible', () => {
    expect(isCameraVisibleAt(null, 0)).toBe(true);
    expect(isCameraVisibleAt(undefined, 42)).toBe(true);
  });

  it('treats an empty range list as never visible', () => {
    expect(isCameraVisibleAt([], 0)).toBe(false);
  });

  it('includes the range start and excludes the range end', () => {
    const ranges = [makeSegment('camera-1', 1, 3)];
    expect(isCameraVisibleAt(ranges, 0.999)).toBe(false);
    expect(isCameraVisibleAt(ranges, 1)).toBe(true);
    expect(isCameraVisibleAt(ranges, 2.999)).toBe(true);
    expect(isCameraVisibleAt(ranges, 3)).toBe(false);
  });

  it('matches any of several ranges', () => {
    const ranges = [
      makeSegment('camera-1', 0, 2),
      makeSegment('camera-2', 5, 8),
    ];
    expect(isCameraVisibleAt(ranges, 1)).toBe(true);
    expect(isCameraVisibleAt(ranges, 3)).toBe(false);
    expect(isCameraVisibleAt(ranges, 6)).toBe(true);
  });
});

describe('renderCamera visible ranges', () => {
  it('draws the overlay when no ranges are recorded', () => {
    expect(renderAt(4, null).drawImage).toHaveBeenCalled();
  });

  it('draws the overlay inside a recorded range', () => {
    expect(
      renderAt(1, [makeSegment('camera-1', 0, 2)]).drawImage
    ).toHaveBeenCalled();
  });

  it('skips the overlay outside every recorded range', () => {
    expect(
      renderAt(4, [makeSegment('camera-1', 0, 2)]).drawImage
    ).not.toHaveBeenCalled();
  });
});
