import { ipcMain } from 'electron';
import fs from 'fs';
import { getWindowData } from '../window-manager';
import { probeVideo } from '@/main/utils/ffmpeg';
import type { VideoMetadata } from '@/types/video';

export function registerMetadataHandlers(): void {
  ipcMain.handle('video-editor:getVideoFileSize', event => {
    const data = getWindowData(event.sender.id);
    if (!data || !fs.existsSync(data.filePath)) {
      return null;
    }
    try {
      const stats = fs.statSync(data.filePath);
      return stats.size;
    } catch {
      return null;
    }
  });

  ipcMain.handle(
    'video-editor:getVideoMetadata',
    async (event): Promise<VideoMetadata | null> => {
      const data = getWindowData(event.sender.id);
      if (!data || !fs.existsSync(data.filePath)) {
        return null;
      }

      const result = await probeVideo(data.filePath);
      return result?.metadata ?? null;
    }
  );
}
