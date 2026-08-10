import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockMuxAudioWithVideo = vi.fn();
const mockInvoke = vi.fn();
const mockSend = vi.fn();

vi.mock('mediabunny', () => ({
  Input: class {},
  Output: class {},
  BlobSource: class {},
  Mp4OutputFormat: class {},
  StreamTarget: class {},
  CanvasSource: class {},
  VideoSampleSink: class {},
  ALL_FORMATS: [],
}));

vi.mock('@/renderer/components/video-editor/composition', () => ({
  VideoCompositionEngine: class {},
}));

vi.mock(
  '@/renderer/components/video-editor/composition/device-frame-canvas-renderer',
  () => ({ calculateDeviceFrameLayout: vi.fn() })
);

vi.mock('@/renderer/components/video-editor/utils', () => ({
  getTotalTimelineDuration: () => 10,
  timelineToVideo: (_segments: unknown, timelineTime: number) => ({
    videoTime: timelineTime,
  }),
}));

vi.mock('@/renderer/components/video-editor/export/audio-muxer', () => ({
  muxAudioWithVideo: (...args: unknown[]) => mockMuxAudioWithVideo(...args),
}));

vi.stubGlobal('window', {
  ipcRenderer: {
    invoke: mockInvoke,
    send: mockSend,
  },
});

interface AudioMuxingHarness {
  handleAudioMuxing: (params: {
    tempVideoPath: string;
    outputPath: string;
    sourceVideoPath: string;
    systemAudioPath: string | null;
    micAudioPath: string | null;
    systemAudioEnabled: boolean;
    micAudioEnabled: boolean;
    systemAudioVolume: number;
    micAudioVolume: number;
    hasEmbeddedAudio: boolean;
    musicTracks?: Array<{
      id: string;
      name: string;
      source: 'system' | 'mic' | 'music';
      fileName: string;
      volume: number;
      enabled: boolean;
      startTime: number;
      endTime: number;
      originalDuration: number;
      trimStart: number;
      trimEnd: number;
      speed: number;
    }>;
    firstFrameDuration?: number;
    segments: Array<{
      id: string;
      originalStart: number;
      originalEnd: number;
      trimMinStart: number;
      trimMaxEnd: number;
    }>;
  }) => Promise<{ success: boolean; error?: string }>;
}

interface FrameProcessingHarness {
  cancel: () => void;
  processFrames: (params: unknown) => Promise<void>;
  renderFrameToCanvas: (...args: unknown[]) => unknown;
}

const baseParams = {
  tempVideoPath: '/tmp/video.mp4',
  outputPath: '/output/video.mp4',
  sourceVideoPath: '/source/video.mp4',
  systemAudioPath: '/source/system.aac',
  micAudioPath: '/source/mic.aac',
  systemAudioEnabled: true,
  micAudioEnabled: true,
  systemAudioVolume: 0.5,
  micAudioVolume: 0.75,
  hasEmbeddedAudio: false,
  segments: [
    {
      id: 'segment',
      originalStart: 0,
      originalEnd: 10,
      trimMinStart: 0,
      trimMaxEnd: 10,
    },
  ],
};

describe('WebCodecs audio export', () => {
  beforeEach(() => {
    mockMuxAudioWithVideo.mockReset();
    mockMuxAudioWithVideo.mockResolvedValue({ success: true });
    mockInvoke.mockReset();
    mockInvoke.mockImplementation((channel: string) =>
      channel === 'video-editor:export:begin' ? 'session-1' : undefined
    );
    mockSend.mockReset();
  });

  it('keeps one main-process cancellation session around an export', async () => {
    const { WebCodecsExporter } =
      await import('@/renderer/components/video-editor/export/webcodecs-exporter');
    const exporter = new WebCodecsExporter();

    await exporter.begin();
    exporter.cancel();
    await exporter.finish();

    expect(mockInvoke).toHaveBeenNthCalledWith(1, 'video-editor:export:begin');
    expect(mockSend).toHaveBeenCalledWith(
      'video-editor:export:cancel',
      'session-1'
    );
    expect(mockInvoke).toHaveBeenNthCalledWith(
      2,
      'video-editor:export:finish',
      'session-1'
    );
  });

  it('retains cancellation state until the active export unwinds', async () => {
    const { WebCodecsExporter } =
      await import('@/renderer/components/video-editor/export/webcodecs-exporter');
    const exporter = new WebCodecsExporter();

    expect(exporter.isCancelled()).toBe(false);
    exporter.cancel();
    expect(exporter.isCancelled()).toBe(true);
  });

  it('closes current and prefetched samples when frame export is cancelled', async () => {
    const { WebCodecsExporter } =
      await import('@/renderer/components/video-editor/export/webcodecs-exporter');
    const exporter =
      new WebCodecsExporter() as unknown as FrameProcessingHarness;
    const samples = Array.from({ length: 40 }, (_, index) => ({
      timestamp: index,
      duration: 0.25,
      codedWidth: 1920,
      codedHeight: 1080,
      format: 'NV12',
      close: vi.fn(),
      toVideoFrame: () => ({
        displayWidth: 1920,
        displayHeight: 1080,
        codedWidth: 1920,
        codedHeight: 1080,
        format: 'NV12',
        close: vi.fn(),
      }),
    }));
    let sampleIndex = 0;
    const videoSink = {
      async *samplesAtTimestamps(timestamps: number[]) {
        for (let index = 0; index < timestamps.length; index++) {
          yield samples[sampleIndex++];
        }
      },
    };
    const videoSource = {
      add: vi.fn(async () => {
        exporter.cancel();
      }),
    };

    await expect(
      exporter.processFrames({
        config: { segments: [{}] },
        frameRate: 4,
        videoSink,
        cameraSink: null,
        sourceFirstTimestamp: 0,
        cameraFirstTimestamp: 0,
        engine: {
          getFirstFrameDuration: () => 0,
          renderFrame: vi.fn(),
        },
        outputCtx: {},
        videoSource,
        onProgress: vi.fn(),
      })
    ).rejects.toThrow('Export cancelled');

    expect(samples).toHaveLength(40);
    for (const sample of samples) {
      expect(sample.close).toHaveBeenCalledTimes(1);
    }
  });

  it('clamps source requests to the first decodable timestamp', async () => {
    const { WebCodecsExporter } =
      await import('@/renderer/components/video-editor/export/webcodecs-exporter');
    const exporter =
      new WebCodecsExporter() as unknown as FrameProcessingHarness;
    const requestedTimestamps: number[] = [];
    const samples = Array.from({ length: 10 }, () => ({
      close: vi.fn(),
    }));
    const videoSink = {
      async *samplesAtTimestamps(timestamps: number[]) {
        requestedTimestamps.push(...timestamps);
        yield* samples;
      },
    };
    const videoSource = { add: vi.fn() };
    exporter.renderFrameToCanvas = vi.fn(() => ({
      videoFrame: null,
      cameraFrame: null,
    }));

    await exporter.processFrames({
      config: { segments: [{}] },
      frameRate: 1,
      videoSink,
      cameraSink: null,
      sourceFirstTimestamp: 0.25,
      cameraFirstTimestamp: 0,
      engine: {
        getFirstFrameDuration: () => 0,
        renderFrame: vi.fn(),
      },
      outputCtx: {},
      videoSource,
      onProgress: vi.fn(),
    });

    expect(requestedTimestamps[0]).toBe(0.25);
    expect(requestedTimestamps).toHaveLength(10);
    expect(videoSource.add).toHaveBeenCalledTimes(10);
  });

  it('reuses one placeholder canvas across first-frame output frames', async () => {
    const createdCanvases: object[] = [];
    const originalOffscreenCanvas = globalThis.OffscreenCanvas;
    class TestOffscreenCanvas {
      constructor(_width: number, _height: number) {
        createdCanvases.push(this);
      }
    }
    vi.stubGlobal('OffscreenCanvas', TestOffscreenCanvas);

    try {
      const { WebCodecsExporter } =
        await import('@/renderer/components/video-editor/export/webcodecs-exporter');
      const exporter =
        new WebCodecsExporter() as unknown as FrameProcessingHarness;
      const samples = Array.from({ length: 10 }, () => ({ close: vi.fn() }));
      const videoSink = {
        async *samplesAtTimestamps() {
          yield* samples;
        },
      };
      const renderFrame = vi.fn();
      exporter.renderFrameToCanvas = vi.fn(() => ({
        videoFrame: null,
        cameraFrame: null,
      }));

      await exporter.processFrames({
        config: { segments: [{}] },
        frameRate: 1,
        videoSink,
        cameraSink: null,
        sourceFirstTimestamp: 0,
        cameraFirstTimestamp: 0,
        engine: {
          getFirstFrameDuration: () => 3,
          renderFrame,
        },
        outputCtx: {},
        videoSource: { add: vi.fn() },
        onProgress: vi.fn(),
      });

      expect(createdCanvases).toHaveLength(1);
      expect(renderFrame).toHaveBeenCalledTimes(3);
      expect(renderFrame.mock.calls[0][2].video).toBe(createdCanvases[0]);
      expect(renderFrame.mock.calls[1][2].video).toBe(createdCanvases[0]);
      expect(renderFrame.mock.calls[2][2].video).toBe(createdCanvases[0]);
    } finally {
      vi.stubGlobal('OffscreenCanvas', originalOffscreenCanvas);
    }
  });

  it('closes source and camera samples when camera prefetch fails', async () => {
    const { WebCodecsExporter } =
      await import('@/renderer/components/video-editor/export/webcodecs-exporter');
    const exporter =
      new WebCodecsExporter() as unknown as FrameProcessingHarness;
    const sourceSamples = Array.from({ length: 10 }, () => ({
      close: vi.fn(),
    }));
    const cameraSample = { close: vi.fn() };
    const videoSink = {
      async *samplesAtTimestamps() {
        yield* sourceSamples;
      },
    };
    const cameraSink = {
      async *samplesAtTimestamps() {
        yield cameraSample;
        throw new Error('camera prefetch failed');
      },
    };

    await expect(
      exporter.processFrames({
        config: { segments: [{}] },
        frameRate: 1,
        videoSink,
        cameraSink,
        sourceFirstTimestamp: 0,
        cameraFirstTimestamp: 0,
        engine: {
          getFirstFrameDuration: () => 0,
          renderFrame: vi.fn(),
        },
        outputCtx: {},
        videoSource: { add: vi.fn() },
        onProgress: vi.fn(),
      })
    ).rejects.toThrow('camera prefetch failed');

    for (const sample of sourceSamples) {
      expect(sample.close).toHaveBeenCalledTimes(1);
    }
    expect(cameraSample.close).toHaveBeenCalledTimes(1);
  });

  it('fails instead of shortening the output when a source frame cannot decode', async () => {
    const { WebCodecsExporter } =
      await import('@/renderer/components/video-editor/export/webcodecs-exporter');
    const exporter =
      new WebCodecsExporter() as unknown as FrameProcessingHarness;
    const videoSink = {
      async *samplesAtTimestamps(timestamps: number[]) {
        for (let index = 0; index < timestamps.length; index++) {
          yield null;
        }
      },
    };
    const videoSource = { add: vi.fn() };

    await expect(
      exporter.processFrames({
        config: { segments: [{}] },
        frameRate: 1,
        videoSink,
        cameraSink: null,
        sourceFirstTimestamp: 0,
        cameraFirstTimestamp: 0,
        engine: {
          getFirstFrameDuration: () => 0,
          renderFrame: vi.fn(),
        },
        outputCtx: {},
        videoSource,
        onProgress: vi.fn(),
      })
    ).rejects.toThrow('Unable to decode video frame at 0.000 seconds');

    expect(videoSource.add).not.toHaveBeenCalled();
  });

  it('closes the video frame when camera frame conversion fails', async () => {
    const { WebCodecsExporter } =
      await import('@/renderer/components/video-editor/export/webcodecs-exporter');
    const exporter =
      new WebCodecsExporter() as unknown as FrameProcessingHarness;
    const closeVideoFrame = vi.fn();

    expect(() =>
      exporter.renderFrameToCanvas(
        { renderFrame: vi.fn() },
        {},
        0,
        {
          toVideoFrame: () => ({ close: closeVideoFrame }),
        },
        {
          toVideoFrame: () => {
            throw new Error('camera decode failed');
          },
        },
        30,
        0
      )
    ).toThrow('camera decode failed');
    expect(closeVideoFrame).toHaveBeenCalledTimes(1);
  });

  it('includes configured recording audio when no editor tracks exist', async () => {
    const { WebCodecsExporter } =
      await import('@/renderer/components/video-editor/export/webcodecs-exporter');
    const exporter = new WebCodecsExporter() as unknown as AudioMuxingHarness;

    await exporter.handleAudioMuxing(baseParams);

    expect(mockMuxAudioWithVideo).toHaveBeenCalledWith(
      expect.objectContaining({
        audioTracks: [
          { path: '/source/system.aac', volume: 0.5 },
          { path: '/source/mic.aac', volume: 0.75 },
        ],
      })
    );
  });

  it('uses embedded system audio when no separate system track exists', async () => {
    const { WebCodecsExporter } =
      await import('@/renderer/components/video-editor/export/webcodecs-exporter');
    const exporter = new WebCodecsExporter() as unknown as AudioMuxingHarness;

    await exporter.handleAudioMuxing({
      ...baseParams,
      systemAudioPath: null,
      micAudioPath: null,
      hasEmbeddedAudio: true,
    });

    expect(mockMuxAudioWithVideo).toHaveBeenCalledWith(
      expect.objectContaining({
        audioTracks: [],
        embeddedAudio: {
          sourcePath: '/source/video.mp4',
          volume: 0.5,
        },
      })
    );
  });

  it('keeps embedded system audio when a microphone track is also present', async () => {
    const { WebCodecsExporter } =
      await import('@/renderer/components/video-editor/export/webcodecs-exporter');
    const exporter = new WebCodecsExporter() as unknown as AudioMuxingHarness;

    await exporter.handleAudioMuxing({
      ...baseParams,
      systemAudioPath: null,
      hasEmbeddedAudio: true,
    });

    expect(mockMuxAudioWithVideo).toHaveBeenCalledWith(
      expect.objectContaining({
        audioTracks: [{ path: '/source/mic.aac', volume: 0.75 }],
        embeddedAudio: {
          sourcePath: '/source/video.mp4',
          volume: 0.5,
        },
      })
    );
  });

  it('does not restore disabled editor tracks from legacy settings', async () => {
    const { WebCodecsExporter } =
      await import('@/renderer/components/video-editor/export/webcodecs-exporter');
    const exporter = new WebCodecsExporter() as unknown as AudioMuxingHarness;

    await exporter.handleAudioMuxing({ ...baseParams, musicTracks: [] });

    expect(mockMuxAudioWithVideo).toHaveBeenCalledWith(
      expect.objectContaining({ audioTracks: [], embeddedAudio: undefined })
    );
  });

  it('applies first-frame audio delay only during final muxing', async () => {
    mockInvoke.mockImplementation((channel: string) => {
      if (channel === 'video-editor:music:get-path') {
        return Promise.resolve('/source/music.mp3');
      }
      if (channel === 'video-editor:music:prepare-for-export') {
        return Promise.resolve({ success: true });
      }
      return Promise.resolve(undefined);
    });
    const { WebCodecsExporter } =
      await import('@/renderer/components/video-editor/export/webcodecs-exporter');
    const exporter = new WebCodecsExporter() as unknown as AudioMuxingHarness;

    await exporter.handleAudioMuxing({
      ...baseParams,
      firstFrameDuration: 2,
      musicTracks: [
        {
          id: 'music',
          name: 'Music',
          source: 'music',
          fileName: 'music.mp3',
          volume: 0.8,
          enabled: true,
          startTime: 3,
          endTime: 8,
          originalDuration: 10,
          trimStart: 0,
          trimEnd: 0,
          speed: 1,
        },
      ],
    });

    expect(mockInvoke).toHaveBeenCalledWith(
      'video-editor:music:prepare-for-export',
      expect.objectContaining({ startTime: 3, totalDuration: 10 })
    );
    expect(mockMuxAudioWithVideo).toHaveBeenCalledWith(
      expect.objectContaining({ audioDelaySeconds: 2 })
    );
  });
});
