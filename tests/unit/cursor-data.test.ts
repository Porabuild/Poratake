import { describe, it, expect, vi, beforeEach } from 'vitest';
import path from 'path';
import type { CursorData } from '@/types/cursor';

const projectCursorJsonPath = path.join(
  '/path/to/Recording.capty',
  'cursor.json'
);

// Mock fs/promises
vi.mock('fs/promises', () => ({
  default: {
    readFile: vi.fn(),
    unlink: vi.fn(),
    writeFile: vi.fn(),
  },
}));

describe('Cursor Data Utilities', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  describe('getCursorDataPath', () => {
    it('should return cursor.json in project folder for .capty paths', async () => {
      const { getCursorDataPath } =
        await import('@/main/capture/video/cursor-data');

      // Project folder structure: cursor.json is in the .capty folder
      expect(getCursorDataPath('/path/to/Recording.capty/recording.mov')).toBe(
        projectCursorJsonPath
      );
      expect(getCursorDataPath('/path/to/Recording.capty')).toBe(
        projectCursorJsonPath
      );
    });

    it('should use legacy path for non-project files', async () => {
      const { getCursorDataPath } =
        await import('@/main/capture/video/cursor-data');

      // Legacy: replace extension with .cursor.json
      expect(getCursorDataPath('/path/to/video.mov')).toBe(
        '/path/to/video.cursor.json'
      );
      expect(getCursorDataPath('/path/to/video.mp4')).toBe(
        '/path/to/video.cursor.json'
      );
    });
  });

  describe('loadCursorData', () => {
    it('should load cursor data from project folder', async () => {
      const fs = await import('fs/promises');
      const mockCursorData: CursorData = {
        recordingArea: { width: 1920, height: 1080 },
        events: [
          { timestamp: 0, x: 0.5, y: 0.5, type: 'move' },
          { timestamp: 1, x: 0.6, y: 0.4, type: 'down', button: 'left' },
        ],
        meta: {
          startTime: '2024-01-01T00:00:00.000Z',
          duration: 10,
          sampleRate: 60,
        },
      };

      vi.mocked(fs.default.readFile).mockResolvedValue(
        JSON.stringify(mockCursorData)
      );

      const { loadCursorData } =
        await import('@/main/capture/video/cursor-data');

      const result = await loadCursorData(
        '/path/to/Recording.capty/recording.mov'
      );

      expect(fs.default.readFile).toHaveBeenCalledWith(
        projectCursorJsonPath,
        'utf-8'
      );
      expect(result).toEqual(mockCursorData);
    });

    it('should load cursor data from legacy path', async () => {
      const fs = await import('fs/promises');
      const mockCursorData: CursorData = {
        recordingArea: { width: 1920, height: 1080 },
        events: [{ timestamp: 0, x: 0.5, y: 0.5, type: 'move' }],
        meta: {
          startTime: '2024-01-01T00:00:00.000Z',
          duration: 10,
          sampleRate: 60,
        },
      };

      vi.mocked(fs.default.readFile).mockResolvedValue(
        JSON.stringify(mockCursorData)
      );

      const { loadCursorData } =
        await import('@/main/capture/video/cursor-data');

      const result = await loadCursorData('/path/to/video.mov');

      expect(fs.default.readFile).toHaveBeenCalledWith(
        '/path/to/video.cursor.json',
        'utf-8'
      );
      expect(result).toEqual(mockCursorData);
    });

    it('should return null if file does not exist', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.readFile).mockRejectedValue(
        new Error('ENOENT: no such file')
      );

      const { loadCursorData } =
        await import('@/main/capture/video/cursor-data');

      const result = await loadCursorData('/path/to/video.mov');

      expect(result).toBeNull();
    });

    it('should return null if JSON is invalid', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.readFile).mockResolvedValue('invalid json');

      const { loadCursorData } =
        await import('@/main/capture/video/cursor-data');

      const result = await loadCursorData('/path/to/video.mov');

      expect(result).toBeNull();
    });
  });

  describe('deleteCursorData', () => {
    it('should delete cursor data file from project folder', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.unlink).mockResolvedValue();

      const { deleteCursorData } =
        await import('@/main/capture/video/cursor-data');

      await deleteCursorData('/path/to/Recording.capty/recording.mov');

      expect(fs.default.unlink).toHaveBeenCalledWith(projectCursorJsonPath);
    });

    it('should delete cursor data file from legacy path', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.unlink).mockResolvedValue();

      const { deleteCursorData } =
        await import('@/main/capture/video/cursor-data');

      await deleteCursorData('/path/to/video.mov');

      expect(fs.default.unlink).toHaveBeenCalledWith(
        '/path/to/video.cursor.json'
      );
    });

    it('should not throw if file does not exist', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.unlink).mockRejectedValue(
        new Error('ENOENT: no such file')
      );

      const { deleteCursorData } =
        await import('@/main/capture/video/cursor-data');

      // Should not throw
      await expect(
        deleteCursorData('/path/to/video.mov')
      ).resolves.toBeUndefined();
    });
  });

  describe('saveCursorData', () => {
    it('writes cursor JSON to disk', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.writeFile).mockResolvedValue();
      const { saveCursorData } =
        await import('@/main/capture/video/cursor-data');
      const data = {
        recordingArea: { width: 1920, height: 1080 },
        events: [],
        meta: {
          startTime: '2025-01-01T00:00:00.000Z',
          duration: 10,
          sampleRate: 60,
        },
      } as unknown as Parameters<typeof saveCursorData>[1];
      await saveCursorData('/path/to/video.mov', data);
      expect(fs.default.writeFile).toHaveBeenCalledWith(
        '/path/to/video.cursor.json',
        expect.any(String),
        'utf-8'
      );
    });
  });
});
