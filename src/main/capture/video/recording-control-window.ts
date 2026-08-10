import { BrowserWindow, ipcMain } from 'electron';
import path from 'path';
import { isDev, devServerUrl } from '@/main/utils/env';
import type {
  RecordingControlAction,
  RecordingControlMode,
  RecordingControlState,
} from '@/types/recording-control';

const CONTROL_HEIGHT = 52;
const CONTROL_WIDTHS: Record<RecordingControlMode, number> = {
  'pre-recording': 204,
  recording: 232,
};
const ACTIONS = new Set<RecordingControlAction>([
  'start',
  'cancel',
  'pause',
  'resume',
  'stop',
  'delete',
  'toggle-system-audio',
  'toggle-mic',
  'toggle-camera',
  'toggle-mic-mute',
]);

let controlWindow: BrowserWindow | null = null;
let currentState: RecordingControlState | null = null;
let currentPosition = { x: 100, y: 100 };
let actionHandler: ((action: RecordingControlAction) => void) | null = null;
let loaded = false;
let rendererReady = false;
let visible = false;
let ipcRegistered = false;

function registerIpc(): void {
  if (ipcRegistered) return;
  ipcRegistered = true;

  ipcMain.on('recording-control:action', (event, action: unknown) => {
    if (!controlWindow || event.sender !== controlWindow.webContents) return;
    if (
      typeof action !== 'string' ||
      !ACTIONS.has(action as RecordingControlAction)
    ) {
      return;
    }

    actionHandler?.(action as RecordingControlAction);
  });

  ipcMain.on('recording-control:ready', event => {
    if (!controlWindow || event.sender !== controlWindow.webContents) return;
    rendererReady = true;
    showLoadedWindow();
  });
}

function sendLoad(): void {
  if (
    !controlWindow ||
    controlWindow.isDestroyed() ||
    !currentState ||
    !loaded
  ) {
    return;
  }

  controlWindow.webContents.send('load', {
    type: 'recording-control',
    params: currentState,
  });
}

function showLoadedWindow(): void {
  if (
    !controlWindow ||
    controlWindow.isDestroyed() ||
    !loaded ||
    !rendererReady ||
    !visible
  ) {
    return;
  }

  controlWindow.setOpacity(1);
  controlWindow.showInactive();
}

function createControlWindow(): BrowserWindow {
  registerIpc();

  const window = new BrowserWindow({
    width: CONTROL_WIDTHS['pre-recording'],
    height: CONTROL_HEIGHT,
    frame: false,
    transparent: true,
    resizable: false,
    minimizable: false,
    maximizable: false,
    closable: false,
    fullscreenable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    show: false,
    opacity: 0,
    paintWhenInitiallyHidden: true,
    hasShadow: false,
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

  window.setAlwaysOnTop(true, 'screen-saver');
  window.setContentProtection(true);
  window.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });

  if (devServerUrl) {
    window.loadURL(devServerUrl);
  } else {
    window.loadFile(path.join(__dirname, '../dist/index.html'));
  }

  window.webContents.on('did-finish-load', () => {
    loaded = true;
    sendLoad();
    showLoadedWindow();
  });

  window.on('closed', () => {
    if (controlWindow !== window) return;
    controlWindow = null;
    currentState = null;
    loaded = false;
    rendererReady = false;
    visible = false;
  });

  return window;
}

function ensureControlWindow(): BrowserWindow {
  if (controlWindow && !controlWindow.isDestroyed()) {
    return controlWindow;
  }

  controlWindow = createControlWindow();
  return controlWindow;
}

export function getRecordingControlWindowWidth(
  mode: RecordingControlMode
): number {
  return CONTROL_WIDTHS[mode];
}

export function prewarmRecordingControlBrowserWindow(): void {
  ensureControlWindow();
}

export function showRecordingControlBrowserWindow(
  state: RecordingControlState,
  position: { x: number; y: number },
  onAction: (action: RecordingControlAction) => void
): void {
  const window = ensureControlWindow();
  currentState = state;
  currentPosition = position;
  actionHandler = onAction;
  visible = true;

  window.setBounds({
    ...position,
    width: CONTROL_WIDTHS[state.mode],
    height: CONTROL_HEIGHT,
  });
  sendLoad();
  showLoadedWindow();
}

export function updateRecordingControlBrowserWindow(
  update: Partial<RecordingControlState>
): void {
  if (!currentState) return;

  const previousMode = currentState.mode;
  currentState = { ...currentState, ...update };
  const window = controlWindow;
  if (!window || window.isDestroyed()) return;

  if (currentState.mode !== previousMode) {
    const previousWidth = CONTROL_WIDTHS[previousMode];
    const width = CONTROL_WIDTHS[currentState.mode];
    currentPosition = {
      x: currentPosition.x + Math.round((previousWidth - width) / 2),
      y: currentPosition.y,
    };
    window.setBounds({
      ...currentPosition,
      width,
      height: CONTROL_HEIGHT,
    });
  }

  window.webContents.send('recording-control:update', update);
}

export function updateRecordingControlBrowserWindowPosition(position: {
  x: number;
  y: number;
}): void {
  currentPosition = position;
  const window = controlWindow;
  if (!window || window.isDestroyed() || !currentState) return;

  window.setBounds({
    ...position,
    width: CONTROL_WIDTHS[currentState.mode],
    height: CONTROL_HEIGHT,
  });
}

export function hideRecordingControlBrowserWindow(): void {
  visible = false;
  actionHandler = null;
  currentState = null;

  if (!controlWindow || controlWindow.isDestroyed()) return;
  controlWindow.setOpacity(0);
  controlWindow.hide();
}

export function getRecordingControlBrowserWindow(): BrowserWindow | null {
  return controlWindow && !controlWindow.isDestroyed() ? controlWindow : null;
}
