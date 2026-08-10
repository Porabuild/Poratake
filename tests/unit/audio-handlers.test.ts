import { describe, it, expect, vi, beforeEach } from 'vitest';

type Handler = (...args: unknown[]) => unknown;
const ipcHandle: Record<string, Handler> = {};

const mockExecFileAsync = vi.fn();
const mockWriteFile = vi.fn();
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

vi.mock('@/main/utils/ffmpeg', () => ({
  getFFmpegPath: () => '/bin/ffmpeg',
}));

vi.mock('@/main/capture/video/ipc/export-session', () => ({
  getExportAbortSignal: (...args: unknown[]) =>
    mockGetExportAbortSignal(...args),
  isExportOutputPathAllowed: () => true,
}));

vi.mock('child_process', () => ({
  execFile: (
    cmd: string,
    args: string[],
    cb: (err: Error | null, out?: unknown) => void
  ) => mockExecFileAsync(cmd, args, cb),
}));

vi.mock('util', () => ({
  promisify:
    () =>
    (...args: unknown[]) => {
      const [cmd, fnArgs, options] = args;
      return new Promise((resolve, reject) => {
        try {
          mockExecFileAsync(cmd, fnArgs, options);
          resolve({ stdout: '', stderr: '' });
        } catch (e) {
          reject(e);
        }
      });
    },
}));

vi.mock('fs/promises', () => ({
  writeFile: (...a: unknown[]) => mockWriteFile(...a),
  unlink: (...a: unknown[]) => mockUnlink(...a),
  rename: (...a: unknown[]) => mockRename(...a),
}));

vi.mock('path', () => ({
  dirname: (p: string) => p.split('/').slice(0, -1).join('/'),
}));

describe('audio handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
    mockExecFileAsync.mockReturnValue(undefined);
    mockUnlink.mockResolvedValue(undefined);
    mockGetExportAbortSignal.mockReturnValue(undefined);
  });

  it('extract-audio runs ffmpeg with -vn', async () => {
    const controller = new AbortController();
    mockGetExportAbortSignal.mockReturnValue(controller.signal);
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = await ipcHandle['video-editor:extract-audio'](exportEvent, {
      inputPath: '/p/in.mov',
      outputPath: '/p/out.m4a',
    });
    expect(result).toEqual({ success: true });
    expect(mockExecFileAsync).toHaveBeenCalled();
    const args = mockExecFileAsync.mock.calls[0][1];
    expect(args).toContain('-vn');
    expect(mockExecFileAsync.mock.calls[0][2]).toEqual({
      signal: controller.signal,
    });
  });

  it('extract-audio returns error on failure', async () => {
    mockExecFileAsync.mockImplementationOnce(() => {
      throw new Error('codec missing');
    });
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = (await ipcHandle['video-editor:extract-audio'](exportEvent, {
      inputPath: '/p/in',
      outputPath: '/p/out',
    })) as { success: boolean; error?: string };
    expect(result.success).toBe(false);
    expect(result.error).toBe('codec missing');
    expect(mockUnlink).toHaveBeenCalledWith('/p/out');
  });

  it('extract-audio-segments returns error for empty segments', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = (await ipcHandle['video-editor:extract-audio-segments'](
      exportEvent,
      { inputPath: '/p/in', outputPath: '/p/out', segments: [] }
    )) as { success: boolean; error?: string };
    expect(result.success).toBe(false);
    expect(result.error).toBe('No segments provided');
  });

  it('extract-audio-segments handles single segment', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = await ipcHandle['video-editor:extract-audio-segments'](
      exportEvent,
      {
        inputPath: '/p/in.mov',
        outputPath: '/p/out.aac',
        segments: [{ start: 0, end: 5 }],
      }
    );
    expect(result).toEqual({ success: true });
    expect(mockExecFileAsync).toHaveBeenCalledTimes(1);
  });

  it('extract-audio-segments concatenates multiple segments', async () => {
    mockWriteFile.mockResolvedValue(undefined);
    mockUnlink.mockResolvedValue(undefined);
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = await ipcHandle['video-editor:extract-audio-segments'](
      exportEvent,
      {
        inputPath: '/p/in.mov',
        outputPath: '/p/out.aac',
        segments: [
          { start: 0, end: 5 },
          { start: 10, end: 15 },
        ],
      }
    );
    expect(result).toEqual({ success: true });
    expect(mockWriteFile).toHaveBeenCalled();
  });

  it('uses unique scratch files for concurrent segment exports', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const params = {
      inputPath: '/p/in.mov',
      outputPath: '/p/out.aac',
      segments: [
        { start: 0, end: 5 },
        { start: 10, end: 15 },
      ],
    };

    await ipcHandle['video-editor:extract-audio-segments'](exportEvent, params);
    const firstScratchPath = (
      mockExecFileAsync.mock.calls[0][1] as string[]
    ).at(-1);
    await ipcHandle['video-editor:extract-audio-segments'](exportEvent, params);
    const secondScratchPath = (
      mockExecFileAsync.mock.calls[3][1] as string[]
    ).at(-1);

    expect(firstScratchPath).not.toBe(secondScratchPath);
  });

  it('mux-audio combines video + audio', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioPath: '/p/a.aac',
      outputPath: '/p/out.mp4',
      audioDelaySeconds: 0,
    });
    expect(result).toEqual({ success: true });
    const args = mockExecFileAsync.mock.calls[0][1];
    expect(args).toContain('-shortest');
    expect(args).toContain('apad');
  });

  it('mux-audio applies audio delay offset', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioPath: '/p/a.aac',
      outputPath: '/p/out.mp4',
      audioDelaySeconds: 1.5,
    });
    const args = mockExecFileAsync.mock.calls[0][1];
    expect(args).toContain('-itsoffset');
    expect(args).toContain('1.5');
  });

  it('passes the renderer export signal to ffmpeg', async () => {
    const controller = new AbortController();
    mockGetExportAbortSignal.mockReturnValue(controller.signal);
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();

    await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioPath: '/p/a.aac',
      outputPath: '/p/out.mp4',
    });

    expect(mockGetExportAbortSignal).toHaveBeenCalledWith(1);
    expect(mockExecFileAsync.mock.calls[0][2]).toEqual({
      signal: controller.signal,
    });
  });

  it('removes a partial mux output when ffmpeg is aborted', async () => {
    mockExecFileAsync.mockImplementationOnce(() => {
      throw new Error('The operation was aborted');
    });
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();

    const result = (await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioPath: '/p/a.aac',
      outputPath: '/p/out.mp4',
    })) as { success: boolean };

    expect(result.success).toBe(false);
    expect(mockUnlink).toHaveBeenCalledWith('/p/out.mp4');
  });

  it('mix-audio-tracks composes amix filter', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = await ipcHandle['video-editor:mix-audio-tracks'](
      exportEvent,
      {
        inputPaths: ['/p/a.aac', '/p/b.aac'],
        outputPath: '/p/out.aac',
      }
    );
    expect(result).toEqual({ success: true });
    const args = mockExecFileAsync.mock.calls[0][1];
    expect(args).toContain('-filter_complex');
    expect(args.join(' ')).toContain('amix=inputs=2');
  });

  it('mix-audio-tracks applies per-track volumes', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    await ipcHandle['video-editor:mix-audio-tracks'](exportEvent, {
      inputPaths: ['/p/a.aac', '/p/b.aac'],
      outputPath: '/p/out.aac',
      volumes: [0.5, 1],
    });
    const args = mockExecFileAsync.mock.calls[0][1];
    expect(args.join(' ')).toContain('volume=0.5');
  });

  it('adjust-audio-volume runs volume filter', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    await ipcHandle['video-editor:adjust-audio-volume'](exportEvent, {
      inputPath: '/p/in.aac',
      outputPath: '/p/out.aac',
      volume: 1.5,
    });
    const args = mockExecFileAsync.mock.calls[0][1];
    expect(args).toContain('-af');
    expect(args.join(' ')).toContain('volume=1.5');
  });

  it('extract-audio-segments-with-speed handles empty', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = (await ipcHandle[
      'video-editor:extract-audio-segments-with-speed'
    ](exportEvent, {
      inputPath: '/p/in',
      outputPath: '/p/out',
      segments: [],
    })) as {
      success: boolean;
      error?: string;
    };
    expect(result.success).toBe(false);
  });

  it('rejects invalid segment speeds before starting ffmpeg', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();

    const result = (await ipcHandle[
      'video-editor:extract-audio-segments-with-speed'
    ](exportEvent, {
      inputPath: '/p/in.mov',
      outputPath: '/p/out.aac',
      segments: [{ start: 0, end: 5, speed: 0 }],
    })) as { success: boolean; error?: string };

    expect(result).toEqual({
      success: false,
      error: 'Invalid audio segment',
    });
    expect(mockExecFileAsync).not.toHaveBeenCalled();
  });

  it('extract-audio-segments-with-speed runs one segment with rename', async () => {
    mockRename.mockResolvedValue(undefined);
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = await ipcHandle[
      'video-editor:extract-audio-segments-with-speed'
    ](exportEvent, {
      inputPath: '/p/in.mov',
      outputPath: '/p/out.aac',
      segments: [{ start: 0, end: 5, speed: 1 }],
    });
    expect(result).toEqual({ success: true });
    expect(mockRename).toHaveBeenCalled();
  });

  it('extract-audio-segments-with-speed handles speed > 1 (atempo)', async () => {
    mockRename.mockResolvedValue(undefined);
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    await ipcHandle['video-editor:extract-audio-segments-with-speed'](
      exportEvent,
      {
        inputPath: '/p/in.mov',
        outputPath: '/p/out.aac',
        segments: [{ start: 0, end: 5, speed: 3 }],
      }
    );
    const args = mockExecFileAsync.mock.calls[0][1];
    expect(args).toContain('-af');
    expect(args.join(' ')).toContain('atempo');
  });

  it('extract-audio-segments-with-speed handles slow speed', async () => {
    mockRename.mockResolvedValue(undefined);
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    await ipcHandle['video-editor:extract-audio-segments-with-speed'](
      exportEvent,
      {
        inputPath: '/p/in.mov',
        outputPath: '/p/out.aac',
        segments: [{ start: 0, end: 5, speed: 0.25 }],
      }
    );
    const args = mockExecFileAsync.mock.calls[0][1];
    expect(args.join(' ')).toContain('atempo=0.5');
  });

  it('extract-audio-segments-with-speed concatenates multiple segments', async () => {
    mockWriteFile.mockResolvedValue(undefined);
    mockUnlink.mockResolvedValue(undefined);
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = await ipcHandle[
      'video-editor:extract-audio-segments-with-speed'
    ](exportEvent, {
      inputPath: '/p/in.mov',
      outputPath: '/p/out.aac',
      segments: [
        { start: 0, end: 5, speed: 1 },
        { start: 5, end: 10, speed: 2 },
      ],
    });
    expect(result).toEqual({ success: true });
  });
});
