import fs from 'fs/promises';
import { existsSync } from 'fs';
import { loadCursorData } from './cursor-data';
import { getEditorStatePath } from './recording-project';
import { generateAutoZoomSegments } from './auto-zoom';
import type { VideoEditorState } from '@/types/video-editor-state';
import type { RecordingType } from '@/types/video';
import { DEFAULT_CURSOR_STYLE } from '@/types/cursor';
import { DEFAULT_CAMERA_STYLE } from '@/types/camera';
import { DEFAULT_KEYBOARD_STYLE } from '@/types/keyboard';
import { DEFAULT_SUBTITLE_STYLE } from '@/types/subtitle';
import { DEFAULT_AUDIO_STYLE } from '@/types/audio';
import { DEFAULT_ZOOM_SETTINGS } from '@/types/zoom';
import { getConfig } from '@/main/settings';

interface GenerateEditorStateOptions {
  projectPath: string;
  recordingType?: RecordingType;
}

export async function generateInitialEditorState({
  projectPath,
  recordingType,
}: GenerateEditorStateOptions): Promise<boolean> {
  const statePath = getEditorStatePath(projectPath);
  if (!statePath) {
    console.log('Not a project folder, skipping editor state generation');
    return false;
  }

  if (existsSync(statePath)) {
    console.log(
      'Editor state already exists, skipping editor state generation'
    );
    return false;
  }

  const cursorData = await loadCursorData(projectPath);
  const shouldGenerateAutoZoom = getConfig().recording.autoZoom;
  const zoomSegments =
    shouldGenerateAutoZoom && cursorData
      ? generateAutoZoomSegments(cursorData)
      : [];

  if (zoomSegments.length > 0) {
    console.log(`Generated ${zoomSegments.length} auto-zoom segments`);
  }

  const initialState: VideoEditorState = {
    version: 1,
    savedAt: new Date().toISOString(),
    recordingType,
    segments: [],
    cursorStyle: DEFAULT_CURSOR_STYLE,
    cameraStyle: DEFAULT_CAMERA_STYLE,
    keyboardStyle: DEFAULT_KEYBOARD_STYLE,
    subtitleStyle: DEFAULT_SUBTITLE_STYLE,
    audioStyle: DEFAULT_AUDIO_STYLE,
    zoomSegments,
    zoomSettings: DEFAULT_ZOOM_SETTINGS,
    drawingSegments: [],
    ui: {
      sidebarOpen: zoomSegments.length > 0,
      sidebarTab: 'zoom',
      scrubAudioEnabled: false,
    },
  };

  try {
    await fs.writeFile(statePath, JSON.stringify(initialState, null, 2));
    console.log('Initial editor state saved');
    return true;
  } catch (error) {
    console.error('Failed to save initial editor state:', error);
    return false;
  }
}
