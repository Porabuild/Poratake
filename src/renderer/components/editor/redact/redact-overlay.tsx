import {
  useEffect,
  useRef,
  forwardRef,
  useImperativeHandle,
  useMemo,
} from 'react';
import type {
  Annotation,
  RedactAnnotation,
  RedactIntensity,
} from '@/types/editor';
import type { BalanceCrop } from '@/renderer/utils/color-detection';

export interface RedactOverlayProps {
  image: HTMLImageElement | null;
  imageWidth: number;
  imageHeight: number;
  canvasWidth: number;
  canvasHeight: number;
  offsetX: number;
  offsetY: number;
  balanceCrop?: BalanceCrop;
  annotations: Annotation[];
  currentAnnotation: Annotation | null;
}

export interface RedactOverlayHandle {
  getCanvas: () => HTMLCanvasElement | null;
}

const INTENSITY_MAP: Record<
  RedactIntensity,
  { pixelSize: number; blurRadius: number }
> = {
  1: { pixelSize: 4, blurRadius: 4 },
  2: { pixelSize: 8, blurRadius: 8 },
  3: { pixelSize: 12, blurRadius: 12 },
  4: { pixelSize: 16, blurRadius: 16 },
  5: { pixelSize: 20, blurRadius: 20 },
  6: { pixelSize: 24, blurRadius: 24 },
  7: { pixelSize: 28, blurRadius: 30 },
  8: { pixelSize: 32, blurRadius: 36 },
  9: { pixelSize: 40, blurRadius: 44 },
  10: { pixelSize: 48, blurRadius: 52 },
};

const pixelateRegion = (
  ctx: CanvasRenderingContext2D,
  sourceCanvas: HTMLCanvasElement,
  x: number,
  y: number,
  width: number,
  height: number,
  blockSize: number,
  scale: number
) => {
  const scaledX = Math.floor(x * scale);
  const scaledY = Math.floor(y * scale);
  const scaledWidth = Math.ceil(width * scale);
  const scaledHeight = Math.ceil(height * scale);
  const scaledBlockSize = Math.max(1, Math.round(blockSize * scale));

  const canvasWidth = sourceCanvas.width;
  const canvasHeight = sourceCanvas.height;

  const clampedX = Math.max(0, scaledX);
  const clampedY = Math.max(0, scaledY);
  const clampedWidth = Math.min(
    scaledWidth - (clampedX - scaledX),
    canvasWidth - clampedX
  );
  const clampedHeight = Math.min(
    scaledHeight - (clampedY - scaledY),
    canvasHeight - clampedY
  );

  if (clampedWidth <= 0 || clampedHeight <= 0) return;

  const sourceCtx = sourceCanvas.getContext('2d');
  if (!sourceCtx) return;

  const imageData = sourceCtx.getImageData(
    clampedX,
    clampedY,
    clampedWidth,
    clampedHeight
  );
  const data = imageData.data;

  for (let py = 0; py < clampedHeight; py += scaledBlockSize) {
    for (let px = 0; px < clampedWidth; px += scaledBlockSize) {
      const sampleX = Math.min(
        px + Math.floor(scaledBlockSize / 2),
        clampedWidth - 1
      );
      const sampleY = Math.min(
        py + Math.floor(scaledBlockSize / 2),
        clampedHeight - 1
      );
      const sampleIdx = (sampleY * clampedWidth + sampleX) * 4;

      const r = data[sampleIdx];
      const g = data[sampleIdx + 1];
      const b = data[sampleIdx + 2];
      const a = data[sampleIdx + 3];

      for (let by = 0; by < scaledBlockSize && py + by < clampedHeight; by++) {
        for (let bx = 0; bx < scaledBlockSize && px + bx < clampedWidth; bx++) {
          const idx = ((py + by) * clampedWidth + (px + bx)) * 4;
          data[idx] = r;
          data[idx + 1] = g;
          data[idx + 2] = b;
          data[idx + 3] = a;
        }
      }
    }
  }

  ctx.putImageData(imageData, clampedX, clampedY);
};

const blurRegion = (
  ctx: CanvasRenderingContext2D,
  sourceCanvas: HTMLCanvasElement,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number,
  scale: number
) => {
  const scaledX = Math.floor(x * scale);
  const scaledY = Math.floor(y * scale);
  const scaledWidth = Math.ceil(width * scale);
  const scaledHeight = Math.ceil(height * scale);
  const scaledRadius = Math.max(1, Math.round(radius * scale));

  const canvasWidth = sourceCanvas.width;
  const canvasHeight = sourceCanvas.height;

  const clampedX = Math.max(0, scaledX);
  const clampedY = Math.max(0, scaledY);
  const clampedWidth = Math.min(
    scaledWidth - (clampedX - scaledX),
    canvasWidth - clampedX
  );
  const clampedHeight = Math.min(
    scaledHeight - (clampedY - scaledY),
    canvasHeight - clampedY
  );

  if (clampedWidth <= 0 || clampedHeight <= 0) return;

  const tempCanvas = document.createElement('canvas');
  const padding = scaledRadius * 2;
  tempCanvas.width = clampedWidth + padding * 2;
  tempCanvas.height = clampedHeight + padding * 2;
  const tempCtx = tempCanvas.getContext('2d');

  if (!tempCtx) return;

  const sourceX = Math.max(0, clampedX - padding);
  const sourceY = Math.max(0, clampedY - padding);
  const sourceWidth = Math.min(
    clampedWidth + padding * 2,
    canvasWidth - sourceX
  );
  const sourceHeight = Math.min(
    clampedHeight + padding * 2,
    canvasHeight - sourceY
  );

  tempCtx.drawImage(
    sourceCanvas,
    sourceX,
    sourceY,
    sourceWidth,
    sourceHeight,
    0,
    0,
    sourceWidth,
    sourceHeight
  );

  tempCtx.filter = `blur(${scaledRadius}px)`;
  tempCtx.drawImage(tempCanvas, 0, 0);
  tempCtx.filter = 'none';

  const offsetX = clampedX - sourceX;
  const offsetY = clampedY - sourceY;

  ctx.drawImage(
    tempCanvas,
    offsetX,
    offsetY,
    clampedWidth,
    clampedHeight,
    clampedX,
    clampedY,
    clampedWidth,
    clampedHeight
  );
};

const blackoutRegion = (
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  scale: number
) => {
  const scaledX = Math.floor(x * scale);
  const scaledY = Math.floor(y * scale);
  const scaledWidth = Math.ceil(width * scale);
  const scaledHeight = Math.ceil(height * scale);

  const clampedX = Math.max(0, scaledX);
  const clampedY = Math.max(0, scaledY);
  const clampedWidth = scaledWidth - (clampedX - scaledX);
  const clampedHeight = scaledHeight - (clampedY - scaledY);

  if (clampedWidth <= 0 || clampedHeight <= 0) return;

  ctx.fillStyle = '#000000';
  ctx.fillRect(clampedX, clampedY, clampedWidth, clampedHeight);
};

const applyRedactEffect = (
  ctx: CanvasRenderingContext2D,
  sourceCanvas: HTMLCanvasElement,
  redact: RedactAnnotation,
  offsetX: number,
  offsetY: number,
  scale: number
) => {
  const x = redact.x + offsetX;
  const y = redact.y + offsetY;
  const w = redact.width;
  const h = redact.height;

  const rectX = w < 0 ? x + w : x;
  const rectY = h < 0 ? y + h : y;
  const rectW = Math.abs(w);
  const rectH = Math.abs(h);

  if (rectW <= 0 || rectH <= 0) return;

  const intensity = INTENSITY_MAP[redact.intensity] || INTENSITY_MAP[5];

  switch (redact.style) {
    case 'pixelate':
      pixelateRegion(
        ctx,
        sourceCanvas,
        rectX,
        rectY,
        rectW,
        rectH,
        intensity.pixelSize,
        scale
      );
      break;
    case 'blur':
      blurRegion(
        ctx,
        sourceCanvas,
        rectX,
        rectY,
        rectW,
        rectH,
        intensity.blurRadius,
        scale
      );
      break;
    case 'blackout':
      blackoutRegion(ctx, rectX, rectY, rectW, rectH, scale);
      break;
  }
};

const RedactOverlay = forwardRef<RedactOverlayHandle, RedactOverlayProps>(
  (
    {
      image,
      imageWidth,
      imageHeight,
      canvasWidth,
      canvasHeight,
      offsetX,
      offsetY,
      balanceCrop,
      annotations,
      currentAnnotation,
    },
    ref
  ) => {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const sourceCanvasRef = useRef<HTMLCanvasElement | null>(null);
    const pixelRatio = window.devicePixelRatio || 2;

    useImperativeHandle(ref, () => ({
      getCanvas: () => canvasRef.current,
    }));

    const redactAnnotations = useMemo(
      () => [
        ...annotations.filter(
          (ann): ann is RedactAnnotation => ann.type === 'redact'
        ),
        ...(currentAnnotation?.type === 'redact' ? [currentAnnotation] : []),
      ],
      [annotations, currentAnnotation]
    );

    useEffect(() => {
      const canvas = canvasRef.current;
      if (!canvas || !image) return;

      const ctx = canvas.getContext('2d', {
        alpha: true,
        desynchronized: false,
      });
      if (!ctx) return;

      ctx.clearRect(0, 0, canvas.width, canvas.height);

      if (redactAnnotations.length === 0) return;

      if (!sourceCanvasRef.current) {
        sourceCanvasRef.current = document.createElement('canvas');
      }
      const sourceCanvas = sourceCanvasRef.current;
      sourceCanvas.width = canvas.width;
      sourceCanvas.height = canvas.height;
      const sourceCtx = sourceCanvas.getContext('2d');
      if (!sourceCtx) return;

      sourceCtx.clearRect(0, 0, sourceCanvas.width, sourceCanvas.height);

      const nativeScaleX =
        image.naturalWidth /
        (imageWidth + (balanceCrop?.left ?? 0) + (balanceCrop?.right ?? 0));
      const nativeScaleY =
        image.naturalHeight /
        (imageHeight + (balanceCrop?.top ?? 0) + (balanceCrop?.bottom ?? 0));
      const srcX = balanceCrop ? balanceCrop.left * nativeScaleX : 0;
      const srcY = balanceCrop ? balanceCrop.top * nativeScaleY : 0;
      const srcWidth = imageWidth * nativeScaleX;
      const srcHeight = imageHeight * nativeScaleY;

      sourceCtx.drawImage(
        image,
        srcX,
        srcY,
        srcWidth,
        srcHeight,
        offsetX * pixelRatio,
        offsetY * pixelRatio,
        imageWidth * pixelRatio,
        imageHeight * pixelRatio
      );

      for (const redact of redactAnnotations) {
        applyRedactEffect(
          ctx,
          sourceCanvas,
          redact,
          offsetX,
          offsetY,
          pixelRatio
        );
      }
    }, [
      image,
      imageWidth,
      imageHeight,
      offsetX,
      offsetY,
      pixelRatio,
      redactAnnotations,
      balanceCrop,
    ]);

    if (redactAnnotations.length === 0) {
      return null;
    }

    return (
      <canvas
        ref={canvasRef}
        width={canvasWidth * pixelRatio}
        height={canvasHeight * pixelRatio}
        style={{
          position: 'absolute',
          top: 0,
          left: 0,
          width: canvasWidth,
          height: canvasHeight,
          pointerEvents: 'none',
        }}
      />
    );
  }
);

RedactOverlay.displayName = 'RedactOverlay';

export default RedactOverlay;
