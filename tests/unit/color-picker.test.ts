import { describe, expect, it } from 'vitest';
import {
  fillLoupePixels,
  sampleHex,
  type SampleSource,
} from '@/renderer/components/area-overlay/color-picker';

const GRID = 15;
const HALF = Math.floor(GRID / 2);

function createSource(width: number, height: number): SampleSource {
  const data = new Uint8ClampedArray(width * height * 4);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const index = (y * width + x) * 4;
      data[index] = x;
      data[index + 1] = y;
      data[index + 2] = x + y;
      data[index + 3] = 255;
    }
  }
  return { data, width, height };
}

describe('color picker pixel sampling', () => {
  it.each([
    [0, 0],
    [19, 0],
    [0, 19],
    [19, 19],
    [10, 0],
    [0, 10],
    [19, 10],
    [10, 19],
    [10, 10],
  ])('keeps the sampled pixel under the center marker at %i,%i', (x, y) => {
    const source = createSource(20, 20);
    const loupe = new Uint8ClampedArray(GRID * GRID * 4);

    fillLoupePixels(source, x, y, loupe);

    const center = (HALF * GRID + HALF) * 4;
    expect(Array.from(loupe.slice(center, center + 3))).toEqual([x, y, x + y]);
    expect(sampleHex(source, x, y)).toBe(
      `#${x.toString(16).padStart(2, '0')}${y
        .toString(16)
        .padStart(2, '0')}${(x + y).toString(16).padStart(2, '0')}`
    );
  });
});
