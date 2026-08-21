import { hexToRgb } from './color';
export interface ContentBounds {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

function rgbToHex(r: number, g: number, b: number): string {
  return (
    '#' +
    [r, g, b]
      .map(x => {
        const hex = x.toString(16);
        return hex.length === 1 ? '0' + hex : hex;
      })
      .join('')
  );
}

export function detectDominantEdgeColor(
  image: HTMLImageElement
): string | null {
  if (!image || !image.complete || image.naturalWidth === 0) {
    return null;
  }

  const canvas = document.createElement('canvas');
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) return null;

  const width = image.naturalWidth;
  const height = image.naturalHeight;

  canvas.width = width;
  canvas.height = height;

  ctx.drawImage(image, 0, 0);

  const colorCounts = new Map<string, number>();

  const topEdge = ctx.getImageData(0, 0, width, 1);
  processImageData(topEdge, colorCounts);

  const bottomEdge = ctx.getImageData(0, height - 1, width, 1);
  processImageData(bottomEdge, colorCounts);

  const leftEdge = ctx.getImageData(0, 1, 1, height - 2);
  processImageData(leftEdge, colorCounts);

  const rightEdge = ctx.getImageData(width - 1, 1, 1, height - 2);
  processImageData(rightEdge, colorCounts);

  let dominantColor: string | null = null;
  let maxCount = 0;

  colorCounts.forEach((count, color) => {
    if (count > maxCount) {
      maxCount = count;
      dominantColor = color;
    }
  });

  return dominantColor;
}

function processImageData(
  imageData: ImageData,
  colorCounts: Map<string, number>
): void {
  const data = imageData.data;

  for (let i = 0; i < data.length; i += 4) {
    const r = data[i];
    const g = data[i + 1];
    const b = data[i + 2];
    const a = data[i + 3];

    if (a === 0) continue;

    const hex = rgbToHex(r, g, b);
    const currentCount = colorCounts.get(hex) || 0;
    colorCounts.set(hex, currentCount + 1);
  }
}

const COLOR_SIMILARITY_THRESHOLD = 30;

const BALANCE_CROP_BUFFER = 10;

function colorsAreSimilar(
  r1: number,
  g1: number,
  b1: number,
  r2: number,
  g2: number,
  b2: number,
  threshold: number = COLOR_SIMILARITY_THRESHOLD
): boolean {
  return (
    Math.abs(r1 - r2) <= threshold &&
    Math.abs(g1 - g2) <= threshold &&
    Math.abs(b1 - b2) <= threshold
  );
}

export function detectContentBounds(
  image: HTMLImageElement,
  backgroundColor: string
): ContentBounds | null {
  if (!image || !image.complete || image.naturalWidth === 0) {
    return null;
  }

  const bgRgb = hexToRgb(backgroundColor);
  if (!bgRgb) return null;

  const canvas = document.createElement('canvas');
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) return null;

  const width = image.naturalWidth;
  const height = image.naturalHeight;

  canvas.width = width;
  canvas.height = height;
  ctx.drawImage(image, 0, 0);

  const imageData = ctx.getImageData(0, 0, width, height);
  const data = imageData.data;

  const pixelDiffersFromBg = (x: number, y: number): boolean => {
    const idx = (y * width + x) * 4;
    const r = data[idx];
    const g = data[idx + 1];
    const b = data[idx + 2];
    const a = data[idx + 3];

    if (a < 128) return false;

    return !colorsAreSimilar(r, g, b, bgRgb.r, bgRgb.g, bgRgb.b);
  };

  const sampleStep = width > 1000 || height > 1000 ? 2 : 1;

  const rowHasContent = (y: number): boolean => {
    for (let x = 0; x < width; x += sampleStep) {
      if (pixelDiffersFromBg(x, y)) return true;
    }
    return false;
  };

  const colHasContent = (x: number): boolean => {
    for (let y = 0; y < height; y += sampleStep) {
      if (pixelDiffersFromBg(x, y)) return true;
    }
    return false;
  };

  let top = 0;
  let foundTop = false;
  for (let y = 0; y < height; y++) {
    if (rowHasContent(y)) {
      top = y;
      foundTop = true;
      break;
    }
  }

  let bottom = 0;
  let foundBottom = false;
  for (let y = height - 1; y >= 0; y--) {
    if (rowHasContent(y)) {
      bottom = height - 1 - y;
      foundBottom = true;
      break;
    }
  }

  let left = 0;
  let foundLeft = false;
  for (let x = 0; x < width; x++) {
    if (colHasContent(x)) {
      left = x;
      foundLeft = true;
      break;
    }
  }

  let right = 0;
  let foundRight = false;
  for (let x = width - 1; x >= 0; x--) {
    if (colHasContent(x)) {
      right = width - 1 - x;
      foundRight = true;
      break;
    }
  }

  if (!foundTop && !foundBottom && !foundLeft && !foundRight) {
    return { top: 0, right: 0, bottom: 0, left: 0 };
  }

  return {
    top: Math.max(0, top - BALANCE_CROP_BUFFER),
    right: Math.max(0, right - BALANCE_CROP_BUFFER),
    bottom: Math.max(0, bottom - BALANCE_CROP_BUFFER),
    left: Math.max(0, left - BALANCE_CROP_BUFFER),
  };
}

export type BalanceCrop = ContentBounds;

export function sampleDominantInsetColor(
  image: HTMLImageElement,
  balanceCrop: BalanceCrop = { left: 0, top: 0, right: 0, bottom: 0 }
): string | null {
  const canvas = document.createElement('canvas');
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) return null;

  const srcX = balanceCrop.left;
  const srcY = balanceCrop.top;
  const srcW = image.naturalWidth - balanceCrop.left - balanceCrop.right;
  const srcH = image.naturalHeight - balanceCrop.top - balanceCrop.bottom;

  if (srcW <= 0 || srcH <= 0) return null;

  canvas.width = srcW;
  canvas.height = srcH;
  ctx.drawImage(image, srcX, srcY, srcW, srcH, 0, 0, srcW, srcH);

  const colorCounts = new Map<string, number>();
  processImageData(ctx.getImageData(0, 0, srcW, 1), colorCounts);
  processImageData(ctx.getImageData(0, srcH - 1, srcW, 1), colorCounts);
  if (srcH > 2) {
    processImageData(ctx.getImageData(0, 1, 1, srcH - 2), colorCounts);
    processImageData(ctx.getImageData(srcW - 1, 1, 1, srcH - 2), colorCounts);
  }

  let dominant: string | null = null;
  let max = 0;
  colorCounts.forEach((count, color) => {
    if (count > max) {
      max = count;
      dominant = color;
    }
  });

  return dominant;
}
