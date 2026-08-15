import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

type ExecCallback = (
  error: Error | null,
  stdout: string,
  stderr: string
) => void;
const mockExecFile =
  vi.fn<(file: string, args: string[], callback: ExecCallback) => void>();
const mockClipboardWriteText = vi.fn();
const mockNotificationShow = vi.fn();
const mockGetConfig = vi.fn();
const mockHideDesktopIcons = vi.fn();
const mockShowDesktopIcons = vi.fn();
const mockIsDesktopIconsSupported = vi.fn();
const mockDaemonCall = vi.fn();
const mockFsExistsSync = vi.fn();
const mockFsUnlinkSync = vi.fn();

vi.mock('child_process', () => ({
  execFile: (file: string, args: string[], cb: ExecCallback) =>
    mockExecFile(file, args, cb),
}));

class MockNotification {
  constructor(_args: unknown) {
    void _args;
  }
  show() {
    mockNotificationShow();
  }
}

vi.mock('electron', () => ({
  app: { getPath: () => '/tmp' },
  clipboard: { writeText: (...a: unknown[]) => mockClipboardWriteText(...a) },
  Notification: MockNotification,
}));

vi.mock('fs', () => ({
  default: {
    existsSync: (...a: unknown[]) => mockFsExistsSync(...a),
    unlinkSync: (...a: unknown[]) => mockFsUnlinkSync(...a),
  },
  existsSync: (...a: unknown[]) => mockFsExistsSync(...a),
  unlinkSync: (...a: unknown[]) => mockFsUnlinkSync(...a),
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
}));

vi.mock('@/main/capture/desktop-icons', () => ({
  hideDesktopIcons: (...a: unknown[]) => mockHideDesktopIcons(...a),
  showDesktopIcons: (...a: unknown[]) => mockShowDesktopIcons(...a),
  checkAccessibilityPermission: () => true,
  isSupported: () => mockIsDesktopIconsSupported(),
}));

vi.mock('@/main/daemon', () => ({
  daemon: { call: (...a: unknown[]) => mockDaemonCall(...a) },
}));

const mockCaptureAreaToFile = vi.fn();

vi.mock('@/main/capture/area-overlay', () => ({
  captureAreaToFile: (...a: unknown[]) => mockCaptureAreaToFile(...a),
}));

describe('scanQRCode', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.resetAllMocks();
    vi.resetModules();
    Object.defineProperty(process, 'platform', { value: 'darwin' });
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: false } });
    mockIsDesktopIconsSupported.mockReturnValue(true);
    mockCaptureAreaToFile.mockResolvedValue(true);
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  it('uses the unified area overlay instead of screencapture', async () => {
    mockDaemonCall.mockResolvedValue({ payload: 'https://example.com' });
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    expect(mockExecFile).not.toHaveBeenCalled();
    expect(mockCaptureAreaToFile).toHaveBeenCalledWith(
      expect.stringContaining('poratake-qrcode-')
    );
    expect(mockClipboardWriteText).toHaveBeenCalledWith('https://example.com');
  });

  it('notifies when no QR detected', async () => {
    mockDaemonCall.mockResolvedValue({ payload: '' });
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    expect(mockClipboardWriteText).not.toHaveBeenCalled();
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('bails when the area capture is cancelled', async () => {
    mockCaptureAreaToFile.mockResolvedValue(false);
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it('reports capture failures with a notification', async () => {
    mockCaptureAreaToFile.mockRejectedValue(new Error('capture failed'));
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('hides and restores desktop icons when enabled', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockDaemonCall.mockResolvedValue({ payload: 'x' });
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    expect(mockHideDesktopIcons).toHaveBeenCalledWith('capture');
    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
  });

  it('uses a unique temporary path for every scan', async () => {
    mockDaemonCall.mockResolvedValue({ payload: 'x' });
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    await scan();
    expect(mockCaptureAreaToFile.mock.calls[0][0]).not.toBe(
      mockCaptureAreaToFile.mock.calls[1][0]
    );
  });

  it('ignores a second scan while the first is active', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockDaemonCall.mockResolvedValue({ payload: 'x' });
    let finishCapture: (value: boolean) => void = () => {};
    mockCaptureAreaToFile.mockReturnValue(
      new Promise<boolean>(resolve => {
        finishCapture = resolve;
      })
    );
    const scan = (await import('@/main/capture/qrcode')).default;

    const firstScan = scan();
    await Promise.resolve();
    const secondScan = scan();

    expect(mockCaptureAreaToFile).toHaveBeenCalledTimes(1);
    expect(mockHideDesktopIcons).toHaveBeenCalledTimes(1);

    finishCapture(true);
    await Promise.all([firstScan, secondScan]);

    expect(mockShowDesktopIcons).toHaveBeenCalledTimes(1);
  });
});

describe('scanQRCode on Windows', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.resetAllMocks();
    vi.resetModules();
    Object.defineProperty(process, 'platform', { value: 'win32' });
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: false } });
    mockIsDesktopIconsSupported.mockReturnValue(true);
  });

  afterEach(() => {
    vi.doUnmock('@/main/system/capabilities');
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  it('uses the shared area capture path and daemon detect on win32', async () => {
    mockCaptureAreaToFile.mockResolvedValue(true);
    mockDaemonCall.mockResolvedValue({
      payload: 'https://example.com/windows',
    });

    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();

    expect(mockExecFile).not.toHaveBeenCalled();
    expect(mockCaptureAreaToFile).toHaveBeenCalledWith(
      expect.stringContaining('poratake-qrcode-')
    );
    expect(mockDaemonCall).toHaveBeenCalledWith('qrcode', 'detect', {
      imagePath: expect.stringContaining('poratake-qrcode-'),
    });
    expect(mockClipboardWriteText).toHaveBeenCalledWith(
      'https://example.com/windows'
    );
  });

  it('does not call the daemon when area capture is cancelled', async () => {
    mockCaptureAreaToFile.mockResolvedValue(false);

    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();

    expect(mockDaemonCall).not.toHaveBeenCalled();
    expect(mockFsUnlinkSync).not.toHaveBeenCalled();
  });

  it('cleans up a partial file when area capture is cancelled', async () => {
    mockCaptureAreaToFile.mockResolvedValue(false);
    mockFsExistsSync.mockReturnValue(true);

    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();

    expect(mockFsUnlinkSync).toHaveBeenCalled();
  });

  it('reports capture failures and restores desktop icons', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockCaptureAreaToFile.mockRejectedValue(new Error('capture failed'));
    mockFsExistsSync.mockReturnValue(true);

    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();

    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
    expect(mockFsUnlinkSync).toHaveBeenCalled();
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('restores desktop icons before QR detection finishes', async () => {
    let finishDetection: (result: { payload: string }) => void = () => {};
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockCaptureAreaToFile.mockResolvedValue(true);
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockReturnValue(
      new Promise(resolve => {
        finishDetection = resolve;
      })
    );
    const scan = (await import('@/main/capture/qrcode')).default;

    const scanning = scan();
    await vi.waitFor(() => {
      expect(mockDaemonCall).toHaveBeenCalled();
    });

    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
    finishDetection({ payload: 'x' });
    await scanning;
  });
});
