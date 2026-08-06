import { describe, it, expect, vi, beforeEach } from 'vitest';

let daemonEventHandler: ((e: string, d: unknown) => void) | null = null;
const ipcHandle: Record<string, (...a: unknown[]) => unknown> = {};

const mockDaemonCall = vi.fn();

vi.mock('electron', () => ({
  ipcMain: {
    handle: (e: string, h: (...a: unknown[]) => unknown) => {
      ipcHandle[e] = h;
    },
  },
}));

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...a: unknown[]) => mockDaemonCall(...a),
    onEvent: (cb: (e: string, d: unknown) => void) => {
      daemonEventHandler = cb;
    },
    offEvent: vi.fn(),
  },
}));

const sampleSettings = {
  enabled: true,
  selectedDeviceId: 'cam-1',
  selectedDeviceName: 'FaceTime HD',
  resolution: '1080p',
  position: { x: 100, y: 200 },
};

describe('camera-preview', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
    daemonEventHandler = null;
    mockDaemonCall.mockResolvedValue(null);
  });

  it('showCameraPreview calls daemon show', async () => {
    const m = await import('@/main/capture/video/camera-preview');
    m.showCameraPreview(sampleSettings);
    expect(mockDaemonCall).toHaveBeenCalledWith(
      'camera-preview',
      'show',
      expect.objectContaining({ deviceId: 'cam-1' })
    );
    expect(m.isCameraPreviewVisible()).toBe(true);
  });

  it('showCameraPreview defaults to 720p when no resolution', async () => {
    const m = await import('@/main/capture/video/camera-preview');
    m.showCameraPreview({ ...sampleSettings, resolution: undefined });
    const args = mockDaemonCall.mock.calls[0][2] as Record<string, unknown>;
    expect(args.resolution).toBe('720p');
  });

  it('hideCameraPreview calls daemon hide and clears settings', async () => {
    const m = await import('@/main/capture/video/camera-preview');
    m.showCameraPreview(sampleSettings);
    m.hideCameraPreview();
    expect(mockDaemonCall).toHaveBeenCalledWith('camera-preview', 'hide');
    expect(m.isCameraPreviewVisible()).toBe(false);
  });

  it('updateCameraPreview calls daemon update with position', async () => {
    const m = await import('@/main/capture/video/camera-preview');
    m.updateCameraPreview(sampleSettings);
    expect(mockDaemonCall).toHaveBeenCalledWith(
      'camera-preview',
      'update',
      expect.objectContaining({ x: 100, y: 200 })
    );
  });

  it('getCameraPreviewWindow returns null', async () => {
    const m = await import('@/main/capture/video/camera-preview');
    expect(m.getCameraPreviewWindow()).toBeNull();
  });

  describe('getCameraPreviewPosition', () => {
    it('returns position from daemon', async () => {
      mockDaemonCall.mockResolvedValue({ x: 50, y: 60 });
      const m = await import('@/main/capture/video/camera-preview');
      expect(await m.getCameraPreviewPosition()).toEqual({ x: 50, y: 60 });
    });

    it('returns null on invalid response', async () => {
      mockDaemonCall.mockResolvedValue({ x: 'bad' });
      const m = await import('@/main/capture/video/camera-preview');
      expect(await m.getCameraPreviewPosition()).toBeNull();
    });

    it('returns null on daemon error', async () => {
      mockDaemonCall.mockRejectedValue(new Error('boom'));
      const m = await import('@/main/capture/video/camera-preview');
      expect(await m.getCameraPreviewPosition()).toBeNull();
    });
  });

  describe('content protection', () => {
    it('enableCameraContentProtection sets flag and calls daemon', async () => {
      const m = await import('@/main/capture/video/camera-preview');
      m.enableCameraContentProtection();
      expect(m.isCameraContentProtectionEnabled()).toBe(true);
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'camera-preview',
        'setContentProtection',
        { enabled: true }
      );
    });

    it('disableCameraContentProtection clears flag', async () => {
      const m = await import('@/main/capture/video/camera-preview');
      m.enableCameraContentProtection();
      m.disableCameraContentProtection();
      expect(m.isCameraContentProtectionEnabled()).toBe(false);
    });

    it('showCameraPreview re-enables content protection when flagged', async () => {
      const m = await import('@/main/capture/video/camera-preview');
      m.enableCameraContentProtection();
      mockDaemonCall.mockClear();
      m.showCameraPreview(sampleSettings);
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'camera-preview',
        'setContentProtection',
        { enabled: true }
      );
    });
  });

  describe('registerCameraPreviewIpcHandlers', () => {
    it('registers position-changed event handler', async () => {
      const m = await import('@/main/capture/video/camera-preview');
      m.registerCameraPreviewIpcHandlers();
      expect(daemonEventHandler).toBeDefined();
    });

    it('updates position on event', async () => {
      const m = await import('@/main/capture/video/camera-preview');
      m.showCameraPreview(sampleSettings);
      m.registerCameraPreviewIpcHandlers();
      daemonEventHandler!('camera-preview:position-changed', { x: 5, y: 7 });
      const settings = ipcHandle['camera:get-settings']() as Record<
        string,
        unknown
      >;
      expect(settings.position).toEqual({ x: 5, y: 7 });
    });

    it('camera:get-settings returns current settings', async () => {
      const m = await import('@/main/capture/video/camera-preview');
      m.registerCameraPreviewIpcHandlers();
      expect(ipcHandle['camera:get-settings']()).toBeNull();
      m.showCameraPreview(sampleSettings);
      expect(ipcHandle['camera:get-settings']()).not.toBeNull();
    });
  });
});
