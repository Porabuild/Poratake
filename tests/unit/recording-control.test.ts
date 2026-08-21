import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockDaemonCall = vi.fn();
const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockShowCameraPreview = vi.fn();
const mockUpdateCameraPreviewPosition = vi.fn();
const mockHideCameraPreview = vi.fn();
const mockGetCameraPreviewSettings = vi.fn();
const mockEnableCameraContentProtection = vi.fn();

const RECORDING_CONFIG = {
  includeAudio: true,
  micEnabled: false,
  micDeviceId: null,
  micDeviceName: null,
  cameraEnabled: false,
  cameraDeviceId: null,
  cameraDeviceName: null,
  frameRate: 60,
  outputPath: '/tmp/project',
};
const mockGetDisplayMatching = vi.fn();
const mockGetCursorScreenPoint = vi.fn();
const mockGetDisplayNearestPoint = vi.fn();
const mockGlobalShortcutRegister = vi.fn(() => true);
const mockGlobalShortcutUnregister = vi.fn();
const mockGetBrowserWindowWidth = vi.fn((mode: string) =>
  mode === 'recording' ? 400 : 236
);
const mockHideBrowserWindow = vi.fn();
const mockPrewarmBrowserWindow = vi.fn();
const mockShowBrowserWindow = vi.fn();
const mockUpdateBrowserWindow = vi.fn();
const mockUpdateBrowserWindowPosition = vi.fn();
const mockClearBrowserWindowParent = vi.fn();

vi.mock('electron', () => ({
  dialog: { showMessageBox: (...a: unknown[]) => mockShowMessageBox(...a) },
  BrowserWindow: { getFocusedWindow: vi.fn(() => null) },
  globalShortcut: {
    register: (...a: unknown[]) => mockGlobalShortcutRegister(...a),
    unregister: (...a: unknown[]) => mockGlobalShortcutUnregister(...a),
  },
  screen: {
    getDisplayMatching: (...a: unknown[]) => mockGetDisplayMatching(...a),
    getCursorScreenPoint: () => mockGetCursorScreenPoint(),
    getDisplayNearestPoint: (...a: unknown[]) =>
      mockGetDisplayNearestPoint(...a),
  },
}));

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...a: unknown[]) => mockDaemonCall(...a),
  },
}));

vi.mock('@/main/capture/video/recording-control-window', () => ({
  clearRecordingControlBrowserWindowParent: () =>
    mockClearBrowserWindowParent(),
  getRecordingControlWindowWidth: (...a: unknown[]) =>
    mockGetBrowserWindowWidth(...a),
  hideRecordingControlBrowserWindow: () => mockHideBrowserWindow(),
  prewarmRecordingControlBrowserWindow: () => mockPrewarmBrowserWindow(),
  showRecordingControlBrowserWindow: (...a: unknown[]) =>
    mockShowBrowserWindow(...a),
  updateRecordingControlBrowserWindow: (...a: unknown[]) =>
    mockUpdateBrowserWindow(...a),
  updateRecordingControlBrowserWindowPosition: (...a: unknown[]) =>
    mockUpdateBrowserWindowPosition(...a),
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
  updateConfig: (...a: unknown[]) => mockUpdateConfig(...a),
}));

vi.mock('@/main/capture/video/camera-preview', () => ({
  showCameraPreview: (...a: unknown[]) => mockShowCameraPreview(...a),
  updateCameraPreviewPosition: (...a: unknown[]) =>
    mockUpdateCameraPreviewPosition(...a),
  hideCameraPreview: () => mockHideCameraPreview(),
  getCameraPreviewSettings: () => mockGetCameraPreviewSettings(),
  enableCameraContentProtection: () => mockEnableCameraContentProtection(),
}));

const mockStartPendingRecording = vi.fn();
const mockCancelPendingRecording = vi.fn();
const mockStopRecordingAction = vi.fn();
const mockDeleteRecordingAction = vi.fn();

const mockPauseRecording = vi.fn();
const mockResumeRecording = vi.fn();
vi.mock('@/main/capture/video/recorder', () => ({
  pauseRecording: () => mockPauseRecording(),
  resumeRecording: () => mockResumeRecording(),
}));

const mockCheckMic = vi.fn();
const mockCheckCamera = vi.fn();
const mockShowRecordingError = vi.fn();
vi.mock('@/main/capture/video/permissions', () => ({
  checkAndRequestMicrophonePermission: () => mockCheckMic(),
  checkAndRequestCameraPermission: () => mockCheckCamera(),
  showRecordingError: (...a: unknown[]) => mockShowRecordingError(...a),
}));

const mockSetAspectRatio = vi.fn();
const mockHideAreaSelector = vi.fn();
const mockShowAreaSelector = vi.fn();
vi.mock('@/main/capture/area-selector', () => ({
  setAreaSelectorAspectRatio: (...a: unknown[]) => mockSetAspectRatio(...a),
  hideAreaSelector: () => mockHideAreaSelector(),
  showAreaSelector: () => mockShowAreaSelector(),
}));

const mockShowMessageBox = vi.fn();

type ActionHandler = (action: string, data?: unknown) => void;

function flush(): Promise<void> {
  return new Promise(resolve => setImmediate(resolve));
}

function lastActionHandler(): ActionHandler {
  const calls = mockShowBrowserWindow.mock.calls;
  return calls[calls.length - 1][2] as ActionHandler;
}

describe('recording-control', () => {
  const originalPlatform = process.platform;

  beforeEach(async () => {
    Object.defineProperty(process, 'platform', { value: 'darwin' });
    vi.clearAllMocks();
    vi.resetModules();
    mockUpdateConfig.mockReset();
    mockDaemonCall.mockResolvedValue({});
    mockCancelPendingRecording.mockResolvedValue(undefined);
    mockStartPendingRecording.mockResolvedValue(undefined);
    mockStopRecordingAction.mockResolvedValue(undefined);
    mockDeleteRecordingAction.mockResolvedValue(undefined);
    mockPauseRecording.mockResolvedValue(undefined);
    mockResumeRecording.mockResolvedValue(undefined);
    mockShowCameraPreview.mockResolvedValue(undefined);
    mockEnableCameraContentProtection.mockResolvedValue(undefined);
    mockUpdateCameraPreviewPosition.mockResolvedValue(undefined);
    mockShowRecordingError.mockResolvedValue(undefined);
    mockShowMessageBox.mockResolvedValue({ response: 1 });
    mockCheckCamera.mockResolvedValue(true);
    mockCheckMic.mockResolvedValue(true);
    mockGetCameraPreviewSettings.mockReturnValue(null);
    mockGetDisplayMatching.mockReturnValue({
      workArea: { x: 0, y: 0, width: 1920, height: 1080 },
    });
    mockGetCursorScreenPoint.mockReturnValue({ x: 500, y: 500 });
    mockGetDisplayNearestPoint.mockReturnValue({
      workArea: { x: 0, y: 0, width: 1920, height: 1080 },
    });
    mockGetConfig.mockReturnValue({
      recording: {
        systemAudio: true,
        micEnabled: false,
        camera: { enabled: false },
      },
    });
    const recordingControl =
      await import('@/main/capture/video/recording-control');
    recordingControl.setRecordingControlActions({
      startPendingRecording: (...args) => mockStartPendingRecording(...args),
      cancelPendingRecording: () => mockCancelPendingRecording(),
      stopRecordingAction: () => mockStopRecordingAction(),
      deleteRecordingAction: () => mockDeleteRecordingAction(),
    });
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
    vi.resetModules();
  });

  it('names the picked window on the control bar', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl(
      { x: 100, y: 100, width: 800, height: 600 },
      'Inbox — Chrome'
    );

    expect(mockShowBrowserWindow).toHaveBeenCalledWith(
      expect.objectContaining({ targetName: 'Inbox — Chrome' }),
      expect.anything(),
      expect.any(Function)
    );
  });

  it('leaves the control bar unnamed for an area selection', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl({ x: 100, y: 100, width: 800, height: 600 });

    expect(mockShowBrowserWindow).toHaveBeenCalledWith(
      expect.objectContaining({ targetName: null }),
      expect.anything(),
      expect.any(Function)
    );
  });

  it('shows the Electron toolbar for the macOS pre-recording panel', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl({ x: 100, y: 100, width: 800, height: 600 });

    expect(mockShowBrowserWindow).toHaveBeenCalledWith(
      expect.objectContaining({ mode: 'pre-recording' }),
      { x: 842, y: 24 },
      expect.any(Function)
    );
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it('handles microphone selection from the toolbar menu', async () => {
    mockCheckMic.mockResolvedValue(true);
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl();

    lastActionHandler()('select-mic', {
      deviceId: 'mic-1',
      deviceName: 'Microphone 1',
    });

    await vi.waitFor(() =>
      expect(mockUpdateConfig).toHaveBeenCalledWith({
        recording: expect.objectContaining({
          micEnabled: true,
          selectedMicId: 'mic-1',
          selectedMicName: 'Microphone 1',
        }),
      })
    );
  });

  it('anchors the control to the top centre of the selected display', async () => {
    mockGetDisplayMatching.mockReturnValue({
      workArea: { x: 1920, y: 40, width: 1920, height: 1040 },
    });

    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl({ x: 2000, y: 600, width: 800, height: 600 });

    expect(mockShowBrowserWindow).toHaveBeenCalledWith(
      expect.objectContaining({ mode: 'pre-recording' }),
      { x: 2762, y: 64 },
      expect.any(Function)
    );
  });

  it('keeps the toolbar centered when recording starts', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl({ x: 100, y: 100, width: 800, height: 600 });

    await m.showRecordingControl(RECORDING_CONFIG);

    expect(mockShowBrowserWindow).toHaveBeenLastCalledWith(
      expect.objectContaining({ mode: 'recording' }),
      { x: 760, y: 24 },
      expect.any(Function)
    );
    await m.hideRecordingControl();
  });

  it('detaches the toolbar from the overlay window on every platform', async () => {
    const m = await import('@/main/capture/video/recording-control');

    m.detachRecordingControlFromOverlay();

    expect(mockClearBrowserWindowParent).toHaveBeenCalledTimes(1);
  });

  it('routes renderer actions through the recording controller', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl();
    mockUpdateConfig.mockImplementation(update => {
      mockGetConfig.mockReturnValue(update);
    });

    lastActionHandler()('toggle-system-audio');

    await vi.waitFor(() => expect(mockUpdateConfig).toHaveBeenCalled());
    expect(mockUpdateBrowserWindow).toHaveBeenCalledWith(
      expect.objectContaining({ systemAudio: false })
    );
  });

  it('discards straight away from the toolbar', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl();

    lastActionHandler()('delete');

    await vi.waitFor(() =>
      expect(mockDeleteRecordingAction).toHaveBeenCalledTimes(1)
    );
  });

  it('showPreRecordingControl shows camera preview when enabled', async () => {
    mockGetConfig.mockReturnValue({
      recording: {
        systemAudio: true,
        micEnabled: false,
        camera: { enabled: true, selectedDeviceId: 'cam-1' },
      },
    });
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl({ x: 0, y: 0, width: 100, height: 100 });
    expect(mockShowCameraPreview).toHaveBeenCalled();
  });

  it('places the camera inside the selected area', async () => {
    mockGetConfig.mockReturnValue({
      recording: {
        systemAudio: true,
        micEnabled: false,
        camera: {
          enabled: true,
          selectedDeviceId: 'cam-1',
          position: { x: 10, y: 20 },
        },
      },
    });
    mockGetDisplayMatching.mockReturnValue({
      workArea: { x: 1920, y: 0, width: 1920, height: 1080 },
    });
    const m = await import('@/main/capture/video/recording-control');

    m.showPreRecordingControl({ x: 2000, y: 100, width: 800, height: 600 });

    expect(mockShowCameraPreview).toHaveBeenCalledWith(
      expect.objectContaining({ position: { x: 2498, y: 398 } })
    );
  });

  it('showPreRecordingControl without an area follows the mouse display', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl();
    expect(mockShowBrowserWindow).toHaveBeenCalledWith(
      expect.anything(),
      { x: 842, y: 24 },
      expect.any(Function)
    );
  });

  it('updateRecordingControlPosition moves the Electron toolbar', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.updateRecordingControlPosition({
      x: 200,
      y: 200,
      width: 100,
      height: 100,
    });
    expect(mockUpdateBrowserWindowPosition).toHaveBeenCalledWith({
      x: 842,
      y: 24,
    });
  });

  it('keeps the camera inside an updated selected area', async () => {
    mockGetConfig.mockReturnValue({
      recording: {
        systemAudio: true,
        micEnabled: false,
        camera: { enabled: true },
      },
    });
    const m = await import('@/main/capture/video/recording-control');

    m.updateRecordingControlPosition({
      x: 300,
      y: 200,
      width: 900,
      height: 700,
    });

    expect(mockUpdateCameraPreviewPosition).toHaveBeenCalledWith({
      x: 898,
      y: 598,
    });
  });

  it('hidePreRecordingControl hides the toolbar and camera by default', async () => {
    const m = await import('@/main/capture/video/recording-control');
    await m.hidePreRecordingControl();
    expect(mockHideBrowserWindow).toHaveBeenCalled();
    expect(mockHideCameraPreview).toHaveBeenCalled();
  });

  it('hidePreRecordingControl with hideCamera=false keeps the toolbar', async () => {
    const m = await import('@/main/capture/video/recording-control');
    await m.hidePreRecordingControl(false);
    expect(mockHideCameraPreview).not.toHaveBeenCalled();
    expect(mockHideBrowserWindow).not.toHaveBeenCalled();
  });

  it('showRecordingControl switches the toolbar to recording mode', async () => {
    const m = await import('@/main/capture/video/recording-control');
    await m.showRecordingControl(RECORDING_CONFIG);
    expect(mockShowBrowserWindow).toHaveBeenLastCalledWith(
      expect.objectContaining({ mode: 'recording' }),
      expect.anything(),
      expect.any(Function)
    );
    await m.hideRecordingControl();
  });

  it('hideRecordingControl stops the timer and hides the toolbar', async () => {
    const m = await import('@/main/capture/video/recording-control');
    await m.showRecordingControl(RECORDING_CONFIG);
    mockHideBrowserWindow.mockClear();

    await m.hideRecordingControl();

    expect(mockHideBrowserWindow).toHaveBeenCalled();
  });

  it('prewarmRecordingControlWindow warms the Electron toolbar', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.prewarmRecordingControlWindow();
    expect(mockPrewarmBrowserWindow).toHaveBeenCalledTimes(1);
  });

  it('pauseTimer is no-op when not recording', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.pauseTimer();
    expect(mockUpdateBrowserWindow).not.toHaveBeenCalled();
  });

  it('resumeTimer is no-op when not paused', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.resumeTimer();
    expect(mockUpdateBrowserWindow).not.toHaveBeenCalled();
  });

  describe('event handlers', () => {
    async function setupAndFire(action: string, data?: unknown): Promise<void> {
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      lastActionHandler()(action, data);
      await flush();
    }

    it('toggle-system-audio flips config', async () => {
      await setupAndFire('toggle-system-audio');
      expect(mockUpdateConfig).toHaveBeenCalled();
    });

    it('toggle-mic enables mic when permission granted', async () => {
      mockCheckMic.mockResolvedValue(true);
      await setupAndFire('toggle-mic');
      expect(mockUpdateConfig).toHaveBeenCalled();
    });

    it('toggle-mic aborts when permission denied', async () => {
      mockGetConfig.mockReturnValue({
        recording: { micEnabled: false, systemAudio: true, camera: null },
      });
      mockCheckMic.mockResolvedValue(false);
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      mockUpdateConfig.mockClear();
      lastActionHandler()('toggle-mic');
      await flush();
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });

    it('select-mic with deviceId updates config', async () => {
      mockCheckMic.mockResolvedValue(true);
      await setupAndFire('select-mic', {
        deviceId: 'mic-1',
        deviceName: 'Mic 1',
      });
      expect(mockUpdateConfig).toHaveBeenCalled();
    });

    it('select-mic with null deviceId enables the system default microphone', async () => {
      await setupAndFire('select-mic', {
        deviceId: null,
        deviceName: null,
      });
      expect(mockUpdateConfig).toHaveBeenCalledWith({
        recording: expect.objectContaining({
          micEnabled: true,
          selectedMicId: null,
          selectedMicName: null,
        }),
      });
    });

    it('toggle-camera enables camera when permission granted', async () => {
      mockGetConfig.mockReturnValue({
        recording: {
          systemAudio: true,
          micEnabled: false,
          camera: { enabled: false },
        },
      });
      mockCheckCamera.mockResolvedValue(true);
      await setupAndFire('toggle-camera');
      expect(mockShowCameraPreview).toHaveBeenCalled();
    });

    it('keeps camera disabled and reports a preview start failure', async () => {
      mockGetConfig.mockReturnValue({
        recording: {
          systemAudio: true,
          micEnabled: false,
          camera: { enabled: false },
        },
      });
      mockShowCameraPreview.mockRejectedValue(new Error('camera failed'));
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      mockUpdateConfig.mockClear();
      lastActionHandler()('toggle-camera');

      await vi.waitFor(() =>
        expect(mockShowRecordingError).toHaveBeenCalledWith(
          expect.objectContaining({ message: 'camera failed' })
        )
      );
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });

    it('toggle-camera disables camera and hides preview', async () => {
      mockGetConfig.mockReturnValue({
        recording: {
          systemAudio: true,
          micEnabled: false,
          camera: { enabled: true },
        },
      });
      await setupAndFire('toggle-camera');
      expect(mockHideCameraPreview).toHaveBeenCalled();
    });

    it('select-camera with null enables the system default camera', async () => {
      await setupAndFire('select-camera', {
        deviceId: null,
        deviceName: null,
      });
      expect(mockShowCameraPreview).toHaveBeenCalledWith(
        expect.objectContaining({ enabled: true, selectedDeviceId: null })
      );
    });

    it('select-camera with deviceId enables and shows preview', async () => {
      mockCheckCamera.mockResolvedValue(true);
      await setupAndFire('select-camera', {
        deviceId: 'cam-1',
        deviceName: 'Cam 1',
      });
      expect(mockShowCameraPreview).toHaveBeenCalled();
    });

    it('start delegates to startPendingRecording', async () => {
      mockStartPendingRecording.mockResolvedValue(undefined);
      await setupAndFire('start');
      expect(mockStartPendingRecording).toHaveBeenCalled();
    });

    it('keeps and starts with separate All-in-One recording toggles', async () => {
      mockGetConfig.mockReturnValue({
        recording: {
          systemAudio: false,
          micEnabled: false,
          selectedMicId: 'shared-mic',
          selectedMicName: 'Shared Mic',
          camera: {
            enabled: false,
            selectedDeviceId: 'shared-camera',
            selectedDeviceName: 'Shared Camera',
          },
          iosDevice: null,
        },
        allInOne: {
          rememberChoices: true,
          lastMode: 'record',
          lastTargets: { screenshot: 'area', record: 'area' },
          lastArea: null,
          recording: {
            systemAudio: true,
            micEnabled: false,
            cameraEnabled: false,
          },
        },
      });
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl(undefined, undefined, {
        preferenceScope: 'all-in-one',
      });

      expect(mockShowBrowserWindow).toHaveBeenCalledWith(
        expect.objectContaining({
          systemAudio: true,
          micEnabled: false,
          cameraEnabled: false,
        }),
        expect.anything(),
        expect.any(Function)
      );

      lastActionHandler()('toggle-system-audio');
      await flush();
      lastActionHandler()('toggle-mic');
      await flush();
      lastActionHandler()('toggle-camera');
      await flush();

      expect(mockUpdateConfig).toHaveBeenCalledWith({
        allInOne: expect.objectContaining({
          recording: {
            systemAudio: false,
            micEnabled: true,
            cameraEnabled: true,
          },
        }),
      });
      expect(mockUpdateConfig).not.toHaveBeenCalledWith(
        expect.objectContaining({ recording: expect.anything() })
      );

      lastActionHandler()('start');
      await vi.waitFor(() =>
        expect(mockStartPendingRecording).toHaveBeenCalledWith(
          expect.objectContaining({
            systemAudio: false,
            micEnabled: true,
            micDeviceId: 'shared-mic',
            cameraEnabled: true,
            cameraDeviceId: 'shared-camera',
          })
        )
      );
    });

    it('does not persist All-in-One toggles when remembering is disabled', async () => {
      mockGetConfig.mockReturnValue({
        recording: {
          systemAudio: false,
          micEnabled: true,
          camera: { enabled: true },
          iosDevice: null,
        },
        allInOne: {
          rememberChoices: false,
          recording: {
            systemAudio: false,
            micEnabled: true,
            cameraEnabled: true,
          },
        },
      });
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl(undefined, undefined, {
        preferenceScope: 'all-in-one',
      });
      mockUpdateConfig.mockClear();

      expect(mockShowBrowserWindow).toHaveBeenCalledWith(
        expect.objectContaining({
          systemAudio: true,
          micEnabled: false,
          cameraEnabled: false,
        }),
        expect.anything(),
        expect.any(Function)
      );

      lastActionHandler()('toggle-system-audio');
      await flush();
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });

    it('start is debounced', async () => {
      let resolveFirst: () => void = () => {};
      mockStartPendingRecording.mockImplementation(
        () =>
          new Promise<void>(res => {
            resolveFirst = res;
          })
      );
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      lastActionHandler()('start');
      await vi.waitFor(() =>
        expect(mockStartPendingRecording).toHaveBeenCalledTimes(1)
      );
      lastActionHandler()('start');
      await flush();
      resolveFirst();
      await vi.waitFor(() =>
        expect(mockStartPendingRecording).toHaveBeenCalledTimes(1)
      );
      expect(mockStartPendingRecording).toHaveBeenCalledTimes(1);
    });

    it('ignores mutable controls while recording startup is pending', async () => {
      let resolveStart: () => void = () => {};
      mockStartPendingRecording.mockImplementation(
        () =>
          new Promise<void>(resolve => {
            resolveStart = resolve;
          })
      );
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      lastActionHandler()('start');
      await vi.waitFor(() =>
        expect(mockStartPendingRecording).toHaveBeenCalledTimes(1)
      );
      mockUpdateConfig.mockClear();

      lastActionHandler()('toggle-system-audio');
      await flush();
      expect(mockUpdateConfig).not.toHaveBeenCalled();

      resolveStart();
    });

    it('cancel calls cancelPendingRecording', async () => {
      await setupAndFire('cancel');
      expect(mockCancelPendingRecording).toHaveBeenCalled();
    });

    it('pause + resume coordinate timer', async () => {
      await setupAndFire('pause');
      expect(mockPauseRecording).toHaveBeenCalled();
      lastActionHandler()('resume');
      await flush();
      expect(mockResumeRecording).toHaveBeenCalled();
    });

    it('reports rejected recording actions', async () => {
      mockPauseRecording.mockRejectedValue(new Error('pause failed'));
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      lastActionHandler()('pause');

      await vi.waitFor(() =>
        expect(mockShowRecordingError).toHaveBeenCalledWith(
          expect.objectContaining({ message: 'pause failed' })
        )
      );
    });

    it('reports a rejected stop action', async () => {
      mockStopRecordingAction.mockRejectedValue(new Error('stop failed'));
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      lastActionHandler()('stop');

      await vi.waitFor(() =>
        expect(mockShowRecordingError).toHaveBeenCalledWith(
          expect.objectContaining({ message: 'stop failed' })
        )
      );
    });

    it('stop calls stopRecordingAction', async () => {
      await setupAndFire('stop');
      expect(mockStopRecordingAction).toHaveBeenCalled();
    });

    it('delete calls deleteRecordingAction', async () => {
      await setupAndFire('delete');
      expect(mockDeleteRecordingAction).toHaveBeenCalled();
    });

    it('select-ios-device hides selector when device chosen', async () => {
      await setupAndFire('select-ios-device', {
        deviceId: 'ios-1',
        deviceName: 'iPhone',
      });
      expect(mockHideAreaSelector).toHaveBeenCalled();
    });

    it('select-ios-device shows selector when cleared', async () => {
      await setupAndFire('select-ios-device', {
        deviceId: null,
        deviceName: null,
      });
      expect(mockShowAreaSelector).toHaveBeenCalled();
    });
  });

  describe('camera permission check', () => {
    it('keeps the camera disabled when permission is denied', async () => {
      mockGetConfig.mockReturnValue({
        recording: {
          systemAudio: true,
          micEnabled: false,
          camera: { enabled: false },
        },
      });
      mockCheckCamera.mockResolvedValue(false);
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      mockUpdateConfig.mockClear();
      lastActionHandler()('toggle-camera');
      await flush();
      expect(mockShowCameraPreview).not.toHaveBeenCalled();
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });
  });

  describe('mid-recording device changes', () => {
    const RECORDING_MIC_CONFIG = {
      ...RECORDING_CONFIG,
      micEnabled: true,
      micDeviceId: 'old-mic',
      micDeviceName: 'Old Mic',
    };
    const RECORDING_CAMERA_CONFIG = {
      ...RECORDING_CONFIG,
      cameraEnabled: true,
      cameraDeviceId: 'cam',
      cameraDeviceName: 'Cam',
    };

    async function startRecording(config: typeof RECORDING_CONFIG) {
      Object.defineProperty(process, 'platform', { value: 'win32' });
      vi.resetModules();
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      await m.showRecordingControl(config);
      mockDaemonCall.mockClear();
      mockUpdateConfig.mockClear();
      mockUpdateBrowserWindow.mockClear();
      mockShowCameraPreview.mockClear();
      mockHideCameraPreview.mockClear();
      return m;
    }

    it('switches the microphone live without writing to config', async () => {
      await startRecording(RECORDING_MIC_CONFIG);

      lastActionHandler()('select-mic', {
        deviceId: 'new-mic',
        deviceName: 'New Mic',
      });

      await vi.waitFor(() =>
        expect(mockDaemonCall).toHaveBeenCalledWith(
          'screen-recorder',
          'setMicrophone',
          { enabled: true, deviceId: 'new-mic', deviceName: 'New Mic' }
        )
      );
      expect(mockUpdateBrowserWindow).toHaveBeenCalledWith(
        expect.objectContaining({
          micEnabled: true,
          selectedMicId: 'new-mic',
        })
      );
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });

    it('switches to the system default microphone while recording', async () => {
      await startRecording(RECORDING_MIC_CONFIG);

      lastActionHandler()('select-mic', {
        deviceId: null,
        deviceName: null,
      });

      await vi.waitFor(() =>
        expect(mockDaemonCall).toHaveBeenCalledWith(
          'screen-recorder',
          'setMicrophone',
          { enabled: true, deviceId: null, deviceName: null }
        )
      );
      expect(mockUpdateBrowserWindow).toHaveBeenCalledWith(
        expect.objectContaining({ micEnabled: true, selectedMicId: null })
      );
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });

    it('skips the live microphone switch when permission is denied', async () => {
      await startRecording(RECORDING_MIC_CONFIG);
      mockCheckMic.mockResolvedValue(false);

      lastActionHandler()('select-mic', {
        deviceId: 'new-mic',
        deviceName: 'New Mic',
      });
      await flush();

      expect(mockDaemonCall).not.toHaveBeenCalledWith(
        'screen-recorder',
        'setMicrophone',
        expect.anything()
      );
    });

    it('toggles system audio live without writing to config', async () => {
      await startRecording(RECORDING_CONFIG);

      lastActionHandler()('toggle-system-audio');

      await vi.waitFor(() =>
        expect(mockDaemonCall).toHaveBeenCalledWith(
          'screen-recorder',
          'setSystemAudio',
          { enabled: false }
        )
      );
      expect(mockUpdateBrowserWindow).toHaveBeenCalledWith(
        expect.objectContaining({ systemAudio: false })
      );
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });

    it('turns the camera off and back on while recording', async () => {
      mockGetCameraPreviewSettings.mockReturnValue({
        enabled: true,
        selectedDeviceId: 'cam',
        selectedDeviceName: 'Cam',
        position: { x: 10, y: 20 },
      });
      await startRecording(RECORDING_CAMERA_CONFIG);

      lastActionHandler()('toggle-camera');

      await vi.waitFor(() =>
        expect(mockHideCameraPreview).toHaveBeenCalledTimes(1)
      );
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'setCamera',
        { enabled: false }
      );
      expect(mockUpdateConfig).not.toHaveBeenCalled();

      lastActionHandler()('toggle-camera');

      await vi.waitFor(() =>
        expect(mockShowCameraPreview).toHaveBeenLastCalledWith(
          expect.objectContaining({
            enabled: true,
            selectedDeviceId: 'cam',
            position: { x: 10, y: 20 },
          })
        )
      );
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'setCamera',
        { enabled: true }
      );
    });

    it('keeps the camera enabled when System Default is selected while recording', async () => {
      mockGetCameraPreviewSettings.mockReturnValue({
        enabled: true,
        selectedDeviceId: 'cam',
        selectedDeviceName: 'Cam',
        position: { x: 10, y: 20 },
      });
      await startRecording(RECORDING_CAMERA_CONFIG);

      lastActionHandler()('select-camera', {
        deviceId: null,
        deviceName: null,
      });

      await vi.waitFor(() =>
        expect(mockDaemonCall).toHaveBeenCalledWith(
          'screen-recorder',
          'setCamera',
          { enabled: true }
        )
      );
      expect(mockHideCameraPreview).not.toHaveBeenCalled();
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });

    it('starts the fixed camera track when it was off at recording start', async () => {
      await startRecording(RECORDING_CONFIG);

      lastActionHandler()('toggle-camera');

      await vi.waitFor(() =>
        expect(mockShowCameraPreview).toHaveBeenCalledWith(
          expect.objectContaining({ enabled: true })
        )
      );
      expect(mockEnableCameraContentProtection).toHaveBeenCalledTimes(1);
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'setCamera',
        { enabled: true }
      );
    });

    it('reports the camera device as locked on the toolbar', async () => {
      Object.defineProperty(process, 'platform', { value: 'win32' });
      vi.resetModules();
      mockGetCameraPreviewSettings.mockReturnValue({ enabled: true });
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();

      await m.showRecordingControl(RECORDING_CAMERA_CONFIG);

      expect(mockShowBrowserWindow).toHaveBeenLastCalledWith(
        expect.objectContaining({
          mode: 'recording',
          cameraLocked: true,
          cameraEnabled: true,
          selectedCameraId: 'cam',
        }),
        expect.anything(),
        expect.any(Function)
      );
      await m.hideRecordingControl();
    });

    it('locks the selected camera while recording starts with it off', async () => {
      Object.defineProperty(process, 'platform', { value: 'win32' });
      vi.resetModules();
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();

      await m.showRecordingControl(RECORDING_CONFIG);

      expect(mockShowBrowserWindow).toHaveBeenLastCalledWith(
        expect.objectContaining({ mode: 'recording', cameraLocked: true }),
        expect.anything(),
        expect.any(Function)
      );
      await m.hideRecordingControl();
    });

    it('exposes live device changes on macOS', async () => {
      Object.defineProperty(process, 'platform', { value: 'darwin' });
      vi.resetModules();
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();

      await m.showRecordingControl(RECORDING_CAMERA_CONFIG);

      expect(mockShowBrowserWindow).toHaveBeenLastCalledWith(
        expect.objectContaining({ mode: 'recording', cameraLocked: true }),
        expect.anything(),
        expect.any(Function)
      );

      lastActionHandler()('toggle-system-audio');
      await flush();
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'setSystemAudio',
        { enabled: false }
      );
      await m.hideRecordingControl();
    });
  });

  describe('recording countdown', () => {
    it('ticks the countdown down and completes', async () => {
      vi.useFakeTimers();
      try {
        const m = await import('@/main/capture/video/recording-control');
        m.showPreRecordingControl({ x: 100, y: 100, width: 800, height: 600 });

        const pending = m.startRecordingCountdown(3);
        expect(mockGlobalShortcutRegister).toHaveBeenCalledWith(
          'Escape',
          expect.any(Function)
        );
        expect(mockUpdateBrowserWindow).toHaveBeenCalledWith({
          countdownSeconds: 3,
        });

        await vi.advanceTimersByTimeAsync(1000);
        expect(mockUpdateBrowserWindow).toHaveBeenLastCalledWith({
          countdownSeconds: 2,
        });

        await vi.advanceTimersByTimeAsync(2000);
        await expect(pending).resolves.toBe('completed');
        expect(mockGlobalShortcutUnregister).toHaveBeenCalledWith('Escape');
        expect(mockUpdateBrowserWindow).toHaveBeenLastCalledWith({
          countdownSeconds: null,
        });
      } finally {
        vi.useRealTimers();
      }
    });

    it('cancels from the toolbar action while counting down', async () => {
      vi.useFakeTimers();
      try {
        const m = await import('@/main/capture/video/recording-control');
        m.showPreRecordingControl({ x: 100, y: 100, width: 800, height: 600 });

        const pending = m.startRecordingCountdown(3);
        lastActionHandler()('cancel');

        await expect(pending).resolves.toBe('cancelled');
        expect(mockUpdateBrowserWindow).toHaveBeenLastCalledWith({
          countdownSeconds: null,
        });
        expect(mockCancelPendingRecording).not.toHaveBeenCalled();
      } finally {
        vi.useRealTimers();
      }
    });

    it('cancels the countdown when the pre-recording control is hidden', async () => {
      vi.useFakeTimers();
      try {
        const m = await import('@/main/capture/video/recording-control');
        m.showPreRecordingControl({ x: 100, y: 100, width: 800, height: 600 });

        const pending = m.startRecordingCountdown(3);
        await m.hidePreRecordingControl();

        await expect(pending).resolves.toBe('cancelled');
        expect(mockHideBrowserWindow).toHaveBeenCalled();
      } finally {
        vi.useRealTimers();
      }
    });
  });
});
