import { describe, it, expect, vi, beforeEach } from 'vitest';

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
const mockCaptureFrozenWindowToFile = vi.fn();
const mockCaptureWindowByIdToFile = vi.fn();
const mockHideDesktopIcons = vi.fn();
const mockShowDesktopIcons = vi.fn();
const mockDesktopIconsSupported = vi.fn();
const mockCheckAccessibilityPermission = vi.fn();

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
  captureFrozenWindowToFile: (...a: unknown[]) =>
    mockCaptureFrozenWindowToFile(...a),
  captureWindowByIdToFile: (...a: unknown[]) =>
    mockCaptureWindowByIdToFile(...a),
}));

vi.mock('@/main/capture/desktop-icons', () => ({
  hideDesktopIcons: (...a: unknown[]) => mockHideDesktopIcons(...a),
  showDesktopIcons: (...a: unknown[]) => mockShowDesktopIcons(...a),
  checkAccessibilityPermission: (...a: unknown[]) =>
    mockCheckAccessibilityPermission(...a),
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

vi.mock('@/main/capture/screenshot/capture-sound', () => ({
  playCaptureSound: vi.fn(),
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
    mockCheckAccessibilityPermission.mockReturnValue(true);
    mockFsExistsSync.mockReturnValue(true);
    mockAddToHistory.mockResolvedValue({ id: 'h1' });
    mockPrepareCapturePreview.mockReturnValue({
      dispose: mockDisposePreparedPreview,
    });
    mockShowCapturePreview.mockReturnValue({ revealed: Promise.resolve() });
    mockCaptureRegionToFile.mockResolvedValue(true);
    mockCaptureFrozenWindowToFile.mockResolvedValue(true);
    mockCaptureWindowByIdToFile.mockResolvedValue(true);
  });

  it('rejects invalid area (missing dimensions)', async () => {
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    const result = await captureArea({ status: 'confirmed' } as never);
    expect(result).toBeNull();
    expect(mockCaptureRegionToFile).not.toHaveBeenCalled();
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

    expect(mockCaptureWindowByIdToFile).toHaveBeenCalledWith(
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
    expect(mockCaptureWindowByIdToFile).not.toHaveBeenCalled();
  });

  it('captures complete retained window bounds across displays', async () => {
    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    const windowBounds = { x: 1800, y: 200, width: 400, height: 300 };

    await captureArea(
      { status: 'confirmed', x: 1800, y: 200, width: 400, height: 300 },
      { cached: true, windowId: 4242, windowBounds }
    );

    expect(mockCaptureFrozenWindowToFile).toHaveBeenCalledWith(
      windowBounds,
      '/path/Screenshot.png',
      4242
    );
    expect(mockCaptureRegionToFile).not.toHaveBeenCalled();
    expect(mockCaptureWindowByIdToFile).not.toHaveBeenCalled();
  });

  it('rejects when the daemon capture fails', async () => {
    mockCaptureRegionToFile.mockRejectedValue(new Error('cap fail'));
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
    expect(mockOpenScreenshotEditor).not.toHaveBeenCalled();
  });

  it('writes image to clipboard when captureToClipboard is enabled', async () => {
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: true },
      screenshot: { captureToClipboard: true, showPreview: true },
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
    expect(mockClipboardWriteImage).toHaveBeenCalled();
    expect(mockShowCapturePreview).not.toHaveBeenCalled();
  });

  it('shows preview when configured', async () => {
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: true },
      screenshot: { captureToClipboard: false, showPreview: true },
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
    expect(mockShowCapturePreview).toHaveBeenCalledWith(
      '/path/Screenshot.png',
      'screenshot',
      undefined,
      expect.objectContaining({ dispose: mockDisposePreparedPreview }),
      expect.any(Promise)
    );
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

  it('skips hiding desktop icons when accessibility permission is denied', async () => {
    mockGetConfig.mockReturnValue({
      general: { playSoundOnScreenshot: true },
      screenshot: {
        captureToClipboard: false,
        showPreview: false,
        hideDesktopIcons: true,
      },
    });
    mockDesktopIconsSupported.mockReturnValue(true);
    mockCheckAccessibilityPermission.mockReturnValue(false);

    const { captureArea } =
      await import('@/main/capture/screenshot/capture-area');
    await captureArea({
      status: 'confirmed',
      x: 110,
      y: 70,
      width: 300,
      height: 200,
    });

    expect(mockHideDesktopIcons).not.toHaveBeenCalled();
    expect(mockShowDesktopIcons).not.toHaveBeenCalled();
    expect(mockCaptureRegionToFile).toHaveBeenCalled();
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

  it('calls onCaptured hook after successful capture', async () => {
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
