import { describe, expect, it, vi, beforeEach } from 'vitest';
import { renderDrawings } from '../../src/renderer/components/video-editor/composition/drawing-canvas-renderer';
import type { DrawingSegment } from '../../src/types/drawing';
import type { Annotation } from '../../src/types/editor';

interface DrawImageCall {
  sx: number;
  sy: number;
  sw: number;
  sh: number;
}

const scratchDrawImageCalls: DrawImageCall[] = [];

class FakeOffscreenCanvas {
  width: number;
  height: number;

  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
  }

  getContext(): unknown {
    return {
      imageSmoothingEnabled: true,
      clearRect: vi.fn(),
      drawImage: vi.fn((_image: unknown, ...rest: number[]) => {
        if (rest.length >= 8) {
          scratchDrawImageCalls.push({
            sx: rest[0],
            sy: rest[1],
            sw: rest[2],
            sh: rest[3],
          });
        }
      }),
    };
  }
}

function createFakeContext(
  canvasWidth: number,
  canvasHeight: number,
  scale: number
) {
  return {
    canvas: { width: canvasWidth, height: canvasHeight },
    getTransform: () => ({ a: scale, b: 0, c: 0, d: scale, e: 0, f: 0 }),
    save: vi.fn(),
    restore: vi.fn(),
    imageSmoothingEnabled: true,
    drawImage: vi.fn(),
  };
}

const redact: Annotation = {
  id: 'r1',
  type: 'redact',
  x: 10,
  y: 10,
  width: 20,
  height: 20,
  style: 'pixelate',
  intensity: 5,
};

function makeSegment(): DrawingSegment {
  return {
    id: 's1',
    startTime: 0,
    endTime: 5,
    canvasWidth: 100,
    canvasHeight: 100,
    annotations: [redact],
  };
}

describe('renderDrawings redact scaling', () => {
  beforeEach(() => {
    scratchDrawImageCalls.length = 0;
    vi.stubGlobal('OffscreenCanvas', FakeOffscreenCanvas);
  });

  it('reads the source region in device pixels when the context is scaled', () => {
    const ctx = createFakeContext(200, 200, 2);

    renderDrawings(ctx as never, {
      drawingSegments: [makeSegment()],
      timelineTime: 1,
      width: 100,
      height: 100,
    });

    expect(scratchDrawImageCalls).toContainEqual({
      sx: 20,
      sy: 20,
      sw: 40,
      sh: 40,
    });
  });

  it('reads the source region in composition pixels when not scaled', () => {
    const ctx = createFakeContext(100, 100, 1);

    renderDrawings(ctx as never, {
      drawingSegments: [makeSegment()],
      timelineTime: 1,
      width: 100,
      height: 100,
    });

    expect(scratchDrawImageCalls).toContainEqual({
      sx: 10,
      sy: 10,
      sw: 20,
      sh: 20,
    });
  });
});
