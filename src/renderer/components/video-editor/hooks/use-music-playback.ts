import { useEffect, useRef, useCallback } from 'react';
import type { MusicTrack } from '@/types/music';

interface AudioRef {
  trackId: string;
  audio: HTMLAudioElement;
  path: string | null;
}

interface UseMusicPlaybackProps {
  musicTracks: MusicTrack[];
  timelinePosition: number;
  isPlaying: boolean;
  systemAudioPath: string | null;
  micAudioPath: string | null;
}

export function useMusicPlayback({
  musicTracks,
  timelinePosition,
  isPlaying,
  systemAudioPath,
  micAudioPath,
}: UseMusicPlaybackProps) {
  const audioRefsMap = useRef<Map<string, AudioRef>>(new Map());
  const isPlayingRef = useRef(isPlaying);
  isPlayingRef.current = isPlaying;

  const resolvePath = useCallback(
    (track: MusicTrack): string | null => {
      if (track.source === 'system') return systemAudioPath;
      if (track.source === 'mic') return micAudioPath;
      return null;
    },
    [systemAudioPath, micAudioPath]
  );

  useEffect(() => {
    const currentIds = new Set(musicTracks.map(t => t.id));
    const map = audioRefsMap.current;

    for (const [id, ref] of map) {
      if (!currentIds.has(id)) {
        ref.audio.pause();
        ref.audio.src = '';
        map.delete(id);
      }
    }

    for (const track of musicTracks) {
      if (!map.has(track.id)) {
        const audio = new Audio();
        audio.preload = 'auto';
        const entry: AudioRef = {
          trackId: track.id,
          audio,
          path: null,
        };
        map.set(track.id, entry);

        const knownPath = resolvePath(track);
        if (knownPath) {
          entry.path = knownPath;
          entry.audio.src = `file://${knownPath}`;
        } else if (track.source === 'music' && track.fileName) {
          window.ipcRenderer
            .invoke('video-editor:music:get-path', {
              fileName: track.fileName,
            })
            .then((resolved: string | null) => {
              const ref = map.get(track.id);
              if (ref && resolved) {
                ref.path = resolved;
                ref.audio.src = `file://${resolved}`;
              }
            })
            .catch(() => {});
        }
      }
    }
  }, [musicTracks, resolvePath]);

  useEffect(() => {
    for (const track of musicTracks) {
      const ref = audioRefsMap.current.get(track.id);
      if (!ref) continue;

      ref.audio.volume = track.enabled ? track.volume : 0;
      ref.audio.playbackRate = track.speed;
    }
  }, [musicTracks]);

  const syncAudio = useCallback(
    (tlPos: number, playing: boolean) => {
      for (const track of musicTracks) {
        const ref = audioRefsMap.current.get(track.id);
        if (!ref?.path) continue;

        const isInRange =
          track.enabled && tlPos >= track.startTime && tlPos < track.endTime;

        if (!isInRange || !playing) {
          if (!ref.audio.paused) {
            ref.audio.pause();
          }
          continue;
        }

        const offsetInTrack = tlPos - track.startTime;
        const sourceTime = track.trimStart + offsetInTrack * track.speed;

        ref.audio.volume = track.volume;
        ref.audio.playbackRate = track.speed;

        if (Math.abs(ref.audio.currentTime - sourceTime) > 0.3) {
          ref.audio.currentTime = sourceTime;
        }

        if (ref.audio.paused) {
          ref.audio.play().catch(() => {});
        }
      }
    },
    [musicTracks]
  );

  useEffect(() => {
    syncAudio(timelinePosition, isPlaying);
  }, [timelinePosition, isPlaying, syncAudio]);

  useEffect(() => {
    const map = audioRefsMap.current;
    return () => {
      for (const ref of map.values()) {
        ref.audio.pause();
        ref.audio.src = '';
      }
      map.clear();
    };
  }, []);
}
