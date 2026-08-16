import { beforeEach, describe, expect, it, vi } from 'vitest';

type ProgressListener = (event: unknown, ...args: unknown[]) => void;

const mockInvoke = vi.fn();
const mockOn = vi.fn();
const mockOff = vi.fn();
const progressListeners = new Set<ProgressListener>();

vi.stubGlobal('window', {
  ipcRenderer: {
    invoke: mockInvoke,
    on: (channel: string, listener: ProgressListener) => {
      progressListeners.add(listener);
      mockOn(channel, listener);
    },
    off: (channel: string, listener: ProgressListener) => {
      progressListeners.delete(listener);
      mockOff(channel, listener);
    },
  },
});

function emitMuxProgress(percent: number): void {
  for (const listener of progressListeners) {
    listener({}, percent);
  }
}

const UNCUT_SEGMENTS = [{ originalStart: 0, originalEnd: 10 }];
const CUT_SEGMENTS = [
  { originalStart: 0, originalEnd: 4 },
  { originalStart: 6, originalEnd: 10 },
];

describe('audio muxer', () => {
  beforeEach(() => {
    vi.resetModules();
    mockInvoke.mockReset();
    mockOn.mockReset();
    mockOff.mockReset();
    progressListeners.clear();
  });

  it('reports a failed video-only rename', async () => {
    mockInvoke.mockResolvedValue({ success: false, error: 'rename failed' });
    const { muxAudioWithVideo } =
      await import('@/renderer/components/video-editor/export/audio-muxer');

    const result = await muxAudioWithVideo({
      videoPath: '/tmp/video.mp4',
      audioTracks: [],
      outputPath: '/output/video.mp4',
      segments: [],
      outputDurationSeconds: 10,
    });

    expect(result).toEqual({ success: false, error: 'rename failed' });
  });

  it('completes a video-only export after a successful rename', async () => {
    mockInvoke.mockResolvedValue({ success: true });
    const onProgress = vi.fn();
    const { muxAudioWithVideo } =
      await import('@/renderer/components/video-editor/export/audio-muxer');

    const result = await muxAudioWithVideo({
      videoPath: '/tmp/video.mp4',
      audioTracks: [],
      outputPath: '/output/video.mp4',
      segments: [],
      outputDurationSeconds: 10,
      onProgress,
    });

    expect(result).toEqual({ success: true });
    expect(mockInvoke).toHaveBeenCalledWith('file:rename', {
      oldPath: '/tmp/video.mp4',
      newPath: '/output/video.mp4',
    });
    expect(onProgress).toHaveBeenCalledWith(100);
  });

  it('rejects an invalid output duration before muxing', async () => {
    mockInvoke.mockResolvedValue({ success: true });
    const { muxAudioWithVideo } =
      await import('@/renderer/components/video-editor/export/audio-muxer');

    const result = await muxAudioWithVideo({
      videoPath: '/tmp/video.mp4',
      audioTracks: [{ path: '/audio/mic.aac', volume: 1 }],
      outputPath: '/output/video.mp4',
      segments: UNCUT_SEGMENTS,
      outputDurationSeconds: 0,
    });

    expect(result).toEqual({
      success: false,
      error: 'Invalid output duration',
    });
    expect(mockInvoke).not.toHaveBeenCalledWith(
      'video-editor:mux-audio',
      expect.anything()
    );
  });

  it('skips extraction for an uncut timeline and muxes source audio directly', async () => {
    mockInvoke.mockResolvedValue({ success: true });
    const { muxAudioWithVideo } =
      await import('@/renderer/components/video-editor/export/audio-muxer');

    const result = await muxAudioWithVideo({
      videoPath: '/tmp/video.mp4',
      audioTracks: [{ path: '/audio/mic.m4a', volume: 0.75 }],
      outputPath: '/output/video.mp4',
      segments: UNCUT_SEGMENTS,
      outputDurationSeconds: 10,
    });

    expect(result).toEqual({ success: true });
    expect(mockInvoke).not.toHaveBeenCalledWith(
      'video-editor:extract-audio-segments',
      expect.anything()
    );
    expect(mockInvoke).toHaveBeenCalledWith('video-editor:mux-audio', {
      videoPath: '/tmp/video.mp4',
      audioTracks: [{ path: '/audio/mic.m4a', volume: 0.75 }],
      outputPath: '/output/video.mp4',
      audioDelaySeconds: 0,
      durationSeconds: 10,
    });
  });

  it('extracts once per track for a trimmed timeline and muxes the results', async () => {
    mockInvoke.mockResolvedValue({ success: true });
    const { muxAudioWithVideo } =
      await import('@/renderer/components/video-editor/export/audio-muxer');

    const result = await muxAudioWithVideo({
      videoPath: '/tmp/video.mp4',
      audioTracks: [
        { path: '/audio/mic.aac', volume: 0.75 },
        {
          path: '/audio/keyboard.m4a',
          volume: 0.7,
          skipSegmentExtraction: true,
        },
      ],
      outputPath: '/output/video.mp4',
      segments: CUT_SEGMENTS,
      outputDurationSeconds: 8,
    });

    const extractedTempPath = expect.stringMatching(
      /^\/output\/video\.mp4\.temp-[0-9a-f-]+\.temp_audio_0\.aac$/
    );
    expect(result).toEqual({ success: true });
    expect(mockInvoke).toHaveBeenCalledTimes(4);
    expect(mockInvoke).toHaveBeenCalledWith(
      'video-editor:extract-audio-segments',
      {
        inputPath: '/audio/mic.aac',
        outputPath: extractedTempPath,
        segments: [
          { start: 0, end: 4 },
          { start: 6, end: 10 },
        ],
      }
    );
    expect(mockInvoke).toHaveBeenCalledWith('video-editor:mux-audio', {
      videoPath: '/tmp/video.mp4',
      audioTracks: [
        { path: extractedTempPath, volume: 0.75 },
        { path: '/audio/keyboard.m4a', volume: 0.7 },
      ],
      outputPath: '/output/video.mp4',
      audioDelaySeconds: 0,
      durationSeconds: 8,
    });
    expect(mockInvoke).toHaveBeenCalledWith('video-editor:delete-temp-file', {
      filePath: extractedTempPath,
    });
  });

  it('reports monotonic phase progress and interpolates the final mux', async () => {
    mockInvoke.mockImplementation((channel: string) => {
      if (channel === 'video-editor:mux-audio') {
        emitMuxProgress(0);
        emitMuxProgress(50);
        emitMuxProgress(100);
      }
      return Promise.resolve({ success: true });
    });
    const progress: number[] = [];
    const { muxAudioWithVideo } =
      await import('@/renderer/components/video-editor/export/audio-muxer');

    const result = await muxAudioWithVideo({
      videoPath: '/tmp/video.mp4',
      audioTracks: [{ path: '/audio/mic.aac', volume: 1 }],
      outputPath: '/output/video.mp4',
      segments: UNCUT_SEGMENTS,
      outputDurationSeconds: 12.5,
      onProgress: percent => progress.push(percent),
    });

    expect(result).toEqual({ success: true });
    expect(progress).toEqual([0, 50, 100, 100]);
    expect(progress).toEqual([...progress].sort((a, b) => a - b));
    expect(mockInvoke).toHaveBeenCalledWith(
      'video-editor:mux-audio',
      expect.objectContaining({ durationSeconds: 12.5 })
    );
    expect(mockOn).toHaveBeenCalledWith(
      'video-editor:mux-audio:progress',
      expect.any(Function)
    );
    expect(mockOff).toHaveBeenCalledWith(
      'video-editor:mux-audio:progress',
      mockOn.mock.calls[0][1]
    );
    expect(progressListeners.size).toBe(0);
  });

  it('unsubscribes the mux progress listener when muxing fails', async () => {
    mockInvoke.mockImplementation((channel: string) => {
      if (channel === 'video-editor:mux-audio') {
        return Promise.reject(new Error('mux crashed'));
      }
      return Promise.resolve({ success: true });
    });
    const { muxAudioWithVideo } =
      await import('@/renderer/components/video-editor/export/audio-muxer');

    const result = await muxAudioWithVideo({
      videoPath: '/tmp/video.mp4',
      audioTracks: [{ path: '/audio/mic.aac', volume: 1 }],
      outputPath: '/output/video.mp4',
      segments: UNCUT_SEGMENTS,
      outputDurationSeconds: 10,
    });

    expect(result).toEqual({ success: false, error: 'mux crashed' });
    expect(progressListeners.size).toBe(0);
  });

  it('muxes embedded system audio with a separate microphone track in one pass', async () => {
    mockInvoke.mockResolvedValue({ success: true });
    const { muxAudioWithVideo } =
      await import('@/renderer/components/video-editor/export/audio-muxer');

    const result = await muxAudioWithVideo({
      videoPath: '/tmp/video.mp4',
      audioTracks: [{ path: '/audio/mic.aac', volume: 0.75 }],
      embeddedAudio: { sourcePath: '/source/video.mp4', volume: 0.5 },
      outputPath: '/output/video.mp4',
      segments: UNCUT_SEGMENTS,
      outputDurationSeconds: 10,
    });

    expect(result).toEqual({ success: true });
    expect(mockInvoke).not.toHaveBeenCalledWith(
      'video-editor:extract-audio-segments',
      expect.anything()
    );
    expect(mockInvoke).toHaveBeenCalledWith('video-editor:mux-audio', {
      videoPath: '/tmp/video.mp4',
      audioTracks: [
        { path: '/source/video.mp4', volume: 0.5 },
        { path: '/audio/mic.aac', volume: 0.75 },
      ],
      outputPath: '/output/video.mp4',
      audioDelaySeconds: 0,
      durationSeconds: 10,
    });
  });

  it('does not create a video-only final file when audio extraction fails', async () => {
    mockInvoke.mockImplementation((channel: string) => {
      if (channel === 'video-editor:extract-audio-segments') {
        return Promise.resolve({ success: false, error: 'audio corrupt' });
      }
      return Promise.resolve({ success: true });
    });
    const { muxAudioWithVideo } =
      await import('@/renderer/components/video-editor/export/audio-muxer');

    const result = await muxAudioWithVideo({
      videoPath: '/tmp/video.mp4',
      audioTracks: [{ path: '/audio/mic.aac', volume: 1 }],
      outputPath: '/output/video.mp4',
      segments: CUT_SEGMENTS,
      outputDurationSeconds: 8,
    });

    expect(result).toEqual({ success: false, error: 'audio corrupt' });
    expect(mockInvoke).not.toHaveBeenCalledWith(
      'video-editor:mux-audio',
      expect.anything()
    );
    expect(mockInvoke).not.toHaveBeenCalledWith(
      'file:rename',
      expect.anything()
    );
  });

  it('applies the first-frame delay to the final mux only', async () => {
    mockInvoke.mockResolvedValue({ success: true });
    const { muxAudioWithVideo } =
      await import('@/renderer/components/video-editor/export/audio-muxer');

    await muxAudioWithVideo({
      videoPath: '/tmp/video.mp4',
      audioTracks: [{ path: '/audio/mic.aac', volume: 1 }],
      outputPath: '/output/video.mp4',
      segments: UNCUT_SEGMENTS,
      audioDelaySeconds: 2,
      outputDurationSeconds: 12,
    });

    expect(mockInvoke).toHaveBeenCalledWith(
      'video-editor:mux-audio',
      expect.objectContaining({
        audioDelaySeconds: 2,
        durationSeconds: 12,
      })
    );
  });
});
