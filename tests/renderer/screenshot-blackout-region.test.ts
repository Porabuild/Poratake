import { describe, expect, it, vi } from 'vitest';
import { blackoutRegion } from '@/renderer/utils/redact';

describe('screenshot blackout pixel coverage', () => {
  it.each([
    [8.8, 3.8, 12.3, 12.3, 8, 3, 14, 14],
    [8.5, 3.25, 12.3, 12.4, 8, 3, 13, 13],
    [-2.5, 3.8, 12.3, 12.3, 0, 3, 10, 14],
    [8.8, -2.5, 12.3, 12.3, 8, 0, 14, 10],
  ])(
    'covers all touched pixels for %s, %s, %s, %s',
    (x, y, width, height, left, top, pixelWidth, pixelHeight) => {
      const context = {
        fillStyle: '',
        fillRect: vi.fn(),
      } as unknown as CanvasRenderingContext2D;

      blackoutRegion(context, x, y, width, height);

      expect(context.fillStyle).toBe('#000000');
      expect(context.fillRect).toHaveBeenCalledExactlyOnceWith(
        left,
        top,
        pixelWidth,
        pixelHeight
      );
    }
  );

  it.each([
    [8.8, 3.8, 0, 12.3],
    [8.8, 3.8, 12.3, 0],
    [-5, 3, 2, 4],
    [3, -5, 4, 2],
  ])('skips empty or fully clipped rectangles', (x, y, width, height) => {
    const context = {
      fillRect: vi.fn(),
    } as unknown as CanvasRenderingContext2D;

    blackoutRegion(context, x, y, width, height);

    expect(context.fillRect).not.toHaveBeenCalled();
  });
});
