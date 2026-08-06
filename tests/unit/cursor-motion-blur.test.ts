import { describe, expect, it, vi, beforeEach } from 'vitest';
import type { CursorData, CursorStyle } from '../../src/types/cursor';
import { DEFAULT_CURSOR_STYLE } from '../../src/types/cursor';
import type { VideoSegment } from '../../src/types/video';

class MockImage {
  complete = true;
  naturalWidth = 32;
  set src(_value: string) {}
}

interface MockContext {
  drawCount: number;
  alphas: number[];
  globalAlpha: number;
  save: () => void;
  restore: () => void;
  translate: () => void;
  scale: () => void;
  clearRect: () => void;
  drawImage: () => void;
}

function createMockContext(): MockContext {
  return {
    drawCount: 0,
    alphas: [],
    globalAlpha: 1,
    save() {},
    restore() {},
    translate() {},
    scale() {},
    clearRect() {},
    drawImage() {
      this.drawCount += 1;
      this.alphas.push(this.globalAlpha);
    },
  };
}

let bufferContexts: MockContext[] = [];

class MockOffscreenCanvas {
  width: number;
  height: number;
  private ctx: MockContext;

  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
    this.ctx = createMockContext();
    bufferContexts.push(this.ctx);
  }

  getContext() {
    return this.ctx;
  }
}

beforeEach(() => {
  vi.resetModules();
  bufferContexts = [];
  vi.stubGlobal('Image', MockImage);
  vi.stubGlobal('OffscreenCanvas', MockOffscreenCanvas);
  vi.stubGlobal(
    'Blob',
    class {
      constructor(..._args: unknown[]) {}
    }
  );
  URL.createObjectURL = () => 'blob:mock';
  URL.revokeObjectURL = () => {};
});

const segments: VideoSegment[] = [
  { id: 'seg-1', startTime: 0, endTime: 10, timelineStart: 0, speed: 1 },
];

function movingCursor(): CursorData {
  return {
    recordingArea: { width: 1920, height: 1080 },
    events: [
      { timestamp: 0, x: 0.1, y: 0.5, type: 'move' },
      { timestamp: 1, x: 0.9, y: 0.5, type: 'move' },
      { timestamp: 2, x: 0.9, y: 0.5, type: 'move' },
    ],
    meta: {
      startTime: '2026-05-29T00:00:00.000Z',
      duration: 2,
      sampleRate: 30,
    },
  };
}

function staticCursor(): CursorData {
  return {
    recordingArea: { width: 1920, height: 1080 },
    events: [
      { timestamp: 0, x: 0.5, y: 0.5, type: 'move' },
      { timestamp: 1, x: 0.5, y: 0.5, type: 'move' },
      { timestamp: 2, x: 0.5, y: 0.5, type: 'move' },
    ],
    meta: {
      startTime: '2026-05-29T00:00:00.000Z',
      duration: 2,
      sampleRate: 30,
    },
  };
}

const baseStyle: CursorStyle = {
  ...DEFAULT_CURSOR_STYLE,
  smoothing: 0,
  showClickHighlight: false,
};

async function loadRenderer() {
  return import('../../src/renderer/components/video-editor/composition/cursor-canvas-renderer');
}

describe('cursor motion blur', () => {
  it('accumulates blur taps in an offscreen buffer and blits once', async () => {
    const { renderCursor } = await loadRenderer();
    const ctx = createMockContext();

    renderCursor(ctx as unknown as CanvasRenderingContext2D, 0.5, {
      cursorData: movingCursor(),
      cursorStyle: { ...baseStyle, motionBlur: true, motionBlurStrength: 1 },
      segments,
      videoWidth: 1920,
      videoHeight: 1080,
      offsetX: 0,
      offsetY: 0,
    });

    expect(ctx.drawCount).toBe(1);
    expect(bufferContexts).toHaveLength(1);
    expect(bufferContexts[0].drawCount).toBe(9);
    expect(bufferContexts[0].alphas[0]).toBe(1);
    expect(bufferContexts[0].alphas[8]).toBeCloseTo(1 / 9, 6);
  });

  it('keeps the blitted cursor fully opaque', async () => {
    const { renderCursor } = await loadRenderer();
    const ctx = createMockContext();

    renderCursor(ctx as unknown as CanvasRenderingContext2D, 0.5, {
      cursorData: movingCursor(),
      cursorStyle: { ...baseStyle, motionBlur: true, motionBlurStrength: 1 },
      segments,
      videoWidth: 1920,
      videoHeight: 1080,
      offsetX: 0,
      offsetY: 0,
    });

    expect(ctx.alphas[0]).toBe(1);
  });

  it('draws a single sprite when motion blur is disabled', async () => {
    const { renderCursor } = await loadRenderer();
    const ctx = createMockContext();

    renderCursor(ctx as unknown as CanvasRenderingContext2D, 0.5, {
      cursorData: movingCursor(),
      cursorStyle: { ...baseStyle, motionBlur: false },
      segments,
      videoWidth: 1920,
      videoHeight: 1080,
      offsetX: 0,
      offsetY: 0,
    });

    expect(ctx.drawCount).toBe(1);
    expect(bufferContexts).toHaveLength(0);
  });

  it('draws a single sprite when the cursor is not moving', async () => {
    const { renderCursor } = await loadRenderer();
    const ctx = createMockContext();

    renderCursor(ctx as unknown as CanvasRenderingContext2D, 1.5, {
      cursorData: staticCursor(),
      cursorStyle: { ...baseStyle, motionBlur: true, motionBlurStrength: 1 },
      segments,
      videoWidth: 1920,
      videoHeight: 1080,
      offsetX: 0,
      offsetY: 0,
    });

    expect(ctx.drawCount).toBe(1);
    expect(bufferContexts).toHaveLength(0);
  });
});
