import { BrowserWindow, ipcMain, shell } from 'electron';
import {
  appWebPreferences,
  centeredPosition,
  loadAppWindow,
} from '@/main/utils/window-factory';
import { getWindowData } from '@/main/capture/video/window-manager';
import { openKeyboardShortcutPreferences } from '@/main/system/permissions';
import { sendWindowLoad } from '@/main/utils/window-load';
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

  const windowWidth = 500;
  const windowHeight = 650;

  const window = new BrowserWindow({
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
    ...centeredPosition({ width: windowWidth, height: windowHeight }),
    backgroundColor: titleBarColors('background').color,
    title: 'Set up Poratake',
    webPreferences: appWebPreferences(),
  });
  onboardingWindow = window;

  loadAppWindow(window);

  window.webContents.on('did-finish-load', () => {
    sendWindowLoad(window.webContents, {
      type: 'onboarding',
      params: {},
    });
  });

  window.once('ready-to-show', () => {
    window.show();
  });

  trackTitleBarTheme(window, {
    height: TITLE_BAR_HEIGHT,
    surface: 'background',
    syncBackground: true,
  });

  window.on('closed', () => {
    onboardingWindow = null;
  });

  return window;
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

  ipcMain.on('onboarding:openKeyboardSettings', () => {
    openKeyboardShortcutPreferences();
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
