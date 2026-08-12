import { afterEach, describe, expect, it, vi } from 'vitest';
import { loadScreenshotImage } from '@/renderer/utils/screenshot-image';

class MockImage {
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  private value = '';

  get src(): string {
    return this.value;
  }

  set src(source: string) {
    this.value = source;
    queueMicrotask(() => {
      if (source.includes('broken')) {
        this.onerror?.();
        return;
      }

      this.onload?.();
    });
  }
}

describe('loadScreenshotImage', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('loads the direct file URL without reading base64', async () => {
    vi.stubGlobal('Image', MockImage);
    const readFile = vi.fn<() => Promise<string>>();

    const image = await loadScreenshotImage('file:///capture.png', readFile);

    expect(image.src).toBe('file:///capture.png');
    expect(readFile).not.toHaveBeenCalled();
  });

  it('reads base64 only when the direct file URL fails', async () => {
    vi.stubGlobal('Image', MockImage);
    const readFile = vi.fn().mockResolvedValue('encoded-image');

    const image = await loadScreenshotImage('file:///broken.png', readFile);

    expect(readFile).toHaveBeenCalledOnce();
    expect(image.src).toBe('data:image/png;base64,encoded-image');
  });
});
