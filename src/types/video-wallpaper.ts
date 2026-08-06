import type { AspectRatio } from './aspect-ratio';
import type { GradientOption } from './editor';

export interface VideoWallpaperSettings {
  enabled: boolean;
  gradient: GradientOption | null;
  backgroundImage: string | null;
  padding: number;
  corners: number;
  shadow: number;
  aspectRatio: AspectRatio | null;
  deviceFrame: boolean;
}

export const DEFAULT_VIDEO_WALLPAPER: VideoWallpaperSettings = {
  enabled: false,
  gradient: null,
  backgroundImage: null,
  padding: 0,
  corners: 0,
  shadow: 0,
  aspectRatio: null,
  deviceFrame: false,
};

export const IOS_DEVICE_DEFAULT_WALLPAPER: Omit<
  VideoWallpaperSettings,
  'backgroundImage'
> = {
  enabled: true,
  gradient: null,
  padding: 100,
  corners: 0,
  shadow: 200,
  aspectRatio: { name: '4:3', width: 4, height: 3 },
  deviceFrame: true,
};

export interface WallpaperDimensions {
  width: number;
  height: number;
  videoX: number;
  videoY: number;
}

export function calculateWallpaperDimensions(
  videoWidth: number,
  videoHeight: number,
  padding: number,
  aspectRatio?: AspectRatio | null
): WallpaperDimensions {
  const baseWidth = videoWidth + padding * 2;
  const baseHeight = videoHeight + padding * 2;

  if (!aspectRatio || (aspectRatio.width === 0 && aspectRatio.height === 0)) {
    return {
      width: baseWidth,
      height: baseHeight,
      videoX: padding,
      videoY: padding,
    };
  }

  const targetRatio = aspectRatio.width / aspectRatio.height;
  const currentRatio = baseWidth / baseHeight;

  let compositionWidth: number;
  let compositionHeight: number;

  if (currentRatio < targetRatio) {
    compositionWidth = Math.round(baseHeight * targetRatio);
    compositionHeight = baseHeight;
  } else {
    compositionWidth = baseWidth;
    compositionHeight = Math.round(baseWidth / targetRatio);
  }

  return {
    width: compositionWidth,
    height: compositionHeight,
    videoX: Math.round((compositionWidth - videoWidth) / 2),
    videoY: Math.round((compositionHeight - videoHeight) / 2),
  };
}

export function hasWallpaperEffect(settings: VideoWallpaperSettings): boolean {
  return (
    settings.gradient !== null ||
    settings.backgroundImage !== null ||
    settings.padding > 0 ||
    settings.corners > 0 ||
    settings.shadow > 0 ||
    settings.aspectRatio !== null ||
    settings.deviceFrame
  );
}
