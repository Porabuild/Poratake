import { describe, it, expect, vi, beforeEach } from 'vitest';
import path from 'path';
import type { KeyboardData } from '@/types/keyboard';

const projectKeysJsonPath = path.join('/path/to/Rec.poratake', 'keys.json');

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
      expect(getKeyboardDataPath('/path/to/Rec.poratake/recording.mov')).toBe(
        projectKeysJsonPath
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
      await deleteKeyboardData('/path/to/Rec.poratake/recording.mov');
      expect(fs.default.unlink).toHaveBeenCalledWith(projectKeysJsonPath);
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

  describe('keyboard rendering', () => {
    it('renders Windows shortcut modifiers with Windows labels', async () => {
      const fillText = vi.fn();
      const context = {
        save: vi.fn(),
        restore: vi.fn(),
        measureText: vi.fn(() => ({ width: 20 })),
        beginPath: vi.fn(),
        roundRect: vi.fn(),
        fill: vi.fn(),
        fillText,
      } as never;
      const { renderKeyboard } =
        await import('@/renderer/components/video-editor/composition/keyboard-canvas-renderer');

      renderKeyboard(context, 1, {
        keyboardData: {
          events: [
            {
              timestamp: 0.5,
              key: 'K',
              keyCode: 75,
              modifiers: ['control', 'alt'],
              type: 'down',
            },
          ],
          meta: {
            startTime: '2025-01-01T00:00:00.000Z',
            duration: 2,
            sampleRate: 1,
            platform: 'windows',
          },
        },
        keyboardStyle: {
          visible: true,
          displayDuration: 2,
          position: 'bottom-center',
          fontSize: 'medium',
          opacity: 0.75,
        },
        segments: [
          {
            id: 'segment-1',
            startTime: 0,
            endTime: 2,
            timelineStart: 0,
          },
        ],
        videoWidth: 1920,
        videoHeight: 1080,
      });

      expect(fillText).toHaveBeenCalledWith(
        'Ctrl+Alt+K',
        expect.any(Number),
        expect.any(Number)
      );
    });
  });
});
