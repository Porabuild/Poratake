import { useCallback, useMemo } from 'react';
import type {
  Annotation,
  GradientOption,
  HighlightAnnotation,
  ImageLayer,
  RedactAnnotation,
  RedactIntensity,
  WindowFrameStyle,
} from '@/types/editor';
import type { ScreenshotFormat } from '@/types/settings';
import {
  detectDominantEdgeColor,
  detectContentBounds,
  sampleDominantInsetColor,
  type BalanceCrop,
} from '@/renderer/utils/color-detection';
import { renderNoise } from '@/renderer/utils/noise';
import {
  computeLayerLayout,
  findPrimaryRect,
} from '@/renderer/utils/layer-layout';

const WINDOW_FRAME_TITLE_BAR_HEIGHT = 28;
const WINDOW_FRAME_CORNER_RADIUS = 10;
const TRAFFIC_LIGHT_SIZE = 12;
const TRAFFIC_LIGHT_SPACING = 8;
const TRAFFIC_LIGHT_OFFSET_X = 13;

const TRAFFIC_LIGHT_COLORS = {
  close: '#FF5F57',
  minimize: '#FFBD2E',
  maximize: '#28C840',
};

const WINDOW_FRAME_THEMES = {
  'macos-light': {
    titleBar: '#E8E8E8',
    titleBarBorder: '#D1D1D1',
    content: '#FFFFFF',
    frameBorder: '#C0C0C0',
  },
  'macos-dark': {
    titleBar: '#3A3A3C',
    titleBarBorder: '#2A2A2C',
    content: '#1C1C1E',
    frameBorder: '#4A4A4C',
  },
};

interface UseCanvasExportProps {
  padding: number;
  inset?: number;
  image: HTMLImageElement | null;
  imageWidth: number;
  imageHeight: number;
  cornerRadius: number;
  shadow: number;
  spacing?: number;
  gradient: GradientOption | null;
  backgroundImage: string | null;
  backgroundBlur?: number;
  noise?: number;
  getSvgForExport: (scale: number) => string;
  annotations?: Annotation[];
  windowFrame?: WindowFrameStyle;
  balance?: boolean;
  extraLayers?: ImageLayer[];
  extraLayerImages?: Record<string, HTMLImageElement>;
}

interface UseCanvasExportReturn {
  exportToImage: (format?: ScreenshotFormat) => Promise<string>;
}

const renderGradientToCanvas = (
  ctx: CanvasRenderingContext2D,
  gradient: GradientOption,
  width: number,
  height: number,
  blurRadius: number = 0
) => {
  ctx.save();

  if (blurRadius > 0) {
    ctx.filter = `blur(${blurRadius}px)`;
  }

  const blurExpand = blurRadius * 2;
  const expandedWidth = width + blurExpand * 2;
  const expandedHeight = height + blurExpand * 2;

  const angleRad = ((gradient.angle - 90) * Math.PI) / 180;
  const cos = Math.cos(angleRad);
  const sin = Math.sin(angleRad);

  const halfWidth = width / 2;
  const halfHeight = height / 2;
  const length =
    Math.sqrt(expandedWidth * expandedWidth + expandedHeight * expandedHeight) /
    2;

  const startX = halfWidth - cos * length;
  const startY = halfHeight - sin * length;
  const endX = halfWidth + cos * length;
  const endY = halfHeight + sin * length;

  const grad = ctx.createLinearGradient(startX, startY, endX, endY);
  const colorCount = gradient.colors.length;
  gradient.colors.forEach((color, index) => {
    grad.addColorStop(index / (colorCount - 1), color);
  });

  ctx.fillStyle = grad;
  ctx.fillRect(-blurExpand, -blurExpand, expandedWidth, expandedHeight);
  ctx.restore();
};

const renderBackgroundImageToCanvas = (
  ctx: CanvasRenderingContext2D,
  bgImage: HTMLImageElement,
  width: number,
  height: number,
  blurRadius: number = 0
) => {
  ctx.save();

  if (blurRadius > 0) {
    ctx.filter = `blur(${blurRadius}px)`;
  }

  const blurExpand = blurRadius * 2;
  const expandedWidth = width + blurExpand * 2;
  const expandedHeight = height + blurExpand * 2;

  const imgAspect = bgImage.width / bgImage.height;
  const canvasAspect = expandedWidth / expandedHeight;

  let drawWidth: number;
  let drawHeight: number;
  let drawX: number;
  let drawY: number;

  if (imgAspect > canvasAspect) {
    drawHeight = expandedHeight;
    drawWidth = expandedHeight * imgAspect;
    drawX = (width - drawWidth) / 2;
    drawY = -blurExpand;
  } else {
    drawWidth = expandedWidth;
    drawHeight = expandedWidth / imgAspect;
    drawX = -blurExpand;
    drawY = (height - drawHeight) / 2;
  }

  ctx.drawImage(bgImage, drawX, drawY, drawWidth, drawHeight);
  ctx.restore();
};

const loadImageFromDataUrl = (dataUrl: string): Promise<HTMLImageElement> => {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error('Failed to load background image'));
    img.src = dataUrl;
  });
};

const drawTrafficLights = (
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  scale: number
) => {
  const buttons = [
    TRAFFIC_LIGHT_COLORS.close,
    TRAFFIC_LIGHT_COLORS.minimize,
    TRAFFIC_LIGHT_COLORS.maximize,
  ];

  const scaledSize = TRAFFIC_LIGHT_SIZE * scale;
  const scaledSpacing = TRAFFIC_LIGHT_SPACING * scale;

  buttons.forEach((color, index) => {
    const buttonX = x + index * (scaledSize + scaledSpacing);
    ctx.beginPath();
    ctx.arc(buttonX, y, scaledSize / 2, 0, Math.PI * 2);
    ctx.fillStyle = color;
    ctx.fill();
  });
};

const renderWindowFrame = (
  ctx: CanvasRenderingContext2D,
  image: HTMLImageElement,
  x: number,
  y: number,
  croppedWidth: number,
  croppedHeight: number,
  frameStyle: 'macos-light' | 'macos-dark',
  shadow: number,
  scale: number,
  inset: number = 0,
  insetColor: string | null = null,
  balanceCrop: BalanceCrop = { left: 0, top: 0, right: 0, bottom: 0 }
) => {
  ctx.save();

  const theme = WINDOW_FRAME_THEMES[frameStyle];
  const scaledTitleBarHeight = WINDOW_FRAME_TITLE_BAR_HEIGHT * scale;
  const scaledCornerRadius = WINDOW_FRAME_CORNER_RADIUS * scale;

  const frameWidth = croppedWidth + inset * 2;
  const frameHeight = croppedHeight + scaledTitleBarHeight + inset * 2;

  const shadowBlur = (shadow / 100) * 50 * scale;
  const shadowOpacity = shadow > 0 ? 0.2 + (shadow / 100) * 0.3 : 0;
  const shadowOffsetY = (shadow / 100) * 15 * scale;

  if (shadow > 0) {
    ctx.shadowColor = `rgba(0, 0, 0, ${shadowOpacity})`;
    ctx.shadowBlur = shadowBlur;
    ctx.shadowOffsetX = 0;
    ctx.shadowOffsetY = shadowOffsetY;
  }

  ctx.beginPath();
  ctx.roundRect(x, y, frameWidth, frameHeight, scaledCornerRadius);

  if (shadow > 0) {
    ctx.fillStyle = theme.content;
    ctx.fill();
  }

  ctx.shadowColor = 'transparent';
  ctx.shadowBlur = 0;
  ctx.shadowOffsetY = 0;

  ctx.clip();

  ctx.fillStyle = theme.titleBar;
  ctx.fillRect(x, y, frameWidth, scaledTitleBarHeight);

  ctx.strokeStyle = theme.titleBarBorder;
  ctx.lineWidth = 0.5 * scale;
  ctx.beginPath();
  ctx.moveTo(x, y + scaledTitleBarHeight);
  ctx.lineTo(x + frameWidth, y + scaledTitleBarHeight);
  ctx.stroke();

  const trafficLightY = y + scaledTitleBarHeight / 2;
  const trafficLightX = x + TRAFFIC_LIGHT_OFFSET_X * scale;
  drawTrafficLights(ctx, trafficLightX, trafficLightY, scale);

  if (inset > 0 && insetColor) {
    ctx.fillStyle = insetColor;
    ctx.fillRect(
      x,
      y + scaledTitleBarHeight,
      frameWidth,
      frameHeight - scaledTitleBarHeight
    );
  }

  const contentX = x + inset;
  const contentY = y + scaledTitleBarHeight + inset;

  const srcX = balanceCrop.left;
  const srcY = balanceCrop.top;
  const srcW = image.naturalWidth - balanceCrop.left - balanceCrop.right;
  const srcH = image.naturalHeight - balanceCrop.top - balanceCrop.bottom;

  ctx.drawImage(
    image,
    srcX,
    srcY,
    srcW,
    srcH,
    contentX,
    contentY,
    croppedWidth,
    croppedHeight
  );

  ctx.restore();
  ctx.save();

  ctx.strokeStyle = theme.frameBorder;
  ctx.lineWidth = 1 * scale;
  ctx.beginPath();
  ctx.roundRect(x, y, frameWidth, frameHeight, scaledCornerRadius);
  ctx.stroke();

  ctx.restore();
};

const renderImageToCanvas = (
  ctx: CanvasRenderingContext2D,
  image: HTMLImageElement,
  x: number,
  y: number,
  croppedWidth: number,
  croppedHeight: number,
  cornerRadius: number,
  shadow: number,
  balanceCrop: BalanceCrop = { left: 0, top: 0, right: 0, bottom: 0 }
) => {
  ctx.save();

  const shadowBlur = (shadow / 100) * 50;
  const shadowOpacity = shadow > 0 ? 0.2 + (shadow / 100) * 0.3 : 0;
  const shadowOffsetY = (shadow / 100) * 15;

  if (shadow > 0) {
    ctx.shadowColor = `rgba(0, 0, 0, ${shadowOpacity})`;
    ctx.shadowBlur = shadowBlur;
    ctx.shadowOffsetX = 0;
    ctx.shadowOffsetY = shadowOffsetY;
  }

  ctx.beginPath();
  if (cornerRadius > 0) {
    ctx.roundRect(x, y, croppedWidth, croppedHeight, cornerRadius);
  } else {
    ctx.rect(x, y, croppedWidth, croppedHeight);
  }

  if (shadow > 0) {
    ctx.fillStyle = 'white';
    ctx.fill();
  }

  ctx.clip();

  const srcX = balanceCrop.left;
  const srcY = balanceCrop.top;
  const srcW = image.naturalWidth - balanceCrop.left - balanceCrop.right;
  const srcH = image.naturalHeight - balanceCrop.top - balanceCrop.bottom;

  ctx.drawImage(
    image,
    srcX,
    srcY,
    srcW,
    srcH,
    x,
    y,
    croppedWidth,
    croppedHeight
  );

  ctx.restore();
};

const renderImageWithInset = (
  ctx: CanvasRenderingContext2D,
  image: HTMLImageElement,
  x: number,
  y: number,
  frameWidth: number,
  frameHeight: number,
  cornerRadius: number,
  shadow: number,
  inset: number,
  insetColor: string,
  balanceCrop: BalanceCrop = { left: 0, top: 0, right: 0, bottom: 0 }
) => {
  ctx.save();

  const shadowBlur = (shadow / 100) * 50;
  const shadowOpacity = shadow > 0 ? 0.2 + (shadow / 100) * 0.3 : 0;
  const shadowOffsetY = (shadow / 100) * 15;

  if (shadow > 0) {
    ctx.shadowColor = `rgba(0, 0, 0, ${shadowOpacity})`;
    ctx.shadowBlur = shadowBlur;
    ctx.shadowOffsetX = 0;
    ctx.shadowOffsetY = shadowOffsetY;
  }

  ctx.beginPath();
  if (cornerRadius > 0) {
    ctx.roundRect(x, y, frameWidth, frameHeight, cornerRadius);
  } else {
    ctx.rect(x, y, frameWidth, frameHeight);
  }

  ctx.fillStyle = insetColor;
  ctx.fill();

  ctx.shadowColor = 'transparent';
  ctx.shadowBlur = 0;
  ctx.shadowOffsetY = 0;

  ctx.clip();

  const contentX = x + inset;
  const contentY = y + inset;
  const croppedWidth = frameWidth - inset * 2;
  const croppedHeight = frameHeight - inset * 2;

  const srcX = balanceCrop.left;
  const srcY = balanceCrop.top;
  const srcW = image.naturalWidth - balanceCrop.left - balanceCrop.right;
  const srcH = image.naturalHeight - balanceCrop.top - balanceCrop.bottom;

  ctx.drawImage(
    image,
    srcX,
    srcY,
    srcW,
    srcH,
    contentX,
    contentY,
    croppedWidth,
    croppedHeight
  );

  ctx.restore();
};

const svgToImage = (svgString: string): Promise<HTMLImageElement> => {
  return new Promise((resolve, reject) => {
    const img = new Image();
    const blob = new Blob([svgString], { type: 'image/svg+xml' });
    const url = URL.createObjectURL(blob);

    img.onload = () => {
      URL.revokeObjectURL(url);
      resolve(img);
    };

    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error('Failed to load SVG as image'));
    };

    img.src = url;
  });
};

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
  x: number,
  y: number,
  width: number,
  height: number,
  blockSize: number
) => {
  const canvasWidth = ctx.canvas.width;
  const canvasHeight = ctx.canvas.height;

  const flooredX = Math.floor(x);
  const flooredY = Math.floor(y);
  const clampedX = Math.max(0, flooredX);
  const clampedY = Math.max(0, flooredY);
  const clampedWidth = Math.min(
    Math.ceil(width) - (clampedX - flooredX),
    canvasWidth - clampedX
  );
  const clampedHeight = Math.min(
    Math.ceil(height) - (clampedY - flooredY),
    canvasHeight - clampedY
  );

  if (clampedWidth <= 0 || clampedHeight <= 0) return;

  const imageData = ctx.getImageData(
    clampedX,
    clampedY,
    clampedWidth,
    clampedHeight
  );
  const data = imageData.data;

  for (let py = 0; py < clampedHeight; py += blockSize) {
    for (let px = 0; px < clampedWidth; px += blockSize) {
      const sampleX = Math.min(
        px + Math.floor(blockSize / 2),
        clampedWidth - 1
      );
      const sampleY = Math.min(
        py + Math.floor(blockSize / 2),
        clampedHeight - 1
      );
      const sampleIdx = (sampleY * clampedWidth + sampleX) * 4;

      const r = data[sampleIdx];
      const g = data[sampleIdx + 1];
      const b = data[sampleIdx + 2];

      for (let by = 0; by < blockSize && py + by < clampedHeight; by++) {
        for (let bx = 0; bx < blockSize && px + bx < clampedWidth; bx++) {
          const idx = ((py + by) * clampedWidth + (px + bx)) * 4;
          data[idx] = r;
          data[idx + 1] = g;
          data[idx + 2] = b;
        }
      }
    }
  }

  ctx.putImageData(imageData, clampedX, clampedY);
};

const blurRegion = (
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number
) => {
  const canvasWidth = ctx.canvas.width;
  const canvasHeight = ctx.canvas.height;

  const flooredX = Math.floor(x);
  const flooredY = Math.floor(y);
  const clampedX = Math.max(0, flooredX);
  const clampedY = Math.max(0, flooredY);
  const clampedWidth = Math.min(
    Math.ceil(width) - (clampedX - flooredX),
    canvasWidth - clampedX
  );
  const clampedHeight = Math.min(
    Math.ceil(height) - (clampedY - flooredY),
    canvasHeight - clampedY
  );

  if (clampedWidth <= 0 || clampedHeight <= 0) return;

  const tempCanvas = document.createElement('canvas');
  const padding = radius * 2;
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
    ctx.canvas,
    sourceX,
    sourceY,
    sourceWidth,
    sourceHeight,
    0,
    0,
    sourceWidth,
    sourceHeight
  );

  tempCtx.filter = `blur(${radius}px)`;
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
  height: number
) => {
  const flooredX = Math.floor(x);
  const flooredY = Math.floor(y);
  const clampedX = Math.max(0, flooredX);
  const clampedY = Math.max(0, flooredY);
  const clampedWidth = Math.ceil(width) - (clampedX - flooredX);
  const clampedHeight = Math.ceil(height) - (clampedY - flooredY);

  if (clampedWidth <= 0 || clampedHeight <= 0) return;

  ctx.fillStyle = '#000000';
  ctx.fillRect(clampedX, clampedY, clampedWidth, clampedHeight);
};

const pointsToCoordinates = (points: number[]): [number, number][] => {
  const coords: [number, number][] = [];
  for (let i = 0; i < points.length; i += 2) {
    coords.push([points[i], points[i + 1]]);
  }
  return coords;
};

const applyHighlightEffect = (
  ctx: CanvasRenderingContext2D,
  highlight: HighlightAnnotation,
  offsetX: number,
  offsetY: number,
  scale: number
) => {
  const coords = pointsToCoordinates(highlight.points).map(
    ([x, y]) =>
      [(x + offsetX) * scale, (y + offsetY) * scale] as [number, number]
  );

  if (coords.length < 2) return;

  const halfWidth = (highlight.strokeWidth * scale) / 2;

  const upperEdge: [number, number][] = [];
  const lowerEdge: [number, number][] = [];

  for (let i = 0; i < coords.length; i++) {
    const [x, y] = coords[i];

    let dx = 0;
    let dy = 1;

    if (i < coords.length - 1) {
      const [nx, ny] = coords[i + 1];
      const len = Math.sqrt((nx - x) ** 2 + (ny - y) ** 2);
      if (len > 0) {
        dx = -(ny - y) / len;
        dy = (nx - x) / len;
      }
    } else if (i > 0) {
      const [px, py] = coords[i - 1];
      const len = Math.sqrt((x - px) ** 2 + (y - py) ** 2);
      if (len > 0) {
        dx = -(y - py) / len;
        dy = (x - px) / len;
      }
    }

    upperEdge.push([x + dx * halfWidth, y + dy * halfWidth]);
    lowerEdge.push([x - dx * halfWidth, y - dy * halfWidth]);
  }

  ctx.save();
  ctx.globalCompositeOperation = 'multiply';
  ctx.globalAlpha = highlight.opacity;
  ctx.fillStyle = highlight.fill;

  ctx.beginPath();
  ctx.moveTo(upperEdge[0][0], upperEdge[0][1]);

  for (let i = 1; i < upperEdge.length; i++) {
    ctx.lineTo(upperEdge[i][0], upperEdge[i][1]);
  }

  for (let i = lowerEdge.length - 1; i >= 0; i--) {
    ctx.lineTo(lowerEdge[i][0], lowerEdge[i][1]);
  }

  ctx.closePath();
  ctx.fill();
  ctx.restore();
};

const applyRedactEffect = (
  ctx: CanvasRenderingContext2D,
  redact: RedactAnnotation,
  offsetX: number,
  offsetY: number,
  scale: number
) => {
  const x = (redact.x + offsetX) * scale;
  const y = (redact.y + offsetY) * scale;
  const w = redact.width * scale;
  const h = redact.height * scale;

  const rectX = w < 0 ? x + w : x;
  const rectY = h < 0 ? y + h : y;
  const rectW = Math.abs(w);
  const rectH = Math.abs(h);

  const intensity = INTENSITY_MAP[redact.intensity] || INTENSITY_MAP[5];
  const scaledPixelSize = Math.max(1, Math.round(intensity.pixelSize * scale));
  const scaledBlurRadius = Math.max(
    1,
    Math.round(intensity.blurRadius * scale)
  );

  switch (redact.style) {
    case 'pixelate':
      pixelateRegion(ctx, rectX, rectY, rectW, rectH, scaledPixelSize);
      break;
    case 'blur':
      blurRegion(ctx, rectX, rectY, rectW, rectH, scaledBlurRadius);
      break;
    case 'blackout':
      blackoutRegion(ctx, rectX, rectY, rectW, rectH);
      break;
  }
};

export const useCanvasExport = ({
  padding,
  inset = 0,
  image,
  imageWidth,
  imageHeight,
  cornerRadius,
  shadow,
  spacing = 0,
  gradient,
  backgroundImage,
  backgroundBlur = 0,
  noise = 0,
  getSvgForExport,
  annotations = [],
  windowFrame = 'none',
  balance = false,
  extraLayers = [],
  extraLayerImages = {},
}: UseCanvasExportProps): UseCanvasExportReturn => {
  const originalEdgeColor = useMemo(() => {
    if (!image || !balance) return null;
    return detectDominantEdgeColor(image);
  }, [image, balance]);

  const nativeBalanceCrop: BalanceCrop = useMemo(() => {
    if (!balance || !image || !originalEdgeColor) {
      return { left: 0, top: 0, right: 0, bottom: 0 };
    }

    const bounds = detectContentBounds(image, originalEdgeColor);
    if (!bounds) return { left: 0, top: 0, right: 0, bottom: 0 };

    return bounds;
  }, [image, balance, originalEdgeColor]);

  const insetColor = useMemo(() => {
    if (!image || inset <= 0) return null;
    return sampleDominantInsetColor(image, nativeBalanceCrop);
  }, [image, inset, nativeBalanceCrop]);

  const exportToImage = useCallback(
    async (format: ScreenshotFormat = 'png'): Promise<string> => {
      if (!image) {
        throw new Error('Image not loaded');
      }

      const nativeWidth = image.naturalWidth;
      const nativeHeight = image.naturalHeight;

      const scaleX = nativeWidth / imageWidth;
      const scaleY = nativeHeight / imageHeight;
      const nativeScale = Math.max(scaleX, scaleY);

      const nativeCroppedWidth =
        nativeWidth - nativeBalanceCrop.left - nativeBalanceCrop.right;
      const nativeCroppedHeight =
        nativeHeight - nativeBalanceCrop.top - nativeBalanceCrop.bottom;

      const nativePadding = Math.round(padding * nativeScale);
      const nativeInset = Math.round(inset * nativeScale);
      const nativeSpacing = Math.round(spacing * nativeScale);
      const nativeCornerRadius = Math.round(cornerRadius * nativeScale);

      const hasWindowFrame = windowFrame !== 'none';
      const nativeTitleBarHeight = hasWindowFrame
        ? Math.round(WINDOW_FRAME_TITLE_BAR_HEIGHT * nativeScale)
        : 0;

      const primaryFrameW = nativeCroppedWidth + nativeInset * 2;
      const primaryFrameH =
        nativeCroppedHeight + nativeTitleBarHeight + nativeInset * 2;

      const layout = computeLayerLayout({
        primaryWidth: primaryFrameW,
        primaryHeight: primaryFrameH,
        extraLayers: extraLayers.map(l => ({
          ...l,
          naturalWidth: l.naturalWidth + nativeInset * 2,
          naturalHeight:
            l.naturalHeight + nativeTitleBarHeight + nativeInset * 2,
        })),
        spacing: nativeSpacing,
      });

      const frameWidth = layout.width;
      const frameHeight = layout.height;
      const canvasWidth = frameWidth + nativePadding * 2;
      const canvasHeight = frameHeight + nativePadding * 2;

      const exportCanvas = document.createElement('canvas');
      exportCanvas.width = canvasWidth;
      exportCanvas.height = canvasHeight;
      const ctx = exportCanvas.getContext('2d');

      if (!ctx) {
        throw new Error('Failed to get canvas context');
      }

      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = 'high';

      const nativeBlurRadius = (backgroundBlur / 100) * 50 * nativeScale;

      if (backgroundImage) {
        try {
          const bgImg = await loadImageFromDataUrl(backgroundImage);
          renderBackgroundImageToCanvas(
            ctx,
            bgImg,
            canvasWidth,
            canvasHeight,
            nativeBlurRadius
          );
        } catch (error) {
          console.error('Failed to render background image:', error);
        }
      } else if (gradient) {
        renderGradientToCanvas(
          ctx,
          gradient,
          canvasWidth,
          canvasHeight,
          nativeBlurRadius
        );
      }

      if ((backgroundImage || gradient) && noise > 0) {
        renderNoise(ctx, canvasWidth, canvasHeight, noise);
      }

      const primaryRect = findPrimaryRect(layout.rects);

      const drawLayerNative = (
        srcImage: HTMLImageElement,
        rect: { x: number; y: number; width: number; height: number },
        layerBalanceCrop: BalanceCrop,
        layerInsetColor: string | null
      ) => {
        const layerFrameX = nativePadding + rect.x;
        const layerFrameY = nativePadding + rect.y;
        const layerContentW = rect.width - nativeInset * 2;
        const layerContentH =
          rect.height - nativeTitleBarHeight - nativeInset * 2;

        if (hasWindowFrame) {
          renderWindowFrame(
            ctx,
            srcImage,
            layerFrameX,
            layerFrameY,
            layerContentW,
            layerContentH,
            windowFrame as 'macos-light' | 'macos-dark',
            shadow,
            nativeScale,
            nativeInset,
            layerInsetColor,
            layerBalanceCrop
          );
          return;
        }

        if (nativeInset > 0 && layerInsetColor) {
          renderImageWithInset(
            ctx,
            srcImage,
            layerFrameX,
            layerFrameY,
            rect.width,
            rect.height,
            nativeCornerRadius,
            shadow,
            nativeInset,
            layerInsetColor,
            layerBalanceCrop
          );
          return;
        }

        renderImageToCanvas(
          ctx,
          srcImage,
          layerFrameX,
          layerFrameY,
          layerContentW,
          layerContentH,
          nativeCornerRadius,
          shadow,
          layerBalanceCrop
        );
      };

      drawLayerNative(image, primaryRect, nativeBalanceCrop, insetColor);

      for (const layer of extraLayers) {
        const img = extraLayerImages[layer.id];
        if (!img) continue;
        const rect = layout.rects.find(r => r.id === layer.id);
        if (!rect) continue;
        const layerInsetColor =
          nativeInset > 0 ? sampleDominantInsetColor(img) : null;
        drawLayerNative(
          img,
          rect,
          { left: 0, top: 0, right: 0, bottom: 0 },
          layerInsetColor
        );
      }

      const redactAnnotations = annotations.filter(
        (ann): ann is RedactAnnotation => ann.type === 'redact'
      );

      const effectOffsetX =
        (nativePadding + primaryRect.x + nativeInset) / nativeScale;
      const effectOffsetY =
        (nativePadding + primaryRect.y + nativeInset + nativeTitleBarHeight) /
        nativeScale;

      for (const redact of redactAnnotations) {
        applyRedactEffect(
          ctx,
          redact,
          effectOffsetX,
          effectOffsetY,
          nativeScale
        );
      }

      const svgString = getSvgForExport(nativeScale);
      if (svgString) {
        try {
          const svgImage = await svgToImage(svgString);
          ctx.drawImage(svgImage, 0, 0);
        } catch (error) {
          console.error('Failed to render SVG annotations:', error);
        }
      }

      const highlightAnnotations = annotations.filter(
        (ann): ann is HighlightAnnotation => ann.type === 'highlight'
      );

      for (const highlight of highlightAnnotations) {
        applyHighlightEffect(
          ctx,
          highlight,
          effectOffsetX,
          effectOffsetY,
          nativeScale
        );
      }

      const mimeType = format === 'jpeg' ? 'image/jpeg' : 'image/png';
      const quality = format === 'jpeg' ? 0.92 : 1;
      const dataURL = exportCanvas.toDataURL(mimeType, quality);
      return dataURL.replace(/^data:image\/(png|jpeg);base64,/, '');
    },
    [
      padding,
      inset,
      spacing,
      insetColor,
      image,
      imageWidth,
      imageHeight,
      cornerRadius,
      shadow,
      gradient,
      backgroundImage,
      backgroundBlur,
      noise,
      getSvgForExport,
      annotations,
      windowFrame,
      nativeBalanceCrop,
      extraLayers,
      extraLayerImages,
    ]
  );

  return { exportToImage };
};
