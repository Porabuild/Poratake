import { ipcMain, dialog, BrowserWindow } from 'electron';
import fs from 'fs/promises';
import { existsSync, mkdirSync, unlinkSync, rmSync } from 'fs';
import crypto from 'crypto';
import type {
  HistoryItem,
  HistoryItemSummary,
  HistoryItemType,
  EditorState,
  VideoRecordingFeatures,
} from '@/types/history.ts';
import { getConfigDir, getHistoryFilePath } from '@/main/utils/paths.ts';
import {
  getProjectFolder,
  getMicAudioPath,
  getSystemAudioPath,
  getCameraVideoPath,
  getCursorPath,
} from '@/main/capture/video/recording-project.ts';

export {
  preloadHistoryPopover,
  showHistoryPopover,
  closeHistoryPopover,
  toggleHistoryPopover,
  getHistoryPopover,
  isHistoryPopoverWebContents,
  isHistoryPopoverVisible,
} from './popover';
import { isHistoryPopoverWebContents } from './popover';
import { getConfig } from '../settings';
import {
  getThumbnail,
  deleteThumbnail,
  clearAllThumbnails,
} from '@/main/utils/thumbnails.ts';

const CONFIG_DIR = getConfigDir();
const HISTORY_FILE = getHistoryFilePath();

let historyItems: HistoryItem[] = [];
let historyLoadPromise: Promise<HistoryItem[]> | null = null;

let writeQueue: Promise<void> = Promise.resolve();

function ensureDirectories() {
  if (!existsSync(CONFIG_DIR)) {
    mkdirSync(CONFIG_DIR, { recursive: true });
  }
}

async function readHistory(): Promise<HistoryItem[]> {
  try {
    ensureDirectories();
    if (!existsSync(HISTORY_FILE)) {
      historyItems = [];
      return historyItems;
    }

    const fileContent = await fs.readFile(HISTORY_FILE, 'utf-8');
    const storedItems = JSON.parse(fileContent) as HistoryItem[];
    historyItems = storedItems
      .filter(item => existsSync(item.originalPath))
      .map(item => ({
        ...item,
        type: item.type || ('screenshot' as const),
      }));
  } catch (error) {
    console.error('Failed to load history:', error);
    historyItems = [];
  }
  return historyItems;
}

export function loadHistory(): Promise<HistoryItem[]> {
  historyLoadPromise ??= readHistory();
  return historyLoadPromise;
}

async function saveHistoryToFile(): Promise<void> {
  writeQueue = writeQueue.then(async () => {
    try {
      ensureDirectories();
      await fs.writeFile(
        HISTORY_FILE,
        JSON.stringify(historyItems, null, 2),
        'utf-8'
      );
    } catch (error) {
      console.error('Failed to save history:', error);
    }
  });
  return writeQueue;
}

export async function addToHistory(
  originalPath: string,
  type: HistoryItemType = 'screenshot',
  duration?: number
): Promise<HistoryItem | null> {
  const config = getConfig();
  if (!config.history.enabled) {
    return null;
  }
  await loadHistory();

  try {
    const item: HistoryItem = {
      id: crypto.randomUUID(),
      timestamp: Date.now(),
      originalPath,
      type,
      editorState: null,
      ...(duration !== undefined && { duration }),
    };

    historyItems.unshift(item);

    while (historyItems.length > config.history.maxItems) {
      const removed = historyItems.pop();
      if (removed) {
        cleanupHistoryItem(removed);
      }
    }

    await saveHistoryToFile();
    return item;
  } catch (error) {
    console.error('Failed to add to history:', error);
    return null;
  }
}

export async function updateHistoryItem(
  id: string,
  editorState: EditorState
): Promise<HistoryItem | null> {
  await loadHistory();
  const index = historyItems.findIndex(item => item.id === id);
  if (index === -1) {
    return null;
  }

  historyItems[index] = {
    ...historyItems[index],
    editorState,
  };

  await saveHistoryToFile();
  return historyItems[index];
}

export async function updateHistoryItemByPath(
  originalPath: string,
  editorState: EditorState
): Promise<HistoryItem | null> {
  await loadHistory();
  const index = historyItems.findIndex(
    item => item.originalPath === originalPath
  );
  if (index === -1) {
    return null;
  }

  historyItems[index] = {
    ...historyItems[index],
    editorState,
  };

  await saveHistoryToFile();
  return historyItems[index];
}

export async function updateHistoryItemPath(
  oldPath: string,
  newPath: string
): Promise<boolean> {
  await loadHistory();
  const index = historyItems.findIndex(item => item.originalPath === oldPath);
  if (index === -1) {
    return false;
  }

  historyItems[index] = {
    ...historyItems[index],
    originalPath: newPath,
  };

  await saveHistoryToFile();
  return true;
}

function cleanupHistoryItem(item: HistoryItem): void {
  try {
    const projectFolder = getProjectFolder(item.originalPath);

    if (projectFolder && existsSync(projectFolder)) {
      deleteThumbnail(item.originalPath);
      rmSync(projectFolder, { recursive: true, force: true });
      return;
    }

    if (existsSync(item.originalPath)) {
      unlinkSync(item.originalPath);
    }
    deleteThumbnail(item.originalPath);
    if (item.type === 'video') {
      const cursorDataPath = item.originalPath.replace(
        /\.[^.]+$/,
        '.cursor.json'
      );
      if (existsSync(cursorDataPath)) {
        unlinkSync(cursorDataPath);
      }
      const mouseDataPath = item.originalPath.replace(/\.mov$/, '.mouse.json');
      if (existsSync(mouseDataPath)) {
        unlinkSync(mouseDataPath);
      }
      const cameraJsonPath = item.originalPath.replace(
        /\.[^.]+$/,
        '.camera.json'
      );
      if (existsSync(cameraJsonPath)) {
        unlinkSync(cameraJsonPath);
      }
      const cameraVideoPath = item.originalPath.replace(
        /\.[^.]+$/,
        '.camera.mov'
      );
      if (existsSync(cameraVideoPath)) {
        unlinkSync(cameraVideoPath);
      }
    }
  } catch {
    console.warn(`Failed to clean up history item: ${item.id}`);
  }
}

export async function deleteHistoryItem(id: string): Promise<boolean> {
  await loadHistory();
  const index = historyItems.findIndex(item => item.id === id);
  if (index === -1) {
    return false;
  }

  const removed = historyItems.splice(index, 1)[0];
  cleanupHistoryItem(removed);
  await saveHistoryToFile();
  return true;
}

export async function clearHistory(): Promise<void> {
  await loadHistory();
  for (const item of historyItems) {
    cleanupHistoryItem(item);
  }
  clearAllThumbnails();
  historyItems = [];
  await saveHistoryToFile();
}

export function getHistory(): HistoryItem[] {
  return historyItems;
}

export function getHistorySummaries(): HistoryItemSummary[] {
  return historyItems.map(({ id, timestamp, type, duration }) => ({
    id,
    timestamp,
    type,
    ...(duration !== undefined && { duration }),
  }));
}

export function getHistoryItem(id: string): HistoryItem | null {
  return historyItems.find(item => item.id === id) || null;
}

export function getHistoryItemByPath(originalPath: string): HistoryItem | null {
  return historyItems.find(item => item.originalPath === originalPath) || null;
}

export function getVideoRecordingFeatures(
  originalPath: string
): VideoRecordingFeatures {
  const projectFolder = getProjectFolder(originalPath);
  if (!projectFolder) {
    return {
      hasMic: false,
      hasSystemAudio: false,
      hasCamera: false,
      hasCursor: false,
    };
  }

  return {
    hasMic: existsSync(getMicAudioPath(originalPath)),
    hasSystemAudio: existsSync(getSystemAudioPath(originalPath)),
    hasCamera: existsSync(getCameraVideoPath(originalPath)),
    hasCursor: existsSync(getCursorPath(originalPath)),
  };
}

export async function init(): Promise<void> {
  await loadHistory();

  ipcMain.handle('history:get', event => {
    if (!isHistoryPopoverWebContents(event.sender)) return [];

    return getHistorySummaries();
  });

  ipcMain.handle('history:delete', async (event, id: string) => {
    if (!isHistoryPopoverWebContents(event.sender)) return false;

    return await deleteHistoryItem(id);
  });

  ipcMain.handle('history:clear', async event => {
    if (!isHistoryPopoverWebContents(event.sender)) return false;

    const parentWindow = BrowserWindow.fromWebContents(event.sender);
    const options = {
      type: 'warning' as const,
      title: 'Clear History',
      message: 'Are you sure you want to clear all history?',
      detail:
        'This will permanently delete all screenshots and videos from your history. This action cannot be undone.',
      buttons: ['Clear History', 'Cancel'],
      defaultId: 0,
      cancelId: 1,
    };

    const result = parentWindow
      ? await dialog.showMessageBox(parentWindow, options)
      : await dialog.showMessageBox(options);

    if (result.response !== 0) return false;

    await clearHistory();
    return true;
  });

  ipcMain.handle(
    'history:getThumbnail',
    async (event, id: string): Promise<string | null> => {
      if (!isHistoryPopoverWebContents(event.sender)) return null;

      const item = getHistoryItem(id);
      if (!item) return null;

      const result = await getThumbnail(item.originalPath, item.type);
      return result.base64;
    }
  );

  ipcMain.handle(
    'history:getVideoFeatures',
    (event, id: string): VideoRecordingFeatures => {
      if (!isHistoryPopoverWebContents(event.sender)) {
        return {
          hasMic: false,
          hasSystemAudio: false,
          hasCamera: false,
          hasCursor: false,
        };
      }

      const item = getHistoryItem(id);
      if (!item || item.type !== 'video') {
        return {
          hasMic: false,
          hasSystemAudio: false,
          hasCamera: false,
          hasCursor: false,
        };
      }

      return getVideoRecordingFeatures(item.originalPath);
    }
  );
}
