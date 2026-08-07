import { app, BrowserWindow, ipcMain, screen } from 'electron';
import type { Display, Rectangle } from 'electron';
import fs from 'fs';
import path from 'path';
import { pathToFileURL } from 'url';
import { isDev, devServerUrl } from '@/main/utils/env';
import {
  captureRegionToFile,
  releaseRetainedDisplays,
} from '@/main/capture/screenshot/native-capture';
import type { AreaOverlayResult } from '@/types/area-overlay';

export interface OverlaySelection {
  display: Display;
  rect: Rectangle;
  release: () => Promise<void>;
}

interface OverlayWindowEntry {
  window: BrowserWindow;
  display: Display;
  previewPath: string;
  loaded: boolean;
  imageUrl: string | null;
}

interface OverlaySession {
  entries: OverlayWindowEntry[];
  displays: Map<number, Display>;
  displayIdsByWebContents: Map<number, number>;
  focusDisplayId: number;
  resolve: (selection: OverlaySelection | null) => void;
}

const PREVIEW_PREFIX = 'capty-frozen-';
const PREVIEW_EXTENSION = '.bmp';

let activeSession: OverlaySession | null = null;
let ipcRegistered = false;

function registerOverlayIpc(): void {
  if (ipcRegistered) return;
  ipcRegistered = true;

  ipcMain.on('area-overlay:ready', event => {
    const session = activeSession;
    if (!session) return;

    const displayId = session.displayIdsByWebContents.get(event.sender.id);
    if (displayId === undefined) return;

    revealOverlay(session, displayId);
  });

  ipcMain.on('area-overlay:confirm', (event, result: unknown) => {
    const session = activeSession;
    if (!session) return;

    const displayId = session.displayIdsByWebContents.get(event.sender.id);
    if (displayId === undefined) return;

    const display = session.displays.get(displayId);
    if (!display || !isValidResult(result, display)) return;

    finishSession(result, session);
  });

  ipcMain.on('area-overlay:cancel', event => {
    const session = activeSession;
    if (!session?.displayIdsByWebContents.has(event.sender.id)) return;
    finishSession(null, session);
  });
}

function revealOverlay(session: OverlaySession, displayId: number): void {
  const entry = session.entries.find(item => item.display.id === displayId);
  if (!entry || entry.window.isDestroyed() || entry.window.isVisible()) return;

  entry.window.showInactive();

  if (displayId === session.focusDisplayId) {
    entry.window.focus();
  }
}

function deliverFrozenFrame(entry: OverlayWindowEntry): void {
  if (!entry.loaded || !entry.imageUrl || entry.window.isDestroyed()) return;

  entry.window.webContents.send('load', {
    type: 'area-overlay',
    params: {
      displayId: entry.display.id,
      imageUrl: entry.imageUrl,
    },
  });
}

function isValidResult(
  result: unknown,
  display: Display
): result is AreaOverlayResult {
  if (!result || typeof result !== 'object') return false;

  const value = result as Partial<AreaOverlayResult>;
  if (value.displayId !== display.id) return false;

  const coordinates = [value.x, value.y, value.width, value.height];
  if (
    !coordinates.every(
      item => typeof item === 'number' && Number.isFinite(item)
    )
  ) {
    return false;
  }

  const { x, y, width, height } = value as AreaOverlayResult;
  if (x < 0 || y < 0 || width <= 0 || height <= 0) return false;

  return (
    x + width <= display.bounds.width && y + height <= display.bounds.height
  );
}

function discardEntry(entry: OverlayWindowEntry): void {
  if (!entry.window.isDestroyed()) {
    entry.window.destroy();
  }

  try {
    fs.rmSync(entry.previewPath, { force: true });
  } catch (error) {
    console.error('Failed to remove the frozen frame:', error);
  }
}

function sweepStalePreviews(): void {
  const directory = app.getPath('temp');

  try {
    for (const name of fs.readdirSync(directory)) {
      if (name.startsWith(PREVIEW_PREFIX) && name.endsWith(PREVIEW_EXTENSION)) {
        fs.rmSync(path.join(directory, name), { force: true });
      }
    }
  } catch (error) {
    console.error('Failed to clear stale frozen frames:', error);
  }
}

function finishSession(
  result: AreaOverlayResult | null,
  expectedSession: OverlaySession | null = activeSession
): void {
  const session = activeSession;
  if (!session || session !== expectedSession) return;
  activeSession = null;

  session.entries.forEach(discardEntry);

  const display = result ? session.displays.get(result.displayId) : undefined;

  if (!result || !display) {
    void releaseRetainedDisplays();
    session.resolve(null);
    return;
  }

  session.resolve({
    display,
    rect: {
      x: display.bounds.x + result.x,
      y: display.bounds.y + result.y,
      width: result.width,
      height: result.height,
    },
    release: releaseRetainedDisplays,
  });
}

function createOverlayWindow(display: Display): BrowserWindow {
  const overlayWindow = new BrowserWindow({
    x: display.bounds.x,
    y: display.bounds.y,
    width: display.bounds.width,
    height: display.bounds.height,
    frame: false,
    // Drop WS_THICKFRAME so DWM does not play its zoom-in animation on show.
    thickFrame: false,
    transparent: false,
    backgroundColor: '#000000',
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    show: false,
    paintWhenInitiallyHidden: true,
    roundedCorners: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      webSecurity: false,
      devTools: isDev,
    },
  });

  overlayWindow.setAlwaysOnTop(true, 'screen-saver');
  overlayWindow.setBounds(display.bounds);

  if (devServerUrl) {
    overlayWindow.loadURL(devServerUrl);
  } else {
    overlayWindow.loadFile(path.join(__dirname, '../dist/index.html'));
  }

  return overlayWindow;
}

export function isOverlayActive(): boolean {
  return activeSession !== null;
}

export function cancelOverlaySelection(): void {
  finishSession(null);
}

function previewPathFor(display: Display): string {
  return path.join(
    app.getPath('temp'),
    `${PREVIEW_PREFIX}${display.id}-${Date.now()}${PREVIEW_EXTENSION}`
  );
}

export async function selectAreaWithOverlay(): Promise<OverlaySelection | null> {
  if (activeSession) {
    return null;
  }

  registerOverlayIpc();
  sweepStalePreviews();

  const displays = screen.getAllDisplays();
  let resolveSession!: (selection: OverlaySelection | null) => void;
  const resultPromise = new Promise<OverlaySelection | null>(resolve => {
    resolveSession = resolve;
  });
  const session: OverlaySession = {
    entries: [],
    displays: new Map(displays.map(display => [display.id, display])),
    displayIdsByWebContents: new Map(),
    focusDisplayId: screen.getDisplayNearestPoint(screen.getCursorScreenPoint())
      .id,
    resolve: resolveSession,
  };
  activeSession = session;

  try {
    for (const display of displays) {
      const window = createOverlayWindow(display);
      const entry: OverlayWindowEntry = {
        window,
        display,
        previewPath: previewPathFor(display),
        loaded: false,
        imageUrl: null,
      };
      session.entries.push(entry);
      session.displayIdsByWebContents.set(window.webContents.id, display.id);

      window.webContents.on('did-finish-load', () => {
        entry.loaded = true;
        deliverFrozenFrame(entry);
      });

      window.webContents.on('render-process-gone', () => {
        finishSession(null, session);
      });

      window.on('closed', () => {
        if (activeSession === session) {
          finishSession(null, session);
        }
      });
    }

    await Promise.all(
      session.entries.map(async entry => {
        const captured = await captureRegionToFile(
          entry.display.bounds,
          entry.previewPath,
          { retain: true }
        );

        if (activeSession !== session) return;

        if (!captured) {
          discardEntry(entry);
          session.entries = session.entries.filter(item => item !== entry);
          return;
        }

        entry.imageUrl = pathToFileURL(entry.previewPath).href;
        deliverFrozenFrame(entry);
      })
    );

    if (activeSession === session && session.entries.length === 0) {
      finishSession(null, session);
    }

    return resultPromise;
  } catch (error) {
    finishSession(null, session);
    throw error;
  }
}

export async function captureAreaToFile(filePath: string): Promise<boolean> {
  const selection = await selectAreaWithOverlay();
  if (!selection) {
    return false;
  }

  try {
    return await captureRegionToFile(selection.rect, filePath, {
      cached: true,
    });
  } finally {
    await selection.release();
  }
}
