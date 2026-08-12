import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockClipboardWriteImage = vi.fn();
const mockCreateFromBuffer = vi.fn(() => ({ isEmpty: () => false }));
const mockGetConfig = vi.fn();
const mockAddToHistory = vi.fn();
const mockPrepareCapturePreview = vi.fn();
const mockShowCapturePreview = vi.fn();
const mockOpenScreenshotEditor = vi.fn();
const mockFsExistsSync = vi.fn();
const mockFsReadFileSync = vi.fn(() => Buffer.from('image-bytes'));

vi.mock('electron', () => ({
  clipboard: {
    writeImage: (...a: unknown[]) => mockClipboardWriteImage(...a),
  },
  nativeImage: {
    createFromBuffer: (...a: unknown[]) => mockCreateFromBuffer(...a),
  },
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

vi.mock('@/main/capture/capture-preview', () => ({
  prepareCapturePreview: (...a: unknown[]) => mockPrepareCapturePreview(...a),
  showCapturePreview: (...a: unknown[]) => mockShowCapturePreview(...a),
}));

vi.mock('@/main/capture/screenshot/open-editor', () => ({
  openScreenshotEditor: (...a: unknown[]) => mockOpenScreenshotEditor(...a),
}));

function setScreenshotConfig(overrides: Record<string, unknown>): void {
  mockGetConfig.mockReturnValue({
    screenshot: {
      autoCopyToClipboard: true,
      captureToClipboard: false,
      showPreview: false,
      ...overrides,
    },
  });
}

async function importFinalize() {
  return import('@/main/capture/screenshot/finalize');
}

describe('finalizeCapture', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockFsExistsSync.mockReturnValue(true);
    mockAddToHistory.mockResolvedValue({ id: 'h1' });
    mockShowCapturePreview.mockReturnValue({ revealed: Promise.resolve() });
    setScreenshotConfig({});
  });

  it('does nothing when the capture file is missing', async () => {
    mockFsExistsSync.mockReturnValue(false);
    const { finalizeCapture } = await importFinalize();

    await finalizeCapture('/path/missing.png');

    expect(mockAddToHistory).not.toHaveBeenCalled();
    expect(mockClipboardWriteImage).not.toHaveBeenCalled();
    expect(mockOpenScreenshotEditor).not.toHaveBeenCalled();
  });

  it('copies to clipboard and still opens the editor by default', async () => {
    const { finalizeCapture } = await importFinalize();

    await finalizeCapture('/path/shot.png');

    expect(mockClipboardWriteImage).toHaveBeenCalledTimes(1);
    expect(mockOpenScreenshotEditor).toHaveBeenCalledWith(
      '/path/shot.png',
      'h1'
    );
  });

  it('copies to clipboard and still shows the preview', async () => {
    setScreenshotConfig({ showPreview: true });
    const { finalizeCapture } = await importFinalize();

    await finalizeCapture('/path/shot.png');

    expect(mockClipboardWriteImage).toHaveBeenCalledTimes(1);
    expect(mockShowCapturePreview).toHaveBeenCalledWith(
      '/path/shot.png',
      'screenshot',
      undefined,
      undefined,
      expect.any(Promise)
    );
    expect(mockOpenScreenshotEditor).not.toHaveBeenCalled();
  });

  it('starts history persistence only after the preview is visible', async () => {
    setScreenshotConfig({ autoCopyToClipboard: false, showPreview: true });
    let revealPreview: () => void = () => {};
    mockShowCapturePreview.mockReturnValueOnce({
      revealed: new Promise(resolve => {
        revealPreview = resolve;
      }),
    });
    const { finalizeCapture } = await importFinalize();

    const finalizing = finalizeCapture('/path/shot.png');

    expect(mockShowCapturePreview).toHaveBeenCalledWith(
      '/path/shot.png',
      'screenshot',
      undefined,
      undefined,
      expect.any(Promise)
    );
    expect(mockAddToHistory).not.toHaveBeenCalled();

    revealPreview();
    await finalizing;

    expect(mockAddToHistory).toHaveBeenCalledWith('/path/shot.png');
  });

  it('waits until the preview is visible before auto-copying', async () => {
    setScreenshotConfig({ showPreview: true });
    let revealPreview: () => void = () => {};
    mockShowCapturePreview.mockReturnValueOnce({
      revealed: new Promise(resolve => {
        revealPreview = resolve;
      }),
    });
    const { finalizeCapture } = await importFinalize();

    const finalizing = finalizeCapture('/path/shot.png');

    expect(mockClipboardWriteImage).not.toHaveBeenCalled();

    revealPreview();
    await finalizing;

    expect(mockClipboardWriteImage).toHaveBeenCalledTimes(1);
  });

  it('falls back to the editor when preview creation fails', async () => {
    setScreenshotConfig({ showPreview: true });
    const dispose = vi.fn();
    mockShowCapturePreview.mockImplementationOnce(() => {
      throw new Error('preview failed');
    });
    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => {});
    const { finalizeCapture } = await importFinalize();

    await finalizeCapture('/path/shot.png', { dispose } as never);

    expect(dispose).toHaveBeenCalledTimes(1);
    expect(mockAddToHistory).toHaveBeenCalledWith('/path/shot.png');
    expect(mockClipboardWriteImage).toHaveBeenCalledTimes(1);
    expect(mockOpenScreenshotEditor).toHaveBeenCalledWith(
      '/path/shot.png',
      'h1'
    );
    consoleError.mockRestore();
  });

  it('skips the clipboard when auto copy is disabled', async () => {
    setScreenshotConfig({ autoCopyToClipboard: false, showPreview: true });
    const { finalizeCapture } = await importFinalize();

    await finalizeCapture('/path/shot.png');

    expect(mockClipboardWriteImage).not.toHaveBeenCalled();
    expect(mockShowCapturePreview).toHaveBeenCalled();
  });

  it('still copies in clipboard only mode when auto copy is disabled', async () => {
    setScreenshotConfig({
      autoCopyToClipboard: false,
      captureToClipboard: true,
      showPreview: true,
    });
    const { finalizeCapture } = await importFinalize();

    await finalizeCapture('/path/shot.png');

    expect(mockClipboardWriteImage).toHaveBeenCalledTimes(1);
    expect(mockShowCapturePreview).not.toHaveBeenCalled();
    expect(mockOpenScreenshotEditor).not.toHaveBeenCalled();
  });

  it('copies once when both clipboard settings are enabled', async () => {
    setScreenshotConfig({ captureToClipboard: true });
    const { finalizeCapture } = await importFinalize();

    await finalizeCapture('/path/shot.png');

    expect(mockClipboardWriteImage).toHaveBeenCalledTimes(1);
  });

  it('does not overwrite the clipboard with an empty image', async () => {
    mockCreateFromBuffer.mockReturnValueOnce({ isEmpty: () => true });
    const { finalizeCapture } = await importFinalize();

    await finalizeCapture('/path/shot.png');

    expect(mockClipboardWriteImage).not.toHaveBeenCalled();
    expect(mockOpenScreenshotEditor).toHaveBeenCalled();
  });

  it('continues to the editor when reading the capture fails', async () => {
    mockFsReadFileSync.mockImplementationOnce(() => {
      throw new Error('read failed');
    });
    const { finalizeCapture } = await importFinalize();

    await finalizeCapture('/path/shot.png');

    expect(mockClipboardWriteImage).not.toHaveBeenCalled();
    expect(mockOpenScreenshotEditor).toHaveBeenCalled();
  });
});
