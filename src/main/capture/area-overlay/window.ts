import { BrowserWindow } from 'electron';
import type { Display } from 'electron';
import path from 'path';
import { isDev, devServerUrl } from '@/main/utils/env';
import { isMac } from '@/main/utils/platform';

export function createOverlayWindow(display: Display): BrowserWindow {
  const overlayWindow = new BrowserWindow({
    ...(isMac ? { type: 'panel' } : {}),
    x: display.bounds.x,
    y: display.bounds.y,
    width: display.bounds.width,
    height: display.bounds.height,
    frame: false,
    thickFrame: false,
    transparent: true,
    backgroundColor: '#00000000',
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    hiddenInMissionControl: true,
    alwaysOnTop: true,
    show: false,
    opacity: 0,
    paintWhenInitiallyHidden: true,
    roundedCorners: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      webSecurity: !isDev,
      devTools: isDev,
      backgroundThrottling: false,
    },
  });

  overlayWindow.setAlwaysOnTop(true, 'screen-saver');
  overlayWindow.setBounds(display.bounds);
  overlayWindow.setIgnoreMouseEvents(true);
  overlayWindow.setContentProtection(true);
  if (isMac) {
    overlayWindow.excludedFromShownWindowsMenu = true;
  }

  if (devServerUrl) {
    const url = new URL(devServerUrl);
    url.searchParams.set('window', 'area-overlay');
    overlayWindow.loadURL(url.toString());
  } else {
    overlayWindow.loadFile(path.join(__dirname, '../dist/index.html'), {
      query: { window: 'area-overlay' },
    });
  }

  return overlayWindow;
}
