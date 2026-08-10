import { beforeEach, describe, expect, it, vi } from 'vitest';

const windows: MockBrowserWindow[] = [];
const ipcOn: Record<string, (...args: unknown[]) => void> = {};

class MockBrowserWindow {
  handlers: Record<string, Array<(...args: unknown[]) => void>> = {};
  destroyed = false;
  options: Electron.BrowserWindowConstructorOptions;
  webContents = {
    on: vi.fn((event: string, handler: (...args: unknown[]) => void) => {
      this.handlers[`webContents:${event}`] ??= [];
      this.handlers[`webContents:${event}`].push(handler);
    }),
    send: vi.fn(),
  };
  setAlwaysOnTop = vi.fn();
  setContentProtection = vi.fn();
  setVisibleOnAllWorkspaces = vi.fn();
  loadURL = vi.fn();
  loadFile = vi.fn();
  setOpacity = vi.fn();
  showInactive = vi.fn();
  setBounds = vi.fn();
  hide = vi.fn();
  isDestroyed = vi.fn(() => this.destroyed);
  on = vi.fn((event: string, handler: (...args: unknown[]) => void) => {
    this.handlers[event] ??= [];
    this.handlers[event].push(handler);
  });

  constructor(options: Electron.BrowserWindowConstructorOptions) {
    this.options = options;
    windows.push(this);
  }
}

vi.mock('electron', () => ({
  BrowserWindow: MockBrowserWindow,
  ipcMain: {
    on: (event: string, handler: (...args: unknown[]) => void) => {
      ipcOn[event] = handler;
    },
  },
}));

vi.mock('@/main/utils/env', () => ({
  isDev: true,
  devServerUrl: 'http://localhost:5173',
}));

const state = {
  mode: 'pre-recording' as const,
  systemAudio: true,
  micEnabled: false,
  micMuted: false,
  cameraEnabled: false,
  isPaused: false,
  isStarting: false,
  elapsedSeconds: 0,
};

describe('recording-control-window', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    windows.splice(0);
    Object.keys(ipcOn).forEach(key => delete ipcOn[key]);
  });

  it('prewarms a protected transparent toolbar window', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');

    control.prewarmRecordingControlBrowserWindow();

    expect(windows).toHaveLength(1);
    expect(windows[0].options).toEqual(
      expect.objectContaining({
        frame: false,
        transparent: true,
        show: false,
      })
    );
    expect(windows[0].options.webPreferences).toEqual(
      expect.objectContaining({
        contextIsolation: true,
        nodeIntegration: false,
        webSecurity: false,
      })
    );
    expect(windows[0].setContentProtection).toHaveBeenCalledWith(true);
    expect(windows[0].setAlwaysOnTop).toHaveBeenCalledWith(
      true,
      'screen-saver'
    );
  });

  it('shows the loaded toolbar at the requested position', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');
    const onAction = vi.fn();

    control.showRecordingControlBrowserWindow(
      state,
      { x: 400, y: 24 },
      onAction
    );
    const window = windows[0];
    window.handlers['webContents:did-finish-load'][0]();
    expect(window.showInactive).not.toHaveBeenCalled();
    ipcOn['recording-control:ready']({ sender: window.webContents });

    expect(window.setBounds).toHaveBeenCalledWith({
      x: 400,
      y: 24,
      width: 204,
      height: 52,
    });
    expect(window.webContents.send).toHaveBeenCalledWith('load', {
      type: 'recording-control',
      params: state,
    });
    expect(window.setOpacity).toHaveBeenCalledWith(1);
    expect(window.showInactive).toHaveBeenCalled();
  });

  it('accepts actions only from its own renderer', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');
    const onAction = vi.fn();
    control.showRecordingControlBrowserWindow(
      state,
      { x: 400, y: 24 },
      onAction
    );
    const window = windows[0];

    ipcOn['recording-control:action']({ sender: {} }, 'start');
    ipcOn['recording-control:action'](
      { sender: window.webContents },
      'unknown'
    );
    ipcOn['recording-control:action']({ sender: window.webContents }, 'start');

    expect(onAction).toHaveBeenCalledTimes(1);
    expect(onAction).toHaveBeenCalledWith('start');
  });

  it('keeps the toolbar centered when recording mode changes', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');
    control.showRecordingControlBrowserWindow(
      state,
      { x: 400, y: 24 },
      vi.fn()
    );
    const window = windows[0];

    control.updateRecordingControlBrowserWindow({ mode: 'recording' });

    expect(window.setBounds).toHaveBeenLastCalledWith({
      x: 386,
      y: 24,
      width: 232,
      height: 52,
    });
    expect(window.webContents.send).toHaveBeenCalledWith(
      'recording-control:update',
      { mode: 'recording' }
    );
  });

  it('hides the reusable window and clears its active action handler', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');
    const onAction = vi.fn();
    control.showRecordingControlBrowserWindow(
      state,
      { x: 400, y: 24 },
      onAction
    );
    const window = windows[0];

    control.hideRecordingControlBrowserWindow();
    ipcOn['recording-control:action']({ sender: window.webContents }, 'start');

    expect(window.setOpacity).toHaveBeenCalledWith(0);
    expect(window.hide).toHaveBeenCalled();
    expect(onAction).not.toHaveBeenCalled();
  });
});
