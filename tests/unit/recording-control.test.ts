import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockDaemonCall = vi.fn();
let daemonEventHandler: ((e: string, d?: unknown) => void) | null = null;
const mockDaemonOnEvent = vi.fn((cb: (e: string, d?: unknown) => void) => {
  daemonEventHandler = cb;
});
const mockDaemonOffEvent = vi.fn(() => {
  daemonEventHandler = null;
});
const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockShowCameraPreview = vi.fn();
const mockUpdateCameraPreviewPosition = vi.fn();
const mockHideCameraPreview = vi.fn();
const mockGetCameraPreviewSettings = vi.fn();

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
const mockDipToScreenPoint = vi.fn();
const mockGetDisplayMatching = vi.fn();
const mockBrowserWindow = { id: 'recording-control' };
const mockGetBrowserWindow = vi.fn(() => mockBrowserWindow);
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
  shell: { openExternal: (...a: unknown[]) => mockShellOpenExternal(...a) },
  screen: {
    dipToScreenPoint: (...a: unknown[]) => mockDipToScreenPoint(...a),
    getDisplayMatching: (...a: unknown[]) => mockGetDisplayMatching(...a),
  },
}));

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...a: unknown[]) => mockDaemonCall(...a),
    onEvent: (...a: unknown[]) => mockDaemonOnEvent(...a),
    offEvent: (...a: unknown[]) => mockDaemonOffEvent(...a),
  },
}));

vi.mock('@/main/capture/video/recording-control-window', () => ({
  clearRecordingControlBrowserWindowParent: () =>
    mockClearBrowserWindowParent(),
  getRecordingControlBrowserWindow: () => mockGetBrowserWindow(),
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
}));

const mockStartPendingRecording = vi.fn();
const mockCancelPendingRecording = vi.fn();
const mockStopRecordingAction = vi.fn();
const mockDeleteRecordingAction = vi.fn();
const mockRestartRecordingAction = vi.fn();

vi.mock('@/main/capture/video/recording-actions', () => ({
  startPendingRecording: (...a: unknown[]) => mockStartPendingRecording(...a),
  cancelPendingRecording: () => mockCancelPendingRecording(),
  stopRecordingAction: () => mockStopRecordingAction(),
  deleteRecordingAction: () => mockDeleteRecordingAction(),
  restartRecordingAction: () => mockRestartRecordingAction(),
}));

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
const mockShellOpenExternal = vi.fn();

describe('recording-control', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    Object.defineProperty(process, 'platform', { value: 'darwin' });
    vi.clearAllMocks();
    vi.resetModules();
    mockDaemonCall.mockResolvedValue({});
    mockCancelPendingRecording.mockResolvedValue(undefined);
    mockStartPendingRecording.mockResolvedValue(undefined);
    mockStopRecordingAction.mockResolvedValue(undefined);
    mockDeleteRecordingAction.mockResolvedValue(undefined);
    mockRestartRecordingAction.mockResolvedValue(undefined);
    mockPauseRecording.mockResolvedValue(undefined);
    mockResumeRecording.mockResolvedValue(undefined);
    mockShowCameraPreview.mockResolvedValue(undefined);
    mockUpdateCameraPreviewPosition.mockResolvedValue(undefined);
    mockShowRecordingError.mockResolvedValue(undefined);
    mockShowMessageBox.mockResolvedValue({ response: 1 });
    mockCheckCamera.mockResolvedValue(true);
    mockCheckMic.mockResolvedValue(true);
    mockGetCameraPreviewSettings.mockReturnValue(null);
    mockDipToScreenPoint.mockImplementation(position => position);
    mockGetDisplayMatching.mockReturnValue({
      workArea: { x: 0, y: 0, width: 1920, height: 1080 },
    });
    mockGetConfig.mockReturnValue({
      recording: {
        systemAudio: true,
        micEnabled: false,
        camera: { enabled: false },
      },
    });
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
    vi.resetModules();
  });

  it('getCurrentRecordingAreaSelection returns null initially', async () => {
    const m = await import('@/main/capture/video/recording-control');
    expect(m.getCurrentRecordingAreaSelection()).toBeNull();
  });

  it('names the picked window on the control bar', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
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
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl({ x: 100, y: 100, width: 800, height: 600 });

    expect(mockShowBrowserWindow).toHaveBeenCalledWith(
      expect.objectContaining({ targetName: null }),
      expect.anything(),
      expect.any(Function)
    );
  });

  it('showPreRecordingControl calls daemon show with area position', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl({ x: 100, y: 100, width: 800, height: 600 });
    expect(mockDaemonCall).toHaveBeenCalledWith(
      'recording-control',
      'show',
      expect.objectContaining({ mode: 'pre-recording' })
    );
    expect(m.getCurrentRecordingAreaSelection()).toEqual({
      x: 100,
      y: 100,
      width: 800,
      height: 600,
    });
  });

  it('handles microphone selection from the Windows toolbar menu', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    mockCheckMic.mockResolvedValue(true);
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl();
    const onAction = mockShowBrowserWindow.mock.calls[0][2] as (
      action: string,
      data: { deviceId: string; deviceName: string }
    ) => void;

    onAction('select-mic', {
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
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
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

  it('keeps the Electron control position in logical pixels', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    mockDipToScreenPoint.mockReturnValue({ x: 900, y: 1000 });

    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl({ x: 100, y: 100, width: 800, height: 600 });

    expect(mockDipToScreenPoint).not.toHaveBeenCalled();
    expect(mockShowBrowserWindow).toHaveBeenCalledWith(
      expect.objectContaining({ mode: 'pre-recording' }),
      { x: 842, y: 24 },
      expect.any(Function)
    );
  });

  it('keeps the Windows toolbar centered when recording starts', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
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

  it('detaches the Windows toolbar from the overlay window', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    const m = await import('@/main/capture/video/recording-control');

    m.detachRecordingControlFromOverlay();

    expect(mockClearBrowserWindowParent).toHaveBeenCalledTimes(1);
  });

  it('skips overlay detachment outside Windows', async () => {
    const m = await import('@/main/capture/video/recording-control');

    m.detachRecordingControlFromOverlay();

    expect(mockClearBrowserWindowParent).not.toHaveBeenCalled();
  });

  it('routes Windows renderer actions through the recording controller', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl();
    mockUpdateConfig.mockImplementation(update => {
      mockGetConfig.mockReturnValue(update);
    });
    const actionHandler = mockShowBrowserWindow.mock.calls[0][2] as (
      action: string
    ) => void;

    actionHandler('toggle-system-audio');

    await vi.waitFor(() => expect(mockUpdateConfig).toHaveBeenCalled());
    expect(mockUpdateBrowserWindow).toHaveBeenCalledWith(
      expect.objectContaining({ systemAudio: false })
    );
  });

  it('discards straight away from the Windows toolbar', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl();
    const actionHandler = mockShowBrowserWindow.mock.calls[0][2] as (
      action: string
    ) => void;

    actionHandler('delete');

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

  it('places the Windows camera inside the selected area', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
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

  it('showPreRecordingControl falls back to default position', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl();
    expect(mockDaemonCall).toHaveBeenCalledWith(
      'recording-control',
      'show',
      expect.objectContaining({ x: 100, y: 100 })
    );
  });

  it('updateRecordingControlPosition calls daemon update', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.updateRecordingControlPosition({
      x: 200,
      y: 200,
      width: 100,
      height: 100,
    });
    expect(mockDaemonCall).toHaveBeenCalledWith(
      'recording-control',
      'update',
      expect.objectContaining({ x: expect.any(Number) })
    );
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

  it('hidePreRecordingControl hides daemon and camera by default', async () => {
    const m = await import('@/main/capture/video/recording-control');
    await m.hidePreRecordingControl();
    expect(mockDaemonCall).toHaveBeenCalledWith('recording-control', 'hide');
    expect(mockHideCameraPreview).toHaveBeenCalled();
  });

  it('hidePreRecordingControl with hideCamera=false skips camera hide', async () => {
    const m = await import('@/main/capture/video/recording-control');
    await m.hidePreRecordingControl(false);
    expect(mockHideCameraPreview).not.toHaveBeenCalled();
  });

  it('showRecordingControl sets mode to recording', async () => {
    const m = await import('@/main/capture/video/recording-control');
    await m.showRecordingControl(RECORDING_CONFIG);
    expect(mockDaemonCall).toHaveBeenCalledWith(
      'recording-control',
      'setMode',
      { mode: 'recording' }
    );
  });

  it('hideRecordingControl stops timer and hides daemon', async () => {
    const m = await import('@/main/capture/video/recording-control');
    await m.hideRecordingControl();
    expect(mockDaemonCall).toHaveBeenCalledWith('recording-control', 'hide');
  });

  it('hidePreRecordingControl swallows daemon errors', async () => {
    mockDaemonCall.mockRejectedValue(new Error('boom'));
    const m = await import('@/main/capture/video/recording-control');
    await expect(m.hidePreRecordingControl()).resolves.toBeUndefined();
  });

  it('showRecordingControl propagates daemon errors', async () => {
    mockDaemonCall.mockRejectedValue(new Error('boom'));
    const m = await import('@/main/capture/video/recording-control');
    await expect(m.showRecordingControl(RECORDING_CONFIG)).rejects.toThrow(
      'boom'
    );
  });

  it('showPreRecordingControl cancels when the native window fails', async () => {
    mockDaemonCall.mockRejectedValue(new Error('boom'));
    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl();
    await vi.waitFor(() =>
      expect(mockCancelPendingRecording).toHaveBeenCalledTimes(1)
    );
  });

  it('hideRecordingControl swallows daemon errors', async () => {
    mockDaemonCall.mockRejectedValue(new Error('boom'));
    const m = await import('@/main/capture/video/recording-control');
    await expect(m.hideRecordingControl()).resolves.toBeUndefined();
  });

  it('updateRecordingControlPosition swallows daemon errors', async () => {
    mockDaemonCall.mockRejectedValue(new Error('boom'));
    const m = await import('@/main/capture/video/recording-control');
    expect(() =>
      m.updateRecordingControlPosition({
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      })
    ).not.toThrow();
  });

  it('getRecordingControlWindow returns null', async () => {
    const m = await import('@/main/capture/video/recording-control');
    expect(m.getRecordingControlWindow()).toBeNull();
  });

  it('prewarmRecordingControlWindow is a no-op', async () => {
    const m = await import('@/main/capture/video/recording-control');
    expect(() => m.prewarmRecordingControlWindow()).not.toThrow();
  });

  it('pauseTimer is no-op when not recording', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.pauseTimer();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it('resumeTimer is no-op when not paused', async () => {
    const m = await import('@/main/capture/video/recording-control');
    m.resumeTimer();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it('handles timer control update failures', async () => {
    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => {});
    const m = await import('@/main/capture/video/recording-control');
    await m.showRecordingControl(RECORDING_CONFIG);
    mockDaemonCall.mockRejectedValueOnce(new Error('control unavailable'));

    m.pauseTimer();

    await vi.waitFor(() =>
      expect(consoleError).toHaveBeenCalledWith(
        'Failed to update paused control state:',
        expect.objectContaining({ message: 'control unavailable' })
      )
    );
    consoleError.mockRestore();
  });

  it('swallows daemon errors on show', async () => {
    mockDaemonCall.mockRejectedValue(new Error('boom'));
    const m = await import('@/main/capture/video/recording-control');
    expect(() => m.showPreRecordingControl()).not.toThrow();
  });

  describe('event handlers', () => {
    async function setupAndFire(event: string, data?: unknown): Promise<void> {
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      expect(daemonEventHandler).not.toBeNull();
      await daemonEventHandler!(event, data);
    }

    it('toggle-system-audio flips config', async () => {
      await setupAndFire('recording-control:toggle-system-audio');
      expect(mockUpdateConfig).toHaveBeenCalled();
    });

    it('toggle-mic enables mic when permission granted', async () => {
      mockCheckMic.mockResolvedValue(true);
      await setupAndFire('recording-control:toggle-mic');
      expect(mockUpdateConfig).toHaveBeenCalled();
    });

    it('toggle-mic aborts when permission denied', async () => {
      mockGetConfig.mockReturnValue({
        recording: { micEnabled: false, systemAudio: true, camera: null },
      });
      mockCheckMic.mockResolvedValue(false);
      mockUpdateConfig.mockClear();
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      mockUpdateConfig.mockClear();
      await daemonEventHandler!('recording-control:toggle-mic');
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });

    it('select-mic with deviceId updates config', async () => {
      mockCheckMic.mockResolvedValue(true);
      await setupAndFire('recording-control:select-mic', {
        deviceId: 'mic-1',
        deviceName: 'Mic 1',
      });
      expect(mockUpdateConfig).toHaveBeenCalled();
    });

    it('select-mic with null deviceId enables the system default microphone', async () => {
      await setupAndFire('recording-control:select-mic', {
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
      await setupAndFire('recording-control:toggle-camera');
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
      daemonEventHandler!('recording-control:toggle-camera');

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
      await setupAndFire('recording-control:toggle-camera');
      expect(mockHideCameraPreview).toHaveBeenCalled();
    });

    it('select-camera with null enables the system default camera', async () => {
      await setupAndFire('recording-control:select-camera', {
        deviceId: null,
        deviceName: null,
      });
      expect(mockShowCameraPreview).toHaveBeenCalledWith(
        expect.objectContaining({ enabled: true, selectedDeviceId: null })
      );
    });

    it('select-camera with deviceId enables and shows preview', async () => {
      mockCheckCamera.mockResolvedValue(true);
      await setupAndFire('recording-control:select-camera', {
        deviceId: 'cam-1',
        deviceName: 'Cam 1',
      });
      expect(mockShowCameraPreview).toHaveBeenCalled();
    });

    it('toggle-mic-mute toggles mute via daemon', async () => {
      await setupAndFire('recording-control:toggle-mic-mute');
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'setMicMuted',
        expect.objectContaining({ muted: expect.any(Boolean) })
      );
    });

    it('retries the same mic mute state after a native failure', async () => {
      const consoleError = vi
        .spyOn(console, 'error')
        .mockImplementation(() => {});
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      mockDaemonCall.mockRejectedValueOnce(new Error('mute failed'));

      await daemonEventHandler!('recording-control:toggle-mic-mute');
      await daemonEventHandler!('recording-control:toggle-mic-mute');

      const muteCalls = mockDaemonCall.mock.calls.filter(
        call => call[0] === 'screen-recorder' && call[1] === 'setMicMuted'
      );
      expect(muteCalls).toEqual([
        ['screen-recorder', 'setMicMuted', { muted: true }],
        ['screen-recorder', 'setMicMuted', { muted: true }],
      ]);
      consoleError.mockRestore();
    });

    it('start delegates to startPendingRecording', async () => {
      mockStartPendingRecording.mockResolvedValue(undefined);
      await setupAndFire('recording-control:start');
      expect(mockStartPendingRecording).toHaveBeenCalled();
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
      const first = daemonEventHandler!('recording-control:start');
      const second = daemonEventHandler!('recording-control:start');
      resolveFirst();
      await first;
      await second;
      expect(mockStartPendingRecording).toHaveBeenCalledTimes(1);
    });

    it('allows a retry when the native starting state update fails', async () => {
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      mockDaemonCall.mockRejectedValueOnce(new Error('state update failed'));

      daemonEventHandler!('recording-control:start');
      await vi.waitFor(() =>
        expect(mockShowRecordingError).toHaveBeenCalledWith(
          expect.objectContaining({ message: 'state update failed' })
        )
      );

      daemonEventHandler!('recording-control:start');
      await vi.waitFor(() =>
        expect(mockStartPendingRecording).toHaveBeenCalledTimes(1)
      );
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
      daemonEventHandler!('recording-control:start');
      await vi.waitFor(() =>
        expect(mockStartPendingRecording).toHaveBeenCalledTimes(1)
      );
      mockUpdateConfig.mockClear();

      daemonEventHandler!('recording-control:toggle-system-audio');
      await Promise.resolve();
      expect(mockUpdateConfig).not.toHaveBeenCalled();

      resolveStart();
    });

    it('cancel calls cancelPendingRecording', async () => {
      await setupAndFire('recording-control:cancel');
      expect(mockCancelPendingRecording).toHaveBeenCalled();
    });

    it('pause + resume coordinate timer', async () => {
      await setupAndFire('recording-control:pause');
      expect(mockPauseRecording).toHaveBeenCalled();
      await daemonEventHandler!('recording-control:resume');
      expect(mockResumeRecording).toHaveBeenCalled();
    });

    it('reports rejected recording actions', async () => {
      mockPauseRecording.mockRejectedValue(new Error('pause failed'));
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      daemonEventHandler!('recording-control:pause');

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
      daemonEventHandler!('recording-control:stop');

      await vi.waitFor(() =>
        expect(mockShowRecordingError).toHaveBeenCalledWith(
          expect.objectContaining({ message: 'stop failed' })
        )
      );
    });

    it('stop calls stopRecordingAction', async () => {
      await setupAndFire('recording-control:stop');
      expect(mockStopRecordingAction).toHaveBeenCalled();
    });

    it('restart calls restartRecordingAction', async () => {
      await setupAndFire('recording-control:restart');
      expect(mockRestartRecordingAction).toHaveBeenCalled();
    });

    it('delete calls deleteRecordingAction', async () => {
      await setupAndFire('recording-control:delete');
      expect(mockDeleteRecordingAction).toHaveBeenCalled();
    });

    it('open-ios-help opens external URL', async () => {
      await setupAndFire('recording-control:open-ios-help');
      expect(mockShellOpenExternal).toHaveBeenCalledWith(
        expect.stringContaining('capty.app')
      );
    });

    it('select-ios-device hides selector when device chosen', async () => {
      await setupAndFire('recording-control:select-ios-device', {
        deviceId: 'ios-1',
        deviceName: 'iPhone',
      });
      expect(mockHideAreaSelector).toHaveBeenCalled();
    });

    it('select-ios-device shows selector when cleared', async () => {
      await setupAndFire('recording-control:select-ios-device', {
        deviceId: null,
        deviceName: null,
      });
      expect(mockShowAreaSelector).toHaveBeenCalled();
    });

    it('select-aspect-ratio applies aspect ratio', async () => {
      await setupAndFire('recording-control:select-aspect-ratio', {
        width: 16,
        height: 9,
        name: '16:9',
      });
      expect(mockSetAspectRatio).toHaveBeenCalled();
    });

    it('select-aspect-ratio is no-op without width/height', async () => {
      await setupAndFire('recording-control:select-aspect-ratio', {
        name: 'x',
      });
      expect(mockSetAspectRatio).not.toHaveBeenCalled();
    });

    it('select-aspect-ratio clears iosDevice if set', async () => {
      mockGetConfig.mockReturnValue({
        recording: {
          systemAudio: true,
          micEnabled: false,
          camera: null,
          iosDevice: { id: 'ios-1', name: 'iPhone' },
        },
      });
      await setupAndFire('recording-control:select-aspect-ratio', {
        width: 4,
        height: 3,
        name: '4:3',
      });
      expect(mockUpdateConfig).toHaveBeenCalled();
    });
  });

  describe('timer pause / resume', () => {
    it('resumeTimer is no-op when not paused', async () => {
      const m = await import('@/main/capture/video/recording-control');
      m.resumeTimer();
      expect(mockDaemonCall).not.toHaveBeenCalled();
    });
  });

  describe('camera permission check', () => {
    it('keeps the camera disabled when permission is denied', async () => {
      // Permission flow only triggers when toggling camera ON
      // (not when toggling OFF). Set camera to disabled so toggle enables it.
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
      await daemonEventHandler!('recording-control:toggle-camera');
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
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      await m.showRecordingControl(config);
      mockDaemonCall.mockClear();
      mockUpdateConfig.mockClear();
      mockShowCameraPreview.mockClear();
      mockHideCameraPreview.mockClear();
      return m;
    }

    it('switches the microphone live without writing to config', async () => {
      await startRecording(RECORDING_MIC_CONFIG);

      await daemonEventHandler!('recording-control:select-mic', {
        deviceId: 'new-mic',
        deviceName: 'New Mic',
      });

      await vi.waitFor(() =>
        expect(mockDaemonCall).toHaveBeenCalledWith(
          'recording-control',
          'updateSettings',
          expect.objectContaining({
            micEnabled: true,
            selectedMicId: 'new-mic',
            selectedMicName: 'New Mic',
          })
        )
      );
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'setMicrophone',
        { enabled: true, deviceId: 'new-mic', deviceName: 'New Mic' }
      );
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });

    it('switches to the system default microphone while recording', async () => {
      await startRecording(RECORDING_MIC_CONFIG);

      await daemonEventHandler!('recording-control:select-mic', {
        deviceId: null,
        deviceName: null,
      });

      await vi.waitFor(() =>
        expect(mockDaemonCall).toHaveBeenCalledWith(
          'recording-control',
          'updateSettings',
          expect.objectContaining({ micEnabled: true, selectedMicId: null })
        )
      );
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'setMicrophone',
        { enabled: true, deviceId: null, deviceName: null }
      );
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });

    it('skips the live microphone switch when permission is denied', async () => {
      await startRecording(RECORDING_MIC_CONFIG);
      mockCheckMic.mockResolvedValue(false);

      await daemonEventHandler!('recording-control:select-mic', {
        deviceId: 'new-mic',
        deviceName: 'New Mic',
      });

      expect(mockDaemonCall).not.toHaveBeenCalledWith(
        'screen-recorder',
        'setMicrophone',
        expect.anything()
      );
    });

    it('toggles system audio live without writing to config', async () => {
      await startRecording(RECORDING_CONFIG);

      await daemonEventHandler!('recording-control:toggle-system-audio');

      await vi.waitFor(() =>
        expect(mockDaemonCall).toHaveBeenCalledWith(
          'recording-control',
          'updateSettings',
          expect.objectContaining({ systemAudio: false })
        )
      );
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'setSystemAudio',
        { enabled: false }
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

      await daemonEventHandler!('recording-control:toggle-camera');

      await vi.waitFor(() =>
        expect(mockHideCameraPreview).toHaveBeenCalledTimes(1)
      );
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'setCamera',
        { enabled: false }
      );
      expect(mockUpdateConfig).not.toHaveBeenCalled();

      await daemonEventHandler!('recording-control:toggle-camera');

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

      await daemonEventHandler!('recording-control:select-camera', {
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

    it('ignores camera changes when the recording has no camera track', async () => {
      await startRecording(RECORDING_CONFIG);

      await daemonEventHandler!('recording-control:toggle-camera');

      expect(mockDaemonCall).not.toHaveBeenCalledWith(
        'screen-recorder',
        'setCamera',
        expect.anything()
      );
      expect(mockShowCameraPreview).not.toHaveBeenCalled();
    });

    it('reports the camera device as locked on the Windows toolbar', async () => {
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
    });

    it('leaves the camera unlocked when the recording has no camera', async () => {
      Object.defineProperty(process, 'platform', { value: 'win32' });
      vi.resetModules();
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();

      await m.showRecordingControl(RECORDING_CONFIG);

      expect(mockShowBrowserWindow).toHaveBeenLastCalledWith(
        expect.objectContaining({ mode: 'recording', cameraLocked: false }),
        expect.anything(),
        expect.any(Function)
      );
    });
  });
});
