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
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  it('copies detected QR payload', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ payload: 'https://example.com' });
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    expect(mockClipboardWriteText).toHaveBeenCalledWith('https://example.com');
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('notifies when no QR detected', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ payload: '' });
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    expect(mockClipboardWriteText).not.toHaveBeenCalled();
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('bails on screencapture error', async () => {
    mockExecFile.mockImplementation((_file, _args, cb) =>
      cb(new Error('cap fail'), '', '')
    );
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it('recognizes a captured file when screencapture also writes stderr', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ payload: 'captured value' });
    mockExecFile.mockImplementation((_file, _args, cb) =>
      cb(null, '', 'capture warning')
    );
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    expect(mockDaemonCall).toHaveBeenCalled();
    expect(mockClipboardWriteText).toHaveBeenCalledWith('captured value');
  });

  it('bails when temp file missing', async () => {
    mockFsExistsSync.mockReturnValue(false);
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it('shows failure notification on daemon error', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockRejectedValue(new Error('qr crash'));
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('hides and restores desktop icons when enabled', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ payload: 'x' });
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    expect(mockHideDesktopIcons).toHaveBeenCalledWith('capture');
    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
  });

  it('awaits the native capture command', async () => {
    let finishCapture: ExecCallback = () => {};
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ payload: 'x' });
    mockExecFile.mockImplementation((_file, _args, cb) => {
      finishCapture = cb;
    });
    const scan = (await import('@/main/capture/qrcode')).default;
    const scanning = scan();

    await Promise.resolve();
    expect(mockDaemonCall).not.toHaveBeenCalled();

    finishCapture(null, '', '');
    await scanning;

    expect(mockExecFile).toHaveBeenCalledWith(
      'screencapture',
      ['-i', '-x', '-t', 'png', expect.stringContaining('poratake-qrcode-')],
      expect.any(Function)
    );
    expect(mockDaemonCall).toHaveBeenCalled();
  });

  it('uses a unique temporary path for every scan', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ payload: 'x' });
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const scan = (await import('@/main/capture/qrcode')).default;

    await scan();
    await scan();

    expect(mockExecFile.mock.calls[0][1][4]).not.toBe(
      mockExecFile.mock.calls[1][1][4]
    );
  });

  it('keeps the copied payload when temporary-file cleanup fails', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockFsUnlinkSync.mockImplementation(() => {
      throw new Error('file locked');
    });
    mockDaemonCall.mockResolvedValue({ payload: 'copied value' });
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const scan = (await import('@/main/capture/qrcode')).default;

    await scan();

    expect(mockClipboardWriteText).toHaveBeenCalledWith('copied value');
  });

  it('ignores a second scan while the first is active', async () => {
    let finishCapture: ExecCallback = () => {};
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ payload: 'x' });
    mockExecFile.mockImplementation((_file, _args, cb) => {
      finishCapture = cb;
    });
    const scan = (await import('@/main/capture/qrcode')).default;

    const firstScan = scan();
    await Promise.resolve();
    const secondScan = scan();

    expect(mockExecFile).toHaveBeenCalledTimes(1);
    expect(mockHideDesktopIcons).toHaveBeenCalledTimes(1);

    finishCapture(null, '', '');
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
