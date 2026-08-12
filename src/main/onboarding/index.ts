import { BrowserWindow, ipcMain, screen, shell } from 'electron';
import path from 'path';
import { isDev, devServerUrl } from '@/main/utils/env';
import { openExternalUrl } from '@/main/utils/external-url';
import { getWindowData } from '@/main/capture/video/window-manager';
import {
  titleBarColors,
  titleBarWindowOptions,
  trackTitleBarTheme,
} from '@/main/utils/title-bar';
import {
  markOnboardingCompleted,
  markOnboardingSkipped,
  needsOnboarding,
} from '@/main/settings';

const TITLE_BAR_HEIGHT = 32;

let onboardingWindow: BrowserWindow | null = null;
let onCompletedCallback: (() => Promise<void>) | null = null;

export function createOnboardingWindow(): BrowserWindow {
  if (onboardingWindow && !onboardingWindow.isDestroyed()) {
    onboardingWindow.focus();
    return onboardingWindow;
  }

  const primaryDisplay = screen.getPrimaryDisplay();
  const { width: screenWidth, height: screenHeight } =
    primaryDisplay.workAreaSize;

  const windowWidth = 500;
  const windowHeight = 650;

  onboardingWindow = new BrowserWindow({
    width: windowWidth,
    height: windowHeight,
    minWidth: windowWidth,
    minHeight: windowHeight,
    resizable: true,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    show: false,
    ...titleBarWindowOptions({
      height: TITLE_BAR_HEIGHT,
      surface: 'background',
      trafficLightPosition: { x: 16, y: 18 },
    }),
    x: Math.floor((screenWidth - windowWidth) / 2),
    y: Math.floor((screenHeight - windowHeight) / 2),
    backgroundColor: titleBarColors('background').color,
    title: 'Set up Poratake',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      devTools: isDev,
    },
  });

  if (devServerUrl) {
    onboardingWindow.loadURL(devServerUrl);
  } else {
    onboardingWindow.loadFile(path.join(__dirname, '../dist/index.html'));
  }

  onboardingWindow.webContents.on('did-finish-load', () => {
    onboardingWindow?.webContents.send('load', {
      type: 'onboarding',
      params: {},
    });
  });

  onboardingWindow.once('ready-to-show', () => {
    onboardingWindow?.show();
  });

  trackTitleBarTheme(onboardingWindow, {
    height: TITLE_BAR_HEIGHT,
    surface: 'background',
    syncBackground: true,
  });

  onboardingWindow.on('closed', () => {
    onboardingWindow = null;
  });

  return onboardingWindow;
}

export function closeOnboardingWindow(): void {
  if (onboardingWindow && !onboardingWindow.isDestroyed()) {
    onboardingWindow.close();
    onboardingWindow = null;
  }
}

export function getOnboardingWindow(): BrowserWindow | null {
  return onboardingWindow;
}

export function setOnCompletedCallback(callback: () => Promise<void>): void {
  onCompletedCallback = callback;
}

export function init(): void {
  ipcMain.on('onboarding:complete', async () => {
    markOnboardingCompleted();
    closeOnboardingWindow();
    if (onCompletedCallback) {
      await onCompletedCallback();
    }
  });

  ipcMain.on('onboarding:skip', async () => {
    markOnboardingSkipped();
    closeOnboardingWindow();
    if (onCompletedCallback) {
      await onCompletedCallback();
    }
  });

  ipcMain.on('shell:open-external', (_, url: unknown) => {
    openExternalUrl(url);
  });

  ipcMain.on('shell:reveal-in-finder', event => {
    const data = getWindowData(event.sender.id);
    if (!data) return;

    shell.showItemInFolder(data.filePath);
  });
}

export async function showOnboardingOrRun(
  callback: () => Promise<void>
): Promise<void> {
  if (needsOnboarding()) {
    setOnCompletedCallback(callback);
    createOnboardingWindow();
    return;
  }
  await callback();
}
