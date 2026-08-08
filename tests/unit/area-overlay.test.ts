import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockCaptureRegionToFile = vi.fn();
const mockReleaseRetainedDisplays = vi.fn();
const mockIsFreezeScreenEnabled = vi.fn();
const mockRmSync = vi.fn();
const mockReaddirSync = vi.fn();
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

  constructor(options: Record<string, unknown>) {
    this.options = options;
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
  },
}));

vi.mock('fs', () => ({
  default: {
    readdirSync: (...a: unknown[]) => mockReaddirSync(...a),
    rmSync: (...a: unknown[]) => mockRmSync(...a),
  },
  readdirSync: (...a: unknown[]) => mockReaddirSync(...a),
  rmSync: (...a: unknown[]) => mockRmSync(...a),
}));

vi.mock('@/main/utils/env', () => ({
  isDev: false,
  devServerUrl: null,
}));

vi.mock('@/main/capture/screenshot/native-capture', () => ({
  captureRegionToFile: (...a: unknown[]) => mockCaptureRegionToFile(...a),
  releaseRetainedDisplays: (...a: unknown[]) =>
    mockReleaseRetainedDisplays(...a),
}));

vi.mock('@/main/capture/freeze-screen/preference', () => ({
  isFreezeScreenEnabled: () => mockIsFreezeScreenEnabled(),
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
    mockReleaseRetainedDisplays.mockResolvedValue(undefined);
    mockIsFreezeScreenEnabled.mockReturnValue(true);
    mockReaddirSync.mockReturnValue([]);
  });

  it('retains the frozen display and keeps the overlay hidden until it paints', async () => {
    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay();
    await settle();

    expect(mockCaptureRegionToFile).toHaveBeenCalledWith(
      display.bounds,
      expect.stringContaining('.bmp'),
      { retain: true }
    );
    expect(overlayWindows).toHaveLength(1);
    expect(overlayWindows[0].showInactive).not.toHaveBeenCalled();

    fire('area-overlay:ready', overlayWindows[0].webContents.id);
    expect(overlayWindows[0].showInactive).toHaveBeenCalled();
    expect(overlayWindows[0].focus).toHaveBeenCalled();

    fire('area-overlay:cancel', overlayWindows[0].webContents.id);
    expect(await selection).toBeNull();
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
    expect(mockReleaseRetainedDisplays).not.toHaveBeenCalled();
  });

  it('deletes the frozen preview and releases the daemon on cancellation', async () => {
    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay();
    await settle();

    fire('area-overlay:cancel', overlayWindows[0].webContents.id);
    await selection;

    expect(mockRmSync).toHaveBeenCalledWith(
      expect.stringContaining('.bmp'),
      expect.objectContaining({ force: true })
    );
    expect(mockReleaseRetainedDisplays).toHaveBeenCalled();
    expect(overlayWindows[0].destroy).toHaveBeenCalled();
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

  it('tears the overlay down when no display could be frozen', async () => {
    mockCaptureRegionToFile.mockResolvedValue(false);
    const module = await import('@/main/capture/area-overlay');

    expect(await module.selectAreaWithOverlay()).toBeNull();
    expect(overlayWindows[0].destroy).toHaveBeenCalled();
    expect(mockReleaseRetainedDisplays).toHaveBeenCalled();
  });

  it('boots the overlay window before the capture finishes', async () => {
    let finishCapture!: (captured: boolean) => void;
    mockCaptureRegionToFile.mockImplementation(
      () => new Promise<boolean>(resolve => (finishCapture = resolve))
    );

    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay();
    await settle();

    expect(overlayWindows).toHaveLength(1);
    expect(overlayWindows[0].loadFile).toHaveBeenCalled();
    expect(overlayWindows[0].webContents.send).not.toHaveBeenCalled();

    loadHandlers.get(overlayWindows[0].webContents.id)?.();
    expect(overlayWindows[0].webContents.send).not.toHaveBeenCalled();

    finishCapture(true);
    await settle();

    expect(overlayWindows[0].webContents.send).toHaveBeenCalledWith(
      'load',
      expect.objectContaining({ type: 'area-overlay' })
    );

    fire('area-overlay:cancel', overlayWindows[0].webContents.id);
    await selection;
  });

  it('waits for a pending frozen capture before releasing it', async () => {
    let finishCapture!: (captured: boolean) => void;
    mockCaptureRegionToFile.mockImplementationOnce(
      () => new Promise<boolean>(resolve => (finishCapture = resolve))
    );

    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay();
    await settle();

    fire('area-overlay:cancel', overlayWindows[0].webContents.id);
    const nextSelection = module.selectAreaWithOverlay();

    await settle();
    expect(mockReleaseRetainedDisplays).not.toHaveBeenCalled();
    expect(overlayWindows).toHaveLength(1);

    finishCapture(true);
    expect(await selection).toBeNull();
    await vi.waitFor(() =>
      expect(mockReleaseRetainedDisplays).toHaveBeenCalledTimes(1)
    );
    expect(mockRmSync).toHaveBeenCalledWith(
      expect.stringContaining('.bmp'),
      expect.objectContaining({ force: true })
    );

    await settle();
    expect(overlayWindows).toHaveLength(2);
    fire('area-overlay:cancel', overlayWindows[1].webContents.id);
    expect(await nextSelection).toBeNull();
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

    loadHandlers.get(overlayWindows[0].webContents.id)?.();
    expect(overlayWindows[0].webContents.send).toHaveBeenCalledWith('load', {
      type: 'area-overlay',
      params: {
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
    expect(mockReleaseRetainedDisplays).not.toHaveBeenCalled();
    expect(mockRmSync).not.toHaveBeenCalled();
  });

  it('leaves the daemon untouched when a live overlay is cancelled', async () => {
    const module = await import('@/main/capture/area-overlay');
    const selection = module.selectAreaWithOverlay({ freeze: false });
    await settle();

    fire('area-overlay:cancel', overlayWindows[0].webContents.id);

    expect(await selection).toBeNull();
    expect(mockReleaseRetainedDisplays).not.toHaveBeenCalled();
    expect(overlayWindows[0].destroy).toHaveBeenCalled();
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
    expect(mockReleaseRetainedDisplays).toHaveBeenCalled();
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
    expect(overlayWindows[0].destroy).toHaveBeenCalled();
    expect(mockReleaseRetainedDisplays).not.toHaveBeenCalled();
  });
});
