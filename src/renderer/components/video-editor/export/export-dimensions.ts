import type { AspectRatio } from '@/types/aspect-ratio';
import type { VideoResolution } from '@/types/video';
import { calculateWallpaperDimensions } from '@/types/video-wallpaper';
import type { ExportDimensions } from './export-types';
import {
  RESOLUTION_MAP,
  MAX_H264_DIMENSION,
  MAX_H264_PIXELS,
} from './export-types';

export function calculateExportDimensions(
  videoWidth: number,
  videoHeight: number,
  padding: number,
  resolution: VideoResolution,
  wallpaperAspectRatio?: AspectRatio | null
): ExportDimensions {
  const dims = calculateWallpaperDimensions(
    videoWidth,
    videoHeight,
    padding,
    wallpaperAspectRatio
  );
  const compositionWidth = dims.width;
  const compositionHeight = dims.height;
  const aspectRatio = compositionWidth / compositionHeight;

  let targetHeight: number;

  if (resolution === 'original') {
    targetHeight = compositionHeight;
  } else {
    targetHeight = RESOLUTION_MAP[resolution];
  }

  let targetWidth = Math.round(targetHeight * aspectRatio);

  if (targetWidth % 2 !== 0) targetWidth += 1;
  if (targetHeight % 2 !== 0) targetHeight += 1;

  if (
    targetWidth > MAX_H264_DIMENSION ||
    targetHeight > MAX_H264_DIMENSION ||
    targetWidth * targetHeight > MAX_H264_PIXELS
  ) {
    const scaleFactor = Math.min(
      MAX_H264_DIMENSION / targetWidth,
      MAX_H264_DIMENSION / targetHeight,
      Math.sqrt(MAX_H264_PIXELS / (targetWidth * targetHeight))
    );

    targetWidth = Math.floor(targetWidth * scaleFactor);
    targetHeight = Math.floor(targetHeight * scaleFactor);

    if (targetWidth % 2 !== 0) targetWidth -= 1;
    if (targetHeight % 2 !== 0) targetHeight -= 1;
  }

  const scale = targetHeight / compositionHeight;

  return { width: targetWidth, height: targetHeight, scale };
}
