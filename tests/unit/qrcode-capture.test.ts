import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

type ExecCallback = (
  error: Error | null,
  stdout: string,
  stderr: string
) => void;
const mockExec = vi.fn<(command: string, callback: ExecCallback) => void>();
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
  exec: (cmd: string, cb: ExecCallback) => mockExec(cmd, cb),
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

vi.mock('@/main/capture/area-capture', () => ({
  captureAreaToFile: (...a: unknown[]) => mockCaptureAreaToFile(...a),
}));

async function flush(): Promise<void> {
  await new Promise(resolve => setImmediate(resolve));
  await new Promise(resolve => setImmediate(resolve));
}

describe('scanQRCode', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.clearAllMocks();
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
    mockExec.mockImplementation((_c, cb) => cb(null, '', ''));
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    await flush();
    expect(mockClipboardWriteText).toHaveBeenCalledWith('https://example.com');
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('notifies when no QR detected', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ payload: '' });
    mockExec.mockImplementation((_c, cb) => cb(null, '', ''));
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    await flush();
    expect(mockClipboardWriteText).not.toHaveBeenCalled();
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('bails on screencapture error', async () => {
    mockExec.mockImplementation((_c, cb) => cb(new Error('cap fail'), '', ''));
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    await flush();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it('bails on stderr', async () => {
    mockExec.mockImplementation((_c, cb) => cb(null, '', 'err'));
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    await flush();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it('bails when temp file missing', async () => {
    mockFsExistsSync.mockReturnValue(false);
    mockExec.mockImplementation((_c, cb) => cb(null, '', ''));
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    await flush();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it('shows failure notification on daemon error', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockRejectedValue(new Error('qr crash'));
    mockExec.mockImplementation((_c, cb) => cb(null, '', ''));
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    await flush();
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('hides and restores desktop icons when enabled', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ payload: 'x' });
    mockExec.mockImplementation((_c, cb) => cb(null, '', ''));
    const scan = (await import('@/main/capture/qrcode')).default;
    await scan();
    await flush();
    expect(mockHideDesktopIcons).toHaveBeenCalledWith('capture');
    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
  });
});

describe('scanQRCode on Windows', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.clearAllMocks();
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

    expect(mockExec).not.toHaveBeenCalled();
    expect(mockCaptureAreaToFile).toHaveBeenCalledWith(
      expect.stringContaining('capty-qrcode-')
    );
    expect(mockDaemonCall).toHaveBeenCalledWith('qrcode', 'detect', {
      imagePath: expect.stringContaining('capty-qrcode-'),
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
  });
});
