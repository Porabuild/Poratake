import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { KeyboardData } from '@/types/keyboard';

vi.mock('fs/promises', () => ({
  default: {
    readFile: vi.fn(),
    unlink: vi.fn(),
  },
}));

describe('keyboard-data', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  describe('getKeyboardDataPath', () => {
    it('returns keys.json inside project folder', async () => {
      const { getKeyboardDataPath } =
        await import('@/main/capture/video/keyboard-data');
      expect(getKeyboardDataPath('/path/to/Rec.capty/recording.mov')).toBe(
        '/path/to/Rec.capty/keys.json'
      );
    });

    it('returns legacy path for non-project files', async () => {
      const { getKeyboardDataPath } =
        await import('@/main/capture/video/keyboard-data');
      expect(getKeyboardDataPath('/path/to/video.mov')).toBe(
        '/path/to/video.keys.json'
      );
    });
  });

  describe('loadKeyboardData', () => {
    it('parses keyboard data from file', async () => {
      const fs = await import('fs/promises');
      const sample: KeyboardData = {
        events: [
          {
            timestamp: 0,
            type: 'down',
            key: 'a',
            keyCode: 65,
            modifiers: [],
          },
        ],
        meta: {
          startTime: '2025-01-01T00:00:00.000Z',
          duration: 1,
          sampleRate: 60,
        },
      };
      vi.mocked(fs.default.readFile).mockResolvedValue(JSON.stringify(sample));
      const { loadKeyboardData } =
        await import('@/main/capture/video/keyboard-data');
      const result = await loadKeyboardData('/path/to/video.mov');
      expect(result).toEqual(sample);
      expect(fs.default.readFile).toHaveBeenCalledWith(
        '/path/to/video.keys.json',
        'utf-8'
      );
    });

    it('returns null on file read failure', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.readFile).mockRejectedValue(new Error('ENOENT'));
      const { loadKeyboardData } =
        await import('@/main/capture/video/keyboard-data');
      expect(await loadKeyboardData('/path/to/video.mov')).toBeNull();
    });

    it('returns null on invalid JSON', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.readFile).mockResolvedValue('not json');
      const { loadKeyboardData } =
        await import('@/main/capture/video/keyboard-data');
      expect(await loadKeyboardData('/path/to/video.mov')).toBeNull();
    });
  });

  describe('deleteKeyboardData', () => {
    it('unlinks the keyboard file', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.unlink).mockResolvedValue();
      const { deleteKeyboardData } =
        await import('@/main/capture/video/keyboard-data');
      await deleteKeyboardData('/path/to/Rec.capty/recording.mov');
      expect(fs.default.unlink).toHaveBeenCalledWith(
        '/path/to/Rec.capty/keys.json'
      );
    });

    it('swallows errors silently', async () => {
      const fs = await import('fs/promises');
      vi.mocked(fs.default.unlink).mockRejectedValue(new Error('ENOENT'));
      const { deleteKeyboardData } =
        await import('@/main/capture/video/keyboard-data');
      await expect(
        deleteKeyboardData('/path/to/video.mov')
      ).resolves.toBeUndefined();
    });
  });
});
