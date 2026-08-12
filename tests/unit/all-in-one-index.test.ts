import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockStartAreaSelection = vi.fn();
const mockCancelAreaSelection = vi.fn();
const mockUpdateAreaSelection = vi.fn();
const mockUpdateAreaSelectionCallbacks = vi.fn();
const mockShowAllInOneControl = vi.fn();
const mockUpdateAllInOnePosition = vi.fn();
const mockHideAllInOneControl = vi.fn();
const mockGetCurrentAreaSelection = vi.fn();
const mockSetAllInOneCallbacks = vi.fn();
const mockCaptureArea = vi.fn();
const mockShowPreRecordingControl = vi.fn();
const mockUpdateRecordingControlPosition = vi.fn();
const mockHidePreRecordingControl = vi.fn();
const mockPrewarmRecordingControl = vi.fn();
const mockPrewarmRecorder = vi.fn();
const mockPrewarmOverlay = vi.fn();
const mockGlobalShortcutRegister = vi.fn();
const mockGlobalShortcutUnregister = vi.fn();
const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockGetAllDisplays = vi.fn();
const mockIsFeatureSupported = vi.fn();
const mockSetAreaSelectorAspectRatio = vi.fn();
const mockCaptureText = vi.fn();
const mockClipboardWriteText = vi.fn();
const mockNotificationShow = vi.fn();
const mockSetOverlayToolbar = vi.fn();
const mockSetAreaSelectionMode = vi.fn();
const mockHideAreaSelector = vi.fn();
const mockHideRecordingOverlay = vi.fn();
const mockShowRecordedWindowOutline = vi.fn();
const mockIsScreenFrozen = vi.fn();
const mockIsFreezeScreenEnabled = vi.fn();

class MockNotification {
  show = mockNotificationShow;
}

vi.mock('electron', () => ({
  clipboard: { writeText: (...a: unknown[]) => mockClipboardWriteText(...a) },
  globalShortcut: {
    register: (key: string, cb: () => void) =>
      mockGlobalShortcutRegister(key, cb),
    unregister: (key: string) => mockGlobalShortcutUnregister(key),
  },
  screen: {
    getAllDisplays: () => mockGetAllDisplays(),
  },
  Notification: MockNotification,
}));

vi.mock('@/main/capture/area-selector', () => ({
  startAreaSelection: (...a: unknown[]) => mockStartAreaSelection(...a),
  cancelAreaSelection: (...a: unknown[]) => mockCancelAreaSelection(...a),
  hideAreaSelector: (...a: unknown[]) => mockHideAreaSelector(...a),
  updateAreaSelection: (...a: unknown[]) => mockUpdateAreaSelection(...a),
  updateAreaSelectionCallbacks: (...a: unknown[]) =>
    mockUpdateAreaSelectionCallbacks(...a),
  setAreaSelectionMode: (...a: unknown[]) => mockSetAreaSelectionMode(...a),
  setAreaSelectorAspectRatio: (...a: unknown[]) =>
    mockSetAreaSelectorAspectRatio(...a),
}));

vi.mock('@/main/capture/area-overlay', () => ({
  setOverlayToolbar: (...a: unknown[]) => mockSetOverlayToolbar(...a),
}));

vi.mock('@/main/capture/freeze-screen', () => ({
  isScreenFrozen: () => mockIsScreenFrozen(),
}));

vi.mock('@/main/capture/freeze-screen/preference', () => ({
  isFreezeScreenEnabled: () => mockIsFreezeScreenEnabled(),
}));

vi.mock('@/main/capture/all-in-one/open-all-in-one.ts', () => ({
  showAllInOneControl: (...a: unknown[]) => mockShowAllInOneControl(...a),
  updateAllInOnePosition: (...a: unknown[]) => mockUpdateAllInOnePosition(...a),
  hideAllInOneControl: (...a: unknown[]) => mockHideAllInOneControl(...a),
  getCurrentAreaSelection: () => mockGetCurrentAreaSelection(),
  setAllInOneCallbacks: (...a: unknown[]) => mockSetAllInOneCallbacks(...a),
}));

vi.mock('@/main/capture/screenshot/capture-area.ts', () => ({
  captureArea: (...a: unknown[]) => mockCaptureArea(...a),
}));

vi.mock('@/main/capture/ocr', () => ({
  default: (...a: unknown[]) => mockCaptureText(...a),
}));

vi.mock('@/main/capture/video/recording-control.ts', () => ({
  showPreRecordingControl: (...a: unknown[]) =>
    mockShowPreRecordingControl(...a),
  updateRecordingControlPosition: (...a: unknown[]) =>
    mockUpdateRecordingControlPosition(...a),
  hidePreRecordingControl: () => mockHidePreRecordingControl(),
  prewarmRecordingControlWindow: () => mockPrewarmRecordingControl(),
}));

vi.mock('@/main/capture/video/recorder.ts', () => ({
  prewarmRecorder: () => mockPrewarmRecorder(),
}));

vi.mock('@/main/capture/video/overlay.ts', () => ({
  prewarmOverlay: () => mockPrewarmOverlay(),
  hideRecordingOverlay: (...a: unknown[]) => mockHideRecordingOverlay(...a),
  showRecordedWindowOutline: (...a: unknown[]) =>
    mockShowRecordedWindowOutline(...a),
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
  updateConfig: (...a: unknown[]) => mockUpdateConfig(...a),
}));

vi.mock('@/main/system/capabilities', () => ({
  isFeatureSupported: (...a: unknown[]) => mockIsFeatureSupported(...a),
}));

describe('all-in-one orchestrator', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockCaptureArea.mockReset();
    mockCaptureText.mockReset();
    mockGetConfig.mockReturnValue({ allInOne: {} });
    mockUpdateAreaSelection.mockResolvedValue(true);
    mockIsFeatureSupported.mockReturnValue(true);
    mockIsScreenFrozen.mockReturnValue(false);
    mockIsFreezeScreenEnabled.mockReturnValue(true);
    mockGetAllDisplays.mockReturnValue([
      { bounds: { x: 0, y: 0, width: 1920, height: 1080 } },
    ]);
  });

  it('init registers all-in-one callbacks', async () => {
    const { init } = await import('@/main/capture/all-in-one');
    init();
    expect(mockSetAllInOneCallbacks).toHaveBeenCalled();
  });

  it('startAllInOne calls startAreaSelection', async () => {
    mockStartAreaSelection.mockResolvedValue({ status: 'confirmed' });
    const startAllInOne = (await import('@/main/capture/all-in-one')).default;
    await startAllInOne();
    expect(mockStartAreaSelection).toHaveBeenCalled();
  });

  it('starts without a preset even when a previous area exists', async () => {
    mockGetConfig.mockReturnValue({
      allInOne: { lastArea: { x: 100, y: 100, width: 200, height: 200 } },
    });
    mockStartAreaSelection.mockResolvedValue({ status: 'confirmed' });
    const startAllInOne = (await import('@/main/capture/all-in-one')).default;
    await startAllInOne();
    const [opts] = mockStartAreaSelection.mock.calls[0];
    expect(opts.preset).toBeUndefined();
  });

  it('cleans up when area selection returns null', async () => {
    mockStartAreaSelection.mockResolvedValue(null);
    const startAllInOne = (await import('@/main/capture/all-in-one')).default;
    await startAllInOne();
    expect(mockHideAllInOneControl).toHaveBeenCalled();
  });

  it('persists the final area and shows control when selected', async () => {
    mockStartAreaSelection.mockImplementation(
      async ({ onSelected }: { onSelected: (s: unknown) => void }) => {
        onSelected({
          status: 'selected',
          x: 10,
          y: 20,
          width: 100,
          height: 100,
        });
        return {
          status: 'confirmed',
          x: 10,
          y: 20,
          width: 100,
          height: 100,
        };
      }
    );
    const startAllInOne = (await import('@/main/capture/all-in-one')).default;
    await startAllInOne();
    expect(mockUpdateConfig).toHaveBeenCalledWith({
      allInOne: { lastArea: { x: 10, y: 20, width: 100, height: 100 } },
    });
    expect(mockShowAllInOneControl).toHaveBeenCalled();
    expect(mockGlobalShortcutRegister).toHaveBeenCalledTimes(3);
  });

  it('omits recording actions when recording is unsupported', async () => {
    mockIsFeatureSupported.mockImplementation(
      (feature: unknown) => feature === 'all-in-one'
    );
    mockStartAreaSelection.mockImplementation(
      async ({ onSelected }: { onSelected: (s: unknown) => void }) => {
        onSelected({
          status: 'selected',
          x: 10,
          y: 20,
          width: 100,
          height: 100,
        });
        return { status: 'selected' };
      }
    );

    const { default: startAllInOne, init } =
      await import('@/main/capture/all-in-one');
    await startAllInOne();
    init();
    const callbacks = mockSetAllInOneCallbacks.mock.calls.at(-1)?.[0] as {
      onRecord: () => void;
    };
    callbacks.onRecord();

    expect(mockGlobalShortcutRegister).toHaveBeenCalledTimes(2);
    expect(mockGlobalShortcutRegister).not.toHaveBeenCalledWith(
      'R',
      expect.any(Function)
    );
    expect(mockPrewarmRecorder).not.toHaveBeenCalled();
  });

  it('onUpdate forwards bounds to update', async () => {
    mockStartAreaSelection.mockImplementation(
      async ({ onUpdate }: { onUpdate: (s: unknown) => void }) => {
        onUpdate({
          status: 'updated',
          x: 50,
          y: 60,
          width: 200,
          height: 100,
        });
        return { status: 'selected' };
      }
    );
    const startAllInOne = (await import('@/main/capture/all-in-one')).default;
    await startAllInOne();
    expect(mockUpdateAllInOnePosition).toHaveBeenCalledWith({
      x: 50,
      y: 60,
      width: 200,
      height: 100,
    });
    expect(mockUpdateConfig).not.toHaveBeenCalled();
  });

  it('passes the overlay toolbar and its action handler to selection', async () => {
    mockStartAreaSelection.mockResolvedValue({ status: 'confirmed' });
    const startAllInOne = (await import('@/main/capture/all-in-one')).default;
    await startAllInOne();
    const [opts] = mockStartAreaSelection.mock.calls[0];
    expect(opts.toolbar).toEqual({
      kind: 'all-in-one',
      recordingEnabled: true,
      ocrEnabled: true,
      activeMode: 'screenshot',
      activeTarget: 'area',
    });
    expect(opts.freeze).toBe(process.platform === 'win32');
    expect(opts.onToolbarAction).toBeInstanceOf(Function);
  });

  it('runs the selected mode after a Windows area is drawn', async () => {
    const originalPlatform = process.platform;
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    mockGetCurrentAreaSelection.mockReturnValue({
      x: 10,
      y: 20,
      width: 100,
      height: 100,
    });
    mockStartAreaSelection.mockImplementation(
      async ({ onSelected, onToolbarAction }) => {
        onToolbarAction({ action: 'select-capture-mode', mode: 'record' });
        onSelected({
          status: 'selected',
          x: 10,
          y: 20,
          width: 100,
          height: 100,
        });
        return null;
      }
    );

    try {
      const startAllInOne = (await import('@/main/capture/all-in-one')).default;
      await startAllInOne();
      await Promise.resolve();

      expect(mockSetOverlayToolbar).toHaveBeenCalledWith(
        expect.objectContaining({ activeMode: 'record' })
      );
      expect(mockShowPreRecordingControl).toHaveBeenCalledWith(
        { x: 10, y: 20, width: 100, height: 100 },
        undefined
      );
      expect(mockCaptureArea).not.toHaveBeenCalled();
    } finally {
      Object.defineProperty(process, 'platform', { value: originalPlatform });
    }
  });

  it('hands a picked window to the recording control', async () => {
    const originalPlatform = process.platform;
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    mockGetCurrentAreaSelection.mockReturnValue({
      x: 200,
      y: 100,
      width: 800,
      height: 600,
    });
    mockStartAreaSelection.mockImplementation(
      async ({ onSelected, onToolbarAction }) => {
        onToolbarAction({ action: 'select-capture-mode', mode: 'record' });
        onToolbarAction({ action: 'select-capture-target', target: 'window' });
        onSelected({
          status: 'selected',
          x: 200,
          y: 100,
          width: 800,
          height: 600,
          windowId: 4242,
          windowName: 'Window',
        });
        return null;
      }
    );

    try {
      const startAllInOne = (await import('@/main/capture/all-in-one')).default;
      await startAllInOne();
      await Promise.resolve();

      expect(mockSetAreaSelectionMode).toHaveBeenCalledWith('window');
      expect(mockShowPreRecordingControl).toHaveBeenCalledWith(
        { x: 200, y: 100, width: 800, height: 600 },
        'Window'
      );
      expect(mockHideAreaSelector).toHaveBeenCalled();
      expect(mockShowRecordedWindowOutline).toHaveBeenCalledWith(4242);
    } finally {
      Object.defineProperty(process, 'platform', { value: originalPlatform });
    }
  });

  it('captures a picked window as a window screenshot', async () => {
    const originalPlatform = process.platform;
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    mockGetCurrentAreaSelection.mockReturnValue({
      x: 200,
      y: 100,
      width: 800,
      height: 600,
    });
    mockStartAreaSelection.mockImplementation(
      async ({ onSelected, onToolbarAction }) => {
        onToolbarAction({ action: 'select-capture-target', target: 'window' });
        onSelected({
          status: 'selected',
          x: 200,
          y: 100,
          width: 800,
          height: 600,
          windowId: 4242,
          windowName: 'Window',
        });
        return null;
      }
    );

    try {
      const startAllInOne = (await import('@/main/capture/all-in-one')).default;
      await startAllInOne();
      await new Promise(resolve => setTimeout(resolve, 0));

      expect(mockCaptureArea).toHaveBeenCalledWith(expect.anything(), {
        windowId: 4242,
      });
    } finally {
      Object.defineProperty(process, 'platform', { value: originalPlatform });
    }
  });

  describe('toolbar actions', () => {
    async function getToolbarHandler(): Promise<(action: unknown) => void> {
      mockStartAreaSelection.mockResolvedValue({ status: 'confirmed' });
      const startAllInOne = (await import('@/main/capture/all-in-one')).default;
      await startAllInOne();
      return mockStartAreaSelection.mock.calls[0][0].onToolbarAction;
    }

    it('screenshot action captures the current area', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 1,
        y: 2,
        width: 30,
        height: 40,
      });
      const handle = await getToolbarHandler();
      handle({ action: 'screenshot' });
      await new Promise(resolve => setImmediate(resolve));
      expect(mockCaptureArea).toHaveBeenCalled();
      expect(mockUpdateConfig).toHaveBeenCalledWith({
        allInOne: { lastArea: { x: 1, y: 2, width: 30, height: 40 } },
      });
    });

    it('captures from the native freeze before closing the selector', async () => {
      mockIsScreenFrozen.mockReturnValue(true);
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 1,
        y: 2,
        width: 30,
        height: 40,
      });
      mockCaptureArea.mockImplementation(async (_area, options) => {
        expect(mockCancelAreaSelection).not.toHaveBeenCalled();
        await options.onCaptured();
      });
      const handle = await getToolbarHandler();

      handle({ action: 'screenshot' });
      await new Promise(resolve => setImmediate(resolve));

      expect(mockCaptureArea).toHaveBeenCalledWith(expect.any(Object), {
        cached: true,
        onCaptured: expect.any(Function),
      });
      expect(mockCancelAreaSelection).toHaveBeenCalled();
    });

    it('record action starts the pre-recording flow', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 1,
        y: 2,
        width: 30,
        height: 40,
      });
      const handle = await getToolbarHandler();
      handle({ action: 'record' });
      expect(mockShowPreRecordingControl).toHaveBeenCalled();
      expect(mockUpdateConfig).toHaveBeenCalledWith({
        allInOne: { lastArea: { x: 1, y: 2, width: 30, height: 40 } },
      });
    });

    it('OCR action captures text from the current area', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 1,
        y: 2,
        width: 30,
        height: 40,
      });
      const handle = await getToolbarHandler();
      handle({ action: 'ocr' });
      await new Promise(resolve => setImmediate(resolve));
      expect(mockCaptureText).toHaveBeenCalledWith({
        x: 1,
        y: 2,
        width: 30,
        height: 40,
      });
      expect(mockCancelAreaSelection).toHaveBeenCalled();
      expect(mockHideAllInOneControl).toHaveBeenCalled();
    });

    it('copies a picked color and closes the overlay', async () => {
      const handle = await getToolbarHandler();
      handle({ action: 'copy-color', color: '#12abEF' });
      expect(mockClipboardWriteText).toHaveBeenCalledWith('#12abEF');
      expect(mockNotificationShow).toHaveBeenCalled();
      expect(mockCancelAreaSelection).toHaveBeenCalled();
      expect(mockHideAllInOneControl).toHaveBeenCalled();
    });

    it('switches capture mode across overlay windows', async () => {
      const handle = await getToolbarHandler();
      handle({ action: 'select-capture-mode', mode: 'ocr' });
      expect(mockSetOverlayToolbar).toHaveBeenCalledWith({
        kind: 'all-in-one',
        recordingEnabled: true,
        ocrEnabled: true,
        activeMode: 'ocr',
        activeTarget: 'area',
      });
    });

    it('switches the overlay to window picking for the active mode', async () => {
      const handle = await getToolbarHandler();
      handle({ action: 'select-capture-target', target: 'window' });

      expect(mockSetAreaSelectionMode).toHaveBeenCalledWith('window');
      expect(mockSetOverlayToolbar).toHaveBeenCalledWith(
        expect.objectContaining({
          activeMode: 'screenshot',
          activeTarget: 'window',
        })
      );
    });

    it('keeps a target per capture mode', async () => {
      const handle = await getToolbarHandler();
      handle({ action: 'select-capture-target', target: 'screen' });
      handle({ action: 'select-capture-mode', mode: 'record' });

      expect(mockSetAreaSelectionMode).toHaveBeenLastCalledWith('manual');
      expect(mockSetOverlayToolbar).toHaveBeenLastCalledWith(
        expect.objectContaining({ activeMode: 'record', activeTarget: 'area' })
      );

      handle({ action: 'select-capture-mode', mode: 'screenshot' });

      expect(mockSetAreaSelectionMode).toHaveBeenLastCalledWith('display');
      expect(mockSetOverlayToolbar).toHaveBeenLastCalledWith(
        expect.objectContaining({
          activeMode: 'screenshot',
          activeTarget: 'screen',
        })
      );
    });

    it('ignores a target change while capturing text', async () => {
      const handle = await getToolbarHandler();
      handle({ action: 'select-capture-mode', mode: 'ocr' });
      mockSetAreaSelectionMode.mockClear();
      handle({ action: 'select-capture-target', target: 'window' });

      expect(mockSetAreaSelectionMode).not.toHaveBeenCalled();
    });

    it('close action cancels the selection', async () => {
      const handle = await getToolbarHandler();
      handle({ action: 'close' });
      expect(mockCancelAreaSelection).toHaveBeenCalled();
      expect(mockHideAllInOneControl).toHaveBeenCalled();
    });

    it('aspect ratio action forwards to the area selector', async () => {
      const handle = await getToolbarHandler();
      handle({
        action: 'select-aspect-ratio',
        name: '16:9',
        width: 16,
        height: 9,
      });
      expect(mockSetAreaSelectorAspectRatio).toHaveBeenCalledWith({
        name: '16:9',
        width: 16,
        height: 9,
      });
    });

    it('update-size action resizes the selection', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 100,
        y: 100,
        width: 400,
        height: 200,
      });
      const handle = await getToolbarHandler();
      handle({ action: 'update-size', width: 200, height: 100 });
      await Promise.resolve();
      expect(mockUpdateAreaSelection).toHaveBeenCalledWith({
        x: 200,
        y: 150,
        width: 200,
        height: 100,
      });
    });

    it('size editor actions suspend and restore shortcuts', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const handle = await getToolbarHandler();
      const registrationsBefore = mockGlobalShortcutRegister.mock.calls.length;
      handle({ action: 'size-editor-opened' });
      expect(mockGlobalShortcutUnregister).toHaveBeenCalled();
      handle({ action: 'size-editor-closed' });
      expect(mockGlobalShortcutRegister.mock.calls.length).toBeGreaterThan(
        registrationsBefore
      );
    });
  });

  it('onCancelled unregisters shortcuts', async () => {
    mockStartAreaSelection.mockImplementation(
      async ({ onCancelled }: { onCancelled: () => void }) => {
        onCancelled();
        return null;
      }
    );
    const startAllInOne = (await import('@/main/capture/all-in-one')).default;
    await startAllInOne();
    expect(mockGlobalShortcutUnregister).toHaveBeenCalled();
  });

  describe('callbacks installed', () => {
    it('init installs handleScreenshotAction that captures', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 10,
        y: 20,
        width: 100,
        height: 100,
      });
      const { init } = await import('@/main/capture/all-in-one');
      init();
      const cbs = mockSetAllInOneCallbacks.mock.calls[0][0] as {
        onScreenshot: () => Promise<void>;
      };
      await cbs.onScreenshot();
      expect(mockCaptureArea).toHaveBeenCalled();
      expect(mockUpdateConfig).toHaveBeenCalledWith({
        allInOne: { lastArea: { x: 10, y: 20, width: 100, height: 100 } },
      });
    });

    it('handleScreenshotAction no-op when no area', async () => {
      mockGetCurrentAreaSelection.mockReturnValue(null);
      const { init } = await import('@/main/capture/all-in-one');
      init();
      const cbs = mockSetAllInOneCallbacks.mock.calls[0][0] as {
        onScreenshot: () => Promise<void>;
      };
      await cbs.onScreenshot();
      expect(mockCaptureArea).not.toHaveBeenCalled();
    });

    it('handleScreenshotAction swallows captureArea errors', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 0,
        y: 0,
        width: 10,
        height: 10,
      });
      mockCaptureArea.mockRejectedValue(new Error('boom'));
      const { init } = await import('@/main/capture/all-in-one');
      init();
      const cbs = mockSetAllInOneCallbacks.mock.calls[0][0] as {
        onScreenshot: () => Promise<void>;
      };
      await expect(cbs.onScreenshot()).resolves.toBeUndefined();
    });

    it('handleRecordAction starts pre-recording flow', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 10,
        y: 20,
        width: 100,
        height: 100,
      });
      const { init } = await import('@/main/capture/all-in-one');
      init();
      const cbs = mockSetAllInOneCallbacks.mock.calls[0][0] as {
        onRecord: () => void;
      };
      cbs.onRecord();
      expect(mockPrewarmRecordingControl).toHaveBeenCalled();
      expect(mockPrewarmRecorder).toHaveBeenCalled();
      expect(mockPrewarmOverlay).toHaveBeenCalled();
      expect(mockShowPreRecordingControl).toHaveBeenCalled();
      expect(mockUpdateAreaSelectionCallbacks).toHaveBeenCalled();
      expect(mockUpdateConfig).toHaveBeenCalledWith({
        allInOne: { lastArea: { x: 10, y: 20, width: 100, height: 100 } },
      });
    });

    it('handleRecordAction no-op when no area', async () => {
      mockGetCurrentAreaSelection.mockReturnValue(null);
      const { init } = await import('@/main/capture/all-in-one');
      init();
      const cbs = mockSetAllInOneCallbacks.mock.calls[0][0] as {
        onRecord: () => void;
      };
      cbs.onRecord();
      expect(mockPrewarmRecorder).not.toHaveBeenCalled();
    });

    it('handleUpdateSizeAction resizes the selector around current center', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 100,
        y: 100,
        width: 400,
        height: 200,
      });
      const { init } = await import('@/main/capture/all-in-one');
      init();
      const cbs = mockSetAllInOneCallbacks.mock.calls[0][0] as {
        onUpdateSize: (s: { width: number; height: number }) => Promise<void>;
      };
      await cbs.onUpdateSize({ width: 200, height: 100 });
      const expectedBounds = { x: 200, y: 150, width: 200, height: 100 };
      expect(mockUpdateAreaSelection).toHaveBeenCalledWith(expectedBounds);
      expect(mockUpdateConfig).toHaveBeenCalledWith({
        allInOne: { lastArea: expectedBounds },
      });
      expect(mockUpdateAllInOnePosition).toHaveBeenCalledWith(expectedBounds);
    });

    it('handleUpdateSizeAction clamps size inside the active display', async () => {
      mockGetAllDisplays.mockReturnValue([
        { bounds: { x: 0, y: 0, width: 1000, height: 800 } },
      ]);
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 900,
        y: 700,
        width: 80,
        height: 80,
      });
      const { init } = await import('@/main/capture/all-in-one');
      init();
      const cbs = mockSetAllInOneCallbacks.mock.calls[0][0] as {
        onUpdateSize: (s: { width: number; height: number }) => Promise<void>;
      };
      await cbs.onUpdateSize({ width: 300, height: 300 });
      expect(mockUpdateAreaSelection).toHaveBeenCalledWith({
        x: 700,
        y: 500,
        width: 300,
        height: 300,
      });
    });

    it('handleUpdateSizeAction no-ops without current area', async () => {
      mockGetCurrentAreaSelection.mockReturnValue(null);
      const { init } = await import('@/main/capture/all-in-one');
      init();
      const cbs = mockSetAllInOneCallbacks.mock.calls[0][0] as {
        onUpdateSize: (s: { width: number; height: number }) => Promise<void>;
      };
      await cbs.onUpdateSize({ width: 200, height: 100 });
      expect(mockUpdateAreaSelection).not.toHaveBeenCalled();
    });

    it('handleUpdateSizeAction stops when selector update fails', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 100,
        y: 100,
        width: 400,
        height: 200,
      });
      mockUpdateAreaSelection.mockResolvedValue(false);
      const { init } = await import('@/main/capture/all-in-one');
      init();
      const cbs = mockSetAllInOneCallbacks.mock.calls[0][0] as {
        onUpdateSize: (s: { width: number; height: number }) => Promise<void>;
      };
      await cbs.onUpdateSize({ width: 200, height: 100 });
      expect(mockUpdateConfig).not.toHaveBeenCalled();
      expect(mockUpdateAllInOnePosition).not.toHaveBeenCalled();
    });

    it('size editor callbacks suspend and restore shortcuts', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const { init } = await import('@/main/capture/all-in-one');
      init();
      const cbs = mockSetAllInOneCallbacks.mock.calls[0][0] as {
        onSizeEditorOpened: () => void;
        onSizeEditorClosed: () => void;
      };
      cbs.onSizeEditorOpened();
      cbs.onSizeEditorClosed();
      expect(mockGlobalShortcutUnregister).toHaveBeenCalled();
      expect(mockGlobalShortcutRegister).toHaveBeenCalledTimes(3);
    });

    it('record callback onUpdate updates control position', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const { init } = await import('@/main/capture/all-in-one');
      init();
      const cbs = mockSetAllInOneCallbacks.mock.calls[0][0] as {
        onRecord: () => void;
      };
      cbs.onRecord();
      const updateCallbacks = mockUpdateAreaSelectionCallbacks.mock
        .calls[0][0] as {
        onUpdate: (s: unknown) => void;
        onCancelled: () => void;
      };
      updateCallbacks.onUpdate({
        status: 'updated',
        x: 50,
        y: 60,
        width: 200,
        height: 100,
      });
      expect(mockUpdateRecordingControlPosition).toHaveBeenCalledWith({
        x: 50,
        y: 60,
        width: 200,
        height: 100,
      });
      updateCallbacks.onCancelled();
      expect(mockHidePreRecordingControl).toHaveBeenCalled();
    });

    it('handleCloseAction unregisters shortcuts and cancels selection', async () => {
      const { init } = await import('@/main/capture/all-in-one');
      init();
      const cbs = mockSetAllInOneCallbacks.mock.calls[0][0] as {
        onClose: () => void;
      };
      cbs.onClose();
      expect(mockCancelAreaSelection).toHaveBeenCalled();
      expect(mockHideAllInOneControl).toHaveBeenCalled();
    });
  });

  it('renders every all-in-one toolbar action', async () => {
    const React = await import('react');
    vi.stubGlobal('React', React);
    vi.stubGlobal('window', { EyeDropper: class {} });
    const { renderToStaticMarkup } = await import('react-dom/server');
    const { default: AllInOneToolbar } =
      await import('@/renderer/components/area-overlay/all-in-one-toolbar');

    const markup = renderToStaticMarkup(
      React.createElement(AllInOneToolbar, {
        recordingEnabled: true,
        ocrEnabled: true,
        activeMode: 'screenshot',
        onAction: vi.fn(),
        onPickingColorChange: vi.fn(),
      })
    );

    expect(markup).toContain('aria-label="Capture text"');
    expect(markup).toContain('aria-label="Pick color"');
    expect(markup).toContain('aria-label="Close"');
    expect(
      markup.match(/--button-fg:rgb\(255 255 255 \/ 0\.85\)/g)
    ).toHaveLength(3);
    expect(markup).toContain('rounded-4xl border-2');
    expect(markup.match(/size-8 min-w-8 rounded-3xl/g)).toHaveLength(3);
    expect(markup).not.toContain('tooltip__trigger');
  }, 30000);
});
