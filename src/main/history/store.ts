import crypto from 'crypto';
import { existsSync, mkdirSync } from 'fs';
import fs from 'fs/promises';
import type {
  EditorState,
  HistoryItem,
  HistoryItemSummary,
  HistoryItemType,
} from '@/types/history.ts';
import { getConfigDir, getHistoryFilePath } from '@/main/utils/paths.ts';
import { getConfig } from '@/main/settings';
import { clearAllThumbnails } from '@/main/utils/thumbnails.ts';
import { cleanupHistoryItem, prepareHistoryEditorState } from './media.ts';

const CONFIG_DIR = getConfigDir();
const HISTORY_FILE = getHistoryFilePath();

let historyItems: HistoryItem[] = [];
let historyLoadPromise: Promise<HistoryItem[]> | null = null;
let writeQueue: Promise<void> = Promise.resolve();
let releaseHistoryFile: (filePath: string) => Promise<void> = async () => {};

export function setHistoryFileReleaseHandler(
  handler: (filePath: string) => Promise<void>
): void {
  releaseHistoryFile = handler;
}

function ensureDirectories(): void {
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
    historyItems = storedItems.map(item => ({
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
      if (!removed) break;
      if (await cleanupHistoryItem(removed, releaseHistoryFile)) continue;

      historyItems.push(removed);
      break;
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
    editorState: prepareHistoryEditorState(editorState),
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
    editorState: prepareHistoryEditorState(editorState),
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

export async function deleteHistoryItem(id: string): Promise<boolean> {
  await loadHistory();
  const index = historyItems.findIndex(item => item.id === id);
  if (index === -1) {
    return false;
  }

  const item = historyItems[index];
  if (!(await cleanupHistoryItem(item, releaseHistoryFile))) {
    return false;
  }

  const currentIndex = historyItems.findIndex(
    historyItem => historyItem.id === id
  );
  if (currentIndex === -1) {
    return true;
  }

  historyItems.splice(currentIndex, 1);
  await saveHistoryToFile();
  return true;
}

export async function clearHistory(): Promise<boolean> {
  await loadHistory();
  const retainedItems: HistoryItem[] = [];
  for (const item of historyItems) {
    if (!(await cleanupHistoryItem(item, releaseHistoryFile))) {
      retainedItems.push(item);
    }
  }
  historyItems = retainedItems;
  if (retainedItems.length === 0) {
    clearAllThumbnails();
  }
  await saveHistoryToFile();
  return retainedItems.length === 0;
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
