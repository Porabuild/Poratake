import { describe, it, expect, vi, beforeEach } from 'vitest';

type Handler = (...args: unknown[]) => unknown;
const ipcHandle: Record<string, Handler> = {};

const mockExecFileAsync = vi.fn();
const mockUnlink = vi.fn();
const mockRename = vi.fn();

vi.mock('electron', () => ({
  ipcMain: {
    handle: (e: string, h: Handler) => {
      ipcHandle[e] = h;
    },
  },
}));

vi.mock('child_process', () => ({ execFile: vi.fn() }));

vi.mock('util', () => ({
  promisify:
    () =>
    async (...args: unknown[]) =>
      mockExecFileAsync(...args),
}));

vi.mock('fs/promises', () => ({
  default: {
    unlink: (...a: unknown[]) => mockUnlink(...a),
    rename: (...a: unknown[]) => mockRename(...a),
  },
  unlink: (...a: unknown[]) => mockUnlink(...a),
  rename: (...a: unknown[]) => mockRename(...a),
}));

vi.mock('path', async () => {
  const actual = await vi.importActual<typeof import('path')>('path');
  return { ...actual, default: actual };
});

vi.mock('@/main/utils/ffmpeg', () => ({
  getFFmpegPath: () => '/bin/ffmpeg',
}));

vi.mock('@/main/utils/paths', () => ({
  getPublicAssetPath: (rel: string) => `/assets/${rel}`,
}));

vi.mock('@/types/audio', () => ({
  KEYBOARD_SOUND_SAMPLES_PER_TYPE: 5,
}));

describe('keyboard-sound handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
    mockExecFileAsync.mockResolvedValue({ stdout: '', stderr: '' });
    mockUnlink.mockResolvedValue(undefined);
    mockRename.mockResolvedValue(undefined);
  });

  it('returns error for empty key presses', async () => {
    const { registerKeyboardSoundHandlers } =
      await import('@/main/capture/video/ipc/keyboard-sound-handlers');
    registerKeyboardSoundHandlers();
    const result = (await ipcHandle['video-editor:generate-keyboard-audio'](
      {},
      {
        keyPresses: [],
        soundType: 'mechanical',
        duration: 10,
        outputPath: '/p/out.aac',
      }
    )) as { success: boolean; error?: string };
    expect(result.success).toBe(false);
    expect(result.error).toBe('No key presses provided');
  });

  it('generates single chunk for small input', async () => {
    const { registerKeyboardSoundHandlers } =
      await import('@/main/capture/video/ipc/keyboard-sound-handlers');
    registerKeyboardSoundHandlers();
    const result = await ipcHandle['video-editor:generate-keyboard-audio'](
      {},
      {
        keyPresses: [{ timestamp: 0 }, { timestamp: 1 }],
        soundType: 'mechanical',
        duration: 10,
        outputPath: '/p/out.aac',
      }
    );
    expect(result).toEqual({ success: true });
    expect(mockExecFileAsync).toHaveBeenCalled();
    const args = mockExecFileAsync.mock.calls[0][1];
    expect(args.join(' ')).toContain('amix');
  });

  it('returns error on ffmpeg failure', async () => {
    mockExecFileAsync.mockRejectedValue(new Error('codec'));
    const { registerKeyboardSoundHandlers } =
      await import('@/main/capture/video/ipc/keyboard-sound-handlers');
    registerKeyboardSoundHandlers();
    const result = (await ipcHandle['video-editor:generate-keyboard-audio'](
      {},
      {
        keyPresses: [{ timestamp: 0 }],
        soundType: 'mechanical',
        duration: 10,
        outputPath: '/p/out.aac',
      }
    )) as { success: boolean; error?: string };
    expect(result.success).toBe(false);
    expect(result.error).toBe('codec');
  });

  it('chunks input larger than MAX_AMIX_INPUTS', async () => {
    const keyPresses = Array.from({ length: 51 }, (_, i) => ({ timestamp: i }));
    const { registerKeyboardSoundHandlers } =
      await import('@/main/capture/video/ipc/keyboard-sound-handlers');
    registerKeyboardSoundHandlers();
    const result = await ipcHandle['video-editor:generate-keyboard-audio'](
      {},
      {
        keyPresses,
        soundType: 'mechanical',
        duration: 60,
        outputPath: '/p/out.aac',
      }
    );
    expect(result).toEqual({ success: true });
    expect(mockExecFileAsync.mock.calls.length).toBeGreaterThan(1);
  });
});
