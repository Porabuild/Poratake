import { ipcMain, BrowserWindow, dialog, Notification, shell } from 'electron';
import { convertMp4ToGif } from '@/main/utils/ffmpeg';
import {
  rememberSaveDirectory,
  resolveSaveDialogPath,
} from '@/main/utils/save-location';
import {
  authorizeExportOutputPaths,
  getExportAbortSignal,
  isExportOutputPathAllowed,
} from './export-session';

function formatExportDuration(seconds: number): string {
  if (seconds < 60) {
    return `${Math.round(seconds)} seconds`;
  }
  const mins = Math.floor(seconds / 60);
  const secs = Math.round(seconds % 60);
  if (secs === 0) {
    return `${mins} minute${mins > 1 ? 's' : ''}`;
  }
  return `${mins}m ${secs}s`;
}

export function registerExportHandlers(): void {
  ipcMain.handle(
    'video-editor:show-save-dialog',
    async (
      event,
      { defaultName, format }: { defaultName: string; format?: 'mp4' | 'gif' }
    ): Promise<{ canceled: boolean; filePath?: string }> => {
      const parentWindow = BrowserWindow.fromWebContents(event.sender);
      const filters =
        format === 'gif'
          ? [{ name: 'GIF Image', extensions: ['gif'] }]
          : [{ name: 'MP4 Video', extensions: ['mp4'] }];
      const options = {
        defaultPath: resolveSaveDialogPath('video', defaultName),
        filters,
      };
      const result = parentWindow
        ? await dialog.showSaveDialog(parentWindow, options)
        : await dialog.showSaveDialog(options);

      if (!result.canceled && result.filePath) {
        const filePath =
          format === 'gif' && !result.filePath.match(/\.gif$/i)
            ? `${result.filePath.replace(/\.[^/.]+$/, '')}.gif`
            : result.filePath;
        const outputPaths =
          format === 'gif'
            ? [filePath, filePath.replace(/\.gif$/i, '-temp.mp4')]
            : [filePath];
        authorizeExportOutputPaths(event.sender, outputPaths);
        rememberSaveDirectory('video', filePath);

        return { canceled: false, filePath };
      }

      return {
        canceled: result.canceled,
      };
    }
  );

  ipcMain.handle(
    'video-export:show-completion',
    async (
      event,
      {
        durationSeconds,
        filePath,
        openInFinder,
      }: { durationSeconds: number; filePath?: string; openInFinder?: boolean }
    ): Promise<void> => {
      const notification = new Notification({
        title: 'Export Complete',
        body: `Video exported successfully in ${formatExportDuration(durationSeconds)}`,
      });
      notification.show();

      if (
        openInFinder &&
        filePath &&
        isExportOutputPathAllowed(event.sender.id, filePath)
      ) {
        shell.showItemInFolder(filePath);
      }
    }
  );

  ipcMain.handle(
    'video-editor:convert-to-gif',
    async (
      event,
      {
        inputPath,
        outputPath,
        resolution,
        frameRate,
      }: {
        inputPath: string;
        outputPath: string;
        resolution: 'original' | '4k' | '1080p' | '720p' | '480p';
        frameRate: string;
      }
    ): Promise<{ success: boolean; error?: string; outputPath?: string }> => {
      try {
        if (
          !isExportOutputPathAllowed(event.sender.id, inputPath) ||
          !isExportOutputPathAllowed(event.sender.id, outputPath)
        ) {
          return { success: false, error: 'Export path is not authorized' };
        }
        const result = await convertMp4ToGif({
          inputPath,
          outputPath,
          resolution,
          frameRate,
          abortSignal: getExportAbortSignal(event.sender.id),
        });
        if (result.success) {
          return { success: true, outputPath: result.outputPath };
        }
        return { success: false, error: result.message };
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );
}
