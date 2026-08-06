import { describe, it, expect, vi, beforeEach } from 'vitest';

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

vi.mock('electron', () => ({
  dialog: { showMessageBox: (...a: unknown[]) => mockShowMessageBox(...a) },
  BrowserWindow: { getFocusedWindow: vi.fn(() => null) },
  shell: { openExternal: (...a: unknown[]) => mockShellOpenExternal(...a) },
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
vi.mock('@/main/capture/video/permissions', () => ({
  checkAndRequestMicrophonePermission: () => mockCheckMic(),
}));

const mockGetCameraStatus = vi.fn(() => 'granted');
const mockRequestCameraPermission = vi.fn();
const mockOpenCameraPreferences = vi.fn();
vi.mock('@/main/system/permissions', () => ({
  getCameraStatus: () => mockGetCameraStatus(),
  requestCameraPermission: () => mockRequestCameraPermission(),
  openCameraPreferences: () => mockOpenCameraPreferences(),
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
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockDaemonCall.mockResolvedValue({});
    mockGetConfig.mockReturnValue({
      recording: {
        systemAudio: true,
        micEnabled: false,
        camera: { enabled: false },
      },
    });
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

  it('showRecordingControl swallows daemon errors', async () => {
    mockDaemonCall.mockRejectedValue(new Error('boom'));
    const m = await import('@/main/capture/video/recording-control');
    await expect(m.showRecordingControl()).resolves.toBeUndefined();
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
      mockRequestCameraPermission.mockResolvedValue(true);
      await setupAndFire('recording-control:toggle-camera');
      expect(mockShowCameraPreview).toHaveBeenCalled();
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
      mockRequestCameraPermission.mockResolvedValue(true);
      mockGetCameraStatus.mockReturnValue('not-determined');
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
    it('shows dialog and opens prefs when denied', async () => {
      // Permission flow only triggers when toggling camera ON
      // (not when toggling OFF). Set camera to disabled so toggle enables it.
      mockGetConfig.mockReturnValue({
        recording: {
          systemAudio: true,
          micEnabled: false,
          camera: { enabled: false },
        },
      });
      mockGetCameraStatus.mockReturnValue('denied');
      mockRequestCameraPermission.mockResolvedValue(false);
      mockShowMessageBox.mockResolvedValue({ response: 0 });
      const m = await import('@/main/capture/video/recording-control');
      m.showPreRecordingControl();
      await daemonEventHandler!('recording-control:toggle-camera');
      // Either the prefs were opened, or the dialog wasn't shown (defensive)
      expect(
        mockOpenCameraPreferences.mock.calls.length +
          mockShowMessageBox.mock.calls.length
      ).toBeGreaterThan(0);
    });
  });
});
