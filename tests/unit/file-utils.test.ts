import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockFetch = vi.fn();
const mockInvoke = vi.fn();

vi.stubGlobal('fetch', mockFetch);
vi.stubGlobal('window', {
  ipcRenderer: {
    invoke: mockInvoke,
  },
});

describe('file-utils', () => {
  beforeEach(() => {
    vi.resetModules();
    mockFetch.mockReset();
    mockInvoke.mockReset();
  });

  describe('loadFileAsBlob', () => {
    it('should fetch file with file:// protocol', async () => {
      const mockBlob = new Blob(['test content'], { type: 'video/mp4' });
      mockFetch.mockResolvedValue({
        blob: () => Promise.resolve(mockBlob),
      });

      const { loadFileAsBlob } =
        await import('@/renderer/components/video-editor/export/file-utils');
      const result = await loadFileAsBlob('/path/to/video.mp4');

      expect(mockFetch).toHaveBeenCalledWith('file:///path/to/video.mp4');
      expect(result).toBe(mockBlob);
    });

    it('should handle paths with spaces', async () => {
      const mockBlob = new Blob(['test'], { type: 'video/mp4' });
      mockFetch.mockResolvedValue({
        blob: () => Promise.resolve(mockBlob),
      });

      const { loadFileAsBlob } =
        await import('@/renderer/components/video-editor/export/file-utils');
      await loadFileAsBlob('/path/to/my video.mp4');

      expect(mockFetch).toHaveBeenCalledWith('file:///path/to/my video.mp4');
    });
  });

  describe('loadImage', () => {
    it('should resolve with image on successful load', async () => {
      let capturedSrc = '';

      vi.stubGlobal(
        'Image',
        class {
          src = '';
          onload: (() => void) | null = null;
          onerror: (() => void) | null = null;
          constructor() {
            setTimeout(() => {
              capturedSrc = this.src;
              this.onload?.();
            }, 0);
          }
        }
      );

      const { loadImage } =
        await import('@/renderer/components/video-editor/export/file-utils');

      const result = await loadImage('data:image/png;base64,abc');
      expect(result).not.toBeNull();
      expect(capturedSrc).toBe('data:image/png;base64,abc');
    });

    it('should resolve with null on load error', async () => {
      vi.stubGlobal(
        'Image',
        class {
          src = '';
          onload: (() => void) | null = null;
          onerror: (() => void) | null = null;
          constructor() {
            setTimeout(() => {
              this.onerror?.();
            }, 0);
          }
        }
      );

      const { loadImage } =
        await import('@/renderer/components/video-editor/export/file-utils');

      const result = await loadImage('invalid-url');
      expect(result).toBeNull();
    });

    it('should set image src', async () => {
      let capturedSrc = '';

      vi.stubGlobal(
        'Image',
        class {
          private _src = '';
          get src() {
            return this._src;
          }
          set src(value: string) {
            this._src = value;
            capturedSrc = value;
          }
          onload: (() => void) | null = null;
          onerror: (() => void) | null = null;
          constructor() {
            setTimeout(() => {
              this.onload?.();
            }, 0);
          }
        }
      );

      const { loadImage } =
        await import('@/renderer/components/video-editor/export/file-utils');

      const testSrc = 'https://example.com/image.png';
      await loadImage(testSrc);

      expect(capturedSrc).toBe(testSrc);
    });
  });

  describe('writeBuffer', () => {
    it('should invoke file:write-buffer IPC with correct params', async () => {
      mockInvoke.mockResolvedValue(undefined);

      const { writeBuffer } =
        await import('@/renderer/components/video-editor/export/file-utils');

      const buffer = new Uint8Array([1, 2, 3, 4]);
      await writeBuffer('/path/to/output.mp4', buffer);

      expect(mockInvoke).toHaveBeenCalledWith('file:write-buffer', {
        path: '/path/to/output.mp4',
        buffer,
      });
    });

    it('should handle write errors', async () => {
      mockInvoke.mockRejectedValue(new Error('Write failed'));

      const { writeBuffer } =
        await import('@/renderer/components/video-editor/export/file-utils');

      const buffer = new Uint8Array([1, 2, 3]);
      await expect(writeBuffer('/path/to/file.mp4', buffer)).rejects.toThrow(
        'Write failed'
      );
    });
  });
});
