import type { RedactIntensity } from '@/types/editor';

export const REDACT_INTENSITY_MAP: Record<
  RedactIntensity,
  { pixelSize: number; blurRadius: number }
> = {
  1: { pixelSize: 2, blurRadius: 4 },
  2: { pixelSize: 3, blurRadius: 8 },
  3: { pixelSize: 4, blurRadius: 12 },
  4: { pixelSize: 6, blurRadius: 16 },
  5: { pixelSize: 8, blurRadius: 20 },
  6: { pixelSize: 10, blurRadius: 24 },
  7: { pixelSize: 12, blurRadius: 30 },
  8: { pixelSize: 14, blurRadius: 36 },
  9: { pixelSize: 16, blurRadius: 44 },
  10: { pixelSize: 20, blurRadius: 52 },
};

export const pixelateImageData = (
  data: Uint8ClampedArray,
  width: number,
  height: number,
  blockSize: number
): void => {
  const size = Math.max(1, Math.round(blockSize));

  for (let y = 0; y < height; y += size) {
    for (let x = 0; x < width; x += size) {
      const blockWidth = Math.min(size, width - x);
      const blockHeight = Math.min(size, height - y);
      const pixelCount = blockWidth * blockHeight;
      let red = 0;
      let green = 0;
      let blue = 0;
      let alpha = 0;

      for (let blockY = 0; blockY < blockHeight; blockY++) {
        for (let blockX = 0; blockX < blockWidth; blockX++) {
          const index = ((y + blockY) * width + x + blockX) * 4;
          red += data[index];
          green += data[index + 1];
          blue += data[index + 2];
          alpha += data[index + 3];
        }
      }

      const averageRed = Math.round(red / pixelCount);
      const averageGreen = Math.round(green / pixelCount);
      const averageBlue = Math.round(blue / pixelCount);
      const averageAlpha = Math.round(alpha / pixelCount);

      for (let blockY = 0; blockY < blockHeight; blockY++) {
        for (let blockX = 0; blockX < blockWidth; blockX++) {
          const index = ((y + blockY) * width + x + blockX) * 4;
          data[index] = averageRed;
          data[index + 1] = averageGreen;
          data[index + 2] = averageBlue;
          data[index + 3] = averageAlpha;
        }
      }
    }
  }
};
