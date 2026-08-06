import { ipcMain } from 'electron';
import { getWindowData, updateWindowFilePath } from '../window-manager';
import { renameRecordingProject, getProjectFolder } from '../recording-project';
import { updateHistoryItemPath } from '@/main/history';
import { getHistoryPopover } from '@/main/history/popover';
import { rekeyThumbnail } from '@/main/utils/thumbnails';
import type { ProjectRenameResult } from '@/types/video';

async function syncProjectPathChange(
  webContentsId: number,
  oldVideoPath: string,
  result: ProjectRenameResult
): Promise<ProjectRenameResult> {
  if (!result.success) return result;

  await updateHistoryItemPath(oldVideoPath, result.newVideoPath);
  rekeyThumbnail(oldVideoPath, result.newVideoPath);
  updateWindowFilePath(webContentsId, result.newVideoPath);

  const popover = getHistoryPopover();
  if (popover && !popover.isDestroyed()) {
    popover.webContents.send('history:refresh');
  }

  return result;
}

export function registerProjectHandlers(): void {
  ipcMain.handle(
    'project:rename',
    async (event, newName: string): Promise<ProjectRenameResult> => {
      const data = getWindowData(event.sender.id);
      if (!data) {
        return {
          success: false,
          newProjectPath: '',
          newVideoPath: '',
          error: 'No active project',
        };
      }

      const projectPath = getProjectFolder(data.filePath);
      if (!projectPath) {
        return {
          success: false,
          newProjectPath: data.filePath,
          newVideoPath: data.filePath,
          error: 'Not a recording project',
        };
      }

      const result = renameRecordingProject(projectPath, newName);
      return syncProjectPathChange(event.sender.id, data.filePath, result);
    }
  );
}
