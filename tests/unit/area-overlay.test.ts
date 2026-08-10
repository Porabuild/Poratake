import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockCaptureRegionToFile = vi.fn();
const mockFreezeScreen = vi.fn();
const mockReleaseScreen = vi.fn();
const mockIsFreezeScreenEnabled = vi.fn();
const mockDaemonCall = vi.fn();
const mockGlobalShortcutRegister = vi.fn();
const mockGlobalShortcutUnregister = vi.fn();
const mockGetAllDisplays = vi.fn();
const mockGetCursorScreenPoint = vi.fn();
const mockGetDisplayNearestPoint = vi.fn();

const ipcHandlers = new Map<string, (event: unknown, data?: unknown) => void>();
const overlayWindows: MockBrowserWindow[] = [];
const loadHandlers = new Map<number, () => void>();

class MockBrowserWindow {
  static nextWebContentsId = 1;

  options: Record<string, unknown>;
  destroyed = false;
  visible = false;
  webContents = {
    id: MockBrowserWindow.nextWebContentsId++,
    on: (event: string, handler: () => void) => {
      if (event === 'did-finish-load') {
        loadHandlers.set(this.webContents.id, handler);
      }
    },
    send: vi.fn(),
  };

  showInactive = vi.fn(() => {
    this.visible = true;
  });
  hide = vi.fn(() => {
    this.visible = false;
  });
  focus = vi.fn();
  moveTop = vi.fn();
  destroy = vi.fn(() => {
    this.destroyed = true;
  });
  isDestroyed = () => this.destroyed;
  isVisible = () => this.visible;
  getNativeWindowHandle = () => {
    const handle = Buffer.alloc(8);
    handle.writeBigUInt64LE(BigInt(this.webContents.id));
    return handle;
  };
  setAlwaysOnTop = vi.fn();
  setBounds = vi.fn();
  setIgnoreMouseEvents = vi.fn();
  setOpacity = vi.fn();
  loadURL = vi.fn();
  loadFile = vi.fn();
  on = vi.fn();

  constructor(options: Record<string, unknown>) {
    this.options = options;
    overlayWindows.push(this);
  }
}

vi.mock('electron', () => ({
  app: { getPath: () => '/tmp' },
  BrowserWindow: MockBrowserWindow,
  globalShortcut: {
    register: (...a: unknown[]) => mockGlobalShortcutRegister(...a),
    unregister: (...a: unknown[]) => mockGlobalShortcutUnregister(...a),
  },
  ipcMain: {
    on: (
      channel: string,
      handler: (event: unknown, data?: unknown) => void
    ) => {
      ipcHandlers.set(channel, handler);
    },
  },
  screen: {
    getAllDisplays: () => mockGetAllDisplays(),
    getCursorScreenPoint: () => mockGetCursorScreenPoint(),
    getDisplayNearestPoint: (...a: unknown[]) =>
      mockGetDisplayNearestPoint(...a),
  },
}));

vi.mock('@/main/utils/env', () => ({
  isDev: false,
  devServerUrl: null,
}));

vi.mock('@/main/capture/screenshot/native-capture', () => ({
  captureRegionToFile: (...a: unknown[]) => mockCaptureRegionToFile(...a),
}));

vi.mock('@/main/capture/freeze-screen', () => ({
  freezeScreen: (...a: unknown[]) => mockFreezeScreen(...a),
  releaseScreen: (...a: unknown[]) => mockReleaseScreen(...a),
}));

vi.mock('@/main/capture/freeze-screen/preference', () => ({
  isFreezeScreenEnabled: () => mockIsFreezeScreenEnabled(),
}));

vi.mock('@/main/daemon', () => ({
  daemon: { call: (...a: unknown[]) => mockDaemonCall(...a) },
}));

const display = {
  id: 7,
  bounds: { x: 100, y: 50, width: 1920, height: 1080 },
};

function settle(): Promise<void> {
  return new Promise(resolve => setImmediate(resolve));
}

function fire(channel: string, webContentsId: number, data?: unknown): void {
  ipcHandlers.get(channel)?.({ sender: { id: webContentsId } }, data);
}

function prepare(window: MockBrowserWindow): void {
  fire('area-overlay:renderer-prepared', window.webContents.id);
}

describe('area overlay', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    ipcHandlers.clear();
    overlayWindows.length = 0;
    loadHandlers.clear();
    mockGetAllDisplays.mockReturnValue([display]);
    mockGetCursorScreenPoint.mockReturnValue({ x: 200, y: 100 });
    mockGetDisplayNearestPoint.mockReturnValue(display);
    mockCaptureRegionToFile.mockResolvedValue(true);
    mockFreezeScreen.mockResolvedValue(true);
    mockReleaseScreen.mockResolvedValue(true);
    mockIsFreezeScreenEnabled.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ disabled: true });
    mockGlobalShortcutRegister.mockReturnValue(true);
  });

  it('mounts the overlay UI while the prewarmed window is hidden', async () => {
    const module = await import('@/main/capture/area-overlay');
    module.prewarmAreaOverlay();
    await settle();

    expect(overlayWindows).toHaveLength(1);
    expect(mockDaemonCall).toHaveBeenCalledWith(
      'area-selector',
      'disableWindowTransitions',
      { windowHandle: overlayWindows[0].webContents.id.toString() }
    );
    expect(overlayWindows[0].showInactive).not.toHaveBeenCalled();
    expect(overlayWindows[0].hide).toHaveBeenCalled();
    expect(overlayWindows[0].isVisible()).toBe(false);
    expect(overlayWindows[0].setOpacity).toHaveBeenLastCalledWith(0);

    fire('area-overlay:renderer-mounted', overlayWindows[0].webContents.id);
    prepare(overlayWindows[0]);

    expect(overlayWindows[0].webContents.send).toHaveBeenCalledWith('load', {
      type: 'area-overlay',
      params: {
        sessionId: 0,
        displayId: display.id,
        imageUrl: null,
        interactive: true,
        showPrompt: true,
        aspectRatio: null,
        toolbar: null,
        rect: null,
      },
    });
  });

  it('blocks input without activating the selected app', async () => {
    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay();
    await settle();

    expect(mockFreezeScreen).toHaveBeenCalledTimes(1);
    expect(mockCaptureRegionToFile).not.toHaveBeenCalled();
    expect(overlayWindows).toHaveLength(1);
    expect(overlayWindows[0].options).toMatchObject({
      transparent: true,
      backgroundColor: '#00000000',
    });
    expect(overlayWindows[0].options.opacity).toBe(0);
    expect(overlayWindows[0].options).toMatchObject({
      webPreferences: { webSecurity: true },
    });
    expect(overlayWindows[0].showInactive).toHaveBeenCalledTimes(1);
    expect(overlayWindows[0].setIgnoreMouseEvents).toHaveBeenLastCalledWith(
      false
    );
    expect(mockGlobalShortcutRegister).toHaveBeenCalledWith(
      'Escape',
      expect.any(Function)
    );

    fire('area-overlay:ready', overlayWindows[0].webContents.id);
    expect(overlayWindows[0].showInactive).toHaveBeenCalledTimes(1);
    expect(overlayWindows[0].setOpacity).toHaveBeenCalledWith(1);
    expect(overlayWindows[0].setIgnoreMouseEvents).toHaveBeenLastCalledWith(
      false
    );
    expect(overlayWindows[0].focus).not.toHaveBeenCalled();
    expect(overlayWindows[0].moveTop).toHaveBeenCalled();

    fire('area-overlay:cancel', overlayWindows[0].webContents.id);
    expect(await selection).toBeNull();
    expect(mockGlobalShortcutUnregister).toHaveBeenCalledWith('Escape');
  });

  it('resolves display-local coordinates as a global rect', async () => {
    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay();
    await settle();

    fire('area-overlay:confirm', overlayWindows[0].webContents.id, {
      displayId: display.id,
      x: 10,
      y: 20,
      width: 300,
      height: 200,
    });

    const result = await selection;
    expect(result?.rect).toEqual({
      x: 110,
      y: 70,
      width: 300,
      height: 200,
    });
    expect(result?.frozen).toBe(true);
    expect(mockReleaseScreen).not.toHaveBeenCalled();
  });

  it('returns Escape handling to the color picker while it is active', async () => {
    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay();
    await settle();

    fire('area-overlay:color-picker', overlayWindows[0].webContents.id, true);
    expect(mockGlobalShortcutUnregister).toHaveBeenLastCalledWith('Escape');

    fire('area-overlay:color-picker', overlayWindows[0].webContents.id, false);
    expect(mockGlobalShortcutRegister).toHaveBeenCalledTimes(2);

    fire('area-overlay:cancel', overlayWindows[0].webContents.id);
    expect(await selection).toBeNull();
  });

  it('releases the native freeze on cancellation', async () => {
    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay();
    await settle();

    fire('area-overlay:cancel', overlayWindows[0].webContents.id);
    await selection;

    expect(mockReleaseScreen).toHaveBeenCalled();
    expect(overlayWindows[0].hide).toHaveBeenCalled();
    expect(overlayWindows[0].setOpacity).toHaveBeenLastCalledWith(0);
    expect(overlayWindows[0].setIgnoreMouseEvents).toHaveBeenLastCalledWith(
      true
    );
  });

  it('ignores geometry that leaves the display', async () => {
    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay();
    await settle();

    fire('area-overlay:confirm', overlayWindows[0].webContents.id, {
      displayId: display.id,
      x: 1900,
      y: 20,
      width: 300,
      height: 200,
    });
    expect(module.isOverlayActive()).toBe(true);

    fire('area-overlay:cancel', overlayWindows[0].webContents.id);
    expect(await selection).toBeNull();
  });

  it('continues with a live selector when the native freeze fails', async () => {
    mockFreezeScreen.mockResolvedValue(false);
    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay();
    await settle();

    fire('area-overlay:confirm', overlayWindows[0].webContents.id, {
      displayId: display.id,
      x: 10,
      y: 20,
      width: 300,
      height: 200,
    });

    expect((await selection)?.frozen).toBe(false);
    expect(mockReleaseScreen).not.toHaveBeenCalled();
  });

  it('keeps the prewarmed selector hidden until the native freeze is ready', async () => {
    let finishFreeze!: (frozen: boolean) => void;
    mockFreezeScreen.mockImplementation(
      () => new Promise<boolean>(resolve => (finishFreeze = resolve))
    );

    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay();
    await settle();

    expect(mockFreezeScreen).toHaveBeenCalled();
    expect(overlayWindows).toHaveLength(1);
    expect(overlayWindows[0].showInactive).toHaveBeenCalledTimes(1);
    expect(overlayWindows[0].setOpacity).toHaveBeenLastCalledWith(0);

    finishFreeze(true);
    await settle();

    expect(overlayWindows).toHaveLength(1);
    prepare(overlayWindows[0]);
    expect(overlayWindows[0].loadFile).toHaveBeenCalled();
    expect(overlayWindows[0].webContents.send).toHaveBeenCalledWith(
      'load',
      expect.objectContaining({ type: 'area-overlay' })
    );

    fire('area-overlay:cancel', overlayWindows[0].webContents.id);
    await selection;
  });

  it('waits for a pending native release before starting another freeze', async () => {
    let finishRelease!: (released: boolean) => void;
    mockReleaseScreen.mockImplementationOnce(
      () => new Promise<boolean>(resolve => (finishRelease = resolve))
    );

    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay();
    await settle();

    fire('area-overlay:cancel', overlayWindows[0].webContents.id);
    const nextSelection = module.selectAreaWithOverlay();

    await settle();
    expect(mockReleaseScreen).toHaveBeenCalledTimes(1);
    expect(overlayWindows).toHaveLength(1);

    finishRelease(true);
    expect(await selection).toBeNull();

    await settle();
    expect(overlayWindows).toHaveLength(1);
    fire('area-overlay:cancel', overlayWindows[0].webContents.id);
    expect(await nextSelection).toBeNull();
  });

  it('reuses the prepared renderer with fresh state for the next capture', async () => {
    const module = await import('@/main/capture/area-overlay');
    const firstSelection = module.selectAreaWithOverlay();
    await settle();

    fire('area-overlay:renderer-mounted', overlayWindows[0].webContents.id);
    expect(overlayWindows[0].webContents.send).toHaveBeenCalledWith(
      'area-overlay:prepare-renderer'
    );
    prepare(overlayWindows[0]);

    const firstLoad = overlayWindows[0].webContents.send.mock.calls.find(
      ([channel]) => channel === 'load'
    );
    const firstSessionId = firstLoad?.[1].params.sessionId;

    fire('area-overlay:selected', overlayWindows[0].webContents.id, {
      displayId: display.id,
      x: 10,
      y: 20,
      width: 300,
      height: 200,
    });
    fire('area-overlay:cancel', overlayWindows[0].webContents.id);
    await firstSelection;
    expect(overlayWindows[0].webContents.send).toHaveBeenCalledWith(
      'area-overlay:set-rect',
      { rect: null }
    );

    const secondSelection = module.selectAreaWithOverlay();
    await settle();

    expect(overlayWindows).toHaveLength(1);
    const loadCalls = overlayWindows[0].webContents.send.mock.calls.filter(
      ([channel]) => channel === 'load'
    );
    expect(loadCalls).toHaveLength(2);
    expect(loadCalls[1][1].params.sessionId).not.toBe(firstSessionId);
    expect(overlayWindows[0].showInactive).toHaveBeenCalledTimes(2);
    expect(overlayWindows[0].hide).toHaveBeenCalled();

    fire('area-overlay:cancel', overlayWindows[0].webContents.id);
    await secondSelection;
  });

  it('selects on a live transparent overlay when freezing is disabled', async () => {
    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay({ freeze: false });
    await settle();

    expect(mockCaptureRegionToFile).not.toHaveBeenCalled();
    expect(overlayWindows[0].options).toMatchObject({
      transparent: true,
      backgroundColor: '#00000000',
    });

    prepare(overlayWindows[0]);
    expect(overlayWindows[0].webContents.send).toHaveBeenCalledWith('load', {
      type: 'area-overlay',
      params: {
        sessionId: expect.any(Number),
        displayId: display.id,
        imageUrl: null,
        interactive: false,
        showPrompt: true,
        aspectRatio: null,
        toolbar: null,
        rect: null,
      },
    });

    fire('area-overlay:confirm', overlayWindows[0].webContents.id, {
      displayId: display.id,
      x: 10,
      y: 20,
      width: 300,
      height: 200,
    });

    const result = await selection;
    expect(result?.rect).toEqual({ x: 110, y: 70, width: 300, height: 200 });

    await result?.release();
    expect(mockReleaseScreen).not.toHaveBeenCalled();
  });

  it('leaves the daemon untouched when a live overlay is cancelled', async () => {
    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay({ freeze: false });
    await settle();

    fire('area-overlay:cancel', overlayWindows[0].webContents.id);

    expect(await selection).toBeNull();
    expect(mockReleaseScreen).not.toHaveBeenCalled();
    expect(overlayWindows[0].hide).toHaveBeenCalled();
    expect(overlayWindows[0].setOpacity).toHaveBeenLastCalledWith(0);
  });

  it('crops the saved file from the retained frame and then releases it', async () => {
    const module = await import('@/main/capture/area-overlay');
    const captured = module.captureAreaToFile('/tmp/shot.png');
    await settle();

    fire('area-overlay:confirm', overlayWindows[0].webContents.id, {
      displayId: display.id,
      x: 10,
      y: 20,
      width: 300,
      height: 200,
    });

    expect(await captured).toBe(true);
    expect(mockCaptureRegionToFile).toHaveBeenCalledWith(
      { x: 110, y: 70, width: 300, height: 200 },
      '/tmp/shot.png',
      { cached: true }
    );
    expect(mockReleaseScreen).toHaveBeenCalled();
  });

  it('captures the live screen when the freeze screen setting is disabled', async () => {
    mockIsFreezeScreenEnabled.mockReturnValue(false);

    const module = await import('@/main/capture/area-overlay');
    const captured = module.captureAreaToFile('/tmp/shot.png');
    await settle();

    expect(mockCaptureRegionToFile).not.toHaveBeenCalled();
    expect(overlayWindows[0].options).toMatchObject({ transparent: true });

    fire('area-overlay:confirm', overlayWindows[0].webContents.id, {
      displayId: display.id,
      x: 10,
      y: 20,
      width: 300,
      height: 200,
    });

    expect(await captured).toBe(true);
    expect(mockCaptureRegionToFile).toHaveBeenCalledWith(
      { x: 110, y: 70, width: 300, height: 200 },
      '/tmp/shot.png',
      { cached: false }
    );
    expect(overlayWindows[0].hide).toHaveBeenCalled();
    expect(overlayWindows[0].setOpacity).toHaveBeenLastCalledWith(0);
    expect(mockReleaseScreen).not.toHaveBeenCalled();
  });
});
