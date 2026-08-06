import type { CompositionConfig } from '../composition';
import type {
  VideoResolution,
  VideoExportOptions,
  VideoQualityPreset,
} from '@/types/video';
import type { MusicTrack } from '@/types/music';

export type {
  AudioTrack,
  AudioSegment,
  AudioSegmentWithSpeed,
} from '@/types/audio';

export interface ExportOptions {
  sourceVideoPath: string;
  systemAudioPath?: string | null;
  micAudioPath?: string | null;
  systemAudioEnabled?: boolean;
  micAudioEnabled?: boolean;
  systemAudioVolume?: number;
  micAudioVolume?: number;
  hasEmbeddedAudio?: boolean;
  keyboardSoundPath?: string | null;
  keyboardSoundVolume?: number;
  cameraVideoPath?: string | null;
  musicTracks?: MusicTrack[];
  outputPath: string;
  config: CompositionConfig;
  frameRate: number;
  qualityPreset: VideoQualityPreset;
  resolution: VideoResolution;
  exportOptions?: VideoExportOptions;
  onProgress: (percent: number) => void;
}

export interface ExportResult {
  success: boolean;
  outputPath?: string;
  error?: string;
}

export interface EmbeddedAudioConfig {
  sourcePath: string;
  volume: number;
}

export interface ExportDimensions {
  width: number;
  height: number;
  scale: number;
}

export const RESOLUTION_MAP: Record<
  Exclude<VideoResolution, 'original'>,
  number
> = {
  '4k': 2160,
  '1080p': 1080,
  '720p': 720,
  '480p': 480,
};

export const MAX_H264_DIMENSION = 4096;
export const MAX_H264_PIXELS = 8847360;
export const PREFETCH_BATCH_SIZE = 20;
