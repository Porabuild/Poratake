import { describe, expect, it } from 'vitest';
import {
  pixelateImageData,
  REDACT_INTENSITY_MAP,
} from '../../src/renderer/utils/redact';

describe('redact pixelation', () => {
  it('uses compact blocks at the default intensity', () => {
    expect(REDACT_INTENSITY_MAP[5].pixelSize).toBe(8);
  });

  it('averages each block so thin content is not skipped', () => {
    const data = new Uint8ClampedArray([
      0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255,
    ]);

    pixelateImageData(data, 4, 1, 4);

    expect([...data]).toEqual([
      64, 64, 64, 255, 64, 64, 64, 255, 64, 64, 64, 255, 64, 64, 64, 255,
    ]);
  });
});
