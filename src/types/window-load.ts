import type { AreaOverlayParams } from './area-overlay';
import type { CapturePreviewParams } from './capture-preview';
import type { EditorState } from './history';
import type { RecordingControlState } from './recording-control';
import type { ScrollCaptureOverlayParams } from './scroll-capture';
import type {
  EditorActionShortcuts,
  EditorPreferences,
  EditorShortcuts,
  SettingsConfig,
} from './settings';

export interface ScreenshotWindowParams {
  filePath: string;
  imageUrl?: string;
  width?: number;
  height?: number;
  editorState?: EditorState;
  historyId?: string;
  initialPreferences: EditorPreferences;
  screenshotSettings: SettingsConfig['screenshot'];
  editorShortcuts: EditorShortcuts;
  editorActionShortcuts: EditorActionShortcuts;
}

export interface PinWindowParams {
  imageBase64: string;
  width: number;
  height: number;
  pinId: string;
}

export interface VideoEditorWindowParams {
  filePath: string;
}

export interface SettingsWindowParams {
  nativeMaterial: boolean;
}

export type WindowLoadPayload =
  | { type: 'screenshot'; params: ScreenshotWindowParams }
  | { type: 'settings'; params: SettingsWindowParams }
  | { type: 'onboarding'; params: Record<string, never> }
  | { type: 'pin'; params: PinWindowParams }
  | { type: 'video-editor'; params: VideoEditorWindowParams }
  | { type: 'capture-preview'; params: CapturePreviewParams }
  | { type: 'area-overlay'; params: AreaOverlayParams }
  | { type: 'recording-control'; params: RecordingControlState }
  | { type: 'scroll-capture-overlay'; params: AreaOverlayParams }
  | { type: 'scroll-capture-control'; params: ScrollCaptureOverlayParams };

export type WindowType = WindowLoadPayload['type'];
