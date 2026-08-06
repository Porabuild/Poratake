import { ipcMain, dialog, BrowserWindow } from 'electron';
import { existsSync } from 'fs';
import fs from 'fs/promises';
import { getWindowData } from '../window-manager';
import { loadCursorData, saveCursorData } from '../cursor-data';
import { loadCameraData, getAbsoluteCameraVideoPath } from '../camera-data';
import { loadKeyboardData } from '../keyboard-data';
import { getSystemAudioPath, getMicAudioPath } from '../recording-project';
import { getFFmpegPath } from '@/main/utils/ffmpeg';
import { validateCursorData } from '@/types/cursor';
import type { CursorData } from '@/types/cursor';
import type { CameraData } from '@/types/camera';
import type { KeyboardData } from '@/types/keyboard';

async function videoHasAudio(videoPath: string): Promise<boolean> {
  try {
    const ffmpegPath = getFFmpegPath();
    const { execFile } = await import('child_process');
    const { promisify } = await import('util');
    const execFileAsync = promisify(execFile);

    let stderr = '';
    try {
      await execFileAsync(ffmpegPath, ['-i', videoPath], { timeout: 10000 });
    } catch (error) {
      stderr = (error as { stderr?: string }).stderr || '';
    }

    return stderr.includes('Audio:');
  } catch {
    return false;
  }
}

export function registerDataHandlers(): void {
  ipcMain.handle('video-editor:getVideoPath', event => {
    const data = getWindowData(event.sender.id);
    return data?.filePath ?? null;
  });

  ipcMain.handle(
    'video-editor:getCursorData',
    async (event): Promise<CursorData | null> => {
      const data = getWindowData(event.sender.id);
      if (!data) return null;

      return loadCursorData(data.filePath);
    }
  );

  ipcMain.handle(
    'video-editor:getCameraData',
    async (
      event
    ): Promise<{ cameraData: CameraData; cameraVideoPath: string } | null> => {
      const data = getWindowData(event.sender.id);
      if (!data) return null;

      const cameraData = await loadCameraData(data.filePath);
      if (!cameraData) return null;

      const cameraVideoPath = getAbsoluteCameraVideoPath(
        data.filePath,
        cameraData
      );
      return { cameraData, cameraVideoPath };
    }
  );

  ipcMain.handle(
    'video-editor:getKeyboardData',
    async (event): Promise<KeyboardData | null> => {
      const data = getWindowData(event.sender.id);
      if (!data) return null;

      return loadKeyboardData(data.filePath);
    }
  );

  ipcMain.handle(
    'video-editor:getAudioPaths',
    async (
      event
    ): Promise<{
      systemAudioPath: string | null;
      micAudioPath: string | null;
      hasEmbeddedAudio: boolean;
    }> => {
      const data = getWindowData(event.sender.id);
      if (!data) {
        return {
          systemAudioPath: null,
          micAudioPath: null,
          hasEmbeddedAudio: false,
        };
      }

      const systemAudioFilePath = getSystemAudioPath(data.filePath);
      const micAudioFilePath = getMicAudioPath(data.filePath);

      const systemAudioExists = existsSync(systemAudioFilePath);
      const micAudioExists = existsSync(micAudioFilePath);

      let hasEmbeddedAudio = false;
      if (!systemAudioExists && !micAudioExists) {
        hasEmbeddedAudio = await videoHasAudio(data.filePath);
      }

      return {
        systemAudioPath: systemAudioExists ? systemAudioFilePath : null,
        micAudioPath: micAudioExists ? micAudioFilePath : null,
        hasEmbeddedAudio,
      };
    }
  );

  ipcMain.handle(
    'video-editor:saveCursorData',
    async (
      event,
      cursorData: CursorData
    ): Promise<{ success: boolean; error?: string }> => {
      const data = getWindowData(event.sender.id);
      if (!data) {
        return { success: false, error: 'No video data found' };
      }

      const validation = validateCursorData(cursorData);
      if (!validation.valid) {
        return { success: false, error: validation.error };
      }

      try {
        await saveCursorData(data.filePath, cursorData);
        return { success: true };
      } catch (error) {
        return {
          success: false,
          error: error instanceof Error ? error.message : 'Unknown error',
        };
      }
    }
  );

  ipcMain.handle(
    'video-editor:importCursorData',
    async (
      event
    ): Promise<{ success: boolean; data?: CursorData; error?: string }> => {
      const windowData = getWindowData(event.sender.id);
      if (!windowData) {
        return { success: false, error: 'No video data found' };
      }

      const window = BrowserWindow.fromWebContents(event.sender);
      if (!window) {
        return { success: false, error: 'Window not found' };
      }

      const result = await dialog.showOpenDialog(window, {
        title: 'Import Cursor Data',
        filters: [{ name: 'JSON Files', extensions: ['json'] }],
        properties: ['openFile'],
      });

      if (result.canceled || result.filePaths.length === 0) {
        return { success: false, error: 'Cancelled' };
      }

      try {
        const content = await fs.readFile(result.filePaths[0], 'utf-8');
        const parsed = JSON.parse(content);
        const validation = validateCursorData(parsed);

        if (!validation.valid) {
          return { success: false, error: validation.error };
        }

        await saveCursorData(windowData.filePath, validation.data!);
        return { success: true, data: validation.data };
      } catch (error) {
        return {
          success: false,
          error: error instanceof Error ? error.message : 'Failed to import',
        };
      }
    }
  );
}
