import type { VideoQualityPreset } from '@/types/video';

interface QualityConfig {
  factor: number;
  minMbps: number;
  maxMbps: number;
}

const QUALITY_CONFIGS: Record<VideoQualityPreset, QualityConfig> = {
  studio: { factor: 0.15, minMbps: 12, maxMbps: 100 },
  social: { factor: 0.07, minMbps: 8, maxMbps: 16 },
  web: { factor: 0.028, minMbps: 1.5, maxMbps: 4 },
  'web-low': { factor: 0.018, minMbps: 0.6, maxMbps: 1.5 },
};

type ContentType = 'screen-only' | 'screen-camera' | 'camera-only';

const CONTENT_FACTORS: Record<ContentType, number> = {
  'screen-only': 1.1,
  'screen-camera': 1.15,
  'camera-only': 1.0,
};

export interface BitrateOptions {
  width: number;
  height: number;
  fps: number;
  qualityPreset: VideoQualityPreset;
  hasCamera?: boolean;
}

export function calculateBitrate(options: BitrateOptions): number {
  const { width, height, fps, qualityPreset, hasCamera = false } = options;

  const config = QUALITY_CONFIGS[qualityPreset];
  const contentType: ContentType = hasCamera ? 'screen-camera' : 'screen-only';
  const contentFactor = CONTENT_FACTORS[contentType];

  const pixels = width * height;
  const bitrateMbps =
    (pixels * fps * config.factor * contentFactor) / 1_000_000;

  const clampedMbps = Math.max(
    config.minMbps,
    Math.min(config.maxMbps, bitrateMbps)
  );

  return Math.round(clampedMbps * 1_000_000);
}
