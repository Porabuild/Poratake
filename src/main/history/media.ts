import { existsSync, unlinkSync } from 'fs';
import type {
  EditorState,
  HistoryItem,
  VideoRecordingFeatures,
} from '@/types/history.ts';
import {
  deleteRecordingAssets,
  getCameraVideoPath,
  getCursorPath,
  getMicAudioPath,
  getProjectFolder,
  getSystemAudioPath,
} from '@/main/capture/video/recording-project.ts';
import { embedWallpaperImage } from '@/main/settings/wallpaper-assets.ts';
import { deleteThumbnail } from '@/main/utils/thumbnails.ts';

export const EMPTY_VIDEO_FEATURES: VideoRecordingFeatures = {
  hasMic: false,
  hasSystemAudio: false,
  hasCamera: false,
  hasCursor: false,
};

export function prepareHistoryEditorState(
  editorState: EditorState
): EditorState {
  const wallpaper = editorState.wallpaper;
  if (!wallpaper?.backgroundImage) {
    return editorState;
  }

  try {
    const embeddedImage = embedWallpaperImage(wallpaper.backgroundImage);
    if (embeddedImage === wallpaper.backgroundImage) {
      return editorState;
    }
    return {
      ...editorState,
      wallpaper: {
        ...wallpaper,
        backgroundImage: embeddedImage,
      },
    };
  } catch (error) {
    console.error('Failed to preserve history wallpaper:', error);
    return editorState;
  }
}

export async function cleanupHistoryItem(
  item: HistoryItem,
  releaseHistoryFile: (filePath: string) => Promise<void>
): Promise<boolean> {
  try {
    deleteThumbnail(item.originalPath);
  } catch {
    console.warn(`Failed to clean up history thumbnail: ${item.id}`);
  }

  try {
    if (item.type === 'video') {
      await releaseHistoryFile(item.originalPath);
      deleteRecordingAssets(item.originalPath);
      return true;
    }

    if (existsSync(item.originalPath)) {
      unlinkSync(item.originalPath);
    }
    return true;
  } catch {
    console.warn(`Failed to clean up history item: ${item.id}`);
    return false;
  }
}

export function getVideoRecordingFeatures(
  originalPath: string
): VideoRecordingFeatures {
  const projectFolder = getProjectFolder(originalPath);
  if (!projectFolder) {
    return EMPTY_VIDEO_FEATURES;
  }

  return {
    hasMic: existsSync(getMicAudioPath(originalPath)),
    hasSystemAudio: existsSync(getSystemAudioPath(originalPath)),
    hasCamera: existsSync(getCameraVideoPath(originalPath)),
    hasCursor: existsSync(getCursorPath(originalPath)),
  };
}
