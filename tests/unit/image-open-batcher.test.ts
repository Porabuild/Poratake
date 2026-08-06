import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockGetConfig = vi.fn();
const mockOpenScreenshotEditorWithLayers = vi.fn();

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
}));

vi.mock('@/main/capture/screenshot/open-editor', () => ({
  openScreenshotEditorWithLayers: (...a: unknown[]) =>
    mockOpenScreenshotEditorWithLayers(...a),
}));

describe('image-open-batcher', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    vi.useFakeTimers();
    mockGetConfig.mockReturnValue({
      screenshot: { multiImageAttachEdge: 'bottom' },
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('flush no-op when empty', async () => {
    const { flushPendingImages } =
      await import('@/main/capture/screenshot/image-open-batcher');
    flushPendingImages();
    expect(mockOpenScreenshotEditorWithLayers).not.toHaveBeenCalled();
  });

  it('bufferImageFile appends without timer', async () => {
    const { bufferImageFile, flushPendingImages } =
      await import('@/main/capture/screenshot/image-open-batcher');
    bufferImageFile('/p/a.png');
    bufferImageFile('/p/b.png');
    flushPendingImages();
    expect(mockOpenScreenshotEditorWithLayers).toHaveBeenCalledWith(
      '/p/a.png',
      ['/p/b.png'],
      'bottom'
    );
  });

  it('queueImageFile flushes after debounce', async () => {
    const { queueImageFile } =
      await import('@/main/capture/screenshot/image-open-batcher');
    queueImageFile('/p/a.png');
    queueImageFile('/p/b.png');
    vi.advanceTimersByTime(120);
    expect(mockOpenScreenshotEditorWithLayers).toHaveBeenCalledTimes(1);
    expect(mockOpenScreenshotEditorWithLayers).toHaveBeenCalledWith(
      '/p/a.png',
      ['/p/b.png'],
      'bottom'
    );
  });

  it('queueImageFile resets timer on each call', async () => {
    const { queueImageFile } =
      await import('@/main/capture/screenshot/image-open-batcher');
    queueImageFile('/p/a.png');
    vi.advanceTimersByTime(50);
    queueImageFile('/p/b.png');
    vi.advanceTimersByTime(50);
    expect(mockOpenScreenshotEditorWithLayers).not.toHaveBeenCalled();
    vi.advanceTimersByTime(60);
    expect(mockOpenScreenshotEditorWithLayers).toHaveBeenCalled();
  });

  it('explicit flush clears pending timer', async () => {
    const { queueImageFile, flushPendingImages } =
      await import('@/main/capture/screenshot/image-open-batcher');
    queueImageFile('/p/a.png');
    flushPendingImages();
    expect(mockOpenScreenshotEditorWithLayers).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(150);
    // Should not fire again
    expect(mockOpenScreenshotEditorWithLayers).toHaveBeenCalledTimes(1);
  });
});
