import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  renderImageToCanvas,
  renderImageWithInset,
  renderWallpaperComposite,
} from '@/renderer/utils/wallpaper-render';
import { WINDOW_FRAME_TITLE_BAR_HEIGHT } from '@/renderer/utils/window-frame';

interface CanvasStub {
  width: number;
  height: number;
  getContext: () => Record<string, unknown>;
}

const createContextStub = () => ({
  save: vi.fn(),
  restore: vi.fn(),
  scale: vi.fn(),
  beginPath: vi.fn(),
  closePath: vi.fn(),
  clip: vi.fn(),
  fill: vi.fn(),
  stroke: vi.fn(),
  rect: vi.fn(),
  roundRect: vi.fn(),
  fillRect: vi.fn(),
  strokeRect: vi.fn(),
  moveTo: vi.fn(),
  lineTo: vi.fn(),
  arc: vi.fn(),
  drawImage: vi.fn(),
  createLinearGradient: vi.fn(() => ({ addColorStop: vi.fn() })),
  fillStyle: '' as string,
  globalCompositeOperation: 'source-over' as string,
  shadowColor: '' as string,
  shadowBlur: 0,
  shadowOffsetX: 0,
  shadowOffsetY: 0,
});

let ctx: ReturnType<typeof createContextStub>;
let canvas: CanvasStub;

const createImageStub = (naturalWidth: number, naturalHeight: number) =>
  ({
    naturalWidth,
    naturalHeight,
    width: naturalWidth,
    height: naturalHeight,
  }) as unknown as HTMLImageElement;

beforeEach(() => {
  ctx = createContextStub();
  canvas = {
    width: 0,
    height: 0,
    getContext: () => ctx as unknown as Record<string, unknown>,
  };

  vi.stubGlobal('document', { createElement: () => canvas });
  vi.stubGlobal('window', { devicePixelRatio: 2 });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('renderWallpaperComposite', () => {
  it('scales padding by the display scale factor', async () => {
    const result = await renderWallpaperComposite(createImageStub(1600, 1000), {
      gradient: null,
      padding: 50,
      corners: 12,
      shadow: 0,
    });

    // devicePixelRatio 2 => native scale 2 => 50 CSS px of padding per side.
    expect(result.width).toBe(1600 + 200);
    expect(result.height).toBe(1000 + 200);
  });

  it('renders the image with scaled corner radius and no background', async () => {
    await renderWallpaperComposite(createImageStub(800, 600), {
      gradient: null,
      padding: 10,
      corners: 8,
      shadow: 0,
    });

    expect(ctx.createLinearGradient).not.toHaveBeenCalled();
    expect(ctx.fillRect).not.toHaveBeenCalled();
    expect(ctx.roundRect).toHaveBeenCalledWith(20, 20, 800, 600, 16);
    expect(ctx.drawImage).toHaveBeenCalledWith(
      expect.anything(),
      0,
      0,
      800,
      600,
      20,
      20,
      800,
      600
    );
  });

  it('paints a gradient background across the full canvas', async () => {
    await renderWallpaperComposite(createImageStub(400, 400), {
      gradient: { id: 'g1', colors: ['#000', '#fff'], angle: 90 },
      padding: 25,
      corners: 0,
      shadow: 0,
    });

    expect(ctx.createLinearGradient).toHaveBeenCalled();
    expect(ctx.fillRect.mock.calls[0]).toEqual([
      expect.closeTo(0),
      expect.closeTo(0),
      500,
      500,
    ]);
  });

  it('reserves room for the title bar when a window frame is set', async () => {
    const result = await renderWallpaperComposite(createImageStub(800, 600), {
      gradient: null,
      padding: 0,
      corners: 0,
      shadow: 0,
      windowFrame: { style: 'macos-dark' },
    });

    expect(result.height).toBe(600 + WINDOW_FRAME_TITLE_BAR_HEIGHT * 2);
    expect(result.width).toBe(800);
  });

  it('fits all three macOS title-bar lights inside a narrow frame', async () => {
    vi.stubGlobal('window', { devicePixelRatio: 1 });
    await renderWallpaperComposite(createImageStub(40, 70), {
      gradient: null,
      padding: 0,
      corners: 0,
      shadow: 0,
      windowFrame: { style: 'macos-dark' },
    });

    expect(ctx.arc).toHaveBeenCalledTimes(3);
    for (const [centerX, , radius] of ctx.arc.mock.calls) {
      expect(centerX - radius).toBeGreaterThan(0);
      expect(centerX + radius).toBeLessThan(40);
    }
  });

  it('renders at 1x when the display is not scaled', async () => {
    vi.stubGlobal('window', { devicePixelRatio: 1 });

    const result = await renderWallpaperComposite(createImageStub(800, 600), {
      gradient: null,
      padding: 32,
      corners: 0,
      shadow: 0,
    });

    expect(result.width).toBe(800 + 64);
    expect(result.height).toBe(600 + 64);
  });

  it('scales odd-sized captures at fractional display scales', async () => {
    vi.stubGlobal('window', { devicePixelRatio: 1.25 });

    const result = await renderWallpaperComposite(createImageStub(641, 361), {
      gradient: null,
      padding: 50,
      corners: 12,
      shadow: 0,
    });

    expect(result.width).toBe(767);
    expect(result.height).toBe(487);
    expect(ctx.roundRect).toHaveBeenCalledWith(63, 63, 641, 361, 15);
  });

  it('masks shadowed rounded images once offscreen without filling white', () => {
    const offCtx = createContextStub();
    const offCanvas: CanvasStub = {
      width: 0,
      height: 0,
      getContext: () => offCtx as unknown as Record<string, unknown>,
    };
    vi.stubGlobal('OffscreenCanvas', undefined);
    vi.stubGlobal('document', { createElement: () => offCanvas });

    renderImageToCanvas(
      ctx as unknown as CanvasRenderingContext2D,
      createImageStub(200, 120),
      40,
      40,
      200,
      120,
      49,
      73
    );

    expect(ctx.fill).not.toHaveBeenCalled();
    expect(ctx.fillStyle).not.toBe('white');
    expect(offCtx.globalCompositeOperation).toBe('destination-in');
    expect(offCtx.roundRect).toHaveBeenCalledWith(0, 0, 200, 120, 49);
    expect(offCtx.fill).toHaveBeenCalledTimes(1);
    expect(ctx.shadowBlur).toBeGreaterThan(0);
    expect(ctx.drawImage).toHaveBeenCalledWith(offCanvas, 40, 40, 200, 120);
  });

  it('masks rounded inset images once offscreen when corner radius exceeds inset', () => {
    const offCtx = createContextStub();
    const offCanvas: CanvasStub = {
      width: 0,
      height: 0,
      getContext: () => offCtx as unknown as Record<string, unknown>,
    };
    vi.stubGlobal('OffscreenCanvas', undefined);
    vi.stubGlobal('document', { createElement: () => offCanvas });

    renderImageWithInset(
      ctx as unknown as CanvasRenderingContext2D,
      createImageStub(200, 120),
      30,
      30,
      212,
      132,
      40,
      50,
      6,
      '#ffffff'
    );

    expect(ctx.fill).not.toHaveBeenCalled();
    expect(offCtx.fillRect).toHaveBeenCalledWith(0, 0, 212, 132);
    expect(offCtx.globalCompositeOperation).toBe('destination-in');
    expect(offCtx.roundRect).toHaveBeenCalledWith(0, 0, 212, 132, 40);
    expect(ctx.drawImage).toHaveBeenCalledWith(offCanvas, 30, 30, 212, 132);
  });
});
