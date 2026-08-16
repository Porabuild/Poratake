import type { CursorStyle } from './cursor';
import type { CameraStyle, CameraSegment } from './camera';
import type { KeyboardStyle } from './keyboard';
import type { SubtitleStyle } from './subtitle';
import type { AudioStyle } from './audio';
import type { ZoomSegment, ZoomSettings } from './zoom';
import type { VideoWallpaperSettings } from './video-wallpaper';
import type { Segment } from '@/renderer/components/video-editor/types';
import type { DrawingSegment } from './drawing';
import type {
  VideoFormat,
  VideoResolution,
  VideoFrameRate,
  VideoQualityPreset,
  RecordingType,
} from './video';
import type { FirstFrameSettings } from './first-frame';
import type { MusicTrack } from './music';

export interface ExportSettings {
  format: VideoFormat;
  resolution: VideoResolution;
  qualityPreset: VideoQualityPreset;
  frameRate: VideoFrameRate;
  openInFinder: boolean;
}

export const EDITOR_STATE_VERSION = 2;

export interface VideoEditorState {
  version: 1 | 2;
  savedAt: string;
  recordingType?: RecordingType;
  sourceDuration?: number;

  segments: Segment[];

  cursorStyle: CursorStyle;

  cameraStyle: CameraStyle;

  keyboardStyle: KeyboardStyle;

  subtitleStyle: SubtitleStyle;

  audioStyle: AudioStyle;

  zoomSegments: ZoomSegment[];

  zoomSettings: ZoomSettings;

  cameraSegments?: CameraSegment[];

  drawingSegments?: DrawingSegment[];

  wallpaper?: VideoWallpaperSettings;

  firstFrame?: FirstFrameSettings;

  musicTracks?: MusicTrack[];

  exportSettings?: ExportSettings;

  timelineZoom?: number;

  ui: {
    sidebarOpen: boolean;
    sidebarTab:
      | 'cursor'
      | 'zoom'
      | 'drawing'
      | 'camera'
      | 'audio'
      | 'wallpaper'
      | 'keyboard'
      | 'subtitle'
      | 'first-frame'
      | 'export';
    scrubAudioEnabled?: boolean;
  };
}
