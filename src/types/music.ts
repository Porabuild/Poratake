import { Mic, Music, Volume2 } from 'lucide-react';

export type AudioTrackSource = 'system' | 'mic' | 'music';

export interface MusicTrack {
  id: string;
  groupId: string;
  name: string;
  source: AudioTrackSource;
  fileName: string;
  volume: number;
  enabled: boolean;
  startTime: number;
  endTime: number;
  originalDuration: number;
  trimStart: number;
  trimEnd: number;
  speed: number;
}

export function groupMusicTracks(tracks: MusicTrack[]): MusicTrack[][] {
  const groups: MusicTrack[][] = [];
  const byGroupId = new Map<string, MusicTrack[]>();

  for (const track of tracks) {
    const group = byGroupId.get(track.groupId);
    if (group) {
      group.push(track);
      continue;
    }

    const newGroup = [track];
    byGroupId.set(track.groupId, newGroup);
    groups.push(newGroup);
  }

  return groups;
}

export const SOURCE_ICONS: Record<AudioTrackSource, typeof Volume2> = {
  system: Volume2,
  mic: Mic,
  music: Music,
};

export const DEFAULT_MUSIC_TRACK_VOLUME = 0.8;

export const SUPPORTED_MUSIC_EXTENSIONS = ['mp3', 'm4a', 'wav', 'aac', 'ogg'];
