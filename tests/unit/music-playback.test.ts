import { describe, expect, it } from 'vitest';
import { resolveBuiltInMusicTrackPath } from '@/renderer/components/video-editor/hooks/use-music-playback';
import type { MusicTrack } from '@/types/music';

function createTrack(source: MusicTrack['source']): MusicTrack {
  return {
    id: source,
    name: source,
    source,
    fileName: '',
    volume: 1,
    enabled: true,
    startTime: 0,
    endTime: 10,
    originalDuration: 10,
    trimStart: 0,
    trimEnd: 0,
    speed: 1,
  };
}

describe('music playback paths', () => {
  it('uses the source video for an embedded system-audio track', () => {
    expect(
      resolveBuiltInMusicTrackPath(
        createTrack('system'),
        null,
        null,
        '/recording.mp4'
      )
    ).toBe('/recording.mp4');
  });

  it('prefers separate system audio and resolves microphone audio', () => {
    expect(
      resolveBuiltInMusicTrackPath(
        createTrack('system'),
        '/system.aac',
        '/mic.aac',
        '/recording.mp4'
      )
    ).toBe('/system.aac');
    expect(
      resolveBuiltInMusicTrackPath(
        createTrack('mic'),
        '/system.aac',
        '/mic.aac',
        '/recording.mp4'
      )
    ).toBe('/mic.aac');
  });
});
