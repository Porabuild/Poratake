import { BrowserWindow, ipcMain } from 'electron';
import path from 'path';
import { getActiveOverlayWindowAtPoint } from '@/main/capture/area-overlay';
import { daemon } from '@/main/daemon';
import { listMediaDevices } from '@/main/devices';
import { isDev, devServerUrl } from '@/main/utils/env';
import type { MediaDeviceLists } from '@/types/devices';
import type {
  RecordingControlAction,
  RecordingControlActionData,
  RecordingControlMode,
  RecordingControlState,
} from '@/types/recording-control';

const CONTROL_HEIGHT = 52;
const DEVICE_MENU_WIDTH = 300;
const DEVICE_MENU_HEIGHT = 300;
const CONTROL_WIDTHS: Record<RecordingControlMode, number> = {
  'pre-recording': 236,
  recording: 400,
};
const TARGET_LABEL_WIDTH = 140;
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
const DEVICE_ACTIONS = new Set<RecordingControlAction>([
  'select-mic',
  'select-camera',
]);
const EMPTY_MEDIA_DEVICES: MediaDeviceLists = {
  microphones: [],
  cameras: [],
  defaultMicrophoneId: null,
  defaultCameraId: null,
};

let controlWindow: BrowserWindow | null = null;
let currentState: RecordingControlState | null = null;
let currentPosition = { x: 100, y: 100 };
let actionHandler:
  | ((
      action: RecordingControlAction,
      data?: RecordingControlActionData
    ) => void)
  | null = null;
let loaded = false;
let rendererReady = false;
let visible = false;
let ipcRegistered = false;
let deviceMenuOpen = false;
let visibilityVersion = 0;

function isDeviceActionData(
  value: unknown
): value is RecordingControlActionData {
  if (!value || typeof value !== 'object') return false;

  const data = value as Partial<RecordingControlActionData>;
  return (
    (data.deviceId === null || typeof data.deviceId === 'string') &&
    (data.deviceName === null || typeof data.deviceName === 'string')
  );
}

function getControlBounds(mode: RecordingControlMode): Electron.Rectangle {
  const controlWidth = getRecordingControlWindowWidth(
    mode,
    currentState?.targetName != null
  );
  const width = Math.max(controlWidth, DEVICE_MENU_WIDTH);

  return {
    x: currentPosition.x - Math.round((width - controlWidth) / 2),
    y: currentPosition.y,
    width,
    height: deviceMenuOpen ? DEVICE_MENU_HEIGHT : CONTROL_HEIGHT,
  };
}

function setDeviceMenuOpen(open: boolean): void {
  deviceMenuOpen = open;
  if (!controlWindow || controlWindow.isDestroyed() || !currentState) return;

  controlWindow.setBounds(getControlBounds(currentState.mode));
}

function updateParentWindow(
  window: BrowserWindow,
  mode: RecordingControlMode
): void {
  const parent =
    mode === 'pre-recording'
      ? getActiveOverlayWindowAtPoint(currentPosition)
      : null;
  window.setParentWindow(parent);
}

function registerIpc(): void {
  if (ipcRegistered) return;
  ipcRegistered = true;

  ipcMain.on(
    'recording-control:action',
    (event, action: unknown, data: unknown) => {
      if (!controlWindow || event.sender !== controlWindow.webContents) return;
      if (typeof action !== 'string') return;

      const recordingAction = action as RecordingControlAction;
      if (ACTIONS.has(recordingAction)) {
        actionHandler?.(recordingAction);
        return;
      }

      if (!DEVICE_ACTIONS.has(recordingAction) || !isDeviceActionData(data)) {
        return;
      }

      actionHandler?.(recordingAction, data);
    }
  );

  ipcMain.handle(
    'recording-control:devices',
    async (event): Promise<MediaDeviceLists> => {
      if (!controlWindow || event.sender !== controlWindow.webContents) {
        return EMPTY_MEDIA_DEVICES;
      }

      return listMediaDevices();
    }
  );

  ipcMain.on('recording-control:device-menu-open', (event, open: unknown) => {
    if (!controlWindow || event.sender !== controlWindow.webContents) return;
    if (typeof open !== 'boolean' || !currentState) return;

    setDeviceMenuOpen(open);
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

function disableControlWindowTransitions(window: BrowserWindow): Promise<void> {
  if (window.isDestroyed()) return Promise.resolve();

  const windowHandle = window
    .getNativeWindowHandle()
    .readBigUInt64LE()
    .toString();

  return daemon
    .call('area-selector', 'disableWindowTransitions', {
      windowHandle,
      noActivate: false,
    })
    .then(() => undefined)
    .catch(error => {
      console.error(
        'Failed to disable recording control window transitions:',
        error
      );
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

  if (controlWindow.isVisible()) {
    controlWindow.setOpacity(1);
    controlWindow.moveTop();
    return;
  }

  const version = visibilityVersion;
  controlWindow.setOpacity(0);
  void disableControlWindowTransitions(controlWindow).then(() => {
    if (
      !controlWindow ||
      controlWindow.isDestroyed() ||
      !loaded ||
      !rendererReady ||
      !visible ||
      visibilityVersion !== version
    ) {
      return;
    }

    controlWindow.showInactive();
    controlWindow.setOpacity(1);
    controlWindow.moveTop();
  });
}

function createControlWindow(): BrowserWindow {
  registerIpc();

  const window = new BrowserWindow({
    width: CONTROL_WIDTHS['pre-recording'],
    height: CONTROL_HEIGHT,
    frame: false,
    transparent: true,
    backgroundColor: '#00000000',
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

  void disableControlWindowTransitions(window);

  if (devServerUrl) {
    const url = new URL(devServerUrl);
    url.searchParams.set('window', 'recording-control');
    window.loadURL(url.toString());
  } else {
    window.loadFile(path.join(__dirname, '../dist/index.html'), {
      query: { window: 'recording-control' },
    });
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
    deviceMenuOpen = false;
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
  mode: RecordingControlMode,
  hasTargetName = false
): number {
  return CONTROL_WIDTHS[mode] + (hasTargetName ? TARGET_LABEL_WIDTH : 0);
}

export function prewarmRecordingControlBrowserWindow(): void {
  ensureControlWindow();
}

export function showRecordingControlBrowserWindow(
  state: RecordingControlState,
  position: { x: number; y: number },
  onAction: (
    action: RecordingControlAction,
    data?: RecordingControlActionData
  ) => void
): void {
  const window = ensureControlWindow();
  visibilityVersion += 1;
  currentState = state;
  currentPosition = position;
  actionHandler = onAction;
  visible = true;
  deviceMenuOpen = false;

  updateParentWindow(window, state.mode);
  window.setBounds(getControlBounds(state.mode));
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
    deviceMenuOpen = false;
    updateParentWindow(window, currentState.mode);
    window.setBounds(getControlBounds(currentState.mode));
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

  updateParentWindow(window, currentState.mode);
  window.setBounds(getControlBounds(currentState.mode));
}

export function clearRecordingControlBrowserWindowParent(): void {
  if (!controlWindow || controlWindow.isDestroyed()) return;

  controlWindow.setParentWindow(null);
}

export function hideRecordingControlBrowserWindow(): void {
  const resetState = currentState
    ? {
        ...currentState,
        mode: 'pre-recording' as const,
        micMuted: false,
        cameraLocked: false,
        isPaused: false,
        isStarting: false,
        elapsedSeconds: 0,
      }
    : null;
  visibilityVersion += 1;
  visible = false;
  actionHandler = null;
  currentState = null;
  deviceMenuOpen = false;

  if (!controlWindow || controlWindow.isDestroyed()) return;
  controlWindow.setParentWindow(null);
  controlWindow.setOpacity(0);
  controlWindow.hide();
  if (resetState) {
    controlWindow.webContents.send('recording-control:update', resetState);
  }
}

export function getRecordingControlBrowserWindow(): BrowserWindow | null {
  return controlWindow && !controlWindow.isDestroyed() ? controlWindow : null;
}
