import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

type ExecCallback = (
  error: Error | null,
  stdout: string,
  stderr: string
) => void;
const mockExecFile =
  vi.fn<(file: string, args: string[], callback: ExecCallback) => void>();
const mockClipboardWriteImage = vi.fn();
const mockCreateFromBuffer = vi.fn(() => ({
  image: true,
  isEmpty: () => false,
}));
const mockGetConfig = vi.fn();
const mockAddToHistory = vi.fn();
const mockGenerateScreenshotPath = vi.fn(() => '/path/Screenshot.png');
const mockPrepareCapturePreview = vi.fn();
const mockShowCapturePreview = vi.fn();
const mockDisposePreparedPreview = vi.fn();
const mockOpenScreenshotEditor = vi.fn();
const mockFsExistsSync = vi.fn();
const mockFsReadFileSync = vi.fn(() => Buffer.from('image-bytes'));
const mockCaptureRegionToFile = vi.fn();
const mockCaptureWindowToFile = vi.fn();
const mockHideDesktopIcons = vi.fn();
const mockShowDesktopIcons = vi.fn();
const mockDesktopIconsSupported = vi.fn();

vi.mock('child_process', () => ({
  execFile: (file: string, args: string[], cb: ExecCallback) =>
    mockExecFile(file, args, cb),
}));

vi.mock('electron', () => ({
  clipboard: {
    writeImage: (...a: unknown[]) => mockClipboardWriteImage(...a),
  },
  nativeImage: {
    createFromBuffer: (...a: unknown[]) => mockCreateFromBuffer(...a),
  },
}));

vi.mock('@/main/capture/screenshot/native-capture', () => ({
  captureRegionToFile: (...a: unknown[]) => mockCaptureRegionToFile(...a),
  captureWindowToFile: (...a: unknown[]) => mockCaptureWindowToFile(...a),
}));

vi.mock('@/main/capture/desktop-icons', () => ({
  hideDesktopIcons: (...a: unknown[]) => mockHideDesktopIcons(...a),
  showDesktopIcons: (...a: unknown[]) => mockShowDesktopIcons(...a),
  isSupported: () => mockDesktopIconsSupported(),
}));

vi.mock('fs', () => ({
  default: {
    existsSync: (...a: unknown[]) => mockFsExistsSync(...a),
    readFileSync: (...a: unknown[]) => mockFsReadFileSync(...a),
  },
  existsSync: (...a: unknown[]) => mockFsExistsSync(...a),
  readFileSync: (...a: unknown[]) => mockFsReadFileSync(...a),
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
}));

vi.mock('@/main/history', () => ({
  addToHistory: (...a: unknown[]) => mockAddToHistory(...a),
}));

vi.mock('@/main/capture/screenshot/utils.ts', () => ({
  generateScreenshotPath: () => mockGenerateScreenshotPath(),
}));

vi.mock('@/main/capture/capture-preview', () => ({
  prepareCapturePreview: (...a: unknown[]) => mockPrepareCapturePreview(...a),
  showCapturePreview: (...a: unknown[]) => mockShowCapturePreview(...a),
}));

vi.mock('@/main/capture/screenshot/open-editor', () => ({
  openScreenshotEditor: (...a: unknown[]) => mockOpenScreenshotEditor(...a),
}));

describe('captureArea', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: true },
      screenshot: {
        captureToClipboard: false,
        showPreview: false,
        hideDesktopIcons: false,
      },
    });
    mockDesktopIconsSupported.mockReturnValue(false);
    mockFsExistsSync.mockReturnValue(true);
    mockAddToHistory.mockResolvedValue({ id: 'h1' });
    mockPrepareCapturePreview.mockReturnValue({
      dispose: mockDisposePreparedPreview,
    });
    mockShowCapturePreview.mockReturnValue({ revealed: Promise.resolve() });
  });

  it('rejects invalid area (missing dimensions)', async () => {
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    const result = await captureArea({ status: 'confirmed' } as never);
    expect(result).toBeNull();
    expect(mockExecFile).not.toHaveBeenCalled();
  });

  it('runs screencapture with -R bounds and opens editor by default', async () => {
    mockExecFile.mockImplementation((file, args, cb) => {
      expect(file).toBe('screencapture');
      expect(args).toEqual([
        '-R',
        '10,20,800,600',
        '-t',
        'png',
        '/path/Screenshot.png',
      ]);
      cb(null, '', '');
    });
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    const result = await captureArea({
      status: 'confirmed',
      x: 10,
      y: 20,
      width: 800,
      height: 600,
    });
    expect(result).toBe('/path/Screenshot.png');
    expect(mockOpenScreenshotEditor).toHaveBeenCalledWith(
      '/path/Screenshot.png',
      'h1'
    );
  });

  it('omits sound (-x) when playSoundOnScreenshot is true', async () => {
    mockExecFile.mockImplementation((_file, args, cb) => {
      expect(args).not.toContain('-x');
      cb(null, '', '');
    });
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    await captureArea({
      status: 'confirmed',
      x: 0,
      y: 0,
      width: 10,
      height: 10,
    });
  });

  it('passes -x to disable sound when playSoundOnScreenshot is false', async () => {
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: false },
      screenshot: { captureToClipboard: false, showPreview: false },
    });
    mockExecFile.mockImplementation((_file, args, cb) => {
      expect(args).toContain('-x');
      cb(null, '', '');
    });
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    await captureArea({
      status: 'confirmed',
      x: 0,
      y: 0,
      width: 10,
      height: 10,
    });
  });

  it('rejects on exec error', async () => {
    mockExecFile.mockImplementation((_file, _args, cb) =>
      cb(new Error('cap fail'), '', '')
    );
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    await expect(
      captureArea({
        status: 'confirmed',
        x: 0,
        y: 0,
        width: 10,
        height: 10,
      })
    ).rejects.toThrow('cap fail');
  });

  it('returns null when screenshot file is missing', async () => {
    mockFsExistsSync.mockReturnValue(false);
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    const result = await captureArea({
      status: 'confirmed',
      x: 0,
      y: 0,
      width: 10,
      height: 10,
    });
    expect(result).toBeNull();
  });

  it('writes image to clipboard when captureToClipboard is enabled', async () => {
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: true },
      screenshot: { captureToClipboard: true, showPreview: true },
    });
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    await captureArea({
      status: 'confirmed',
      x: 0,
      y: 0,
      width: 10,
      height: 10,
    });
    expect(mockClipboardWriteImage).toHaveBeenCalled();
    expect(mockShowCapturePreview).not.toHaveBeenCalled();
  });

  it('shows preview when configured', async () => {
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: true },
      screenshot: { captureToClipboard: false, showPreview: true },
    });
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    await captureArea({
      status: 'confirmed',
      x: 0,
      y: 0,
      width: 10,
      height: 10,
    });
    expect(mockShowCapturePreview).toHaveBeenCalledWith(
      '/path/Screenshot.png',
      'screenshot',
      undefined,
      expect.objectContaining({ dispose: mockDisposePreparedPreview }),
      expect.any(Promise)
    );
  });

  it('calls onCaptured hook after successful capture', async () => {
    mockExecFile.mockImplementation((_file, _args, cb) => cb(null, '', ''));
    const onCaptured = vi.fn().mockResolvedValue(undefined);
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    await captureArea(
      { status: 'confirmed', x: 0, y: 0, width: 10, height: 10 },
      { onCaptured }
    );
    expect(onCaptured).toHaveBeenCalled();
  });
});

describe('captureArea on Windows', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.defineProperty(process, 'platform', { value: 'win32' });
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: true },
      screenshot: {
        captureToClipboard: false,
        showPreview: false,
        hideDesktopIcons: false,
      },
    });
    mockHideDesktopIcons.mockReset();
    mockShowDesktopIcons.mockReset();
    mockDesktopIconsSupported.mockReset();
    mockDesktopIconsSupported.mockReturnValue(false);
    mockFsExistsSync.mockReturnValue(true);
    mockAddToHistory.mockResolvedValue({ id: 'h1' });
    mockCaptureRegionToFile.mockResolvedValue(true);
    mockCaptureWindowToFile.mockResolvedValue(true);
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  it('captures the requested region through the daemon', async () => {
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    const result = await captureArea({
      status: 'confirmed',
      x: 110,
      y: 70,
      width: 300,
      height: 200,
    });

    expect(mockExecFile).not.toHaveBeenCalled();
    expect(mockCaptureRegionToFile).toHaveBeenCalledWith(
      { x: 110, y: 70, width: 300, height: 200 },
      '/path/Screenshot.png'
    );
    expect(result).toBe('/path/Screenshot.png');
    expect(mockOpenScreenshotEditor).toHaveBeenCalled();
  });

  it('captures from the retained native frame when requested', async () => {
    const onCaptured = vi.fn();
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');

    await captureArea(
      {
        status: 'confirmed',
        x: 110,
        y: 70,
        width: 300,
        height: 200,
      },
      { cached: true, onCaptured }
    );

    expect(mockCaptureRegionToFile).toHaveBeenCalledWith(
      { x: 110, y: 70, width: 300, height: 200 },
      '/path/Screenshot.png',
      { cached: true }
    );
    expect(onCaptured).toHaveBeenCalled();
  });

  it('captures a picked window instead of the pixels covering it', async () => {
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');

    await captureArea(
      { status: 'confirmed', x: 110, y: 70, width: 300, height: 200 },
      { windowId: 4242 }
    );

    expect(mockCaptureWindowToFile).toHaveBeenCalledWith(
      4242,
      '/path/Screenshot.png'
    );
    expect(mockCaptureRegionToFile).not.toHaveBeenCalled();
  });

  it('masks the picked window out of the retained frame', async () => {
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');

    await captureArea(
      { status: 'confirmed', x: 110, y: 70, width: 300, height: 200 },
      { cached: true, windowId: 4242 }
    );

    expect(mockCaptureRegionToFile).toHaveBeenCalledWith(
      { x: 110, y: 70, width: 300, height: 200 },
      '/path/Screenshot.png',
      { cached: true, windowId: 4242 }
    );
    expect(mockCaptureWindowToFile).not.toHaveBeenCalled();
  });

  it('hides desktop icons only while acquiring capture pixels', async () => {
    const calls: string[] = [];
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: true },
      screenshot: {
        captureToClipboard: false,
        showPreview: false,
        hideDesktopIcons: true,
      },
    });
    mockDesktopIconsSupported.mockReturnValue(true);
    mockHideDesktopIcons.mockImplementation(async () => {
      calls.push('hide');
    });
    mockCaptureRegionToFile.mockImplementation(async () => {
      calls.push('capture');
      return true;
    });
    mockShowDesktopIcons.mockImplementation(async () => {
      calls.push('show');
    });

    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    await captureArea({
      status: 'confirmed',
      x: 110,
      y: 70,
      width: 300,
      height: 200,
    });

    expect(calls).toEqual(['hide', 'capture', 'show']);
  });

  it('starts the preview while desktop icons are being restored', async () => {
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: true },
      screenshot: {
        autoCopyToClipboard: false,
        captureToClipboard: false,
        showPreview: true,
        hideDesktopIcons: true,
      },
    });
    mockDesktopIconsSupported.mockReturnValue(true);
    let finishRestore: (restored: boolean) => void = () => {};
    mockShowDesktopIcons.mockReturnValueOnce(
      new Promise(resolve => {
        finishRestore = resolve;
      })
    );
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');

    const capturing = captureArea({
      status: 'confirmed',
      x: 110,
      y: 70,
      width: 300,
      height: 200,
    });

    await vi.waitFor(() => {
      expect(mockShowCapturePreview).toHaveBeenCalledTimes(1);
    });

    finishRestore(true);
    await capturing;
  });

  it('restores desktop icons and returns null when the capture fails', async () => {
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: true },
      screenshot: {
        captureToClipboard: false,
        showPreview: false,
        hideDesktopIcons: true,
      },
    });
    mockDesktopIconsSupported.mockReturnValue(true);
    mockCaptureRegionToFile.mockResolvedValue(false);
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');

    const result = await captureArea({
      status: 'confirmed',
      x: 0,
      y: 0,
      width: 10,
      height: 10,
    });

    expect(result).toBeNull();
    expect(mockShowDesktopIcons).toHaveBeenCalledWith('capture');
    expect(mockOpenScreenshotEditor).not.toHaveBeenCalled();
  });
});
