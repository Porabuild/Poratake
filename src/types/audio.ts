export type KeyboardSoundType = 'cherry-blue' | 'cherry-brown' | 'cherry-red';

export const KEYBOARD_SOUND_OPTIONS: {
  value: KeyboardSoundType;
  label: string;
}[] = [
  { value: 'cherry-blue', label: 'Cherry MX Blue' },
  { value: 'cherry-brown', label: 'Cherry MX Brown' },
  { value: 'cherry-red', label: 'Cherry MX Red' },
];

export const KEYBOARD_SOUND_SAMPLES_PER_TYPE = 4;

export interface AudioStyle {
  systemAudioEnabled: boolean;
  micAudioEnabled: boolean;
  systemAudioVolume: number;
  micAudioVolume: number;
  keyboardSoundEnabled: boolean;
  keyboardSoundVolume: number;
  keyboardSoundType: KeyboardSoundType;
}

export const DEFAULT_AUDIO_STYLE: AudioStyle = {
  systemAudioEnabled: true,
  micAudioEnabled: true,
  systemAudioVolume: 1,
  micAudioVolume: 1,
  keyboardSoundEnabled: false,
  keyboardSoundVolume: 0.7,
  keyboardSoundType: 'cherry-blue',
};

export interface AudioTrack {
  path: string;
  volume: number;
  skipSegmentExtraction?: boolean;
}

export interface AudioSegment {
  start: number;
  end: number;
}

export interface AudioSegmentWithSpeed extends AudioSegment {
  speed: number;
}
