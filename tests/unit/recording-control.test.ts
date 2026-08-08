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
const mockHideCameraPreview = vi.fn();
const mockDipToScreenPoint = vi.fn();
const mockGetDisplayMatching = vi.fn();

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

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
  updateConfig: (...a: unknown[]) => mockUpdateConfig(...a),
}));

vi.mock('@/main/capture/video/camera-preview', () => ({
  showCameraPreview: (...a: unknown[]) => mockShowCameraPreview(...a),
  hideCameraPreview: () => mockHideCameraPreview(),
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
    mockShowRecordingError.mockResolvedValue(undefined);
    mockCheckCamera.mockResolvedValue(true);
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

  it('anchors the control to the top centre of the selected display', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    mockGetDisplayMatching.mockReturnValue({
      workArea: { x: 1920, y: 40, width: 1920, height: 1040 },
    });

    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl({ x: 2000, y: 600, width: 800, height: 600 });

    expect(mockDaemonCall).toHaveBeenCalledWith(
      'recording-control',
      'show',
      expect.objectContaining({ x: 2621, y: 64 })
    );
  });

  it('converts the Windows control position to physical pixels', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    mockDipToScreenPoint.mockReturnValue({ x: 900, y: 1000 });

    const m = await import('@/main/capture/video/recording-control');
    m.showPreRecordingControl({ x: 100, y: 100, width: 800, height: 600 });

    expect(mockDipToScreenPoint).toHaveBeenCalled();
    expect(mockDaemonCall).toHaveBeenCalledWith(
      'recording-control',
      'show',
      expect.objectContaining({ x: 900, y: 1000 })
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

  it('places an unpositioned Windows camera on the selected display', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    mockGetConfig.mockReturnValue({
      recording: {
        systemAudio: true,
        micEnabled: false,
        camera: { enabled: true, selectedDeviceId: 'cam-1' },
      },
    });
    mockGetDisplayMatching.mockReturnValue({
      workArea: { x: 1920, y: 0, width: 1920, height: 1080 },
    });
    const m = await import('@/main/capture/video/recording-control');

    m.showPreRecordingControl({ x: 2000, y: 100, width: 800, height: 600 });

    expect(mockShowCameraPreview).toHaveBeenCalledWith(
      expect.objectContaining({ position: { x: 3538, y: 778 } })
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
    await m.showRecordingControl();
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
    await expect(m.showRecordingControl()).rejects.toThrow('boom');
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

    it('select-mic with null deviceId disables mic', async () => {
      await setupAndFire('recording-control:select-mic', {
        deviceId: null,
        deviceName: null,
      });
      expect(mockUpdateConfig).toHaveBeenCalled();
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

    it('select-camera with null disables camera', async () => {
      await setupAndFire('recording-control:select-camera', {
        deviceId: null,
        deviceName: null,
      });
      expect(mockHideCameraPreview).toHaveBeenCalled();
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
});
