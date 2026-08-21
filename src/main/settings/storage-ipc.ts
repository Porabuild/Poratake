import { app, dialog, ipcMain } from 'electron';
import fs from 'fs';
import path from 'path';
import {
  generateFilename,
  getAvailableTokens,
  validateNamingPattern,
  type CaptureType,
} from '@/main/utils/filename-generator';
import { getConfig } from './store.ts';

export function registerStorageIpc(): void {
  ipcMain.handle(
    'storage:selectPath',
    async (_event, type: 'screenshots' | 'recordings') => {
      const defaultPath =
        type === 'screenshots'
          ? app.getPath('pictures')
          : app.getPath('videos');

      const result = await dialog.showOpenDialog({
        properties: ['openDirectory', 'createDirectory'],
        defaultPath,
        title: `Select ${type === 'screenshots' ? 'Screenshots' : 'Recordings'} Folder`,
      });

      if (result.canceled || result.filePaths.length === 0) {
        return null;
      }

      const selectedPath = result.filePaths[0];
      const validation = validateStoragePath(selectedPath);

      if (!validation.valid) {
        return { error: validation.error };
      }

      return { path: selectedPath };
    }
  );

  ipcMain.handle('storage:validatePattern', (_event, pattern: string) => {
    return validateNamingPattern(pattern);
  });

  ipcMain.handle(
    'storage:previewFilename',
    (_event, pattern: string, type: CaptureType) => {
      const extension = type === 'Screenshot' ? 'png' : 'mov';
      return generateFilename({ pattern, type, extension });
    }
  );

  ipcMain.handle('storage:getTokens', () => {
    return getAvailableTokens();
  });

  ipcMain.handle('storage:getDefaultPaths', () => {
    const storage = getConfig().storage;
    return {
      screenshots:
        storage.screenshotsPath ||
        path.join(app.getPath('pictures'), 'Poratake'),
      recordings:
        storage.recordingsPath || path.join(app.getPath('videos'), 'Poratake'),
    };
  });

  ipcMain.handle('cursor:selectImage', async () => {
    try {
      const result = await dialog.showOpenDialog({
        properties: ['openFile'],
        filters: [
          {
            name: 'Images',
            extensions: ['png', 'jpg', 'jpeg', 'svg', 'webp', 'gif'],
          },
        ],
      });

      if (result.canceled || result.filePaths.length === 0) {
        return null;
      }

      const filePath = result.filePaths[0];
      const imageBuffer = fs.readFileSync(filePath);
      const ext = path.extname(filePath).toLowerCase();

      const mimeType =
        ext === '.png'
          ? 'image/png'
          : ext === '.jpg' || ext === '.jpeg'
            ? 'image/jpeg'
            : ext === '.svg'
              ? 'image/svg+xml'
              : ext === '.webp'
                ? 'image/webp'
                : ext === '.gif'
                  ? 'image/gif'
                  : 'image/png';

      const base64 = imageBuffer.toString('base64');
      return `data:${mimeType};base64,${base64}`;
    } catch (error) {
      console.error('Failed to select cursor image:', error);
      return null;
    }
  });
}

function validateStoragePath(dirPath: string): {
  valid: boolean;
  error?: string;
} {
  try {
    if (!fs.existsSync(dirPath)) {
      fs.mkdirSync(dirPath, { recursive: true });
    }

    const stats = fs.statSync(dirPath);
    if (!stats.isDirectory()) {
      return { valid: false, error: 'Selected path is not a directory' };
    }

    const testFile = path.join(dirPath, `.poratake-test-${Date.now()}`);
    fs.writeFileSync(testFile, '');
    fs.unlinkSync(testFile);

    return { valid: true };
  } catch (error) {
    return {
      valid: false,
      error:
        error instanceof Error ? error.message : 'Unable to access directory',
    };
  }
}
