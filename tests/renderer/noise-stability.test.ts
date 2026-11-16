import { afterEach, describe, expect, it, vi } from 'vitest';
import { renderNoise } from '@/renderer/utils/noise';

afterEach(() => vi.unstubAllGlobals());

describe('wallpaper noise', () => {
  it('produces the same varied pixels for repeated renders', () => {
    const frames: Uint8ClampedArray[] = [];
    class FakeCanvas {
      getContext() {
        return {
          createImageData: (width: number, height: number) => ({
            data: new Uint8ClampedArray(width * height * 4),
          }),
          putImageData: (image: { data: Uint8ClampedArray }) => {
            frames.push(image.data);
          },
        };
      }
    }
    vi.stubGlobal('OffscreenCanvas', FakeCanvas);
    const context = {
      save: vi.fn(),
      restore: vi.fn(),
      drawImage: vi.fn(),
      globalAlpha: 1,
      globalCompositeOperation: 'source-over',
    } as unknown as CanvasRenderingContext2D;

    renderNoise(context, 16, 16, 50);
    renderNoise(context, 16, 16, 50);

    expect(frames).toHaveLength(2);
    expect(frames[0]).toEqual(frames[1]);
    expect(
      new Set(frames[0].filter((_, index) => index % 4 === 0)).size
    ).toBeGreaterThan(100);
  });
});
