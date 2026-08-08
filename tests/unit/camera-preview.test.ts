import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

let daemonEventHandler: ((e: string, d: unknown) => void) | null = null;
const ipcHandle: Record<string, (...a: unknown[]) => unknown> = {};

const mockDaemonCall = vi.fn();
const mockDipToScreenPoint = vi.fn();
const mockScreenToDipPoint = vi.fn();

vi.mock('electron', () => ({
  ipcMain: {
    handle: (e: string, h: (...a: unknown[]) => unknown) => {
      ipcHandle[e] = h;
    },
  },
  screen: {
    dipToScreenPoint: (...a: unknown[]) => mockDipToScreenPoint(...a),
    screenToDipPoint: (...a: unknown[]) => mockScreenToDipPoint(...a),
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
  flipped: true,
};

describe('camera-preview', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
    daemonEventHandler = null;
    mockDaemonCall.mockResolvedValue(null);
    mockDipToScreenPoint.mockImplementation(position => position);
    mockScreenToDipPoint.mockImplementation(position => position);
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
    vi.resetModules();
  });

  it('showCameraPreview calls daemon show', async () => {
    const m = await import('@/main/capture/video/camera-preview');
    await m.showCameraPreview(sampleSettings);
    expect(mockDaemonCall).toHaveBeenCalledWith(
      'camera-preview',
      'show',
      expect.objectContaining({ deviceId: 'cam-1', flipped: true })
    );
    expect(m.isCameraPreviewVisible()).toBe(true);
  });

  it('showCameraPreview defaults to 720p when no resolution', async () => {
    const m = await import('@/main/capture/video/camera-preview');
    await m.showCameraPreview({ ...sampleSettings, resolution: undefined });
    const args = mockDaemonCall.mock.calls[0][2] as Record<string, unknown>;
    expect(args.resolution).toBe('720p');
  });

  it('keeps the preview hidden when the daemon rejects show', async () => {
    mockDaemonCall
      .mockRejectedValueOnce(new Error('camera failed'))
      .mockResolvedValueOnce(null);
    const m = await import('@/main/capture/video/camera-preview');

    await expect(m.showCameraPreview(sampleSettings)).rejects.toThrow(
      'camera failed'
    );
    expect(m.isCameraPreviewVisible()).toBe(false);
    expect(mockDaemonCall).toHaveBeenCalledWith('camera-preview', 'hide');
  });

  it('hideCameraPreview calls daemon hide and clears settings', async () => {
    const m = await import('@/main/capture/video/camera-preview');
    await m.showCameraPreview(sampleSettings);
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
      expect.objectContaining({ x: 100, y: 200, flipped: true })
    );
  });

  it('round-trips Windows camera positions through physical pixels', async () => {
    Object.defineProperty(process, 'platform', { value: 'win32' });
    vi.resetModules();
    mockDipToScreenPoint.mockReturnValue({ x: 200, y: 400 });
    mockScreenToDipPoint.mockReturnValue({ x: 100, y: 200 });
    mockDaemonCall.mockResolvedValue({ x: 200, y: 400 });

    const m = await import('@/main/capture/video/camera-preview');
    await m.showCameraPreview(sampleSettings);
    expect(mockDaemonCall).toHaveBeenCalledWith(
      'camera-preview',
      'show',
      expect.objectContaining({ x: 200, y: 400 })
    );
    expect(await m.getCameraPreviewPosition()).toEqual({ x: 100, y: 200 });

    m.registerCameraPreviewIpcHandlers();
    daemonEventHandler!('camera-preview:position-changed', { x: 200, y: 400 });
    const settings = ipcHandle['camera:get-settings']() as {
      position: { x: number; y: number };
    };
    expect(settings.position).toEqual({ x: 100, y: 200 });
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
      await m.enableCameraContentProtection();
      expect(m.isCameraContentProtectionEnabled()).toBe(true);
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'camera-preview',
        'setContentProtection',
        { enabled: true }
      );
    });

    it('disableCameraContentProtection clears flag', async () => {
      const m = await import('@/main/capture/video/camera-preview');
      await m.enableCameraContentProtection();
      await m.disableCameraContentProtection();
      expect(m.isCameraContentProtectionEnabled()).toBe(false);
    });

    it('enableCameraContentProtection rejects and resets the flag', async () => {
      mockDaemonCall.mockRejectedValue(new Error('protection failed'));
      const m = await import('@/main/capture/video/camera-preview');
      await expect(m.enableCameraContentProtection()).rejects.toThrow(
        'protection failed'
      );
      expect(m.isCameraContentProtectionEnabled()).toBe(false);
    });

    it('showCameraPreview re-enables content protection when flagged', async () => {
      const m = await import('@/main/capture/video/camera-preview');
      await m.enableCameraContentProtection();
      mockDaemonCall.mockClear();
      await m.showCameraPreview(sampleSettings);
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
      await m.showCameraPreview(sampleSettings);
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
      await m.showCameraPreview(sampleSettings);
      expect(ipcHandle['camera:get-settings']()).not.toBeNull();
    });
  });
});
