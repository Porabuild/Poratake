import { BrowserWindow, ipcMain, screen, shell } from 'electron';
import path from 'path';
import { isDev, devServerUrl } from '@/main/utils/env';
import {
  markOnboardingCompleted,
  markOnboardingSkipped,
  needsOnboarding,
} from '@/main/settings';

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
    titleBarStyle: 'hiddenInset',
    trafficLightPosition: { x: 16, y: 18 },
    x: Math.floor((screenWidth - windowWidth) / 2),
    y: Math.floor((screenHeight - windowHeight) / 2),
    backgroundColor: '#1e1e1e',
    title: 'Setup Capty',
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

  ipcMain.on('shell:open-external', (_, url: string) => {
    shell.openExternal(url);
  });

  ipcMain.on('shell:reveal-in-finder', (_, path: string) => {
    shell.showItemInFolder(`${path}/recording.mov`);
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
