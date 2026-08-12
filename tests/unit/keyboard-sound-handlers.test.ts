import { describe, it, expect, vi, beforeEach } from 'vitest';

type Handler = (...args: unknown[]) => unknown;
const ipcHandle: Record<string, Handler> = {};

const mockExecFileAsync = vi.fn();
const mockUnlink = vi.fn();
const mockRename = vi.fn();
const mockGetExportAbortSignal = vi.fn();
const exportEvent = { sender: { id: 1 } };

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

vi.mock('@/main/capture/video/ipc/export-session', () => ({
  getExportAbortSignal: (...args: unknown[]) =>
    mockGetExportAbortSignal(...args),
  isExportOutputPathAllowed: () => true,
}));

vi.mock('@/types/audio', () => ({
  KEYBOARD_SOUND_SAMPLES_PER_TYPE: 5,
  KEYBOARD_SOUND_OPTIONS: [
    { value: 'cherry-blue', label: 'Cherry MX Blue' },
    { value: 'cherry-brown', label: 'Cherry MX Brown' },
    { value: 'cherry-red', label: 'Cherry MX Red' },
  ],
}));

describe('keyboard-sound handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
    mockExecFileAsync.mockResolvedValue({ stdout: '', stderr: '' });
    mockUnlink.mockResolvedValue(undefined);
    mockRename.mockResolvedValue(undefined);
    mockGetExportAbortSignal.mockReturnValue(undefined);
  });

  it('returns error for empty key presses', async () => {
    const { registerKeyboardSoundHandlers } =
      await import('@/main/capture/video/ipc/keyboard-sound-handlers');
    registerKeyboardSoundHandlers();
    const result = (await ipcHandle['video-editor:generate-keyboard-audio'](
      exportEvent,
      {
        keyPresses: [],
        soundType: 'cherry-blue',
        duration: 10,
        outputPath: '/p/out.aac',
      }
    )) as { success: boolean; error?: string };
    expect(result.success).toBe(false);
    expect(result.error).toBe('No key presses provided');
  });

  it('generates single chunk for small input', async () => {
    const controller = new AbortController();
    mockGetExportAbortSignal.mockReturnValue(controller.signal);
    const { registerKeyboardSoundHandlers } =
      await import('@/main/capture/video/ipc/keyboard-sound-handlers');
    registerKeyboardSoundHandlers();
    const result = await ipcHandle['video-editor:generate-keyboard-audio'](
      exportEvent,
      {
        keyPresses: [{ timestamp: 0 }, { timestamp: 1 }],
        soundType: 'cherry-blue',
        duration: 10,
        outputPath: '/p/out.aac',
      }
    );
    expect(result).toEqual({ success: true });
    expect(mockExecFileAsync).toHaveBeenCalled();
    const args = mockExecFileAsync.mock.calls[0][1];
    expect(args.join(' ')).toContain('amix');
    expect(mockExecFileAsync.mock.calls[0][2]).toEqual({
      maxBuffer: 50 * 1024 * 1024,
      signal: controller.signal,
    });
  });

  it('returns error on ffmpeg failure', async () => {
    mockExecFileAsync.mockRejectedValue(new Error('codec'));
    const { registerKeyboardSoundHandlers } =
      await import('@/main/capture/video/ipc/keyboard-sound-handlers');
    registerKeyboardSoundHandlers();
    const result = (await ipcHandle['video-editor:generate-keyboard-audio'](
      exportEvent,
      {
        keyPresses: [{ timestamp: 0 }],
        soundType: 'cherry-blue',
        duration: 10,
        outputPath: '/p/out.aac',
      }
    )) as { success: boolean; error?: string };
    expect(result.success).toBe(false);
    expect(result.error).toBe('codec');
    expect(mockUnlink).toHaveBeenCalledWith('/p/out.aac');
  });

  it('chunks input larger than MAX_AMIX_INPUTS', async () => {
    const keyPresses = Array.from({ length: 51 }, (_, i) => ({ timestamp: i }));
    const { registerKeyboardSoundHandlers } =
      await import('@/main/capture/video/ipc/keyboard-sound-handlers');
    registerKeyboardSoundHandlers();
    const result = await ipcHandle['video-editor:generate-keyboard-audio'](
      exportEvent,
      {
        keyPresses,
        soundType: 'cherry-blue',
        duration: 60,
        outputPath: '/p/out.aac',
      }
    );
    expect(result).toEqual({ success: true });
    expect(mockExecFileAsync.mock.calls.length).toBeGreaterThan(1);
  });

  it('uses unique chunk files for concurrent exports', async () => {
    const keyPresses = Array.from({ length: 51 }, (_, i) => ({ timestamp: i }));
    const { registerKeyboardSoundHandlers } =
      await import('@/main/capture/video/ipc/keyboard-sound-handlers');
    registerKeyboardSoundHandlers();
    const params = {
      keyPresses,
      soundType: 'cherry-blue',
      duration: 60,
      outputPath: '/p/out.aac',
    };

    await ipcHandle['video-editor:generate-keyboard-audio'](
      exportEvent,
      params
    );
    const firstChunkPath = (mockExecFileAsync.mock.calls[0][1] as string[]).at(
      -1
    );
    await ipcHandle['video-editor:generate-keyboard-audio'](
      exportEvent,
      params
    );
    const secondChunkPath = (mockExecFileAsync.mock.calls[3][1] as string[]).at(
      -1
    );

    expect(firstChunkPath).not.toBe(secondChunkPath);
  });

  it('rejects invalid keyboard audio timing', async () => {
    const { registerKeyboardSoundHandlers } =
      await import('@/main/capture/video/ipc/keyboard-sound-handlers');
    registerKeyboardSoundHandlers();

    const invalidDuration = await ipcHandle[
      'video-editor:generate-keyboard-audio'
    ](exportEvent, {
      keyPresses: [{ timestamp: 0 }],
      soundType: 'cherry-blue',
      duration: 0,
      outputPath: '/p/out.aac',
    });
    const invalidTimestamp = await ipcHandle[
      'video-editor:generate-keyboard-audio'
    ](exportEvent, {
      keyPresses: [{ timestamp: 10 }],
      soundType: 'cherry-blue',
      duration: 10,
      outputPath: '/p/out.aac',
    });

    expect(invalidDuration).toEqual({
      success: false,
      error: 'Invalid keyboard audio duration',
    });
    expect(invalidTimestamp).toEqual({
      success: false,
      error: 'Invalid key press timestamp',
    });
    expect(mockExecFileAsync).not.toHaveBeenCalled();
  });

  it('rejects an unknown keyboard sound type', async () => {
    const { registerKeyboardSoundHandlers } =
      await import('@/main/capture/video/ipc/keyboard-sound-handlers');
    registerKeyboardSoundHandlers();

    const result = await ipcHandle['video-editor:generate-keyboard-audio'](
      exportEvent,
      {
        keyPresses: [{ timestamp: 0 }],
        soundType: 'unknown',
        duration: 10,
        outputPath: '/p/out.aac',
      }
    );

    expect(result).toEqual({
      success: false,
      error: 'Invalid keyboard sound type',
    });
    expect(mockExecFileAsync).not.toHaveBeenCalled();
  });
});
