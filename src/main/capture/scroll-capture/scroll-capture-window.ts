import { BrowserWindow, globalShortcut, ipcMain, screen } from 'electron';
import path from 'path';
import {
  concealOverlayHandoff,
  retainOverlayHandoffWindow,
} from '@/main/capture/area-overlay';
import { isDev, devServerUrl } from '@/main/utils/env';
import { sendWindowLoad } from '@/main/utils/window-load';
import { EMPTY_SCROLL_CAPTURE_STATE } from '@/types/scroll-capture';
import type {
  ScrollCaptureAction,
  ScrollCaptureControlState,
  ScrollCaptureOverlayParams,
  ScrollCaptureOverlayState,
} from '@/types/scroll-capture';

const CONTROL_HEIGHT = 52;
const CONTROL_WIDTH = 168;
const CONTROL_GAP = 16;
const CONTROL_SHORTCUTS: Record<string, ScrollCaptureAction> = {
  Escape: 'cancel',
  Enter: 'done',
};

let captureOverlayWindow: BrowserWindow | null = null;
let controlWindow: BrowserWindow | null = null;
let actionHandler: ((action: ScrollCaptureAction) => void) | null = null;
let currentParams: ScrollCaptureOverlayParams | null = null;
let ipcRegistered = false;
let controlLoaded = false;
let visible = false;

function computeControlBounds(
  params: ScrollCaptureOverlayParams
): Electron.Rectangle {
  const area = {
    x: params.area.x - params.displayBounds.x,
    y: params.area.y - params.displayBounds.y,
    width: params.area.width,
    height: params.area.height,
  };

  const centerX = area.x + area.width / 2;
  const preferredTop = area.y + area.height + CONTROL_GAP;
  const top =
    preferredTop + CONTROL_HEIGHT <= params.displayBounds.height
      ? preferredTop
      : Math.max(0, area.y - CONTROL_GAP - CONTROL_HEIGHT);

  return {
    x: params.displayBounds.x + Math.round(centerX - CONTROL_WIDTH / 2),
    y: params.displayBounds.y + Math.round(top),
    width: CONTROL_WIDTH,
    height: CONTROL_HEIGHT,
  };
}

function setControlShortcutsEnabled(enabled: boolean): void {
  for (const accelerator of Object.keys(CONTROL_SHORTCUTS)) {
    globalShortcut.unregister(accelerator);
  }

  if (!enabled) return;

  for (const [accelerator, action] of Object.entries(CONTROL_SHORTCUTS)) {
    globalShortcut.register(accelerator, () => actionHandler?.(action));
  }
}

function parkControlWindow(): void {
  if (!controlWindow || controlWindow.isDestroyed()) return;

  controlWindow.setIgnoreMouseEvents(true, { forward: true });
  controlWindow.setOpacity(0);
  if (!controlWindow.isVisible()) {
    controlWindow.showInactive();
  }
}

function revealControlWindow(): void {
  if (
    !visible ||
    !currentParams ||
    !controlWindow ||
    controlWindow.isDestroyed() ||
    !controlLoaded
  ) {
    return;
  }

  sendWindowLoad(controlWindow.webContents, {
    type: 'scroll-capture-control',
    params: currentParams,
  });
  if (!controlWindow.isVisible()) {
    controlWindow.showInactive();
  }
  controlWindow.setIgnoreMouseEvents(false, { forward: false });
  controlWindow.setOpacity(1);
  controlWindow.moveTop();
}

function createControlWindow(bounds: Electron.Rectangle): BrowserWindow {
  const win = new BrowserWindow({
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
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      webSecurity: !isDev,
      devTools: isDev,
      backgroundThrottling: false,
    },
  });

  win.setAlwaysOnTop(true, 'pop-up-menu');
  win.setBounds(bounds);
  win.setIgnoreMouseEvents(true, { forward: true });
  win.setContentProtection(true);
  if (process.platform === 'darwin') {
    win.excludedFromShownWindowsMenu = true;
  }

  win.webContents.on('did-finish-load', () => {
    if (win.isDestroyed()) return;
    controlLoaded = true;
    if (visible && currentParams) {
      revealControlWindow();
      return;
    }
    parkControlWindow();
  });

  win.on('closed', () => {
    if (controlWindow !== win) return;
    controlWindow = null;
    controlLoaded = false;
  });

  if (devServerUrl) {
    const url = new URL(devServerUrl);
    url.searchParams.set('window', 'scroll-capture-control');
    win.loadURL(url.toString());
  } else {
    win.loadFile(path.join(__dirname, '../dist/index.html'), {
      query: { window: 'scroll-capture-control' },
    });
  }

  return win;
}

function ensureControlWindow(): void {
  if (controlWindow && !controlWindow.isDestroyed()) return;

  const displayBounds = screen.getPrimaryDisplay().bounds;
  controlLoaded = false;
  controlWindow = createControlWindow({
    x: displayBounds.x,
    y: displayBounds.y,
    width: CONTROL_WIDTH,
    height: CONTROL_HEIGHT,
  });
}

function registerIpc(): void {
  if (ipcRegistered) return;
  ipcRegistered = true;

  ipcMain.on('scroll-capture:action', (event, action: unknown) => {
    if (!controlWindow || event.sender !== controlWindow.webContents) return;
    if (
      action === 'toggle-auto-scroll' ||
      action === 'done' ||
      action === 'cancel'
    ) {
      actionHandler?.(action);
    }
  });
}

export function prewarmScrollCaptureControl(): void {
  registerIpc();
  ensureControlWindow();
}

export function showScrollCaptureOverlay(
  params: Omit<ScrollCaptureOverlayParams, 'displayBounds'>,
  onAction: (action: ScrollCaptureAction) => void
): boolean {
  const display =
    screen.getAllDisplays().find(item => item.id === params.displayId) ??
    screen.getPrimaryDisplay();
  const overlay = retainOverlayHandoffWindow(display.id);
  if (!overlay) return false;

  currentParams = {
    ...params,
    displayBounds: display.bounds,
  };
  captureOverlayWindow = overlay;
  actionHandler = onAction;
  visible = true;

  overlay.webContents.send('scroll-capture:begin', currentParams);
  overlay.setIgnoreMouseEvents(true, { forward: true });
  overlay.setOpacity(1);
  overlay.moveTop();

  prewarmScrollCaptureControl();
  controlWindow?.setBounds(computeControlBounds(currentParams));
  revealControlWindow();
  setControlShortcutsEnabled(true);
  return true;
}

export function updateScrollCaptureState(
  state: ScrollCaptureOverlayState
): void {
  if (captureOverlayWindow && !captureOverlayWindow.isDestroyed()) {
    captureOverlayWindow.webContents.send('scroll-capture:update', state);
  }
  if (controlWindow && !controlWindow.isDestroyed() && controlLoaded) {
    const controlState: ScrollCaptureControlState = {
      isAutoScrolling: state.isAutoScrolling,
      cursorOutside: state.cursorOutside,
      frameCount: state.frameCount,
      estimatedHeight: state.estimatedHeight,
    };
    controlWindow.webContents.send('scroll-capture:update', controlState);
  }
}

export function hideScrollCaptureOverlay(): void {
  setControlShortcutsEnabled(false);
  visible = false;
  actionHandler = null;
  currentParams = null;

  if (captureOverlayWindow && !captureOverlayWindow.isDestroyed()) {
    captureOverlayWindow.webContents.send(
      'scroll-capture:update',
      EMPTY_SCROLL_CAPTURE_STATE
    );
  }
  captureOverlayWindow = null;
  concealOverlayHandoff();

  if (controlWindow && !controlWindow.isDestroyed() && controlLoaded) {
    controlWindow.webContents.send(
      'scroll-capture:update',
      EMPTY_SCROLL_CAPTURE_STATE
    );
  }
  parkControlWindow();
}
