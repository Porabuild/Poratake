import { BrowserWindow, ipcMain, app, screen, Notification } from 'electron';
import path from 'path';
import { isDev, devServerUrl } from '@/main/utils/env';
import { openExternalUrl } from '@/main/utils/external-url';
import {
  titleBarWindowOptions,
  trackTitleBarTheme,
} from '@/main/utils/title-bar';

const ACTIVATION_TITLE_BAR_HEIGHT = 32;

let activationWindow: BrowserWindow | null = null;

export function createActivationWindow(): BrowserWindow {
  if (activationWindow && !activationWindow.isDestroyed()) {
    activationWindow.focus();
    return activationWindow;
  }

  const primaryDisplay = screen.getPrimaryDisplay();
  const { width: screenWidth, height: screenHeight } =
    primaryDisplay.workAreaSize;

  const windowWidth = 500;
  const windowHeight = 650;

  activationWindow = new BrowserWindow({
    width: windowWidth,
    height: windowHeight,
    resizable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    show: false,
    ...titleBarWindowOptions({
      height: ACTIVATION_TITLE_BAR_HEIGHT,
      surface: 'background',
      trafficLightPosition: { x: 16, y: 18 },
    }),
    x: Math.floor((screenWidth - windowWidth) / 2),
    y: Math.floor((screenHeight - windowHeight) / 2),
    backgroundColor: '#1e1e1e',
    title: 'Activate Poratake',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      devTools: isDev,
    },
  });

  trackTitleBarTheme(activationWindow, {
    height: ACTIVATION_TITLE_BAR_HEIGHT,
    surface: 'background',
  });

  if (devServerUrl) {
    activationWindow.loadURL(devServerUrl);
  } else {
    activationWindow.loadFile(path.join(__dirname, '../dist/index.html'));
  }

  activationWindow.webContents.on('did-finish-load', () => {
    activationWindow?.webContents.send('load', {
      type: 'activation',
      params: {},
    });
  });

  activationWindow.once('ready-to-show', () => {
    app.focus({ steal: true });
    activationWindow?.show();
    activationWindow?.focus();
  });

  activationWindow.on('closed', () => {
    activationWindow = null;
  });

  activationWindow.webContents.setWindowOpenHandler(({ url }) => {
    openExternalUrl(url);
    return { action: 'deny' };
  });

  return activationWindow;
}

export function closeActivationWindow(): void {
  if (activationWindow && !activationWindow.isDestroyed()) {
    activationWindow.close();
    activationWindow = null;
  }
}

export function getActivationWindow(): BrowserWindow | null {
  return activationWindow;
}

function broadcastLicenseChanged(): void {
  for (const window of BrowserWindow.getAllWindows()) {
    if (!window.isDestroyed()) {
      window.webContents.send('license:changed');
    }
  }
}

export function init(): void {
  ipcMain.on('license:activated', () => {
    closeActivationWindow();
    broadcastLicenseChanged();

    if (Notification.isSupported()) {
      const notification = new Notification({
        title: 'Capty License Activated',
        body: 'Your Capty license now unlocks Pro features in Poratake.',
      });
      notification.show();
    }
  });

  ipcMain.on('license:close', () => {
    closeActivationWindow();
  });

  ipcMain.on('license:deleted', () => {
    broadcastLicenseChanged();
  });

  ipcMain.on('license:open-activation', () => {
    createActivationWindow();
  });
}
