import { BrowserWindow, dialog, ipcMain, shell } from 'electron';
import { existsSync } from 'fs';
import type { VideoRecordingFeatures } from '@/types/history.ts';
import { getThumbnail } from '@/main/utils/thumbnails.ts';
import { isHistoryPopoverWebContents } from './popover';
import { EMPTY_VIDEO_FEATURES, getVideoRecordingFeatures } from './media.ts';
import {
  clearHistory,
  deleteHistoryItem,
  getHistoryItem,
  getHistorySummaries,
} from './store.ts';

export function registerHistoryIpc(): void {
  ipcMain.handle('history:get', event => {
    if (!isHistoryPopoverWebContents(event.sender)) return [];

    return getHistorySummaries();
  });

  ipcMain.handle('history:delete', async (event, id: string) => {
    if (!isHistoryPopoverWebContents(event.sender)) return false;

    return await deleteHistoryItem(id);
  });

  ipcMain.handle('history:reveal', (event, id: string): boolean => {
    if (!isHistoryPopoverWebContents(event.sender)) return false;

    const item = getHistoryItem(id);
    if (!item || !existsSync(item.originalPath)) return false;

    shell.showItemInFolder(item.originalPath);
    return true;
  });

  ipcMain.handle('history:clear', async event => {
    if (!isHistoryPopoverWebContents(event.sender)) return false;

    const parentWindow = BrowserWindow.fromWebContents(event.sender);
    const options = {
      type: 'warning' as const,
      title: 'Clear History',
      message: 'Are you sure you want to clear all history?',
      detail:
        'This will permanently delete all screenshots and videos from your history. This action cannot be undone.',
      buttons: ['Clear History', 'Cancel'],
      defaultId: 0,
      cancelId: 1,
    };

    const result = parentWindow
      ? await dialog.showMessageBox(parentWindow, options)
      : await dialog.showMessageBox(options);

    if (result.response !== 0) return false;

    return await clearHistory();
  });

  ipcMain.handle(
    'history:getThumbnail',
    async (event, id: string): Promise<string | null> => {
      if (!isHistoryPopoverWebContents(event.sender)) return null;

      const item = getHistoryItem(id);
      if (!item) return null;

      const result = await getThumbnail(item.originalPath, item.type);
      return result.base64;
    }
  );

  ipcMain.handle(
    'history:getVideoFeatures',
    (event, id: string): VideoRecordingFeatures => {
      if (!isHistoryPopoverWebContents(event.sender)) {
        return EMPTY_VIDEO_FEATURES;
      }

      const item = getHistoryItem(id);
      if (!item || item.type !== 'video') {
        return EMPTY_VIDEO_FEATURES;
      }

      return getVideoRecordingFeatures(item.originalPath);
    }
  );
}
