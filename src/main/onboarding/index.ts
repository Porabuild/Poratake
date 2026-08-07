import { BrowserWindow, ipcMain, nativeTheme, screen, shell } from 'electron';
import path from 'path';
import { isDev, devServerUrl } from '@/main/utils/env';
import { isMac } from '@/main/utils/platform';
import {
  markOnboardingCompleted,
  markOnboardingSkipped,
  needsOnboarding,
} from '@/main/settings';

const TITLE_BAR_HEIGHT = 32;

/**
 * Resolved values of the `--background` / `--foreground` tokens in
 * src/renderer/styles/base.css, so the title bar and its window-control
 * buttons render on the same color as the page underneath.
 */
function titleBarColors(): { color: string; symbolColor: string } {
  return nativeTheme.shouldUseDarkColors
    ? { color: '#181818', symbolColor: '#fafafa' }
    : { color: '#ffffff', symbolColor: '#000000' };
}

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
    ...(isMac
      ? {
          titleBarStyle: 'hiddenInset' as const,
          trafficLightPosition: { x: 16, y: 18 },
        }
      : {
          titleBarStyle: 'hidden' as const,
          titleBarOverlay: {
            ...titleBarColors(),
            height: TITLE_BAR_HEIGHT,
          },
        }),
    x: Math.floor((screenWidth - windowWidth) / 2),
    y: Math.floor((screenHeight - windowHeight) / 2),
    backgroundColor: titleBarColors().color,
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

  if (!isMac) {
    // Re-apply on show and on theme changes: the overlay keeps the system
    // colors otherwise, which leaves a light backplate behind the close button.
    const applyTitleBarOverlay = () => {
      if (!onboardingWindow || onboardingWindow.isDestroyed()) return;
      onboardingWindow.setTitleBarOverlay({
        ...titleBarColors(),
        height: TITLE_BAR_HEIGHT,
      });
      onboardingWindow.setBackgroundColor(titleBarColors().color);
    };

    onboardingWindow.once('ready-to-show', applyTitleBarOverlay);
    nativeTheme.on('updated', applyTitleBarOverlay);
    onboardingWindow.once('closed', () => {
      nativeTheme.off('updated', applyTitleBarOverlay);
    });
  }

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
