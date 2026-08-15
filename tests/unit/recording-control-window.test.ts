import { beforeEach, describe, expect, it, vi } from 'vitest';

const windows: MockBrowserWindow[] = [];
const ipcOn: Record<string, (...args: unknown[]) => void> = {};
const ipcHandle: Record<string, (...args: unknown[]) => unknown> = {};
const mockListMediaDevices = vi.fn();
const mockListIOSDevices = vi.fn();
const mockDaemonCall = vi.fn();
const overlayParent = {};
const mockGetActiveOverlayWindowAtPoint = vi.fn(() => overlayParent);
const { mockIsWindows } = vi.hoisted(() => ({
  mockIsWindows: { value: true },
}));

class MockBrowserWindow {
  handlers: Record<string, Array<(...args: unknown[]) => void>> = {};
  destroyed = false;
  visible = false;
  options: Electron.BrowserWindowConstructorOptions;
  webContents = {
    id: 1,
    on: vi.fn((event: string, handler: (...args: unknown[]) => void) => {
      this.handlers[`webContents:${event}`] ??= [];
      this.handlers[`webContents:${event}`].push(handler);
    }),
    once: vi.fn(),
    send: vi.fn(),
  };
  setAlwaysOnTop = vi.fn();
  setContentProtection = vi.fn();
  setVisibleOnAllWorkspaces = vi.fn();
  getNativeWindowHandle = vi.fn(() => Buffer.from([1, 0, 0, 0, 0, 0, 0, 0]));
  loadURL = vi.fn();
  loadFile = vi.fn();
  setOpacity = vi.fn();
  showInactive = vi.fn(() => {
    this.visible = true;
  });
  moveTop = vi.fn();
  setBounds = vi.fn();
  setParentWindow = vi.fn();
  hide = vi.fn(() => {
    this.visible = false;
  });
  isVisible = vi.fn(() => this.visible);
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
    handle: (event: string, handler: (...args: unknown[]) => unknown) => {
      ipcHandle[event] = handler;
    },
  },
}));

vi.mock('@/main/devices', () => ({
  listMediaDevices: (...a: unknown[]) => mockListMediaDevices(...a),
  listIOSDevices: () => mockListIOSDevices(),
}));

vi.mock('@/main/utils/platform', () => ({
  isWindows: mockIsWindows.value,
}));

vi.mock('@/main/capture/area-overlay', () => ({
  getActiveOverlayWindowAtPoint: (point: Electron.Point) =>
    mockGetActiveOverlayWindowAtPoint(point),
}));

vi.mock('@/main/daemon', () => ({
  daemon: { call: (...a: unknown[]) => mockDaemonCall(...a) },
}));

vi.mock('@/main/utils/env', () => ({
  isDev: true,
  devServerUrl: 'http://localhost:5173',
}));

const state = {
  mode: 'pre-recording' as const,
  targetName: null,
  systemAudio: true,
  micEnabled: false,
  micMuted: false,
  selectedMicId: null,
  cameraEnabled: false,
  selectedCameraId: null,
  selectedIOSDeviceId: null,
  selectedIOSDeviceName: null,
  cameraLocked: false,
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
    Object.keys(ipcHandle).forEach(key => delete ipcHandle[key]);
    mockIsWindows.value = true;
    mockDaemonCall.mockResolvedValue({ disabled: true });
    mockListMediaDevices.mockResolvedValue({
      microphones: [{ id: 'mic-1', label: 'Microphone 1' }],
      cameras: [{ id: 'camera-1', label: 'Camera 1' }],
      defaultMicrophoneId: 'mic-1',
      defaultCameraId: 'camera-1',
    });
    mockListIOSDevices.mockResolvedValue([{ id: 'ios-1', label: 'iPhone' }]);
  });

  function settle(): Promise<void> {
    return new Promise(resolve => setImmediate(resolve));
  }

  it('prewarms a protected transparent toolbar window', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');

    control.prewarmRecordingControlBrowserWindow();

    expect(windows).toHaveLength(1);
    expect(windows[0].options).toEqual(
      expect.objectContaining({
        frame: false,
        transparent: true,
        backgroundColor: '#00000000',
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
    expect(windows[0].loadURL).toHaveBeenCalledWith(
      'http://localhost:5173/?window=recording-control'
    );
  });

  it('disables window transitions while keeping the toolbar activatable', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');

    control.prewarmRecordingControlBrowserWindow();

    expect(mockDaemonCall).toHaveBeenCalledWith(
      'area-selector',
      'disableWindowTransitions',
      {
        windowHandle: '1',
        noActivate: false,
      }
    );
  });

  it('reveals the toolbar only after transitions are disabled', async () => {
    let resolveTransitions!: (value: { disabled: boolean }) => void;
    mockDaemonCall.mockReturnValue(
      new Promise(resolve => {
        resolveTransitions = resolve;
      })
    );

    const control =
      await import('@/main/capture/video/recording-control-window');
    control.showRecordingControlBrowserWindow(
      state,
      { x: 400, y: 24 },
      vi.fn()
    );
    const window = windows[0];
    window.handlers['webContents:did-finish-load'][0]();
    ipcOn['recording-control:ready']({ sender: window.webContents });
    await settle();

    expect(window.showInactive).not.toHaveBeenCalled();

    resolveTransitions({ disabled: true });
    await settle();

    expect(window.setOpacity).toHaveBeenCalledWith(1);
    expect(window.showInactive).toHaveBeenCalled();
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
    await settle();

    expect(window.setBounds).toHaveBeenCalledWith({
      x: 352,
      y: 24,
      width: 332,
      height: 52,
    });
    expect(mockGetActiveOverlayWindowAtPoint).toHaveBeenCalledWith({
      x: 400,
      y: 24,
    });
    expect(window.setParentWindow).toHaveBeenCalledWith(overlayParent);
    expect(window.webContents.send).toHaveBeenCalledWith('load', {
      type: 'recording-control',
      params: state,
    });
    expect(window.setOpacity).toHaveBeenCalledWith(1);
    expect(window.showInactive).toHaveBeenCalled();
    expect(window.moveTop).toHaveBeenCalled();
  });

  it('resizes the toolbar window to the width reported by its renderer', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');
    const onAction = vi.fn();

    control.showRecordingControlBrowserWindow(
      { ...state, mode: 'recording' },
      { x: 400, y: 24 },
      onAction
    );
    const window = windows[0];
    window.handlers['webContents:did-finish-load'][0]();
    ipcOn['recording-control:ready']({ sender: window.webContents });
    await settle();

    ipcOn['recording-control:content-width'](
      { sender: window.webContents },
      196
    );

    expect(window.setBounds).toHaveBeenLastCalledWith({
      x: 400 - Math.round((300 - 196) / 2) - 16,
      y: 24,
      width: 300 + 16 * 2,
      height: 52,
    });
  });

  it('ignores content-width reports from other renderers', async () => {
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
    ipcOn['recording-control:ready']({ sender: window.webContents });
    await settle();
    window.setBounds.mockClear();

    ipcOn['recording-control:content-width']({ sender: { id: 999 } }, 196);

    expect(window.setBounds).not.toHaveBeenCalled();
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

  it('serves devices and expands only below the toolbar for HeroUI menus', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');
    const onAction = vi.fn();
    control.showRecordingControlBrowserWindow(
      state,
      { x: 400, y: 24 },
      onAction
    );
    const window = windows[0];

    await expect(
      ipcHandle['recording-control:devices']({ sender: {} }, 'microphone')
    ).resolves.toEqual({
      microphones: [],
      cameras: [],
      defaultMicrophoneId: null,
      defaultCameraId: null,
    });
    expect(mockListMediaDevices).not.toHaveBeenCalled();

    await expect(
      ipcHandle['recording-control:devices'](
        { sender: window.webContents },
        'microphone'
      )
    ).resolves.toEqual({
      microphones: [{ id: 'mic-1', label: 'Microphone 1' }],
      cameras: [{ id: 'camera-1', label: 'Camera 1' }],
      defaultMicrophoneId: 'mic-1',
      defaultCameraId: 'camera-1',
    });
    expect(mockListMediaDevices).toHaveBeenCalledWith(['microphone']);

    await expect(
      ipcHandle['recording-control:devices'](
        { sender: window.webContents },
        'camera'
      )
    ).resolves.toEqual({
      microphones: [{ id: 'mic-1', label: 'Microphone 1' }],
      cameras: [{ id: 'camera-1', label: 'Camera 1' }],
      defaultMicrophoneId: 'mic-1',
      defaultCameraId: 'camera-1',
    });
    expect(mockListMediaDevices).toHaveBeenLastCalledWith(['camera']);

    await expect(
      ipcHandle['recording-control:devices'](
        { sender: window.webContents },
        'unknown'
      )
    ).resolves.toEqual({
      microphones: [],
      cameras: [],
      defaultMicrophoneId: null,
      defaultCameraId: null,
    });
    expect(mockListMediaDevices).toHaveBeenCalledTimes(2);

    ipcOn['recording-control:device-menu-open'](
      { sender: window.webContents },
      true
    );
    expect(window.setBounds).toHaveBeenLastCalledWith({
      x: 352,
      y: 24,
      width: 332,
      height: 300,
    });

    ipcOn['recording-control:action'](
      { sender: window.webContents },
      'select-mic',
      {
        deviceId: 'mic-1',
        deviceName: 'Microphone 1',
      }
    );
    expect(onAction).toHaveBeenCalledWith('select-mic', {
      deviceId: 'mic-1',
      deviceName: 'Microphone 1',
    });

    ipcOn['recording-control:device-menu-open'](
      { sender: window.webContents },
      false
    );
    expect(window.setBounds).toHaveBeenLastCalledWith({
      x: 352,
      y: 24,
      width: 332,
      height: 52,
    });
  });

  it('renders microphone and camera menu triggers', async () => {
    vi.stubGlobal('window', { appPlatform: 'win32' });
    const React = await import('react');
    vi.stubGlobal('React', React);
    const { renderToStaticMarkup } = await import('react-dom/server');
    const { default: RecordingControlWindow } =
      await import('@/renderer/windows/recording-control-window');

    const markup = renderToStaticMarkup(
      React.createElement(RecordingControlWindow, { params: state })
    );

    expect(markup).toContain('aria-label="Select microphone"');
    expect(markup).toContain('aria-label="Select camera"');
    expect(markup).not.toContain('aria-label="Select iPhone or iPad"');
    expect(
      markup.match(
        /inline-flex h-8 w-12 min-w-12 flex-row items-center justify-center gap-1 rounded-3xl/g
      )
    ).toHaveLength(2);
    expect(markup).toContain('data-slot="dropdown-trigger"');
  }, 30000);

  it('renders the iOS device trigger only on macOS', async () => {
    vi.stubGlobal('window', { appPlatform: 'darwin' });
    const React = await import('react');
    vi.stubGlobal('React', React);
    const { renderToStaticMarkup } = await import('react-dom/server');
    const { default: RecordingControlWindow } =
      await import('@/renderer/windows/recording-control-window');

    const markup = renderToStaticMarkup(
      React.createElement(RecordingControlWindow, { params: state })
    );

    expect(markup).toContain('aria-label="Select iPhone or iPad"');
    expect(
      markup.match(
        /inline-flex h-8 w-12 min-w-12 flex-row items-center justify-center gap-1 rounded-3xl/g
      )
    ).toHaveLength(3);
  }, 30000);

  it('renders the live device triggers during recording on macOS', async () => {
    vi.stubGlobal('window', { appPlatform: 'darwin' });
    const React = await import('react');
    vi.stubGlobal('React', React);
    const { renderToStaticMarkup } = await import('react-dom/server');
    const { default: RecordingControlWindow } =
      await import('@/renderer/windows/recording-control-window');

    const markup = renderToStaticMarkup(
      React.createElement(RecordingControlWindow, {
        params: { ...state, mode: 'recording', cameraLocked: true },
      })
    );

    expect(markup).toContain('aria-label="Select microphone"');
    expect(markup).toContain('aria-label="Select camera"');
    expect(markup).toContain('Turn system sounds');
    expect(markup).toContain('aria-label="Stop recording"');
  }, 30000);

  it('serves iOS devices only to its own renderer', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');
    control.showRecordingControlBrowserWindow(
      state,
      { x: 400, y: 24 },
      vi.fn()
    );
    const window = windows[0];

    await expect(
      ipcHandle['recording-control:ios-devices']({ sender: {} })
    ).resolves.toEqual([]);
    expect(mockListIOSDevices).not.toHaveBeenCalled();

    await expect(
      ipcHandle['recording-control:ios-devices']({
        sender: window.webContents,
      })
    ).resolves.toEqual([{ id: 'ios-1', label: 'iPhone' }]);
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
      x: 302,
      y: 24,
      width: 432,
      height: 52,
    });
    expect(window.setParentWindow).toHaveBeenLastCalledWith(null);
    expect(window.webContents.send).toHaveBeenCalledWith(
      'recording-control:update',
      { mode: 'recording' }
    );
  });

  it('does not show the already visible toolbar again when recording starts', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');
    control.showRecordingControlBrowserWindow(
      state,
      { x: 400, y: 24 },
      vi.fn()
    );
    const window = windows[0];
    window.handlers['webContents:did-finish-load'][0]();
    ipcOn['recording-control:ready']({ sender: window.webContents });
    await settle();

    window.showInactive.mockClear();
    control.showRecordingControlBrowserWindow(
      { ...state, mode: 'recording' },
      { x: 318, y: 24 },
      vi.fn()
    );

    expect(window.showInactive).not.toHaveBeenCalled();
    expect(window.moveTop).toHaveBeenCalled();
  });

  it('does not resurrect a toolbar reveal that was followed by a hide', async () => {
    let resolveTransitions!: (value: { disabled: boolean }) => void;
    mockDaemonCall.mockResolvedValueOnce({ disabled: true });
    mockDaemonCall.mockImplementationOnce(
      () =>
        new Promise(resolve => {
          resolveTransitions = resolve;
        })
    );

    const control =
      await import('@/main/capture/video/recording-control-window');
    control.showRecordingControlBrowserWindow(
      state,
      { x: 400, y: 24 },
      vi.fn()
    );
    const window = windows[0];
    window.handlers['webContents:did-finish-load'][0]();
    ipcOn['recording-control:ready']({ sender: window.webContents });
    control.hideRecordingControlBrowserWindow();

    resolveTransitions({ disabled: true });
    await settle();

    expect(window.showInactive).not.toHaveBeenCalled();
  });

  it('clears the parent without hiding the toolbar', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');
    control.showRecordingControlBrowserWindow(
      state,
      { x: 400, y: 24 },
      vi.fn()
    );
    const window = windows[0];
    expect(window.setParentWindow).toHaveBeenLastCalledWith(overlayParent);

    control.clearRecordingControlBrowserWindowParent();

    expect(window.setParentWindow).toHaveBeenLastCalledWith(null);
    expect(window.hide).not.toHaveBeenCalled();
  });

  it('ignores clearing the parent when no toolbar exists', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');

    expect(() =>
      control.clearRecordingControlBrowserWindowParent()
    ).not.toThrow();
    expect(windows).toHaveLength(0);
  });

  it('hides the reusable window and resets its session state', async () => {
    const control =
      await import('@/main/capture/video/recording-control-window');
    const onAction = vi.fn();
    control.showRecordingControlBrowserWindow(
      { ...state, targetName: 'Preview' },
      { x: 400, y: 24 },
      onAction
    );
    const window = windows[0];
    control.updateRecordingControlBrowserWindow({
      mode: 'recording',
      isPaused: true,
      isStarting: true,
      elapsedSeconds: 42,
    });

    control.hideRecordingControlBrowserWindow();
    ipcOn['recording-control:action']({ sender: window.webContents }, 'start');

    expect(window.setOpacity).toHaveBeenCalledWith(0);
    expect(window.hide).toHaveBeenCalled();
    expect(window.webContents.send).toHaveBeenLastCalledWith(
      'recording-control:update',
      {
        ...state,
        mode: 'pre-recording',
        targetName: null,
        cameraLocked: false,
        isPaused: false,
        isStarting: false,
        elapsedSeconds: 0,
      }
    );
    expect(onAction).not.toHaveBeenCalled();
  });
});
