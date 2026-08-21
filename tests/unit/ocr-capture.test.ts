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
  once() {
    return this;
  }
  close() {}
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

vi.mock('@/main/capture/desktop-icons/preference', () => ({
  shouldHideDesktopIconsForCapture: () =>
    Boolean(mockGetConfig().screenshot?.hideDesktopIcons) &&
    mockIsDesktopIconsSupported(),
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
    mockCaptureAreaToFile.mockResolvedValue(true);
  });

  it('uses the unified area overlay and copies recognized text', async () => {
    mockFsExistsSync.mockReturnValue(true);
    mockDaemonCall.mockResolvedValue({ text: '  Hello world  ' });
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    await flush();
    expect(mockExecFile).not.toHaveBeenCalled();
    expect(mockCaptureAreaToFile).toHaveBeenCalledWith(
      expect.stringContaining('poratake-ocr-')
    );
    expect(mockClipboardWriteText).toHaveBeenCalledWith('Hello world');
    expect(mockNotificationCreate).toHaveBeenCalledWith({
      title: 'Text copied',
      body: 'Recognized text has been copied to the clipboard',
      silent: true,
      timeoutType: 'default',
    });
    expect(mockNotificationShow).toHaveBeenCalled();
    expect(mockFsUnlinkSync).toHaveBeenCalled();
  });

  it('notifies when no text is detected', async () => {
    mockDaemonCall.mockResolvedValue({ text: '' });
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    await flush();
    expect(mockClipboardWriteText).not.toHaveBeenCalled();
    expect(mockNotificationCreate).toHaveBeenCalledWith({
      title: 'No Text Found',
      body: 'No text was detected in the selected area',
    });
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('bails out when the area capture is cancelled', async () => {
    mockCaptureAreaToFile.mockResolvedValue(false);
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    await flush();
    expect(mockDaemonCall).not.toHaveBeenCalled();
  });

  it('shows a failure notification on daemon error', async () => {
    mockDaemonCall.mockRejectedValue(new Error('ocr crash'));
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    await flush();
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('keeps recognized text when temporary-file cleanup fails', async () => {
    mockFsUnlinkSync.mockImplementation(() => {
      throw new Error('file locked');
    });
    mockDaemonCall.mockResolvedValue({ text: 'Recognized text' });
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    expect(mockClipboardWriteText).toHaveBeenCalledWith('Recognized text');
  });

  it('hides and restores desktop icons when enabled', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockDaemonCall.mockResolvedValue({ text: 'x' });
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    await flush();
    expect(mockHideDesktopIcons).toHaveBeenCalledWith('capture');
    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
  });

  it('uses a unique temporary path for every capture', async () => {
    mockDaemonCall.mockResolvedValue({ text: 'x' });
    const captureText = (await import('@/main/capture/ocr')).default;
    await captureText();
    await captureText();
    expect(mockCaptureAreaToFile.mock.calls[0][0]).not.toBe(
      mockCaptureAreaToFile.mock.calls[1][0]
    );
  });

  it('ignores a second capture while the first is active', async () => {
    mockGetConfig.mockReturnValue({ screenshot: { hideDesktopIcons: true } });
    mockDaemonCall.mockResolvedValue({ text: 'x' });
    let finishCapture: (value: boolean) => void = () => {};
    mockCaptureAreaToFile.mockReturnValue(
      new Promise<boolean>(resolve => {
        finishCapture = resolve;
      })
    );
    const captureText = (await import('@/main/capture/ocr')).default;

    const firstCapture = captureText();
    await Promise.resolve();
    const secondCapture = captureText();

    expect(mockCaptureAreaToFile).toHaveBeenCalledTimes(1);
    expect(mockHideDesktopIcons).toHaveBeenCalledTimes(1);

    finishCapture(true);
    await Promise.all([firstCapture, secondCapture]);

    expect(mockShowDesktopIcons).toHaveBeenCalledTimes(1);
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
      expect.stringContaining('poratake-ocr-'),
      expect.stringContaining('poratake-ocr-processed-')
    );
    expect(mockDaemonCall).toHaveBeenCalledWith('ocr', 'recognize', {
      imagePath: expect.stringContaining('poratake-ocr-processed-'),
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
      expect.stringContaining('poratake-ocr-')
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
      expect.stringContaining('poratake-ocr-'),
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
      imagePath: expect.not.stringContaining('poratake-ocr-processed-'),
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
