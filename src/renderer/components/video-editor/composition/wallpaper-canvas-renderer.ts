import type { VideoWallpaperSettings } from '@/types/video-wallpaper';
import { calculateWallpaperDimensions } from '@/types/video-wallpaper';
import type { Context2D } from './types';
import {
  renderBackgroundImageToCanvas,
  renderGradientToCanvas,
} from '@/renderer/utils/wallpaper-render';

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
    renderGradientToCanvas(ctx, wallpaper.gradient, dims.width, dims.height);
  } else if (isEnabled && backgroundImage) {
    renderBackgroundImageToCanvas(
      ctx,
      backgroundImage,
      dims.width,
      dims.height
    );
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
