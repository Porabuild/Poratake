export type PlaybackSpeed = 0.5 | 0.75 | 1 | 1.25 | 1.5 | 2 | 3 | 4;

export const PLAYBACK_SPEED_PRESETS: PlaybackSpeed[] = [
  0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4,
];

export const DEFAULT_PLAYBACK_SPEED: PlaybackSpeed = 1;

export function formatPlaybackSpeed(speed: number): string {
  return `${speed}x`;
}

export function isValidPlaybackSpeed(speed: number): speed is PlaybackSpeed {
  return PLAYBACK_SPEED_PRESETS.includes(speed as PlaybackSpeed);
}
