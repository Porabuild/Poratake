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

describe('cursor sprite size', () => {
  it('renders a life-size cursor at 100% on a 1080p recording', async () => {
    const { resolveCursorSpriteSize } = await loadRenderer();

    expect(resolveCursorSpriteSize(100, 1080, 1080)).toBeCloseTo(49, 6);
  });

  it('grows with the recording resolution to track display scaling', async () => {
    const { resolveCursorSpriteSize } = await loadRenderer();

    expect(resolveCursorSpriteSize(100, 2160, 1080)).toBeCloseTo(98, 6);
    expect(resolveCursorSpriteSize(100, 1600, 1000)).toBeCloseTo(78.4, 1);
  });

  it('does not scale a 4K recording captured at 1x', async () => {
    const { resolveCursorSpriteSize } = await loadRenderer();

    expect(resolveCursorSpriteSize(100, 2160, 2160)).toBeCloseTo(49, 6);
  });

  it('clamps the display scale for very tall recordings', async () => {
    const { resolveCursorSpriteSize } = await loadRenderer();

    expect(resolveCursorSpriteSize(100, 8640, 1080)).toBeCloseTo(49 * 2.5, 6);
  });

  it('falls back to 1x when the recording height is invalid', async () => {
    const { resolveCursorSpriteSize } = await loadRenderer();

    expect(resolveCursorSpriteSize(100, 1080, 0)).toBeCloseTo(49, 6);
  });

  it('scales linearly with the configured percentage', async () => {
    const { resolveCursorSpriteSize } = await loadRenderer();

    expect(resolveCursorSpriteSize(50, 1080, 1080)).toBeCloseTo(24.5, 6);
    expect(resolveCursorSpriteSize(250, 1080, 1080)).toBeCloseTo(122.5, 6);
  });
});

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
