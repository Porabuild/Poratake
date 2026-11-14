import { describe, it, expect, vi, beforeEach } from 'vitest';
import type * as FileSystem from 'node:fs';
import type * as ScreenshotUtils from '@/main/capture/screenshot/utils';

const mockDaemonCall = vi.fn();
const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockHideDesktopIcons = vi.fn();
const mockShowDesktopIcons = vi.fn();
const mockIsSupported = vi.fn();
const mockCheckAccessibility = vi.fn();
const mockExistsSync = vi.fn();
const mockReadFileSync = vi.fn();
const mockLink = vi.fn();
const mockCopyFile = vi.fn();
const mockUnlink = vi.fn();
const mockUnlinkSync = vi.fn();
const mockPublishScreenshot = vi.fn();
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
const mockShowNotification = vi.fn();

vi.mock('electron', () => ({
  app: { isPackaged: false, getPath: vi.fn() },
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
    promises: {
      link: (...a: unknown[]) => mockLink(...a),
      copyFile: (...a: unknown[]) => mockCopyFile(...a),
      unlink: (...a: unknown[]) => mockUnlink(...a),
    },
    constants: { COPYFILE_EXCL: 1 },
    unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
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
  publishScreenshot: (...a: unknown[]) => mockPublishScreenshot(...a),
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

vi.mock('@/main/utils/notification', () => ({
  showNotification: (...a: unknown[]) => mockShowNotification(...a),
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
    mockLink.mockReset();
    mockCopyFile.mockReset();
    mockUnlink.mockReset();
    mockPublishScreenshot.mockReset();
    mockPublishScreenshot.mockImplementation(
      async (_temporary: string, output: string) => output
    );
    mockUnlinkSync.mockReset();
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

    it('keeps a cancelled session active until the daemon responds', async () => {
      const areaSelector = await import('@/main/capture/area-selector');
      let options:
        | { onSelected: (selection: unknown) => Promise<void> }
        | undefined;
      let finishCancel: () => void = () => {};
      vi.mocked(areaSelector.startAreaSelection).mockImplementation((opts => {
        options = opts as typeof options;
        return new Promise<null>(() => {});
      }) as never);
      mockDaemonCall.mockImplementation((_module, method) => {
        if (method === 'cancel') {
          return new Promise<void>(resolve => {
            finishCancel = resolve;
          });
        }
        return Promise.resolve({});
      });

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
      const action = mockShowScrollCaptureOverlay.mock.calls[0][1] as (
        action: 'cancel' | 'done'
      ) => Promise<void>;
      const cancellation = action('cancel');
      await action('done');
      expect(
        mockDaemonCall.mock.calls.filter(([, method]) => method === 'finish')
      ).toHaveLength(0);
      await new Promise(resolve => setImmediate(resolve));
      const second = startScrollCapture();
      const selectionCount = vi.mocked(areaSelector.startAreaSelection).mock
        .calls.length;

      finishCancel();
      await cancellation;
      await first;
      expect(selectionCount).toBe(1);
      await second;
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

    it.each(['win32', 'darwin'])(
      'reports an interrupted scroll without discarding the capture on %s',
      async platform => {
        const originalPlatform = process.platform;
        Object.defineProperty(process, 'platform', { value: platform });
        try {
          const daemonModule = await import('@/main/daemon');
          const areaSelector = await import('@/main/capture/area-selector');
          let daemonHandler:
            | ((event: string, data?: unknown) => void)
            | undefined;
          let select: ((selection: unknown) => Promise<void>) | undefined;
          vi.mocked(daemonModule.daemon.onEvent).mockImplementation(
            (handler => {
              daemonHandler = handler as typeof daemonHandler;
            }) as never
          );
          vi.mocked(areaSelector.startAreaSelection).mockImplementation(
            (opts => {
              select = (opts as { onSelected: typeof select }).onSelected;
              return new Promise<null>(() => {});
            }) as never
          );
          mockSelectAreaWithOverlay.mockResolvedValue({
            display: { id: 7, scaleFactor: 1 },
            rect: { x: 10, y: 20, width: 200, height: 100 },
          });
          mockGetConfig.mockReturnValue({
            screenshot: { hideDesktopIcons: false, showPreview: true },
            scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
          });
          mockExistsSync.mockReturnValue(true);
          let finish: (result: unknown) => void = () => {};
          mockDaemonCall.mockImplementation((_module, method) =>
            method === 'finish'
              ? new Promise(resolve => {
                  finish = resolve;
                })
              : Promise.resolve({})
          );
          const { startScrollCapture } =
            await import('@/main/capture/scroll-capture');
          const capture = startScrollCapture();
          if (platform !== 'win32') {
            await select?.({
              status: 'selected',
              x: 10,
              y: 20,
              width: 200,
              height: 100,
            });
          }
          await vi.waitFor(() =>
            expect(mockDaemonCall).toHaveBeenCalledWith(
              'scroll-capture',
              'start',
              expect.anything()
            )
          );

          if (platform !== 'win32') {
            daemonHandler?.('scroll-capture:auto-scroll', { scrolling: true });
          }
          expect(
            mockUpdateScrollCaptureState.mock.lastCall?.[0]?.isAutoScrolling
          ).toBe(platform === 'win32' ? undefined : true);
          for (const reason of ['duplicate', 'max-height']) {
            daemonHandler?.('scroll-capture:scroll-ended', { reason });
          }
          expect(
            mockUpdateScrollCaptureState.mock.lastCall?.[0]?.isAutoScrolling
          ).toBe(platform === 'win32' ? undefined : false);
          expect(mockShowNotification).not.toHaveBeenCalled();
          for (const reason of ['no-overlap', 'capture-error', 'input-error']) {
            daemonHandler?.('scroll-capture:scroll-ended', {
              reason,
              frameCount: 2,
            });
          }
          expect(mockShowNotification.mock.calls).toEqual([
            [
              'Scroll Capture Stopped',
              'The latest frame could not be aligned. You can save the captured portion.',
            ],
            [
              'Scroll Capture Stopped',
              'A frame could not be captured. You can save the captured portion.',
            ],
            [
              'Scroll Capture Stopped',
              'Scrolling could not be continued. You can save the captured portion.',
            ],
          ]);
          expect(mockDaemonCall).not.toHaveBeenCalledWith(
            'scroll-capture',
            'cancel'
          );
          expect(mockDaemonCall).not.toHaveBeenCalledWith(
            'scroll-capture',
            'finish',
            expect.anything(),
            null
          );

          daemonHandler?.('scroll-capture:done');
          await vi.waitFor(() =>
            expect(mockDaemonCall).toHaveBeenCalledWith(
              'scroll-capture',
              'finish',
              expect.anything(),
              null
            )
          );
          daemonHandler?.('scroll-capture:scroll-ended', {
            reason: 'no-overlap',
          });
          expect(mockShowNotification).toHaveBeenCalledTimes(3);
          finish({ success: true });
          await capture;
          expect(mockPublishScreenshot).toHaveBeenCalled();
          expect(mockShowCapturePreview).toHaveBeenCalled();
        } finally {
          Object.defineProperty(process, 'platform', {
            value: originalPlatform,
          });
        }
      }
    );

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

      const temporaryPath = mockDaemonCall.mock.calls.find(
        ([, method]) => method === 'finish'
      )?.[2].outputPath;
      expect(
        mockDaemonCall.mock.calls.find(([, method]) => method === 'finish')?.[3]
      ).toBeNull();
      expect(temporaryPath).toMatch(/^\/p\/out\.png\..+\.tmp\.png$/);
      expect(mockPublishScreenshot).toHaveBeenCalledWith(
        temporaryPath,
        '/p/out.png'
      );
      expect(mockUnlinkSync).not.toHaveBeenCalled();
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

    it('keeps the session active to retry a retryable finish error', async () => {
      mockGetConfig.mockReturnValue({
        screenshot: {
          hideDesktopIcons: false,
          captureToClipboard: false,
          showPreview: false,
        },
        scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
      });
      mockExistsSync.mockReturnValue(true);
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let options:
        | { onSelected: (selection: unknown) => Promise<void> }
        | undefined;
      let daemonHandler: ((event: string, data?: unknown) => void) | null =
        null;
      let finishAttempts = 0;
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation(handler => {
        daemonHandler = handler as typeof daemonHandler;
      });
      mockDaemonCall.mockImplementation((_module, method) => {
        if (method !== 'finish') return Promise.resolve({});
        finishAttempts++;
        if (finishAttempts === 1) {
          return Promise.reject(
            Object.assign(new Error('could not start stitch worker'), {
              code: 'STITCH_RETRYABLE',
            })
          );
        }
        return Promise.resolve({ success: true });
      });
      vi.mocked(areaSelector.startAreaSelection).mockImplementation(opts => {
        options = opts as typeof options;
        return new Promise<null>(() => {});
      });

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      let settled = false;
      const capture = startScrollCapture().then(() => {
        settled = true;
      });
      await options?.onSelected({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const action = mockShowScrollCaptureOverlay.mock.calls[0][1] as (
        action: 'done' | 'cancel'
      ) => Promise<void>;

      await action('done');
      expect(settled).toBe(false);
      expect(finishAttempts).toBe(1);
      expect(mockDaemonCall).not.toHaveBeenCalledWith(
        'scroll-capture',
        'cancel'
      );
      expect(daemonModule.daemon.offEvent).not.toHaveBeenCalled();
      expect(mockHideScrollCaptureOverlay).not.toHaveBeenCalled();
      expect(mockPublishScreenshot).not.toHaveBeenCalled();
      expect(mockUnlinkSync).toHaveBeenCalledTimes(1);
      expect(mockShowNotification).toHaveBeenCalledWith(
        'Scroll Capture Ready to Retry',
        'Choose Done to retry stitching or Cancel to discard the capture.'
      );

      await action('done');
      await capture;
      expect(finishAttempts).toBe(2);
      expect(mockPublishScreenshot).toHaveBeenCalledTimes(1);
      expect(daemonModule.daemon.offEvent).toHaveBeenCalledTimes(1);
      expect(mockHideScrollCaptureOverlay).toHaveBeenCalledTimes(1);
      expect(settled).toBe(true);
    });

    it('cancels and releases a retryable finish session', async () => {
      const areaSelector = await import('@/main/capture/area-selector');
      let options:
        | { onSelected: (selection: unknown) => Promise<void> }
        | undefined;
      let cancelDaemon = false;
      mockDaemonCall.mockImplementation((_module, method) => {
        if (method === 'finish') {
          return Promise.reject(
            Object.assign(new Error('could not start stitch worker'), {
              code: 'STITCH_RETRYABLE',
            })
          );
        }
        if (method === 'cancel') cancelDaemon = true;
        return Promise.resolve({});
      });
      vi.mocked(areaSelector.startAreaSelection).mockImplementation(opts => {
        options = opts as typeof options;
        return new Promise<null>(() => {});
      });

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      const capture = startScrollCapture();
      await options?.onSelected({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const action = mockShowScrollCaptureOverlay.mock.calls[0][1] as (
        action: 'done' | 'cancel'
      ) => Promise<void>;
      await action('done');
      await action('cancel');
      await capture;

      expect(cancelDaemon).toBe(true);
      expect(mockHideScrollCaptureOverlay).toHaveBeenCalledTimes(1);
      vi.mocked(areaSelector.startAreaSelection).mockResolvedValueOnce(null);
      await startScrollCapture();
      expect(areaSelector.startAreaSelection).toHaveBeenCalledTimes(2);
    });

    it('does not show retry guidance when cancel wins an in-flight finish', async () => {
      mockExistsSync.mockReturnValue(true);
      const areaSelector = await import('@/main/capture/area-selector');
      let options:
        | { onSelected: (selection: unknown) => Promise<void> }
        | undefined;
      let failFinish: (error: Error) => void = () => {};
      let finishCancel: () => void = () => {};
      mockDaemonCall.mockImplementation((_module, method) => {
        if (method === 'finish') {
          return new Promise((_resolve, reject) => {
            failFinish = reject;
          });
        }
        if (method === 'cancel') {
          return new Promise<void>(resolve => {
            finishCancel = resolve;
          });
        }
        return Promise.resolve({});
      });
      vi.mocked(areaSelector.startAreaSelection).mockImplementation(opts => {
        options = opts as typeof options;
        return new Promise<null>(() => {});
      });

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      let settled = false;
      const capture = startScrollCapture().then(() => {
        settled = true;
      });
      await options?.onSelected({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const action = mockShowScrollCaptureOverlay.mock.calls[0][1] as (
        action: 'done' | 'cancel'
      ) => Promise<void>;
      const finishing = action('done');
      await vi.waitFor(() =>
        expect(
          mockDaemonCall.mock.calls.some(([, method]) => method === 'finish')
        ).toBe(true)
      );
      const cancellation = action('cancel');
      await vi.waitFor(() =>
        expect(mockDaemonCall).toHaveBeenCalledWith(
          'scroll-capture',
          'cancel',
          undefined,
          null
        )
      );
      failFinish(
        Object.assign(new Error('could not start stitch worker'), {
          code: 'STITCH_RETRYABLE',
        })
      );
      await new Promise(resolve => setImmediate(resolve));
      expect(mockShowNotification).not.toHaveBeenCalledWith(
        'Scroll Capture Ready to Retry',
        expect.anything()
      );
      expect(mockUnlinkSync).not.toHaveBeenCalled();
      expect(settled).toBe(false);

      finishCancel();
      await Promise.all([finishing, cancellation, capture]);
      expect(mockShowNotification).not.toHaveBeenCalledWith(
        'Scroll Capture Ready to Retry',
        expect.anything()
      );
      expect(mockUnlinkSync).toHaveBeenCalledTimes(1);
      expect(mockHideScrollCaptureOverlay).toHaveBeenCalledTimes(1);
      expect(settled).toBe(true);
    });

    it.each(['editor', 'publish'])(
      'releases the session when %s fails after stitching',
      async failure => {
        const daemonModule = await import('@/main/daemon');
        const areaSelector = await import('@/main/capture/area-selector');
        let options:
          | { onSelected: (selection: unknown) => Promise<void> }
          | undefined;
        let daemonHandler: ((event: string) => Promise<void>) | undefined;
        vi.mocked(areaSelector.startAreaSelection).mockImplementation((opts => {
          options = opts as typeof options;
          return new Promise<null>(() => {});
        }) as never);
        vi.mocked(daemonModule.daemon.onEvent).mockImplementation((handler => {
          daemonHandler = handler as typeof daemonHandler;
        }) as never);
        mockGetConfig.mockReturnValue({
          screenshot: { hideDesktopIcons: false, showPreview: false },
          scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
        });
        mockExistsSync.mockReturnValue(true);
        if (failure === 'editor') {
          mockOpenScreenshotEditor.mockImplementationOnce(() => {
            throw new Error('editor unavailable');
          });
        } else {
          mockPublishScreenshot.mockRejectedValueOnce(
            new Error('destination unavailable')
          );
        }
        mockDaemonCall.mockImplementation(async (_module, method) =>
          method === 'finish' ? { success: true } : {}
        );
        const error = vi.spyOn(console, 'error').mockImplementation(() => {});
        try {
          const { startScrollCapture } =
            await import('@/main/capture/scroll-capture');
          let settled = false;
          const capture = startScrollCapture().then(() => {
            settled = true;
          });
          await options?.onSelected({
            status: 'selected',
            x: 0,
            y: 0,
            width: 100,
            height: 100,
          });
          await daemonHandler?.('scroll-capture:done');
          await new Promise(resolve => setImmediate(resolve));
          expect(settled).toBe(true);
          await capture;
          const temporaryPath = mockDaemonCall.mock.calls.find(
            ([, method]) => method === 'finish'
          )?.[2].outputPath;
          expect(mockUnlinkSync).not.toHaveBeenCalled();
          expect(
            error.mock.calls.filter(
              ([message]) => message === 'Scroll capture retained at:'
            )
          ).toEqual(
            failure === 'publish'
              ? [['Scroll capture retained at:', temporaryPath]]
              : []
          );
          expect(mockShowNotification.mock.calls).toEqual(
            failure === 'publish'
              ? [
                  [
                    'Scroll Capture Not Saved',
                    `Capture data may be available at ${temporaryPath}`,
                  ],
                ]
              : []
          );
          vi.mocked(areaSelector.startAreaSelection).mockResolvedValueOnce(
            null
          );
          await startScrollCapture();
          expect(areaSelector.startAreaSelection).toHaveBeenCalledTimes(2);
        } finally {
          error.mockRestore();
        }
      }
    );

    it.each([
      ['success', 'finish-first'],
      ['failure', 'finish-first'],
      ['daemon-exit', 'finish-first'],
      ['success', 'cancel-first'],
      ['failure', 'cancel-first'],
      ['daemon-exit', 'cancel-first'],
    ])(
      'waits for finish and cancel before cleanup: %s %s',
      async (outcome, order) => {
        mockGetConfig.mockReturnValue({
          screenshot: {
            hideDesktopIcons: false,
            captureToClipboard: false,
            showPreview: true,
          },
          scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
        });
        mockExistsSync.mockReturnValue(true);
        const daemonModule = await import('@/main/daemon');
        const areaSelector = await import('@/main/capture/area-selector');
        let daemonHandler: ((event: string) => void) | null = null;
        let finishCapture: (result: unknown) => void = () => {};
        let failFinish: (error: Error) => void = () => {};
        let finishCancel: () => void = () => {};
        vi.mocked(daemonModule.daemon.onEvent).mockImplementation((handler => {
          daemonHandler = handler as typeof daemonHandler;
        }) as never);
        mockDaemonCall.mockImplementation((_module, method) => {
          if (method === 'finish') {
            return new Promise((resolve, reject) => {
              finishCapture = resolve;
              failFinish = reject;
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

        const { startScrollCapture } =
          await import('@/main/capture/scroll-capture');
        const capturing = startScrollCapture();
        await vi.waitFor(() =>
          expect(mockPrepareCapturePreview).toHaveBeenCalledTimes(1)
        );

        const action = mockShowScrollCaptureOverlay.mock.calls[0][1] as (
          action: 'cancel' | 'toggle-auto-scroll'
        ) => Promise<void>;
        await action('toggle-auto-scroll');
        expect(mockDaemonCall).not.toHaveBeenCalledWith(
          'scroll-capture',
          'startAutoScroll'
        );
        const cancellation = action('cancel');
        await new Promise(resolve => setImmediate(resolve));
        expect(mockDisposeCapturePreview).toHaveBeenCalledTimes(1);
        expect(mockDaemonCall).toHaveBeenCalledWith(
          'scroll-capture',
          'cancel',
          undefined,
          null
        );
        expect(mockUnlinkSync).not.toHaveBeenCalled();
        const second = startScrollCapture();
        expect(areaSelector.startAreaSelection).toHaveBeenCalledTimes(1);

        const error = vi.spyOn(console, 'error').mockImplementation(() => {});
        try {
          const settleFinish = () => {
            if (outcome === 'daemon-exit') {
              failFinish(new Error('Daemon exited before responding'));
            } else {
              finishCapture({ success: outcome === 'success' });
            }
          };
          const settleFirst =
            order === 'finish-first' ? settleFinish : finishCancel;
          const settleSecond =
            order === 'finish-first' ? finishCancel : settleFinish;
          settleFirst();
          await new Promise(resolve => setImmediate(resolve));
          expect(mockUnlinkSync).not.toHaveBeenCalled();
          expect(areaSelector.startAreaSelection).toHaveBeenCalledTimes(1);
          settleSecond();
          await Promise.all([cancellation, capturing, second]);
        } finally {
          error.mockRestore();
        }
        const temporaryPath = mockDaemonCall.mock.calls.find(
          ([, method]) => method === 'finish'
        )?.[2].outputPath;
        expect(temporaryPath).toMatch(/^\/p\/out\.png\..+\.tmp\.png$/);
        expect(mockPublishScreenshot).not.toHaveBeenCalled();
        expect(mockUnlinkSync).toHaveBeenCalledExactlyOnceWith(temporaryPath);
        expect(mockShowNotification).not.toHaveBeenCalled();
        expect(mockShowCapturePreview).not.toHaveBeenCalled();
        expect(mockDisposeCapturePreview).toHaveBeenCalledTimes(1);
        vi.mocked(areaSelector.startAreaSelection).mockResolvedValueOnce(null);
        await startScrollCapture();
        expect(areaSelector.startAreaSelection).toHaveBeenCalledTimes(2);
      }
    );

    it('waits for an already publishing capture before handling cancellation', async () => {
      const daemonModule = await import('@/main/daemon');
      const areaSelector = await import('@/main/capture/area-selector');
      let options:
        | { onSelected: (selection: unknown) => Promise<void> }
        | undefined;
      let daemonHandler: ((event: string) => Promise<void>) | undefined;
      let finishPublication: (path: string) => void = () => {};
      vi.mocked(areaSelector.startAreaSelection).mockImplementation((opts => {
        options = opts as typeof options;
        return new Promise<null>(() => {});
      }) as never);
      vi.mocked(daemonModule.daemon.onEvent).mockImplementation((handler => {
        daemonHandler = handler as typeof daemonHandler;
      }) as never);
      mockGetConfig.mockReturnValue({
        screenshot: { hideDesktopIcons: false, showPreview: true },
        scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
      });
      mockExistsSync.mockReturnValue(true);
      mockShowCapturePreview.mockReturnValue({ revealed: Promise.resolve() });
      mockDaemonCall.mockImplementation(async (_module, method) =>
        method === 'finish' ? { success: true } : {}
      );
      mockPublishScreenshot.mockReturnValueOnce(
        new Promise<string>(resolve => {
          finishPublication = resolve;
        })
      );

      const { startScrollCapture } =
        await import('@/main/capture/scroll-capture');
      const capturing = startScrollCapture();
      await options?.onSelected({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const finishing = daemonHandler?.('scroll-capture:done');
      await vi.waitFor(() => expect(mockPublishScreenshot).toHaveBeenCalled());
      const action = mockShowScrollCaptureOverlay.mock.calls[0][1] as (
        action: 'cancel'
      ) => Promise<void>;
      const cancellation = action('cancel');
      const second = startScrollCapture();
      await new Promise(resolve => setImmediate(resolve));
      expect(areaSelector.startAreaSelection).toHaveBeenCalledTimes(1);
      expect(mockDisposeCapturePreview).not.toHaveBeenCalled();
      expect(mockShowCapturePreview).not.toHaveBeenCalled();

      finishPublication('/p/out (1).png');
      await Promise.all([capturing, finishing, cancellation, second]);
      expect(mockShowCapturePreview).toHaveBeenCalledWith(
        '/p/out (1).png',
        'screenshot',
        undefined,
        expect.anything(),
        expect.any(Promise)
      );
      expect(mockDaemonCall).not.toHaveBeenCalledWith(
        'scroll-capture',
        'cancel'
      );
    });

    it.each(['save', 'cancel', 'daemon-exit', 'failed'])(
      'preserves the right files when a stitched capture ends: %s',
      async outcome => {
        const realFs = await vi.importActual<typeof FileSystem>('node:fs');
        const path = await import('node:path');
        const os = await import('node:os');
        const directory = realFs.mkdtempSync(
          path.join(os.tmpdir(), 'poratake-scroll-')
        );
        const outputPath = path.join(directory, 'capture.png');
        const prior = Buffer.from('existing capture');
        const captured = Buffer.from('new capture');
        let temporaryPath = '';
        realFs.writeFileSync(outputPath, prior);
        mockExistsSync.mockImplementation((file: string) =>
          realFs.existsSync(file)
        );
        mockLink.mockImplementation((from: string, to: string) =>
          realFs.promises.link(from, to)
        );
        mockCopyFile.mockImplementation(
          (from: string, to: string, flags: number) =>
            realFs.promises.copyFile(from, to, flags)
        );
        mockUnlink.mockImplementation((file: string) =>
          realFs.promises.unlink(file)
        );
        mockUnlinkSync.mockImplementation((file: string) =>
          realFs.unlinkSync(file)
        );
        const error = vi.spyOn(console, 'error').mockImplementation(() => {});

        try {
          const utils = await import('@/main/capture/screenshot/utils');
          const actualUtils = await vi.importActual<typeof ScreenshotUtils>(
            '@/main/capture/screenshot/utils'
          );
          mockPublishScreenshot.mockImplementation(
            actualUtils.publishScreenshot
          );
          vi.mocked(utils.generateScreenshotPath).mockReturnValueOnce(
            outputPath
          );
          const daemonModule = await import('@/main/daemon');
          const areaSelector = await import('@/main/capture/area-selector');
          let options:
            | { onSelected: (selection: unknown) => Promise<void> }
            | undefined;
          let daemonHandler: ((event: string) => Promise<void>) | undefined;
          let finishCapture: (result: unknown) => void = () => {};
          let failFinish: (error: Error) => void = () => {};
          vi.mocked(areaSelector.startAreaSelection).mockImplementation(
            (opts => {
              options = opts as typeof options;
              return new Promise<null>(() => {});
            }) as never
          );
          vi.mocked(daemonModule.daemon.onEvent).mockImplementation(
            (handler => {
              daemonHandler = handler as typeof daemonHandler;
            }) as never
          );
          mockGetConfig.mockReturnValue({
            screenshot: { hideDesktopIcons: false, showPreview: false },
            scrollCapture: { autoScrollSpeed: 'medium', maxHeight: 20000 },
          });
          mockDaemonCall.mockImplementation((_module, method, params) => {
            if (method === 'finish') {
              temporaryPath = params.outputPath;
              realFs.writeFileSync(temporaryPath, captured);
              return new Promise((resolve, reject) => {
                finishCapture = resolve;
                failFinish = reject;
              });
            }
            return Promise.resolve({});
          });

          const { startScrollCapture } =
            await import('@/main/capture/scroll-capture');
          const capture = startScrollCapture();
          await options?.onSelected({
            status: 'selected',
            x: 0,
            y: 0,
            width: 100,
            height: 100,
          });
          const finish = daemonHandler?.('scroll-capture:done');
          const action = mockShowScrollCaptureOverlay.mock.calls[0][1] as (
            action: 'cancel'
          ) => Promise<void>;
          const cancellation =
            outcome === 'cancel' ? action('cancel') : undefined;
          if (outcome === 'daemon-exit') {
            failFinish(new Error('Daemon exited before responding'));
          } else {
            finishCapture({ success: outcome !== 'failed' });
          }
          await Promise.all([capture, finish, cancellation]);

          expect(temporaryPath).not.toBe(outputPath);
          const retained = outcome === 'daemon-exit' || outcome === 'failed';
          expect(realFs.existsSync(temporaryPath)).toBe(retained);
          expect(
            realFs.existsSync(temporaryPath)
              ? realFs.readFileSync(temporaryPath)
              : null
          ).toEqual(retained ? captured : null);
          expect(realFs.readFileSync(outputPath)).toEqual(prior);
          const publishedPath = path.join(directory, 'capture (1).png');
          expect(realFs.existsSync(publishedPath)).toBe(outcome === 'save');
          expect(
            outcome === 'save' ? realFs.readFileSync(publishedPath) : null
          ).toEqual(outcome === 'save' ? captured : null);
          expect(mockAddToHistory.mock.calls).toEqual(
            outcome === 'save' ? [[publishedPath]] : []
          );
          expect(mockOpenScreenshotEditor.mock.calls).toEqual(
            outcome === 'save' ? [[publishedPath, 'h1']] : []
          );
          expect(mockShowNotification.mock.calls).toEqual(
            retained
              ? [
                  [
                    'Scroll Capture Not Saved',
                    `Capture data may be available at ${temporaryPath}`,
                  ],
                ]
              : []
          );
        } finally {
          error.mockRestore();
          mockExistsSync.mockReset();
          mockLink.mockReset();
          mockCopyFile.mockReset();
          mockUnlink.mockReset();
          mockUnlinkSync.mockReset();
          if (temporaryPath && realFs.existsSync(temporaryPath))
            realFs.unlinkSync(temporaryPath);
          if (realFs.existsSync(outputPath)) realFs.unlinkSync(outputPath);
          const publishedPath = path.join(directory, 'capture (1).png');
          if (realFs.existsSync(publishedPath))
            realFs.unlinkSync(publishedPath);
          realFs.rmdirSync(directory);
        }
      }
    );

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

      const cancellation = cancelScrollCapture();
      await Promise.resolve();
      expect(mockDisposeCapturePreview).toHaveBeenCalledTimes(1);
      expect(mockShowCapturePreview).not.toHaveBeenCalled();

      restoreIcons();
      await Promise.all([cancellation, capturing]);
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
