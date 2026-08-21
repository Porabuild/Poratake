import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockDaemonCall = vi.fn();
const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockHideDesktopIcons = vi.fn();
const mockShowDesktopIcons = vi.fn();
const mockIsSupported = vi.fn();
const mockCheckAccessibility = vi.fn();
const mockExistsSync = vi.fn();
const mockReadFileSync = vi.fn();
const mockClipboardWriteImage = vi.fn();
const mockShowCapturePreview = vi.fn();
const mockPrepareCapturePreview = vi.fn();
const mockDisposeCapturePreview = vi.fn();
const mockOpenScreenshotEditor = vi.fn();
const mockAddToHistory = vi.fn();
const mockDipToScreenRect = vi.fn();
const mockGetDisplayMatching = vi.fn();
const mockSelectAreaWithOverlay = vi.fn();
const mockCancelOverlaySelection = vi.fn();
const mockReleaseSelection = vi.fn();
const mockConfirmAreaSelection = vi.fn();
const mockPrewarmScrollCaptureControl = vi.fn();
const mockShowScrollCaptureOverlay = vi.fn();
const mockUpdateScrollCaptureState = vi.fn();
const mockHideScrollCaptureOverlay = vi.fn();

vi.mock('electron', () => ({
  clipboard: { writeImage: (...a: unknown[]) => mockClipboardWriteImage(...a) },
  nativeImage: { createFromBuffer: vi.fn(() => ({ isEmpty: () => false })) },
  screen: {
    dipToScreenRect: (...a: unknown[]) => mockDipToScreenRect(...a),
    getDisplayMatching: (...a: unknown[]) => mockGetDisplayMatching(...a),
    getPrimaryDisplay: () => ({ id: 1 }),
    getAllDisplays: () => [{ id: 1 }],
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

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...a: unknown[]) => mockDaemonCall(...a),
    onEvent: vi.fn(),
    offEvent: vi.fn(),
  },
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
  updateConfig: (...a: unknown[]) => mockUpdateConfig(...a),
}));

vi.mock('@/main/capture/desktop-icons', () => ({
  hideDesktopIcons: (...a: unknown[]) => mockHideDesktopIcons(...a),
  showDesktopIcons: (...a: unknown[]) => mockShowDesktopIcons(...a),
  isSupported: () => mockIsSupported(),
  checkAccessibilityPermission: (...a: unknown[]) =>
    mockCheckAccessibility(...a),
}));

vi.mock('@/main/capture/desktop-icons/preference', () => ({
  shouldHideDesktopIconsForCapture: () => {
    const config = mockGetConfig();
    if (!config.screenshot?.hideDesktopIcons || !mockIsSupported())
      return false;
    if (mockCheckAccessibility(false)) return true;
    mockUpdateConfig({
      screenshot: { ...config.screenshot, hideDesktopIcons: false },
    });
    return false;
  },
}));

vi.mock('@/main/history', () => ({
  addToHistory: (...a: unknown[]) => mockAddToHistory(...a),
}));

vi.mock('@/main/capture/screenshot/utils', () => ({
  generateScreenshotPath: vi.fn(() => '/p/out.png'),
}));

vi.mock('@/main/capture/capture-preview', () => ({
  showCapturePreview: (...a: unknown[]) => mockShowCapturePreview(...a),
  prepareCapturePreview: () => mockPrepareCapturePreview(),
}));

vi.mock('@/main/capture/screenshot/open-editor', () => ({
  openScreenshotEditor: (...a: unknown[]) => mockOpenScreenshotEditor(...a),
}));

vi.mock('@/main/capture/area-selector', () => ({
  startAreaSelection: vi.fn(),
  confirmAreaSelection: (...args: unknown[]) =>
    mockConfirmAreaSelection(...args),
}));

vi.mock('@/main/capture/area-overlay', () => ({
  cancelOverlaySelection: (...a: unknown[]) => mockCancelOverlaySelection(...a),
  selectAreaWithOverlay: (...a: unknown[]) => mockSelectAreaWithOverlay(...a),
}));

vi.mock('@/main/capture/scroll-capture/scroll-capture-window', () => ({
  prewarmScrollCaptureControl: () => mockPrewarmScrollCaptureControl(),
  showScrollCaptureOverlay: (...a: unknown[]) =>
    mockShowScrollCaptureOverlay(...a),
  updateScrollCaptureState: (...a: unknown[]) =>
    mockUpdateScrollCaptureState(...a),
  hideScrollCaptureOverlay: (...a: unknown[]) =>
    mockHideScrollCaptureOverlay(...a),
}));

describe('scroll-capture', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockGetConfig.mockReturnValue({
      screenshot: { hideDesktopIcons: false },
      scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
    });
    mockIsSupported.mockReturnValue(true);
    mockCheckAccessibility.mockReturnValue(true);
    mockAddToHistory.mockResolvedValue({ id: 'h1' });
    mockDipToScreenRect.mockImplementation((_window, bounds) => bounds);
    mockGetDisplayMatching.mockReturnValue({ scaleFactor: 1 });
    mockReleaseSelection.mockResolvedValue(undefined);
    mockConfirmAreaSelection.mockResolvedValue({ status: 'confirmed' });
    mockShowScrollCaptureOverlay.mockReturnValue(true);
    mockPrepareCapturePreview.mockReturnValue({
      dispose: mockDisposeCapturePreview,
    });
  });

  describe('cancelScrollCapture', () => {
    it('calls daemon cancel', async () => {
      mockDaemonCall.mockResolvedValue({});
      const { cancelScrollCapture } =
        await import('@/main/capture/scroll-capture');
      await cancelScrollCapture();
      expect(mockDaemonCall).toHaveBeenCalledWith('scroll-capture', 'cancel');
    });

    it('swallows daemon errors', async () => {
      mockDaemonCall.mockRejectedValue(new Error('boom'));
      const { cancelScrollCapture } =
        await import('@/main/capture/scroll-capture');
      await expect(cancelScrollCapture()).resolves.toBeUndefined();
    });

    it('cancels a pending Windows overlay selection', async () => {
      const originalPlatform = process.platform;
      Object.defineProperty(process, 'platform', { value: 'win32' });

      try {
        const { cancelScrollCapture } =
          await import('@/main/capture/scroll-capture');
        await cancelScrollCapture();
        expect(mockCancelOverlaySelection).toHaveBeenCalledWith(true);
      } finally {
        Object.defineProperty(process, 'platform', { value: originalPlatform });
      }
    });
  });

  describe('startScrollCapture', () => {
    it('hides desktop icons when configured', async () => {
      mockGetConfig.mockReturnValue({
        screenshot: { hideDesktopIcons: true },
      });
      const areaSelector = await import('@/main/capture/area-selector');
      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onCancelled?: () => Promise<void>;
      }) => {
        setImmediate(() => opts.onCancelled?.());
        return undefined;
      }) as never);
      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      await startScrollCapture();
      expect(mockHideDesktopIcons).toHaveBeenCalledWith('capture');
      expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
    });

    it('disables hide-icons when accessibility permission is missing', async () => {
      mockGetConfig.mockReturnValue({
        screenshot: { hideDesktopIcons: true },
      });
      mockCheckAccessibility.mockReturnValue(false);
      const areaSelector = await import('@/main/capture/area-selector');
      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onCancelled?: () => Promise<void>;
      }) => {
        setImmediate(() => opts.onCancelled?.());
        return undefined;
      }) as never);
      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      await startScrollCapture();
      expect(mockUpdateConfig).toHaveBeenCalled();
      expect(mockHideDesktopIcons).not.toHaveBeenCalled();
    });

    it('completes when area selection is cancelled', async () => {
      const areaSelector = await import('@/main/capture/area-selector');
      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onCancelled?: () => Promise<void>;
      }) => {
        setImmediate(() => opts.onCancelled?.());
        return undefined;
      }) as never);
      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      await expect(startScrollCapture()).resolves.toBeUndefined();
    });

    it('keeps the session active after handing off the selector overlay', async () => {
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let options:
        | {
            onSelected: (selection: unknown) => Promise<void>;
          }
        | undefined;
      let resolveSelector: ((selection: null) => void) | undefined;
      let daemonHandler: ((event: string, data?: unknown) => void) | null =
        null;

      vi.mocked(areaSelector.startAreaSelection).mockImplementation((opts => {
        options = opts as typeof options;
        return new Promise<null>(resolve => {
          resolveSelector = resolve;
        });
      }) as never);
      mockConfirmAreaSelection.mockImplementationOnce(async () => {
        resolveSelector?.(null);
        return { status: 'confirmed' };
      });
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation((handler => {
        daemonHandler = handler as typeof daemonHandler;
      }) as never);
      mockDaemonCall.mockResolvedValue({});

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      let settled = false;
      const capture = startScrollCapture().then(() => {
        settled = true;
      });

      await options?.onSelected({
        status: 'selected',
        x: 10,
        y: 20,
        width: 200,
        height: 100,
        screenId: 7,
      });
      await Promise.resolve();

      expect(settled).toBe(false);
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'scroll-capture',
        'start',
        expect.anything()
      );

      await daemonHandler?.('scroll-capture:cancelled');
      await capture;
      expect(settled).toBe(true);
    });

    it('restores desktop icons when selector startup returns null', async () => {
      mockGetConfig.mockReturnValue({
        screenshot: { hideDesktopIcons: true },
      });
      const areaSelector = await import('@/main/capture/area-selector');
      vi.mocked(areaSelector.startAreaSelection).mockResolvedValue(null);

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      await startScrollCapture();

      expect(mockPrewarmScrollCaptureControl).toHaveBeenCalledTimes(1);
      expect(mockHideDesktopIcons).toHaveBeenCalledWith('capture');
      expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
      expect(mockDaemonCall).not.toHaveBeenCalledWith(
        'scroll-capture',
        'start',
        expect.anything()
      );
    });

    it('does not replace an active session or its event handler', async () => {
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let options:
        | {
            onSelected: (selection: unknown) => Promise<void>;
          }
        | undefined;
      let resolveSelector: ((selection: null) => void) | undefined;
      let daemonHandler: ((event: string, data?: unknown) => void) | null =
        null;

      vi.mocked(areaSelector.startAreaSelection).mockImplementation((opts => {
        options = opts as typeof options;
        return new Promise<null>(resolve => {
          resolveSelector = resolve;
        });
      }) as never);
      mockConfirmAreaSelection.mockImplementationOnce(async () => {
        resolveSelector?.(null);
        return { status: 'confirmed' };
      });
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation((handler => {
        daemonHandler = handler as typeof daemonHandler;
      }) as never);
      mockDaemonCall.mockResolvedValue({});

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      const first = startScrollCapture();
      await options?.onSelected({
        status: 'selected',
        x: 10,
        y: 20,
        width: 200,
        height: 100,
      });
      const second = startScrollCapture();

      expect(areaSelector.startAreaSelection).toHaveBeenCalledTimes(1);
      expect(daemonModule.daemon.onEvent).toHaveBeenCalledTimes(1);

      await daemonHandler?.('scroll-capture:cancelled');
      await Promise.all([first, second]);

      vi.mocked(areaSelector.startAreaSelection).mockResolvedValueOnce(null);
      await startScrollCapture();
      expect(areaSelector.startAreaSelection).toHaveBeenCalledTimes(2);
    });

    it('processes area selection then triggers daemon scroll-capture start', async () => {
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let daemonHandler: ((e: string, d?: unknown) => void) | null = null;
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation(((
        cb: (e: string, d?: unknown) => void
      ) => {
        daemonHandler = cb;
      }) as never);
      mockDaemonCall.mockResolvedValue({});

      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onSelected?: (s: unknown) => Promise<void>;
      }) => {
        setImmediate(async () => {
          await opts.onSelected?.({
            status: 'selected',
            x: 10,
            y: 20,
            width: 200,
            height: 100,
            screenId: 7,
          });
          // Fire scroll-capture:done event after async setup
          await new Promise(res => setImmediate(res));
          mockDaemonCall.mockResolvedValueOnce({
            success: true,
            outputPath: '/p/out.png',
            width: 200,
            height: 1000,
          });
          daemonHandler?.('scroll-capture:done');
        });
        return undefined;
      }) as never);

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      await startScrollCapture();
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'scroll-capture',
        'start',
        expect.objectContaining({
          x: 10,
          y: 20,
          width: 200,
          height: 100,
          displayId: 7,
        })
      );
      expect(areaSelector.startAreaSelection).toHaveBeenCalledWith(
        expect.objectContaining({ renderer: 'scroll-capture-overlay' })
      );
      expect(mockConfirmAreaSelection.mock.invocationCallOrder[0]).toBeLessThan(
        mockShowScrollCaptureOverlay.mock.invocationCallOrder[0]
      );
    });

    it('selects with the Electron overlay on Windows and converts DIP bounds', async () => {
      const originalPlatform = process.platform;
      Object.defineProperty(process, 'platform', { value: 'win32' });
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let daemonHandler: ((event: string) => void) | null = null;
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation(((
        handler: (event: string) => void
      ) => {
        daemonHandler = handler;
      }) as never);
      mockDaemonCall.mockResolvedValue({});
      mockDipToScreenRect.mockReturnValue({
        x: 20,
        y: 40,
        width: 400,
        height: 200,
      });
      mockSelectAreaWithOverlay.mockImplementation(async () => {
        setImmediate(() => daemonHandler?.('scroll-capture:cancelled'));
        return {
          display: {
            id: 7,
            scaleFactor: 2,
            bounds: { x: 0, y: 0, width: 1920, height: 1080 },
          },
          rect: { x: 10, y: 20, width: 200, height: 100 },
          release: (...a: unknown[]) => mockReleaseSelection(...a),
        };
      });

      try {
        const { startScrollCapture } =
          await import('@/main/capture/scroll-capture');
        await startScrollCapture();

        expect(mockSelectAreaWithOverlay).toHaveBeenCalledWith({
          freeze: false,
        });
        expect(mockReleaseSelection).not.toHaveBeenCalled();
        expect(mockDipToScreenRect).toHaveBeenCalledWith(null, {
          x: 10,
          y: 20,
          width: 200,
          height: 100,
        });
        expect(mockDaemonCall).toHaveBeenCalledWith(
          'scroll-capture',
          'start',
          expect.objectContaining({
            x: 20,
            y: 40,
            width: 400,
            height: 200,
            scaleFactor: 2,
          })
        );
        expect(areaSelector.startAreaSelection).not.toHaveBeenCalled();
      } finally {
        Object.defineProperty(process, 'platform', { value: originalPlatform });
      }
    });

    it('completes without starting when the overlay selection is cancelled on Windows', async () => {
      const originalPlatform = process.platform;
      Object.defineProperty(process, 'platform', { value: 'win32' });
      mockSelectAreaWithOverlay.mockResolvedValue(null);

      try {
        const { startScrollCapture } =
          await import('@/main/capture/scroll-capture');
        await expect(startScrollCapture()).resolves.toBeUndefined();
        expect(mockDaemonCall).not.toHaveBeenCalledWith(
          'scroll-capture',
          'start',
          expect.anything()
        );
      } finally {
        Object.defineProperty(process, 'platform', { value: originalPlatform });
      }
    });

    it('handles scroll-capture:cancelled event', async () => {
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let daemonHandler: ((e: string, d?: unknown) => void) | null = null;
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation(((
        cb: (e: string, d?: unknown) => void
      ) => {
        daemonHandler = cb;
      }) as never);
      mockDaemonCall.mockResolvedValue({});

      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onSelected?: (s: unknown) => Promise<void>;
      }) => {
        setImmediate(async () => {
          await opts.onSelected?.({
            status: 'selected',
            x: 0,
            y: 0,
            width: 100,
            height: 100,
          });
          await new Promise(res => setImmediate(res));
          daemonHandler?.('scroll-capture:cancelled');
        });
        return undefined;
      }) as never);

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      await expect(startScrollCapture()).resolves.toBeUndefined();
    });

    it('handles daemon start failure gracefully', async () => {
      const areaSelector = await import('@/main/capture/area-selector');
      mockDaemonCall.mockRejectedValue(new Error('daemon dead'));

      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onSelected?: (s: unknown) => Promise<void>;
      }) => {
        setImmediate(() =>
          opts.onSelected?.({
            status: 'selected',
            x: 0,
            y: 0,
            width: 100,
            height: 100,
          })
        );
        return undefined;
      }) as never);

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      await expect(startScrollCapture()).resolves.toBeUndefined();
    });

    it('ignores area selection without complete bounds', async () => {
      const areaSelector = await import('@/main/capture/area-selector');
      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onSelected?: (s: unknown) => Promise<void>;
        onCancelled?: () => Promise<void>;
      }) => {
        setImmediate(async () => {
          await opts.onSelected?.({ status: 'selected' });
          await opts.onCancelled?.();
        });
        return undefined;
      }) as never);

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      await startScrollCapture();
      expect(mockDaemonCall).not.toHaveBeenCalledWith(
        'scroll-capture',
        'start',
        expect.anything()
      );
    });

    it('opens screenshot editor after successful capture (default flow)', async () => {
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let daemonHandler: ((e: string, d?: unknown) => void) | null = null;
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation(((
        cb: (e: string, d?: unknown) => void
      ) => {
        daemonHandler = cb;
      }) as never);
      mockGetConfig.mockReturnValue({
        screenshot: {
          hideDesktopIcons: false,
          captureToClipboard: false,
          showPreview: false,
        },
        scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
      });
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue(Buffer.from('image'));
      let finishCount = 0;
      mockDaemonCall.mockImplementation(async (_module, method) => {
        if (method === 'finish') {
          finishCount++;
          return {
            success: true,
            outputPath: '/p/out.png',
            width: 200,
            height: 1000,
          };
        }
        return {};
      });

      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onSelected?: (s: unknown) => Promise<void>;
      }) => {
        setImmediate(async () => {
          await opts.onSelected?.({
            status: 'selected',
            x: 0,
            y: 0,
            width: 100,
            height: 100,
          });
          await new Promise(res => setImmediate(res));
          daemonHandler?.('scroll-capture:done');
        });
        return undefined;
      }) as never);

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      await startScrollCapture();
      expect(finishCount).toBeGreaterThan(0);
      expect(mockOpenScreenshotEditor).toHaveBeenCalled();
    });

    it('writes to clipboard when captureToClipboard enabled', async () => {
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let daemonHandler: ((e: string, d?: unknown) => void) | null = null;
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation(((
        cb: (e: string, d?: unknown) => void
      ) => {
        daemonHandler = cb;
      }) as never);
      mockGetConfig.mockReturnValue({
        screenshot: {
          hideDesktopIcons: false,
          captureToClipboard: true,
          showPreview: false,
        },
        scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
      });
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue(Buffer.from('image'));
      mockDaemonCall.mockImplementation(async (_module, method) => {
        if (method === 'finish') {
          return {
            success: true,
            outputPath: '/p/out.png',
            width: 200,
            height: 1000,
          };
        }
        return {};
      });

      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onSelected?: (s: unknown) => Promise<void>;
      }) => {
        setImmediate(async () => {
          await opts.onSelected?.({
            status: 'selected',
            x: 0,
            y: 0,
            width: 100,
            height: 100,
          });
          await new Promise(res => setImmediate(res));
          daemonHandler?.('scroll-capture:done');
        });
        return undefined;
      }) as never);

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      await startScrollCapture();
      expect(mockClipboardWriteImage).toHaveBeenCalled();
    });

    it('shows capture preview when configured', async () => {
      const calls: string[] = [];
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let daemonHandler: ((e: string, d?: unknown) => void) | null = null;
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation(((
        cb: (e: string, d?: unknown) => void
      ) => {
        daemonHandler = cb;
      }) as never);
      mockGetConfig.mockReturnValue({
        screenshot: {
          hideDesktopIcons: false,
          captureToClipboard: false,
          showPreview: true,
        },
        scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
      });
      mockExistsSync.mockReturnValue(true);
      let finishCapture: (result: unknown) => void = () => {};
      mockDaemonCall.mockImplementation((_module, method) => {
        if (method === 'finish') {
          calls.push('finish');
          return new Promise(resolve => {
            finishCapture = resolve;
          });
        }
        return Promise.resolve({});
      });
      const preparation = { dispose: mockDisposeCapturePreview };
      mockPrepareCapturePreview.mockImplementationOnce(() => {
        calls.push('preview');
        return preparation;
      });

      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onSelected?: (s: unknown) => Promise<void>;
      }) => {
        setImmediate(async () => {
          await opts.onSelected?.({
            status: 'selected',
            x: 0,
            y: 0,
            width: 100,
            height: 100,
          });
          await new Promise(res => setImmediate(res));
          daemonHandler?.('scroll-capture:done');
        });
        return undefined;
      }) as never);

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      const capturing = startScrollCapture();

      await vi.waitFor(() => expect(calls).toEqual(['finish', 'preview']));
      finishCapture({
        success: true,
        outputPath: '/p/out.png',
        width: 200,
        height: 1000,
      });
      await capturing;

      expect(mockShowCapturePreview).toHaveBeenCalledWith(
        '/p/out.png',
        'screenshot',
        undefined,
        preparation,
        expect.any(Promise)
      );
    });

    it('handles failed daemon.finish response', async () => {
      mockGetConfig.mockReturnValue({
        screenshot: {
          hideDesktopIcons: false,
          captureToClipboard: false,
          showPreview: true,
        },
        scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
      });
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let daemonHandler: ((e: string, d?: unknown) => void) | null = null;
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation(((
        cb: (e: string, d?: unknown) => void
      ) => {
        daemonHandler = cb;
      }) as never);
      mockDaemonCall.mockImplementation(async (_module, method) => {
        if (method === 'finish') {
          return { success: false };
        }
        return {};
      });

      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onSelected?: (s: unknown) => Promise<void>;
      }) => {
        setImmediate(async () => {
          await opts.onSelected?.({
            status: 'selected',
            x: 0,
            y: 0,
            width: 100,
            height: 100,
          });
          await new Promise(res => setImmediate(res));
          daemonHandler?.('scroll-capture:done');
        });
        return undefined;
      }) as never);

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      await expect(startScrollCapture()).resolves.toBeUndefined();
      expect(mockPrepareCapturePreview).toHaveBeenCalledTimes(1);
      expect(mockDisposeCapturePreview).toHaveBeenCalledTimes(1);
    });

    it('disposes a pending finish preview immediately on cancellation', async () => {
      mockGetConfig.mockReturnValue({
        screenshot: {
          hideDesktopIcons: false,
          captureToClipboard: false,
          showPreview: true,
        },
        scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
      });
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let daemonHandler: ((event: string) => void) | null = null;
      let finishCapture: (result: unknown) => void = () => {};
      let finishCancel: () => void = () => {};
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation((handler => {
        daemonHandler = handler as typeof daemonHandler;
      }) as never);
      mockDaemonCall.mockImplementation((_module, method) => {
        if (method === 'finish') {
          return new Promise(resolve => {
            finishCapture = resolve;
          });
        }
        if (method === 'cancel') {
          return new Promise<void>(resolve => {
            finishCancel = resolve;
          });
        }
        return Promise.resolve({});
      });
      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onSelected?: (selection: unknown) => Promise<void>;
      }) => {
        setImmediate(async () => {
          await opts.onSelected?.({
            status: 'selected',
            x: 0,
            y: 0,
            width: 100,
            height: 100,
          });
          await new Promise(resolve => setImmediate(resolve));
          daemonHandler?.('scroll-capture:done');
        });
        return undefined;
      }) as never);

      const { cancelScrollCapture, startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      const capturing = startScrollCapture();
      await vi.waitFor(() =>
        expect(mockPrepareCapturePreview).toHaveBeenCalledTimes(1)
      );

      const cancellation = cancelScrollCapture();
      await Promise.resolve();
      expect(mockDisposeCapturePreview).toHaveBeenCalledTimes(1);
      finishCancel();
      await cancellation;
      await capturing;

      finishCapture({ success: true, outputPath: '/p/out.png' });
      await Promise.resolve();
      expect(mockShowCapturePreview).not.toHaveBeenCalled();
      expect(mockDisposeCapturePreview).toHaveBeenCalledTimes(1);
    });

    it('cancels a finish preview while session cleanup is pending', async () => {
      mockGetConfig.mockReturnValue({
        screenshot: {
          hideDesktopIcons: true,
          captureToClipboard: false,
          showPreview: true,
        },
        scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
      });
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let daemonHandler: ((event: string) => void) | null = null;
      let restoreIcons: () => void = () => {};
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation((handler => {
        daemonHandler = handler as typeof daemonHandler;
      }) as never);
      mockShowDesktopIcons.mockReturnValueOnce(
        new Promise<boolean>(resolve => {
          restoreIcons = () => resolve(true);
        })
      );
      mockDaemonCall.mockImplementation((_module, method) => {
        if (method === 'finish') {
          return Promise.resolve({ success: true, outputPath: '/p/out.png' });
        }
        return Promise.resolve({});
      });
      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onSelected?: (selection: unknown) => Promise<void>;
      }) => {
        setImmediate(async () => {
          await opts.onSelected?.({
            status: 'selected',
            x: 0,
            y: 0,
            width: 100,
            height: 100,
          });
          await new Promise(resolve => setImmediate(resolve));
          daemonHandler?.('scroll-capture:done');
        });
        return undefined;
      }) as never);

      const { cancelScrollCapture, startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      const capturing = startScrollCapture();
      await vi.waitFor(() =>
        expect(mockShowDesktopIcons).toHaveBeenCalledTimes(1)
      );

      await cancelScrollCapture();
      expect(mockDisposeCapturePreview).toHaveBeenCalledTimes(1);
      expect(mockShowCapturePreview).not.toHaveBeenCalled();

      restoreIcons();
      await capturing;
      expect(mockShowCapturePreview).not.toHaveBeenCalled();
    });

    it('shares one finish request across duplicate completion events', async () => {
      mockGetConfig.mockReturnValue({
        screenshot: {
          hideDesktopIcons: false,
          captureToClipboard: false,
          showPreview: true,
        },
        scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
      });
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let daemonHandler: ((event: string) => void) | null = null;
      let finishCapture: (result: unknown) => void = () => {};
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation((handler => {
        daemonHandler = handler as typeof daemonHandler;
      }) as never);
      mockDaemonCall.mockImplementation((_module, method) => {
        if (method === 'finish') {
          return new Promise(resolve => {
            finishCapture = resolve;
          });
        }
        return Promise.resolve({});
      });
      vi.mocked(areaSelector.startAreaSelection).mockImplementation(((opts: {
        onSelected?: (selection: unknown) => Promise<void>;
      }) => {
        setImmediate(async () => {
          await opts.onSelected?.({
            status: 'selected',
            x: 0,
            y: 0,
            width: 100,
            height: 100,
          });
          await new Promise(resolve => setImmediate(resolve));
          daemonHandler?.('scroll-capture:done');
          daemonHandler?.('scroll-capture:done');
        });
        return undefined;
      }) as never);

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      const capturing = startScrollCapture();
      await vi.waitFor(() =>
        expect(mockPrepareCapturePreview).toHaveBeenCalledTimes(1)
      );

      expect(
        mockDaemonCall.mock.calls.filter(([, method]) => method === 'finish')
      ).toHaveLength(1);
      finishCapture({ success: true, outputPath: '/p/out.png' });
      await capturing;
    });
  });
});
