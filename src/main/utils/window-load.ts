import { ipcMain } from 'electron';
import type { WebContents } from 'electron';
import type { WindowLoadPayload } from '@/types/window-load';

const loadPayloads = new Map<number, WindowLoadPayload>();
const trackedWebContents = new Set<number>();

export function sendWindowLoad(
  webContents: WebContents,
  payload: WindowLoadPayload
): void {
  loadPayloads.set(webContents.id, payload);
  webContents.send('load', payload);

  if (trackedWebContents.has(webContents.id)) return;

  trackedWebContents.add(webContents.id);
  webContents.once('destroyed', () => {
    loadPayloads.delete(webContents.id);
    trackedWebContents.delete(webContents.id);
  });
}

export function initWindowLoad(): void {
  ipcMain.handle('window:get-load-data', event => {
    return loadPayloads.get(event.sender.id) ?? null;
  });
}
