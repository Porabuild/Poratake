import { describe, it, expect, vi, beforeEach } from 'vitest';

const browserWindows: MockBrowserWindow[] = [];
const ipcOn: Record<string, (...a: unknown[]) => unknown> = {};
const ipcHandle: Record<string, (...a: unknown[]) => unknown> = {};

const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockGetThumbnail = vi.fn();
const mockDeleteHistoryItem = vi.fn();
const mockGetHistoryItemByPath = vi.fn();
const mockOpenScreenshotEditor = vi.fn();
const mockCreateVideoEditorWindow = vi.fn();
const mockDeleteVideo = vi.fn();
const mockClipboardWriteImage = vi.fn();
const mockReadFileSync = vi.fn(() => Buffer.from('image'));
const mockNativeImageCreateFromBuffer = vi.fn(() => ({ image: true }));
const mockNativeImageCreateFromPath = vi.fn(() => ({
  resize: () => ({ image: true }),
}));

class MockBrowserWindow {
  static webContentsCounter = 0;

  windowHandlers: Record<string, ((...a: unknown[]) => unknown)[]> = {};
  webContents = {
    id: ++MockBrowserWindow.webContentsCounter,
    on: vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
      this.windowHandlers[`wc:${event}`] ??= [];
      this.windowHandlers[`wc:${event}`].push(cb);
    }),
    send: vi.fn(),
    startDrag: vi.fn(),
  };

  destroyedFlag = false;
  loadURL = vi.fn();
  loadFile = vi.fn();
  show = vi.fn();
  showInactive = vi.fn();
  focus = vi.fn();
  close = vi.fn(() => {
    this.destroyedFlag = true;
    (this.windowHandlers['closed'] || []).forEach(cb => cb());
  });
  setVisibleOnAllWorkspaces = vi.fn();
  setAlwaysOnTop = vi.fn();
  setBounds = vi.fn();
  setPosition = vi.fn();
  getBounds = vi.fn(() => ({ x: 0, y: 0, width: 200, height: 140 }));
  isDestroyed = vi.fn(() => this.destroyedFlag);
  on = vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
    this.windowHandlers[event] ??= [];
    this.windowHandlers[event].push(cb);
  });
  once = vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
    this.windowHandlers[event] ??= [];
    this.windowHandlers[event].push(cb);
  });

  constructor(_opts: unknown) {
    void _opts;
    browserWindows.push(this);
  }
}

vi.mock('electron', () => ({
  BrowserWindow: MockBrowserWindow,
  ipcMain: {
    on: (e: string, h: (...a: unknown[]) => unknown) => {
      ipcOn[e] = h;
    },
    handle: (e: string, h: (...a: unknown[]) => unknown) => {
      ipcHandle[e] = h;
    },
  },
  app: {
    whenReady: () => Promise.resolve(),
    getPath: () => '/tmp',
  },
  screen: {
    getPrimaryDisplay: () => ({
      id: 1,
      workArea: { x: 0, y: 0, width: 1920, height: 1080 },
    }),
    getAllDisplays: () => [
      { id: 1, workArea: { x: 0, y: 0, width: 1920, height: 1080 } },
    ],
    getDisplayMatching: vi.fn(() => ({ id: 1 })),
    on: vi.fn(),
  },
  clipboard: {
    writeImage: (...a: unknown[]) => mockClipboardWriteImage(...a),
  },
  nativeImage: {
    createFromBuffer: (...a: unknown[]) =>
      mockNativeImageCreateFromBuffer(...a),
    createFromPath: (...a: unknown[]) => mockNativeImageCreateFromPath(...a),
  },
}));

vi.mock('fs', () => ({
  default: { readFileSync: (...a: unknown[]) => mockReadFileSync(...a) },
  readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
}));

vi.mock('@/main/utils/env', () => ({
  isDev: true,
  devServerUrl: 'http://localhost:5173',
}));

vi.mock('@/main/utils/thumbnails', () => ({
  getThumbnail: (...a: unknown[]) => mockGetThumbnail(...a),
}));

vi.mock('@/main/history', () => ({
  deleteHistoryItem: (...a: unknown[]) => mockDeleteHistoryItem(...a),
  getHistoryItemByPath: (...a: unknown[]) => mockGetHistoryItemByPath(...a),
}));

vi.mock('@/main/capture/screenshot/open-editor', () => ({
  openScreenshotEditor: (...a: unknown[]) => mockOpenScreenshotEditor(...a),
}));

vi.mock('@/main/capture/video/video-editor', () => ({
  createVideoEditorWindow: (...a: unknown[]) =>
    mockCreateVideoEditorWindow(...a),
}));

vi.mock('@/main/capture/video/delete-video', () => ({
  deleteVideo: (...a: unknown[]) => mockDeleteVideo(...a),
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
  updateConfig: (...a: unknown[]) => mockUpdateConfig(...a),
}));

vi.mock('@/main/capture/capture-preview/video-export', () => ({
  registerPreviewExportIpc: vi.fn(),
}));

vi.mock('@/main/utils/window-animation', () => ({
  animateWindowIn: vi.fn(),
  animateWindowMove: vi.fn(),
  getInitialBounds: () => ({ x: 0, y: 0, width: 200, height: 140 }),
}));

describe('capture-preview index', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    browserWindows.splice(0);
    MockBrowserWindow.webContentsCounter = 0;
    Object.keys(ipcOn).forEach(k => delete ipcOn[k]);
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
    mockGetConfig.mockReturnValue({ preview: { displayId: 1 } });
    mockGetThumbnail.mockResolvedValue({ base64: 'abc', cached: false });
  });

  it('showCapturePreview creates a preview window', async () => {
    const { showCapturePreview } =
      await import('@/main/capture/capture-preview');
    await showCapturePreview('/p/img.png', 'screenshot');
    expect(browserWindows.length).toBe(1);
  });

  it('closeAllPreviewWindows closes all windows', async () => {
    const m = await import('@/main/capture/capture-preview');
    await m.showCapturePreview('/p/img.png', 'screenshot');
    await m.showCapturePreview('/p/img2.png', 'video');
    m.closeAllPreviewWindows();
    expect(browserWindows[0].close).toHaveBeenCalled();
  });

  describe('IPC handlers', () => {
    beforeEach(async () => {
      const { registerCapturePreviewIpc } =
        await import('@/main/capture/capture-preview');
      registerCapturePreviewIpc();
    });

    it('close closes the matching preview window', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/img.png', 'screenshot');
      const id = browserWindows[0].webContents.id;
      ipcOn['capture-preview:close']({ sender: { id } });
      expect(browserWindows[0].close).toHaveBeenCalled();
    });

    it('copy writes image to clipboard for screenshots', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/img.png', 'screenshot');
      const id = browserWindows[0].webContents.id;
      ipcOn['capture-preview:copy']({ sender: { id } });
      expect(mockClipboardWriteImage).toHaveBeenCalled();
    });

    it('copy ignores video content type', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/v.mov', 'video');
      const id = browserWindows[0].webContents.id;
      ipcOn['capture-preview:copy']({ sender: { id } });
      expect(mockClipboardWriteImage).not.toHaveBeenCalled();
    });

    it('open-editor opens screenshot editor for screenshots', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/img.png', 'screenshot', 'h1');
      const id = browserWindows[0].webContents.id;
      ipcOn['capture-preview:open-editor']({ sender: { id } });
      expect(mockOpenScreenshotEditor).toHaveBeenCalledWith('/p/img.png', 'h1');
    });

    it('open-editor opens video editor for videos', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/v.mov', 'video');
      const id = browserWindows[0].webContents.id;
      ipcOn['capture-preview:open-editor']({ sender: { id } });
      expect(mockCreateVideoEditorWindow).toHaveBeenCalledWith('/p/v.mov');
    });

    it('delete deletes video without notification', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/v.mov', 'video');
      const id = browserWindows[0].webContents.id;
      await ipcOn['capture-preview:delete']({ sender: { id } });
      expect(mockDeleteVideo).toHaveBeenCalledWith('/p/v.mov', {
        showNotification: false,
      });
    });

    it('delete deletes screenshot history item', async () => {
      mockGetHistoryItemByPath.mockReturnValue({ id: 'h1' });
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/img.png', 'screenshot');
      const id = browserWindows[0].webContents.id;
      await ipcOn['capture-preview:delete']({ sender: { id } });
      expect(mockDeleteHistoryItem).toHaveBeenCalledWith('h1');
    });

    it('start-drag invokes startDrag on web contents', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/img.png', 'screenshot');
      const sender = browserWindows[0].webContents;
      ipcOn['capture-preview:start-drag'](
        { sender: { ...sender, id: sender.id } },
        '/p/img.png'
      );
    });

    it('get-displays returns display info', async () => {
      const result = await ipcHandle['capture-preview:get-displays']();
      expect(result).toBeInstanceOf(Array);
    });

    it('move-to-display updates config and reposition', async () => {
      const result = await ipcHandle['capture-preview:move-to-display']({}, 1);
      expect(mockUpdateConfig).toHaveBeenCalledWith({
        preview: { displayId: 1 },
      });
      expect(Array.isArray(result)).toBe(true);
    });
  });
});
