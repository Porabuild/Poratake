import type {
  VideoExportOptions,
  VideoFormat,
  VideoResolution,
  VideoFrameRate,
  VideoQualityPreset,
} from './video';

export const FREE_FORMATS: VideoFormat[] = ['mp4'];

export const FREE_RESOLUTIONS: VideoResolution[] = ['1080p', '720p', '480p'];

export const FREE_FRAME_RATES: VideoFrameRate[] = [
  '30',
  '25',
  '24',
  '20',
  '10',
];

export const FREE_QUALITY_PRESETS: VideoQualityPreset[] = [
  'social',
  'web',
  'web-low',
];

export const PRO_FEATURES = [
  '4K & original resolution export',
  '60fps export',
  'Studio quality & GIF export',
  'Hosted cloud uploads & shareable links',
];

export function isFormatFree(format: VideoFormat): boolean {
  return FREE_FORMATS.includes(format);
}

export function isResolutionFree(resolution: VideoResolution): boolean {
  return FREE_RESOLUTIONS.includes(resolution);
}

export function isFrameRateFree(frameRate: VideoFrameRate): boolean {
  return FREE_FRAME_RATES.includes(frameRate);
}

export function isQualityPresetFree(preset: VideoQualityPreset): boolean {
  return FREE_QUALITY_PRESETS.includes(preset);
}

export function clampExportOptionsToFree(
  options: VideoExportOptions
): VideoExportOptions {
  return {
    ...options,
    format: isFormatFree(options.format) ? options.format : 'mp4',
    resolution: isResolutionFree(options.resolution)
      ? options.resolution
      : '1080p',
    frameRate: isFrameRateFree(options.frameRate) ? options.frameRate : '30',
    qualityPreset: isQualityPresetFree(options.qualityPreset)
      ? options.qualityPreset
      : 'social',
  };
}
