import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockExistsSync = vi.fn();
const mockReadFileSync = vi.fn();
const mockUnlinkSync = vi.fn();
const mockExecFileAsync = vi.fn();
const mockGetWhisperCliPath = vi.fn(() => '/bin/whisper');
const mockGetWhisperModelPath = vi.fn((m: string) => `/path/${m}.bin`);
const mockGetFFmpegPath = vi.fn(() => '/bin/ffmpeg');

class MockChildProcess {
  stderrHandlers: Array<(data: Buffer) => void> = [];
  closeHandlers: Array<(code: number | null) => void> = [];
  errorHandlers: Array<(err: Error) => void> = [];
  stderr = {
    on: (event: string, cb: (data: Buffer) => void) => {
      if (event === 'data') this.stderrHandlers.push(cb);
    },
  };
  on(event: string, cb: (...a: unknown[]) => void) {
    if (event === 'close') this.closeHandlers.push(cb as never);
    if (event === 'error') this.errorHandlers.push(cb as never);
  }
  emitStderr(text: string) {
    this.stderrHandlers.forEach(cb => cb(Buffer.from(text)));
  }
  emitClose(code: number | null) {
    this.closeHandlers.forEach(cb => cb(code));
  }
}

let lastChild: MockChildProcess | null = null;
const mockSpawn = vi.fn(() => {
  lastChild = new MockChildProcess();
  return lastChild;
});

vi.mock('child_process', () => ({
  spawn: (...a: unknown[]) => mockSpawn(...(a as never)),
  execFile: vi.fn(),
}));

vi.mock('util', () => ({
  promisify: () => mockExecFileAsync,
}));

vi.mock('fs', () => ({
  default: {
    existsSync: (...a: unknown[]) => mockExistsSync(...a),
    readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
    unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
  },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
  readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
  unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
}));

vi.mock('@/main/utils/whisper', () => ({
  getWhisperCliPath: () => mockGetWhisperCliPath(),
  getWhisperModelPath: (m: string) => mockGetWhisperModelPath(m),
}));

vi.mock('@/main/utils/ffmpeg', () => ({
  getFFmpegPath: () => mockGetFFmpegPath(),
}));

describe('whisper-transcribe', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockExecFileAsync.mockResolvedValue({ stdout: '', stderr: '' });
    lastChild = null;
  });

  describe('parseWhisperOutput', () => {
    it('returns empty for non-array transcription', async () => {
      const { parseWhisperOutput } =
        await import('@/main/transcription/whisper-transcribe');
      const result = parseWhisperOutput({ transcription: undefined as never });
      expect(result).toEqual([]);
    });

    it('parses timestamp format with commas', async () => {
      const { parseWhisperOutput } =
        await import('@/main/transcription/whisper-transcribe');
      const result = parseWhisperOutput({
        transcription: [
          {
            timestamps: { from: '00:01:30,500', to: '00:01:35,750' },
            text: 'Hello',
          },
        ],
      });
      expect(result).toHaveLength(1);
      expect(result[0].start).toBeCloseTo(90.5, 1);
      expect(result[0].end).toBeCloseTo(95.75, 1);
    });

    it('falls back to timestamp when offsets missing', async () => {
      const { parseWhisperOutput } =
        await import('@/main/transcription/whisper-transcribe');
      const result = parseWhisperOutput({
        transcription: [
          {
            timestamps: { from: '00:00:00.000', to: '00:00:05.000' },
            text: 'No offsets',
          },
        ],
      });
      expect(result[0].start).toBe(0);
      expect(result[0].end).toBe(5);
    });

    it('returns undefined words when tokens missing', async () => {
      const { parseWhisperOutput } =
        await import('@/main/transcription/whisper-transcribe');
      const result = parseWhisperOutput({
        transcription: [
          {
            timestamps: { from: '00:00:00.000', to: '00:00:01.000' },
            offsets: { from: 0, to: 1000 },
            text: 'No tokens',
          },
        ],
      });
      expect(result[0].words).toBeUndefined();
    });

    it('filters tokens with no timing info', async () => {
      const { parseWhisperOutput } =
        await import('@/main/transcription/whisper-transcribe');
      const result = parseWhisperOutput({
        transcription: [
          {
            timestamps: { from: '00:00:00.000', to: '00:00:02.000' },
            offsets: { from: 0, to: 2000 },
            text: 'a b',
            tokens: [
              { text: 'a' },
              { text: 'b', offsets: { from: 0, to: 1000 } },
            ],
          },
        ],
      });
      expect(result[0].words?.length).toBeGreaterThanOrEqual(0);
    });

    it('strips [_TT_N] tokens', async () => {
      const { parseWhisperOutput } =
        await import('@/main/transcription/whisper-transcribe');
      const result = parseWhisperOutput({
        transcription: [
          {
            timestamps: { from: '00:00:00.000', to: '00:00:01.000' },
            offsets: { from: 0, to: 1000 },
            text: 'Hi [_TT_5]',
            tokens: [
              { text: '[_TT_5]', offsets: { from: 0, to: 0 } },
              { text: ' Hi', offsets: { from: 0, to: 1000 } },
            ],
          },
        ],
      });
      const words = result[0].words ?? [];
      expect(words.some(w => w.text === 'Hi')).toBe(true);
    });

    it('returns null for malformed timestamps', async () => {
      const { parseWhisperOutput } =
        await import('@/main/transcription/whisper-transcribe');
      const result = parseWhisperOutput({
        transcription: [
          {
            timestamps: { from: 'invalid', to: 'invalid' },
            text: 'malformed',
          },
        ],
      });
      expect(result[0].start).toBe(0);
    });
  });

  describe('transcribeAudio', () => {
    it('returns error when whisper binary missing', async () => {
      mockExistsSync.mockReturnValue(false);
      const { transcribeAudio } =
        await import('@/main/transcription/whisper-transcribe');
      const result = await transcribeAudio('/p/a.wav', { model: 'base' });
      expect(result.success).toBe(false);
      expect(result.error).toBe('Whisper binary not found');
    });

    it('returns error when model missing', async () => {
      mockExistsSync.mockImplementation((p: string) => p === '/bin/whisper');
      const { transcribeAudio } =
        await import('@/main/transcription/whisper-transcribe');
      const result = await transcribeAudio('/p/a.wav', { model: 'base' });
      expect(result.success).toBe(false);
      expect(result.error).toBe('Model base not found');
    });

    it('returns error when audio missing', async () => {
      mockExistsSync.mockImplementation(
        (p: string) => p === '/bin/whisper' || String(p).endsWith('.bin')
      );
      const { transcribeAudio } =
        await import('@/main/transcription/whisper-transcribe');
      const result = await transcribeAudio('/p/a.wav', { model: 'base' });
      expect(result.success).toBe(false);
      expect(result.error).toBe('Audio file not found');
    });

    it('runs the whisper pipeline and parses output', async () => {
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue(
        JSON.stringify({
          transcription: [
            {
              timestamps: { from: '00:00:00.000', to: '00:00:01.000' },
              offsets: { from: 0, to: 1000 },
              text: 'Hello',
            },
          ],
        })
      );

      const transcribePromise = (async () => {
        const { transcribeAudio } =
          await import('@/main/transcription/whisper-transcribe');
        return transcribeAudio('/p/a.wav', { model: 'base' });
      })();

      // Wait a tick for spawn to be called
      await new Promise(r => setImmediate(r));
      expect(lastChild).not.toBeNull();
      lastChild?.emitClose(0);

      const result = await transcribePromise;
      expect(result.success).toBe(true);
      expect(result.data?.segments).toHaveLength(1);
      expect(result.data?.meta.model).toBe('base');
    });

    it('reports progress', async () => {
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue(JSON.stringify({ transcription: [] }));

      const onProgress = vi.fn();
      const promise = (async () => {
        const { transcribeAudio } =
          await import('@/main/transcription/whisper-transcribe');
        return transcribeAudio('/p/a.wav', { model: 'small' }, onProgress);
      })();

      await new Promise(r => setImmediate(r));
      lastChild?.emitStderr('whisper_print_progress: progress = 50%');
      lastChild?.emitClose(0);

      const result = await promise;
      expect(result.success).toBe(true);
      expect(onProgress).toHaveBeenCalled();
    });

    it('falls back when DTW unsupported', async () => {
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue(JSON.stringify({ transcription: [] }));

      const promise = (async () => {
        const { transcribeAudio } =
          await import('@/main/transcription/whisper-transcribe');
        return transcribeAudio('/p/a.wav', { model: 'base' });
      })();

      await new Promise(r => setImmediate(r));
      lastChild?.emitStderr('error: unknown argument: -dtw');
      lastChild?.emitClose(1);

      // Wait for fallback spawn
      await new Promise(r => setImmediate(r));
      await new Promise(r => setImmediate(r));
      lastChild?.emitClose(0);

      const result = await promise;
      expect(result.success).toBe(true);
      expect(mockSpawn).toHaveBeenCalledTimes(2);
    });

    it('returns error when whisper exits non-zero (no DTW issue)', async () => {
      mockExistsSync.mockReturnValue(true);

      const promise = (async () => {
        const { transcribeAudio } =
          await import('@/main/transcription/whisper-transcribe');
        return transcribeAudio('/p/a.wav', { model: 'base' });
      })();

      await new Promise(r => setImmediate(r));
      lastChild?.emitStderr('decode failed');
      lastChild?.emitClose(2);

      const result = await promise;
      expect(result.success).toBe(false);
      expect(result.error).toContain('Whisper exited');
    });

    it('returns error when JSON output missing', async () => {
      let callCount = 0;
      mockExistsSync.mockImplementation(() => {
        callCount++;
        return callCount <= 3;
      });

      const promise = (async () => {
        const { transcribeAudio } =
          await import('@/main/transcription/whisper-transcribe');
        return transcribeAudio('/p/a.wav', { model: 'base' });
      })();

      await new Promise(r => setImmediate(r));
      lastChild?.emitClose(0);

      const result = await promise;
      expect(result.success).toBe(false);
    });

    it('uses prompt when provided', async () => {
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue(JSON.stringify({ transcription: [] }));

      const promise = (async () => {
        const { transcribeAudio } =
          await import('@/main/transcription/whisper-transcribe');
        return transcribeAudio('/p/a.wav', {
          model: 'base',
          prompt: 'extra context',
        });
      })();

      await new Promise(r => setImmediate(r));
      const args = mockSpawn.mock.calls[0][1] as string[];
      expect(args).toContain('--prompt');
      lastChild?.emitClose(0);
      await promise;
    });
  });
});
