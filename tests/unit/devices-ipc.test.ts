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
  return {
    send: vi.fn(),
    isDestroyed: vi.fn(() => false),
    once: vi.fn((event: string, callback: () => void) => {
      listeners[event] = callback;
    }),
    emitDestroyed: () => listeners['destroyed']?.(),
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
  });

  describe('devices:list', () => {
    it('returns device lists from the daemon', async () => {
      mockDaemonCall.mockResolvedValue({
        microphones: [{ id: 'mic-1', label: 'Mic 1' }],
        cameras: [{ id: 'cam-1', label: 'Cam 1' }],
      });
      await loadAndInit();

      const result = await ipcHandle['devices:list']();

      expect(mockDaemonCall).toHaveBeenCalledWith('media-devices', 'list');
      expect(result).toEqual({
        microphones: [{ id: 'mic-1', label: 'Mic 1' }],
        cameras: [{ id: 'cam-1', label: 'Cam 1' }],
      });
    });

    it('defaults to empty lists when the daemon omits them', async () => {
      mockDaemonCall.mockResolvedValue({});
      await loadAndInit();

      const result = await ipcHandle['devices:list']();

      expect(result).toEqual({ microphones: [], cameras: [] });
    });
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

      await ipcHandle['devices:mic-test:stop']();

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

      await ipcHandle['devices:camera-test:stop']();

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

      await ipcHandle['devices:camera-test:stop']();

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
  });
});
