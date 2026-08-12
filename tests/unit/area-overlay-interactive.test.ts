import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockGetAllDisplays = vi.fn();
const mockGetCursorScreenPoint = vi.fn();
const mockGetDisplayNearestPoint = vi.fn();
const mockGetDisplayMatching = vi.fn();
const mockDaemonCall = vi.fn();
const mockGlobalShortcutRegister = vi.fn();
const mockGlobalShortcutUnregister = vi.fn();

const ipcHandlers = new Map<string, (event: unknown, data?: unknown) => void>();
const overlayWindows: MockBrowserWindow[] = [];
const loadHandlers = new Map<number, () => void>();
const commandLineSwitches = new Set<string>();

class MockBrowserWindow {
  static nextWebContentsId = 1;

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

  constructor() {
    overlayWindows.push(this);
  }
}

vi.mock('electron', () => ({
  app: {
    commandLine: {
      appendSwitch: (value: string) => commandLineSwitches.add(value),
      hasSwitch: (value: string) => commandLineSwitches.has(value),
      removeSwitch: (value: string) => commandLineSwitches.delete(value),
    },
    getPath: () => '/tmp',
  },
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
    getDisplayMatching: (...a: unknown[]) => mockGetDisplayMatching(...a),
  },
}));

vi.mock('fs', () => ({
  default: { rmSync: vi.fn() },
  rmSync: vi.fn(),
}));

vi.mock('@/main/utils/env', () => ({
  isDev: false,
  devServerUrl: null,
}));

vi.mock('@/main/capture/screenshot/native-capture', () => ({
  captureRegionToFile: vi.fn(),
}));

vi.mock('@/main/capture/freeze-screen/preference', () => ({
  isFreezeScreenEnabled: () => true,
}));

vi.mock('@/main/daemon', () => ({
  daemon: { call: (...a: unknown[]) => mockDaemonCall(...a) },
}));

const primary = { id: 7, bounds: { x: 0, y: 0, width: 1920, height: 1080 } };
const secondary = {
  id: 8,
  bounds: { x: 1920, y: 0, width: 1280, height: 1024 },
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

function windowFor(displayIndex: number): MockBrowserWindow {
  return overlayWindows[displayIndex];
}

describe('interactive area overlay', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    ipcHandlers.clear();
    overlayWindows.length = 0;
    loadHandlers.clear();
    commandLineSwitches.clear();
    mockGetAllDisplays.mockReturnValue([primary, secondary]);
    mockGetCursorScreenPoint.mockReturnValue({ x: 10, y: 10 });
    mockGetDisplayNearestPoint.mockReturnValue(primary);
    mockDaemonCall.mockImplementation(
      (_module: string, method: string, params: { windowHandle?: string }) => {
        if (method === 'showWindowWithoutTransitions') {
          const window = overlayWindows.find(
            item => item.webContents.id.toString() === params.windowHandle
          );
          if (window) window.visible = true;
        }
        if (method === 'hideWindowWithoutTransitions') {
          const window = overlayWindows.find(
            item => item.webContents.id.toString() === params.windowHandle
          );
          if (window) window.visible = false;
        }
        return Promise.resolve({ disabled: true });
      }
    );
    mockGlobalShortcutRegister.mockReturnValue(true);
    mockGetDisplayMatching.mockImplementation(
      (rect: { x: number; y: number }) =>
        rect.x >= secondary.bounds.x ? secondary : primary
    );
  });

  it('preselects a preset area and reports it once the overlay is on screen', async () => {
    const module = await import('@/main/capture/area-overlay');
    const onSelected = vi.fn();

    const session = module.startInteractiveOverlay({
      preset: { x: 2000, y: 100, width: 400, height: 300 },
      callbacks: { onSelected },
    });
    await settle();

    expect(windowFor(0).hide).not.toHaveBeenCalled();
    expect(windowFor(0).setOpacity).toHaveBeenLastCalledWith(1);
    expect(windowFor(0).setIgnoreMouseEvents).toHaveBeenLastCalledWith(true);
    expect(onSelected).not.toHaveBeenCalled();

    fire('area-overlay:ready', windowFor(1).webContents.id);

    expect(windowFor(1).showInactive).toHaveBeenCalledTimes(1);
    expect(onSelected).toHaveBeenCalledWith(
      expect.objectContaining({
        rect: { x: 2000, y: 100, width: 400, height: 300 },
      })
    );

    prepare(windowFor(1));
    expect(windowFor(1).webContents.send).toHaveBeenCalledWith('load', {
      type: 'area-overlay',
      params: {
        sessionId: expect.any(Number),
        displayId: secondary.id,
        imageUrl: null,
        interactive: true,
        showPrompt: true,
        aspectRatio: null,
        toolbar: null,
        rect: { displayId: 8, x: 80, y: 100, width: 400, height: 300 },
        pickTargets: null,
        prompt: null,
      },
    });

    module.cancelOverlaySelection();
    await session;
  });

  it('reports a hidden preset without showing its overlay window', async () => {
    const module = await import('@/main/capture/area-overlay');
    const onSelected = vi.fn();

    const session = module.startInteractiveOverlay({
      visible: false,
      preset: primary.bounds,
      callbacks: { onSelected },
    });
    await settle();

    fire('area-overlay:ready', windowFor(0).webContents.id);
    await settle();

    expect(onSelected).toHaveBeenCalledWith(
      expect.objectContaining({ rect: primary.bounds })
    );
    expect(
      mockDaemonCall.mock.calls.filter(
        ([, method]) => method === 'showWindowWithoutTransitions'
      )
    ).toHaveLength(0);
    expect(module.getActiveOverlayWindowAtPoint({ x: 10, y: 10 })).toBeNull();

    module.cancelOverlaySelection();
    await session;
  });

  it('delivers pick targets clipped to each display in local coordinates', async () => {
    const module = await import('@/main/capture/area-overlay');

    const session = module.startInteractiveOverlay({
      pickTargets: [
        { id: 11, rect: { x: 100, y: 100, width: 400, height: 300 } },
        { id: 12, rect: { x: 1800, y: 200, width: 400, height: 300 } },
        { id: 13, rect: { x: -500, y: -500, width: 100, height: 100 } },
      ],
      prompt: 'Click a window to select it · Esc to cancel',
    });
    await settle();

    prepare(windowFor(0));
    expect(windowFor(0).webContents.send).toHaveBeenCalledWith('load', {
      type: 'area-overlay',
      params: expect.objectContaining({
        pickTargets: [
          { id: 11, x: 100, y: 100, width: 400, height: 300 },
          { id: 12, x: 1800, y: 200, width: 120, height: 300 },
        ],
        prompt: 'Click a window to select it · Esc to cancel',
      }),
    });

    prepare(windowFor(1));
    expect(windowFor(1).webContents.send).toHaveBeenCalledWith('load', {
      type: 'area-overlay',
      params: expect.objectContaining({
        pickTargets: [{ id: 12, x: 0, y: 200, width: 280, height: 300 }],
        prompt: 'Click a window to select it · Esc to cancel',
      }),
    });

    module.cancelOverlaySelection();
    await session;
  });

  it('switches a running session between picking and free-form selection', async () => {
    const module = await import('@/main/capture/area-overlay');

    const session = module.startInteractiveOverlay({});
    await settle();

    fire('area-overlay:selected', windowFor(0).webContents.id, {
      displayId: primary.id,
      x: 10,
      y: 20,
      width: 100,
      height: 100,
    });
    expect(windowFor(1).setIgnoreMouseEvents).toHaveBeenLastCalledWith(true);

    module.setOverlayPickTargets(
      [{ id: 11, rect: { x: 100, y: 100, width: 400, height: 300 } }],
      'Click a window to select it · Esc to cancel'
    );

    expect(windowFor(0).webContents.send).toHaveBeenCalledWith(
      'area-overlay:set-pick-targets',
      {
        pickTargets: [{ id: 11, x: 100, y: 100, width: 400, height: 300 }],
        prompt: 'Click a window to select it · Esc to cancel',
      }
    );
    expect(windowFor(1).webContents.send).toHaveBeenCalledWith(
      'area-overlay:set-pick-targets',
      {
        pickTargets: [],
        prompt: 'Click a window to select it · Esc to cancel',
      }
    );

    await settle();
    expect(windowFor(1).setIgnoreMouseEvents).toHaveBeenLastCalledWith(false);

    module.setOverlayPickTargets(null, null);
    expect(windowFor(0).webContents.send).toHaveBeenCalledWith(
      'area-overlay:set-pick-targets',
      { pickTargets: null, prompt: null }
    );

    module.confirmOverlaySelection();
    expect(await session).toBeNull();
  });

  it('keeps pick mode active on displays without targets', async () => {
    const module = await import('@/main/capture/area-overlay');

    const session = module.startInteractiveOverlay({
      pickTargets: [
        { id: 11, rect: { x: 100, y: 100, width: 400, height: 300 } },
      ],
    });
    await settle();

    prepare(windowFor(1));
    expect(windowFor(1).webContents.send).toHaveBeenCalledWith('load', {
      type: 'area-overlay',
      params: expect.objectContaining({
        pickTargets: [],
        prompt: null,
      }),
    });

    module.cancelOverlaySelection();
    await session;
  });

  it('clamps a preset that overflows its display', async () => {
    const module = await import('@/main/capture/area-overlay');
    const onSelected = vi.fn();

    const session = module.startInteractiveOverlay({
      preset: { x: 1800, y: 900, width: 400, height: 400 },
      callbacks: { onSelected },
    });
    await settle();

    fire('area-overlay:ready', windowFor(0).webContents.id);

    expect(onSelected).toHaveBeenCalledWith(
      expect.objectContaining({
        rect: { x: 1520, y: 680, width: 400, height: 400 },
      })
    );

    module.cancelOverlaySelection();
    await session;
  });

  it('keeps a single display authoritative when the selection moves', async () => {
    const module = await import('@/main/capture/area-overlay');
    const onSelected = vi.fn();
    const onUpdated = vi.fn();

    const session = module.startInteractiveOverlay({
      callbacks: { onSelected, onUpdated },
    });
    await settle();

    fire('area-overlay:selected', windowFor(1).webContents.id, {
      displayId: secondary.id,
      x: 10,
      y: 20,
      width: 300,
      height: 200,
    });

    expect(mockDaemonCall).toHaveBeenCalledWith(
      'area-selector',
      'setWindowRegion',
      {
        windowHandle: windowFor(1).webContents.id.toString(),
        x: 10,
        y: 20,
        width: 300,
        height: 200,
        windowWidth: secondary.bounds.width,
        windowHeight: secondary.bounds.height,
        inset: 16,
      }
    );
    expect(onSelected).toHaveBeenCalledWith(
      expect.objectContaining({
        rect: { x: 1930, y: 20, width: 300, height: 200 },
      })
    );
    expect(windowFor(0).hide).not.toHaveBeenCalled();
    expect(windowFor(0).setOpacity).toHaveBeenLastCalledWith(1);
    expect(windowFor(0).setIgnoreMouseEvents).toHaveBeenLastCalledWith(true);
    expect(windowFor(0).webContents.send).toHaveBeenCalledWith(
      'area-overlay:set-rect',
      { rect: null }
    );
    expect(windowFor(1).webContents.send).not.toHaveBeenCalledWith(
      'area-overlay:set-rect',
      expect.anything()
    );

    fire('area-overlay:updated', windowFor(1).webContents.id, {
      displayId: secondary.id,
      x: 10,
      y: 30,
      width: 300,
      height: 200,
    });

    expect(onUpdated).toHaveBeenCalledWith(
      expect.objectContaining({
        rect: { x: 1930, y: 30, width: 300, height: 200 },
      })
    );

    module.confirmOverlaySelection();

    expect(await session).toEqual(
      expect.objectContaining({
        rect: { x: 1930, y: 30, width: 300, height: 200 },
      })
    );
    expect(windowFor(0).hide).not.toHaveBeenCalled();
    expect(windowFor(1).hide).not.toHaveBeenCalled();
    expect(windowFor(0).setOpacity).toHaveBeenLastCalledWith(1);
    expect(windowFor(1).setOpacity).toHaveBeenLastCalledWith(1);
  });

  it('ignores geometry that leaves the reporting display', async () => {
    const module = await import('@/main/capture/area-overlay');
    const onSelected = vi.fn();

    const session = module.startInteractiveOverlay({
      callbacks: { onSelected },
    });
    await settle();

    fire('area-overlay:selected', windowFor(0).webContents.id, {
      displayId: primary.id,
      x: 1900,
      y: 20,
      width: 300,
      height: 200,
    });

    expect(onSelected).not.toHaveBeenCalled();

    module.cancelOverlaySelection();
    await session;
  });

  it('pushes an externally updated rect to every overlay', async () => {
    const module = await import('@/main/capture/area-overlay');

    const session = module.startInteractiveOverlay({});
    await settle();

    expect(
      module.updateOverlaySelection({
        x: 200,
        y: 150,
        width: 640,
        height: 480,
      })
    ).toBe(true);

    expect(windowFor(0).webContents.send).toHaveBeenCalledWith(
      'area-overlay:set-rect',
      { rect: { displayId: 7, x: 200, y: 150, width: 640, height: 480 } }
    );
    expect(windowFor(1).webContents.send).toHaveBeenCalledWith(
      'area-overlay:set-rect',
      { rect: null }
    );

    module.confirmOverlaySelection();
    expect(await session).toEqual(
      expect.objectContaining({
        rect: { x: 200, y: 150, width: 640, height: 480 },
      })
    );
  });

  it('broadcasts aspect ratio changes and toggles visibility', async () => {
    const module = await import('@/main/capture/area-overlay');

    const session = module.startInteractiveOverlay({});
    await settle();

    fire('area-overlay:ready', windowFor(0).webContents.id);
    fire('area-overlay:ready', windowFor(1).webContents.id);

    module.setOverlayAspectRatio(16 / 9);
    expect(windowFor(0).webContents.send).toHaveBeenCalledWith(
      'area-overlay:set-aspect-ratio',
      { aspectRatio: 16 / 9 }
    );

    module.setOverlayVisible(false);
    expect(windowFor(0).hide).not.toHaveBeenCalled();
    expect(windowFor(1).hide).not.toHaveBeenCalled();
    expect(windowFor(0).setOpacity).toHaveBeenLastCalledWith(1);
    expect(windowFor(1).setOpacity).toHaveBeenLastCalledWith(1);
    expect(windowFor(0).setIgnoreMouseEvents).toHaveBeenLastCalledWith(true);
    expect(windowFor(1).setIgnoreMouseEvents).toHaveBeenLastCalledWith(true);

    module.setOverlayVisible(true);
    await settle();
    expect(windowFor(0).showInactive).toHaveBeenCalledTimes(1);
    expect(mockDaemonCall).toHaveBeenLastCalledWith(
      'area-selector',
      'showWindowWithoutTransitions',
      { windowHandle: windowFor(1).webContents.id.toString() }
    );
    expect(windowFor(0).setOpacity).toHaveBeenLastCalledWith(1);
    expect(windowFor(1).setOpacity).toHaveBeenLastCalledWith(1);
    expect(windowFor(0).setIgnoreMouseEvents).toHaveBeenLastCalledWith(false);
    expect(windowFor(1).setIgnoreMouseEvents).toHaveBeenLastCalledWith(false);

    module.cancelOverlaySelection();
    await session;
  });

  it('forwards valid toolbar actions and broadcasts toolbar changes', async () => {
    const module = await import('@/main/capture/area-overlay');
    const onToolbarAction = vi.fn();

    const session = module.startInteractiveOverlay({
      toolbar: {
        kind: 'all-in-one',
        recordingEnabled: true,
        ocrEnabled: true,
        activeMode: 'screenshot',
      },
      callbacks: { onToolbarAction },
    });
    await settle();

    fire('area-overlay:toolbar', windowFor(0).webContents.id, {
      action: 'screenshot',
    });
    expect(onToolbarAction).toHaveBeenCalledWith({ action: 'screenshot' });

    fire('area-overlay:toolbar', windowFor(0).webContents.id, {
      action: 'update-size',
      width: 100,
      height: '50',
    });
    fire('area-overlay:toolbar', windowFor(0).webContents.id, {
      action: 'unknown',
    });
    fire('area-overlay:toolbar', 999, { action: 'close' });
    expect(onToolbarAction).toHaveBeenCalledTimes(1);

    fire('area-overlay:toolbar', windowFor(1).webContents.id, {
      action: 'select-aspect-ratio',
      name: '16:9',
      width: 16,
      height: 9,
    });

    fire('area-overlay:toolbar', windowFor(1).webContents.id, {
      action: 'ocr',
    });
    expect(onToolbarAction).toHaveBeenCalledWith({ action: 'ocr' });

    fire('area-overlay:toolbar', windowFor(1).webContents.id, {
      action: 'copy-color',
      color: '#12abef',
    });
    expect(onToolbarAction).toHaveBeenCalledWith({
      action: 'copy-color',
      color: '#12abef',
    });

    fire('area-overlay:toolbar', windowFor(1).webContents.id, {
      action: 'select-capture-mode',
      mode: 'record',
    });
    expect(onToolbarAction).toHaveBeenCalledWith({
      action: 'select-capture-mode',
      mode: 'record',
    });

    fire('area-overlay:toolbar', windowFor(1).webContents.id, {
      action: 'select-capture-target',
      target: 'window',
    });
    expect(onToolbarAction).toHaveBeenCalledWith({
      action: 'select-capture-target',
      target: 'window',
    });

    fire('area-overlay:toolbar', windowFor(1).webContents.id, {
      action: 'copy-color',
      color: 'not-a-color',
    });
    fire('area-overlay:toolbar', windowFor(1).webContents.id, {
      action: 'select-capture-mode',
      mode: 'invalid',
    });
    fire('area-overlay:toolbar', windowFor(1).webContents.id, {
      action: 'select-capture-target',
      target: 'invalid',
    });
    expect(onToolbarAction).toHaveBeenCalledTimes(6);
    expect(onToolbarAction).toHaveBeenCalledWith({
      action: 'select-aspect-ratio',
      name: '16:9',
      width: 16,
      height: 9,
    });

    module.setOverlayToolbar(null);
    expect(windowFor(0).webContents.send).toHaveBeenCalledWith(
      'area-overlay:set-toolbar',
      { toolbar: null }
    );

    module.cancelOverlaySelection();
    await session;
  });

  it('reports cancellation once and stays silent when killed', async () => {
    const module = await import('@/main/capture/area-overlay');
    const onCancelled = vi.fn();

    const first = module.startInteractiveOverlay({
      callbacks: { onCancelled },
    });
    await settle();

    fire('area-overlay:cancel', windowFor(0).webContents.id);
    expect(await first).toBeNull();
    expect(onCancelled).toHaveBeenCalledTimes(1);

    const second = module.startInteractiveOverlay({
      callbacks: { onCancelled },
    });
    await settle();

    module.cancelOverlaySelection(true);
    expect(await second).toBeNull();
    expect(onCancelled).toHaveBeenCalledTimes(1);
  });

  it('keeps the confirmed overlay visible until the handoff is concealed', async () => {
    const module = await import('@/main/capture/area-overlay');

    const session = module.startInteractiveOverlay({});
    await settle();

    fire('area-overlay:selected', windowFor(0).webContents.id, {
      displayId: primary.id,
      x: 10,
      y: 20,
      width: 300,
      height: 200,
    });

    vi.clearAllMocks();

    module.confirmOverlaySelection(true);

    expect(await session).toEqual(
      expect.objectContaining({
        rect: { x: 10, y: 20, width: 300, height: 200 },
      })
    );
    expect(module.isOverlayActive()).toBe(false);
    expect(module.hasOverlayHandoff()).toBe(true);
    expect(windowFor(0).hide).not.toHaveBeenCalled();
    expect(windowFor(0).setOpacity).not.toHaveBeenCalledWith(0);
    expect(windowFor(0).setIgnoreMouseEvents).toHaveBeenLastCalledWith(true);
    expect(windowFor(0).webContents.send).toHaveBeenCalledWith(
      'area-overlay:handoff'
    );
    expect(windowFor(0).webContents.send).not.toHaveBeenCalledWith(
      'area-overlay:set-rect',
      expect.anything()
    );

    module.concealOverlayHandoff();

    expect(module.hasOverlayHandoff()).toBe(false);
    expect(windowFor(0).hide).not.toHaveBeenCalled();
    expect(windowFor(0).setOpacity).not.toHaveBeenCalled();
    expect(windowFor(0).webContents.send).toHaveBeenCalledWith(
      'area-overlay:set-rect',
      { rect: null }
    );

    vi.clearAllMocks();
    module.concealOverlayHandoff();
    expect(windowFor(0).hide).not.toHaveBeenCalled();
  });

  it('parks the overlay immediately on a plain confirm', async () => {
    const module = await import('@/main/capture/area-overlay');

    const session = module.startInteractiveOverlay({});
    await settle();

    fire('area-overlay:selected', windowFor(0).webContents.id, {
      displayId: primary.id,
      x: 10,
      y: 20,
      width: 300,
      height: 200,
    });

    vi.clearAllMocks();

    module.confirmOverlaySelection();

    expect(await session).not.toBeNull();
    expect(windowFor(0).hide).not.toHaveBeenCalled();
    expect(windowFor(0).webContents.send).not.toHaveBeenCalledWith(
      'area-overlay:handoff'
    );
  });
});
