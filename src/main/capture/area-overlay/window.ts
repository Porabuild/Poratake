import { app, BrowserWindow } from 'electron';
import type { Display } from 'electron';
import { randomUUID } from 'crypto';
import fs from 'fs';
import path from 'path';
import { isDev, devServerUrl } from '@/main/utils/env';

const PREVIEW_PREFIX = 'capty-frozen-';
const PREVIEW_EXTENSION = '.bmp';

export function previewPathFor(display: Display): string {
  return path.join(
    app.getPath('temp'),
    `${PREVIEW_PREFIX}${display.id}-${randomUUID()}${PREVIEW_EXTENSION}`
  );
}

export function removePreview(previewPath: string): void {
  try {
    fs.rmSync(previewPath, { force: true });
  } catch (error) {
    console.error('Failed to remove the frozen frame:', error);
  }
}

export function sweepStalePreviews(): void {
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

export function createOverlayWindow(
  display: Display,
  freeze: boolean
): BrowserWindow {
  const overlayWindow = new BrowserWindow({
    x: display.bounds.x,
    y: display.bounds.y,
    width: display.bounds.width,
    height: display.bounds.height,
    frame: false,
    thickFrame: false,
    transparent: !freeze,
    backgroundColor: freeze ? '#000000' : '#00000000',
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
