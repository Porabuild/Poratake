import { describe, it, expect, vi, beforeEach } from 'vitest';

const browserWindows: MockBrowserWindow[] = [];
const ipcOn: Record<string, (...a: unknown[]) => unknown> = {};
const ipcHandle: Record<string, (...a: unknown[]) => unknown> = {};

const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockGetThumbnail = vi.fn();
const mockDeleteHistoryItem = vi.fn();
const mockGetHistoryItemByPath = vi.fn();
const mockSetHistoryFileReleaseHandler = vi.fn();
const mockOpenScreenshotEditor = vi.fn();
const mockCreateVideoEditorWindow = vi.fn();
const mockDeleteVideo = vi.fn();
const mockClipboardWriteImage = vi.fn();
const mockShellShowItemInFolder = vi.fn();
const mockReadFileSync = vi.fn(() => Buffer.from('image'));
const mockNativeImageCreateFromBuffer = vi.fn(() => ({ image: true }));
const mockNativeImageCreateFromPath = vi.fn(() => ({
  resize: () => ({ image: true }),
}));
const mockNativeImageCreateFromDataURL = vi.fn(() => ({
  isEmpty: () => false,
}));

class MockBrowserWindow {
  static webContentsCounter = 0;

  windowHandlers: Record<string, ((...a: unknown[]) => unknown)[]> = {};
  webContents = {
    id: ++MockBrowserWindow.webContentsCounter,
    ipc: {
      once: vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
        this.windowHandlers[`ipc:${event}`] ??= [];
        this.windowHandlers[`ipc:${event}`].push(cb);
      }),
    },
    on: vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
      this.windowHandlers[`wc:${event}`] ??= [];
      this.windowHandlers[`wc:${event}`].push(cb);
    }),
    once: vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
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

  options: Electron.BrowserWindowConstructorOptions;

  constructor(opts: Electron.BrowserWindowConstructorOptions) {
    this.options = opts;
    browserWindows.push(this);
  }
}

function preparePreviewRenderer(window: MockBrowserWindow): void {
  window.windowHandlers['ipc:capture-preview:renderer-mounted'][0]();
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
  shell: {
    showItemInFolder: (...a: unknown[]) => mockShellShowItemInFolder(...a),
  },
  nativeImage: {
    createFromBuffer: (...a: unknown[]) =>
      mockNativeImageCreateFromBuffer(...a),
    createFromPath: (...a: unknown[]) => mockNativeImageCreateFromPath(...a),
    createFromDataURL: (...a: unknown[]) =>
      mockNativeImageCreateFromDataURL(...a),
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
  setHistoryFileReleaseHandler: (...a: unknown[]) =>
    mockSetHistoryFileReleaseHandler(...a),
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
  animateWindowMove: vi.fn(),
}));

describe('capture-preview index', () => {
  beforeEach(async () => {
    await new Promise(resolve => setImmediate(resolve));
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
    expect(browserWindows[0].options).toMatchObject({
      webPreferences: {
        backgroundThrottling: false,
        webSecurity: false,
      },
    });
  });

  describe('preview window focusable flag', () => {
    const originalPlatform = process.platform;

    afterEach(() => {
      Object.defineProperty(process, 'platform', {
        value: originalPlatform,
      });
      vi.resetModules();
    });

    it('keeps the preview non-focusable on macOS', async () => {
      Object.defineProperty(process, 'platform', { value: 'darwin' });
      vi.resetModules();
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/img.png', 'screenshot');
      expect(browserWindows[0].options).toMatchObject({ focusable: false });
    });

    it('makes the preview focusable on Windows so mouse presses are delivered', async () => {
      Object.defineProperty(process, 'platform', { value: 'win32' });
      vi.resetModules();
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/img.png', 'screenshot');
      expect(browserWindows[0].options).toMatchObject({ focusable: true });
    });
  });

  it('prepares the preview renderer before a capture claims the window', async () => {
    const { prepareCapturePreview } =
      await import('@/main/capture/capture-preview');

    prepareCapturePreview();
    preparePreviewRenderer(browserWindows[0]);

    expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
      'capture-preview:prepare-renderer'
    );
  });

  it('reveals a screenshot when its first image is ready', async () => {
    let finishThumbnail: (value: {
      base64: string;
      cached: boolean;
    }) => void = () => {};
    mockGetThumbnail.mockReturnValueOnce(
      new Promise(resolve => {
        finishThumbnail = resolve;
      })
    );
    const { showCapturePreview } =
      await import('@/main/capture/capture-preview');
    showCapturePreview('/p/img.png', 'screenshot');

    preparePreviewRenderer(browserWindows[0]);
    await vi.waitFor(() => {
      expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
        'load',
        expect.objectContaining({
          params: expect.objectContaining({
            imageUrl: expect.stringMatching(/^file:/),
          }),
        })
      );
    });
    expect(browserWindows[0].showInactive).not.toHaveBeenCalled();
    expect(mockGetThumbnail).toHaveBeenCalledWith('/p/img.png', 'screenshot');

    browserWindows[0].windowHandlers['ipc:capture-preview:content-ready'][0]();

    expect(browserWindows[0].showInactive).toHaveBeenCalledTimes(1);
    finishThumbnail({ base64: 'abc', cached: false });
    await vi.waitFor(() => {
      expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
        'load',
        expect.objectContaining({
          params: expect.objectContaining({
            thumbnailUrl: 'data:image/jpeg;base64,abc',
          }),
        })
      );
    });
  });

  it('sends preview data as soon as the renderer shell is mounted', async () => {
    const { showCapturePreview } =
      await import('@/main/capture/capture-preview');
    showCapturePreview('/p/img.png', 'screenshot');

    browserWindows[0].windowHandlers[
      'ipc:capture-preview:renderer-mounted'
    ][0]();

    await vi.waitFor(() => {
      expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
        'load',
        expect.objectContaining({
          params: expect.objectContaining({
            imageUrl: expect.stringMatching(/^file:/),
          }),
        })
      );
    });
  });

  it('sends a direct video URL before its thumbnail is generated', async () => {
    let finishThumbnail: (value: {
      base64: string;
      cached: boolean;
    }) => void = () => {};
    mockGetThumbnail.mockReturnValueOnce(
      new Promise(resolve => {
        finishThumbnail = resolve;
      })
    );
    const { showCapturePreview } =
      await import('@/main/capture/capture-preview');
    showCapturePreview('/p/video.mov', 'video');

    preparePreviewRenderer(browserWindows[0]);
    await vi.waitFor(() => {
      expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
        'load',
        expect.objectContaining({
          params: expect.objectContaining({
            imageUrl: expect.stringMatching(/^file:/),
          }),
        })
      );
    });
    expect(browserWindows[0].showInactive).not.toHaveBeenCalled();
    expect(mockGetThumbnail).toHaveBeenCalledWith('/p/video.mov', 'video');
    browserWindows[0].windowHandlers['ipc:capture-preview:content-ready'][0]();
    expect(browserWindows[0].showInactive).toHaveBeenCalledTimes(1);
    finishThumbnail({ base64: 'abc', cached: false });
    await vi.waitFor(() => {
      expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
        'load',
        expect.objectContaining({
          params: expect.objectContaining({
            thumbnailUrl: 'data:image/jpeg;base64,abc',
          }),
        })
      );
    });
  });

  it('waits for the renderer before sending a failed video thumbnail', async () => {
    mockGetThumbnail.mockRejectedValueOnce(new Error('thumbnail failed'));
    const { showCapturePreview } =
      await import('@/main/capture/capture-preview');
    showCapturePreview('/p/video.mov', 'video');

    await Promise.resolve();
    await Promise.resolve();
    expect(browserWindows[0].webContents.send).not.toHaveBeenCalled();

    preparePreviewRenderer(browserWindows[0]);
    await vi.waitFor(() => {
      expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
        'load',
        expect.objectContaining({
          params: expect.objectContaining({
            imageUrl: expect.stringMatching(/^file:/),
          }),
        })
      );
    });
  });

  it('settles the preview when renderer preparation fails', async () => {
    const { showCapturePreview } =
      await import('@/main/capture/capture-preview');
    const preview = showCapturePreview('/p/img.png', 'screenshot');

    browserWindows[0].windowHandlers[
      'ipc:capture-preview:renderer-mounted'
    ][0]();
    browserWindows[0].windowHandlers[
      'ipc:capture-preview:renderer-failed'
    ][0]();
    await preview.revealed;

    expect(browserWindows[0].close).toHaveBeenCalledTimes(1);
    expect(browserWindows[0].showInactive).not.toHaveBeenCalled();
    expect(mockGetThumbnail).not.toHaveBeenCalled();
  });

  it('settles the preview when its page fails to load', async () => {
    const { showCapturePreview } =
      await import('@/main/capture/capture-preview');
    const preview = showCapturePreview('/p/img.png', 'screenshot');

    browserWindows[0].windowHandlers['wc:did-fail-load'][0](
      {},
      -105,
      'Name not resolved',
      'http://localhost:5173',
      true
    );
    await preview.revealed;

    expect(browserWindows[0].close).toHaveBeenCalledTimes(1);
    expect(browserWindows[0].showInactive).not.toHaveBeenCalled();
    expect(mockGetThumbnail).not.toHaveBeenCalled();
  });

  it('settles the preview when its render process exits', async () => {
    const { showCapturePreview } =
      await import('@/main/capture/capture-preview');
    const preview = showCapturePreview('/p/img.png', 'screenshot');

    browserWindows[0].windowHandlers['wc:render-process-gone'][0]();
    await preview.revealed;

    expect(browserWindows[0].close).toHaveBeenCalledTimes(1);
    expect(browserWindows[0].showInactive).not.toHaveBeenCalled();
    expect(mockGetThumbnail).not.toHaveBeenCalled();
  });

  it('reuses a preview window prepared before capture finishes', async () => {
    const { prepareCapturePreview, showCapturePreview } =
      await import('@/main/capture/capture-preview');

    const preparation = prepareCapturePreview();

    expect(browserWindows).toHaveLength(1);
    expect(browserWindows[0].loadURL).toHaveBeenCalledWith(
      'http://localhost:5173/?window=capture-preview'
    );

    showCapturePreview('/p/img.png', 'screenshot', undefined, preparation);

    expect(browserWindows).toHaveLength(1);
    expect(preparation.claimed).toBe(true);
    expect(browserWindows[0].setPosition).toHaveBeenCalled();

    preparation.dispose();
    expect(browserWindows[0].close).not.toHaveBeenCalled();
  });

  it('does not retain another hidden window after reveal', async () => {
    mockGetConfig.mockReturnValue({
      preview: { displayId: 1 },
      screenshot: { showPreview: true, captureToClipboard: false },
    });
    const { prepareCapturePreview, showCapturePreview } =
      await import('@/main/capture/capture-preview');

    const preparation = prepareCapturePreview();
    preparePreviewRenderer(browserWindows[0]);
    const preview = showCapturePreview(
      '/p/img.png',
      'screenshot',
      undefined,
      preparation
    );

    expect(browserWindows).toHaveLength(1);
    await vi.waitFor(() => {
      expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
        'load',
        expect.any(Object)
      );
    });
    browserWindows[0].windowHandlers['ipc:capture-preview:content-ready'][0]();
    await preview.revealed;
    await new Promise(resolve => setImmediate(resolve));

    expect(browserWindows).toHaveLength(1);
  });

  it('reveals a prewarmed screenshot as soon as its content is ready', async () => {
    mockGetConfig.mockReturnValue({
      preview: { displayId: 1 },
      screenshot: { showPreview: true, captureToClipboard: false },
    });
    const { prepareCapturePreview, showCapturePreview } =
      await import('@/main/capture/capture-preview');

    const preparation = prepareCapturePreview();
    expect(browserWindows).toHaveLength(1);

    preparePreviewRenderer(browserWindows[0]);
    await Promise.resolve();
    expect(browserWindows[0].webContents.send).not.toHaveBeenCalledWith(
      'load',
      expect.anything()
    );

    const preview = showCapturePreview(
      '/p/img.png',
      'screenshot',
      undefined,
      preparation
    );

    expect(browserWindows[0].showInactive).not.toHaveBeenCalled();
    await vi.waitFor(() => {
      expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
        'load',
        expect.objectContaining({
          params: expect.objectContaining({
            filePath: '/p/img.png',
            imageUrl: expect.stringMatching(/^file:/),
          }),
        })
      );
    });
    browserWindows[0].windowHandlers['ipc:capture-preview:content-ready'][0]();

    let revealed = false;
    void preview.revealed.then(() => {
      revealed = true;
    });
    await preview.revealed;
    await new Promise(resolve => setImmediate(resolve));
    expect(browserWindows[0].showInactive).toHaveBeenCalledTimes(1);
    expect(revealed).toBe(true);
    expect(mockGetThumbnail).toHaveBeenCalledWith('/p/img.png', 'screenshot');
    expect(browserWindows).toHaveLength(1);
  });

  it('does not start the next preview while current content loads', async () => {
    mockGetConfig.mockReturnValue({
      preview: { displayId: 1 },
      screenshot: { showPreview: true, captureToClipboard: false },
    });
    const { prepareCapturePreview, showCapturePreview } =
      await import('@/main/capture/capture-preview');

    const preparation = prepareCapturePreview();
    preparePreviewRenderer(browserWindows[0]);
    showCapturePreview('/p/img.png', 'screenshot', undefined, preparation);

    expect(browserWindows).toHaveLength(1);
    expect(browserWindows[0].showInactive).not.toHaveBeenCalled();
  });

  it('creates a second preview on demand before the first is ready', async () => {
    mockGetConfig.mockReturnValue({
      preview: { displayId: 1 },
      screenshot: { showPreview: true, captureToClipboard: false },
    });
    const { prepareCapturePreview, showCapturePreview } =
      await import('@/main/capture/capture-preview');

    const firstPreparation = prepareCapturePreview();
    preparePreviewRenderer(browserWindows[0]);
    showCapturePreview(
      '/p/first.png',
      'screenshot',
      undefined,
      firstPreparation
    );

    const secondPreparation = prepareCapturePreview();
    preparePreviewRenderer(browserWindows[1]);
    showCapturePreview(
      '/p/second.png',
      'screenshot',
      undefined,
      secondPreparation
    );

    expect(browserWindows).toHaveLength(2);
    await vi.waitFor(() => {
      expect(browserWindows[1].webContents.send).toHaveBeenCalledWith(
        'load',
        expect.objectContaining({
          params: expect.objectContaining({ filePath: '/p/second.png' }),
        })
      );
    });
    expect(browserWindows[0].showInactive).not.toHaveBeenCalled();
    expect(browserWindows[1].showInactive).not.toHaveBeenCalled();

    browserWindows[1].windowHandlers['ipc:capture-preview:content-ready'][0]();

    expect(browserWindows[1].showInactive).toHaveBeenCalledTimes(1);
  });

  it('disposes an unused prepared preview window', async () => {
    const { prepareCapturePreview } =
      await import('@/main/capture/capture-preview');

    const preparation = prepareCapturePreview();
    preparation.dispose();

    expect(browserWindows[0].close).toHaveBeenCalledTimes(1);
    expect(browserWindows).toHaveLength(1);
  });

  it('keeps a prepared preview active after it is claimed', async () => {
    const { prepareCapturePreview, showCapturePreview } =
      await import('@/main/capture/capture-preview');
    const preparation = prepareCapturePreview();
    showCapturePreview('/p/img.png', 'screenshot', undefined, preparation);

    preparation.dispose();

    preparePreviewRenderer(browserWindows[0]);
    await vi.waitFor(() => {
      expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
        'load',
        expect.any(Object)
      );
    });
    browserWindows[0].windowHandlers['ipc:capture-preview:content-ready'][0]();

    expect(browserWindows[0].showInactive).toHaveBeenCalledTimes(1);
  });

  describe('preview corner', () => {
    const corners = [
      {
        corner: 'bottom-right',
        first: { x: 1696, y: 916 },
        second: { x: 1696, y: 764 },
      },
      {
        corner: 'bottom-left',
        first: { x: 24, y: 916 },
        second: { x: 24, y: 764 },
      },
      {
        corner: 'top-right',
        first: { x: 1696, y: 24 },
        second: { x: 1696, y: 176 },
      },
      {
        corner: 'top-left',
        first: { x: 24, y: 24 },
        second: { x: 24, y: 176 },
      },
    ];

    it.each(corners)(
      'anchors previews to the $corner corner and stacks away from it',
      async ({ corner, first, second }) => {
        mockGetConfig.mockReturnValue({ preview: { displayId: 1, corner } });
        const { showCapturePreview } =
          await import('@/main/capture/capture-preview');

        await showCapturePreview('/p/a.png', 'screenshot');
        await showCapturePreview('/p/b.png', 'screenshot');

        expect(browserWindows[0].options).toMatchObject(first);
        expect(browserWindows[1].options).toMatchObject(second);
      }
    );

    it('falls back to the bottom-right corner for an unknown value', async () => {
      mockGetConfig.mockReturnValue({
        preview: { displayId: 1, corner: 'middle' },
      });
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');

      await showCapturePreview('/p/a.png', 'screenshot');

      expect(browserWindows[0].options).toMatchObject({ x: 1696, y: 916 });
    });
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

    it('reveals the preview as soon as its content is ready', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      showCapturePreview('/p/img.png', 'screenshot');

      expect(browserWindows[0].showInactive).not.toHaveBeenCalled();

      preparePreviewRenderer(browserWindows[0]);

      await vi.waitFor(() => {
        expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
          'load',
          expect.any(Object)
        );
      });

      browserWindows[0].windowHandlers[
        'ipc:capture-preview:content-ready'
      ][0]();

      expect(browserWindows[0].showInactive).toHaveBeenCalledTimes(1);
      expect(browserWindows[0].setBounds).not.toHaveBeenCalled();
    });

    it('releases a matching preview before history deletes its file', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      showCapturePreview('/p/video.mov', 'video');
      const releaseFile = mockSetHistoryFileReleaseHandler.mock.calls.at(
        -1
      )?.[0] as (filePath: string) => Promise<void>;

      await releaseFile('/p/video.mov');

      expect(browserWindows[0].close).toHaveBeenCalledOnce();
    });

    it('reveals the preview only once when its data updates', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      showCapturePreview('/p/video.mov', 'video');

      preparePreviewRenderer(browserWindows[0]);

      await vi.waitFor(() => {
        expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
          'load',
          expect.any(Object)
        );
      });

      await vi.waitFor(() => {
        expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
          'load',
          expect.objectContaining({
            params: expect.objectContaining({
              thumbnailUrl: 'data:image/jpeg;base64,abc',
            }),
          })
        );
      });

      browserWindows[0].windowHandlers[
        'ipc:capture-preview:content-ready'
      ][0]();

      expect(browserWindows[0].showInactive).toHaveBeenCalledTimes(1);
    });

    it('close closes the matching preview window', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/img.png', 'screenshot');
      const id = browserWindows[0].webContents.id;
      ipcOn['capture-preview:close']({ sender: { id } });
      expect(browserWindows[0].close).toHaveBeenCalled();
    });

    it('show-in-folder reveals the recording file', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/Rec.poratake/recording.mov', 'video');
      const id = browserWindows[0].webContents.id;
      ipcOn['capture-preview:show-in-folder']({ sender: { id } });
      expect(mockShellShowItemInFolder).toHaveBeenCalledWith(
        '/p/Rec.poratake/recording.mov'
      );
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

    it('get-source-image returns a data URL for screenshots', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/img.jpg', 'screenshot');
      const id = browserWindows[0].webContents.id;

      const result = ipcHandle['capture-preview:get-source-image']({
        sender: { id },
      });

      expect(result).toBe(
        `data:image/jpeg;base64,${Buffer.from('image').toString('base64')}`
      );
    });

    it('get-source-image returns null for videos', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/v.mov', 'video');
      const id = browserWindows[0].webContents.id;

      expect(
        ipcHandle['capture-preview:get-source-image']({ sender: { id } })
      ).toBeNull();
    });

    it('copy-composited writes the rendered image and closes the preview', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/img.png', 'screenshot');
      const id = browserWindows[0].webContents.id;

      const result = ipcHandle['capture-preview:copy-composited'](
        { sender: { id } },
        'data:image/png;base64,abc'
      );

      expect(result).toBe(true);
      expect(mockNativeImageCreateFromDataURL).toHaveBeenCalledWith(
        'data:image/png;base64,abc'
      );
      expect(mockClipboardWriteImage).toHaveBeenCalled();
      expect(browserWindows[0].close).toHaveBeenCalled();
    });

    it('copy-composited ignores an empty image', async () => {
      mockNativeImageCreateFromDataURL.mockReturnValueOnce({
        isEmpty: () => true,
      });
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/img.png', 'screenshot');
      const id = browserWindows[0].webContents.id;

      const result = ipcHandle['capture-preview:copy-composited'](
        { sender: { id } },
        'data:image/png;base64,abc'
      );

      expect(result).toBe(false);
      expect(mockClipboardWriteImage).not.toHaveBeenCalled();
      expect(browserWindows[0].close).not.toHaveBeenCalled();
    });

    it('open-editor opens screenshot editor for screenshots', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/img.png', 'screenshot', 'h1');
      const id = browserWindows[0].webContents.id;
      ipcOn['capture-preview:open-editor']({ sender: { id } });
      expect(mockOpenScreenshotEditor).toHaveBeenCalledWith('/p/img.png', 'h1');
    });

    it('waits for screenshot history before opening the editor', async () => {
      let resolveHistoryId: (id: string) => void = () => {};
      const historyIdPromise = new Promise<string>(resolve => {
        resolveHistoryId = resolve;
      });
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      showCapturePreview(
        '/p/img.png',
        'screenshot',
        undefined,
        undefined,
        historyIdPromise
      );
      const id = browserWindows[0].webContents.id;

      const opening = ipcOn['capture-preview:open-editor']({ sender: { id } });
      expect(mockOpenScreenshotEditor).not.toHaveBeenCalled();

      resolveHistoryId('h1');
      await opening;

      expect(mockOpenScreenshotEditor).toHaveBeenCalledWith('/p/img.png', 'h1');
    });

    it('open-editor opens video editor for videos', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/v.mov', 'video');
      const id = browserWindows[0].webContents.id;
      await ipcOn['capture-preview:open-editor']({ sender: { id } });
      expect(mockCreateVideoEditorWindow).toHaveBeenCalledWith('/p/v.mov');
    });

    it('waits for video preparation before opening the editor', async () => {
      let finishPreparation: () => void = () => {};
      const actionReadyPromise = new Promise<void>(resolve => {
        finishPreparation = resolve;
      });
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      showCapturePreview(
        '/p/v.mov',
        'video',
        undefined,
        undefined,
        undefined,
        actionReadyPromise
      );
      const id = browserWindows[0].webContents.id;

      const opening = ipcOn['capture-preview:open-editor']({ sender: { id } });
      expect(mockCreateVideoEditorWindow).not.toHaveBeenCalled();

      finishPreparation();
      await opening;

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

    it('waits for the video preview window to close before deleting', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      showCapturePreview('/p/v.mov', 'video');
      const previewWindow = browserWindows[0];
      const id = previewWindow.webContents.id;
      previewWindow.close.mockImplementationOnce(() => {});

      const deleting = ipcOn['capture-preview:delete']({ sender: { id } });
      await Promise.resolve();
      expect(mockDeleteVideo).not.toHaveBeenCalled();

      previewWindow.destroyedFlag = true;
      previewWindow.windowHandlers['closed'].forEach(handler => handler());
      await deleting;

      expect(mockDeleteVideo).toHaveBeenCalledWith('/p/v.mov', {
        showNotification: false,
      });
    });

    it('waits for video thumbnail generation before deleting', async () => {
      let finishThumbnail: (result: { base64: string }) => void = () => {};
      mockGetThumbnail.mockReturnValueOnce(
        new Promise(resolve => {
          finishThumbnail = resolve;
        })
      );
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      showCapturePreview('/p/v.mov', 'video');
      const previewWindow = browserWindows[0];
      preparePreviewRenderer(previewWindow);
      await vi.waitFor(() => {
        expect(mockGetThumbnail).toHaveBeenCalledWith('/p/v.mov', 'video');
      });

      const deleting = ipcOn['capture-preview:delete']({
        sender: { id: previewWindow.webContents.id },
      });
      expect(mockDeleteVideo).not.toHaveBeenCalled();

      finishThumbnail({ base64: 'abc' });
      await deleting;

      expect(mockDeleteVideo).toHaveBeenCalledWith('/p/v.mov', {
        showNotification: false,
      });
    });

    it('ignores repeated delete requests for one preview', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      showCapturePreview('/p/v.mov', 'video');
      const previewWindow = browserWindows[0];
      const id = previewWindow.webContents.id;
      previewWindow.close.mockImplementation(() => {});

      const firstDelete = ipcOn['capture-preview:delete']({ sender: { id } });
      const secondDelete = ipcOn['capture-preview:delete']({ sender: { id } });
      previewWindow.windowHandlers['closed'].forEach(handler => handler());
      await Promise.all([firstDelete, secondDelete]);

      expect(mockDeleteVideo).toHaveBeenCalledTimes(1);
    });

    it('waits for video history persistence before deleting', async () => {
      let resolveHistoryId: (id: string) => void = () => {};
      const historyIdPromise = new Promise<string>(resolve => {
        resolveHistoryId = resolve;
      });
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      showCapturePreview(
        '/p/v.mov',
        'video',
        undefined,
        undefined,
        historyIdPromise
      );
      const id = browserWindows[0].webContents.id;

      const deleting = ipcOn['capture-preview:delete']({ sender: { id } });
      expect(mockDeleteVideo).not.toHaveBeenCalled();

      resolveHistoryId('h1');
      await deleting;

      expect(mockDeleteVideo).toHaveBeenCalledWith('/p/v.mov', {
        showNotification: false,
      });
    });

    it('delete deletes screenshot history item', async () => {
      let resolveHistoryId: (id: string) => void = () => {};
      const historyIdPromise = new Promise<string>(resolve => {
        resolveHistoryId = resolve;
      });
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      showCapturePreview(
        '/p/img.png',
        'screenshot',
        undefined,
        undefined,
        historyIdPromise
      );
      const id = browserWindows[0].webContents.id;
      const deleting = ipcOn['capture-preview:delete']({ sender: { id } });
      expect(mockDeleteHistoryItem).not.toHaveBeenCalled();

      resolveHistoryId('h1');
      await deleting;

      expect(mockDeleteHistoryItem).toHaveBeenCalledWith('h1');
    });

    it('start-drag invokes startDrag on web contents', async () => {
      const { showCapturePreview } =
        await import('@/main/capture/capture-preview');
      await showCapturePreview('/p/img.png', 'screenshot');
      const sender = browserWindows[0].webContents;
      ipcOn['capture-preview:start-drag'](
        { sender: { ...sender, id: sender.id } },
        '/p/unrelated.png'
      );
      expect(sender.startDrag).toHaveBeenCalledWith(
        expect.objectContaining({ file: '/p/img.png' })
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
