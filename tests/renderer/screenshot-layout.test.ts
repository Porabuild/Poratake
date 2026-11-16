import { describe, expect, it } from 'vitest';
import { computeScreenshotLayout } from '@/renderer/utils/screenshot-layout';
import type { ImageLayerEdge } from '@/types/editor';

const base = {
  nativeWidth: 200,
  nativeHeight: 160,
  imageWidth: 100,
  imageHeight: 80,
  crop: { left: 0, top: 0, right: 0, bottom: 0 },
  padding: 0,
  inset: 4,
  spacing: 0,
  windowFrame: 'none' as const,
  aspectRatio: 'auto' as const,
};

describe('screenshot layout', () => {
  it.each([
    ['left', 475, 176, 267, 8],
    ['right', 475, 176, 8, 8],
    ['top', 216, 323, 8, 155],
    ['bottom', 216, 323, 8, 8],
  ] as [ImageLayerEdge, number, number, number, number][])(
    'uses native rounding for an inset attachment on the %s',
    (edge, width, height, offsetX, offsetY) => {
      const result = computeScreenshotLayout({
        ...base,
        extraLayers: [
          {
            id: 'attachment',
            base64: '',
            naturalWidth: 40,
            naturalHeight: 22,
            edge,
          },
        ],
      });
      expect(result.scale).toBe(2);
      expect([result.canvasWidth, result.canvasHeight]).toEqual([
        width,
        height,
      ]);
      expect([result.contentOffsetX, result.contentOffsetY]).toEqual([
        offsetX,
        offsetY,
      ]);
    }
  );

  it.each([
    ['left', 292, 136, 144, 8],
    ['right', 292, 136, 8, 8],
    ['top', 156, 292, 8, 164],
    ['bottom', 156, 292, 8, 8],
  ] as [ImageLayerEdge, number, number, number, number][])(
    'balances the primary before laying out the inset attachment on the %s',
    (edge, width, height, offsetX, offsetY) => {
      const result = computeScreenshotLayout({
        ...base,
        imageWidth: 200,
        imageHeight: 160,
        crop: { left: 30, top: 20, right: 30, bottom: 20 },
        inset: 8,
        extraLayers: [
          {
            id: 'attachment',
            base64: '',
            naturalWidth: 100,
            naturalHeight: 100,
            edge,
          },
        ],
      });
      expect([result.canvasWidth, result.canvasHeight]).toEqual([
        width,
        height,
      ]);
      expect([result.contentOffsetX, result.contentOffsetY]).toEqual([
        offsetX,
        offsetY,
      ]);
    }
  );

  it('centers the image on a rounded widescreen canvas', () => {
    const result = computeScreenshotLayout({
      ...base,
      inset: 0,
      aspectRatio: '16:9',
      extraLayers: [],
    });
    expect([result.canvasWidth, result.canvasHeight]).toEqual([284, 160]);
    expect([result.contentOffsetX, result.contentOffsetY]).toEqual([42, 0]);
  });

  it('positions cropped content inside a framed square composition', () => {
    const result = computeScreenshotLayout({
      ...base,
      crop: { left: 10, top: 5, right: 20, bottom: 15 },
      padding: 5,
      windowFrame: 'macos-light',
      aspectRatio: '1:1',
      extraLayers: [],
    });
    expect([result.canvasWidth, result.canvasHeight]).toEqual([232, 232]);
    expect([result.contentWidth, result.contentHeight]).toEqual([170, 140]);
    expect([result.contentOffsetX, result.contentOffsetY]).toEqual([31, 74]);
  });
});
