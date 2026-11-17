import { ipcMain, BrowserWindow, dialog } from 'electron';
import { getWindowData } from '../window-manager';
import { confirmVideoDelete, deleteVideo } from '../delete-video';

export function registerDialogHandlers(): void {
  ipcMain.on('video-editor:reset-conflict', event => {
    const data = getWindowData(event.sender.id);
    if (!data || data.closeSavePending || data.window.isDestroyed()) return;
    void dialog
      .showMessageBox(data.window, {
        type: 'info',
        message: 'Reset was interrupted by a new edit.',
        detail: 'Your edits remain in the editor. Try resetting again.',
        buttons: ['OK'],
      })
      .catch(error => console.error('Failed to show reset message:', error));
  });

  ipcMain.on('video-editor:close-handler-ready', event => {
    const data = getWindowData(event.sender.id);
    if (data) data.closeHandlerReady = true;
  });

  ipcMain.on('video-editor:close-save-failed', event => {
    const data = getWindowData(event.sender.id);
    if (!data || !data.closeSavePending || data.window.isDestroyed()) return;
    data.closeSavePending = false;
    void dialog
      .showMessageBox(data.window, {
        type: 'error',
        message: 'Could not save your video edits.',
        detail: 'The editor remains open so you can try saving again.',
        buttons: ['OK'],
      })
      .catch(error => console.error('Failed to show save error:', error));
  });

  ipcMain.on('video-editor:close-confirmed', event => {
    const data = getWindowData(event.sender.id);
    if (data?.closeSavePending && !data.window.isDestroyed()) {
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

    if (data.isExporting) {
      await dialog.showMessageBox(data.window, {
        type: 'info',
        message:
          'Wait for the video export to finish before deleting the recording.',
        buttons: ['OK'],
      });
      return;
    }

    const videoPath = data.filePath;

    if (!data.window.isDestroyed()) {
      data.isClosingConfirmed = true;
      data.window.close();
    }

    await deleteVideo(videoPath);
  });
}
