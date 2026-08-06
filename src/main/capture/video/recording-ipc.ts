import { ipcMain, dialog, BrowserWindow } from 'electron';
import { pauseTimer, resumeTimer } from './recording-control.ts';
import {
  getRecordingState,
  getRecordingDuration,
  getCurrentRecordingPath,
  pauseRecording,
  resumeRecording,
} from './recorder.ts';
import { hasPendingSelection } from '@/main/capture/area-selector';
import {
  stopRecordingAction,
  startPendingRecording,
  cancelPendingRecording,
  deleteRecordingAction,
  restartRecordingAction,
  type RecordingOptions,
} from './recording-actions.ts';
import { confirmVideoDelete } from './delete-video.ts';
import { createVideoEditorWindow } from './video-editor.ts';
import type { HistoryItem } from '@/types/history.ts';

export function registerRecordingIpcHandlers(): void {
  ipcMain.on('recording:stop', async () => {
    try {
      await stopRecordingAction();
    } catch (error) {
      console.error('Error stopping recording:', error);
    }
  });

  ipcMain.on('recording:pause', async () => {
    try {
      await pauseRecording();
      pauseTimer();
    } catch (error) {
      console.error('Error pausing recording:', error);
    }
  });

  ipcMain.on('recording:resume', async () => {
    try {
      await resumeRecording();
      resumeTimer();
    } catch (error) {
      console.error('Error resuming recording:', error);
    }
  });

  ipcMain.on(
    'recording:start-pending',
    async (_event, options?: RecordingOptions) => {
      try {
        await startPendingRecording(options);
      } catch (error) {
        console.error('Error starting pending recording:', error);
      }
    }
  );

  ipcMain.on('recording:cancel-pending', () => {
    cancelPendingRecording();
  });

  ipcMain.on('recording:delete', async () => {
    try {
      await deleteRecordingAction();
    } catch (error) {
      console.error('Error deleting recording:', error);
    }
  });

  ipcMain.on('recording:restart', async () => {
    try {
      await restartRecordingAction();
    } catch (error) {
      console.error('Error restarting recording:', error);
    }
  });

  ipcMain.handle('recording:getState', () => {
    return {
      state: getRecordingState(),
      duration: getRecordingDuration(),
      outputPath: getCurrentRecordingPath(),
      hasPendingSelection: hasPendingSelection(),
    };
  });

  ipcMain.handle(
    'recording:confirmAction',
    async (event, action: 'delete' | 'restart') => {
      const wasRecording = getRecordingState() === 'recording';

      if (wasRecording) {
        await pauseRecording();
        pauseTimer();
        await new Promise(resolve => setTimeout(resolve, 150));
      }

      let confirmed = false;
      const isDelete = action === 'delete';
      const parentWindow = BrowserWindow.fromWebContents(event.sender);

      if (isDelete) {
        confirmed = await confirmVideoDelete(parentWindow);
      } else {
        const options = {
          type: 'none' as const,
          title: 'Restart Recording?',
          message: 'Restart Recording?',
          detail:
            'This will discard the current recording and start a new one. This action cannot be undone.',
          buttons: ['Cancel', 'Restart'],
          defaultId: 1,
          cancelId: 0,
        };

        const result = parentWindow
          ? await dialog.showMessageBox(parentWindow, options)
          : await dialog.showMessageBox(options);
        confirmed = result.response === 1;
      }

      if (!confirmed && wasRecording) {
        await new Promise(resolve => setTimeout(resolve, 200));
        await resumeRecording();
        resumeTimer();
      }

      return confirmed;
    }
  );

  ipcMain.on('history:openVideo', (_event, item: HistoryItem) => {
    if (item.type === 'video') {
      createVideoEditorWindow(item.originalPath);
    }
  });
}
