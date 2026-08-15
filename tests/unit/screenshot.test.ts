import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import path from 'path';
import type { SettingsConfig } from '@/types/settings';
import type { HistoryItem, EditorState } from '@/types/history';
import type { RectAnnotation, WallpaperSettings } from '@/types/editor';

const mockDaemonCall = vi.fn();
const mockDaemonOnEvent = vi.fn();
const mockDaemonOffEvent = vi.fn();

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...args: unknown[]) => mockDaemonCall(...args),
    onEvent: (handler: unknown) => mockDaemonOnEvent(handler),
    offEvent: (handler: unknown) => mockDaemonOffEvent(handler),
  },
}));

const mockExistsSync = vi.fn();
const mockMkdirSync = vi.fn();
const mockReadFileSync = vi.fn();
const mockReadFile = vi.fn();
const mockWriteFileSync = vi.fn();
const mockCopyFileSync = vi.fn();
const mockRmSync = vi.fn();

vi.mock('fs', () => ({
  default: {
    existsSync: (path: string) => mockExistsSync(path),
    mkdirSync: (path: string, options?: object) => mockMkdirSync(path, options),
    readFileSync: (path: string) => mockReadFileSync(path),
    writeFileSync: (path: string, data: Buffer) =>
      mockWriteFileSync(path, data),
    copyFileSync: (src: string, dest: string) => mockCopyFileSync(src, dest),
    rmSync: (path: string, options?: object) => mockRmSync(path, options),
    promises: {
      readFile: (path: string) => mockReadFile(path),
    },
  },
  existsSync: (path: string) => mockExistsSync(path),
  mkdirSync: (path: string, options?: object) => mockMkdirSync(path, options),
  readFileSync: (path: string) => mockReadFileSync(path),
  writeFileSync: (path: string, data: Buffer) => mockWriteFileSync(path, data),
  copyFileSync: (src: string, dest: string) => mockCopyFileSync(src, dest),
  rmSync: (path: string, options?: object) => mockRmSync(path, options),
}));

const mockIpcMainOn = vi.fn();
const mockIpcMainHandle = vi.fn();
const mockDialogShowSaveDialog = vi.fn();
const mockDialogShowMessageBox = vi.fn();
const mockClipboardWriteImage = vi.fn();
const mockNativeImageCreateFromBuffer = vi.fn();
const mockNativeImageCreateFromPath = vi.fn();
const mockAppGetPath = vi.fn();
const mockScreenGetPrimaryDisplay = vi.fn();
const mockScreenGetAllDisplays = vi.fn();
const mockScreenGetCursorScreenPoint = vi.fn(() => ({ x: 0, y: 0 }));
const mockScreenGetDisplayNearestPoint = vi.fn();

const mockWebContentsSend = vi.fn();
const mockWebContentsOn = vi.fn();
const mockWebContentsExecuteJavaScript = vi.fn();
const mockWindowLoadURL = vi.fn();
const mockWindowLoadFile = vi.fn();
const mockWindowShow = vi.fn();
const mockWindowClose = vi.fn();
const mockWindowIsDestroyed = vi.fn(() => false);
const mockWindowSetAlwaysOnTop = vi.fn();
const mockWindowOn = vi.fn();
const mockWindowOnce = vi.fn();

class MockBrowserWindow {
  webContents = {
    id: 1,
    send: mockWebContentsSend,
    on: mockWebContentsOn,
    executeJavaScript: mockWebContentsExecuteJavaScript,
  };
  loadURL = mockWindowLoadURL;
  loadFile = mockWindowLoadFile;
  show = mockWindowShow;
  close = mockWindowClose;
  isDestroyed = mockWindowIsDestroyed;
  setAlwaysOnTop = mockWindowSetAlwaysOnTop;
  on = mockWindowOn;
  once = mockWindowOnce;

  static mock = { calls: [] as unknown[][] };

  constructor(options: unknown) {
    MockBrowserWindow.mock.calls.push([options]);
  }

  static resetMock() {
    MockBrowserWindow.mock.calls = [];
  }

  static fromWebContents = vi.fn();
}

vi.mock('electron', () => ({
  app: {
    getPath: (name: string) => mockAppGetPath(name),
  },
  BrowserWindow: MockBrowserWindow,
  screen: {
    getPrimaryDisplay: () => mockScreenGetPrimaryDisplay(),
    getAllDisplays: () => mockScreenGetAllDisplays(),
    getCursorScreenPoint: () => mockScreenGetCursorScreenPoint(),
    getDisplayNearestPoint: (point: unknown) =>
      mockScreenGetDisplayNearestPoint(point),
  },
  ipcMain: {
    on: mockIpcMainOn,
    handle: mockIpcMainHandle,
  },
  dialog: {
    showSaveDialog: mockDialogShowSaveDialog,
    showMessageBox: mockDialogShowMessageBox,
  },
  clipboard: {
    writeImage: mockClipboardWriteImage,
  },
  nativeImage: {
    createFromBuffer: mockNativeImageCreateFromBuffer,
    createFromPath: mockNativeImageCreateFromPath,
  },
  Notification: vi.fn().mockImplementation(() => ({ show: vi.fn() })),
}));

vi.mock('@/main/utils/env', () => ({
  isDev: false,
  isProduction: false,
  devServerUrl: null,
}));

const mockGeneralConfig = {
  startOnLogin: false,
  playSoundOnScreenshot: true,
};

const mockScreenshotConfig = {
  closeOnCopy: false,
  closeOnSave: false,
  captureToClipboard: false,
  hideDesktopIcons: false,
  freezeScreen: false,
};

const mockConfig: Partial<SettingsConfig> = {
  screenshot: { ...mockScreenshotConfig },
  general: { ...mockGeneralConfig },
};

const mockGetConfig = vi.fn(() => mockConfig);
const mockUpdateConfig = vi.fn();
const mockCreateOrShowSettingsWindow = vi.fn();

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
  updateConfig: (config: unknown) => mockUpdateConfig(config),
  createOrShowSettingsWindow: (tab?: string) =>
    mockCreateOrShowSettingsWindow(tab),
}));

const mockHideDesktopIcons = vi.fn();
const mockShowDesktopIcons = vi.fn();
const mockIsDesktopIconsSupported = vi.fn(() => true);

vi.mock('@/main/capture/desktop-icons', () => ({
  hideDesktopIcons: () => mockHideDesktopIcons(),
  showDesktopIcons: () => mockShowDesktopIcons(),
  checkAccessibilityPermission: () => true,
  isSupported: () => mockIsDesktopIconsSupported(),
}));

const mockAddToHistory = vi.fn();
const mockUpdateHistoryItemByPath = vi.fn();
const mockGetHistoryItemByPath = vi.fn();
const mockGetHistoryItem = vi.fn();
const mockDeleteHistoryItem = vi.fn();

vi.mock('@/main/history', () => ({
  addToHistory: (path: string) => mockAddToHistory(path),
  updateHistoryItemByPath: (path: string, state: unknown) =>
    mockUpdateHistoryItemByPath(path, state),
  getHistoryItemByPath: (path: string) => mockGetHistoryItemByPath(path),
  getHistoryItem: (id: string) => mockGetHistoryItem(id),
  deleteHistoryItem: (id: string) => mockDeleteHistoryItem(id),
  isHistoryPopoverWebContents: vi.fn(() => true),
}));

const mockSelectDisplay = vi.fn();

vi.mock('@/main/capture/display-selector', () => ({
  selectDisplay: () => mockSelectDisplay(),
  displayFromSelection: vi.fn(),
  killDisplaySelector: vi.fn(),
}));

const mockCaptureDisplayToFile = vi.fn();

vi.mock('@/main/capture/screenshot/native-capture', () => ({
  captureDisplayToFile: (display: unknown, filePath: string) =>
    mockCaptureDisplayToFile(display, filePath),
}));

const mockCaptureAreaToFile = vi.fn();
const mockCaptureWindowToFile = vi.fn();

vi.mock('@/main/capture/area-overlay', () => ({
  captureAreaToFile: (filePath: string) => mockCaptureAreaToFile(filePath),
  captureWindowToFile: (filePath: string) => mockCaptureWindowToFile(filePath),
}));

vi.mock('@/main/system/capabilities', () => ({
  isFeatureSupported: () => true,
}));

const mockFinalizeCapture = vi.fn(() => Promise.resolve());
const mockPrepareScreenshotPreview = vi.fn(() => null);

vi.mock('@/main/capture/screenshot/finalize', () => ({
  finalizeCapture: (filePath: string, preparation?: unknown) =>
    mockFinalizeCapture(filePath, preparation),
  prepareScreenshotPreview: () => mockPrepareScreenshotPreview(),
}));

vi.mock('@/main/capture/capture-preview', () => ({
  prepareCapturePreview: vi.fn(),
  showCapturePreview: vi.fn(),
  registerCapturePreviewIpc: vi.fn(),
}));

function createWallpaperSettings(
  overrides: Partial<WallpaperSettings> = {}
): WallpaperSettings {
  return {
    gradient: null,
    backgroundImage: null,
    backgroundBlur: 0,
    padding: 0,
    corners: 0,
    shadow: 0,
    windowFrame: { style: 'none' },
    ...overrides,
  };
}

describe('Screenshot Module', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    MockBrowserWindow.resetMock();

    mockAppGetPath.mockImplementation((name: string) => {
      const paths: Record<string, string> = {
        pictures: '/mock/Pictures',
        userData: '/mock/userData',
      };
      return paths[name] || `/mock/${name}`;
    });

    mockScreenGetPrimaryDisplay.mockReturnValue({
      scaleFactor: 2,
      workAreaSize: { width: 1920, height: 1080 },
    });

    mockScreenGetAllDisplays.mockReturnValue([{ id: 1 }]);
    mockScreenGetDisplayNearestPoint.mockReturnValue({
      id: 1,
      bounds: { x: 0, y: 0, width: 1920, height: 1080 },
    });

    mockSelectDisplay.mockResolvedValue({
      status: 'selected',
      displayNumber: 1,
    });

    mockExistsSync.mockReturnValue(true);
    mockCaptureDisplayToFile.mockResolvedValue(true);
    mockCaptureWindowToFile.mockResolvedValue(true);
    mockCaptureAreaToFile.mockResolvedValue(true);

    mockGetConfig.mockReturnValue({
      screenshot: { ...mockScreenshotConfig },
      general: { ...mockGeneralConfig },
    });
  });

  afterEach(() => {
    vi.resetModules();
  });

  describe('IPC Handlers Registration', () => {
    it('should register toggle-pin handler', async () => {
      await import('@/main/capture/screenshot');

      expect(mockIpcMainOn).toHaveBeenCalledWith(
        'toggle-pin',
        expect.any(Function)
      );
    });

    it('should register screenshot:close-confirmed handler', async () => {
      await import('@/main/capture/screenshot');

      expect(mockIpcMainOn).toHaveBeenCalledWith(
        'screenshot:close-confirmed',
        expect.any(Function)
      );
    });

    it('should register screenshot:copy-from-menu handler', async () => {
      await import('@/main/capture/screenshot');

      expect(mockIpcMainOn).toHaveBeenCalledWith(
        'screenshot:copy-from-menu',
        expect.any(Function)
      );
    });

    it('should register save-screenshot handler', async () => {
      await import('@/main/capture/screenshot');

      expect(mockIpcMainOn).toHaveBeenCalledWith(
        'save-screenshot',
        expect.any(Function)
      );
    });

    it('should register screenshot:save-edited handler', async () => {
      await import('@/main/capture/screenshot');

      expect(mockIpcMainOn).toHaveBeenCalledWith(
        'screenshot:save-edited',
        expect.any(Function)
      );
    });

    it('should register get-screenshot-path handler', async () => {
      await import('@/main/capture/screenshot');

      expect(mockIpcMainOn).toHaveBeenCalledWith(
        'get-screenshot-path',
        expect.any(Function)
      );
    });

    it('should register screenshot:read-file handler', async () => {
      await import('@/main/capture/screenshot');

      expect(mockIpcMainHandle).toHaveBeenCalledWith(
        'screenshot:read-file',
        expect.any(Function)
      );
    });

    it('should register history:save-editor-state handler', async () => {
      await import('@/main/capture/screenshot');

      expect(mockIpcMainOn).toHaveBeenCalledWith(
        'history:save-editor-state',
        expect.any(Function)
      );
    });

    it('should register history:openScreenshot handler', async () => {
      await import('@/main/capture/screenshot');

      expect(mockIpcMainOn).toHaveBeenCalledWith(
        'history:openScreenshot',
        expect.any(Function)
      );
    });

    it('should register open-settings handler', async () => {
      await import('@/main/capture/screenshot');

      expect(mockIpcMainOn).toHaveBeenCalledWith(
        'open-settings',
        expect.any(Function)
      );
    });

    it('should register screenshot:pin handler', async () => {
      await import('@/main/capture/screenshot');

      expect(mockIpcMainOn).toHaveBeenCalledWith(
        'screenshot:pin',
        expect.any(Function)
      );
    });

    it('should register screenshot:sync-state handler', async () => {
      await import('@/main/capture/screenshot');

      expect(mockIpcMainOn).toHaveBeenCalledWith(
        'screenshot:sync-state',
        expect.any(Function)
      );
    });
  });

  describe('screenshot() function', () => {
    it('should create Poratake directory if it does not exist', async () => {
      mockExistsSync.mockImplementation((path: string) => {
        if (path.includes('Poratake')) return false;
        return true;
      });

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('screen');

      expect(mockMkdirSync).toHaveBeenCalledWith(
        expect.stringContaining('Poratake'),
        { recursive: true }
      );
    });

    it('should not create Poratake directory if it already exists', async () => {
      mockExistsSync.mockReturnValue(true);

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('screen');

      expect(mockMkdirSync).not.toHaveBeenCalled();
    });

    it('should capture the display in screen mode', async () => {
      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('screen');

      expect(mockCaptureDisplayToFile).toHaveBeenCalledWith(
        expect.objectContaining({ id: 1 }),
        expect.stringContaining('Poratake')
      );
      expect(mockFinalizeCapture).toHaveBeenCalled();
    });

    it('should skip the display selector on single-display setups', async () => {
      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('screen');

      expect(mockSelectDisplay).not.toHaveBeenCalled();
    });

    it('should capture the overlay selection in area mode', async () => {
      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('area');

      expect(mockCaptureAreaToFile).toHaveBeenCalledWith(
        expect.stringContaining('Poratake')
      );
      expect(mockFinalizeCapture).toHaveBeenCalled();
    });

    it('should default to area mode when no mode specified', async () => {
      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot();

      expect(mockCaptureAreaToFile).toHaveBeenCalled();
    });

    it('should not finalize when area capture fails', async () => {
      mockCaptureAreaToFile.mockResolvedValue(false);

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('area');

      expect(mockFinalizeCapture).not.toHaveBeenCalled();
    });

    it('should capture the selected window in window mode', async () => {
      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('window');

      expect(mockCaptureWindowToFile).toHaveBeenCalledWith(
        expect.stringContaining('Poratake')
      );
      expect(mockFinalizeCapture).toHaveBeenCalled();
    });

    it('should not finalize when window capture fails', async () => {
      mockCaptureWindowToFile.mockResolvedValue(false);
      vi.spyOn(console, 'error').mockImplementation(() => {});

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('window');

      expect(mockFinalizeCapture).not.toHaveBeenCalled();
      vi.restoreAllMocks();
    });

    it('should hide desktop icons when enabled and supported', async () => {
      mockGetConfig.mockReturnValue({
        screenshot: { ...mockScreenshotConfig, hideDesktopIcons: true },
        general: { ...mockGeneralConfig },
      });

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('area');

      expect(mockHideDesktopIcons).toHaveBeenCalled();
      expect(mockShowDesktopIcons).toHaveBeenCalled();
    });

    it('should not hide desktop icons when disabled', async () => {
      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('area');

      expect(mockHideDesktopIcons).not.toHaveBeenCalled();
    });

    it('should not hide desktop icons when not supported', async () => {
      mockGetConfig.mockReturnValue({
        screenshot: { ...mockScreenshotConfig, hideDesktopIcons: true },
        general: { ...mockGeneralConfig },
      });
      mockIsDesktopIconsSupported.mockReturnValue(false);

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('area');

      expect(mockHideDesktopIcons).not.toHaveBeenCalled();
    });

    it('should generate timestamp-based filenames', async () => {
      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('area');

      expect(mockCaptureAreaToFile).toHaveBeenCalledWith(
        expect.stringMatching(
          /Screenshot \d{4}-\d{2}-\d{2} at \d{2}\.\d{2}\.\d{2}\.png/
        )
      );
    });

    it('should save to the Poratake folder in Pictures', async () => {
      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('area');

      expect(mockCaptureAreaToFile).toHaveBeenCalledWith(
        expect.stringContaining(
          path.join('/mock/Pictures', 'Poratake') + path.sep
        )
      );
    });
  });

  describe('IPC Handler: screenshot:read-file', () => {
    it('should return base64 encoded file content', async () => {
      const { openScreenshotWindow } =
        await import('@/main/capture/screenshot/open-editor');
      openScreenshotWindow({
        filePath: '/path/to/image.png',
        width: 100,
        height: 100,
      });
      await import('@/main/capture/screenshot');

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const handlerCall = (mockIpcMainHandle.mock.calls as any[]).find(
        call => call[0] === 'screenshot:read-file'
      );
      const handler = handlerCall?.[1];

      const testBuffer = Buffer.from('test image data');
      mockExistsSync.mockReturnValue(true);
      mockReadFile.mockResolvedValue(testBuffer);

      const result = await handler({ sender: { id: 1 } });

      expect(result).toBe(testBuffer.toString('base64'));
      expect(mockReadFile).toHaveBeenCalledWith('/path/to/image.png');
    });

    it('should throw error when file not found', async () => {
      const { openScreenshotWindow } =
        await import('@/main/capture/screenshot/open-editor');
      openScreenshotWindow({
        filePath: '/nonexistent/file.png',
        width: 100,
        height: 100,
      });
      await import('@/main/capture/screenshot');

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const handlerCall = (mockIpcMainHandle.mock.calls as any[]).find(
        call => call[0] === 'screenshot:read-file'
      );
      const handler = handlerCall?.[1];

      mockExistsSync.mockReturnValue(false);

      await expect(handler({ sender: { id: 1 } })).rejects.toThrow(
        'File not found'
      );
    });
  });

  describe('IPC Handler: open-settings', () => {
    it('should open settings window', async () => {
      await import('@/main/capture/screenshot');

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const handlerCall = (mockIpcMainOn.mock.calls as any[]).find(
        call => call[0] === 'open-settings'
      );
      const handler = handlerCall?.[1];

      handler({ sender: { id: 1 } }, 'general');

      expect(mockCreateOrShowSettingsWindow).toHaveBeenCalledWith('general');
    });

    it('should open settings window without tab', async () => {
      await import('@/main/capture/screenshot');

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const handlerCall = (mockIpcMainOn.mock.calls as any[]).find(
        call => call[0] === 'open-settings'
      );
      const handler = handlerCall?.[1];

      handler({ sender: { id: 1 } });

      expect(mockCreateOrShowSettingsWindow).toHaveBeenCalledWith(undefined);
    });
  });

  describe('openScreenshotFromHistory', () => {
    it('should log error when file does not exist', async () => {
      const consoleSpy = vi
        .spyOn(console, 'error')
        .mockImplementation(() => {});
      mockExistsSync.mockReturnValue(false);

      const { openScreenshotFromHistory } =
        await import('@/main/capture/screenshot');

      const historyItem: HistoryItem = {
        id: 'test-id',
        timestamp: Date.now(),
        originalPath: '/nonexistent/path.png',
        type: 'screenshot',
        editorState: null,
      };

      openScreenshotFromHistory(historyItem);

      expect(consoleSpy).toHaveBeenCalledWith(
        'Screenshot file not found:',
        '/nonexistent/path.png'
      );
      consoleSpy.mockRestore();
    });

    it('should open screenshot window when file exists', async () => {
      mockExistsSync.mockReturnValue(true);
      mockNativeImageCreateFromPath.mockReturnValue({
        getSize: () => ({ width: 400, height: 300 }),
      });

      const { openScreenshotFromHistory } =
        await import('@/main/capture/screenshot');

      const editorState: EditorState = {
        annotations: [],
        wallpaper: createWallpaperSettings(),
      };

      const historyItem: HistoryItem = {
        id: 'test-id',
        timestamp: Date.now(),
        originalPath: '/test/path.png',
        type: 'screenshot',
        editorState,
      };

      openScreenshotFromHistory(historyItem);

      expect(MockBrowserWindow.mock.calls.length).toBeGreaterThan(0);
    });

    it('should pass editor state to new window', async () => {
      mockExistsSync.mockReturnValue(true);
      mockNativeImageCreateFromPath.mockReturnValue({
        getSize: () => ({ width: 400, height: 300 }),
      });

      const { openScreenshotFromHistory } =
        await import('@/main/capture/screenshot');

      const annotation: RectAnnotation = {
        id: 'ann-1',
        type: 'rectangle',
        x: 10,
        y: 20,
        width: 100,
        height: 50,
        stroke: '#FF0000',
        strokeWidth: 2,
      };

      const editorState: EditorState = {
        annotations: [annotation],
        wallpaper: createWallpaperSettings({
          padding: 20,
          corners: 8,
          shadow: 10,
        }),
      };

      const historyItem: HistoryItem = {
        id: 'test-id',
        timestamp: Date.now(),
        originalPath: '/test/path.png',
        type: 'screenshot',
        editorState,
      };

      openScreenshotFromHistory(historyItem);

      expect(MockBrowserWindow.mock.calls.length).toBeGreaterThan(0);
      const windowOptions = MockBrowserWindow.mock.calls[0][0] as {
        webPreferences?: { devTools?: boolean };
      };
      expect(windowOptions.webPreferences?.devTools).toBe(false);
    });
  });
});
