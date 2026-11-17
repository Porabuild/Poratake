import { ipcMain } from 'electron';
import fs from 'fs';
import path from 'path';
import { getWindowData } from '../window-manager';
import { getEditorStatePath } from '../recording-project';
import { generateInitialEditorState } from '../auto-zoom-generator';
import {
  localizeWallpaperImage,
  resolveLocalizedWallpaperImage,
} from '@/main/settings/wallpaper-assets.ts';
import { writeFileAtomicSync } from '@/main/utils/atomic-file';
import type { VideoEditorState } from '@/types/video-editor-state';
import { EDITOR_STATE_VERSION } from '@/types/video-editor-state';
import { isValidEditorState } from '../state-validation';
import type { MusicTrack } from '@/types/music';
import { recordRemovedMusic } from '../music-retention';

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object';
}

function musicReferences(
  value: unknown
): Pick<MusicTrack, 'source' | 'fileName'>[] {
  if (!Array.isArray(value)) return [];
  return value.filter(
    (track): track is Pick<MusicTrack, 'source' | 'fileName'> =>
      isRecord(track) &&
      track.source === 'music' &&
      typeof track.fileName === 'string'
  );
}

function migrateEditorState(state: VideoEditorState): VideoEditorState {
  if (state.version === EDITOR_STATE_VERSION) return state;

  const size = state.cursorStyle?.size;
  const cursorStyle =
    typeof size === 'number' && Number.isFinite(size)
      ? { ...state.cursorStyle, size: Math.round(size / 2) }
      : state.cursorStyle;

  return { ...state, version: EDITOR_STATE_VERSION, cursorStyle };
}

function localizeStateWallpaper(
  state: VideoEditorState,
  statePath: string
): VideoEditorState {
  const wallpaper = state.wallpaper;
  if (!wallpaper?.backgroundImage) {
    return state;
  }

  let localizedImage: string;
  try {
    localizedImage = localizeWallpaperImage(
      wallpaper.backgroundImage,
      path.dirname(statePath)
    );
  } catch (error) {
    console.warn('Failed to localize editor wallpaper:', error);
    try {
      const savedState = JSON.parse(fs.readFileSync(statePath, 'utf-8')) as {
        wallpaper?: { backgroundImage?: unknown };
      };
      if (typeof savedState.wallpaper?.backgroundImage === 'string') {
        localizedImage = savedState.wallpaper.backgroundImage;
      } else {
        return state;
      }
    } catch {
      return state;
    }
  }
  if (localizedImage === wallpaper.backgroundImage) {
    return state;
  }
  return {
    ...state,
    wallpaper: {
      ...wallpaper,
      backgroundImage: localizedImage,
    },
  };
}

function resolveStateWallpaper(
  state: VideoEditorState,
  statePath: string
): VideoEditorState {
  const wallpaper = state.wallpaper;
  if (!wallpaper?.backgroundImage) {
    return state;
  }

  const resolvedImage = resolveLocalizedWallpaperImage(
    wallpaper.backgroundImage,
    path.dirname(statePath)
  );
  if (resolvedImage === wallpaper.backgroundImage) {
    return state;
  }
  return {
    ...state,
    wallpaper: {
      ...wallpaper,
      backgroundImage: resolvedImage,
    },
  };
}

function getRecordingMetadataFromStateFile(statePath: string): {
  recordingType?: VideoEditorState['recordingType'];
  sourceDuration?: number;
  musicTracks: Pick<MusicTrack, 'source' | 'fileName'>[];
} {
  if (!fs.existsSync(statePath)) return { musicTracks: [] };

  try {
    const content = fs.readFileSync(statePath, 'utf-8');
    const parsed = JSON.parse(content) as {
      recordingType?: unknown;
      sourceDuration?: unknown;
    };
    return {
      recordingType:
        parsed.recordingType === 'ios-device'
          ? parsed.recordingType
          : undefined,
      sourceDuration:
        typeof parsed.sourceDuration === 'number' &&
        Number.isFinite(parsed.sourceDuration)
          ? parsed.sourceDuration
          : undefined,
      musicTracks: isValidEditorState(parsed)
        ? musicReferences(parsed.musicTracks)
        : [],
    };
  } catch {
    return { musicTracks: [] };
  }
}

export function registerStateHandlers(): void {
  ipcMain.handle(
    'video-editor:getState',
    async (event): Promise<VideoEditorState | null> => {
      const data = getWindowData(event.sender.id);
      if (!data) return null;

      const statePath = getEditorStatePath(data.filePath);
      if (!statePath || !fs.existsSync(statePath)) return null;

      try {
        const content = await fs.promises.readFile(statePath, 'utf-8');
        const parsed = JSON.parse(content);

        if (!isValidEditorState(parsed)) {
          console.warn('Invalid editor state structure, ignoring saved state');
          return null;
        }

        return resolveStateWallpaper(migrateEditorState(parsed), statePath);
      } catch (error) {
        console.error('Failed to load editor state:', error);
        return null;
      }
    }
  );

  ipcMain.handle(
    'video-editor:saveState',
    async (event, state: VideoEditorState): Promise<boolean> => {
      const data = getWindowData(event.sender.id);
      if (!data) return false;

      const statePath = getEditorStatePath(data.filePath);
      if (!statePath) return false;

      if (!isValidEditorState(state)) {
        console.error('Invalid editor state, refusing to save');
        return false;
      }

      try {
        let previousMusic: Pick<MusicTrack, 'source' | 'fileName'>[] = [];
        try {
          const previous: unknown = JSON.parse(
            fs.readFileSync(statePath, 'utf8')
          );
          if (isRecord(previous))
            previousMusic = musicReferences(previous.musicTracks);
        } catch {}
        const stateToSave = localizeStateWallpaper(state, statePath);
        if (
          !recordRemovedMusic(
            data.filePath,
            previousMusic,
            musicReferences(stateToSave.musicTracks)
          )
        ) {
          return false;
        }
        writeFileAtomicSync(statePath, JSON.stringify(stateToSave, null, 2));
        return true;
      } catch (error) {
        console.error('Failed to save editor state:', error);
        return false;
      }
    }
  );

  ipcMain.handle('video-editor:resetState', async (event): Promise<boolean> => {
    const data = getWindowData(event.sender.id);
    if (!data) return false;

    const statePath = getEditorStatePath(data.filePath);
    if (!statePath) return false;

    const { recordingType, sourceDuration, musicTracks } =
      getRecordingMetadataFromStateFile(statePath);

    try {
      if (!recordRemovedMusic(data.filePath, musicTracks, [])) return false;
      const reset = await generateInitialEditorState({
        projectPath: data.filePath,
        recordingType,
        duration: sourceDuration,
        replaceExisting: true,
      });
      return reset;
    } catch (error) {
      console.error('Failed to reset editor state:', error);
      return false;
    }
  });
}
