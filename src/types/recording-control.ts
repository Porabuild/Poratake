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
  | 'select-mic'
  | 'select-camera'
  | 'select-ios-device';

export type RecordingControlDeviceKind = 'microphone' | 'camera' | 'ios-device';

export interface RecordingControlActionData {
  deviceId: string | null;
  deviceName: string | null;
}

export interface RecordingControlState {
  mode: RecordingControlMode;
  targetName: string | null;
  systemAudio: boolean;
  micEnabled: boolean;
  selectedMicId: string | null;
  cameraEnabled: boolean;
  selectedCameraId: string | null;
  cameraLocked: boolean;
  selectedIOSDeviceId: string | null;
  selectedIOSDeviceName: string | null;
  isPaused: boolean;
  isStarting: boolean;
  elapsedSeconds: number;
}
