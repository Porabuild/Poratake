import { BrowserWindow, app } from 'electron';
import type { WebContents } from 'electron';
import {
  appWebPreferences,
  centeredPosition,
  loadAppWindow,
} from '@/main/utils/window-factory';
import { registerDockWindow } from '@/main/utils/dock';
import { sendWindowLoad } from '@/main/utils/window-load';
import {
  nativeWindowMaterialOptions,
  supportsNativeWindowMaterial,
  titleBarWindowOptions,
  trackTitleBarTheme,
} from '@/main/utils/title-bar';

let settingsWindow: BrowserWindow | null = null;

export function isSettingsWindowWebContents(sender: WebContents): boolean {
  return settingsWindow?.webContents === sender;
}

export function createOrShowSettingsWindow(tab?: string) {
  if (settingsWindow && !settingsWindow.isDestroyed()) {
    if (tab) {
      settingsWindow.webContents.send('navigate-tab', tab);
    }
    settingsWindow.show();
    settingsWindow.focus();
    return;
  }

  const windowWidth = 880;
  const windowHeight = 700;
  const nativeMaterial = supportsNativeWindowMaterial();

  settingsWindow = new BrowserWindow({
    width: windowWidth,
    height: windowHeight,
    minWidth: windowWidth,
    minHeight: windowHeight,
    maximizable: false,
    minimizable: true,
    resizable: true,
    webPreferences: appWebPreferences(),
    ...titleBarWindowOptions({
      surface: 'background',
      transparent: nativeMaterial,
    }),
    ...centeredPosition({ width: windowWidth, height: windowHeight }),
    show: false,
    backgroundColor: nativeMaterial ? '#00000000' : '#070709',
    ...nativeWindowMaterialOptions(),
    title: 'Poratake Settings',
  });

  trackTitleBarTheme(settingsWindow, {
    surface: 'background',
    transparent: nativeMaterial,
  });

  loadAppWindow(settingsWindow, { hash: tab });

  settingsWindow.webContents.on('did-finish-load', () => {
    const win = settingsWindow;
    if (!win) return;

    sendWindowLoad(win.webContents, {
      type: 'settings',
      params: { nativeMaterial },
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
