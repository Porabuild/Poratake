import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

type ExecCallback = (
  error: Error | null,
  stdout: string,
  stderr: string
) => void;
const mockExecFile =
  vi.fn<(file: string, args: string[], callback: ExecCallback) => void>();
const mockClipboardWriteText = vi.fn();
const mockNotificationCreate = vi.fn();
const mockNotificationShow = vi.fn();
const mockGetConfig = vi.fn();
const mockHideDesktopIcons = vi.fn();
const mockShowDesktopIcons = vi.fn();
const mockIsDesktopIconsSupported = vi.fn();
const mockDaemonCall = vi.fn();
const mockFsExistsSync = vi.fn();
const mockFsUnlinkSync = vi.fn();
const mockCaptureRegionToFile = vi.fn();
const mockPreprocessImageForOcr = vi.fn();

vi.mock('child_process', () => ({
  execFile: (file: string, args: string[], cb: ExecCallback) =>
    mockExecFile(file, args, cb),
}));

class MockNotification {
  constructor(args: unknown) {
    mockNotificationCreate(args);
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

vi.mock('@/main/utils/ffmpeg', () => ({
  preprocessImageForOcr: (...a: unknown[]) => mockPreprocessImageForOcr(...a),
}));

const mockCaptureAreaToFile = vi.fn();

vi.mock('@/main/capture/area-overlay', () => ({
  captureAreaToFile: (...a: unknown[]) => mockCaptureAreaToFile(...a),
}));

vi.mock('@/main/capture/screenshot/native-capture', () => ({
  captureRegionToFile: (...a: unknown[]) => mockCaptureRegionToFile(...a),
}));

async function flush(): Promise<void> {
  await new Promise(resolve => setImmediate(resolve));
  await new Promise(resolve => setImmediate(resolve));
}

describe('captureText (OCR)', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.resetModules();
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: false } });
    mockIsDesktopIconsSupported.mockReturnValue(true);
    mockPreprocessImageForOcr.mockResolvedValue(false);
  });

  it('writes detected text to clipboard and shows notification', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ text: '  Hello world  ' });
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    await flush();
    expect(mockClipboardWriteText).toHaveBeenCalledWith('Hello world');
    expect(mockNotificationCreate).toHaveBeenCalledWith({
      title: 'Text copied!',
      body: 'Recognized text has been copied to the clipboard',
    });
    expect(mockNotificationShow).toHaveBeenCalled();
    expect(mockFsUnlinkSync).toHaveBeenCalled();
  });

  it('notifies when no text detected', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ text: '' });
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    await flush();
    expect(mockClipboardWriteText).not.toHaveBeenCalled();
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('bails out when screencapture errors', async () => {
    mockExecFile.mockImplementation((_file, _args, cb) =>
      cb(new Error('cap failed'), '', '')
    );
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    await flush();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it('recognizes a captured file when screencapture also writes stderr', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ text: 'Captured text' });
    mockExecFile.mockImplementation((_file, _args, cb) =>
      cb(null, '', 'capture warning')
    );
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    expect(mockDaemonCall).toHaveBeenCalled();
    expect(mockClipboardWriteText).toHaveBeenCalledWith('Captured text');
  });

  it('bails out when temp screenshot is missing', async () => {
    mockFsExistsSync.mockReturnValue(false);
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    await flush();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it('shows failure notification on daemon error', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockRejectedValue(new Error('ocr crash'));
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    await flush();
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('keeps recognized text when temporary-file cleanup fails', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockFsUnlinkSync.mockImplementation(() => {
      throw new Error('file locked');
    });
    mockDaemonCall.mockResolvedValue({ text: 'Recognized text' });
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const captureText = (await import('@/main/capture/ocr')).default;

    await captureText();

    expect(mockClipboardWriteText).toHaveBeenCalledWith('Recognized text');
  });

  it('hides and restores desktop icons when enabled', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ text: 'x' });
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    await flush();
    expect(mockHideDesktopIcons).toHaveBeenCalledWith('capture');
    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
  });

  it('awaits the native capture command', async () => {
    let finishCapture: ExecCallback = () => {};
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ text: 'x' });
    mockExecFile.mockImplementation((_file, _args, cb) => {
      finishCapture = cb;
    });
    const captureText = (await import('@/main/capture/ocr')).default;
    const capture = captureText();

    await Promise.resolve();
    expect(mockDaemonCall).not.toHaveBeenCalled();

    finishCapture(null, '', '');
    await capture;

    expect(mockExecFile).toHaveBeenCalledWith(
      'screencapture',
      ['-i', '-x', '-t', 'png', expect.stringContaining('capty-ocr-')],
      expect.any(Function)
    );
    expect(mockDaemonCall).toHaveBeenCalled();
  });

  it('uses a unique temporary path for each capture', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ text: 'x' });
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const captureText = (await import('@/main/capture/ocr')).default;

    await captureText();
    await captureText();

    const firstPath = mockExecFile.mock.calls[0][1][4];
    const secondPath = mockExecFile.mock.calls[1][1][4];
    expect(firstPath).not.toBe(secondPath);
  });

  it('ignores a second capture while the first is active', async () => {
    let finishCapture: ExecCallback = () => {};
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ text: 'x' });
    mockExecFile.mockImplementation((_file, _args, cb) => {
      finishCapture = cb;
    });
    const captureText = (await import('@/main/capture/ocr')).default;

    const firstCapture = captureText();
    await Promise.resolve();
    const secondCapture = captureText();

    expect(mockExecFile).toHaveBeenCalledTimes(1);
    expect(mockHideDesktopIcons).toHaveBeenCalledTimes(1);

    finishCapture(null, '', '');
    await Promise.all([firstCapture, secondCapture]);

    expect(mockShowDesktopIcons).toHaveBeenCalledTimes(1);
  });

  it('does not overlap OCR and QR selection sessions', async () => {
    let finishCapture: ExecCallback = () => {};
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ text: 'x' });
    mockExecFile.mockImplementation((_file, _args, cb) => {
      finishCapture = cb;
    });
    const captureText = (await import('@/main/capture/ocr')).default;
    const scanQRCode = (await import('@/main/capture/qrcode')).default;

    const capture = captureText();
    await Promise.resolve();
    await scanQRCode();

    expect(mockExecFile).toHaveBeenCalledTimes(1);

    finishCapture(null, '', '');
    await capture;
  });
});

describe('captureText (OCR) on Windows', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.resetAllMocks();
    vi.resetModules();
    Object.defineProperty(process, 'platform', { value: 'win32' });
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: false } });
    mockIsDesktopIconsSupported.mockReturnValue(true);
    mockPreprocessImageForOcr.mockResolvedValue(true);
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  it('captures via the area overlay and recognizes text', async () => {
    mockCaptureAreaToFile.mockResolvedValue(true);
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ text: 'Windows text' });

    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();

    expect(mockExecFile).not.toHaveBeenCalled();
    expect(mockPreprocessImageForOcr).toHaveBeenCalledWith(
      expect.stringContaining('capty-ocr-'),
      expect.stringContaining('capty-ocr-processed-')
    );
    expect(mockDaemonCall).toHaveBeenCalledWith('ocr', 'recognize', {
      imagePath: expect.stringContaining('capty-ocr-processed-'),
    });
    expect(mockClipboardWriteText).toHaveBeenCalledWith('Windows text');
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('captures a provided area without opening another selector', async () => {
    mockCaptureRegionToFile.mockResolvedValue(true);
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ text: 'Selected text' });

    const captureText = (await import('@/main/capture/ocr')).default;
    const area = { x: 10, y: 20, width: 300, height: 100 };
    await captureText(area);

    expect(mockCaptureAreaToFile).not.toHaveBeenCalled();
    expect(mockCaptureRegionToFile).toHaveBeenCalledWith(
      area,
      expect.stringContaining('capty-ocr-')
    );
    expect(mockClipboardWriteText).toHaveBeenCalledWith('Selected text');
  });

  it('releases the selector after copying a retained frame for OCR', async () => {
    mockCaptureRegionToFile.mockResolvedValue(true);
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ text: 'Frozen text' });
    const onCaptured = vi.fn();
    const captureText = (await import('@/main/capture/ocr')).default;
    const area = { x: 10, y: 20, width: 300, height: 100 };

    await captureText(area, { cached: true, onCaptured });

    expect(mockCaptureRegionToFile).toHaveBeenCalledWith(
      area,
      expect.stringContaining('capty-ocr-'),
      { cached: true }
    );
    expect(onCaptured).toHaveBeenCalled();
  });

  it('falls back to the captured image when preprocessing fails', async () => {
    mockCaptureAreaToFile.mockResolvedValue(true);
    mockPreprocessImageForOcr.mockResolvedValue(false);
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ text: 'Fallback text' });

    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();

    expect(mockDaemonCall).toHaveBeenCalledWith('ocr', 'recognize', {
      imagePath: expect.not.stringContaining('capty-ocr-processed-'),
    });
    expect(mockClipboardWriteText).toHaveBeenCalledWith('Fallback text');
  });

  it('bails out when area selection is cancelled', async () => {
    mockCaptureAreaToFile.mockResolvedValue(false);

    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();

    expect(mockDaemonCall).not.toHaveBeenCalled();
    expect(mockClipboardWriteText).not.toHaveBeenCalled();
  });

  it('restores desktop icons even when selection is cancelled', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockCaptureAreaToFile.mockResolvedValue(false);

    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();

    expect(mockHideDesktopIcons).toHaveBeenCalledWith('capture');
    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
  });

  it('restores desktop icons when area capture fails', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockCaptureAreaToFile.mockRejectedValue(new Error('capture failed'));
    mockFsExistsSync.mockReturnValue(true);

    const captureText = (await import('@/main/capture/ocr')).default;

    await captureText();
    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
    expect(mockFsUnlinkSync).toHaveBeenCalled();
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('restores desktop icons before recognition finishes', async () => {
    let finishRecognition: (result: { text: string }) => void = () => {};
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockCaptureAreaToFile.mockResolvedValue(true);
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockReturnValue(
      new Promise(resolve => {
        finishRecognition = resolve;
      })
    );

    const captureText = (await import('@/main/capture/ocr')).default;
    const capture = captureText();

    await vi.waitFor(() => expect(mockDaemonCall).toHaveBeenCalled());
    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');

    finishRecognition({ text: 'Windows text' });
    await capture;
  });
});
