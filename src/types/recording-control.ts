export type RecordingControlMode = 'pre-recording' | 'recording';

export type RecordingControlAction =
  | 'start'
  | 'cancel'
  | 'pause'
  | 'resume'
  | 'stop'
  | 'delete'
  | 'toggle-system-audio'
  | 'toggle-mic'
  | 'toggle-camera'
  | 'toggle-mic-mute';

export interface RecordingControlState {
  mode: RecordingControlMode;
  systemAudio: boolean;
  micEnabled: boolean;
  micMuted: boolean;
  cameraEnabled: boolean;
  isPaused: boolean;
  isStarting: boolean;
  elapsedSeconds: number;
}
