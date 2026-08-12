import { ipcMain } from 'electron';
import { createVideoEditorWindow } from './video-editor.ts';
import { getHistoryItem, isHistoryPopoverWebContents } from '@/main/history';

export function registerRecordingIpcHandlers(): void {
  ipcMain.on('history:openVideo', (event, id: string) => {
    if (!isHistoryPopoverWebContents(event.sender)) return;

    const item = getHistoryItem(id);
    if (!item || item.type !== 'video') return;

    createVideoEditorWindow(item.originalPath);
  });
}
