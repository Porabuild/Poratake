import { BrowserWindow, screen } from 'electron';
import type { BrowserWindowConstructorOptions, Rectangle } from 'electron';
import path from 'path';
import type { Point, Size } from '@/types/geometry';
import { devServerUrl, isDev } from './env';
import { isMac } from './platform';

type AppWebPreferences = BrowserWindowConstructorOptions['webPreferences'];

export function appWebPreferences(
  overrides: AppWebPreferences = {}
): AppWebPreferences {
  return {
    preload: path.join(__dirname, 'preload.js'),
    contextIsolation: true,
    nodeIntegration: false,
    webSecurity: !isDev,
    devTools: isDev,
    ...overrides,
  };
}

export type AppWindowPage = 'index' | 'history';

export interface AppWindowLoadOptions {
  type?: string;
  hash?: string;
  page?: AppWindowPage;
}

export function loadAppWindow(
  window: BrowserWindow,
  options: AppWindowLoadOptions = {}
): void {
  const page = options.page ?? 'index';

  if (devServerUrl) {
    const url = new URL(
      page === 'index' ? devServerUrl : `${devServerUrl}/${page}.html`
    );
    if (options.type) {
      url.searchParams.set('window', options.type);
    }
    if (options.hash) {
      url.hash = options.hash;
    }
    window.loadURL(url.toString());
    return;
  }

  window.loadFile(path.join(__dirname, `../dist/${page}.html`), {
    ...(options.type && { query: { window: options.type } }),
    ...(options.hash && { hash: options.hash }),
  });
}

export function centeredPosition(size: Size, offset = 0): Point {
  const { width, height } = screen.getPrimaryDisplay().workAreaSize;

  return {
    x: Math.floor((width - size.width) / 2) + offset,
    y: Math.floor((height - size.height) / 2) + offset,
  };
}

export type ClickThroughLevel = 'screen-saver' | 'pop-up-menu';

export interface ClickThroughWindowOptions {
  bounds: Rectangle;
  level: ClickThroughLevel;
  panel?: boolean;
  forwardMouse?: boolean;
}

export function createClickThroughWindow(
  options: ClickThroughWindowOptions
): BrowserWindow {
  const { bounds, level, panel = false, forwardMouse = false } = options;

  const window = new BrowserWindow({
    ...(panel && isMac && { type: 'panel' }),
    x: bounds.x,
    y: bounds.y,
    width: bounds.width,
    height: bounds.height,
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
    webPreferences: appWebPreferences({ backgroundThrottling: false }),
  });

  window.setAlwaysOnTop(true, level);
  window.setBounds(bounds);
  if (forwardMouse) {
    window.setIgnoreMouseEvents(true, { forward: true });
  } else {
    window.setIgnoreMouseEvents(true);
  }
  window.setContentProtection(true);
  if (isMac) {
    window.excludedFromShownWindowsMenu = true;
  }

  return window;
}
