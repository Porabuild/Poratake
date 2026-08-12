import { useState, useCallback, useEffect, useRef } from 'react';
import type { MusicTrack } from '@/types/music';
import { DEFAULT_MUSIC_TRACK_VOLUME } from '@/types/music';
import { splitMusicTrack } from '../timeline-split';
import type { SliceController } from './use-editor-history';

interface UseMusicTracksProps {
  totalTimelineDuration: number;
  slice: SliceController<MusicTrack[]>;
}

interface UseMusicTracksReturn {
  musicTracks: MusicTrack[];
  setMusicTracks: (
    updater: MusicTrack[] | ((prev: MusicTrack[]) => MusicTrack[])
  ) => void;
  selectedMusicTrackId: string | null;
  handleAddMusicTrack: () => Promise<void>;
  handleRemoveMusicTrackGroup: (groupId: string) => void;
  handleUpdateMusicTrackGroup: (
    groupId: string,
    updates: Partial<MusicTrack>
  ) => void;
  handleResizeMusicTrack: (
    id: string,
    startTime: number,
    endTime: number
  ) => void;
  handleMoveMusicTrack: (
    id: string,
    startTime: number,
    endTime: number
  ) => void;
  handleSplitMusicTrack: (id: string, cutTime: number) => void;
  handleCommitMusicGesture: () => void;
  handleSelectMusicTrack: (id: string | null) => void;
  clearMusicSelection: () => void;
  getMusicTrackPath: (fileName: string) => Promise<string | null>;
}

export const SYSTEM_TRACK_ID = 'system-audio';
export const MIC_TRACK_ID = 'mic-audio';

export function withDefaultGroupIds(
  tracks: (MusicTrack & { groupId?: string })[]
): MusicTrack[] {
  return tracks.map(track => {
    if (track.groupId) return track;

    switch (track.source) {
      case 'system':
        return { ...track, groupId: SYSTEM_TRACK_ID };
      case 'mic':
        return { ...track, groupId: MIC_TRACK_ID };
      default:
        return { ...track, groupId: track.id };
    }
  });
}

interface BuildBuiltInTracksParams {
  systemAudioPath: string | null;
  micAudioPath: string | null;
  hasEmbeddedAudio: boolean;
  originalDuration: number;
}

interface BuildImportedMusicTrackParams {
  fileName: string;
  name: string;
  originalDuration: number;
}

export function buildImportedMusicTrack({
  fileName,
  name,
  originalDuration,
}: BuildImportedMusicTrackParams): MusicTrack {
  const id = crypto.randomUUID();

  return {
    id,
    groupId: id,
    name,
    source: 'music',
    fileName,
    volume: DEFAULT_MUSIC_TRACK_VOLUME,
    enabled: true,
    startTime: 0,
    endTime: originalDuration,
    originalDuration,
    trimStart: 0,
    trimEnd: 0,
    speed: 1,
  };
}

export function buildBuiltInMusicTracks({
  systemAudioPath,
  micAudioPath,
  hasEmbeddedAudio,
  originalDuration,
}: BuildBuiltInTracksParams): MusicTrack[] {
  if (originalDuration <= 0) return [];

  const builtIn: MusicTrack[] = [];

  if (systemAudioPath || hasEmbeddedAudio) {
    builtIn.push({
      id: SYSTEM_TRACK_ID,
      groupId: SYSTEM_TRACK_ID,
      name: hasEmbeddedAudio && !systemAudioPath ? 'Audio' : 'System Audio',
      source: 'system',
      fileName: '',
      volume: 1,
      enabled: true,
      startTime: 0,
      endTime: originalDuration,
      originalDuration,
      trimStart: 0,
      trimEnd: 0,
      speed: 1,
    });
  }

  if (micAudioPath) {
    builtIn.push({
      id: MIC_TRACK_ID,
      groupId: MIC_TRACK_ID,
      name: 'Microphone',
      source: 'mic',
      fileName: '',
      volume: 1,
      enabled: true,
      startTime: 0,
      endTime: originalDuration,
      originalDuration,
      trimStart: 0,
      trimEnd: 0,
      speed: 1,
    });
  }

  return builtIn;
}

export function mergeBuiltInMusicTracks(
  existing: MusicTrack[],
  builtIn: MusicTrack[]
): MusicTrack[] {
  const missing = builtIn.filter(
    builtInTrack =>
      !existing.some(track => track.source === builtInTrack.source)
  );
  return missing.length === 0 ? existing : [...missing, ...existing];
}

export function useMusicTracks({
  totalTimelineDuration,
  slice,
}: UseMusicTracksProps): UseMusicTracksReturn {
  const {
    value: musicTracks,
    set: setMusicTracks,
    setWithoutHistory,
    commit,
  } = slice;

  const [selectedMusicTrackId, setSelectedMusicTrackId] = useState<
    string | null
  >(null);
  const gestureActiveRef = useRef(false);

  const musicTracksRef = useRef(musicTracks);
  useEffect(() => {
    musicTracksRef.current = musicTracks;
  }, [musicTracks]);

  useEffect(() => {
    if (totalTimelineDuration === 0) return;

    const needsUpdate = musicTracksRef.current.some(
      track =>
        track.source !== 'music' &&
        (track.startTime >= totalTimelineDuration ||
          track.endTime > totalTimelineDuration)
    );
    if (!needsUpdate) return;

    setWithoutHistory(prev =>
      prev.map(track =>
        track.source === 'music'
          ? track
          : {
              ...track,
              endTime: Math.min(track.endTime, totalTimelineDuration),
            }
      )
    );
  }, [totalTimelineDuration, setWithoutHistory]);

  const handleAddMusicTrack = useCallback(async () => {
    const result = (await window.ipcRenderer.invoke(
      'video-editor:music:add'
    )) as {
      success: boolean;
      fileName?: string;
      name?: string;
      originalDuration?: number;
      error?: string;
    };

    if (!result.success || !result.fileName || !result.originalDuration) {
      return;
    }

    const newTrack = buildImportedMusicTrack({
      fileName: result.fileName,
      name: result.name ?? result.fileName,
      originalDuration: result.originalDuration,
    });

    setMusicTracks(prev => [...prev, newTrack]);
  }, [setMusicTracks]);

  const handleUpdateMusicTrackGroup = useCallback(
    (groupId: string, updates: Partial<MusicTrack>) => {
      setMusicTracks(prev =>
        prev.map(track =>
          track.groupId === groupId ? { ...track, ...updates } : track
        )
      );
    },
    [setMusicTracks]
  );

  const handleRemoveMusicTrackGroup = useCallback(
    (groupId: string) => {
      const group = musicTracksRef.current.filter(
        track => track.groupId === groupId
      );
      if (group.length === 0 || group[0].source !== 'music') return;

      for (const track of group) {
        window.ipcRenderer
          .invoke('video-editor:music:remove', {
            fileName: track.fileName,
          })
          .catch(() => {});
      }
      setMusicTracks(prev => prev.filter(track => track.groupId !== groupId));
      setSelectedMusicTrackId(prev =>
        group.some(track => track.id === prev) ? null : prev
      );
    },
    [setMusicTracks]
  );

  const handleResizeMusicTrack = useCallback(
    (id: string, startTime: number, endTime: number) => {
      gestureActiveRef.current = true;
      setWithoutHistory(prev =>
        prev.map(track => {
          if (track.id !== id) return track;

          const effectiveDuration =
            (track.originalDuration - track.trimStart - track.trimEnd) /
            track.speed;
          const maxEndTime = track.startTime + effectiveDuration;
          const clampedStart = Math.max(0, startTime);
          const clampedEnd = Math.min(
            endTime,
            maxEndTime,
            track.source === 'music' ? maxEndTime : totalTimelineDuration
          );

          const startDelta = clampedStart - track.startTime;
          const endDelta = track.endTime - clampedEnd;

          const newTrimStart = Math.max(
            0,
            track.trimStart + startDelta * track.speed
          );
          const newTrimEnd = Math.max(
            0,
            track.trimEnd + endDelta * track.speed
          );

          return {
            ...track,
            startTime: clampedStart,
            endTime: clampedEnd,
            trimStart: newTrimStart,
            trimEnd: newTrimEnd,
          };
        })
      );
    },
    [totalTimelineDuration, setWithoutHistory]
  );

  const handleMoveMusicTrack = useCallback(
    (id: string, startTime: number, endTime: number) => {
      gestureActiveRef.current = true;
      setWithoutHistory(prev =>
        prev.map(track => {
          if (track.id !== id) return track;
          const duration = endTime - startTime;
          if (duration > totalTimelineDuration) return track;

          const maxStart = Math.max(0, totalTimelineDuration - duration);
          const clampedStart = Math.max(0, Math.min(startTime, maxStart));
          return {
            ...track,
            startTime: clampedStart,
            endTime: Math.min(clampedStart + duration, totalTimelineDuration),
          };
        })
      );
    },
    [totalTimelineDuration, setWithoutHistory]
  );

  const handleSplitMusicTrack = useCallback(
    (id: string, cutTime: number) => {
      const track = musicTracksRef.current.find(t => t.id === id);
      if (!track) return;

      const split = splitMusicTrack(track, cutTime);
      if (!split) return;

      const [left, right] = split;
      setMusicTracks(prev =>
        prev.flatMap(t => (t.id === id ? [left, right] : [t]))
      );
    },
    [setMusicTracks]
  );

  const handleCommitMusicGesture = useCallback(() => {
    if (!gestureActiveRef.current) return;
    gestureActiveRef.current = false;
    commit();
  }, [commit]);

  const handleSelectMusicTrack = useCallback((id: string | null) => {
    setSelectedMusicTrackId(id);
  }, []);

  const clearMusicSelection = useCallback(() => {
    setSelectedMusicTrackId(null);
  }, []);

  const getMusicTrackPath = useCallback(
    async (fileName: string): Promise<string | null> => {
      return (await window.ipcRenderer.invoke('video-editor:music:get-path', {
        fileName,
      })) as string | null;
    },
    []
  );

  return {
    musicTracks,
    setMusicTracks,
    selectedMusicTrackId,
    handleAddMusicTrack,
    handleRemoveMusicTrackGroup,
    handleUpdateMusicTrackGroup,
    handleResizeMusicTrack,
    handleMoveMusicTrack,
    handleSplitMusicTrack,
    handleCommitMusicGesture,
    handleSelectMusicTrack,
    clearMusicSelection,
    getMusicTrackPath,
  };
}
