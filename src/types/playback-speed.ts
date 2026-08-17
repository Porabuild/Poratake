export type PlaybackSpeed = 0.5 | 0.75 | 1 | 1.25 | 1.5 | 2 | 3 | 4;

export const PLAYBACK_SPEED_PRESETS: PlaybackSpeed[] = [
  0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4,
];

export function formatPlaybackSpeed(speed: number): string {
  return `${speed}x`;
}
