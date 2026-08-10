import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockInvoke = vi.fn();

vi.stubGlobal('window', {
  ipcRenderer: {
    invoke: mockInvoke,
  },
});

describe('audio muxer', () => {
  beforeEach(() => {
    vi.resetModules();
    mockInvoke.mockReset();
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
    });

    expect(result).toEqual({ success: false, error: 'rename failed' });
  });

  it('completes a video-only export after a successful rename', async () => {
    mockInvoke.mockResolvedValue({ success: true });
    const { muxAudioWithVideo } =
      await import('@/renderer/components/video-editor/export/audio-muxer');

    const result = await muxAudioWithVideo({
      videoPath: '/tmp/video.mp4',
      audioTracks: [],
      outputPath: '/output/video.mp4',
      segments: [],
    });

    expect(result).toEqual({ success: true });
    expect(mockInvoke).toHaveBeenCalledWith('file:rename', {
      oldPath: '/tmp/video.mp4',
      newPath: '/output/video.mp4',
    });
  });

  it('does not export unadjusted audio when volume processing fails', async () => {
    mockInvoke.mockImplementation((channel: string) => {
      if (channel === 'video-editor:extract-audio-segments') {
        return Promise.resolve({ success: true });
      }
      if (channel === 'video-editor:adjust-audio-volume') {
        return Promise.resolve({ success: false, error: 'volume failed' });
      }
      return Promise.resolve({ success: true });
    });
    const { muxAudioWithVideo } =
      await import('@/renderer/components/video-editor/export/audio-muxer');

    const result = await muxAudioWithVideo({
      videoPath: '/tmp/video.mp4',
      audioTracks: [{ path: '/audio.m4a', volume: 0.5 }],
      outputPath: '/output/video.mp4',
      segments: [{ originalStart: 0, originalEnd: 10 }],
    });

    expect(result).toEqual({ success: false, error: 'volume failed' });
    expect(mockInvoke).not.toHaveBeenCalledWith(
      'video-editor:mux-audio',
      expect.anything()
    );
  });

  it('mixes embedded system audio with a separate microphone track', async () => {
    mockInvoke.mockResolvedValue({ success: true });
    const { muxAudioWithVideo } =
      await import('@/renderer/components/video-editor/export/audio-muxer');

    const result = await muxAudioWithVideo({
      videoPath: '/tmp/video.mp4',
      audioTracks: [{ path: '/audio/mic.aac', volume: 0.75 }],
      embeddedAudio: { sourcePath: '/source/video.mp4', volume: 0.5 },
      outputPath: '/output/video.mp4',
      segments: [{ originalStart: 0, originalEnd: 10 }],
    });

    expect(result).toEqual({ success: true });
    expect(mockInvoke).toHaveBeenCalledWith('video-editor:mix-audio-tracks', {
      inputPaths: [
        expect.stringMatching(
          /^\/output\/video\.mp4\.temp-.+\.temp_adjusted\.aac$/
        ),
        expect.stringMatching(
          /^\/output\/video\.mp4\.temp-.+\.temp_audio_1\.aac$/
        ),
      ],
      outputPath: expect.stringMatching(
        /^\/output\/video\.mp4\.temp-.+\.temp_mixed\.aac$/
      ),
      volumes: [1, 0.75],
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
      segments: [{ originalStart: 0, originalEnd: 10 }],
    });

    expect(result).toEqual({ success: false, error: 'audio corrupt' });
    expect(mockInvoke).not.toHaveBeenCalledWith(
      'file:rename',
      expect.anything()
    );
  });
});
