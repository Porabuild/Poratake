import fs from 'fs';
import { openScreenshotWindow, getImageDimensions } from './open-editor.ts';
import type { HistoryItem } from '@/types/history.ts';

export function openScreenshotFromHistory(item: HistoryItem): void {
  if (!fs.existsSync(item.originalPath)) {
    console.error('Screenshot file not found:', item.originalPath);
    return;
  }

  const { width, height } = getImageDimensions(item.originalPath);

  openScreenshotWindow({
    filePath: item.originalPath,
    width,
    height,
    editorState: item.editorState || undefined,
    historyId: item.id,
  });
}
