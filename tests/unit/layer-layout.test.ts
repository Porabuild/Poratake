import { describe, it, expect } from 'vitest';
import { computeLayerLayout } from '@/renderer/utils/layer-layout';
import type { ImageLayer } from '@/types/editor';

const makeLayer = (
  id: string,
  edge: ImageLayer['edge'],
  w: number,
  h: number
): ImageLayer => ({
  id,
  edge,
  base64: '',
  naturalWidth: w,
  naturalHeight: h,
});

describe('computeLayerLayout', () => {
  it('returns a single primary rect when there are no extras', () => {
    const result = computeLayerLayout({
      primaryWidth: 100,
      primaryHeight: 50,
      extraLayers: [],
      spacing: 0,
    });
    expect(result.rects).toHaveLength(1);
    expect(result.rects[0]).toMatchObject({
      id: 'primary',
      x: 0,
      y: 0,
      width: 100,
      height: 50,
    });
    expect(result.width).toBe(100);
    expect(result.height).toBe(50);
  });

  it('places a right-edge layer to the right of the primary with spacing', () => {
    const result = computeLayerLayout({
      primaryWidth: 100,
      primaryHeight: 50,
      extraLayers: [makeLayer('a', 'right', 80, 40)],
      spacing: 10,
    });
    const aRect = result.rects.find(r => r.id === 'a');
    expect(aRect?.x).toBe(110);
    expect(aRect?.y).toBe(0);
    expect(aRect?.height).toBe(50);
    expect(result.width).toBe(110 + (aRect?.width ?? 0));
    expect(result.height).toBe(50);
  });

  it('shifts the bounding box so a left-edge layer starts at x=0', () => {
    const result = computeLayerLayout({
      primaryWidth: 100,
      primaryHeight: 50,
      extraLayers: [makeLayer('a', 'left', 80, 40)],
      spacing: 10,
    });
    const aRect = result.rects.find(r => r.id === 'a');
    const primaryRect = result.rects.find(r => r.id === 'primary');
    expect(aRect?.x).toBe(0);
    expect((primaryRect?.x ?? 0) > 0).toBe(true);
    expect(result.width).toBe((aRect?.width ?? 0) + 10 + 100);
  });

  it('stacks bottom-edge layers vertically with spacing', () => {
    const result = computeLayerLayout({
      primaryWidth: 100,
      primaryHeight: 50,
      extraLayers: [makeLayer('a', 'bottom', 80, 40)],
      spacing: 8,
    });
    const aRect = result.rects.find(r => r.id === 'a');
    expect(aRect?.x).toBe(0);
    expect(aRect?.y).toBe(58);
    expect(aRect?.width).toBe(100);
  });

  it('chains layers each relative to the previous one', () => {
    const result = computeLayerLayout({
      primaryWidth: 100,
      primaryHeight: 50,
      extraLayers: [
        makeLayer('a', 'right', 80, 40),
        makeLayer('b', 'right', 80, 40),
      ],
      spacing: 10,
    });
    const aRect = result.rects.find(r => r.id === 'a');
    const bRect = result.rects.find(r => r.id === 'b');
    expect(aRect && bRect).toBeTruthy();
    expect(bRect!.x).toBe(aRect!.x + aRect!.width + 10);
  });

  it('applies spacing between primary and every edge when layers are on different edges', () => {
    const result = computeLayerLayout({
      primaryWidth: 100,
      primaryHeight: 50,
      extraLayers: [
        makeLayer('r', 'right', 80, 50),
        makeLayer('b', 'bottom', 100, 40),
        makeLayer('t', 'top', 100, 40),
      ],
      spacing: 10,
    });
    const primary = result.rects.find(r => r.id === 'primary')!;
    const rRect = result.rects.find(r => r.id === 'r')!;
    const bRect = result.rects.find(r => r.id === 'b')!;
    const tRect = result.rects.find(r => r.id === 't')!;

    expect(rRect.x - (primary.x + primary.width)).toBe(10);
    expect(bRect.y - (primary.y + primary.height)).toBe(10);
    expect(primary.y - (tRect.y + tRect.height)).toBe(10);
  });

  it('stacks multiple layers on the same edge with spacing between each', () => {
    const result = computeLayerLayout({
      primaryWidth: 100,
      primaryHeight: 50,
      extraLayers: [
        makeLayer('a', 'right', 80, 50),
        makeLayer('b', 'right', 60, 50),
        makeLayer('c', 'right', 40, 50),
      ],
      spacing: 10,
    });
    const primary = result.rects.find(r => r.id === 'primary')!;
    const aRect = result.rects.find(r => r.id === 'a')!;
    const bRect = result.rects.find(r => r.id === 'b')!;
    const cRect = result.rects.find(r => r.id === 'c')!;

    expect(aRect.x - (primary.x + primary.width)).toBe(10);
    expect(bRect.x - (aRect.x + aRect.width)).toBe(10);
    expect(cRect.x - (bRect.x + bRect.width)).toBe(10);
  });
});
