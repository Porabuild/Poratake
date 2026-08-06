import { ipcMain, BrowserWindow, dialog } from 'electron';
import { getWindowData } from '../window-manager';
import { confirmVideoDelete, deleteVideo } from '../delete-video';

export function registerDialogHandlers(): void {
  ipcMain.on('video-editor:close-confirmed', event => {
    const data = getWindowData(event.sender.id);
    if (data && !data.window.isDestroyed()) {
      data.isClosingConfirmed = true;
      data.window.close();
    }
  });

  ipcMain.handle('video-editor:confirmDelete', () => {
    return confirmVideoDelete();
  });

  ipcMain.handle('video-editor:confirmReset', async event => {
    const parentWindow = BrowserWindow.fromWebContents(event.sender);
    const options = {
      type: 'warning' as const,
      title: 'Reset to Defaults',
      message: 'Reset all changes?',
      detail:
        'This will discard all edits (trims, cuts, zoom, cursor, and camera settings) and reset to the original state.',
      buttons: ['Reset', 'Cancel'],
      defaultId: 1,
      cancelId: 1,
    };

    const result = parentWindow
      ? await dialog.showMessageBox(parentWindow, options)
      : await dialog.showMessageBox(options);

    return result.response === 0;
  });

  ipcMain.on('video-editor:delete', async event => {
    const data = getWindowData(event.sender.id);
    if (!data) return;

    const videoPath = data.filePath;

    if (!data.window.isDestroyed()) {
      data.isClosingConfirmed = true;
      data.window.close();
    }

    await deleteVideo(videoPath);
  });
}
