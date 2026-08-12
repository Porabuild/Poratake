import { ipcMain } from 'electron';
import fs from 'fs';
import { isExportOutputPathAllowed } from './export-session';

export function registerFileHandlers(): void {
  ipcMain.handle(
    'video-editor:delete-temp-file',
    async (
      event,
      { filePath }: { filePath: string }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        if (!isExportOutputPathAllowed(event.sender.id, filePath)) {
          return { success: false, error: 'Export path is not authorized' };
        }
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
    'file:create-output',
    async (
      event,
      { path }: { path: string }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        if (!isExportOutputPathAllowed(event.sender.id, path)) {
          return { success: false, error: 'Export path is not authorized' };
        }
        await fs.promises.writeFile(path, new Uint8Array());
        return { success: true };
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );

  ipcMain.handle(
    'file:write-output-chunk',
    async (
      event,
      {
        path,
        position,
        buffer,
      }: { path: string; position: number; buffer: Uint8Array }
    ): Promise<{ success: boolean; error?: string }> => {
      if (!isExportOutputPathAllowed(event.sender.id, path)) {
        return { success: false, error: 'Export path is not authorized' };
      }
      if (!Number.isSafeInteger(position) || position < 0) {
        return { success: false, error: 'Invalid output position' };
      }
      if (!(buffer instanceof Uint8Array)) {
        return { success: false, error: 'Invalid output buffer' };
      }

      let handle: fs.promises.FileHandle | null = null;
      try {
        handle = await fs.promises.open(path, 'r+');
        let offset = 0;
        while (offset < buffer.byteLength) {
          const { bytesWritten } = await handle.write(
            buffer,
            offset,
            buffer.byteLength - offset,
            position + offset
          );
          if (bytesWritten <= 0) {
            throw new Error('Failed to write output chunk');
          }
          offset += bytesWritten;
        }
        return { success: true };
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      } finally {
        await handle?.close().catch(() => {});
      }
    }
  );

  ipcMain.handle(
    'file:rename',
    async (
      event,
      { oldPath, newPath }: { oldPath: string; newPath: string }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        if (
          !isExportOutputPathAllowed(event.sender.id, oldPath) ||
          !isExportOutputPathAllowed(event.sender.id, newPath)
        ) {
          return { success: false, error: 'Export path is not authorized' };
        }
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
