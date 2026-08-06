import type { VideoWallpaperSettings } from '@/types/video-wallpaper';
import { calculateWallpaperDimensions } from '@/types/video-wallpaper';
import type { GradientOption } from '@/types/editor';
import type { Context2D } from './types';

export function renderGradientBackground(
  ctx: Context2D,
  gradient: GradientOption,
  width: number,
  height: number
): void {
  const angleRad = ((gradient.angle - 90) * Math.PI) / 180;
  const cos = Math.cos(angleRad);
  const sin = Math.sin(angleRad);

  const halfWidth = width / 2;
  const halfHeight = height / 2;
  const length = Math.sqrt(width * width + height * height) / 2;

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
  ctx.fillRect(0, 0, width, height);
}

export function renderImageBackground(
  ctx: Context2D,
  image: HTMLImageElement | ImageBitmap,
  width: number,
  height: number
): void {
  const imgAspect = image.width / image.height;
  const canvasAspect = width / height;

  let drawWidth: number;
  let drawHeight: number;
  let drawX: number;
  let drawY: number;

  if (imgAspect > canvasAspect) {
    drawHeight = height;
    drawWidth = height * imgAspect;
    drawX = (width - drawWidth) / 2;
    drawY = 0;
  } else {
    drawWidth = width;
    drawHeight = width / imgAspect;
    drawX = 0;
    drawY = (height - drawHeight) / 2;
  }

  ctx.drawImage(image, drawX, drawY, drawWidth, drawHeight);
}

export interface ShadowConfig {
  blur: number;
  opacity: number;
  offsetY: number;
}

export function calculateShadowConfig(
  shadowValue: number
): ShadowConfig | null {
  if (shadowValue === 0) return null;

  return {
    blur: Math.round(shadowValue * 0.25),
    opacity: Math.min(0.5, shadowValue / 300),
    offsetY: Math.round(shadowValue * 0.25 * 0.3),
  };
}

export interface WallpaperRenderResult {
  compositionWidth: number;
  compositionHeight: number;
  videoX: number;
  videoY: number;
  videoClipRadius: number;
  shadowConfig: ShadowConfig | null;
}

export function renderWallpaper(
  ctx: Context2D,
  wallpaper: VideoWallpaperSettings | null,
  videoWidth: number,
  videoHeight: number,
  backgroundImage?: HTMLImageElement | ImageBitmap | null
): WallpaperRenderResult {
  const isEnabled = wallpaper?.enabled ?? false;
  const padding = isEnabled ? (wallpaper?.padding ?? 0) : 0;
  const corners = isEnabled ? (wallpaper?.corners ?? 0) : 0;
  const shadow = isEnabled ? (wallpaper?.shadow ?? 0) : 0;
  const aspectRatio = isEnabled ? (wallpaper?.aspectRatio ?? null) : null;

  const dims = calculateWallpaperDimensions(
    videoWidth,
    videoHeight,
    padding,
    aspectRatio
  );

  ctx.clearRect(0, 0, dims.width, dims.height);

  if (isEnabled && wallpaper?.gradient) {
    renderGradientBackground(ctx, wallpaper.gradient, dims.width, dims.height);
  } else if (isEnabled && backgroundImage) {
    renderImageBackground(ctx, backgroundImage, dims.width, dims.height);
  }

  return {
    compositionWidth: dims.width,
    compositionHeight: dims.height,
    videoX: dims.videoX,
    videoY: dims.videoY,
    videoClipRadius: corners,
    shadowConfig: calculateShadowConfig(shadow),
  };
}
