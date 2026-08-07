export type ModifierKey =
  'command' | 'control' | 'option' | 'shift' | 'fn' | 'meta' | 'alt';

export type KeyboardPlatform = 'macos' | 'windows';

export interface KeyboardKeyEvent {
  timestamp: number;
  key: string;
  keyCode: number;
  modifiers: ModifierKey[];
  type: 'down' | 'up';
}

export interface KeyboardData {
  events: KeyboardKeyEvent[];
  meta: {
    startTime: string;
    duration: number;
    sampleRate: number;
    platform?: KeyboardPlatform;
  };
}

export interface KeyboardStyle {
  visible: boolean;
  displayDuration: number;
  position: 'bottom-center';
  fontSize: 'small' | 'medium' | 'large';
  opacity: number;
}

export const DEFAULT_KEYBOARD_STYLE: KeyboardStyle = {
  visible: false,
  displayDuration: 1,
  position: 'bottom-center',
  fontSize: 'medium',
  opacity: 0.75,
};
