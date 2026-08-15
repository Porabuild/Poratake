import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockStartAreaSelection = vi.fn();
const mockCancelAreaSelection = vi.fn();
const mockUpdateAreaSelectionCallbacks = vi.fn();
const mockShowAllInOneControl = vi.fn();
const mockUpdateAllInOnePosition = vi.fn();
const mockHideAllInOneControl = vi.fn();
const mockGetCurrentAreaSelection = vi.fn();
const mockCaptureArea = vi.fn();
const mockShowPreRecordingControl = vi.fn();
const mockUpdateRecordingControlPosition = vi.fn();
const mockHidePreRecordingControl = vi.fn();
const mockPrewarmRecordingControl = vi.fn();
const mockPrewarmRecorder = vi.fn();
const mockPrewarmOverlay = vi.fn();
const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockIsFeatureSupported = vi.fn();
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
  Notification: MockNotification,
}));

vi.mock('@/main/capture/area-selector', () => ({
  startAreaSelection: (...a: unknown[]) => mockStartAreaSelection(...a),
  cancelAreaSelection: (...a: unknown[]) => mockCancelAreaSelection(...a),
  hideAreaSelector: (...a: unknown[]) => mockHideAreaSelector(...a),
  updateAreaSelectionCallbacks: (...a: unknown[]) =>
    mockUpdateAreaSelectionCallbacks(...a),
  setAreaSelectionMode: (...a: unknown[]) => mockSetAreaSelectionMode(...a),
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
    mockIsFeatureSupported.mockReturnValue(true);
    mockIsScreenFrozen.mockReturnValue(false);
    mockIsFreezeScreenEnabled.mockReturnValue(true);
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
  });

  it('omits recording actions when recording is unsupported', async () => {
    mockIsFeatureSupported.mockImplementation(
      (feature: unknown) => feature === 'all-in-one'
    );
    mockGetCurrentAreaSelection.mockReturnValue({
      x: 10,
      y: 20,
      width: 100,
      height: 100,
    });
    mockStartAreaSelection.mockResolvedValue({ status: 'confirmed' });

    const startAllInOne = (await import('@/main/capture/all-in-one')).default;
    await startAllInOne();
    const handle = mockStartAreaSelection.mock.calls[0][0].onToolbarAction;
    handle({ action: 'record' });

    expect(mockShowPreRecordingControl).not.toHaveBeenCalled();
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
    expect(opts.freeze).toBe(true);
    expect(opts.onToolbarAction).toBeInstanceOf(Function);
  });

  it('runs the selected mode after an area is drawn', async () => {
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
  });

  it('hands a picked window to the recording control', async () => {
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
  });

  it('captures a picked window as a window screenshot', async () => {
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

    const startAllInOne = (await import('@/main/capture/all-in-one')).default;
    await startAllInOne();
    await new Promise(resolve => setTimeout(resolve, 0));

    expect(mockCaptureArea).toHaveBeenCalledWith(expect.anything(), {
      windowId: 4242,
    });
  });

  describe('toolbar actions', () => {
    async function getToolbarHandler(): Promise<{
      onToolbarAction: (action: unknown) => void;
      onSelected: (selection: unknown) => void;
    }> {
      const handlers: {
        onToolbarAction?: (action: unknown) => void;
        onSelected?: (selection: unknown) => void;
      } = {};
      mockStartAreaSelection.mockImplementation(
        async (options: {
          onToolbarAction: (action: unknown) => void;
          onSelected: (selection: unknown) => void;
        }) => {
          handlers.onToolbarAction = options.onToolbarAction;
          handlers.onSelected = options.onSelected;
          return { status: 'confirmed' };
        }
      );
      const startAllInOne = (await import('@/main/capture/all-in-one')).default;
      await startAllInOne();
      return handlers as {
        onToolbarAction: (action: unknown) => void;
        onSelected: (selection: unknown) => void;
      };
    }

    it('screenshot mode captures the current area after selection', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 1,
        y: 2,
        width: 30,
        height: 40,
      });
      const { onSelected } = await getToolbarHandler();
      onSelected({ status: 'selected', x: 1, y: 2, width: 30, height: 40 });
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
      const { onSelected } = await getToolbarHandler();

      onSelected({ status: 'selected', x: 1, y: 2, width: 30, height: 40 });
      await new Promise(resolve => setImmediate(resolve));

      expect(mockCaptureArea).toHaveBeenCalledWith(expect.any(Object), {
        cached: true,
        onCaptured: expect.any(Function),
      });
      expect(mockCancelAreaSelection).toHaveBeenCalled();
    });

    it('OCR mode captures text from the current area', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 1,
        y: 2,
        width: 30,
        height: 40,
      });
      const { onSelected, onToolbarAction } = await getToolbarHandler();
      onToolbarAction({ action: 'select-capture-mode', mode: 'ocr' });
      onSelected({ status: 'selected', x: 1, y: 2, width: 30, height: 40 });
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
      const { onToolbarAction } = await getToolbarHandler();
      onToolbarAction({ action: 'copy-color', color: '#12abEF' });
      expect(mockClipboardWriteText).toHaveBeenCalledWith('#12abEF');
      expect(mockNotificationShow).toHaveBeenCalled();
      expect(mockCancelAreaSelection).toHaveBeenCalled();
      expect(mockHideAllInOneControl).toHaveBeenCalled();
    });

    it('switches capture mode across overlay windows', async () => {
      const { onToolbarAction } = await getToolbarHandler();
      onToolbarAction({ action: 'select-capture-mode', mode: 'ocr' });
      expect(mockSetOverlayToolbar).toHaveBeenCalledWith({
        kind: 'all-in-one',
        recordingEnabled: true,
        ocrEnabled: true,
        activeMode: 'ocr',
        activeTarget: 'area',
      });
    });

    it('switches the overlay to window picking for the active mode', async () => {
      const { onToolbarAction } = await getToolbarHandler();
      onToolbarAction({ action: 'select-capture-target', target: 'window' });

      expect(mockSetAreaSelectionMode).toHaveBeenCalledWith('window');
      expect(mockSetOverlayToolbar).toHaveBeenCalledWith(
        expect.objectContaining({
          activeMode: 'screenshot',
          activeTarget: 'window',
        })
      );
    });

    it('keeps a target per capture mode', async () => {
      const { onToolbarAction } = await getToolbarHandler();
      onToolbarAction({ action: 'select-capture-target', target: 'screen' });
      onToolbarAction({ action: 'select-capture-mode', mode: 'record' });

      expect(mockSetAreaSelectionMode).toHaveBeenLastCalledWith('manual');
      expect(mockSetOverlayToolbar).toHaveBeenLastCalledWith(
        expect.objectContaining({ activeMode: 'record', activeTarget: 'area' })
      );

      onToolbarAction({ action: 'select-capture-mode', mode: 'screenshot' });

      expect(mockSetAreaSelectionMode).toHaveBeenLastCalledWith('display');
      expect(mockSetOverlayToolbar).toHaveBeenLastCalledWith(
        expect.objectContaining({
          activeMode: 'screenshot',
          activeTarget: 'screen',
        })
      );
    });

    it('ignores a target change while capturing text', async () => {
      const { onToolbarAction } = await getToolbarHandler();
      onToolbarAction({ action: 'select-capture-mode', mode: 'ocr' });
      mockSetAreaSelectionMode.mockClear();
      onToolbarAction({ action: 'select-capture-target', target: 'window' });

      expect(mockSetAreaSelectionMode).not.toHaveBeenCalled();
    });

    it('close action cancels the selection', async () => {
      const { onToolbarAction } = await getToolbarHandler();
      onToolbarAction({ action: 'close' });
      expect(mockCancelAreaSelection).toHaveBeenCalled();
      expect(mockHideAllInOneControl).toHaveBeenCalled();
    });

    it('screenshot mode swallows capture errors', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 0,
        y: 0,
        width: 10,
        height: 10,
      });
      mockCaptureArea.mockRejectedValue(new Error('boom'));
      const { onSelected } = await getToolbarHandler();
      onSelected({ status: 'selected', x: 0, y: 0, width: 10, height: 10 });
      await new Promise(resolve => setImmediate(resolve));
      expect(mockCaptureArea).toHaveBeenCalled();
    });

    it('record mode wires selection callbacks to the control', async () => {
      mockGetCurrentAreaSelection.mockReturnValue({
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const { onSelected, onToolbarAction } = await getToolbarHandler();
      onToolbarAction({ action: 'select-capture-mode', mode: 'record' });
      onSelected({ status: 'selected', x: 0, y: 0, width: 100, height: 100 });
      await Promise.resolve();
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
