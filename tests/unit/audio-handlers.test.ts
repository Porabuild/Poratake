import { describe, it, expect, vi, beforeEach } from 'vitest';
import { EventEmitter } from 'events';
import { existsSync } from 'node:fs';
import { type as osType } from 'node:os';
import type * as NodeChildProcess from 'node:child_process';

type Handler = (...args: unknown[]) => unknown;
const ipcHandle: Record<string, Handler> = {};

class FakeChildProcess extends EventEmitter {
  stdout = new EventEmitter();
  stderr = new EventEmitter();
  kill = vi.fn();
}

const mockExecFileAsync = vi.fn();
const mockSpawn = vi.fn();
const mockSend = vi.fn();
const mockWriteFile = vi.fn();
const mockUnlink = vi.fn();
const mockGetExportAbortSignal = vi.fn();
const exportEvent = { sender: { id: 1, send: mockSend } };
const isWindowsHost = process.platform === 'win32' || osType() === 'Windows_NT';
const pathSeparator = isWindowsHost ? '\\' : '/';
const joinPath = (...parts: string[]): string => parts.join(pathSeparator);
const bundledFfmpegCandidate = joinPath(
  process.cwd(),
  'src',
  'main',
  'binaries',
  'ffmpeg',
  isWindowsHost ? 'ffmpeg.exe' : 'ffmpeg'
);
const bundledFfmpegPath = existsSync(bundledFfmpegCandidate)
  ? bundledFfmpegCandidate
  : undefined;

function pcmToneAmplitude(
  bytes: Buffer,
  frequency: number,
  startFrame: number,
  endFrame: number
): number {
  let sine = 0;
  let cosine = 0;
  for (let frame = startFrame; frame < endFrame; frame++) {
    const sample = bytes.readInt16LE(frame * 2);
    const phase = (2 * Math.PI * frequency * frame) / 48000;
    sine += sample * Math.sin(phase);
    cosine += sample * Math.cos(phase);
  }
  return (2 * Math.hypot(sine, cosine)) / (endFrame - startFrame);
}

function expectToneLevel(actual: number, expected: number): void {
  expect(Math.abs(actual / expected - 1)).toBeLessThan(0.02);
}

function expectToneAbsent(actual: number, reference: number): void {
  expect(actual).toBeLessThan(reference * 0.02);
}

function renderAudioMix(
  execFile: typeof NodeChildProcess.execFileSync,
  ffmpegPath: string,
  filter: string,
  secondFrequency: number,
  secondDuration = '1'
): Buffer {
  return execFile(
    ffmpegPath,
    [
      '-hide_banner',
      '-loglevel',
      'error',
      '-f',
      'lavfi',
      '-i',
      'color=c=black:s=16x16:r=1:d=1',
      '-f',
      'lavfi',
      '-i',
      'sine=frequency=440:sample_rate=48000:duration=1',
      '-f',
      'lavfi',
      '-i',
      `sine=frequency=${secondFrequency}:sample_rate=48000:duration=${secondDuration}`,
      '-filter_complex',
      filter,
      '-map',
      '[aout]',
      '-t',
      '1',
      '-f',
      's16le',
      '-acodec',
      'pcm_s16le',
      '-ac',
      '1',
      'pipe:1',
    ],
    { maxBuffer: 1024 * 1024, timeout: 10000 }
  );
}

function renderSingleAudioTrack(
  execFile: typeof NodeChildProcess.execFileSync,
  ffmpegPath: string,
  frequency: number
): Buffer {
  return execFile(
    ffmpegPath,
    [
      '-hide_banner',
      '-loglevel',
      'error',
      '-f',
      'lavfi',
      '-i',
      `sine=frequency=${frequency}:sample_rate=48000:duration=1`,
      '-map',
      '0:a',
      '-t',
      '1',
      '-f',
      's16le',
      '-acodec',
      'pcm_s16le',
      '-ac',
      '1',
      'pipe:1',
    ],
    { maxBuffer: 1024 * 1024, timeout: 10000 }
  );
}

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
  spawn: (...args: unknown[]) => mockSpawn(...args),
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
}));

vi.mock('path', () => ({
  dirname: (p: string) => p.split('/').slice(0, -1).join('/'),
}));

function spawnWith(script: (child: FakeChildProcess) => void): void {
  mockSpawn.mockImplementation(() => {
    const child = new FakeChildProcess();
    setTimeout(() => script(child), 0);
    return child;
  });
}

function lastSpawnArgs(): string[] {
  return mockSpawn.mock.calls[mockSpawn.mock.calls.length - 1][1] as string[];
}

describe('audio handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
    mockExecFileAsync.mockReset();
    mockExecFileAsync.mockReturnValue(undefined);
    mockUnlink.mockResolvedValue(undefined);
    mockGetExportAbortSignal.mockReturnValue(undefined);
    spawnWith(child => child.emit('close', 0));
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

  it('extract-audio-segments handles a single segment in one ffmpeg pass', async () => {
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
    const args = mockExecFileAsync.mock.calls[0][1] as string[];
    expect(args).toContain('-filter_complex');
    expect(args.join(' ')).toContain('atrim=start=0:end=5');
    expect(args.join(' ')).not.toContain('concat');
    expect(args.at(-1)).toBe('/p/out.aac');
  });

  it('extract-audio-segments concatenates multiple segments in one ffmpeg pass', async () => {
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
    expect(mockExecFileAsync).toHaveBeenCalledTimes(1);
    const args = mockExecFileAsync.mock.calls[0][1] as string[];
    const filter = args.join(' ');
    expect(filter).toContain('asplit=2');
    expect(filter).toContain('atrim=start=0:end=5');
    expect(filter).toContain('atrim=start=10:end=15');
    expect(filter).toContain('concat=n=2:v=0:a=1');
  });

  it('mux-audio combines video and audio with stream copy', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioTracks: [{ path: '/p/a.aac' }],
      outputPath: '/p/out.mp4',
      audioDelaySeconds: 0,
      durationSeconds: 10,
    });
    expect(result).toEqual({ success: true });
    const args = lastSpawnArgs();
    expect(args.slice(0, 3)).toEqual(['-progress', 'pipe:1', '-nostats']);
    expect(args).toContain('apad');
    expect(args).toContain('-t');
    expect(args).toContain('10');
    expect(args).not.toContain('-shortest');
    expect(args).toContain('-c:v');
    expect(args.join(' ')).toContain('-map 0:v -map 1:a');
    expect(args.at(-1)).toBe('/p/out.mp4');
  });

  it('mux-audio falls back to -shortest without a duration', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioTracks: [{ path: '/p/a.aac' }],
      outputPath: '/p/out.mp4',
    });
    const args = lastSpawnArgs();
    expect(args).toContain('-shortest');
    expect(args).not.toContain('-t');
  });

  it('mux-audio applies volume and delay inside one filter graph', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioTracks: [{ path: '/p/a.aac', volume: 0.5 }],
      outputPath: '/p/out.mp4',
      audioDelaySeconds: 1.5,
      durationSeconds: 10,
    });
    expect(result).toEqual({ success: true });
    const args = lastSpawnArgs();
    const command = args.join(' ');
    expect(command).toContain('volume=0.5');
    expect(command).toContain('adelay=1500:all=1');
    expect(command).toContain('apad');
    expect(args).toContain('-map');
    expect(args).toContain('[aout]');
    expect(args).toContain('-t');
  });

  it('mux-audio mixes multiple tracks with per-track volumes', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioTracks: [{ path: '/p/a.aac', volume: 0.5 }, { path: '/p/b.aac' }],
      outputPath: '/p/out.mp4',
      durationSeconds: 10,
    });
    expect(result).toEqual({ success: true });
    const args = lastSpawnArgs();
    const command = args.join(' ');
    expect(command).toContain('volume=0.5');
    expect(command).toContain('amix=inputs=2:duration=longest:normalize=0');
    expect(command).toContain('apad');
  });

  it('mux-audio preserves source levels for overlapping, muted, and ended tracks', async ({
    skip,
  }) => {
    const { execFileSync, spawnSync } =
      await vi.importActual<typeof NodeChildProcess>('node:child_process');
    let ffmpegPath = bundledFfmpegPath;
    if (!ffmpegPath) {
      const probe = spawnSync('ffmpeg', ['-version'], {
        encoding: 'utf8',
        timeout: 5000,
      });
      if (probe.error?.code === 'ENOENT') {
        skip('FFmpeg executable is not available');
        return;
      }
      if (probe.error) throw probe.error;
      if (probe.status !== 0) {
        throw new Error(`FFmpeg probe failed: ${probe.stderr}`);
      }
      ffmpegPath = 'ffmpeg';
    }
    const { buildMuxAudioFilter } =
      await import('@/main/capture/video/ipc/audio-handlers');
    const baseline440 = renderSingleAudioTrack(execFileSync, ffmpegPath, 440);
    const baseline880 = renderSingleAudioTrack(execFileSync, ffmpegPath, 880);
    const overlapping = renderAudioMix(
      execFileSync,
      ffmpegPath,
      buildMuxAudioFilter(2, [1, 1], 0) ?? '',
      880
    );
    const muted = renderAudioMix(
      execFileSync,
      ffmpegPath,
      buildMuxAudioFilter(2, [1, 0], 0) ?? '',
      880
    );
    const ended = renderAudioMix(
      execFileSync,
      ffmpegPath,
      buildMuxAudioFilter(2, [1, 1], 0) ?? '',
      880,
      '0.25'
    );
    for (const output of [
      baseline440,
      baseline880,
      overlapping,
      muted,
      ended,
    ]) {
      expect(output.byteLength).toBe(48000 * 2);
    }

    const overlapStart = 2400;
    const overlapEnd = 9600;
    const tailStart = 24000;
    const tailEnd = 43200;
    const baseline440Overlap = pcmToneAmplitude(
      baseline440,
      440,
      overlapStart,
      overlapEnd
    );
    const baseline880Overlap = pcmToneAmplitude(
      baseline880,
      880,
      overlapStart,
      overlapEnd
    );
    expect(baseline440Overlap).toBeGreaterThan(1000);
    expect(baseline880Overlap).toBeGreaterThan(1000);
    expectToneLevel(
      pcmToneAmplitude(overlapping, 440, overlapStart, overlapEnd),
      baseline440Overlap
    );
    expectToneLevel(
      pcmToneAmplitude(overlapping, 880, overlapStart, overlapEnd),
      baseline880Overlap
    );
    expectToneLevel(
      pcmToneAmplitude(ended, 440, overlapStart, overlapEnd),
      baseline440Overlap
    );
    expectToneLevel(
      pcmToneAmplitude(ended, 880, overlapStart, overlapEnd),
      baseline880Overlap
    );
    expectToneLevel(
      pcmToneAmplitude(muted, 440, overlapStart, overlapEnd),
      baseline440Overlap
    );
    expectToneAbsent(
      pcmToneAmplitude(muted, 880, overlapStart, overlapEnd),
      baseline880Overlap
    );
    expectToneLevel(
      pcmToneAmplitude(ended, 440, tailStart, tailEnd),
      pcmToneAmplitude(baseline440, 440, tailStart, tailEnd)
    );
    expectToneAbsent(
      pcmToneAmplitude(ended, 880, tailStart, tailEnd),
      baseline880Overlap
    );
  }, 30_000);

  it('mux-audio rejects invalid track payloads', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();

    const empty = (await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioTracks: [],
      outputPath: '/p/out.mp4',
    })) as { success: boolean; error?: string };
    expect(empty.success).toBe(false);

    const negativeVolume = (await ipcHandle['video-editor:mux-audio'](
      exportEvent,
      {
        videoPath: '/p/v.mov',
        audioTracks: [{ path: '/p/a.aac', volume: -1 }],
        outputPath: '/p/out.mp4',
      }
    )) as { success: boolean; error?: string };
    expect(negativeVolume.success).toBe(false);
    expect(mockSpawn).not.toHaveBeenCalled();
  });

  it('mux-audio reports ffmpeg progress and stops at 100', async () => {
    spawnWith(child => {
      child.stdout.emit('data', 'frame=1\nout_time_us=5000000\nprogress=');
      child.stdout.emit('data', 'continue\nout_time_us=5100000\n');
      child.stdout.emit('data', 'out_time_us=10000000\nout_time_us=99000000\n');
      child.emit('close', 0);
    });
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();

    const result = await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioTracks: [{ path: '/p/a.aac' }],
      outputPath: '/p/out.mp4',
      durationSeconds: 10,
    });

    expect(result).toEqual({ success: true });
    expect(mockSend.mock.calls.map(call => call[1] as number)).toEqual([
      50, 51, 100,
    ]);
    expect(mockSend.mock.calls[0][0]).toBe('video-editor:mux-audio:progress');
  });

  it('mux-audio falls back to out_time_ms and skips progress without a duration', async () => {
    spawnWith(child => {
      child.stdout.emit('data', 'out_time_ms=2500000\n');
      child.emit('close', 0);
    });
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();

    await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioTracks: [{ path: '/p/a.aac' }],
      outputPath: '/p/out.mp4',
      durationSeconds: 5,
    });
    expect(mockSend.mock.calls.map(call => call[1] as number)).toEqual([50]);

    mockSend.mockClear();
    await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioTracks: [{ path: '/p/a.aac' }],
      outputPath: '/p/out.mp4',
      durationSeconds: 0,
    });
    expect(mockSend).not.toHaveBeenCalled();
  });

  it('mux-audio fails when ffmpeg exits non-zero', async () => {
    spawnWith(child => {
      child.stderr.emit('data', 'Invalid data found');
      child.emit('close', 1);
    });
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();

    const result = (await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioTracks: [{ path: '/p/a.aac' }],
      outputPath: '/p/out.mp4',
    })) as { success: boolean; error?: string };

    expect(result.success).toBe(false);
    expect(result.error).toContain('Invalid data found');
    expect(mockUnlink).toHaveBeenCalledWith('/p/out.mp4');
  });

  it('removes a partial mux output when ffmpeg is aborted', async () => {
    const controller = new AbortController();
    mockGetExportAbortSignal.mockReturnValue(controller.signal);
    const children: FakeChildProcess[] = [];
    mockSpawn.mockImplementation(() => {
      const child = new FakeChildProcess();
      children.push(child);
      child.kill.mockImplementation(() => {
        setTimeout(() => child.emit('close', null), 0);
        return true;
      });
      setTimeout(() => controller.abort(), 0);
      return child;
    });
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();

    const result = (await ipcHandle['video-editor:mux-audio'](exportEvent, {
      videoPath: '/p/v.mov',
      audioTracks: [{ path: '/p/a.aac' }],
      outputPath: '/p/out.mp4',
    })) as { success: boolean };

    expect(mockGetExportAbortSignal).toHaveBeenCalledWith(1);
    expect(children[0].kill).toHaveBeenCalled();
    expect(result.success).toBe(false);
    expect(mockUnlink).toHaveBeenCalledWith('/p/out.mp4');
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

  it('extract-audio-segments-with-speed applies atempo in one pass', async () => {
    const { registerAudioHandlers } =
      await import('@/main/capture/video/ipc/audio-handlers');
    registerAudioHandlers();
    const result = await ipcHandle[
      'video-editor:extract-audio-segments-with-speed'
    ](exportEvent, {
      inputPath: '/p/in.mov',
      outputPath: '/p/out.aac',
      segments: [{ start: 0, end: 5, speed: 3 }],
    });
    expect(result).toEqual({ success: true });
    expect(mockExecFileAsync).toHaveBeenCalledTimes(1);
    const args = mockExecFileAsync.mock.calls[0][1] as string[];
    const filter = args.join(' ');
    expect(filter).toContain('atempo=2.0,atempo=1.5');
    expect(filter).toContain('asetpts=PTS-STARTPTS');
  });

  it('extract-audio-segments-with-speed chains atempo for slow speeds', async () => {
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
    const args = mockExecFileAsync.mock.calls[0][1] as string[];
    expect(args.join(' ')).toContain('atempo=0.5,atempo=0.5');
  });

  it('extract-audio-segments-with-speed mixes trimmed and re-timed segments in one pass', async () => {
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
    expect(mockExecFileAsync).toHaveBeenCalledTimes(1);
    const filter = (mockExecFileAsync.mock.calls[0][1] as string[]).join(' ');
    expect(filter).toContain('asplit=2');
    expect(filter).toContain('concat=n=2:v=0:a=1');
    expect(filter).toContain('atempo=2');
  });
});
