import { describe, it, expect, vi, beforeEach } from 'vitest';

const browserWindows: MockBrowserWindow[] = [];
const ipcOn: Record<string, (...a: unknown[]) => unknown> = {};

const mockShowOpenDialog = vi.fn();
const mockGetWindowData = vi.fn();
const mockGetWindowFromWebContentsId = vi.fn();
const mockOpenScreenshotWindow = vi.fn();
const mockExistsSync = vi.fn();
const mockReadFileSync = vi.fn();
const mockNativeImageCreateFromBuffer = vi.fn();
const mockNativeImageCreateFromPath = vi.fn();

class MockBrowserWindow {
  static webContentsCounter = 0;

  options: Record<string, unknown>;
  windowHandlers: Record<string, ((...a: unknown[]) => unknown)[]> = {};
  webContents = {
    id: ++MockBrowserWindow.webContentsCounter,
    on: vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
      this.windowHandlers[`wc:${event}`] ??= [];
      this.windowHandlers[`wc:${event}`].push(cb);
    }),
    send: vi.fn(),
  };

  destroyedFlag = false;
  loadURL = vi.fn();
  loadFile = vi.fn();
  show = vi.fn();
  focus = vi.fn();
  close = vi.fn(() => {
    (this.windowHandlers['closed'] || []).forEach(cb => cb());
  });
  setAlwaysOnTop = vi.fn();
  isDestroyed = vi.fn(() => this.destroyedFlag);
  on = vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
    this.windowHandlers[event] ??= [];
    this.windowHandlers[event].push(cb);
  });
  once = vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
    this.windowHandlers[event] ??= [];
    this.windowHandlers[event].push(cb);
  });

  constructor(opts: Record<string, unknown>) {
    this.options = opts;
    browserWindows.push(this);
  }
}

vi.mock('electron', () => ({
  BrowserWindow: MockBrowserWindow,
  ipcMain: {
    on: (e: string, h: (...a: unknown[]) => unknown) => {
      ipcOn[e] = h;
    },
  },
  app: { focus: vi.fn() },
  screen: {
    getPrimaryDisplay: () => ({
      scaleFactor: 2,
      workAreaSize: { width: 1920, height: 1080 },
    }),
  },
  dialog: { showOpenDialog: (...a: unknown[]) => mockShowOpenDialog(...a) },
  nativeImage: {
    createFromBuffer: (...a: unknown[]) =>
      mockNativeImageCreateFromBuffer(...a),
    createFromPath: (...a: unknown[]) => mockNativeImageCreateFromPath(...a),
  },
}));

vi.mock('fs', () => ({
  default: {
    existsSync: (...a: unknown[]) => mockExistsSync(...a),
    readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
  },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
  readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
}));

vi.mock('@/main/utils/env.ts', () => ({
  isDev: true,
  devServerUrl: 'http://localhost:5173',
}));

vi.mock('@/main/utils/dock', () => ({
  registerDockWindow: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@/main/capture/screenshot/open-editor.ts', () => ({
  openScreenshotWindow: (...a: unknown[]) => mockOpenScreenshotWindow(...a),
  getWindowData: (...a: unknown[]) => mockGetWindowData(...a),
  getWindowFromWebContentsId: (...a: unknown[]) =>
    mockGetWindowFromWebContentsId(...a),
}));

describe('pin', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    browserWindows.splice(0);
    MockBrowserWindow.webContentsCounter = 0;
    Object.keys(ipcOn).forEach(k => delete ipcOn[k]);
    mockNativeImageCreateFromBuffer.mockReturnValue({
      getSize: () => ({ width: 800, height: 600 }),
      isEmpty: () => false,
    });
    mockNativeImageCreateFromPath.mockReturnValue({
      getSize: () => ({ width: 800, height: 600 }),
      isEmpty: () => false,
    });
  });

  describe('registerIpcHandlers', () => {
    it('registers IPC handlers', async () => {
      const { registerIpcHandlers } =
        await import('@/main/capture/screenshot/pin');
      registerIpcHandlers();
      expect(ipcOn['screenshot:pin']).toBeDefined();
      expect(ipcOn['toggle-pin']).toBeDefined();
    });

    it('screenshot:pin creates a new pin window', async () => {
      mockGetWindowData.mockReturnValue(undefined);
      const { registerIpcHandlers } =
        await import('@/main/capture/screenshot/pin');
      registerIpcHandlers();
      ipcOn['screenshot:pin'](
        { sender: { id: 1 } },
        {
          imageBase64: 'aGVsbG8=',
          editorState: {},
          filePath: '/p/img.png',
          originalWidth: 1600,
          originalHeight: 1200,
        }
      );
      expect(browserWindows.length).toBe(1);
    });

    it('screenshot:pin scales a full-screen image to half the work area', async () => {
      mockGetWindowData.mockReturnValue(undefined);
      mockNativeImageCreateFromBuffer.mockReturnValue({
        getSize: () => ({ width: 3840, height: 2160 }),
        isEmpty: () => false,
      });
      const { registerIpcHandlers } =
        await import('@/main/capture/screenshot/pin');
      registerIpcHandlers();
      ipcOn['screenshot:pin'](
        { sender: { id: 1 } },
        {
          imageBase64: 'aGVsbG8=',
          editorState: {},
          filePath: '/p/img.png',
          originalWidth: 3840,
          originalHeight: 2160,
        }
      );
      expect(browserWindows[0].options).toMatchObject({
        width: 960,
        height: 540,
      });
    });

    it('screenshot:pin closes existing screenshot window', async () => {
      const close = vi.fn();
      mockGetWindowData.mockReturnValue({
        window: { isDestroyed: () => false, close },
        isClosingConfirmed: false,
      });
      const { registerIpcHandlers } =
        await import('@/main/capture/screenshot/pin');
      registerIpcHandlers();
      ipcOn['screenshot:pin'](
        { sender: { id: 1 } },
        {
          imageBase64: 'aGVsbG8=',
          editorState: {},
          filePath: '/p/img.png',
          originalWidth: 1600,
          originalHeight: 1200,
        }
      );
      expect(close).toHaveBeenCalled();
    });

    it('toggle-pin updates alwaysOnTop on existing window', async () => {
      const setAlwaysOnTop = vi.fn();
      mockGetWindowFromWebContentsId.mockReturnValue({
        isDestroyed: () => false,
        setAlwaysOnTop,
      });
      const { registerIpcHandlers } =
        await import('@/main/capture/screenshot/pin');
      registerIpcHandlers();
      ipcOn['toggle-pin']({ sender: { id: 1 } }, false);
      expect(setAlwaysOnTop).toHaveBeenCalledWith(false);
    });

    it('toggle-pin is a no-op when window not found', async () => {
      mockGetWindowFromWebContentsId.mockReturnValue(null);
      const { registerIpcHandlers } =
        await import('@/main/capture/screenshot/pin');
      registerIpcHandlers();
      expect(() =>
        ipcOn['toggle-pin']({ sender: { id: 1 } }, false)
      ).not.toThrow();
    });
  });

  describe('openImageToPin', () => {
    it('does nothing when dialog cancelled', async () => {
      mockShowOpenDialog.mockResolvedValue({
        canceled: true,
        filePaths: [],
      });
      const { openImageToPin } = await import('@/main/capture/screenshot/pin');
      await openImageToPin();
      expect(browserWindows.length).toBe(0);
    });

    it('skips when selected file does not exist', async () => {
      mockShowOpenDialog.mockResolvedValue({
        canceled: false,
        filePaths: ['/p/missing.png'],
      });
      mockExistsSync.mockReturnValue(false);
      const { openImageToPin } = await import('@/main/capture/screenshot/pin');
      await openImageToPin();
      expect(browserWindows.length).toBe(0);
    });

    it('skips when image is empty', async () => {
      mockShowOpenDialog.mockResolvedValue({
        canceled: false,
        filePaths: ['/p/img.png'],
      });
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue(Buffer.from('image'));
      mockNativeImageCreateFromBuffer.mockReturnValue({
        getSize: () => ({ width: 0, height: 0 }),
        isEmpty: () => true,
      });
      const { openImageToPin } = await import('@/main/capture/screenshot/pin');
      await openImageToPin();
      expect(browserWindows.length).toBe(0);
    });

    it('creates pin window on selection', async () => {
      mockShowOpenDialog.mockResolvedValue({
        canceled: false,
        filePaths: ['/p/img.png'],
      });
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue(Buffer.from('image'));
      const { openImageToPin } = await import('@/main/capture/screenshot/pin');
      await openImageToPin();
      expect(browserWindows.length).toBe(1);
    });
  });

  describe('pin window lifecycle', () => {
    it('did-finish-load sends pin params to renderer', async () => {
      mockGetWindowData.mockReturnValue(undefined);
      const { registerIpcHandlers } =
        await import('@/main/capture/screenshot/pin');
      registerIpcHandlers();
      ipcOn['screenshot:pin'](
        { sender: { id: 1 } },
        {
          imageBase64: 'aGVsbG8=',
          editorState: {},
          filePath: '/p/img.png',
          originalWidth: 1600,
          originalHeight: 1200,
        }
      );
      const win = browserWindows[0];
      const handler = win.windowHandlers['wc:did-finish-load']?.[0];
      handler?.();
      expect(win.webContents.send).toHaveBeenCalledWith(
        'load',
        expect.objectContaining({ type: 'pin' })
      );
    });

    it('ready-to-show focuses and shows the pin window', async () => {
      mockGetWindowData.mockReturnValue(undefined);
      const { registerIpcHandlers } =
        await import('@/main/capture/screenshot/pin');
      registerIpcHandlers();
      ipcOn['screenshot:pin'](
        { sender: { id: 1 } },
        {
          imageBase64: 'aGVsbG8=',
          editorState: {},
          filePath: '/p/img.png',
          originalWidth: 1600,
          originalHeight: 1200,
        }
      );
      const win = browserWindows[0];
      const readyHandler = win.windowHandlers['ready-to-show']?.[0];
      await readyHandler?.();
      expect(win.show).toHaveBeenCalled();
      expect(win.focus).toHaveBeenCalled();
    });

    it('closed handler restores screenshot window when restoreToEditor=true', async () => {
      mockGetWindowData.mockReturnValue(undefined);
      const { registerIpcHandlers } =
        await import('@/main/capture/screenshot/pin');
      registerIpcHandlers();
      ipcOn['screenshot:pin'](
        { sender: { id: 1 } },
        {
          imageBase64: 'aGVsbG8=',
          editorState: {},
          filePath: '/p/img.png',
          originalWidth: 1600,
          originalHeight: 1200,
        }
      );
      const win = browserWindows[0];
      const closedHandler = win.windowHandlers['closed']?.[0];
      closedHandler?.();
      expect(mockOpenScreenshotWindow).toHaveBeenCalled();
    });

    it('closed handler skips restore when openImageToPin path', async () => {
      mockShowOpenDialog.mockResolvedValue({
        canceled: false,
        filePaths: ['/p/img.png'],
      });
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue(Buffer.from('image'));
      const { openImageToPin } = await import('@/main/capture/screenshot/pin');
      await openImageToPin();
      const win = browserWindows[0];
      const closedHandler = win.windowHandlers['closed']?.[0];
      closedHandler?.();
      // openImageToPin sets restoreToEditor=false
      expect(mockOpenScreenshotWindow).not.toHaveBeenCalled();
    });
  });
});
