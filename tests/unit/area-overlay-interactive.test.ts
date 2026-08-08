import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockGetAllDisplays = vi.fn();
const mockGetCursorScreenPoint = vi.fn();
const mockGetDisplayNearestPoint = vi.fn();
const mockGetDisplayMatching = vi.fn();

const ipcHandlers = new Map<string, (event: unknown, data?: unknown) => void>();
const overlayWindows: MockBrowserWindow[] = [];
const loadHandlers = new Map<number, () => void>();

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
  destroy = vi.fn(() => {
    this.destroyed = true;
  });
  isDestroyed = () => this.destroyed;
  isVisible = () => this.visible;
  setAlwaysOnTop = vi.fn();
  setBounds = vi.fn();
  loadURL = vi.fn();
  loadFile = vi.fn();
  on = vi.fn();

  constructor() {
    overlayWindows.push(this);
  }
}

vi.mock('electron', () => ({
  app: { getPath: () => '/tmp' },
  BrowserWindow: MockBrowserWindow,
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
  releaseRetainedDisplays: vi.fn(),
}));

vi.mock('@/main/capture/freeze-screen/preference', () => ({
  isFreezeScreenEnabled: () => true,
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
    mockGetAllDisplays.mockReturnValue([primary, secondary]);
    mockGetCursorScreenPoint.mockReturnValue({ x: 10, y: 10 });
    mockGetDisplayNearestPoint.mockReturnValue(primary);
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

    expect(windowFor(0).hide).toHaveBeenCalled();
    expect(onSelected).not.toHaveBeenCalled();

    fire('area-overlay:ready', windowFor(1).webContents.id);

    expect(windowFor(1).showInactive).toHaveBeenCalled();
    expect(onSelected).toHaveBeenCalledWith(
      expect.objectContaining({
        rect: { x: 2000, y: 100, width: 400, height: 300 },
      })
    );

    loadHandlers.get(windowFor(1).webContents.id)?.();
    expect(windowFor(1).webContents.send).toHaveBeenCalledWith('load', {
      type: 'area-overlay',
      params: {
        displayId: secondary.id,
        imageUrl: null,
        interactive: true,
        showPrompt: true,
        aspectRatio: null,
        toolbar: null,
        rect: { displayId: 8, x: 80, y: 100, width: 400, height: 300 },
      },
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

    expect(onSelected).toHaveBeenCalledWith(
      expect.objectContaining({
        rect: { x: 1930, y: 20, width: 300, height: 200 },
      })
    );
    expect(windowFor(0).hide).toHaveBeenCalled();
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
    expect(windowFor(0).destroy).toHaveBeenCalled();
    expect(windowFor(1).destroy).toHaveBeenCalled();
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
    expect(windowFor(0).hide).toHaveBeenCalled();
    expect(windowFor(1).hide).toHaveBeenCalled();

    module.setOverlayVisible(true);
    expect(windowFor(0).showInactive).toHaveBeenCalledTimes(2);

    module.cancelOverlaySelection();
    await session;
  });

  it('forwards valid toolbar actions and broadcasts toolbar changes', async () => {
    const module = await import('@/main/capture/area-overlay');
    const onToolbarAction = vi.fn();

    const session = module.startInteractiveOverlay({
      toolbar: { kind: 'all-in-one', recordingEnabled: true },
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
});
