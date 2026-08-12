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
  | 'toggle-mic-mute'
  | 'select-mic'
  | 'select-camera';

export type RecordingControlDeviceKind = 'microphone' | 'camera';

export interface RecordingControlActionData {
  deviceId: string | null;
  deviceName: string | null;
}

export interface RecordingControlState {
  mode: RecordingControlMode;
  targetName: string | null;
  systemAudio: boolean;
  micEnabled: boolean;
  micMuted: boolean;
  selectedMicId: string | null;
  cameraEnabled: boolean;
  selectedCameraId: string | null;
  cameraLocked: boolean;
  isPaused: boolean;
  isStarting: boolean;
  elapsedSeconds: number;
}
