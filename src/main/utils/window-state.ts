import type { BrowserWindow } from 'electron';
import fs from 'fs';
import {
  getConfigDir,
  getWindowStateFilePath,
  ensureDirectoryExists,
} from './paths';

export type WindowStateId = 'screenshot-editor' | 'video-editor';

export interface WindowSize {
  width: number;
  height: number;
}

const PERSIST_DEBOUNCE_MS = 400;

let cachedState: Record<string, WindowSize> | null = null;

function isWindowSize(value: unknown): value is WindowSize {
  if (!value || typeof value !== 'object') return false;

  const { width, height } = value as Partial<WindowSize>;
  return (
    typeof width === 'number' &&
    typeof height === 'number' &&
    Number.isFinite(width) &&
    Number.isFinite(height) &&
    width > 0 &&
    height > 0
  );
}

function readState(): Record<string, WindowSize> {
  if (cachedState) {
    return cachedState;
  }

  cachedState = {};

  try {
    const filePath = getWindowStateFilePath();
    if (!fs.existsSync(filePath)) {
      return cachedState;
    }

    const parsed = JSON.parse(fs.readFileSync(filePath, 'utf-8'));
    for (const [id, size] of Object.entries(parsed ?? {})) {
      if (isWindowSize(size)) {
        cachedState[id] = { width: size.width, height: size.height };
      }
    }
  } catch (error) {
    console.error('Failed to read window state:', error);
  }

  return cachedState;
}

function writeState(id: WindowStateId, size: WindowSize): void {
  const state = readState();
  if (state[id]?.width === size.width && state[id]?.height === size.height) {
    return;
  }

  state[id] = size;

  try {
    ensureDirectoryExists(getConfigDir());
    fs.writeFileSync(
      getWindowStateFilePath(),
      JSON.stringify(state, null, 2),
      'utf-8'
    );
  } catch (error) {
    console.error('Failed to save window state:', error);
  }
}

export function getStoredWindowSize(id: WindowStateId): WindowSize | null {
  return readState()[id] ?? null;
}

export function trackWindowSize(
  id: WindowStateId,
  window: BrowserWindow
): void {
  let timer: NodeJS.Timeout | null = null;

  const clearTimer = () => {
    if (!timer) return;
    clearTimeout(timer);
    timer = null;
  };

  const persist = () => {
    clearTimer();
    if (window.isDestroyed() || window.isMinimized() || window.isMaximized()) {
      return;
    }

    const [width, height] = window.getSize();
    writeState(id, { width, height });
  };

  window.on('resize', () => {
    clearTimer();
    timer = setTimeout(persist, PERSIST_DEBOUNCE_MS);
  });
  window.on('close', persist);
  window.once('closed', clearTimer);
}
