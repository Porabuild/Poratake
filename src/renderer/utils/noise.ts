type NoiseContext =
  | CanvasRenderingContext2D
  | OffscreenCanvasRenderingContext2D;

export function renderNoise(
  ctx: NoiseContext,
  width: number,
  height: number,
  noiseValue: number
): void {
  if (noiseValue === 0) return;

  const noiseOpacity = (noiseValue / 100) * 0.3;

  const noiseCanvas =
    typeof OffscreenCanvas !== 'undefined'
      ? new OffscreenCanvas(width, height)
      : (() => {
          const canvas = document.createElement('canvas');
          canvas.width = width;
          canvas.height = height;
          return canvas;
        })();

  const noiseCtx = noiseCanvas.getContext('2d') as NoiseContext | null;
  if (noiseCtx) {
    const imageData = noiseCtx.createImageData(width, height);
    const data = imageData.data;
    let state = 0x9e3779b9;
    for (let i = 0; i < data.length; i += 4) {
      state ^= state << 13;
      state ^= state >>> 17;
      state ^= state << 5;
      const value = state >>> 24;
      data[i] = value;
      data[i + 1] = value;
      data[i + 2] = value;
      data[i + 3] = 255;
    }
    noiseCtx.putImageData(imageData, 0, 0);
  }

  ctx.save();
  ctx.globalAlpha = noiseOpacity;
  ctx.globalCompositeOperation = 'overlay';
  ctx.drawImage(noiseCanvas, 0, 0, width, height);
  ctx.restore();
}
