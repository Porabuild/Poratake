import { describe, it, expect, vi, beforeEach } from 'vitest';

type Handler = (...args: unknown[]) => unknown;
type DaemonEventHandler = (event: string, data?: unknown) => void;

const ipcHandle: Record<string, Handler> = {};

const mockDaemonCall = vi.fn();
const mockShowCameraPreview = vi.fn();
const mockHideCameraPreview = vi.fn();
const mockIsCameraPreviewVisible = vi.fn();
const mockMicPermission = vi.fn();
const mockCameraPermission = vi.fn();
const mockGetConfig = vi.fn();
const mockIsSettingsWindowWebContents = vi.fn();

let daemonEventHandler: DaemonEventHandler | null = null;

vi.mock('electron', () => ({
  ipcMain: {
    handle: (event: string, handler: Handler) => {
      ipcHandle[event] = handler;
    },
  },
}));

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...args: unknown[]) => mockDaemonCall(...args),
    onEvent: (handler: DaemonEventHandler) => {
      daemonEventHandler = handler;
    },
  },
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
}));

vi.mock('@/main/settings/window', () => ({
  isSettingsWindowWebContents: (...args: unknown[]) =>
    mockIsSettingsWindowWebContents(...args),
}));

vi.mock('@/main/capture/video/camera-preview', () => ({
  showCameraPreview: (...args: unknown[]) => mockShowCameraPreview(...args),
  hideCameraPreview: (...args: unknown[]) => mockHideCameraPreview(...args),
  isCameraPreviewVisible: () => mockIsCameraPreviewVisible(),
}));

vi.mock('@/main/capture/video/permissions', () => ({
  checkAndRequestMicrophonePermission: () => mockMicPermission(),
  checkAndRequestCameraPermission: () => mockCameraPermission(),
}));

function createSender() {
  const listeners: Record<string, () => void> = {};
  let destroyed = false;
  return {
    send: vi.fn(),
    isDestroyed: vi.fn(() => destroyed),
    once: vi.fn((event: string, callback: () => void) => {
      listeners[event] = callback;
    }),
    emitDestroyed: () => {
      destroyed = true;
      listeners['destroyed']?.();
    },
  };
}

const baseCamera = {
  enabled: false,
  selectedDeviceId: 'config-camera',
  selectedDeviceName: 'Config Camera',
  shape: 'rounded',
  size: 'large',
  position: null,
  resolution: '720p',
  flipped: false,
};

async function loadAndInit() {
  const m = await import('@/main/devices');
  m.init();
  return m;
}

describe('devices IPC handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandle).forEach(key => delete ipcHandle[key]);
    daemonEventHandler = null;
    mockGetConfig.mockReturnValue({ recording: { camera: baseCamera } });
    mockMicPermission.mockResolvedValue(true);
    mockCameraPermission.mockResolvedValue(true);
    mockIsCameraPreviewVisible.mockReturnValue(false);
    mockShowCameraPreview.mockResolvedValue(undefined);
    mockDaemonCall.mockResolvedValue({});
    mockIsSettingsWindowWebContents.mockReturnValue(true);
  });

  describe('devices:list', () => {
    it('returns device lists from the daemon', async () => {
      mockDaemonCall.mockResolvedValue({
        microphones: [{ id: 'mic-1', label: 'Mic 1' }],
        cameras: [{ id: 'cam-1', label: 'Cam 1' }],
        defaultMicrophoneId: 'mic-1',
        defaultCameraId: 'cam-1',
      });
      await loadAndInit();

      const result = await ipcHandle['devices:list']({
        sender: createSender(),
      });

      expect(mockDaemonCall).toHaveBeenCalledWith('media-devices', 'list');
      expect(result).toEqual({
        microphones: [{ id: 'mic-1', label: 'Mic 1' }],
        cameras: [{ id: 'cam-1', label: 'Cam 1' }],
        defaultMicrophoneId: 'mic-1',
        defaultCameraId: 'cam-1',
      });
    });

    it('defaults to empty lists when the daemon omits them', async () => {
      mockDaemonCall.mockResolvedValue({});
      await loadAndInit();

      const result = await ipcHandle['devices:list']({
        sender: createSender(),
      });

      expect(result).toEqual({
        microphones: [],
        cameras: [],
        defaultMicrophoneId: null,
        defaultCameraId: null,
      });
    });
  });

  it('rejects device access from non-settings windows', async () => {
    mockIsSettingsWindowWebContents.mockReturnValue(false);
    await loadAndInit();
    const sender = createSender();

    const devices = await ipcHandle['devices:list']({ sender });
    const micStarted = await ipcHandle['devices:mic-test:start'](
      { sender },
      { deviceId: 'mic-1', deviceName: 'Mic 1' }
    );
    const cameraStarted = await ipcHandle['devices:camera-test:start'](
      { sender },
      { deviceId: 'cam-1', deviceName: 'Cam 1' }
    );
    const micStopped = await ipcHandle['devices:mic-test:stop']({ sender });
    const cameraStopped = await ipcHandle['devices:camera-test:stop']({
      sender,
    });

    expect(devices).toEqual({
      microphones: [],
      cameras: [],
      defaultMicrophoneId: null,
      defaultCameraId: null,
    });
    expect(micStarted).toBe(false);
    expect(cameraStarted).toBe(false);
    expect(micStopped).toBe(false);
    expect(cameraStopped).toBe(false);
    expect(mockDaemonCall).not.toHaveBeenCalled();
    expect(mockMicPermission).not.toHaveBeenCalled();
    expect(mockCameraPermission).not.toHaveBeenCalled();
    expect(mockShowCameraPreview).not.toHaveBeenCalled();
  });

  describe('devices:mic-test', () => {
    it('does not start when microphone permission is denied', async () => {
      mockMicPermission.mockResolvedValue(false);
      await loadAndInit();
      const sender = createSender();

      const result = await ipcHandle['devices:mic-test:start'](
        { sender },
        { deviceId: 'mic-1', deviceName: 'Mic 1' }
      );

      expect(result).toBe(false);
      expect(mockDaemonCall).not.toHaveBeenCalled();
    });

    it('starts the daemon mic test and forwards level events', async () => {
      await loadAndInit();
      const sender = createSender();

      const result = await ipcHandle['devices:mic-test:start'](
        { sender },
        { deviceId: 'mic-1', deviceName: 'Mic 1' }
      );

      expect(result).toBe(true);
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'media-devices',
        'startMicTest',
        { deviceId: 'mic-1', deviceName: 'Mic 1' }
      );

      daemonEventHandler?.('media-devices:mic-level', { level: 0.42 });
      expect(sender.send).toHaveBeenCalledWith('devices:mic-test:level', 0.42);
    });

    it('ignores level events when no test is running', async () => {
      await loadAndInit();
      const sender = createSender();

      daemonEventHandler?.('media-devices:mic-level', { level: 0.5 });
      expect(sender.send).not.toHaveBeenCalled();
    });

    it('stops the daemon mic test and stops forwarding', async () => {
      await loadAndInit();
      const sender = createSender();
      await ipcHandle['devices:mic-test:start'](
        { sender },
        { deviceId: null, deviceName: null }
      );

      await ipcHandle['devices:mic-test:stop']({ sender });

      expect(mockDaemonCall).toHaveBeenCalledWith(
        'media-devices',
        'stopMicTest'
      );
      daemonEventHandler?.('media-devices:mic-level', { level: 0.5 });
      expect(sender.send).not.toHaveBeenCalled();
    });

    it('stops the mic test when the sender is destroyed', async () => {
      await loadAndInit();
      const sender = createSender();
      await ipcHandle['devices:mic-test:start'](
        { sender },
        { deviceId: null, deviceName: null }
      );

      sender.emitDestroyed();
      await Promise.resolve();

      expect(mockDaemonCall).toHaveBeenCalledWith(
        'media-devices',
        'stopMicTest'
      );
    });

    it('cancels mic startup when the sender is destroyed during permission', async () => {
      let resolvePermission: (granted: boolean) => void = () => {};
      mockMicPermission.mockReturnValueOnce(
        new Promise(resolve => {
          resolvePermission = resolve;
        })
      );
      await loadAndInit();
      const sender = createSender();

      const starting = ipcHandle['devices:mic-test:start'](
        { sender },
        { deviceId: 'mic-1', deviceName: 'Mic 1' }
      );
      sender.emitDestroyed();
      resolvePermission(true);

      expect(await starting).toBe(false);
      expect(mockDaemonCall).not.toHaveBeenCalled();
    });

    it('cancels pending mic startup when the component stops it', async () => {
      let resolvePermission: (granted: boolean) => void = () => {};
      mockMicPermission.mockReturnValueOnce(
        new Promise(resolve => {
          resolvePermission = resolve;
        })
      );
      await loadAndInit();
      const sender = createSender();

      const starting = ipcHandle['devices:mic-test:start'](
        { sender },
        { deviceId: 'mic-1', deviceName: 'Mic 1' }
      );
      await ipcHandle['devices:mic-test:stop']({ sender });
      resolvePermission(true);

      expect(await starting).toBe(false);
      expect(mockDaemonCall).not.toHaveBeenCalled();
    });
  });

  describe('devices:camera-test', () => {
    it('does not start when camera permission is denied', async () => {
      mockCameraPermission.mockResolvedValue(false);
      await loadAndInit();
      const sender = createSender();

      const result = await ipcHandle['devices:camera-test:start'](
        { sender },
        { deviceId: 'cam-1', deviceName: 'Cam 1' }
      );

      expect(result).toBe(false);
      expect(mockShowCameraPreview).not.toHaveBeenCalled();
    });

    it('shows the native preview with the requested device', async () => {
      await loadAndInit();
      const sender = createSender();

      const result = await ipcHandle['devices:camera-test:start'](
        { sender },
        { deviceId: 'cam-1', deviceName: 'Cam 1', flipped: true }
      );

      expect(result).toBe(true);
      expect(mockShowCameraPreview).toHaveBeenCalledWith({
        ...baseCamera,
        selectedDeviceId: 'cam-1',
        selectedDeviceName: 'Cam 1',
        flipped: true,
      });
    });

    it('hides the preview when the test stops', async () => {
      await loadAndInit();
      const sender = createSender();
      await ipcHandle['devices:camera-test:start'](
        { sender },
        { deviceId: 'cam-1', deviceName: 'Cam 1' }
      );

      await ipcHandle['devices:camera-test:stop']({ sender });

      expect(mockHideCameraPreview).toHaveBeenCalled();
    });

    it('restores the configured preview when it was visible before the test', async () => {
      mockIsCameraPreviewVisible.mockReturnValue(true);
      mockGetConfig.mockReturnValue({
        recording: { camera: { ...baseCamera, enabled: true } },
      });
      await loadAndInit();
      const sender = createSender();
      await ipcHandle['devices:camera-test:start'](
        { sender },
        { deviceId: 'cam-1', deviceName: 'Cam 1' }
      );
      mockShowCameraPreview.mockClear();

      await ipcHandle['devices:camera-test:stop']({ sender });

      expect(mockHideCameraPreview).not.toHaveBeenCalled();
      expect(mockShowCameraPreview).toHaveBeenCalledWith({
        ...baseCamera,
        enabled: true,
      });
    });

    it('hides the preview when the sender is destroyed', async () => {
      await loadAndInit();
      const sender = createSender();
      await ipcHandle['devices:camera-test:start'](
        { sender },
        { deviceId: 'cam-1', deviceName: 'Cam 1' }
      );

      sender.emitDestroyed();
      await Promise.resolve();

      expect(mockHideCameraPreview).toHaveBeenCalled();
    });

    it('cancels camera startup when the sender is destroyed during native show', async () => {
      let finishShow: () => void = () => {};
      mockShowCameraPreview.mockReturnValueOnce(
        new Promise<void>(resolve => {
          finishShow = resolve;
        })
      );
      await loadAndInit();
      const sender = createSender();

      const starting = ipcHandle['devices:camera-test:start'](
        { sender },
        { deviceId: 'cam-1', deviceName: 'Cam 1' }
      );
      await vi.waitFor(() => {
        expect(mockShowCameraPreview).toHaveBeenCalled();
      });
      sender.emitDestroyed();
      finishShow();

      expect(await starting).toBe(false);
      expect(mockHideCameraPreview).toHaveBeenCalled();
    });

    it('cancels pending camera startup when the component stops it', async () => {
      let resolvePermission: (granted: boolean) => void = () => {};
      mockCameraPermission.mockReturnValueOnce(
        new Promise(resolve => {
          resolvePermission = resolve;
        })
      );
      await loadAndInit();
      const sender = createSender();

      const starting = ipcHandle['devices:camera-test:start'](
        { sender },
        { deviceId: 'cam-1', deviceName: 'Cam 1' }
      );
      await ipcHandle['devices:camera-test:stop']({ sender });
      resolvePermission(true);

      expect(await starting).toBe(false);
      expect(mockShowCameraPreview).not.toHaveBeenCalled();
    });

    it('does not stop a newer camera test when an old sender is destroyed', async () => {
      await loadAndInit();
      const oldSender = createSender();
      const currentSender = createSender();
      await ipcHandle['devices:mic-test:start'](
        { sender: oldSender },
        { deviceId: null, deviceName: null }
      );
      await ipcHandle['devices:mic-test:stop']({ sender: oldSender });
      await ipcHandle['devices:camera-test:start'](
        { sender: currentSender },
        { deviceId: 'cam-1', deviceName: 'Cam 1' }
      );
      mockHideCameraPreview.mockClear();

      oldSender.emitDestroyed();
      await Promise.resolve();

      expect(mockHideCameraPreview).not.toHaveBeenCalled();
    });

    it('keeps a newer pending camera test when the old sender is destroyed', async () => {
      await loadAndInit();
      const oldSender = createSender();
      const currentSender = createSender();
      await ipcHandle['devices:camera-test:start'](
        { sender: oldSender },
        { deviceId: 'cam-1', deviceName: 'Cam 1' }
      );
      let resolvePermission: (granted: boolean) => void = () => {};
      mockCameraPermission.mockReturnValueOnce(
        new Promise(resolve => {
          resolvePermission = resolve;
        })
      );
      const starting = ipcHandle['devices:camera-test:start'](
        { sender: currentSender },
        { deviceId: 'cam-2', deviceName: 'Cam 2' }
      );
      mockShowCameraPreview.mockClear();
      mockHideCameraPreview.mockClear();

      oldSender.emitDestroyed();
      resolvePermission(true);

      expect(await starting).toBe(true);
      expect(mockShowCameraPreview).toHaveBeenCalledWith({
        ...baseCamera,
        selectedDeviceId: 'cam-2',
        selectedDeviceName: 'Cam 2',
      });
      expect(mockHideCameraPreview).not.toHaveBeenCalled();
    });

    it('restores the configured preview when a camera test fails to start', async () => {
      mockIsCameraPreviewVisible.mockReturnValue(true);
      mockGetConfig.mockReturnValue({
        recording: { camera: { ...baseCamera, enabled: true } },
      });
      mockShowCameraPreview
        .mockRejectedValueOnce(new Error('camera failed'))
        .mockResolvedValueOnce(undefined);
      await loadAndInit();
      const sender = createSender();

      await expect(
        ipcHandle['devices:camera-test:start'](
          { sender },
          { deviceId: 'cam-1', deviceName: 'Cam 1' }
        )
      ).rejects.toThrow('camera failed');

      expect(mockShowCameraPreview).toHaveBeenLastCalledWith({
        ...baseCamera,
        enabled: true,
      });
    });
  });
});
