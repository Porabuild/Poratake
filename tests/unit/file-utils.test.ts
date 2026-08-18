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

      expect(mockFetch).toHaveBeenCalledWith('file:///path/to/my%20video.mp4');
    });

    it('encodes URL-significant characters in Windows paths', async () => {
      const mockBlob = new Blob(['test'], { type: 'video/mp4' });
      mockFetch.mockResolvedValue({
        blob: () => Promise.resolve(mockBlob),
      });

      const { loadFileAsBlob } =
        await import('@/renderer/components/video-editor/export/file-utils');
      await loadFileAsBlob('C:\\Videos\\Demo #1.mp4');

      expect(mockFetch).toHaveBeenCalledWith(
        'file:///C:/Videos/Demo%20%231.mp4'
      );
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
          private srcValue = '';
          get src() {
            return this.srcValue;
          }
          set src(value: string) {
            this.srcValue = value;
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

  describe('streamed output', () => {
    it('creates the output file through IPC', async () => {
      mockInvoke.mockResolvedValue({ success: true });

      const { createOutputFile } =
        await import('@/renderer/components/video-editor/export/file-utils');

      await createOutputFile('/path/to/output.mp4');

      expect(mockInvoke).toHaveBeenCalledWith('file:create-output', {
        path: '/path/to/output.mp4',
      });
    });

    it('writes positioned output chunks through IPC', async () => {
      mockInvoke.mockResolvedValue({ success: true });

      const { writeOutputChunk } =
        await import('@/renderer/components/video-editor/export/file-utils');

      const buffer = new Uint8Array([1, 2, 3, 4]);
      await writeOutputChunk('/path/to/output.mp4', 128, buffer);

      expect(mockInvoke).toHaveBeenCalledWith('file:write-output-chunk', {
        path: '/path/to/output.mp4',
        position: 128,
        buffer,
      });
    });

    it('propagates IPC write errors', async () => {
      mockInvoke.mockRejectedValue(new Error('Write failed'));

      const { writeOutputChunk } =
        await import('@/renderer/components/video-editor/export/file-utils');

      const buffer = new Uint8Array([1, 2, 3]);
      await expect(
        writeOutputChunk('/path/to/file.mp4', 0, buffer)
      ).rejects.toThrow('Write failed');
    });

    it('rejects write failures returned by the main process', async () => {
      mockInvoke.mockResolvedValue({ success: false, error: 'disk full' });

      const { writeOutputChunk } =
        await import('@/renderer/components/video-editor/export/file-utils');

      await expect(
        writeOutputChunk('/path/to/file.mp4', 0, new Uint8Array([1, 2, 3]))
      ).rejects.toThrow('disk full');
    });
  });
});
