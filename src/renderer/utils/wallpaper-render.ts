import type { GradientOption, WindowFrameStyle } from '@/types/editor';
import type { Canvas2DContext } from '@/renderer/utils/canvas';
import { loadImage } from '@/renderer/utils/image';
import type { BalanceCrop } from '@/renderer/utils/color-detection';
import { renderNoise } from '@/renderer/utils/noise';
import {
  getWindowFrameCornerRadius,
  isWindowsFrame,
  WINDOW_FRAME_THEMES,
  WINDOW_FRAME_TITLE_BAR_HEIGHT,
  type FramedWindowStyle,
} from '@/renderer/utils/window-frame';

const TRAFFIC_LIGHT_SIZE = 12;
const TRAFFIC_LIGHT_SPACING = 8;
const TRAFFIC_LIGHT_OFFSET_X = 13;

const TRAFFIC_LIGHT_COLORS = {
  close: '#FF5F57',
  minimize: '#FFBD2E',
  maximize: '#28C840',
};

export function applyImageShadow(
  ctx: Canvas2DContext,
  shadow: number,
  scale = 1
): void {
  if (shadow <= 0) return;

  ctx.shadowColor = `rgba(0, 0, 0, ${0.2 + (shadow / 100) * 0.3})`;
  ctx.shadowBlur = (shadow / 100) * 50 * scale;
  ctx.shadowOffsetX = 0;
  ctx.shadowOffsetY = (shadow / 100) * 15 * scale;
}

export const renderGradientToCanvas = (
  ctx: Canvas2DContext,
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

  const angle = Number.isFinite(gradient.angle) ? gradient.angle : 0;
  const angleRad = ((angle - 90) * Math.PI) / 180;
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

export const renderBackgroundImageToCanvas = (
  ctx: Canvas2DContext,
  bgImage: HTMLImageElement | ImageBitmap,
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

const drawWindowsControls = (
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  color: string,
  scale: number
) => {
  const controlWidth = 34 * scale;
  const centerY = y + (WINDOW_FRAME_TITLE_BAR_HEIGHT * scale) / 2;
  const minimizeX = x + width - controlWidth * 2.5;
  const maximizeX = x + width - controlWidth * 1.5;
  const closeX = x + width - controlWidth * 0.5;
  const iconRadius = 4 * scale;

  ctx.strokeStyle = color;
  ctx.lineWidth = scale;

  ctx.beginPath();
  ctx.moveTo(minimizeX - iconRadius, centerY + iconRadius * 0.75);
  ctx.lineTo(minimizeX + iconRadius, centerY + iconRadius * 0.75);
  ctx.stroke();

  ctx.strokeRect(
    maximizeX - iconRadius,
    centerY - iconRadius,
    iconRadius * 2,
    iconRadius * 2
  );

  ctx.beginPath();
  ctx.moveTo(closeX - iconRadius, centerY - iconRadius);
  ctx.lineTo(closeX + iconRadius, centerY + iconRadius);
  ctx.moveTo(closeX + iconRadius, centerY - iconRadius);
  ctx.lineTo(closeX - iconRadius, centerY + iconRadius);
  ctx.stroke();
};

export const renderWindowFrame = (
  ctx: CanvasRenderingContext2D,
  image: HTMLImageElement,
  x: number,
  y: number,
  croppedWidth: number,
  croppedHeight: number,
  frameStyle: FramedWindowStyle,
  shadow: number,
  scale: number,
  inset: number = 0,
  insetColor: string | null = null,
  balanceCrop: BalanceCrop = { left: 0, top: 0, right: 0, bottom: 0 }
) => {
  ctx.save();

  const theme = WINDOW_FRAME_THEMES[frameStyle];
  const scaledTitleBarHeight = WINDOW_FRAME_TITLE_BAR_HEIGHT * scale;
  const scaledCornerRadius = getWindowFrameCornerRadius(frameStyle) * scale;

  const frameWidth = croppedWidth + inset * 2;
  const frameHeight = croppedHeight + scaledTitleBarHeight + inset * 2;

  applyImageShadow(ctx, shadow, scale);

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

  if (isWindowsFrame(frameStyle)) {
    drawWindowsControls(ctx, x, y, frameWidth, theme.control, scale);
  } else {
    const trafficLightY = y + scaledTitleBarHeight / 2;
    const trafficLightX = x + TRAFFIC_LIGHT_OFFSET_X * scale;
    drawTrafficLights(ctx, trafficLightX, trafficLightY, scale);
  }

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

export const renderImageToCanvas = (
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

  applyImageShadow(ctx, shadow);

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

export const renderImageWithInset = (
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

  applyImageShadow(ctx, shadow);

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

export interface WallpaperCompositeSettings {
  gradient: GradientOption | null;
  backgroundImage?: string | null;
  backgroundBlur?: number;
  noise?: number;
  padding: number;
  corners: number;
  shadow: number;
  windowFrame?: { style: WindowFrameStyle };
}

/**
 * The editor stores wallpaper distances in CSS pixels of the displayed image,
 * and scales them by `naturalSize / displayedSize` on export. Outside the
 * editor there is no displayed image, so reproduce the size the editor would
 * have used: `Math.floor(natural / scaleFactor)` (see `getImageDimensions`).
 */
const getNativeScale = (image: HTMLImageElement): number => {
  const scaleFactor = window.devicePixelRatio || 1;
  const displayWidth = Math.max(
    1,
    Math.floor(image.naturalWidth / scaleFactor)
  );
  const displayHeight = Math.max(
    1,
    Math.floor(image.naturalHeight / scaleFactor)
  );

  return Math.max(
    image.naturalWidth / displayWidth,
    image.naturalHeight / displayHeight
  );
};

/**
 * Renders a single image onto its wallpaper background at native resolution,
 * without annotations, crop, inset or extra layers. This is the headless
 * equivalent of applying a wallpaper preset to a freshly opened editor and
 * exporting it.
 */
export const renderWallpaperComposite = async (
  image: HTMLImageElement,
  settings: WallpaperCompositeSettings
): Promise<HTMLCanvasElement> => {
  const {
    gradient,
    backgroundImage = null,
    backgroundBlur = 0,
    noise = 0,
    padding,
    corners,
    shadow,
    windowFrame,
  } = settings;

  const nativeScale = getNativeScale(image);
  const nativePadding = Math.round(padding * nativeScale);
  const nativeCornerRadius = Math.round(corners * nativeScale);

  const frameStyle = windowFrame?.style ?? 'none';
  const nativeTitleBarHeight =
    frameStyle === 'none'
      ? 0
      : Math.round(WINDOW_FRAME_TITLE_BAR_HEIGHT * nativeScale);

  const contentWidth = image.naturalWidth;
  const contentHeight = image.naturalHeight;
  const canvasWidth = contentWidth + nativePadding * 2;
  const canvasHeight = contentHeight + nativeTitleBarHeight + nativePadding * 2;

  const canvas = document.createElement('canvas');
  canvas.width = canvasWidth;
  canvas.height = canvasHeight;

  const ctx = canvas.getContext('2d');
  if (!ctx) {
    throw new Error('Failed to get canvas context');
  }

  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = 'high';

  const nativeBlurRadius = (backgroundBlur / 100) * 50 * nativeScale;

  if (backgroundImage) {
    const bgImage = await loadImage(backgroundImage);
    renderBackgroundImageToCanvas(
      ctx,
      bgImage,
      canvasWidth,
      canvasHeight,
      nativeBlurRadius
    );
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

  if (frameStyle === 'none') {
    renderImageToCanvas(
      ctx,
      image,
      nativePadding,
      nativePadding,
      contentWidth,
      contentHeight,
      nativeCornerRadius,
      shadow
    );
  } else {
    renderWindowFrame(
      ctx,
      image,
      nativePadding,
      nativePadding,
      contentWidth,
      contentHeight,
      frameStyle as FramedWindowStyle,
      shadow,
      nativeScale
    );
  }

  return canvas;
};
