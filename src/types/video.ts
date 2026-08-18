export const PROJECT_EXTENSION = '.capty';

export interface RecorderResponse {
  success: boolean;
  state: RecorderState | 'error' | 'quit';
  message?: string;
  outputPath?: string;
  cursorPath?: string;
  cameraPath?: string;
  keysPath?: string;
  systemAudioPath?: string;
  micAudioPath?: string;
  duration?: number;
}

export interface CompletedRecording {
  outputPath: string;
  cursorPath?: string;
  cameraPath?: string;
  keysPath?: string;
  systemAudioPath?: string;
  micAudioPath?: string;
  duration: number;
}

export interface RecordingConfig {
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  displayId?: number;
  windowId?: number;
  windowName?: string;
  includeAudio?: boolean;
  micEnabled?: boolean;
  micDeviceId?: string | null;
  micDeviceName?: string | null;
  cameraEnabled?: boolean;
  cameraDeviceId?: string | null;
  cameraDeviceName?: string | null;
  keyboardEnabled?: boolean;
  frameRate?: number;
  outputPath: string;
  iosDeviceId?: string | null;
  iosDeviceName?: string | null;
}

export type RecorderState = 'idle' | 'recording' | 'paused';

export type RecordingType = 'ios-device';

export type VideoResolution = 'original' | '4k' | '1080p' | '720p' | '480p';

export type VideoFrameRate =
  | '60'
  | '50'
  | '40'
  | '30'
  | '25'
  | '24'
  | '20'
  | '10';

export type VideoFormat = 'mp4' | 'gif';

export type VideoExportPreset = 'custom' | 'social';

export type VideoQualityPreset = 'studio' | 'social' | 'web' | 'web-low';

export const VIDEO_QUALITY_PRESETS: Record<VideoQualityPreset, number> = {
  studio: 100,
  social: 75,
  web: 50,
  'web-low': 25,
};

export type SocialMediaResolution = '4k' | '1080p' | '720p';

export type SocialMediaFrameRate = '60' | '30';

export interface SocialMediaPreset {
  resolution: SocialMediaResolution;
  frameRate: SocialMediaFrameRate;
  bitrate: number;
}

export interface FormatConfig {
  frameRates: VideoFrameRate[];
  resolutions: VideoResolution[];
  hasQuality: boolean;
  defaultFrameRate: VideoFrameRate;
  defaultResolution: VideoResolution;
  defaultQualityPreset: VideoQualityPreset;
}

export const FORMAT_CONFIGS: Record<VideoFormat, FormatConfig> = {
  mp4: {
    frameRates: ['60', '50', '40', '30', '25', '24', '20', '10'],
    resolutions: ['original', '4k', '1080p', '720p', '480p'],
    hasQuality: true,
    defaultFrameRate: '30',
    defaultResolution: '4k',
    defaultQualityPreset: 'studio',
  },
  gif: {
    frameRates: ['50', '30', '25', '24', '20', '10'],
    resolutions: ['1080p', '720p', '480p'],
    hasQuality: true,
    defaultFrameRate: '20',
    defaultResolution: '720p',
    defaultQualityPreset: 'web',
  },
};

export interface VideoExportOptions {
  format: VideoFormat;
  preset?: VideoExportPreset;
  resolution: VideoResolution;
  qualityPreset: VideoQualityPreset;
  frameRate: VideoFrameRate;
  socialPreset?: SocialMediaPreset;
}

export interface VideoSegment {
  id: string;
  startTime: number;
  endTime: number;
  timelineStart: number;
  speed?: number;
}

export interface VideoMetadata {
  fileSize: number;
  bitrate: number;
  width: number;
  height: number;
  duration: number;
}

export interface ProjectRenameResult {
  success: boolean;
  newProjectPath: string;
  newVideoPath: string;
  error?: string;
}
