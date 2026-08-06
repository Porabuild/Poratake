import { ipcMain } from 'electron';
import fs from 'fs';

export function registerFileHandlers(): void {
  ipcMain.handle(
    'video-editor:delete-temp-file',
    async (
      _,
      { filePath }: { filePath: string }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        if (fs.existsSync(filePath)) {
          await fs.promises.unlink(filePath);
        }
        return { success: true };
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );

  ipcMain.handle(
    'file:write-buffer',
    async (
      _,
      { path, buffer }: { path: string; buffer: Uint8Array }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        await fs.promises.writeFile(path, buffer);
        return { success: true };
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );

  ipcMain.handle(
    'file:rename',
    async (
      _,
      { oldPath, newPath }: { oldPath: string; newPath: string }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        await fs.promises.rename(oldPath, newPath);
        return { success: true };
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );
}
