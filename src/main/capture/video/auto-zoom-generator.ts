import fs from 'fs/promises';
import { existsSync } from 'fs';
import { randomUUID } from 'crypto';
import { loadCursorData } from './cursor-data';
import { getEditorStatePath } from './recording-project';
import { generateAutoZoomSegments } from '@/types/auto-zoom';
import type { VideoEditorState } from '@/types/video-editor-state';
import { EDITOR_STATE_VERSION } from '@/types/video-editor-state';
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
  duration?: number;
}

export async function generateInitialEditorState({
  projectPath,
  recordingType,
  duration,
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
  const recordingSettings = getConfig().recording;
  const shouldGenerateAutoZoom = recordingSettings.autoZoom;
  const cameraMirrored = recordingSettings.camera?.flipped ?? false;
  const sourceDuration =
    typeof duration === 'number' && Number.isFinite(duration) && duration > 0
      ? duration
      : undefined;
  const zoomSegments =
    shouldGenerateAutoZoom && cursorData
      ? generateAutoZoomSegments(cursorData)
      : [];

  if (zoomSegments.length > 0) {
    console.log(`Generated ${zoomSegments.length} auto-zoom segments`);
  }

  const initialState: VideoEditorState = {
    version: EDITOR_STATE_VERSION,
    savedAt: new Date().toISOString(),
    recordingType,
    sourceDuration,
    segments:
      sourceDuration !== undefined
        ? [
            {
              id: randomUUID(),
              originalStart: 0,
              originalEnd: sourceDuration,
              trimMinStart: 0,
              trimMaxEnd: sourceDuration,
            },
          ]
        : [],
    cursorStyle: DEFAULT_CURSOR_STYLE,
    cameraStyle: { ...DEFAULT_CAMERA_STYLE, mirrored: cameraMirrored },
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
