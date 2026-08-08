import { BrowserWindow, screen, app } from 'electron';
import path from 'path';
import { isDev, devServerUrl } from '@/main/utils/env';
import { registerDockWindow } from '@/main/utils/dock';
import {
  titleBarWindowOptions,
  trackTitleBarTheme,
} from '@/main/utils/title-bar';

let settingsWindow: BrowserWindow | null = null;

export function createOrShowSettingsWindow(tab?: string) {
  if (settingsWindow && !settingsWindow.isDestroyed()) {
    if (tab) {
      settingsWindow.webContents.send('navigate-tab', tab);
    }
    settingsWindow.show();
    settingsWindow.focus();
    return;
  }

  const primaryDisplay = screen.getPrimaryDisplay();
  const { width: screenWidth, height: screenHeight } =
    primaryDisplay.workAreaSize;

  const windowWidth = 880;
  const windowHeight = 700;

  settingsWindow = new BrowserWindow({
    width: windowWidth,
    height: windowHeight,
    minWidth: windowWidth,
    minHeight: windowHeight,
    maximizable: false,
    minimizable: true,
    resizable: true,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      devTools: isDev,
    },
    ...titleBarWindowOptions({ surface: 'background' }),
    x: Math.floor((screenWidth - windowWidth) / 2),
    y: Math.floor((screenHeight - windowHeight) / 2),
    show: false,
    backgroundColor: '#1e1e1e',
    title: 'Settings',
  });

  trackTitleBarTheme(settingsWindow, { surface: 'background' });

  const hash = tab ? `#${tab}` : '';
  if (devServerUrl) {
    settingsWindow.loadURL(`${devServerUrl}${hash}`);
  } else {
    settingsWindow.loadFile(path.join(__dirname, '../dist/index.html'), {
      hash: tab,
    });
  }

  settingsWindow.webContents.on('did-finish-load', () => {
    settingsWindow?.webContents.send('load', {
      type: 'settings',
      params: {},
    });
  });

  settingsWindow.once('ready-to-show', async () => {
    if (settingsWindow) {
      await registerDockWindow(settingsWindow, 'settings');
      app.focus({ steal: true });
      settingsWindow.show();
      settingsWindow.focus();
    }
  });

  settingsWindow.on('closed', () => {
    settingsWindow = null;
  });
}
