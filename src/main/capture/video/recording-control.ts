import { globalShortcut, screen } from 'electron';
import { daemon } from '@/main/daemon';
import { getConfig, updateConfig } from '@/main/settings';
import {
  showCameraPreview,
  hideCameraPreview,
  getCameraPreviewSettings,
  updateCameraPreviewPosition,
  enableCameraContentProtection,
} from './camera-preview';
import { pauseRecording, resumeRecording } from './recorder';
import {
  checkAndRequestCameraPermission,
  checkAndRequestMicrophonePermission,
  showRecordingError,
} from './permissions';
import {
  hideAreaSelector,
  showAreaSelector,
} from '@/main/capture/area-selector';
import {
  clearRecordingControlBrowserWindowParent,
  getRecordingControlWindowWidth,
  hideRecordingControlBrowserWindow,
  prewarmRecordingControlBrowserWindow,
  showRecordingControlBrowserWindow,
  updateRecordingControlBrowserWindow,
  updateRecordingControlBrowserWindowPosition,
} from './recording-control-window';
import type {
  RecordingControlAction,
  RecordingControlActionData,
  RecordingControlMode,
  RecordingControlState,
} from '@/types/recording-control';
import {
  DEFAULT_ALL_IN_ONE_CONFIG,
  type CameraSettings,
} from '@/types/settings';
import type { RecordingConfig, RecordingOptions } from '@/types/video';

let timerInterval: NodeJS.Timeout | null = null;
let recordingStartTime: number | null = null;
let pausedElapsedTime: number = 0;
let isPaused: boolean = false;
let currentMode: RecordingControlMode = 'pre-recording';
let recordingTarget: string | null = null;

interface RecordingActions {
  startPendingRecording: (options: RecordingOptions) => Promise<void>;
  cancelPendingRecording: () => Promise<void>;
  stopRecordingAction: () => Promise<string | null>;
  deleteRecordingAction: () => Promise<void>;
}

let recordingActions: RecordingActions | null = null;

export function setRecordingControlActions(actions: RecordingActions): void {
  recordingActions = actions;
}

function getRecordingActions(): RecordingActions {
  if (!recordingActions) {
    throw new Error('Recording control actions are not initialized');
  }
  return recordingActions;
}

const CONTROL_TOP_MARGIN = 24;
const CAMERA_PREVIEW_SIZE = 270;
const CAMERA_PREVIEW_MARGIN = 32;

export const MIN_RECORDING_START_DELAY = 0;
export const MAX_RECORDING_START_DELAY = 10;

function getWindowWidth(): number {
  return getRecordingControlWindowWidth(currentMode, recordingTarget !== null);
}

function calculateControlPosition(area?: {
  x: number;
  y: number;
  width: number;
  height: number;
}): { x: number; y: number } {
  const { workArea } = area
    ? screen.getDisplayMatching(area)
    : screen.getDisplayNearestPoint(screen.getCursorScreenPoint());
  const x = Math.round(workArea.x + (workArea.width - getWindowWidth()) / 2);
  const y = workArea.y + CONTROL_TOP_MARGIN;
  return { x, y };
}

function calculateCameraPreviewPosition(area: {
  x: number;
  y: number;
  width: number;
  height: number;
}): { x: number; y: number } {
  return {
    x:
      area.x +
      Math.max(0, area.width - CAMERA_PREVIEW_SIZE - CAMERA_PREVIEW_MARGIN),
    y:
      area.y +
      Math.max(0, area.height - CAMERA_PREVIEW_SIZE - CAMERA_PREVIEW_MARGIN),
  };
}

let currentControlCenter = { x: 100, y: 100 };

interface RecordingTogglePreferences {
  systemAudio: boolean;
  micEnabled: boolean;
  cameraEnabled: boolean;
}

let allInOneRecordingPreferences: RecordingTogglePreferences | null = null;

function updateAllInOneRecordingPreferences(
  updates: Partial<RecordingTogglePreferences>
): void {
  if (!allInOneRecordingPreferences) return;

  allInOneRecordingPreferences = {
    ...allInOneRecordingPreferences,
    ...updates,
  };

  const config = getConfig();
  if (!config.allInOne.rememberChoices) return;

  updateConfig({
    allInOne: {
      ...config.allInOne,
      recording: allInOneRecordingPreferences,
    },
  });
}

function getRecordingSettings() {
  const config = getConfig();
  const live = recordingSession;
  const toggles = live ?? allInOneRecordingPreferences;
  return {
    systemAudio: toggles?.systemAudio ?? config.recording.systemAudio,
    micEnabled: toggles?.micEnabled ?? config.recording.micEnabled,
    selectedMicId: live ? live.selectedMicId : config.recording.selectedMicId,
    selectedMicName: live
      ? live.selectedMicName
      : config.recording.selectedMicName,
    cameraEnabled:
      toggles?.cameraEnabled ?? config.recording.camera?.enabled ?? false,
    selectedCameraId: live
      ? live.camera?.selectedDeviceId
      : config.recording.camera?.selectedDeviceId,
    selectedCameraName: live
      ? live.camera?.selectedDeviceName
      : config.recording.camera?.selectedDeviceName,
    cameraSize: config.recording.camera?.size ?? 'medium',
    cameraShape: config.recording.camera?.shape ?? 'circle',
    cameraFlipped: config.recording.camera?.flipped ?? false,
    selectedIOSDeviceId: config.recording.iosDevice?.id ?? null,
    selectedIOSDeviceName: config.recording.iosDevice?.name ?? null,
  };
}

function getRecordingControlState(): RecordingControlState {
  const settings = getRecordingSettings();
  return {
    mode: currentMode,
    targetName: recordingTarget,
    systemAudio: settings.systemAudio,
    micEnabled: settings.micEnabled,
    selectedMicId: settings.selectedMicId ?? null,
    cameraEnabled: settings.cameraEnabled,
    selectedCameraId: settings.selectedCameraId ?? null,
    selectedIOSDeviceId: settings.selectedIOSDeviceId,
    selectedIOSDeviceName: settings.selectedIOSDeviceName,
    cameraLocked: recordingSession?.camera !== undefined,
    isPaused,
    isStarting: isHandlingStart,
    elapsedSeconds: pausedElapsedTime,
    countdownSeconds: null,
  };
}

interface EventData {
  deviceId?: string | null;
  deviceName?: string | null;
}

let isHandlingStart: boolean = false;

interface RecordingSession {
  systemAudio: boolean;
  micEnabled: boolean;
  selectedMicId: string | null;
  selectedMicName: string | null;
  cameraEnabled: boolean;
  camera?: CameraSettings;
}

let recordingSession: RecordingSession | null = null;

async function reportControlError(error: unknown): Promise<void> {
  const normalized = error instanceof Error ? error : new Error(String(error));
  console.error('Failed to handle recording control event:', normalized);
  try {
    await showRecordingError(normalized);
  } catch (dialogError) {
    console.error('Failed to show recording control error:', dialogError);
  }
}

async function handleEvent(event: string, data?: EventData): Promise<void> {
  if (isHandlingStart && currentMode === 'pre-recording') return;

  switch (event) {
    case 'recording-control:toggle-system-audio':
      await handleToggleSystemAudio();
      break;
    case 'recording-control:toggle-mic':
      await handleToggleMic();
      break;
    case 'recording-control:toggle-camera':
      await handleToggleCamera();
      break;
    case 'recording-control:select-mic':
      await handleSelectMic(data?.deviceId ?? null, data?.deviceName ?? null);
      break;
    case 'recording-control:select-camera':
      await handleSelectCamera(
        data?.deviceId ?? null,
        data?.deviceName ?? null
      );
      break;
    case 'recording-control:start':
      await handleStart();
      break;
    case 'recording-control:cancel':
      await handleCancel();
      break;
    case 'recording-control:pause':
      await handlePause();
      break;
    case 'recording-control:resume':
      await handleResume();
      break;
    case 'recording-control:stop':
      await handleStop();
      break;
    case 'recording-control:delete':
      await handleDelete();
      break;
    case 'recording-control:select-ios-device':
      await handleSelectIOSDevice(
        data?.deviceId ?? null,
        data?.deviceName ?? null
      );
      break;
  }
}

async function applySessionSystemAudio(
  session: RecordingSession,
  enabled: boolean
): Promise<void> {
  await daemon.call('screen-recorder', 'setSystemAudio', { enabled });
  session.systemAudio = enabled;
  await syncControlSettings();
}

async function applySessionMicrophone(
  session: RecordingSession,
  device: RecordingControlActionData | null
): Promise<void> {
  if (device && !(await checkAndRequestMicrophonePermission())) return;

  await daemon.call('screen-recorder', 'setMicrophone', {
    enabled: device !== null,
    deviceId: device?.deviceId ?? null,
    deviceName: device?.deviceName ?? null,
  });

  session.micEnabled = device !== null;
  if (device) {
    session.selectedMicId = device.deviceId;
    session.selectedMicName = device.deviceName;
  }
  await syncControlSettings();
}

async function applySessionCamera(
  session: RecordingSession,
  enabled: boolean
): Promise<void> {
  const camera = session.camera;
  if (!camera) return;
  if (enabled && !(await checkAndRequestCameraPermission())) return;

  if (enabled) {
    await enableCameraContentProtection();
  }

  await daemon.call('screen-recorder', 'setCamera', { enabled });

  if (enabled) {
    await showCameraPreview({ ...camera, enabled: true });
  } else {
    session.camera = {
      ...camera,
      position: getCameraPreviewSettings()?.position ?? camera.position,
    };
    hideCameraPreview();
  }

  session.cameraEnabled = enabled;
  await syncControlSettings();
}

async function handleToggleSystemAudio(): Promise<void> {
  if (recordingSession) {
    await applySessionSystemAudio(
      recordingSession,
      !recordingSession.systemAudio
    );
    return;
  }

  if (allInOneRecordingPreferences) {
    updateAllInOneRecordingPreferences({
      systemAudio: !allInOneRecordingPreferences.systemAudio,
    });
    await syncControlSettings();
    return;
  }

  const config = getConfig();
  const newSystemAudio = !config.recording.systemAudio;

  updateConfig({
    recording: {
      ...config.recording,
      systemAudio: newSystemAudio,
    },
  });

  await syncControlSettings();
}

async function handleToggleMic(): Promise<void> {
  if (recordingSession) {
    await applySessionMicrophone(
      recordingSession,
      recordingSession.micEnabled
        ? null
        : {
            deviceId: recordingSession.selectedMicId,
            deviceName: recordingSession.selectedMicName,
          }
    );
    return;
  }

  const newMicEnabled = allInOneRecordingPreferences
    ? !allInOneRecordingPreferences.micEnabled
    : !getConfig().recording.micEnabled;

  if (newMicEnabled) {
    const hasPermission = await checkAndRequestMicrophonePermission();
    if (!hasPermission) return;
  }

  if (allInOneRecordingPreferences) {
    updateAllInOneRecordingPreferences({ micEnabled: newMicEnabled });
  } else {
    const config = getConfig();
    updateConfig({
      recording: {
        ...config.recording,
        micEnabled: newMicEnabled,
      },
    });
  }

  await syncControlSettings();
}

async function handleSelectMic(
  deviceId: string | null,
  deviceName: string | null
): Promise<void> {
  if (recordingSession) {
    await applySessionMicrophone(recordingSession, { deviceId, deviceName });
    return;
  }

  const hasPermission = await checkAndRequestMicrophonePermission();
  if (!hasPermission) return;

  const config = getConfig();

  updateConfig({
    recording: {
      ...config.recording,
      micEnabled: allInOneRecordingPreferences
        ? config.recording.micEnabled
        : true,
      selectedMicId: deviceId,
      selectedMicName: deviceName,
    },
  });

  updateAllInOneRecordingPreferences({ micEnabled: true });

  await syncControlSettings();
}

async function handleToggleCamera(): Promise<void> {
  if (recordingSession) {
    await applySessionCamera(recordingSession, !recordingSession.cameraEnabled);
    return;
  }

  const config = getConfig();
  const newCameraEnabled = allInOneRecordingPreferences
    ? !allInOneRecordingPreferences.cameraEnabled
    : !config.recording.camera?.enabled;

  if (newCameraEnabled) {
    const hasPermission = await checkAndRequestCameraPermission();
    if (!hasPermission) return;
  }

  const previewCamera = {
    ...config.recording.camera,
    enabled: newCameraEnabled,
  };

  if (newCameraEnabled) {
    await showCameraPreview(previewCamera);
  } else {
    hideCameraPreview();
  }

  if (allInOneRecordingPreferences) {
    updateAllInOneRecordingPreferences({ cameraEnabled: newCameraEnabled });
  } else {
    updateConfig({
      recording: {
        ...config.recording,
        camera: previewCamera,
      },
    });
  }

  await syncControlSettings();
}

async function handleSelectCamera(
  deviceId: string | null,
  deviceName: string | null
): Promise<void> {
  if (recordingSession) {
    await applySessionCamera(recordingSession, true);
    return;
  }

  const hasPermission = await checkAndRequestCameraPermission();
  if (!hasPermission) return;

  const config = getConfig();

  const previewCamera = {
    ...config.recording.camera,
    enabled: true,
    selectedDeviceId: deviceId,
    selectedDeviceName: deviceName,
  };

  await showCameraPreview(previewCamera);

  updateConfig({
    recording: {
      ...config.recording,
      camera: {
        ...previewCamera,
        enabled: allInOneRecordingPreferences
          ? config.recording.camera.enabled
          : true,
      },
    },
  });

  updateAllInOneRecordingPreferences({ cameraEnabled: true });

  await syncControlSettings();
}

async function handleStart(): Promise<void> {
  if (isHandlingStart) return;
  isHandlingStart = true;

  try {
    const config = getConfig();
    const settings = getRecordingSettings();
    await updateControlState({
      isStarting: true,
      isPaused: false,
    });
    await getRecordingActions().startPendingRecording({
      systemAudio: settings.systemAudio,
      micEnabled: settings.micEnabled,
      micDeviceId: settings.micEnabled ? settings.selectedMicId : null,
      micDeviceName: settings.micEnabled ? settings.selectedMicName : null,
      cameraEnabled: settings.cameraEnabled,
      cameraDeviceId: settings.selectedCameraId,
      cameraDeviceName: settings.selectedCameraName,
      keyboardEnabled: true,
      iosDeviceId: config.recording.iosDevice?.id ?? null,
      iosDeviceName: config.recording.iosDevice?.name ?? null,
    });
  } finally {
    try {
      await updateControlState({
        isStarting: false,
        isPaused,
      });
    } finally {
      isHandlingStart = false;
    }
  }
}

async function handleCancel(): Promise<void> {
  await getRecordingActions().cancelPendingRecording();
}

async function handlePause(): Promise<void> {
  await pauseRecording();
  pauseTimer();
}

async function handleResume(): Promise<void> {
  await resumeRecording();
  resumeTimer();
}

async function handleStop(): Promise<void> {
  await getRecordingActions().stopRecordingAction();
}

async function handleDelete(): Promise<void> {
  await getRecordingActions().deleteRecordingAction();
}

async function handleSelectIOSDevice(
  deviceId: string | null,
  deviceName: string | null
): Promise<void> {
  const config = getConfig();

  updateConfig({
    recording: {
      ...config.recording,
      iosDevice: deviceId
        ? {
            id: deviceId,
            name: deviceName,
          }
        : null,
    },
  });

  if (deviceId) {
    await hideAreaSelector();
  } else {
    await showAreaSelector();
  }

  await syncControlSettings();
}

async function syncControlSettings(): Promise<void> {
  const settings = getRecordingSettings();

  updateRecordingControlBrowserWindow({
    systemAudio: settings.systemAudio,
    micEnabled: settings.micEnabled,
    selectedMicId: settings.selectedMicId ?? null,
    cameraEnabled: settings.cameraEnabled,
    selectedCameraId: settings.selectedCameraId ?? null,
    selectedIOSDeviceId: settings.selectedIOSDeviceId,
    selectedIOSDeviceName: settings.selectedIOSDeviceName,
  });
}

let countdownInterval: ReturnType<typeof setInterval> | null = null;
let countdownResolve: ((result: 'completed' | 'cancelled') => void) | null =
  null;
let countdownEscapeRegistered = false;

function stopActiveCountdown(result: 'completed' | 'cancelled'): void {
  if (countdownInterval !== null) {
    clearInterval(countdownInterval);
    countdownInterval = null;
  }
  if (countdownEscapeRegistered) {
    globalShortcut.unregister('Escape');
    countdownEscapeRegistered = false;
  }

  updateRecordingControlBrowserWindow({ countdownSeconds: null });

  const resolve = countdownResolve;
  countdownResolve = null;
  resolve?.(result);
}

export function startRecordingCountdown(
  seconds: number
): Promise<'completed' | 'cancelled'> {
  stopActiveCountdown('cancelled');

  return new Promise<'completed' | 'cancelled'>(resolve => {
    let remaining = seconds;
    countdownResolve = resolve;
    countdownEscapeRegistered = globalShortcut.register('Escape', () => {
      stopActiveCountdown('cancelled');
    });
    updateRecordingControlBrowserWindow({ countdownSeconds: remaining });

    countdownInterval = setInterval(() => {
      remaining -= 1;
      if (remaining > 0) {
        updateRecordingControlBrowserWindow({ countdownSeconds: remaining });
        return;
      }
      stopActiveCountdown('completed');
    }, 1000);
  });
}

function handleBrowserAction(
  action: RecordingControlAction,
  data?: RecordingControlActionData
): void {
  if (action === 'cancel' && countdownInterval !== null) {
    stopActiveCountdown('cancelled');
    return;
  }

  void handleEvent(`recording-control:${action}`, data).catch(
    reportControlError
  );
}

function updateControlState(
  update: Partial<RecordingControlState>
): Promise<void> {
  updateRecordingControlBrowserWindow(update);
  return Promise.resolve();
}

function startTimer(): void {
  recordingStartTime = Date.now();
  isPaused = false;
  pausedElapsedTime = 0;

  sendElapsedTime();

  timerInterval = setInterval(() => {
    sendElapsedTime();
  }, 1000);
}

function stopTimer(): void {
  if (timerInterval) {
    clearInterval(timerInterval);
    timerInterval = null;
  }
  recordingStartTime = null;
  pausedElapsedTime = 0;
  isPaused = false;
}

export function pauseTimer(): void {
  if (isPaused || recordingStartTime === null) return;

  isPaused = true;
  pausedElapsedTime = Math.floor((Date.now() - recordingStartTime) / 1000);

  if (timerInterval) {
    clearInterval(timerInterval);
    timerInterval = null;
  }

  void updateControlState({ isPaused: true }).catch(error => {
    console.error('Failed to update paused control state:', error);
  });
}

export function resumeTimer(): void {
  if (!isPaused) return;

  isPaused = false;
  recordingStartTime = Date.now() - pausedElapsedTime * 1000;

  timerInterval = setInterval(() => {
    sendElapsedTime();
  }, 1000);

  sendElapsedTime();

  void updateControlState({ isPaused: false }).catch(error => {
    console.error('Failed to update resumed control state:', error);
  });
}

function sendElapsedTime(): void {
  if (recordingStartTime === null) return;

  const elapsedMs = Date.now() - recordingStartTime;
  const elapsedSeconds = Math.floor(elapsedMs / 1000);

  updateRecordingControlBrowserWindow({ elapsedSeconds });
}

export function prewarmRecordingControlWindow(): void {
  prewarmRecordingControlBrowserWindow();
}

export function showPreRecordingControl(
  area?: {
    x: number;
    y: number;
    width: number;
    height: number;
  },
  targetName?: string,
  options?: { preferenceScope?: 'global' | 'all-in-one' }
): void {
  recordingTarget = targetName ?? null;
  currentMode = 'pre-recording';

  const config = getConfig();
  allInOneRecordingPreferences =
    options?.preferenceScope === 'all-in-one'
      ? {
          ...(config.allInOne.rememberChoices
            ? config.allInOne.recording
            : DEFAULT_ALL_IN_ONE_CONFIG.recording),
        }
      : null;
  const settings = getRecordingSettings();
  if (settings.cameraEnabled) {
    const cameraSettings = area
      ? {
          ...config.recording.camera,
          enabled: true,
          position: calculateCameraPreviewPosition(area),
        }
      : { ...config.recording.camera, enabled: true };
    void showCameraPreview(cameraSettings).catch(async error => {
      if (allInOneRecordingPreferences) {
        updateAllInOneRecordingPreferences({ cameraEnabled: false });
      } else {
        const current = getConfig();
        updateConfig({
          recording: {
            ...current.recording,
            camera: {
              ...current.recording.camera,
              enabled: false,
            },
          },
        });
      }
      await syncControlSettings();
      await reportControlError(error);
    });
  }

  const position = calculateControlPosition(area);
  currentControlCenter = {
    x: position.x + getWindowWidth() / 2,
    y: position.y,
  };

  showRecordingControlBrowserWindow(
    getRecordingControlState(),
    position,
    handleBrowserAction
  );
}

export function updateRecordingControlPosition(area: {
  x: number;
  y: number;
  width: number;
  height: number;
}): void {
  if (getRecordingSettings().cameraEnabled) {
    void updateCameraPreviewPosition(
      calculateCameraPreviewPosition(area)
    ).catch(reportControlError);
  }

  const position = calculateControlPosition(area);
  currentControlCenter = {
    x: position.x + getWindowWidth() / 2,
    y: position.y,
  };

  updateRecordingControlBrowserWindowPosition(position);
}

export function detachRecordingControlFromOverlay(): void {
  clearRecordingControlBrowserWindowParent();
}

export async function hidePreRecordingControl(
  hideCamera: boolean = true
): Promise<void> {
  if (countdownInterval !== null) {
    stopActiveCountdown('cancelled');
  }

  recordingTarget = null;
  currentMode = 'pre-recording';

  if (!hideCamera) return;

  allInOneRecordingPreferences = null;
  hideCameraPreview();
  hideRecordingControlBrowserWindow();
}

function createRecordingSession(config: RecordingConfig): RecordingSession {
  const cameraSettings =
    getCameraPreviewSettings() ?? getConfig().recording.camera;
  return {
    systemAudio: config.includeAudio ?? true,
    micEnabled: config.micEnabled ?? false,
    selectedMicId: config.micDeviceId ?? null,
    selectedMicName: config.micDeviceName ?? null,
    cameraEnabled: config.cameraEnabled ?? false,
    camera: cameraSettings
      ? {
          ...cameraSettings,
          selectedDeviceId: config.cameraDeviceId ?? null,
          selectedDeviceName: config.cameraDeviceName ?? null,
        }
      : undefined,
  };
}

export async function showRecordingControl(
  config: RecordingConfig
): Promise<void> {
  currentMode = 'recording';
  recordingTarget = config.windowName ?? null;
  recordingSession = createRecordingSession(config);
  allInOneRecordingPreferences = null;

  const position = {
    x: Math.round(currentControlCenter.x - getWindowWidth() / 2),
    y: currentControlCenter.y,
  };
  showRecordingControlBrowserWindow(
    getRecordingControlState(),
    position,
    handleBrowserAction
  );
  startTimer();
}

export async function hideRecordingControl(): Promise<void> {
  stopTimer();
  recordingSession = null;
  allInOneRecordingPreferences = null;

  recordingTarget = null;
  currentMode = 'pre-recording';

  hideRecordingControlBrowserWindow();
}
