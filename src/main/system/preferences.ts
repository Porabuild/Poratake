import { ipcMain } from 'electron';
import { openExternalUrl } from '@/main/utils/external-url';

export function init() {
  ipcMain.on('open-external', (_event, url: unknown) => {
    openExternalUrl(url);
  });
}
