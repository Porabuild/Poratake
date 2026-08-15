import { beforeEach, describe, expect, it, vi } from 'vitest';

const windows: MockBrowserWindow[] = [];
const ipcOn: Record<string, (...args: unknown[]) => void> = {};
const mockSendWindowLoad = vi.fn();
const mockGlobalShortcutRegister = vi.fn();
const mockGlobalShortcutUnregister = vi.fn();
const mockRetainOverlayHandoffWindow = vi.fn();
const mockConcealOverlayHandoff = vi.fn();
let borrowedOverlay: MockBrowserWindow;

class MockBrowserWindow {
  static nextId = 1;

  options: Electron.BrowserWindowConstructorOptions;
  handlers: Record<string, Array<(...args: unknown[]) => void>> = {};
  destroyed = false;
  visible = false;
  excludedFromShownWindowsMenu = false;
  webContents = {
    id: MockBrowserWindow.nextId++,
    on: vi.fn((event: string, handler: (...args: unknown[]) => void) => {
      this.handlers[`webContents:${event}`] ??= [];
      this.handlers[`webContents:${event}`].push(handler);
    }),
    send: vi.fn(),
  };
  setAlwaysOnTop = vi.fn();
  setBounds = vi.fn();
  setIgnoreMouseEvents = vi.fn();
  setContentProtection = vi.fn();
  setOpacity = vi.fn();
  showInactive = vi.fn(() => {
    this.visible = true;
  });
  moveTop = vi.fn();
  loadURL = vi.fn();
  loadFile = vi.fn();
  destroy = vi.fn(() => {
    this.destroyed = true;
  });
  isDestroyed = vi.fn(() => this.destroyed);
  isVisible = vi.fn(() => this.visible);
  on = vi.fn((event: string, handler: (...args: unknown[]) => void) => {
    this.handlers[event] ??= [];
    this.handlers[event].push(handler);
  });

  constructor(options: Electron.BrowserWindowConstructorOptions) {
    this.options = options;
    windows.push(this);
  }
}

const display = {
  id: 7,
  bounds: { x: 0, y: 0, width: 1920, height: 1080 },
};

vi.mock('electron', () => ({
  BrowserWindow: MockBrowserWindow,
  globalShortcut: {
    register: (...args: unknown[]) => mockGlobalShortcutRegister(...args),
    unregister: (...args: unknown[]) => mockGlobalShortcutUnregister(...args),
  },
  ipcMain: {
    on: (channel: string, handler: (...args: unknown[]) => void) => {
      ipcOn[channel] = handler;
    },
  },
  screen: {
    getAllDisplays: () => [display],
    getPrimaryDisplay: () => display,
  },
}));

vi.mock('@/main/utils/env', () => ({
  isDev: true,
  devServerUrl: 'http://localhost:5173',
}));

vi.mock('@/main/utils/window-load', () => ({
  sendWindowLoad: (...args: unknown[]) => mockSendWindowLoad(...args),
}));

vi.mock('@/main/capture/area-overlay', () => ({
  retainOverlayHandoffWindow: (...args: unknown[]) =>
    mockRetainOverlayHandoffWindow(...args),
  concealOverlayHandoff: () => mockConcealOverlayHandoff(),
}));

function finishLoading(window: MockBrowserWindow): void {
  window.handlers['webContents:did-finish-load'][0]();
}

describe('scroll capture window', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    windows.splice(0);
    Object.keys(ipcOn).forEach(key => delete ipcOn[key]);
    borrowedOverlay = new MockBrowserWindow({});
    borrowedOverlay.visible = true;
    windows.splice(0);
    mockRetainOverlayHandoffWindow.mockReturnValue(borrowedOverlay);
  });

  it('prewarms one invisible click-through control window', async () => {
    const { prewarmScrollCaptureControl } =
      await import('@/main/capture/scroll-capture/scroll-capture-window');

    prewarmScrollCaptureControl();
    expect(windows).toHaveLength(1);

    const control = windows[0];
    expect(control.options).toEqual(
      expect.objectContaining({
        opacity: 0,
        show: false,
        hiddenInMissionControl: true,
        skipTaskbar: true,
      })
    );
    finishLoading(control);
    expect(control.showInactive).toHaveBeenCalledTimes(1);
    expect(control.setOpacity).toHaveBeenLastCalledWith(0);
    expect(control.setIgnoreMouseEvents).toHaveBeenLastCalledWith(true, {
      forward: true,
    });
  });

  it('reuses the selection overlay and reveals the warm control', async () => {
    const captureWindow =
      await import('@/main/capture/scroll-capture/scroll-capture-window');
    captureWindow.prewarmScrollCaptureControl();
    windows.forEach(finishLoading);
    mockSendWindowLoad.mockClear();

    const params = {
      displayId: 7,
      area: { x: 100, y: 100, width: 400, height: 300, displayId: 7 },
    };
    expect(captureWindow.showScrollCaptureOverlay(params, vi.fn())).toBe(true);

    expect(windows).toHaveLength(1);
    expect(mockRetainOverlayHandoffWindow).toHaveBeenCalledWith(7);
    expect(windows[0].setBounds).toHaveBeenLastCalledWith({
      x: 216,
      y: 416,
      width: 168,
      height: 52,
    });
    expect(borrowedOverlay.webContents.send).toHaveBeenCalledWith(
      'scroll-capture:begin',
      { ...params, displayBounds: display.bounds }
    );
    expect(mockSendWindowLoad).toHaveBeenCalledWith(windows[0].webContents, {
      type: 'scroll-capture-control',
      params: { ...params, displayBounds: display.bounds },
    });
    expect(borrowedOverlay.setIgnoreMouseEvents).toHaveBeenLastCalledWith(
      true,
      {
        forward: true,
      }
    );
    expect(windows[0].setIgnoreMouseEvents).toHaveBeenLastCalledWith(false, {
      forward: false,
    });
    expect(borrowedOverlay.setOpacity).toHaveBeenLastCalledWith(1);
    expect(windows[0].setOpacity).toHaveBeenLastCalledWith(1);
  });

  it('parks the windows for reuse when capture ends', async () => {
    const captureWindow =
      await import('@/main/capture/scroll-capture/scroll-capture-window');
    captureWindow.prewarmScrollCaptureControl();
    windows.forEach(finishLoading);
    captureWindow.showScrollCaptureOverlay(
      {
        displayId: 7,
        area: { x: 100, y: 100, width: 400, height: 300, displayId: 7 },
      },
      vi.fn()
    );

    captureWindow.hideScrollCaptureOverlay();

    expect(windows[0].destroy).not.toHaveBeenCalled();
    expect(windows[0].setOpacity).toHaveBeenLastCalledWith(0);
    expect(windows[0].webContents.send).toHaveBeenCalledWith(
      'scroll-capture:update',
      expect.objectContaining({ preview: null, frameCount: 0 })
    );
    expect(borrowedOverlay.webContents.send).toHaveBeenCalledWith(
      'scroll-capture:update',
      expect.objectContaining({ preview: null, frameCount: 0 })
    );
    expect(mockConcealOverlayHandoff).toHaveBeenCalledTimes(1);
  });
});
