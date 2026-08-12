import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockDaemonCall = vi.fn();
const mockExistsSync = vi.fn(() => true);
const mockReadFileSync = vi.fn(() => Buffer.from('image-bytes'));
const mockWriteFileSync = vi.fn();
const mockMkdirSync = vi.fn();
const mockClipboardWriteImage = vi.fn();
const mockGetAllDisplays = vi.fn();
const mockAddToHistory = vi.fn();
const mockSelectDisplay = vi.fn();
const mockDisplayFromSelection = vi.fn();
const mockSelectWindow = vi.fn();
const mockCaptureDisplayToFile = vi.fn();
const mockCaptureFrozenScreenRegionToFile = vi.fn();
const mockCaptureWindowToFile = vi.fn();
const mockGetCursorScreenPoint = vi.fn();
const mockGetDisplayNearestPoint = vi.fn();
const mockCaptureAreaToFile = vi.fn();
const mockGetConfig = vi.fn();
const mockHideDesktopIcons = vi.fn();
const mockShowDesktopIcons = vi.fn();
const mockDesktopIconsSupported = vi.fn();
const mockPrepareCapturePreview = vi.fn();
const mockShowCapturePreview = vi.fn();
const mockDisposePreparedPreview = vi.fn();
const mockFreezeScreen = vi.fn();
const mockReleaseScreen = vi.fn();

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...a: unknown[]) => mockDaemonCall(...a),
    onEvent: vi.fn(),
    offEvent: vi.fn(),
  },
}));

vi.mock('child_process', () => ({
  execFile: vi.fn(),
}));

vi.mock('fs', () => ({
  default: {
    existsSync: (...a: unknown[]) => mockExistsSync(...a),
    readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
    writeFileSync: (...a: unknown[]) => mockWriteFileSync(...a),
    mkdirSync: (...a: unknown[]) => mockMkdirSync(...a),
  },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
  readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
  writeFileSync: (...a: unknown[]) => mockWriteFileSync(...a),
  mkdirSync: (...a: unknown[]) => mockMkdirSync(...a),
}));

vi.mock('electron', () => ({
  app: { getPath: () => '/mock/pictures' },
  BrowserWindow: class {},
  ipcMain: { on: vi.fn(), handle: vi.fn() },
  dialog: { showSaveDialog: vi.fn(), showMessageBox: vi.fn() },
  clipboard: { writeImage: (...a: unknown[]) => mockClipboardWriteImage(...a) },
  nativeImage: {
    createFromBuffer: vi.fn(() => ({ image: true, isEmpty: () => false })),
  },
  Notification: class {
    show = vi.fn();
  },
  screen: {
    getAllDisplays: () => mockGetAllDisplays(),
    getPrimaryDisplay: vi.fn(),
    getCursorScreenPoint: () => mockGetCursorScreenPoint(),
    getDisplayNearestPoint: (...a: unknown[]) =>
      mockGetDisplayNearestPoint(...a),
  },
}));

vi.mock('@/main/utils/env', () => ({
  isDev: false,
  devServerUrl: null,
  isProduction: true,
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
  updateConfig: vi.fn(),
  createOrShowSettingsWindow: vi.fn(),
}));

vi.mock('@/main/capture/desktop-icons', () => ({
  hideDesktopIcons: (...a: unknown[]) => mockHideDesktopIcons(...a),
  showDesktopIcons: (...a: unknown[]) => mockShowDesktopIcons(...a),
  isSupported: () => mockDesktopIconsSupported(),
  checkAccessibilityPermission: () => true,
}));

vi.mock('@/main/capture/freeze-screen', () => ({
  freezeScreen: (...a: unknown[]) => mockFreezeScreen(...a),
  releaseScreen: (...a: unknown[]) => mockReleaseScreen(...a),
  isSupported: () => true,
}));

vi.mock('@/main/history', () => ({
  addToHistory: (...a: unknown[]) => mockAddToHistory(...a),
  updateHistoryItemByPath: vi.fn(),
  getHistoryItemByPath: vi.fn(),
  deleteHistoryItem: vi.fn(),
}));

vi.mock('@/main/capture/capture-preview', () => ({
  prepareCapturePreview: (...a: unknown[]) => mockPrepareCapturePreview(...a),
  showCapturePreview: (...a: unknown[]) => mockShowCapturePreview(...a),
  registerCapturePreviewIpc: vi.fn(),
}));

vi.mock('@/main/capture/area-selector', () => ({
  startAreaSelection: vi.fn(),
  cancelAreaSelection: vi.fn(),
  hideAreaSelector: vi.fn(),
}));

vi.mock('@/main/capture/display-selector', () => ({
  selectDisplay: (...a: unknown[]) => mockSelectDisplay(...a),
  displayFromSelection: (...a: unknown[]) => mockDisplayFromSelection(...a),
  killDisplaySelector: vi.fn(),
}));

vi.mock('@/main/capture/window-selector', () => ({
  selectWindow: (...a: unknown[]) => mockSelectWindow(...a),
  killWindowSelector: vi.fn(),
}));

vi.mock('@/main/capture/screenshot/native-capture', () => ({
  captureDisplayToFile: (...a: unknown[]) => mockCaptureDisplayToFile(...a),
  captureFrozenScreenRegionToFile: (...a: unknown[]) =>
    mockCaptureFrozenScreenRegionToFile(...a),
  captureWindowToFile: (...a: unknown[]) => mockCaptureWindowToFile(...a),
}));

vi.mock('@/main/capture/area-overlay', () => ({
  captureAreaToFile: (...a: unknown[]) => mockCaptureAreaToFile(...a),
  selectAreaWithOverlay: vi.fn(),
}));

const cursorDisplay = {
  id: 1,
  bounds: { x: 0, y: 0, width: 1920, height: 1080 },
};
const secondDisplay = {
  id: 2,
  bounds: { x: 1920, y: 0, width: 1920, height: 1080 },
};

describe('screenshot on Windows', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockGetConfig.mockReset();
    mockHideDesktopIcons.mockReset();
    mockShowDesktopIcons.mockReset();
    mockDesktopIconsSupported.mockReset();
    mockCaptureDisplayToFile.mockReset();
    mockCaptureFrozenScreenRegionToFile.mockReset();
    mockCaptureWindowToFile.mockReset();
    mockCaptureAreaToFile.mockReset();
    mockPrepareCapturePreview.mockReset();
    mockShowCapturePreview.mockReset();
    mockDisposePreparedPreview.mockReset();
    mockFreezeScreen.mockReset();
    mockReleaseScreen.mockReset();
    Object.defineProperty(process, 'platform', { value: 'win32' });
    mockExistsSync.mockReturnValue(true);
    mockAddToHistory.mockResolvedValue({ id: 'h1' });
    mockGetCursorScreenPoint.mockReturnValue({ x: 0, y: 0 });
    mockGetDisplayNearestPoint.mockReturnValue(cursorDisplay);
    mockGetAllDisplays.mockReturnValue([cursorDisplay]);
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: false },
      screenshot: {
        captureToClipboard: true,
        hideDesktopIcons: false,
        freezeScreen: false,
      },
    });
    mockDesktopIconsSupported.mockReturnValue(false);
    mockCaptureDisplayToFile.mockResolvedValue(true);
    mockCaptureFrozenScreenRegionToFile.mockResolvedValue(true);
    mockCaptureWindowToFile.mockResolvedValue(true);
    mockCaptureAreaToFile.mockResolvedValue(true);
    mockPrepareCapturePreview.mockReturnValue({
      dispose: mockDisposePreparedPreview,
    });
    mockShowCapturePreview.mockReturnValue({ revealed: Promise.resolve() });
    mockFreezeScreen.mockResolvedValue(true);
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  describe('screen mode', () => {
    it('prepares the preview while native capture is running', async () => {
      mockGetConfig.mockReturnValue({
        general: { playSoundOnScreenshot: false },
        screenshot: {
          autoCopyToClipboard: false,
          captureToClipboard: false,
          showPreview: true,
          hideDesktopIcons: false,
        },
      });
      let finishCapture: (captured: boolean) => void = () => {};
      mockCaptureDisplayToFile.mockReturnValueOnce(
        new Promise(resolve => {
          finishCapture = resolve;
        })
      );
      const { default: screenshot } = await import('@/main/capture/screenshot');

      const capturing = screenshot('screen');

      expect(mockPrepareCapturePreview).toHaveBeenCalledTimes(1);
      expect(mockCaptureDisplayToFile).toHaveBeenCalledTimes(1);
      expect(mockShowCapturePreview).not.toHaveBeenCalled();

      finishCapture(true);
      await capturing;

      expect(mockShowCapturePreview).toHaveBeenCalledWith(
        expect.any(String),
        'screenshot',
        undefined,
        expect.objectContaining({ dispose: mockDisposePreparedPreview }),
        expect.any(Promise)
      );
      expect(mockDisposePreparedPreview).toHaveBeenCalledTimes(1);
    });

    it('disposes the prepared preview when native capture fails', async () => {
      mockGetConfig.mockReturnValue({
        general: { playSoundOnScreenshot: false },
        screenshot: {
          autoCopyToClipboard: false,
          captureToClipboard: false,
          showPreview: true,
          hideDesktopIcons: false,
        },
      });
      mockCaptureDisplayToFile.mockResolvedValueOnce(false);
      const { default: screenshot } = await import('@/main/capture/screenshot');

      await screenshot('screen');

      expect(mockShowCapturePreview).not.toHaveBeenCalled();
      expect(mockDisposePreparedPreview).toHaveBeenCalledTimes(1);
    });

    it('captures the cursor display without a selector on a single display', async () => {
      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('screen');

      expect(mockSelectDisplay).not.toHaveBeenCalled();
      expect(mockCaptureDisplayToFile).toHaveBeenCalledWith(
        cursorDisplay,
        expect.any(String)
      );
      expect(mockClipboardWriteImage).toHaveBeenCalled();
    });

    it('uses the display selector when multiple displays exist', async () => {
      mockGetAllDisplays.mockReturnValue([cursorDisplay, secondDisplay]);
      const selection = {
        status: 'selected',
        displayNumber: 2,
        bounds: { x: 1920, y: 0, width: 1920, height: 1080 },
      };
      mockSelectDisplay.mockResolvedValue(selection);
      mockDisplayFromSelection.mockReturnValue(secondDisplay);

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('screen');

      expect(mockSelectDisplay).toHaveBeenCalled();
      expect(mockDisplayFromSelection).toHaveBeenCalledWith(selection);
      expect(mockCaptureDisplayToFile).toHaveBeenCalledWith(
        secondDisplay,
        expect.any(String)
      );
    });

    it('aborts when display selection is cancelled', async () => {
      mockGetAllDisplays.mockReturnValue([cursorDisplay, secondDisplay]);
      mockSelectDisplay.mockResolvedValue({ status: 'cancelled' });

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('screen');

      expect(mockCaptureDisplayToFile).not.toHaveBeenCalled();
    });

    it('falls back to the cursor display when selection fails', async () => {
      mockGetAllDisplays.mockReturnValue([cursorDisplay, secondDisplay]);
      mockSelectDisplay.mockRejectedValue(new Error('daemon down'));

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('screen');

      expect(mockCaptureDisplayToFile).toHaveBeenCalledWith(
        cursorDisplay,
        expect.any(String)
      );
    });

    it('hides desktop icons only while acquiring the display image', async () => {
      const calls: string[] = [];
      mockGetConfig.mockReturnValue({
        general: { playSoundOnScreenshot: false },
        screenshot: {
          captureToClipboard: true,
          hideDesktopIcons: true,
          freezeScreen: false,
        },
      });
      mockDesktopIconsSupported.mockReturnValue(true);
      mockHideDesktopIcons.mockImplementation(async () => {
        calls.push('hide');
      });
      mockCaptureDisplayToFile.mockImplementation(async () => {
        calls.push('capture');
        return true;
      });
      mockShowDesktopIcons.mockImplementation(async () => {
        calls.push('show');
      });

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('screen');

      expect(calls).toEqual(['hide', 'capture', 'show']);
    });

    it('starts the preview while desktop icons are being restored', async () => {
      mockGetConfig.mockReturnValue({
        general: { playSoundOnScreenshot: false },
        screenshot: {
          autoCopyToClipboard: false,
          captureToClipboard: false,
          showPreview: true,
          hideDesktopIcons: true,
          freezeScreen: false,
        },
      });
      mockDesktopIconsSupported.mockReturnValue(true);
      let finishRestore: (restored: boolean) => void = () => {};
      mockShowDesktopIcons.mockReturnValueOnce(
        new Promise(resolve => {
          finishRestore = resolve;
        })
      );
      const { default: screenshot } = await import('@/main/capture/screenshot');

      const capturing = screenshot('screen');

      await vi.waitFor(() => {
        expect(mockShowCapturePreview).toHaveBeenCalledTimes(1);
      });

      finishRestore(true);
      await capturing;
    });

    it('restores desktop icons when display capture throws', async () => {
      mockGetConfig.mockReturnValue({
        general: { playSoundOnScreenshot: false },
        screenshot: {
          captureToClipboard: true,
          hideDesktopIcons: true,
          freezeScreen: false,
        },
      });
      mockDesktopIconsSupported.mockReturnValue(true);
      mockCaptureDisplayToFile.mockRejectedValue(new Error('capture failed'));

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await expect(screenshot('screen')).rejects.toThrow('capture failed');

      expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
    });
  });

  describe('area mode', () => {
    it('hides icons before selection and restores them after cancellation', async () => {
      const calls: string[] = [];
      mockGetConfig.mockReturnValue({
        general: { playSoundOnScreenshot: false },
        screenshot: {
          captureToClipboard: true,
          hideDesktopIcons: true,
          freezeScreen: false,
        },
      });
      mockDesktopIconsSupported.mockReturnValue(true);
      mockHideDesktopIcons.mockImplementation(async () => {
        calls.push('hide');
      });
      mockCaptureAreaToFile.mockImplementation(async () => {
        calls.push('select');
        return false;
      });
      mockShowDesktopIcons.mockImplementation(async () => {
        calls.push('show');
      });

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('area');

      expect(calls).toEqual(['hide', 'select', 'show']);
      expect(mockAddToHistory).not.toHaveBeenCalled();
    });
  });

  describe('window mode', () => {
    it('captures the selected window natively', async () => {
      mockSelectWindow.mockResolvedValue({
        status: 'selected',
        windowId: 264610,
        windowTitle: 'Notepad',
        ownerName: 'notepad',
        ownerPid: 42,
        bounds: { x: 10, y: 20, width: 800, height: 600 },
      });

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('window');

      expect(mockCaptureWindowToFile).toHaveBeenCalledWith(
        264610,
        expect.any(String)
      );
      expect(mockClipboardWriteImage).toHaveBeenCalled();
    });

    it('does nothing when window selection is cancelled', async () => {
      mockSelectWindow.mockResolvedValue({ status: 'cancelled' });

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('window');

      expect(mockCaptureWindowToFile).not.toHaveBeenCalled();
    });

    it('uses the freeze screen setting for window selection', async () => {
      mockGetConfig.mockReturnValue({
        general: { playSoundOnScreenshot: false },
        screenshot: {
          captureToClipboard: true,
          hideDesktopIcons: false,
          freezeScreen: true,
        },
      });
      mockSelectWindow.mockResolvedValue({ status: 'cancelled' });

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('window');

      expect(mockFreezeScreen).toHaveBeenCalledWith(true);
      expect(mockReleaseScreen).toHaveBeenCalledTimes(1);
    });

    it('crops a frozen window from the retained desktop snapshot', async () => {
      mockGetConfig.mockReturnValue({
        general: { playSoundOnScreenshot: false },
        screenshot: {
          captureToClipboard: true,
          hideDesktopIcons: false,
          freezeScreen: true,
        },
      });
      mockSelectWindow.mockResolvedValue({
        status: 'selected',
        windowId: 264610,
        bounds: { x: 200, y: 100, width: 800, height: 600 },
      });

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('window');

      expect(mockCaptureFrozenScreenRegionToFile).toHaveBeenCalledWith(
        { x: 200, y: 100, width: 800, height: 600 },
        expect.any(String),
        264610
      );
      expect(mockCaptureWindowToFile).not.toHaveBeenCalled();
      expect(mockReleaseScreen).toHaveBeenCalledTimes(1);
    });

    it('falls back to live window capture when freezing fails', async () => {
      mockGetConfig.mockReturnValue({
        general: { playSoundOnScreenshot: false },
        screenshot: {
          captureToClipboard: true,
          hideDesktopIcons: false,
          freezeScreen: true,
        },
      });
      mockFreezeScreen.mockResolvedValue(false);
      mockSelectWindow.mockResolvedValue({
        status: 'selected',
        windowId: 264610,
        bounds: { x: 200, y: 100, width: 800, height: 600 },
      });

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await screenshot('window');

      expect(mockCaptureWindowToFile).toHaveBeenCalledWith(
        264610,
        expect.any(String)
      );
      expect(mockCaptureFrozenScreenRegionToFile).not.toHaveBeenCalled();
      expect(mockReleaseScreen).not.toHaveBeenCalled();
    });

    it('handles selector errors gracefully', async () => {
      mockSelectWindow.mockRejectedValue(new Error('NO_WINDOWS'));

      const { default: screenshot } = await import('@/main/capture/screenshot');
      await expect(screenshot('window')).resolves.toBeUndefined();

      expect(mockCaptureWindowToFile).not.toHaveBeenCalled();
    });
  });
});
