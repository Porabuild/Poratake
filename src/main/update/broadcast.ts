import { BrowserWindow } from 'electron';

export function broadcastUpdateEvent(
  channel: string,
  ...args: unknown[]
): void {
  const windows = BrowserWindow.getAllWindows();
  for (const win of windows) {
    win.webContents.send(channel, ...args);
  }
}
